use std::{
    net::SocketAddr,
    sync::{Arc, Mutex},
};

use axum::{
    Router, body::Body, extract::State, http::HeaderMap, response::Response, routing::post,
};
use goat_gateway::{
    App,
    oauth::Stored,
    provider::Catalog,
    store::{AccountState, Store},
};
use serde_json::json;

const SSE: &str = "event: message_start\ndata: {\"type\":\"message_start\"}\n\nevent: message_stop\ndata: {\"type\":\"message_stop\"}\n\n";
const THIRD_PARTY: &str = r#"{"type":"error","error":{"type":"invalid_request_error","message":"Third-party apps now draw from your extra usage, not your plan limits."}}"#;

async fn serve(router: Router) -> SocketAddr {
    let listener = tokio::net::TcpListener::bind(SocketAddr::from(([127, 0, 0, 1], 0)))
        .await
        .unwrap();
    let addr = listener.local_addr().unwrap();
    tokio::spawn(async move {
        axum::serve(listener, router).await.unwrap();
    });
    addr
}

#[derive(Clone, Default)]
struct Seen {
    calls: Arc<Mutex<Vec<HeaderMap>>>,
}

impl Seen {
    fn credentials(&self) -> Vec<String> {
        self.calls
            .lock()
            .unwrap()
            .iter()
            .map(|headers| {
                headers
                    .get("authorization")
                    .or_else(|| headers.get("x-api-key"))
                    .and_then(|value| value.to_str().ok())
                    .unwrap_or_default()
                    .to_owned()
            })
            .collect()
    }
}

async fn anthropic(State(seen): State<Seen>, headers: HeaderMap) -> Response {
    let plan = headers
        .get("authorization")
        .and_then(|value| value.to_str().ok())
        .is_some_and(|value| value == "Bearer at_ana_plan");
    seen.calls.lock().unwrap().push(headers);
    if plan {
        return Response::builder()
            .status(400)
            .header("content-type", "application/json")
            .body(Body::from(THIRD_PARTY))
            .unwrap();
    }
    Response::builder()
        .status(200)
        .header("content-type", "text/event-stream")
        .body(Body::from(SSE))
        .unwrap()
}

fn catalog(upstream: SocketAddr, carried_host: &str) -> Catalog {
    let mut overlay = Catalog::builtin_text().to_owned();
    overlay = overlay.replace(
        "https://api.anthropic.com/v1/messages",
        &format!("http://{upstream}/v1/messages"),
    );
    overlay = overlay.replace(
        "hosts = [\"api.anthropic.com\"]",
        &format!("hosts = [\"{carried_host}\"]"),
    );
    Catalog::with_overlay(&overlay).unwrap()
}

struct Harness {
    gateway: SocketAddr,
    store: Store,
    seen: Seen,
    ana_key: String,
    ana_key_id: String,
}

async fn harness(carried_host: &str, accounts: bool) -> Harness {
    let seen = Seen::default();
    let upstream = serve(
        Router::new()
            .route("/v1/messages", post(anthropic))
            .with_state(seen.clone()),
    )
    .await;

    let store = Store::in_memory(&[7u8; 32]).unwrap();
    let ana = store.add_user("ana").unwrap();
    let mino = store.add_user("mino").unwrap();
    let issued = store.issue_key(&ana.id, "pi").unwrap();
    if accounts {
        let plan = Stored {
            access: "at_ana_plan".into(),
            refresh: None,
            expires_at: None,
            account_id: None,
            client_id: None,
        };
        store
            .add_account("Ana Max", "anthropic", "oauth", &plan.to_bytes())
            .unwrap();
        store.set_owner("Ana Max", Some(&ana.id)).unwrap();
        let mino_plan = Stored {
            access: "at_mino_plan".into(),
            ..plan
        };
        store
            .add_account("Mino Max", "anthropic", "oauth", &mino_plan.to_bytes())
            .unwrap();
        store.set_owner("Mino Max", Some(&mino.id)).unwrap();
        store
            .add_account("Team key", "anthropic", "api_key", b"sk-ant-team")
            .unwrap();
    }

    let app = App::new(store.clone(), [1u8; 32], catalog(upstream, carried_host));
    let gateway = serve(app.router()).await;
    Harness {
        gateway,
        store,
        seen,
        ana_key: issued.secret,
        ana_key_id: issued.id,
    }
}

impl Harness {
    async fn send(&self, extra: &[(&str, &str)], use_gateway_key_header: bool) -> u16 {
        let mut request = reqwest::Client::new()
            .post(format!("http://{}/v1/messages", self.gateway))
            .json(&json!({
                "model": "claude-sonnet-5",
                "max_tokens": 8,
                "stream": true,
                "messages": [{ "role": "user", "content": "hi" }],
            }));
        request = if use_gateway_key_header {
            request.header("x-gateway-key", &self.ana_key)
        } else {
            request.header("x-api-key", &self.ana_key)
        };
        for (name, value) in extra {
            request = request.header(*name, *value);
        }
        let response = request.send().await.unwrap();
        let status = response.status().as_u16();
        response.bytes().await.unwrap();
        status
    }
}

#[tokio::test]
async fn a_client_carrying_its_own_login_is_served_with_it_and_nothing_stored() {
    let harness = harness("127.0.0.1", false).await;
    let status = harness
        .send(&[("authorization", "Bearer sk-ant-oat01-own")], true)
        .await;
    assert_eq!(status, 200);

    assert_eq!(harness.seen.credentials(), ["Bearer sk-ant-oat01-own"]);
    let headers = harness.seen.calls.lock().unwrap()[0].clone();
    assert!(
        !headers.contains_key("x-gateway-key"),
        "our own key must never reach the provider"
    );

    let row = &harness.store.recent_requests(1).unwrap()[0];
    assert_eq!(row.credential.as_deref(), Some("carried"));
    assert_eq!(row.account, None);
    assert!(
        harness.store.accounts().unwrap().is_empty(),
        "a carried login is used, never kept"
    );
}

#[tokio::test]
async fn a_login_nobody_declared_is_refused_and_sent_nowhere() {
    let harness = harness("127.0.0.1", true).await;
    let status = harness
        .send(&[("authorization", "Bearer something-unheard-of")], true)
        .await;
    assert_eq!(status, 401);
    assert!(harness.seen.credentials().is_empty());
}

#[tokio::test]
async fn a_carried_login_never_goes_to_a_host_it_was_not_issued_for() {
    let harness = harness("api.anthropic.com", true).await;
    let status = harness
        .send(&[("authorization", "Bearer sk-ant-oat01-own")], true)
        .await;
    assert_eq!(status, 200);
    assert!(
        !harness
            .seen
            .credentials()
            .iter()
            .any(|sent| sent.contains("sk-ant-oat01-own")),
        "the declared host is api.anthropic.com, so a repointed endpoint must not receive it"
    );
    let row = &harness.store.recent_requests(1).unwrap()[0];
    assert!(
        row.attempts
            .as_ref()
            .is_some_and(|attempts| attempts.to_string().contains("set aside")),
        "{:?}",
        row.attempts
    );
}

#[tokio::test]
async fn a_refused_request_moves_to_the_next_account_and_parks_nothing() {
    let harness = harness("127.0.0.1", true).await;
    let status = harness.send(&[], false).await;
    assert_eq!(
        status, 200,
        "the shared key should have answered after the refusal"
    );

    assert_eq!(
        harness.seen.credentials(),
        ["Bearer at_ana_plan", "sk-ant-team"],
        "her own plan first, then the shared key; never mino's plan"
    );
    let accounts = harness.store.accounts().unwrap();
    let ana = accounts
        .iter()
        .find(|account| account.name == "Ana Max")
        .unwrap();
    assert_eq!(
        ana.state,
        AccountState::Active,
        "a refusal of one program's request says nothing about the account"
    );

    let demotions = harness.store.demotions(&harness.ana_key_id).unwrap();
    assert_eq!(demotions.len(), 1);
    assert_eq!(demotions[0].account, "Ana Max");

    let row = &harness.store.recent_requests(1).unwrap()[0];
    assert_eq!(row.account.as_deref(), Some("Team key"));
    assert_eq!(row.status, "ok");
    assert!(
        row.attempts
            .as_ref()
            .is_some_and(|attempts| attempts.to_string().contains("Ana Max"))
    );
    assert_eq!(
        harness.store.recent_requests(10).unwrap().len(),
        1,
        "one request is one row, however many accounts it took"
    );
}

#[tokio::test]
async fn after_a_refusal_the_key_tries_that_account_last() {
    let harness = harness("127.0.0.1", true).await;
    harness.send(&[], false).await;
    harness.seen.calls.lock().unwrap().clear();

    let status = harness.send(&[], false).await;
    assert_eq!(status, 200);
    assert_eq!(harness.seen.credentials(), ["sk-ant-team"]);
}

#[tokio::test]
async fn someone_elses_plan_never_serves_your_request() {
    let harness = harness("127.0.0.1", true).await;
    harness.store.remove_account("Ana Max").unwrap();
    harness.store.remove_account("Team key").unwrap();

    let status = harness.send(&[], false).await;
    assert_ne!(status, 200);
    assert!(
        harness.seen.credentials().is_empty(),
        "mino's plan exists, and ana's request must still not reach it"
    );
}
