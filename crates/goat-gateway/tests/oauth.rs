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
        client_id: None,
    };
    store
        .add_account("Claude 개인", "anthropic", "oauth", &stale.to_bytes())
        .unwrap();

    let prepared = oauth::prepare(
        &store,
        &oauth::Client::against(format!("http://{addr}")),
        flow("anthropic").as_ref(),
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
    assert_eq!(sent[0]["client_id"], flow("anthropic").unwrap().client_id);

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
        client_id: None,
    };
    store
        .add_account("Claude", "anthropic", "oauth", &stale.to_bytes())
        .unwrap();

    let client = oauth::Client::against(format!("http://{addr}"));
    let anthropic = flow("anthropic");
    for _ in 0..2 {
        let secret = store.secret("Claude").unwrap();
        oauth::prepare(
            &store,
            &client,
            anthropic.as_ref(),
            "Claude",
            "anthropic",
            "oauth",
            &secret,
        )
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
        client_id: None,
    };
    store
        .add_account("Claude 개인", "anthropic", "oauth", &live.to_bytes())
        .unwrap();
    store.set_admin_key("gwa_test").unwrap();
    let user = store.add_user("isac").unwrap();
    let issued = store.issue_key(&user.id, "맥북").unwrap();

    let app = App::new(store.clone(), [1u8; 32], catalog_at(upstream));
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
        headers["anthropic-beta"], "oauth-2025-04-20,claude-code-20250219",
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
        client_id: None,
    };
    store
        .add_account("Claude 회사", "anthropic", "oauth", &live.to_bytes())
        .unwrap();
    store.set_admin_key("gwa_test").unwrap();
    let user = store.add_user("mino").unwrap();
    let issued = store.issue_key(&user.id, "노트북").unwrap();

    let app = App::new(store.clone(), [1u8; 32], catalog_at(upstream));
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
        client_id: None,
    };

    let error = oauth::prepare(
        &store,
        &oauth::Client::against(format!("http://{addr}")),
        flow("anthropic").as_ref(),
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

#[tokio::test]
async fn a_chatgpt_renewal_is_sent_as_json_and_keeps_the_account_it_belongs_to() {
    let seen = Seen::default();
    let addr = serve(
        Router::new()
            .route("/oauth/token", post(token_endpoint))
            .with_state(seen.clone()),
    )
    .await;

    let store = Store::in_memory(&[4u8; 32]).unwrap();
    let stale = Stored {
        access: "at_stale".into(),
        refresh: Some("rt".into()),
        expires_at: Some(goat_gateway::store::now() - 1),
        account_id: Some("acct_7".into()),
        client_id: None,
    };
    store
        .add_account("ChatGPT", "chatgpt", "oauth", &stale.to_bytes())
        .unwrap();

    let prepared = oauth::prepare(
        &store,
        &oauth::Client::against(format!("http://{addr}")),
        flow("chatgpt").as_ref(),
        "ChatGPT",
        "chatgpt",
        "oauth",
        &stale.to_bytes(),
    )
    .await
    .expect("the token endpoint only reads JSON, as auth.openai.com does for a refresh");

    let Auth::Oauth(used) = prepared.auth else {
        panic!("expected an oauth credential");
    };
    assert_eq!(used.token, "at_renewed");
    assert_eq!(
        used.extra,
        vec![("chatgpt-account-id".to_owned(), "acct_7".to_owned())],
        "a renewal without an id_token must not lose the workspace it was signed in to"
    );
    assert_eq!(
        seen.bodies.lock().unwrap()[0]["grant_type"],
        "refresh_token"
    );
}

#[derive(Clone, Default)]
struct Rotating {
    spent: Arc<Mutex<Vec<String>>>,
}

async fn rotating_token_endpoint(
    State(rotating): State<Rotating>,
    Json(body): Json<Value>,
) -> Response {
    let presented = body["refresh_token"]
        .as_str()
        .unwrap_or_default()
        .to_owned();
    let reused = {
        let mut spent = rotating.spent.lock().unwrap();
        let reused = spent.contains(&presented);
        spent.push(presented);
        reused
    };
    tokio::time::sleep(std::time::Duration::from_millis(50)).await;
    if reused {
        return Response::builder()
            .status(400)
            .header("content-type", "application/json")
            .body(Body::from(
                r#"{"error":"invalid_grant","error_description":"refresh_token_reused"}"#,
            ))
            .unwrap();
    }
    Response::builder()
        .status(200)
        .header("content-type", "application/json")
        .body(Body::from(
            r#"{"access_token":"at_next","refresh_token":"rt_next","expires_in":3600}"#,
        ))
        .unwrap()
}

#[tokio::test]
async fn two_calls_on_a_stale_sign_in_spend_the_refresh_token_once() {
    let rotating = Rotating::default();
    let addr = serve(
        Router::new()
            .route("/v1/oauth/token", post(rotating_token_endpoint))
            .with_state(rotating.clone()),
    )
    .await;

    let store = Store::in_memory(&[4u8; 32]).unwrap();
    let stale = Stored {
        access: "at_stale".into(),
        refresh: Some("rt_once".into()),
        expires_at: Some(goat_gateway::store::now() - 1),
        account_id: None,
        client_id: None,
    };
    store
        .add_account("Claude", "anthropic", "oauth", &stale.to_bytes())
        .unwrap();

    let client = oauth::Client::against(format!("http://{addr}"));
    let secret = store.secret("Claude").unwrap();
    let anthropic = flow("anthropic");
    let (first, second) = tokio::join!(
        oauth::prepare(
            &store,
            &client,
            anthropic.as_ref(),
            "Claude",
            "anthropic",
            "oauth",
            &secret
        ),
        oauth::prepare(
            &store,
            &client,
            anthropic.as_ref(),
            "Claude",
            "anthropic",
            "oauth",
            &secret
        ),
    );

    assert!(
        first.is_ok() && second.is_ok(),
        "a rotating refresh token spent twice signs the account out: {first:?} / {second:?}"
    );
    assert_eq!(rotating.spent.lock().unwrap().as_slice(), ["rt_once"]);
}

#[derive(Clone, Default)]
struct Reached {
    path: Arc<Mutex<Option<String>>>,
    headers: Arc<Mutex<Option<HeaderMap>>>,
}

async fn record_where(
    State(reached): State<Reached>,
    uri: axum::http::Uri,
    headers: HeaderMap,
) -> Response {
    *reached.path.lock().unwrap() = Some(uri.path().to_owned());
    *reached.headers.lock().unwrap() = Some(headers);
    Response::builder()
        .status(200)
        .header("content-type", "application/json")
        .body(Body::from(
            r#"{"id":"resp_1","object":"response","output":[]}"#,
        ))
        .unwrap()
}

#[tokio::test]
async fn a_chatgpt_sign_in_reaches_the_codex_responses_path_with_its_account() {
    let reached = Reached::default();
    let upstream = serve(
        Router::new()
            .fallback(post(record_where))
            .with_state(reached.clone()),
    )
    .await;

    let store = Store::in_memory(&[7u8; 32]).unwrap();
    let live = Stored {
        access: "at_chatgpt".into(),
        refresh: None,
        expires_at: None,
        account_id: Some("acct_42".into()),
        client_id: None,
    };
    store
        .add_account("ChatGPT Pro", "chatgpt", "oauth", &live.to_bytes())
        .unwrap();
    store.set_admin_key("gwa_test").unwrap();
    let user = store.add_user("mino").unwrap();
    let issued = store.issue_key(&user.id, "노트북").unwrap();

    let catalog = goat_gateway::provider::Catalog::builtin()
        .with_base_url("chatgpt", &format!("http://{upstream}"));
    let app = App::new(store, [1u8; 32], catalog);
    let gateway = serve(app.router()).await;

    let status = reqwest::Client::new()
        .post(format!("http://{gateway}/v1/responses"))
        .bearer_auth(&issued.secret)
        .json(&json!({ "model": "gpt-5-codex", "input": "hi", "store": false }))
        .send()
        .await
        .unwrap()
        .status();
    assert_eq!(status, StatusCode::OK);

    assert_eq!(
        reached.path.lock().unwrap().as_deref(),
        Some("/backend-api/codex/responses"),
        "the codex backend has no /v1 in its path"
    );
    let headers = reached.headers.lock().unwrap().clone().unwrap();
    assert_eq!(headers["authorization"], "Bearer at_chatgpt");
    assert_eq!(headers["chatgpt-account-id"], "acct_42");
}

#[derive(Clone, Default)]
struct Knocked {
    body: Arc<Mutex<Option<Value>>>,
}

async fn record_knock(State(knocked): State<Knocked>, Json(body): Json<Value>) -> Response {
    *knocked.body.lock().unwrap() = Some(body);
    Response::builder()
        .status(200)
        .header("content-type", "text/event-stream")
        .body(Body::from(
            "event: response.completed\ndata: {\"type\":\"response.completed\"}\n\n",
        ))
        .unwrap()
}

#[tokio::test]
async fn testing_a_chatgpt_sign_in_sends_what_the_codex_backend_accepts() {
    let knocked = Knocked::default();
    let upstream = serve(
        Router::new()
            .route("/backend-api/codex/responses", post(record_knock))
            .with_state(knocked.clone()),
    )
    .await;

    let store = Store::in_memory(&[8u8; 32]).unwrap();
    let live = Stored {
        access: "at".into(),
        refresh: None,
        expires_at: None,
        account_id: Some("acct_1".into()),
        client_id: None,
    };
    store
        .add_account("ChatGPT", "chatgpt", "oauth", &live.to_bytes())
        .unwrap();
    store.set_admin_key("gwa_test").unwrap();

    let catalog = goat_gateway::provider::Catalog::builtin()
        .with_base_url("chatgpt", &format!("http://{upstream}"));
    let gateway = serve(App::new(store, [1u8; 32], catalog).router()).await;

    let reached: Value = reqwest::Client::new()
        .post(format!("http://{gateway}/api/accounts/ChatGPT/test"))
        .header("cookie", "goat_admin=gwa_test")
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    assert_eq!(reached["ok"], true, "{reached}");

    let sent = knocked.body.lock().unwrap().clone().unwrap();
    assert_eq!(sent["stream"], true, "the codex backend only streams");
    assert_eq!(sent["store"], false, "the codex backend refuses to store");
    assert!(
        sent.get("max_output_tokens").is_none(),
        "the codex backend rejects max_output_tokens: {sent}"
    );
}

fn flow(provider: &str) -> Option<oauth::Flow> {
    goat_gateway::provider::Catalog::builtin()
        .flow(provider)
        .cloned()
}

fn catalog_at(upstream: SocketAddr) -> goat_gateway::provider::Catalog {
    goat_gateway::provider::Catalog::builtin()
        .with_base_url("anthropic", &format!("http://{upstream}"))
}
