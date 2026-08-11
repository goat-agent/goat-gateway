use std::net::SocketAddr;

use axum::{Router, body::Body, response::Response, routing::post};
use goat_gateway::{App, store::Store};
use serde_json::json;

const ADMIN: &str = "gwa_test-admin";

async fn upstream() -> Response {
    Response::builder()
        .status(200)
        .header("content-type", "text/event-stream")
        .body(Body::from(
            "event: message_start\ndata: {\"type\":\"message_start\"}\n\nevent: message_stop\ndata: {\"type\":\"message_stop\"}\n\n",
        ))
        .unwrap()
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
    addr: SocketAddr,
    store: Store,
    api_key: String,
    key_id: String,
}

async fn harness() -> Harness {
    let upstream_addr = serve(Router::new().route("/v1/messages", post(upstream))).await;

    let store = Store::in_memory(&[7u8; 32]).unwrap();
    store
        .add_account("personal", "anthropic", "api_key", b"provider-key")
        .unwrap();
    store.set_admin_key(ADMIN).unwrap();
    let user = store.add_user("isac").unwrap();
    let issued = store.issue_key(&user.id, "맥북 Codex").unwrap();

    let app = App::new(store.clone(), [3u8; 32], format!("http://{upstream_addr}"));
    let addr = serve(app.router()).await;

    Harness {
        addr,
        store,
        api_key: issued.secret,
        key_id: issued.id,
    }
}

async fn call_gateway(harness: &Harness, key: Option<&str>) -> u16 {
    let mut request = reqwest::Client::new()
        .post(format!("http://{}/v1/messages", harness.addr))
        .json(&json!({ "model": "claude-sonnet-5" }));
    if let Some(key) = key {
        request = request.header("x-api-key", key);
    }
    request.send().await.unwrap().status().as_u16()
}

async fn call_admin(harness: &Harness, cookie: Option<&str>) -> u16 {
    let mut request = reqwest::Client::new().get(format!("http://{}/api/accounts", harness.addr));
    if let Some(cookie) = cookie {
        request = request.header("cookie", format!("goat_admin={cookie}"));
    }
    request.send().await.unwrap().status().as_u16()
}

#[tokio::test]
async fn the_admin_key_cannot_make_model_calls() {
    let harness = harness().await;
    assert_eq!(
        call_gateway(&harness, Some(ADMIN)).await,
        401,
        "if the admin key worked here nobody would ever create a user key, \
         and usage attribution would quietly die"
    );
}

#[tokio::test]
async fn an_api_key_cannot_reach_the_admin_surface() {
    let harness = harness().await;
    let key = harness.api_key.clone();
    assert_eq!(call_admin(&harness, Some(&key)).await, 401);
}

#[tokio::test]
async fn each_credential_works_on_its_own_surface() {
    let harness = harness().await;
    let key = harness.api_key.clone();
    assert_eq!(call_gateway(&harness, Some(&key)).await, 200);
    assert_eq!(call_admin(&harness, Some(ADMIN)).await, 200);
}

#[tokio::test]
async fn no_credential_is_refused_everywhere() {
    let harness = harness().await;
    assert_eq!(call_gateway(&harness, None).await, 401);
    assert_eq!(call_admin(&harness, None).await, 401);
}

#[tokio::test]
async fn a_revoked_key_stops_working_immediately() {
    let harness = harness().await;
    let key = harness.api_key.clone();
    assert_eq!(call_gateway(&harness, Some(&key)).await, 200);

    harness.store.revoke_key(&harness.key_id).unwrap();
    assert_eq!(call_gateway(&harness, Some(&key)).await, 401);
}

#[tokio::test]
async fn a_refusal_speaks_the_protocol_of_the_endpoint() {
    let harness = harness().await;

    let anthropic: serde_json::Value = reqwest::Client::new()
        .post(format!("http://{}/v1/messages", harness.addr))
        .json(&json!({ "model": "claude-sonnet-5" }))
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    assert_eq!(anthropic["type"], "error");
    assert_eq!(anthropic["error"]["type"], "authentication_error");

    let openai: serde_json::Value = reqwest::Client::new()
        .post(format!("http://{}/v1/responses", harness.addr))
        .json(&json!({ "model": "claude-sonnet-5", "stream": true, "input": "hi" }))
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    assert_eq!(openai["error"]["code"], "invalid_api_key");
}

#[tokio::test]
async fn a_call_is_attributed_to_the_user_behind_the_key() {
    let harness = harness().await;
    let key = harness.api_key.clone();
    call_gateway(&harness, Some(&key)).await;

    let recorded = harness.store.recent_requests(1).unwrap();
    assert_eq!(recorded[0].person.as_deref(), Some("isac"));
}

#[tokio::test]
async fn a_session_can_be_opened_with_the_admin_key_and_not_with_anything_else() {
    let harness = harness().await;
    let client = reqwest::Client::new();

    let good = client
        .post(format!("http://{}/api/session", harness.addr))
        .json(&json!({ "key": ADMIN }))
        .send()
        .await
        .unwrap();
    assert_eq!(good.status(), 204);
    assert!(
        good.headers()["set-cookie"]
            .to_str()
            .unwrap()
            .contains("HttpOnly")
    );

    let bad = client
        .post(format!("http://{}/api/session", harness.addr))
        .json(&json!({ "key": harness.api_key }))
        .send()
        .await
        .unwrap();
    assert_eq!(bad.status(), 401);
}

#[tokio::test]
async fn the_key_never_appears_in_a_listing() {
    let harness = harness().await;
    let listing = reqwest::Client::new()
        .get(format!("http://{}/api/keys", harness.addr))
        .header("cookie", format!("goat_admin={ADMIN}"))
        .send()
        .await
        .unwrap()
        .text()
        .await
        .unwrap();

    assert!(listing.contains("맥북 Codex"));
    assert!(listing.contains("isac"));
    assert!(
        !listing.contains(&harness.api_key),
        "a key is shown once at issue time and never again"
    );
}
