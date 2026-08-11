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

fn gateway_app(upstream: SocketAddr, envelope_key: [u8; 32]) -> (App, String) {
    let store = Store::in_memory(&[7u8; 32]).unwrap();
    store
        .add_account("personal", "anthropic", "api_key", b"provider-key")
        .unwrap();
    let user = store.add_user("jmo").unwrap();
    let issued = store.issue_key(&user.id, "test").unwrap();
    store.set_admin_key("gwa_test-admin").unwrap();
    (
        App::new(store, envelope_key, format!("http://{upstream}")),
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
}

async fn harness() -> Harness {
    let seen = Seen::default();
    let upstream_addr = serve(
        Router::new()
            .route("/v1/messages", post(upstream_handler))
            .with_state(seen.clone()),
    )
    .await;

    let (app, key) = gateway_app(upstream_addr, [1u8; 32]);
    let gateway = serve(app.router()).await;

    Harness { gateway, seen, key }
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
