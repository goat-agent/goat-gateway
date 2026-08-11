use std::{
    net::SocketAddr,
    sync::{Arc, Mutex},
};

use axum::{
    Json, Router,
    body::Body,
    extract::State,
    http::{HeaderMap, StatusCode},
    response::Response,
    routing::post,
};
use goat_gateway::{
    App,
    oauth::{self, Stored},
    store::Store,
    upstream::Auth,
};
use serde_json::{Value, json};

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
    headers: Arc<Mutex<Option<HeaderMap>>>,
    bodies: Arc<Mutex<Vec<Value>>>,
}

async fn token_endpoint(State(seen): State<Seen>, Json(body): Json<Value>) -> Json<Value> {
    seen.bodies.lock().unwrap().push(body);
    Json(json!({
        "access_token": "at_renewed",
        "refresh_token": "rt_renewed",
        "expires_in": 3600,
        "token_type": "Bearer",
    }))
}

async fn messages_endpoint(State(seen): State<Seen>, headers: HeaderMap) -> Response {
    *seen.headers.lock().unwrap() = Some(headers);
    Response::builder()
        .status(200)
        .header("content-type", "application/json")
        .body(Body::from(r#"{"type":"message","content":[]}"#))
        .unwrap()
}

#[tokio::test]
async fn a_stale_sign_in_is_renewed_before_the_call_and_the_new_tokens_are_kept() {
    let seen = Seen::default();
    let addr = serve(
        Router::new()
            .route("/v1/oauth/token", post(token_endpoint))
            .with_state(seen.clone()),
    )
    .await;

    let store = Store::in_memory(&[4u8; 32]).unwrap();
    let stale = Stored {
        access: "at_stale".into(),
        refresh: Some("rt_original".into()),
        expires_at: Some(goat_gateway::store::now() - 1),
        account_id: None,
    };
    store
        .add_account("Claude 개인", "anthropic", "oauth", &stale.to_bytes())
        .unwrap();

    let prepared = oauth::prepare(
        &store,
        &oauth::Client::against(format!("http://{addr}")),
        "Claude 개인",
        "anthropic",
        "oauth",
        &stale.to_bytes(),
    )
    .await
    .unwrap();

    let Auth::Oauth(used) = prepared.auth else {
        panic!("expected an oauth credential");
    };
    assert_eq!(used.token, "at_renewed");

    let sent = seen.bodies.lock().unwrap().clone();
    assert_eq!(sent.len(), 1);
    assert_eq!(sent[0]["grant_type"], "refresh_token");
    assert_eq!(sent[0]["refresh_token"], "rt_original");
    assert_eq!(sent[0]["client_id"], oauth::ANTHROPIC.client_id);

    let kept: Stored = serde_json::from_slice(&store.secret("Claude 개인").unwrap()).unwrap();
    assert_eq!(kept.access, "at_renewed");
    assert_eq!(
        kept.refresh.as_deref(),
        Some("rt_renewed"),
        "keeping the spent refresh token would break the next renewal"
    );
    assert!(
        kept.expires_at.unwrap() > goat_gateway::store::now(),
        "a renewal that does not move the expiry would renew on every single call"
    );
}

#[tokio::test]
async fn a_renewal_happens_once_and_the_second_call_reuses_the_token() {
    let seen = Seen::default();
    let addr = serve(
        Router::new()
            .route("/v1/oauth/token", post(token_endpoint))
            .with_state(seen.clone()),
    )
    .await;

    let store = Store::in_memory(&[4u8; 32]).unwrap();
    let stale = Stored {
        access: "at_stale".into(),
        refresh: Some("rt".into()),
        expires_at: Some(goat_gateway::store::now() - 1),
        account_id: None,
    };
    store
        .add_account("Claude", "anthropic", "oauth", &stale.to_bytes())
        .unwrap();

    let client = oauth::Client::against(format!("http://{addr}"));
    for _ in 0..2 {
        let secret = store.secret("Claude").unwrap();
        oauth::prepare(&store, &client, "Claude", "anthropic", "oauth", &secret)
            .await
            .unwrap();
    }

    assert_eq!(
        seen.bodies.lock().unwrap().len(),
        1,
        "the renewed token was written back, so the second call had nothing to renew"
    );
}

#[tokio::test]
async fn a_signed_in_account_reaches_anthropic_as_a_bearer_with_the_oauth_beta() {
    let seen = Seen::default();
    let upstream = serve(
        Router::new()
            .route("/v1/messages", post(messages_endpoint))
            .with_state(seen.clone()),
    )
    .await;

    let store = Store::in_memory(&[6u8; 32]).unwrap();
    let live = Stored {
        access: "at_live".into(),
        refresh: Some("rt".into()),
        expires_at: Some(goat_gateway::store::now() + 3_600_000),
        account_id: None,
    };
    store
        .add_account("Claude 개인", "anthropic", "oauth", &live.to_bytes())
        .unwrap();
    store.set_admin_key("gwa_test").unwrap();
    let user = store.add_user("isac").unwrap();
    let issued = store.issue_key(&user.id, "맥북").unwrap();

    let app = App::new(store.clone(), [1u8; 32], format!("http://{upstream}"));
    let gateway = serve(app.router()).await;

    let status = reqwest::Client::new()
        .post(format!("http://{gateway}/v1/messages"))
        .header("x-api-key", &issued.secret)
        .header("anthropic-beta", "claude-code-20250219")
        .json(&json!({ "model": "claude-sonnet-5" }))
        .send()
        .await
        .unwrap()
        .status();
    assert_eq!(status, StatusCode::OK);

    let headers = seen.headers.lock().unwrap().clone().unwrap();
    assert_eq!(headers["authorization"], "Bearer at_live");
    assert!(
        !headers.contains_key("x-api-key"),
        "anthropic rejects an oauth call that also carries x-api-key, \
         and the client's own gateway key must never travel upstream"
    );
    assert_eq!(
        headers["anthropic-beta"],
        "oauth-2025-04-20,claude-code-20250219",
        "the client's beta flags must survive alongside the one oauth requires"
    );
}

#[tokio::test]
async fn the_call_is_recorded_against_the_account_that_was_signed_in() {
    let seen = Seen::default();
    let upstream = serve(
        Router::new()
            .route("/v1/messages", post(messages_endpoint))
            .with_state(seen),
    )
    .await;

    let store = Store::in_memory(&[6u8; 32]).unwrap();
    let live = Stored {
        access: "at".into(),
        refresh: None,
        expires_at: None,
        account_id: None,
    };
    store
        .add_account("Claude 회사", "anthropic", "oauth", &live.to_bytes())
        .unwrap();
    store.set_admin_key("gwa_test").unwrap();
    let user = store.add_user("mino").unwrap();
    let issued = store.issue_key(&user.id, "노트북").unwrap();

    let app = App::new(store.clone(), [1u8; 32], format!("http://{upstream}"));
    let gateway = serve(app.router()).await;

    reqwest::Client::new()
        .post(format!("http://{gateway}/v1/messages"))
        .header("x-api-key", &issued.secret)
        .json(&json!({ "model": "claude-sonnet-5" }))
        .send()
        .await
        .unwrap();

    let recorded = store.recent_requests(1).unwrap();
    assert_eq!(recorded[0].account.as_deref(), Some("Claude 회사"));
    assert_eq!(recorded[0].person.as_deref(), Some("mino"));

    let evidence = recorded[0].evidence.clone().unwrap();
    let edits = evidence["header_edits"].as_array().unwrap();
    let names: Vec<&str> = edits
        .iter()
        .filter_map(|edit| edit["name"].as_str())
        .collect();
    assert!(
        names.contains(&"authorization") && names.contains(&"anthropic-beta"),
        "every header we touched has to be announced, and oauth touches two: {names:?}"
    );
}

#[tokio::test]
async fn a_dead_sign_in_fails_the_call_with_the_providers_own_words() {
    let addr = serve(Router::new().route(
        "/v1/oauth/token",
        post(|| async {
            (
                StatusCode::BAD_REQUEST,
                r#"{"error":"invalid_grant","error_description":"refresh token revoked"}"#,
            )
        }),
    ))
    .await;

    let store = Store::in_memory(&[4u8; 32]).unwrap();
    let stale = Stored {
        access: "at".into(),
        refresh: Some("rt_revoked".into()),
        expires_at: Some(goat_gateway::store::now() - 1),
        account_id: None,
    };

    let error = oauth::prepare(
        &store,
        &oauth::Client::against(format!("http://{addr}")),
        "Claude",
        "anthropic",
        "oauth",
        &stale.to_bytes(),
    )
    .await
    .unwrap_err();

    let message = error.to_string();
    assert!(
        message.contains("refresh token revoked"),
        "why the sign-in died only exists in their body: {message}"
    );
    assert!(message.contains("signed in again"));
}
