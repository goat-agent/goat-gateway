use std::{
    net::SocketAddr,
    sync::{Arc, Mutex},
};

use axum::{
    Json, Router,
    body::Body,
    http::StatusCode,
    response::Response,
    routing::{get, post},
};
use goat_gateway::{App, provider::Catalog, store::Store};
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

async fn office_llm(reached: Arc<Mutex<Vec<String>>>) -> SocketAddr {
    let chats = reached.clone();
    serve(
        Router::new()
            .route(
                "/v1/models",
                get(|| async { Json(json!({ "data": [{ "id": "office-large" }] })) }),
            )
            .route(
                "/v1/chat/completions",
                post(move || {
                    let chats = chats.clone();
                    async move {
                        chats.lock().unwrap().push("chat".into());
                        Response::builder()
                            .status(200)
                            .header("content-type", "application/json")
                            .header("x-ratelimit-limit-requests", "100")
                            .header("x-ratelimit-remaining-requests", "99")
                            .body(Body::from(
                                r#"{"id":"c","object":"chat.completion","choices":[{"index":0,"message":{"role":"assistant","content":"hi"},"finish_reason":"stop"}]}"#,
                            ))
                            .unwrap()
                    }
                }),
            )
            .fallback(|| async { (StatusCode::NOT_FOUND, "no such route") }),
    )
    .await
}

struct Harness {
    gateway: SocketAddr,
    store: Store,
    config: std::path::PathBuf,
    cookie: String,
}

async fn harness() -> Harness {
    static NEXT: std::sync::atomic::AtomicUsize = std::sync::atomic::AtomicUsize::new(0);
    let dir = std::env::temp_dir().join(format!(
        "goat-providers-{}-{}",
        std::process::id(),
        NEXT.fetch_add(1, std::sync::atomic::Ordering::Relaxed)
    ));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    let config = dir.join("config.toml");
    std::fs::write(&config, "# written by hand, keep me\n").unwrap();

    let store = Store::in_memory(&[7u8; 32]).unwrap();
    let admin = store.recover("admin", "a long password").unwrap();
    let cookie = format!("goat_session={}", store.open_session(&admin.id).unwrap());
    let app = App::new(store.clone(), [1u8; 32], Catalog::builtin()).reading(config.clone());
    let gateway = serve(app.router()).await;
    Harness {
        gateway,
        store,
        config,
        cookie,
    }
}

impl Harness {
    fn url(&self, path: &str) -> String {
        format!("http://{}{path}", self.gateway)
    }

    async fn call(&self, method: reqwest::Method, path: &str, body: Option<Value>) -> (u16, Value) {
        let mut request = reqwest::Client::new()
            .request(method, self.url(path))
            .header("cookie", &self.cookie);
        if let Some(body) = body {
            request = request.json(&body);
        }
        let response = request.send().await.unwrap();
        let status = response.status().as_u16();
        (status, response.json().await.unwrap_or(Value::Null))
    }
}

#[tokio::test]
async fn an_endpoint_nobody_declared_is_found_saved_and_served_without_a_restart() {
    let reached = Arc::new(Mutex::new(Vec::new()));
    let llm = office_llm(reached.clone()).await;
    let harness = harness().await;

    let (status, found) = harness
        .call(
            reqwest::Method::POST,
            "/api/providers/discover",
            Some(json!({ "base_url": format!("http://{llm}"), "secret": "sk-office" })),
        )
        .await;
    assert_eq!(status, 200, "{found}");
    assert_eq!(found["models"], json!(["office-large"]));
    let serves: Vec<&str> = found["findings"]
        .as_array()
        .unwrap()
        .iter()
        .filter(|finding| finding["serves"] == true)
        .filter_map(|finding| finding["wire"].as_str())
        .collect();
    assert_eq!(serves, ["chat"], "{found}");
    assert_eq!(found["reports_limits"], true);

    let declaration = found["declaration"].as_str().unwrap().to_owned();
    let (status, saved) = harness
        .call(
            reqwest::Method::PUT,
            "/api/providers/office",
            Some(json!({ "declaration": declaration })),
        )
        .await;
    assert_eq!(status, 200, "{saved}");
    let written = std::fs::read_to_string(&harness.config).unwrap();
    assert!(written.contains("# written by hand, keep me"));
    assert!(written.contains("[provider.office]"));

    let (_, listed) = harness
        .call(reqwest::Method::GET, "/api/providers", None)
        .await;
    let office = listed["providers"]
        .as_array()
        .unwrap()
        .iter()
        .find(|provider| provider["id"] == "office")
        .expect("the new provider is listed");
    assert_eq!(office["origin"], "config");

    let person = harness.store.add_user("mino").unwrap();
    harness
        .store
        .add_account("Office", "office", "api_key", b"sk-office")
        .unwrap();
    let key = harness
        .store
        .issue_key(&person.id, "script")
        .unwrap()
        .secret;
    let answer = reqwest::Client::new()
        .post(harness.url("/v1/chat/completions"))
        .bearer_auth(&key)
        .json(
            &json!({ "model": "office-large", "messages": [{ "role": "user", "content": "hi" }] }),
        )
        .send()
        .await
        .unwrap();
    assert_eq!(answer.status(), 200);
    assert!(reached.lock().unwrap().contains(&"chat".to_owned()));

    let (status, _) = harness
        .call(reqwest::Method::DELETE, "/api/providers/office", None)
        .await;
    assert_eq!(status, 204);
    let (_, listed) = harness
        .call(reqwest::Method::GET, "/api/providers", None)
        .await;
    assert!(
        !listed["providers"]
            .as_array()
            .unwrap()
            .iter()
            .any(|provider| provider["id"] == "office")
    );
}

#[tokio::test]
async fn a_declaration_the_gateway_would_refuse_never_reaches_the_file() {
    let harness = harness().await;
    let before = std::fs::read_to_string(&harness.config).unwrap();
    let (status, refused) = harness
        .call(
            reqwest::Method::PUT,
            "/api/providers/broken",
            Some(json!({ "declaration": "label = \"Broken\"\nendpoints = []\n" })),
        )
        .await;
    assert_eq!(status, 400);
    assert!(
        refused["error"].as_str().unwrap().contains("endpoint"),
        "{refused}"
    );
    assert_eq!(std::fs::read_to_string(&harness.config).unwrap(), before);
}

#[tokio::test]
async fn a_built_in_declaration_can_be_read_to_start_an_override_from() {
    let harness = harness().await;
    let (status, found) = harness
        .call(reqwest::Method::GET, "/api/providers/anthropic", None)
        .await;
    assert_eq!(status, 200);
    let declaration = found["declaration"].as_str().unwrap();
    assert!(
        declaration.contains("label = \"Anthropic\""),
        "{declaration}"
    );
    assert!(declaration.contains("[sign_in]"), "{declaration}");
}
