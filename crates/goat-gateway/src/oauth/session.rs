use std::{
    collections::HashMap,
    sync::{Arc, Mutex},
};

use crate::{
    oauth::{
        Client, Mode, Pkce, Stored, account::KIND, authorize_url, flow, listen, random_state,
        split_pasted,
    },
    store::{Store, now},
};

const ABANDONED_AFTER_MS: i64 = 20 * 60 * 1_000;

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
#[serde(tag = "status", rename_all = "snake_case")]
pub enum Status {
    Waiting,
    Done,
    Failed { message: String },
}

#[derive(Debug, Clone)]
struct Session {
    account: String,
    provider: &'static str,
    mode: Mode,
    verifier: String,
    state: String,
    redirect_uri: String,
    opened_at: i64,
    status: Status,
}

#[derive(Clone, Default)]
pub struct Sessions {
    live: Arc<Mutex<HashMap<String, Session>>>,
}

impl std::fmt::Debug for Sessions {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("Sessions")
    }
}

#[derive(Debug, Clone, serde::Serialize)]
pub struct Started {
    pub session: String,
    pub mode: Mode,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub authorize_url: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub user_code: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub verification_url: Option<String>,
}

#[derive(Debug, thiserror::Error)]
pub enum SignInError {
    #[error("this build has no sign-in flow for {0:?}")]
    UnknownProvider(String),
    #[error("{provider} does not offer {mode:?} sign-in")]
    Unsupported { provider: String, mode: Mode },
    #[error("give the account a name before signing in")]
    Unnamed,
    #[error("that sign-in is not open anymore. Start again")]
    Unknown,
    #[error("{0}")]
    Local(String),
    #[error(transparent)]
    Oauth(#[from] crate::oauth::OauthError),
    #[error(transparent)]
    Store(#[from] crate::store::StoreError),
}

impl Sessions {
    pub fn status(&self, session: &str) -> Option<Status> {
        self.live
            .lock()
            .expect("sessions mutex")
            .get(session)
            .map(|open| open.status.clone())
    }

    fn open(&self, session: String, entry: Session) {
        let mut live = self.live.lock().expect("sessions mutex");
        live.retain(|_, open| now() - open.opened_at < ABANDONED_AFTER_MS);
        live.insert(session, entry);
    }

    fn read(&self, session: &str) -> Option<Session> {
        self.live
            .lock()
            .expect("sessions mutex")
            .get(session)
            .cloned()
    }

    fn settle(&self, session: &str, status: Status) {
        if let Some(open) = self.live.lock().expect("sessions mutex").get_mut(session) {
            open.status = status;
        }
    }
}

pub async fn begin(
    store: &Store,
    client: &Client,
    sessions: &Sessions,
    provider: &str,
    account: &str,
    mode: Mode,
) -> Result<Started, SignInError> {
    let account = account.trim().to_owned();
    if account.is_empty() {
        return Err(SignInError::Unnamed);
    }
    let declared =
        flow(provider).ok_or_else(|| SignInError::UnknownProvider(provider.to_owned()))?;
    if !declared.supports(mode) {
        return Err(SignInError::Unsupported {
            provider: provider.to_owned(),
            mode,
        });
    }

    let session = crate::request_id();

    if mode == Mode::Device {
        let start = client.start_device(declared).await?;
        sessions.open(
            session.clone(),
            Session {
                account: account.clone(),
                provider: declared.provider,
                mode,
                verifier: String::new(),
                state: String::new(),
                redirect_uri: String::new(),
                opened_at: now(),
                status: Status::Waiting,
            },
        );

        let started = Started {
            session: session.clone(),
            mode,
            authorize_url: None,
            user_code: Some(start.user_code.clone()),
            verification_url: Some(start.verification_url.clone()),
        };

        let store = store.clone();
        let client = client.clone();
        let sessions = sessions.clone();
        tokio::spawn(async move {
            let outcome = match client.finish_device(declared, &start).await {
                Ok(tokens) => save(&store, &account, declared.provider, &tokens),
                Err(error) => Err(SignInError::Oauth(error)),
            };
            sessions.settle(&session, finished(outcome));
        });

        return Ok(started);
    }

    let pkce = Pkce::generate();
    let state = if declared.sends_state {
        pkce.verifier.clone()
    } else {
        random_state()
    };

    let (redirect_uri, arrived) = match mode {
        Mode::Loopback => {
            let declared_loopback = declared.loopback.expect("checked by supports");
            let listening = listen(declared_loopback.port, declared_loopback.fallback_port)
                .await
                .map_err(|error| SignInError::Local(error.to_string()))?;
            let uri = declared
                .redirect_uri(mode, Some(listening.port))
                .expect("loopback declares a redirect");
            (uri, Some(listening.arrived))
        }
        Mode::Paste => (
            declared
                .redirect_uri(mode, None)
                .expect("paste declares a redirect"),
            None,
        ),
        Mode::Device => unreachable!("handled above"),
    };

    let url = authorize_url(declared, &redirect_uri, &pkce.challenge, &state);
    sessions.open(
        session.clone(),
        Session {
            account: account.clone(),
            provider: declared.provider,
            mode,
            verifier: pkce.verifier,
            state: state.clone(),
            redirect_uri: redirect_uri.clone(),
            opened_at: now(),
            status: Status::Waiting,
        },
    );

    if let Some(arrived) = arrived {
        let store = store.clone();
        let client = client.clone();
        let sessions_for_task = sessions.clone();
        let session_for_task = session.clone();
        let Some(open) = sessions.read(&session) else {
            return Err(SignInError::Unknown);
        };
        tokio::spawn(async move {
            let outcome = match arrived.await {
                Ok(callback) => redeem(&store, &client, &open, callback).await,
                Err(_) => Err(SignInError::Local(
                    "the browser never came back to this machine".to_owned(),
                )),
            };
            sessions_for_task.settle(&session_for_task, finished(outcome));
        });
    }

    Ok(Started {
        session,
        mode,
        authorize_url: Some(url),
        user_code: None,
        verification_url: None,
    })
}

pub async fn finish_paste(
    store: &Store,
    client: &Client,
    sessions: &Sessions,
    session: &str,
    pasted: &str,
) -> Result<(), SignInError> {
    let open = sessions.read(session).ok_or(SignInError::Unknown)?;
    if open.mode != Mode::Paste {
        return Err(SignInError::Local(
            "this sign-in is waiting for the browser to come back, not for a pasted code"
                .to_owned(),
        ));
    }
    let declared = flow(open.provider).ok_or(SignInError::Unknown)?;

    let (code, pasted_state) = split_pasted(pasted);
    if code.is_empty() {
        let error = SignInError::Local("that does not look like an authorization code".to_owned());
        sessions.settle(session, failed(&error));
        return Err(error);
    }
    let state = pasted_state.unwrap_or_else(|| open.state.clone());

    let outcome = client
        .exchange(declared, &code, &state, &open.verifier, &open.redirect_uri)
        .await
        .map_err(SignInError::Oauth)
        .and_then(|tokens| save(store, &open.account, declared.provider, &tokens));

    sessions.settle(
        session,
        match &outcome {
            Ok(()) => Status::Done,
            Err(error) => failed(error),
        },
    );
    outcome
}

async fn redeem(
    store: &Store,
    client: &Client,
    open: &Session,
    callback: crate::oauth::Callback,
) -> Result<(), SignInError> {
    if let Some(error) = callback.error {
        return Err(SignInError::Local(error));
    }
    let Some(code) = callback.code else {
        return Err(SignInError::Local(
            "the provider came back without an authorization code".to_owned(),
        ));
    };
    if let Some(returned) = callback.state.as_deref()
        && returned != open.state
    {
        return Err(SignInError::Local(
            "the sign-in that came back is not the one that started here".to_owned(),
        ));
    }

    let declared = flow(open.provider).ok_or(SignInError::Unknown)?;
    let tokens = client
        .exchange(
            declared,
            &code,
            &open.state,
            &open.verifier,
            &open.redirect_uri,
        )
        .await?;
    save(store, &open.account, declared.provider, &tokens)
}

fn save(
    store: &Store,
    account: &str,
    provider: &str,
    tokens: &crate::oauth::Tokens,
) -> Result<(), SignInError> {
    store.add_account(
        account,
        provider,
        KIND,
        &Stored::from_tokens(tokens).to_bytes(),
    )?;
    Ok(())
}

fn finished(outcome: Result<(), SignInError>) -> Status {
    match outcome {
        Ok(()) => Status::Done,
        Err(error) => failed(&error),
    }
}

fn failed(error: &SignInError) -> Status {
    Status::Failed {
        message: error.to_string(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::oauth::{ANTHROPIC, OPENAI};

    fn parts() -> (Store, Client, Sessions) {
        (
            Store::in_memory(&[5u8; 32]).unwrap(),
            Client::new(),
            Sessions::default(),
        )
    }

    #[tokio::test]
    async fn a_paste_sign_in_hands_back_a_url_without_touching_the_network() {
        let (store, client, sessions) = parts();
        let started = begin(
            &store,
            &client,
            &sessions,
            "anthropic",
            "Claude 개인",
            Mode::Paste,
        )
        .await
        .unwrap();

        let url = started.authorize_url.unwrap();
        assert!(url.starts_with(ANTHROPIC.authorize_url));
        assert!(url.contains("code_challenge_method=S256"));
        assert_eq!(sessions.status(&started.session), Some(Status::Waiting));
    }

    #[tokio::test]
    async fn a_nameless_account_is_refused_before_any_browser_opens() {
        let (store, client, sessions) = parts();
        let error = begin(&store, &client, &sessions, "anthropic", "   ", Mode::Paste)
            .await
            .unwrap_err();
        assert!(matches!(error, SignInError::Unnamed));
    }

    #[tokio::test]
    async fn asking_a_provider_for_a_mode_it_lacks_is_refused_by_name() {
        let (store, client, sessions) = parts();
        let error = begin(&store, &client, &sessions, "openai", "ChatGPT", Mode::Paste)
            .await
            .unwrap_err();
        assert!(matches!(error, SignInError::Unsupported { .. }));
        assert!(OPENAI.paste.is_none());
    }

    #[tokio::test]
    async fn an_unknown_provider_is_named_in_the_refusal() {
        let (store, client, sessions) = parts();
        let error = begin(&store, &client, &sessions, "mistral", "M", Mode::Paste)
            .await
            .unwrap_err();
        assert!(error.to_string().contains("mistral"));
    }

    #[tokio::test]
    async fn anthropic_reuses_the_verifier_as_state_and_openai_does_not() {
        let (store, client, sessions) = parts();
        let anthropic = begin(&store, &client, &sessions, "anthropic", "a", Mode::Paste)
            .await
            .unwrap();
        let open = sessions.read(&anthropic.session).unwrap();
        assert_eq!(open.state, open.verifier);
        assert!(anthropic.authorize_url.unwrap().contains(&open.verifier));
    }

    #[tokio::test]
    async fn a_pasted_code_is_refused_on_a_session_that_is_waiting_for_the_browser() {
        let (store, client, sessions) = parts();
        let started = begin(&store, &client, &sessions, "anthropic", "a", Mode::Loopback)
            .await
            .unwrap();

        let error = finish_paste(&store, &client, &sessions, &started.session, "ac#st")
            .await
            .unwrap_err();
        assert!(error.to_string().contains("not for a pasted code"));
    }

    #[tokio::test]
    async fn a_finish_against_a_stale_session_says_to_start_again() {
        let (store, client, sessions) = parts();
        let error = finish_paste(&store, &client, &sessions, "no-such-session", "code#state")
            .await
            .unwrap_err();
        assert!(matches!(error, SignInError::Unknown));
    }

    #[tokio::test]
    async fn a_callback_from_a_different_sign_in_is_rejected() {
        let open = Session {
            account: "a".into(),
            provider: "anthropic",
            mode: Mode::Loopback,
            verifier: "v".into(),
            state: "expected".into(),
            redirect_uri: "http://localhost:54545/callback".into(),
            opened_at: now(),
            status: Status::Waiting,
        };
        let (store, client, _) = parts();
        let error = redeem(
            &store,
            &client,
            &open,
            crate::oauth::Callback {
                code: Some("ac".into()),
                state: Some("someone-else".into()),
                error: None,
            },
        )
        .await
        .unwrap_err();
        assert!(error.to_string().contains("not the one that started here"));
    }

    #[tokio::test]
    async fn a_provider_refusal_at_the_callback_is_carried_verbatim() {
        let open = Session {
            account: "a".into(),
            provider: "anthropic",
            mode: Mode::Loopback,
            verifier: "v".into(),
            state: "s".into(),
            redirect_uri: "r".into(),
            opened_at: now(),
            status: Status::Waiting,
        };
        let (store, client, _) = parts();
        let error = redeem(
            &store,
            &client,
            &open,
            crate::oauth::Callback {
                code: None,
                state: None,
                error: Some("consent was declined".into()),
            },
        )
        .await
        .unwrap_err();
        assert_eq!(error.to_string(), "consent was declined");
    }

    #[tokio::test]
    async fn a_finished_sign_in_leaves_an_account_the_pool_can_pick() {
        let (store, _, _) = parts();
        let tokens = crate::oauth::Tokens {
            access: "at".into(),
            refresh: Some("rt".into()),
            id_token: None,
            expires_at: Some(now() + 3_600_000),
            account_id: None,
        };
        save(&store, "Claude 개인", "anthropic", &tokens).unwrap();

        let accounts = store.accounts().unwrap();
        assert_eq!(accounts.len(), 1);
        assert_eq!(accounts[0].name, "Claude 개인");
        assert_eq!(accounts[0].credential_kind, KIND);
    }

    #[tokio::test]
    async fn an_abandoned_sign_in_does_not_pile_up_forever() {
        let (store, client, sessions) = parts();
        let stale = begin(&store, &client, &sessions, "anthropic", "a", Mode::Paste)
            .await
            .unwrap();

        if let Some(open) = sessions.live.lock().unwrap().get_mut(&stale.session) {
            open.opened_at = now() - ABANDONED_AFTER_MS - 1;
        }

        begin(&store, &client, &sessions, "anthropic", "b", Mode::Paste)
            .await
            .unwrap();
        assert_eq!(sessions.status(&stale.session), None);
        assert_eq!(sessions.live.lock().unwrap().len(), 1);
    }
}
