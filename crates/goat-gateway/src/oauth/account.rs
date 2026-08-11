use crate::{
    oauth::{Client, Flow, OauthError, Tokens, flow},
    store::{Store, StoreError, now},
    upstream::{Auth, Oauth},
};

pub const KIND: &str = "oauth";
const SKEW_MS: i64 = 60_000;

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct Stored {
    pub access: String,
    #[serde(default)]
    pub refresh: Option<String>,
    #[serde(default)]
    pub expires_at: Option<i64>,
    #[serde(default)]
    pub account_id: Option<String>,
}

impl Stored {
    pub fn from_tokens(tokens: &Tokens) -> Self {
        Self {
            access: tokens.access.clone(),
            refresh: tokens.refresh.clone(),
            expires_at: tokens.expires_at,
            account_id: tokens.account_id.clone(),
        }
    }

    pub fn stale_at(&self, instant: i64) -> bool {
        self.expires_at
            .is_some_and(|expiry| instant + SKEW_MS >= expiry)
    }

    pub fn to_bytes(&self) -> Vec<u8> {
        serde_json::to_vec(self).unwrap_or_default()
    }
}

#[derive(Debug, thiserror::Error)]
pub enum PrepareError {
    #[error("{account:?} holds a credential this build does not know how to use: {kind:?}")]
    UnknownKind { account: String, kind: String },
    #[error("{account:?} is signed in to {provider:?}, which this build has no sign-in flow for")]
    UnknownProvider { account: String, provider: String },
    #[error("{account:?} holds a damaged sign-in and must be signed in again")]
    Damaged { account: String },
    #[error(
        "{account:?} could not renew its sign-in and must be signed in again: {source}"
    )]
    Expired {
        account: String,
        #[source]
        source: OauthError,
    },
    #[error(transparent)]
    Store(#[from] StoreError),
}

#[derive(Debug)]
pub struct Prepared {
    pub auth: Auth,
    pub base_url: Option<&'static str>,
}

pub async fn prepare(
    store: &Store,
    client: &Client,
    account: &str,
    provider: &str,
    credential_kind: &str,
    secret: &[u8],
) -> Result<Prepared, PrepareError> {
    match credential_kind {
        "api_key" => Ok(Prepared {
            auth: Auth::ApiKey(String::from_utf8_lossy(secret).into_owned()),
            base_url: None,
        }),
        KIND => {
            let declared = flow(provider).ok_or_else(|| PrepareError::UnknownProvider {
                account: account.to_owned(),
                provider: provider.to_owned(),
            })?;
            let stored: Stored =
                serde_json::from_slice(secret).map_err(|_| PrepareError::Damaged {
                    account: account.to_owned(),
                })?;

            let fresh = renew_if_stale(store, client, declared, account, provider, stored).await?;
            Ok(Prepared {
                auth: Auth::Oauth(headers_for(declared, &fresh)),
                base_url: declared.base_url,
            })
        }
        other => Err(PrepareError::UnknownKind {
            account: account.to_owned(),
            kind: other.to_owned(),
        }),
    }
}

async fn renew_if_stale(
    store: &Store,
    client: &Client,
    declared: &'static Flow,
    account: &str,
    provider: &str,
    stored: Stored,
) -> Result<Stored, PrepareError> {
    if !stored.stale_at(now()) {
        return Ok(stored);
    }
    let Some(refresh) = stored.refresh.clone() else {
        return Ok(stored);
    };

    let tokens = client.refresh(declared, &refresh).await.map_err(|source| {
        PrepareError::Expired {
            account: account.to_owned(),
            source,
        }
    })?;

    let mut renewed = Stored::from_tokens(&tokens);
    if renewed.account_id.is_none() {
        renewed.account_id = stored.account_id;
    }
    store.add_account(account, provider, KIND, &renewed.to_bytes())?;
    Ok(renewed)
}

fn headers_for(declared: &'static Flow, stored: &Stored) -> Oauth {
    let mut extra = Vec::new();
    if let (Some(identity), Some(account_id)) = (declared.identity, stored.account_id.as_deref()) {
        extra.push((identity.header.to_owned(), account_id.to_owned()));
    }
    Oauth {
        token: stored.access.clone(),
        extra,
        merge_beta: declared.merge_beta.map(str::to_owned),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::oauth::{ANTHROPIC, OPENAI};

    fn store() -> Store {
        Store::in_memory(&[9u8; 32]).unwrap()
    }

    #[tokio::test]
    async fn an_api_key_account_is_untouched_by_the_oauth_path() {
        let prepared = prepare(
            &store(),
            &Client::new(),
            "personal",
            "anthropic",
            "api_key",
            b"sk-ant-secret",
        )
        .await
        .unwrap();
        assert!(matches!(prepared.auth, Auth::ApiKey(key) if key == "sk-ant-secret"));
        assert_eq!(prepared.base_url, None);
    }

    #[tokio::test]
    async fn a_live_token_is_used_without_calling_the_provider() {
        let stored = Stored {
            access: "at_live".into(),
            refresh: Some("rt".into()),
            expires_at: Some(now() + 3_600_000),
            account_id: Some("acct_1".into()),
        };
        let prepared = prepare(
            &store(),
            &Client::new(),
            "chatgpt",
            "openai",
            KIND,
            &stored.to_bytes(),
        )
        .await
        .unwrap();

        let Auth::Oauth(oauth) = prepared.auth else {
            panic!("expected an oauth credential");
        };
        assert_eq!(oauth.token, "at_live");
        assert_eq!(
            oauth.extra,
            vec![("chatgpt-account-id".to_owned(), "acct_1".to_owned())]
        );
        assert_eq!(prepared.base_url, OPENAI.base_url);
    }

    #[tokio::test]
    async fn anthropic_carries_the_oauth_beta_and_no_account_header() {
        let stored = Stored {
            access: "at".into(),
            refresh: None,
            expires_at: None,
            account_id: Some("ignored".into()),
        };
        let prepared = prepare(
            &store(),
            &Client::new(),
            "claude",
            "anthropic",
            KIND,
            &stored.to_bytes(),
        )
        .await
        .unwrap();

        let Auth::Oauth(oauth) = prepared.auth else {
            panic!("expected an oauth credential");
        };
        assert_eq!(oauth.merge_beta.as_deref(), ANTHROPIC.merge_beta);
        assert!(
            oauth.extra.is_empty(),
            "anthropic declares no identity header, so nothing should be invented for it"
        );
        assert_eq!(prepared.base_url, None);
    }

    #[tokio::test]
    async fn a_token_with_no_refresh_is_used_as_is_rather_than_failing_early() {
        let stored = Stored {
            access: "at_expired".into(),
            refresh: None,
            expires_at: Some(now() - 1),
            account_id: None,
        };
        let prepared = prepare(
            &store(),
            &Client::new(),
            "claude",
            "anthropic",
            KIND,
            &stored.to_bytes(),
        )
        .await
        .unwrap();
        assert!(
            matches!(prepared.auth, Auth::Oauth(oauth) if oauth.token == "at_expired"),
            "the provider decides whether a token is dead, not our clock"
        );
    }

    #[tokio::test]
    async fn a_damaged_credential_says_so_instead_of_being_sent_as_a_key() {
        let error = prepare(
            &store(),
            &Client::new(),
            "claude",
            "anthropic",
            KIND,
            b"not json",
        )
        .await
        .unwrap_err();
        assert!(matches!(error, PrepareError::Damaged { .. }));
    }

    #[test]
    fn staleness_is_judged_a_minute_early_so_a_call_never_starts_on_a_dying_token() {
        let stored = Stored {
            access: "at".into(),
            refresh: None,
            expires_at: Some(now() + 30_000),
            account_id: None,
        };
        assert!(stored.stale_at(now()));

        let comfortable = Stored {
            expires_at: Some(now() + 600_000),
            ..stored.clone()
        };
        assert!(!comfortable.stale_at(now()));
    }

    #[test]
    fn a_token_without_an_expiry_is_never_considered_stale() {
        let stored = Stored {
            access: "at".into(),
            refresh: Some("rt".into()),
            expires_at: None,
            account_id: None,
        };
        assert!(!stored.stale_at(now()));
    }
}
