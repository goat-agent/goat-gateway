use std::{
    net::SocketAddr,
    sync::{Arc, Mutex},
};

use axum::{
    Router,
    body::{Body, Bytes},
    extract::State,
    response::Response,
    routing::post,
};
use goat_gateway::{App, store::Store};
use serde_json::{Value, json};

const SIGNATURE: &str = "EqQBCgIYAhIM1gbcDa9GJwZA2b3hGgxBdjrkzLoky3dl1pki";
const ENVELOPE_KEY: [u8; 32] = [4u8; 32];

fn anthropic_sse() -> String {
    [
        r#"event: message_start
data: {"type":"message_start","message":{"id":"msg_1","role":"assistant","content":[],"model":"claude-sonnet-5"}}

"#
        .to_owned(),
        r#"event: content_block_start
data: {"type":"content_block_start","index":0,"content_block":{"type":"thinking","thinking":"","signature":""}}

"#
        .to_owned(),
        r#"event: content_block_delta
data: {"type":"content_block_delta","index":0,"delta":{"type":"thinking_delta","thinking":"weighing the options"}}

"#
        .to_owned(),
        format!(
            "event: content_block_delta\ndata: {{\"type\":\"content_block_delta\",\"index\":0,\"delta\":{{\"type\":\"signature_delta\",\"signature\":\"{SIGNATURE}\"}}}}\n\n"
        ),
        r#"event: content_block_stop
data: {"type":"content_block_stop","index":0}

"#
        .to_owned(),
        r#"event: content_block_start
data: {"type":"content_block_start","index":1,"content_block":{"type":"text","text":""}}

"#
        .to_owned(),
        r#"event: content_block_delta
data: {"type":"content_block_delta","index":1,"delta":{"type":"text_delta","text":"Here you go."}}

"#
        .to_owned(),
        r#"event: content_block_stop
data: {"type":"content_block_stop","index":1}

"#
        .to_owned(),
        r#"event: message_delta
data: {"type":"message_delta","delta":{"stop_reason":"end_turn"},"usage":{"input_tokens":10,"cache_read_input_tokens":200,"output_tokens":8}}

"#
        .to_owned(),
        r#"event: message_stop
data: {"type":"message_stop"}

"#
        .to_owned(),
    ]
    .concat()
}

#[derive(Clone, Default)]
struct Seen {
    body: Arc<Mutex<Option<Bytes>>>,
}

async fn upstream_handler(State(seen): State<Seen>, body: Bytes) -> Response {
    *seen.body.lock().unwrap() = Some(body);
    Response::builder()
        .status(200)
        .header("content-type", "text/event-stream")
        .body(Body::from(anthropic_sse()))
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

    let (app, key) = gateway_app(upstream_addr, ENVELOPE_KEY);
    let gateway = serve(app.router()).await;

    Harness { gateway, seen, key }
}

async fn call(harness: &Harness, request: Value) -> (u16, String) {
    let response = reqwest::Client::new()
        .post(format!("http://{}/v1/responses", harness.gateway))
        .header("x-api-key", &harness.key)
        .json(&request)
        .send()
        .await
        .unwrap();
    let status = response.status().as_u16();
    (status, response.text().await.unwrap())
}

fn events(body: &str) -> Vec<(String, Value)> {
    body.split("\n\n")
        .filter(|frame| !frame.trim().is_empty())
        .map(|frame| {
            let mut event = String::new();
            let mut data = String::new();
            for line in frame.lines() {
                if let Some(rest) = line.strip_prefix("event: ") {
                    event = rest.to_owned();
                } else if let Some(rest) = line.strip_prefix("data: ") {
                    data = rest.to_owned();
                }
            }
            (event, serde_json::from_str(&data).unwrap())
        })
        .collect()
}

fn simple_request() -> Value {
    json!({
        "model": "claude-sonnet-5",
        "stream": true,
        "instructions": "be brief",
        "input": [{ "type": "message", "role": "user", "content": "hello" }],
        "reasoning": { "effort": "high" },
    })
}

#[tokio::test]
async fn a_responses_request_becomes_an_anthropic_request() {
    let harness = harness().await;
    let (status, _) = call(&harness, simple_request()).await;
    assert_eq!(status, 200);

    let sent: Value =
        serde_json::from_slice(&harness.seen.body.lock().unwrap().clone().unwrap()).unwrap();

    assert_eq!(sent["model"], "claude-sonnet-5");
    assert_eq!(sent["system"][0]["text"], "be brief");
    assert_eq!(sent["messages"][0]["role"], "user");
    assert_eq!(sent["messages"][0]["content"][0]["text"], "hello");
    assert_eq!(sent["max_tokens"], 32000);
    assert_eq!(sent["thinking"]["display"], "summarized");
    assert_eq!(sent["output_config"]["effort"], "high");
}

#[tokio::test]
async fn the_client_receives_responses_events() {
    let harness = harness().await;
    let (_, body) = call(&harness, simple_request()).await;
    let kinds: Vec<String> = events(&body).into_iter().map(|(kind, _)| kind).collect();

    assert_eq!(kinds.first().unwrap(), "response.created");
    assert!(kinds.contains(&"response.reasoning_summary_text.delta".to_owned()));
    assert!(kinds.contains(&"response.output_text.delta".to_owned()));
    assert_eq!(kinds.last().unwrap(), "response.completed");
}

#[tokio::test]
async fn the_envelope_survives_a_second_turn_through_the_gateway() {
    let harness = harness().await;
    let (_, body) = call(&harness, simple_request()).await;

    let reasoning = events(&body)
        .into_iter()
        .find(|(kind, data)| {
            kind == "response.output_item.done" && data["item"]["type"] == "reasoning"
        })
        .expect("a reasoning item")
        .1["item"]
        .clone();

    assert!(
        reasoning["encrypted_content"]
            .as_str()
            .unwrap()
            .starts_with("gwe1_")
    );

    let (status, _) = call(
        &harness,
        json!({
            "model": "claude-sonnet-5",
            "stream": true,
            "input": [
                { "type": "message", "role": "user", "content": "hello" },
                reasoning,
                { "type": "message", "role": "user", "content": "go on" },
            ],
        }),
    )
    .await;
    assert_eq!(status, 200);

    let sent: Value =
        serde_json::from_slice(&harness.seen.body.lock().unwrap().clone().unwrap()).unwrap();
    let block = &sent["messages"][1]["content"][0];

    assert_eq!(block["type"], "thinking");
    assert_eq!(block["thinking"], "weighing the options");
    assert_eq!(
        block["signature"], SIGNATURE,
        "the provider's signature must come back byte for byte"
    );
}

#[tokio::test]
async fn an_unregistered_model_is_named_not_guessed() {
    let harness = harness().await;
    let (status, body) = call(
        &harness,
        json!({ "model": "claudette-fast", "stream": true, "input": "hi" }),
    )
    .await;

    assert_eq!(status, 400);
    let error: Value = serde_json::from_str(&body).unwrap();
    let message = error["error"]["message"].as_str().unwrap();
    assert!(message.contains("claudette-fast"), "{message}");
    assert!(message.contains("claude-sonnet-5"), "{message}");
    assert!(harness.seen.body.lock().unwrap().is_none());
}

#[tokio::test]
async fn an_unmappable_tool_never_reaches_the_provider() {
    let harness = harness().await;
    let (status, body) = call(
        &harness,
        json!({
            "model": "claude-sonnet-5",
            "stream": true,
            "input": "draw a cat",
            "tools": [{ "type": "image_generation" }],
        }),
    )
    .await;

    assert_eq!(status, 400);
    assert!(body.contains("image_generation"));
    assert!(
        harness.seen.body.lock().unwrap().is_none(),
        "fail closed means we do not spend a provider call first"
    );
}

#[tokio::test]
async fn a_non_streaming_request_is_refused_before_the_provider_is_called() {
    let harness = harness().await;
    let (status, _) = call(
        &harness,
        json!({ "model": "claude-sonnet-5", "input": "hi" }),
    )
    .await;

    assert_eq!(status, 400);
    assert!(harness.seen.body.lock().unwrap().is_none());
}
