use std::{
    collections::HashMap,
    sync::{Arc, Mutex},
};

use crate::{
    oauth::{
        Asking, Callback, Client, Flow, Mode, Pkce, StateSource, Stored, account::KIND,
        authorize_url, client::Exchange, listen, random_state,
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
    flow: Flow,
    account: String,
    owner: String,
    mode: Mode,
    verifier: String,
    state: String,
    redirect_uri: String,
    client_id: String,
    first: bool,
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
    declared: &Flow,
    account: &str,
    owner: &str,
    mode: Mode,
) -> Result<Started, SignInError> {
    let account = account.trim().to_owned();
    let owner = owner.to_owned();
    if account.is_empty() {
        return Err(SignInError::Unnamed);
    }
    if !declared.supports(mode) {
        return Err(SignInError::Unsupported {
            provider: declared.provider.clone(),
            mode,
        });
    }

    let session = crate::request_id();
    let earlier = earlier_client(store, declared, &account);
    let first = earlier.is_none();
    let client_id = earlier.unwrap_or_else(|| declared.client_id.clone());

    if mode == Mode::Device {
        let start = client.start_device(declared).await?;
        let open = Session {
            flow: declared.clone(),
            account,
            owner,
            mode,
            verifier: String::new(),
            state: String::new(),
            redirect_uri: String::new(),
            client_id,
            first,
            opened_at: now(),
            status: Status::Waiting,
        };
        sessions.open(session.clone(), open.clone());

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
            let outcome = match client.finish_device(&open.flow, &start).await {
                Ok(tokens) => save(&store, &open, None, &tokens),
                Err(error) => Err(SignInError::Oauth(error)),
            };
            sessions.settle(&session, finished(outcome));
        });

        return Ok(started);
    }

    let pkce = Pkce::generate();
    let state = match declared.state {
        StateSource::Verifier => pkce.verifier.clone(),
        StateSource::Random => random_state(),
    };

    let (redirect_uri, arrived) = match (mode, &declared.loopback) {
        (Mode::Loopback, Some(loopback)) => {
            let listening = listen(loopback.port, loopback.fallback_port)
                .await
                .map_err(|error| SignInError::Local(error.to_string()))?;
            let uri = declared
                .redirect_uri(mode, Some(listening.port))
                .ok_or(SignInError::Unknown)?;
            (uri, Some(listening.arrived))
        }
        _ => (
            declared
                .redirect_uri(mode, None)
                .ok_or(SignInError::Unknown)?,
            None,
        ),
    };

    let install = store.install_id()?;
    let nonce = random_state();
    let url = authorize_url(
        declared,
        &client_id,
        &redirect_uri,
        &pkce.challenge,
        &state,
        &Asking {
            install: &install,
            nonce: &nonce,
            first,
        },
    );
    let open = Session {
        flow: declared.clone(),
        account,
        owner,
        mode,
        verifier: pkce.verifier,
        state,
        redirect_uri,
        client_id,
        first,
        opened_at: now(),
        status: Status::Waiting,
    };
    sessions.open(session.clone(), open.clone());

    if let Some(arrived) = arrived {
        let store = store.clone();
        let client = client.clone();
        let sessions = sessions.clone();
        let session = session.clone();
        tokio::spawn(async move {
            let outcome = match arrived.await {
                Ok(callback) => redeem(&store, &client, &open, callback).await,
                Err(_) => Err(SignInError::Local(
                    "the browser never came back to this machine".to_owned(),
                )),
            };
            sessions.settle(&session, finished(outcome));
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

fn earlier_client(store: &Store, declared: &Flow, account: &str) -> Option<String> {
    if !declared.issues_client {
        return None;
    }
    let same_provider = store
        .accounts()
        .ok()?
        .into_iter()
        .any(|row| row.name == account && row.provider == declared.provider);
    if !same_provider {
        return None;
    }
    serde_json::from_slice::<Stored>(&store.secret(account).ok()?)
        .ok()?
        .client_id
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

    let callback = Callback::pasted(pasted);
    if callback.code.is_none() && callback.error.is_none() {
        let error = SignInError::Local(
            "that does not look like an authorization code or the address the browser landed on"
                .to_owned(),
        );
        sessions.settle(session, failed(&error));
        return Err(error);
    }

    let outcome = redeem(store, client, &open, callback).await;
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
    callback: Callback,
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
    let issued = issued_client(open, callback.client_id)?;

    let tokens = client
        .exchange(
            &open.flow,
            Exchange {
                client_id: &issued,
                code: &code,
                state: &open.state,
                verifier: &open.verifier,
                redirect_uri: &open.redirect_uri,
            },
        )
        .await?;
    save(store, open, Some(issued), &tokens)
}

fn issued_client(open: &Session, returned: Option<String>) -> Result<String, SignInError> {
    if !open.flow.issues_client {
        return Ok(open.client_id.clone());
    }
    match (returned, open.first) {
        (Some(returned), true) => Ok(returned),
        (None, true) => Err(SignInError::Local(
            "the provider did not say which client it registered for this gateway, so the \
             registration is incomplete. Start the sign-in again"
                .to_owned(),
        )),
        (Some(returned), false) if returned != open.client_id => Err(SignInError::Local(
            "the provider answered for a different client than this account was registered \
             with, so the answer was refused"
                .to_owned(),
        )),
        (_, false) => Ok(open.client_id.clone()),
    }
}

fn save(
    store: &Store,
    open: &Session,
    issued: Option<String>,
    tokens: &crate::oauth::Tokens,
) -> Result<(), SignInError> {
    let stored = Stored::from_tokens(tokens).issued_by(issued.filter(|_| open.flow.issues_client));
    store.add_account(&open.account, &open.flow.provider, KIND, &stored.to_bytes())?;
    store.set_owner(&open.account, Some(&open.owner))?;
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

    fn parts() -> (Store, Client, Sessions) {
        (
            Store::in_memory(&[5u8; 32]).unwrap(),
            Client::new(),
            Sessions::default(),
        )
    }

    fn flow(provider: &str) -> Flow {
        crate::provider::Catalog::builtin()
            .flow(provider)
            .cloned()
            .unwrap()
    }

    fn waiting(provider: &str, first: bool) -> Session {
        Session {
            flow: flow(provider),
            account: "a".into(),
            owner: "usr_x".into(),
            mode: Mode::Loopback,
            verifier: "v".into(),
            state: "expected".into(),
            redirect_uri: "http://localhost:54545/callback".into(),
            client_id: if first {
                "dynamic_agent_client".into()
            } else {
                "oaiapp_saved".into()
            },
            first,
            opened_at: now(),
            status: Status::Waiting,
        }
    }

    #[tokio::test]
    async fn a_paste_sign_in_hands_back_a_url_without_touching_the_network() {
        let (store, client, sessions) = parts();
        let anthropic = flow("anthropic");
        let started = begin(
            &store,
            &client,
            &sessions,
            &anthropic,
            "Claude 개인",
            "usr_x",
            Mode::Paste,
        )
        .await
        .unwrap();

        let url = started.authorize_url.unwrap();
        assert!(url.starts_with(&anthropic.authorize_url));
        assert!(url.contains("code_challenge_method=S256"));
        assert_eq!(sessions.status(&started.session), Some(Status::Waiting));
    }

    #[tokio::test]
    async fn a_nameless_account_is_refused_before_any_browser_opens() {
        let (store, client, sessions) = parts();
        let error = begin(
            &store,
            &client,
            &sessions,
            &flow("anthropic"),
            "   ",
            "usr_x",
            Mode::Paste,
        )
        .await
        .unwrap_err();
        assert!(matches!(error, SignInError::Unnamed));
    }

    #[tokio::test]
    async fn asking_a_provider_for_a_mode_it_lacks_is_refused_by_name() {
        let (store, client, sessions) = parts();
        let error = begin(
            &store,
            &client,
            &sessions,
            &flow("anthropic"),
            "Claude",
            "usr_x",
            Mode::Device,
        )
        .await
        .unwrap_err();
        assert!(matches!(error, SignInError::Unsupported { .. }));
        assert!(error.to_string().contains("anthropic"));
    }

    #[tokio::test]
    async fn anthropic_reuses_the_verifier_as_state_and_openai_does_not() {
        let (store, client, sessions) = parts();
        let anthropic = begin(
            &store,
            &client,
            &sessions,
            &flow("anthropic"),
            "a",
            "usr_x",
            Mode::Paste,
        )
        .await
        .unwrap();
        let open = sessions.read(&anthropic.session).unwrap();
        assert_eq!(open.state, open.verifier);
        assert!(anthropic.authorize_url.unwrap().contains(&open.verifier));

        let chatgpt = begin(
            &store,
            &client,
            &sessions,
            &flow("chatgpt"),
            "b",
            "usr_x",
            Mode::Paste,
        )
        .await
        .unwrap();
        let open = sessions.read(&chatgpt.session).unwrap();
        assert_ne!(open.state, open.verifier);
    }

    #[tokio::test]
    async fn a_pasted_code_is_refused_on_a_session_that_is_waiting_for_the_browser() {
        let (store, client, sessions) = parts();
        let started = begin(
            &store,
            &client,
            &sessions,
            &flow("anthropic"),
            "a",
            "usr_x",
            Mode::Loopback,
        )
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
        let (store, client, _) = parts();
        let error = redeem(
            &store,
            &client,
            &waiting("anthropic", true),
            Callback {
                code: Some("ac".into()),
                state: Some("someone-else".into()),
                ..Callback::default()
            },
        )
        .await
        .unwrap_err();
        assert!(error.to_string().contains("not the one that started here"));
    }

    #[tokio::test]
    async fn a_provider_refusal_at_the_callback_is_carried_verbatim() {
        let (store, client, _) = parts();
        let error = redeem(
            &store,
            &client,
            &waiting("anthropic", true),
            Callback {
                error: Some("consent was declined".into()),
                ..Callback::default()
            },
        )
        .await
        .unwrap_err();
        assert_eq!(error.to_string(), "consent was declined");
    }

    #[test]
    fn a_first_registration_takes_the_client_the_provider_issued() {
        let open = waiting("chatgpt_signin", true);
        assert_eq!(
            issued_client(&open, Some("oaiapp_new".into())).unwrap(),
            "oaiapp_new"
        );
        assert!(
            issued_client(&open, None).is_err(),
            "without the issued client there is nothing to refresh with later"
        );
    }

    #[test]
    fn a_returning_sign_in_keeps_its_client_and_refuses_a_different_one() {
        let open = waiting("chatgpt_signin", false);
        assert_eq!(issued_client(&open, None).unwrap(), "oaiapp_saved");
        assert_eq!(
            issued_client(&open, Some("oaiapp_saved".into())).unwrap(),
            "oaiapp_saved"
        );
        assert!(issued_client(&open, Some("oaiapp_other".into())).is_err());
    }

    #[test]
    fn a_fixed_client_is_never_replaced_by_what_a_callback_says() {
        let open = waiting("anthropic", true);
        assert_eq!(
            issued_client(&open, Some("oaiapp_injected".into())).unwrap(),
            open.client_id
        );
    }

    #[tokio::test]
    async fn a_returning_official_sign_in_asks_with_the_client_it_was_issued() {
        let (store, client, sessions) = parts();
        let mino = store.add_user("mino").unwrap();
        let earlier = Stored {
            access: "at".into(),
            refresh: Some("rt".into()),
            expires_at: None,
            account_id: None,
            client_id: Some("oaiapp_mine".into()),
        };
        store
            .add_account("ChatGPT", "chatgpt_signin", KIND, &earlier.to_bytes())
            .unwrap();
        let started = begin(
            &store,
            &client,
            &sessions,
            &flow("chatgpt_signin"),
            "ChatGPT",
            &mino.id,
            Mode::Paste,
        )
        .await
        .unwrap();
        let url = started.authorize_url.unwrap();
        assert!(url.contains("client_id=oaiapp_mine"), "{url}");
        assert!(!url.contains("agent_name_hint"), "{url}");
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
            scope: None,
        };
        let mino = store.add_user("mino").unwrap();
        let mut open = waiting("anthropic", true);
        open.account = "Claude 개인".into();
        open.owner = mino.id.clone();
        save(&store, &open, None, &tokens).unwrap();

        let accounts = store.accounts().unwrap();
        assert_eq!(accounts.len(), 1);
        assert_eq!(accounts[0].name, "Claude 개인");
        assert_eq!(accounts[0].credential_kind, KIND);
        assert_eq!(
            accounts[0].owner.as_deref(),
            Some(mino.id.as_str()),
            "a plan belongs to the person who signed in to it"
        );
    }

    #[tokio::test]
    async fn an_abandoned_sign_in_does_not_pile_up_forever() {
        let (store, client, sessions) = parts();
        let anthropic = flow("anthropic");
        let stale = begin(
            &store,
            &client,
            &sessions,
            &anthropic,
            "a",
            "usr_x",
            Mode::Paste,
        )
        .await
        .unwrap();

        if let Some(open) = sessions.live.lock().unwrap().get_mut(&stale.session) {
            open.opened_at = now() - ABANDONED_AFTER_MS - 1;
        }

        begin(
            &store,
            &client,
            &sessions,
            &anthropic,
            "b",
            "usr_x",
            Mode::Paste,
        )
        .await
        .unwrap();
        assert_eq!(sessions.status(&stale.session), None);
        assert_eq!(sessions.live.lock().unwrap().len(), 1);
    }
}
