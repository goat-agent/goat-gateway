use std::{
    net::SocketAddr,
    sync::{Arc, Mutex},
};

use axum::{
    Router,
    body::{Body, Bytes},
    extract::State,
    http::HeaderMap,
    response::Response,
    routing::post,
};
use goat_gateway::{App, provider::Catalog, store::Store};
use serde_json::{Value, json};

const RESPONSES_SSE: &[u8] = b"event: response.created\ndata: {\"type\":\"response.created\"}\n\n\
event: response.completed\ndata: {\"type\":\"response.completed\"}\n\n";

#[derive(Clone, Default)]
struct Seen {
    path: Arc<Mutex<Option<String>>>,
    body: Arc<Mutex<Option<Bytes>>>,
    headers: Arc<Mutex<Option<HeaderMap>>>,
}

async fn record(seen: &Seen, path: &str, headers: HeaderMap, body: Bytes) -> Response {
    *seen.path.lock().unwrap() = Some(path.to_owned());
    *seen.body.lock().unwrap() = Some(body);
    *seen.headers.lock().unwrap() = Some(headers);
    Response::builder()
        .status(200)
        .header("content-type", "text/event-stream")
        .body(Body::from(RESPONSES_SSE))
        .unwrap()
}

async fn responses(State(seen): State<Seen>, headers: HeaderMap, body: Bytes) -> Response {
    record(&seen, "/v1/responses", headers, body).await
}

async fn chat(State(seen): State<Seen>, headers: HeaderMap, body: Bytes) -> Response {
    record(&seen, "/v1/chat/completions", headers, body).await
}

async fn messages(State(seen): State<Seen>, headers: HeaderMap, body: Bytes) -> Response {
    record(&seen, "/v1/messages", headers, body).await
}

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

struct Harness {
    gateway: SocketAddr,
    seen: Seen,
    key: String,
}

impl Harness {
    async fn post(&self, path: &str, body: Value) -> (u16, String) {
        let response = reqwest::Client::new()
            .post(format!("http://{}{path}", self.gateway))
            .header("x-api-key", &self.key)
            .header("content-type", "application/json")
            .json(&body)
            .send()
            .await
            .unwrap();
        let status = response.status().as_u16();
        (status, response.text().await.unwrap())
    }

    fn reached(&self) -> Option<String> {
        self.seen.path.lock().unwrap().clone()
    }

    fn credential(&self) -> Option<String> {
        let headers = self.seen.headers.lock().unwrap().clone()?;
        headers
            .get("authorization")
            .or_else(|| headers.get("x-api-key"))
            .and_then(|value| value.to_str().ok())
            .map(str::to_owned)
    }
}

async fn harness(accounts: &[(&str, &str)]) -> Harness {
    let seen = Seen::default();
    let upstream = serve(
        Router::new()
            .route("/v1/responses", post(responses))
            .route("/v1/chat/completions", post(chat))
            .route("/v1/messages", post(messages))
            .with_state(seen.clone()),
    )
    .await;

    let store = Store::in_memory(&[7u8; 32]).unwrap();
    for (name, provider) in accounts {
        store
            .add_account(name, provider, "api_key", format!("key-{name}").as_bytes())
            .unwrap();
    }
    let user = store.add_user("jmo").unwrap();
    let key = store.issue_key(&user.id, "test").unwrap().secret;
    store.set_admin_key("gwa_test-admin").unwrap();

    let catalog = Catalog::builtin()
        .with_base_url("anthropic", &format!("http://{upstream}"))
        .with_base_url("openai", &format!("http://{upstream}"));
    let gateway = serve(App::new(store, [1u8; 32], catalog).router()).await;

    Harness { gateway, seen, key }
}

#[tokio::test]
async fn an_openai_account_actually_receives_requests() {
    let harness = harness(&[("work", "openai")]).await;

    let (status, body) = harness
        .post(
            "/v1/responses",
            json!({ "model": "gpt-5", "stream": true, "input": "hi" }),
        )
        .await;

    assert_eq!(status, 200, "{body}");
    assert_eq!(harness.reached().as_deref(), Some("/v1/responses"));
    assert_eq!(harness.credential().as_deref(), Some("Bearer key-work"));
}

#[tokio::test]
async fn a_format_the_provider_already_speaks_is_not_translated() {
    let harness = harness(&[("work", "openai")]).await;
    let sent = json!({ "model": "gpt-5", "stream": true, "input": "hi", "invented_today": 1 });

    harness.post("/v1/responses", sent.clone()).await;

    let received: Value =
        serde_json::from_slice(&harness.seen.body.lock().unwrap().clone().unwrap()).unwrap();
    assert_eq!(received, sent, "a same-format request must pass untouched");
}

#[tokio::test]
async fn chat_completions_reaches_a_provider_that_speaks_it() {
    let harness = harness(&[("work", "openai")]).await;

    let (status, body) = harness
        .post(
            "/v1/chat/completions",
            json!({ "model": "gpt-5", "messages": [{ "role": "user", "content": "hi" }] }),
        )
        .await;

    assert_eq!(status, 200, "{body}");
    assert_eq!(harness.reached().as_deref(), Some("/v1/chat/completions"));
}

#[tokio::test]
async fn a_model_picks_its_own_provider_when_several_are_registered() {
    let harness = harness(&[("personal", "anthropic"), ("work", "openai")]).await;

    harness
        .post(
            "/v1/messages",
            json!({ "model": "claude-sonnet-5", "max_tokens": 8 }),
        )
        .await;
    assert_eq!(
        harness.credential().as_deref(),
        Some("key-personal"),
        "anthropic takes its key in x-api-key"
    );

    harness
        .post(
            "/v1/responses",
            json!({ "model": "gpt-5", "stream": true, "input": "hi" }),
        )
        .await;
    assert_eq!(
        harness.credential().as_deref(),
        Some("Bearer key-work"),
        "openai takes the same kind of key as a bearer token"
    );
}

#[tokio::test]
async fn a_format_nobody_registered_serves_is_refused_before_anything_is_sent() {
    let harness = harness(&[("personal", "anthropic")]).await;

    let (status, body) = harness
        .post(
            "/v1/chat/completions",
            json!({ "model": "claude-sonnet-5", "messages": [] }),
        )
        .await;

    assert_eq!(status, 400);
    assert!(body.contains("Chat Completions"), "{body}");
    assert!(harness.reached().is_none());
}
