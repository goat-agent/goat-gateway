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
use goat_gateway::{App, store::Store};

const SSE: &[u8] = b"event: message_start\ndata: {\"type\":\"message_start\"}\n\n\
event: content_block_delta\ndata: {\"type\":\"content_block_delta\",\"delta\":{\"type\":\"thinking_delta\",\"thinking\":\"weighing\"}}\n\n\
event: message_stop\ndata: {\"type\":\"message_stop\"}\n\n";

#[derive(Clone, Default)]
struct Seen {
    body: Arc<Mutex<Option<Bytes>>>,
    headers: Arc<Mutex<Option<HeaderMap>>>,
}

async fn upstream_handler(State(seen): State<Seen>, headers: HeaderMap, body: Bytes) -> Response {
    *seen.body.lock().unwrap() = Some(body);
    *seen.headers.lock().unwrap() = Some(headers);
    Response::builder()
        .status(200)
        .header("content-type", "text/event-stream")
        .header("anthropic-ratelimit-unified-5h-utilization", "0.62")
        .header("request-id", "req_011CSHoEeq")
        .body(Body::from(SSE))
        .unwrap()
}

fn gateway_app(
    upstream: SocketAddr,
    envelope_key: [u8; 32],
    accounts: &[(&str, &[u8])],
) -> (App, String) {
    let store = Store::in_memory(&[7u8; 32]).unwrap();
    for (name, secret) in accounts {
        store
            .add_account(name, "anthropic", "api_key", secret)
            .unwrap();
    }
    let user = store.add_user("jmo").unwrap();
    let issued = store.issue_key(&user.id, "test").unwrap();
    store.set_admin_key("gwa_test-admin").unwrap();
    (
        App::new(store, envelope_key, catalog_at(upstream)),
        issued.secret,
    )
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
    app: App,
}

impl Harness {
    async fn send(&self, body: String) {
        reqwest::Client::new()
            .post(format!("http://{}/v1/messages", self.gateway))
            .header("x-api-key", &self.key)
            .header("content-type", "application/json")
            .body(body)
            .send()
            .await
            .unwrap();
    }

    fn serving_account(&self) -> String {
        let headers = self.seen.headers.lock().unwrap().clone().unwrap();
        headers["x-api-key"].to_str().unwrap().to_owned()
    }
}

async fn harness() -> Harness {
    build(&[("personal", b"provider-key")]).await
}

async fn build(accounts: &[(&str, &[u8])]) -> Harness {
    let seen = Seen::default();
    let upstream_addr = serve(
        Router::new()
            .route("/v1/messages", post(upstream_handler))
            .with_state(seen.clone()),
    )
    .await;

    let (app, key) = gateway_app(upstream_addr, [1u8; 32], accounts);
    let gateway = serve(app.clone().router()).await;

    Harness {
        gateway,
        seen,
        key,
        app,
    }
}

#[tokio::test]
async fn request_body_reaches_the_provider_byte_for_byte() {
    let harness = harness().await;

    let sent = concat!(
        "{ \"model\" : \"claude-sonnet-5\" ,\n",
        "  \"namespace\": \"collaboration\",\n",
        "  \"phase\": \"final_answer\",\n",
        "  \"some_field_shipped_after_we_were_written\": {\"nested\": [1, 2]},\n",
        "  \"max_tokens\" : 16 }"
    );

    let response = reqwest::Client::new()
        .post(format!("http://{}/v1/messages", harness.gateway))
        .header("x-api-key", &harness.key)
        .header("content-type", "application/json")
        .body(sent)
        .send()
        .await
        .unwrap();
    assert_eq!(response.status(), 200);

    let received = harness.seen.body.lock().unwrap().clone().unwrap();
    assert_eq!(
        received.as_ref(),
        sent.as_bytes(),
        "the provider must receive exactly what the client sent, whitespace included"
    );
}

#[tokio::test]
async fn response_stream_reaches_the_client_byte_for_byte() {
    let harness = harness().await;

    let response = reqwest::Client::new()
        .post(format!("http://{}/v1/messages", harness.gateway))
        .header("x-api-key", &harness.key)
        .body(r#"{"model":"claude-sonnet-5"}"#)
        .send()
        .await
        .unwrap();

    assert_eq!(response.headers()["content-type"], "text/event-stream");
    assert_eq!(response.headers()["request-id"], "req_011CSHoEeq");
    assert_eq!(
        response.headers()["anthropic-ratelimit-unified-5h-utilization"],
        "0.62"
    );

    let body = response.bytes().await.unwrap();
    assert_eq!(
        body.as_ref(),
        SSE,
        "SSE bytes are relayed, never re-emitted"
    );
}

#[tokio::test]
async fn unknown_headers_survive_and_client_credentials_do_not() {
    let harness = harness().await;

    reqwest::Client::new()
        .post(format!("http://{}/v1/messages", harness.gateway))
        .header("x-api-key", &harness.key)
        .header("anthropic-beta", "context-management-2026-06-27")
        .header("anthropic-beta", "a-second-value")
        .header("x-invented-yesterday", "1")
        .body("{}")
        .send()
        .await
        .unwrap();

    let headers = harness.seen.headers.lock().unwrap().clone().unwrap();

    assert_eq!(headers["x-invented-yesterday"], "1");
    assert_eq!(headers.get_all("anthropic-beta").iter().count(), 2);
    assert_eq!(headers["x-api-key"], "provider-key");
    assert!(
        !headers
            .get("x-api-key")
            .and_then(|value| value.to_str().ok())
            .unwrap_or_default()
            .starts_with("gwk_"),
        "the client's own key is for us, not for the provider"
    );
    assert!(!headers.contains_key("authorization"));
}

fn long_conversation(model: &str) -> String {
    let block = |text: &str| serde_json::json!({ "type": "text", "text": text });
    let message =
        |role: &str| serde_json::json!({ "role": role, "content": [block(&"x".repeat(6000))] });
    serde_json::json!({
        "model": model,
        "max_tokens": 16,
        "system": [block(&"x".repeat(6000))],
        "messages": [
            message("user"),
            message("assistant"),
            message("user"),
        ],
    })
    .to_string()
}

#[tokio::test]
async fn a_long_prompt_leaves_with_cache_breakpoints_we_wrote_down() {
    let harness = harness().await;

    reqwest::Client::new()
        .post(format!("http://{}/v1/messages", harness.gateway))
        .header("x-api-key", &harness.key)
        .header("content-type", "application/json")
        .body(long_conversation("claude-sonnet-5"))
        .send()
        .await
        .unwrap();

    let received = harness.seen.body.lock().unwrap().clone().unwrap();
    let sent: serde_json::Value = serde_json::from_slice(&received).unwrap();

    assert_eq!(sent["system"][0]["cache_control"]["type"], "ephemeral");
    assert_eq!(
        sent["messages"][0]["content"][0]["cache_control"]["type"], "ephemeral",
        "the previous turn boundary is what the next request reads from"
    );
    assert_eq!(
        sent["messages"][2]["content"][0]["cache_control"]["type"], "ephemeral",
        "the current end is what this request writes"
    );
    assert!(sent["messages"][1]["content"][0]["cache_control"].is_null());
}

#[tokio::test]
async fn a_model_we_have_no_cache_minimum_for_is_sent_untouched() {
    let harness = harness().await;
    let sent = long_conversation("some-model-that-shipped-today");

    reqwest::Client::new()
        .post(format!("http://{}/v1/messages", harness.gateway))
        .header("x-api-key", &harness.key)
        .header("content-type", "application/json")
        .body(sent.clone())
        .send()
        .await
        .unwrap();

    let received = harness.seen.body.lock().unwrap().clone().unwrap();
    assert_eq!(received.as_ref(), sent.as_bytes());
}

#[tokio::test]
async fn a_second_turn_goes_back_to_the_account_holding_the_cache() {
    let harness = build(&[("first", b"secret-first"), ("second", b"secret-second")]).await;
    let opening = |turns: usize| {
        let block = serde_json::json!({ "type": "text", "text": "x".repeat(6000) });
        let mut messages = vec![serde_json::json!({ "role": "user", "content": [block] })];
        for _ in 0..turns {
            messages.push(serde_json::json!({ "role": "assistant", "content": "ok" }));
            messages.push(serde_json::json!({ "role": "user", "content": "go on" }));
        }
        serde_json::json!({
            "model": "claude-sonnet-5",
            "max_tokens": 16,
            "messages": messages,
        })
        .to_string()
    };

    harness.send(opening(0)).await;
    let served = harness.serving_account();

    harness
        .app
        .store()
        .set_rate_limits(
            if served == "secret-first" {
                "first"
            } else {
                "second"
            },
            &goat_gateway::limits::Snapshot {
                windows: vec![goat_gateway::limits::Window {
                    label: "5h".into(),
                    scope: None,
                    used_percent: 97.0,
                    resets_at_ms: None,
                }],
                binding: None,
                said: None,
            },
        )
        .unwrap();

    harness.send(opening(3)).await;
    assert_eq!(
        harness.serving_account(),
        served,
        "moving a live conversation throws away a prefix the other account has never seen"
    );
}

fn catalog_at(upstream: SocketAddr) -> goat_gateway::provider::Catalog {
    goat_gateway::provider::Catalog::builtin()
        .with_base_url("anthropic", &format!("http://{upstream}"))
}
