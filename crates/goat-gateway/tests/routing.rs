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

async fn count_tokens(State(seen): State<Seen>, headers: HeaderMap, body: Bytes) -> Response {
    *seen.path.lock().unwrap() = Some("/v1/messages/count_tokens".to_owned());
    *seen.body.lock().unwrap() = Some(body);
    *seen.headers.lock().unwrap() = Some(headers);
    Response::builder()
        .status(200)
        .header("content-type", "application/json")
        .body(Body::from(r#"{"input_tokens":12}"#))
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
            .route("/v1/messages/count_tokens", post(count_tokens))
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
async fn one_anthropic_account_serves_all_three_formats() {
    let harness = harness(&[("personal", "anthropic")]).await;

    for (path, body) in [
        (
            "/v1/messages",
            json!({ "model": "claude-sonnet-5", "max_tokens": 8, "messages": [] }),
        ),
        (
            "/v1/chat/completions",
            json!({ "model": "claude-sonnet-5", "messages": [] }),
        ),
        (
            "/v1/responses",
            json!({ "model": "claude-sonnet-5", "stream": true, "input": "hi" }),
        ),
    ] {
        let (status, said) = harness.post(path, body).await;
        assert_eq!(
            status, 200,
            "{path} could not be served by the one registered account: {said}"
        );
        assert_eq!(
            harness.reached().as_deref(),
            Some("/v1/messages"),
            "{path} has to end up at the only endpoint the provider serves"
        );
    }
}

const CHAT_SSE: &str = concat!(
    "data: {\"choices\":[{\"index\":0,\"delta\":{\"role\":\"assistant\",\"reasoning_content\":\"weighing\"}}]}\n\n",
    "data: {\"choices\":[{\"index\":0,\"delta\":{\"content\":\"Running it.\"}}]}\n\n",
    "data: {\"choices\":[{\"index\":0,\"delta\":{\"tool_calls\":[{\"index\":0,\"id\":\"call_9\",\"function\":{\"name\":\"bash\",\"arguments\":\"{\\\"cmd\\\":\\\"ls\\\"}\"}}]}}]}\n\n",
    "data: {\"choices\":[{\"index\":0,\"delta\":{},\"finish_reason\":\"tool_calls\"}]}\n\n",
    "data: {\"choices\":[],\"usage\":{\"prompt_tokens\":900,\"completion_tokens\":40,\"prompt_tokens_details\":{\"cached_tokens\":800}}}\n\n",
    "data: [DONE]\n\n",
);

async fn coding_plan(State(seen): State<Seen>, headers: HeaderMap, body: Bytes) -> Response {
    *seen.path.lock().unwrap() = Some("/coding/v1/chat/completions".to_owned());
    *seen.body.lock().unwrap() = Some(body);
    *seen.headers.lock().unwrap() = Some(headers);
    Response::builder()
        .status(200)
        .header("content-type", "text/event-stream")
        .body(Body::from(CHAT_SSE))
        .unwrap()
}

async fn kimi_harness() -> Harness {
    let seen = Seen::default();
    let upstream = serve(
        Router::new()
            .route("/coding/v1/chat/completions", post(coding_plan))
            .with_state(seen.clone()),
    )
    .await;

    let store = Store::in_memory(&[7u8; 32]).unwrap();
    store
        .add_account("coding plan", "kimi", "api_key", b"key-kimi")
        .unwrap();
    let user = store.add_user("jmo").unwrap();
    let key = store.issue_key(&user.id, "test").unwrap().secret;
    store.set_admin_key("gwa_test-admin").unwrap();

    let catalog = Catalog::builtin().with_base_url("kimi", &format!("http://{upstream}"));
    let gateway = serve(App::new(store, [1u8; 32], catalog).router()).await;
    Harness { gateway, seen, key }
}

fn claude_code_turn(stream: bool) -> Value {
    json!({
        "model": "kimi-for-coding",
        "max_tokens": 4096,
        "stream": stream,
        "system": [{ "type": "text", "text": "You are Claude Code." }],
        "tools": [{
            "name": "bash",
            "description": "run a command",
            "input_schema": { "type": "object", "properties": { "cmd": { "type": "string" } } },
        }],
        "messages": [
            { "role": "user", "content": "list the files" },
            { "role": "assistant", "content": [
                { "type": "thinking", "thinking": "earlier reasoning", "signature": "EqQBCgIYAh" },
                { "type": "tool_use", "id": "toolu_01A", "name": "bash", "input": { "cmd": "pwd" } },
            ]},
            { "role": "user", "content": [
                { "type": "tool_result", "tool_use_id": "toolu_01A", "content": "/home" },
            ]},
        ],
    })
}

#[tokio::test]
async fn claude_code_reaches_a_coding_plan_that_only_speaks_chat() {
    let harness = kimi_harness().await;

    let response = reqwest::Client::new()
        .post(format!("http://{}/v1/messages", harness.gateway))
        .header("x-api-key", &harness.key)
        .header("content-type", "application/json")
        .json(&claude_code_turn(true))
        .send()
        .await
        .unwrap();

    assert_eq!(response.status(), 200);
    assert_eq!(response.headers()["content-type"], "text/event-stream");

    let sent: Value =
        serde_json::from_slice(&harness.seen.body.lock().unwrap().clone().unwrap()).unwrap();
    let roles: Vec<&str> = sent["messages"]
        .as_array()
        .unwrap()
        .iter()
        .map(|message| message["role"].as_str().unwrap())
        .collect();
    assert_eq!(roles, ["system", "user", "assistant", "tool"]);
    assert_eq!(sent["messages"][2]["tool_calls"][0]["id"], "toolu_01A");
    assert_eq!(sent["messages"][3]["tool_call_id"], "toolu_01A");
    assert_eq!(sent["tools"][0]["function"]["name"], "bash");
    assert!(
        !sent.to_string().contains("EqQBCgIYAh"),
        "a signature this provider never minted must not be sent to it"
    );

    let body = response.text().await.unwrap();
    assert!(body.contains("event: message_start"), "{body}");
    assert!(body.contains("\"type\":\"thinking_delta\""), "{body}");
    assert!(body.contains("\"type\":\"tool_use\""), "{body}");
    assert!(body.contains("\"id\":\"call_9\""), "{body}");
    assert!(body.contains("\"stop_reason\":\"tool_use\""), "{body}");
    assert!(body.contains("event: message_stop"), "{body}");
}

#[tokio::test]
async fn a_turn_that_did_not_ask_to_stream_still_gets_an_answer() {
    let harness = kimi_harness().await;

    let response = reqwest::Client::new()
        .post(format!("http://{}/v1/messages", harness.gateway))
        .header("x-api-key", &harness.key)
        .header("content-type", "application/json")
        .json(&claude_code_turn(false))
        .send()
        .await
        .unwrap();

    assert_eq!(response.status(), 200);
    let whole: Value = response.json().await.unwrap();
    assert_eq!(whole["role"], "assistant");
    assert_eq!(whole["stop_reason"], "tool_use");
    assert_eq!(whole["content"][0]["thinking"], "weighing");
    assert_eq!(whole["content"][1]["text"], "Running it.");
    assert_eq!(whole["content"][2]["input"]["cmd"], "ls");
    assert_eq!(whole["usage"]["cache_read_input_tokens"], 800);
}

#[tokio::test]
async fn testing_an_account_sends_a_real_request_and_says_what_came_back() {
    let harness = harness(&[("work", "openai")]).await;

    let response = reqwest::Client::new()
        .post(format!("http://{}/api/accounts/work/test", harness.gateway))
        .header("cookie", "goat_admin=gwa_test-admin")
        .send()
        .await
        .unwrap();

    assert_eq!(response.status(), 200);
    let reached: Value = response.json().await.unwrap();
    assert_eq!(reached["ok"], true);
    assert_eq!(reached["status"], 200);
    assert_eq!(reached["model"], "gpt-5");
    assert!(reached["took_ms"].as_i64().is_some());
    assert_eq!(
        harness.reached().as_deref(),
        Some("/v1/responses"),
        "a test that does not send a request tests nothing"
    );
}

const MESSAGES_SSE: &str = concat!(
    "event: message_start\ndata: {\"type\":\"message_start\",\"message\":{\"usage\":{\"input_tokens\":100,\"cache_read_input_tokens\":900}}}\n\n",
    "event: content_block_start\ndata: {\"type\":\"content_block_start\",\"index\":0,\"content_block\":{\"type\":\"text\"}}\n\n",
    "event: content_block_delta\ndata: {\"type\":\"content_block_delta\",\"index\":0,\"delta\":{\"type\":\"text_delta\",\"text\":\"Here.\"}}\n\n",
    "event: content_block_stop\ndata: {\"type\":\"content_block_stop\",\"index\":0}\n\n",
    "event: message_delta\ndata: {\"type\":\"message_delta\",\"delta\":{\"stop_reason\":\"end_turn\"},\"usage\":{\"output_tokens\":8}}\n\n",
    "event: message_stop\ndata: {\"type\":\"message_stop\"}\n\n",
);

async fn anthropic_stream(State(seen): State<Seen>, headers: HeaderMap, body: Bytes) -> Response {
    *seen.path.lock().unwrap() = Some("/v1/messages".to_owned());
    *seen.body.lock().unwrap() = Some(body);
    *seen.headers.lock().unwrap() = Some(headers);
    Response::builder()
        .status(200)
        .header("content-type", "text/event-stream")
        .body(Body::from(MESSAGES_SSE))
        .unwrap()
}

async fn anthropic_only() -> Harness {
    let seen = Seen::default();
    let upstream = serve(
        Router::new()
            .route("/v1/messages", post(anthropic_stream))
            .with_state(seen.clone()),
    )
    .await;

    let store = Store::in_memory(&[7u8; 32]).unwrap();
    store
        .add_account("personal", "anthropic", "api_key", b"key-personal")
        .unwrap();
    let user = store.add_user("jmo").unwrap();
    let key = store.issue_key(&user.id, "test").unwrap().secret;
    store.set_admin_key("gwa_test-admin").unwrap();

    let catalog = Catalog::builtin().with_base_url("anthropic", &format!("http://{upstream}"));
    let gateway = serve(App::new(store, [1u8; 32], catalog).router()).await;
    Harness { gateway, seen, key }
}

fn chat_turn(stream: bool) -> Value {
    json!({
        "model": "claude-sonnet-5",
        "stream": stream,
        "max_tokens": 512,
        "messages": [
            { "role": "system", "content": "Be terse." },
            { "role": "user", "content": "hi" },
            { "role": "assistant", "tool_calls": [
                { "id": "call_9", "type": "function", "function": { "name": "bash", "arguments": "{}" } },
            ]},
            { "role": "tool", "tool_call_id": "call_9", "content": "done" },
        ],
        "tools": [{
            "type": "function",
            "function": { "name": "bash", "parameters": { "type": "object", "properties": {} } },
        }],
    })
}

#[tokio::test]
async fn a_chat_completions_client_reaches_an_anthropic_account() {
    let harness = anthropic_only().await;

    let response = reqwest::Client::new()
        .post(format!("http://{}/v1/chat/completions", harness.gateway))
        .header("x-api-key", &harness.key)
        .json(&chat_turn(true))
        .send()
        .await
        .unwrap();

    assert_eq!(response.status(), 200);
    assert_eq!(harness.reached().as_deref(), Some("/v1/messages"));

    let sent: Value =
        serde_json::from_slice(&harness.seen.body.lock().unwrap().clone().unwrap()).unwrap();
    assert_eq!(sent["system"][0]["text"], "Be terse.");
    assert_eq!(sent["max_tokens"], 512);
    assert_eq!(sent["tools"][0]["name"], "bash");
    assert_eq!(sent["messages"][1]["content"][0]["type"], "tool_use");
    assert_eq!(sent["messages"][1]["content"][0]["id"], "call_9");
    assert_eq!(sent["messages"][2]["content"][0]["type"], "tool_result");

    let body = response.text().await.unwrap();
    assert!(body.contains("chat.completion.chunk"), "{body}");
    assert!(body.contains("\"content\":\"Here.\""), "{body}");
    assert!(body.contains("\"finish_reason\":\"stop\""), "{body}");
    assert!(body.contains("[DONE]"), "{body}");
}

#[tokio::test]
async fn a_chat_client_that_did_not_stream_gets_one_completion() {
    let harness = anthropic_only().await;

    let response = reqwest::Client::new()
        .post(format!("http://{}/v1/chat/completions", harness.gateway))
        .header("x-api-key", &harness.key)
        .json(&chat_turn(false))
        .send()
        .await
        .unwrap();

    assert_eq!(response.status(), 200);
    let whole: Value = response.json().await.unwrap();
    assert_eq!(whole["object"], "chat.completion");
    assert_eq!(whole["choices"][0]["message"]["content"], "Here.");
    assert_eq!(whole["choices"][0]["finish_reason"], "stop");
    assert_eq!(whole["usage"]["prompt_tokens"], 1000);
    assert_eq!(whole["usage"]["completion_tokens"], 8);
}

#[tokio::test]
async fn a_codex_client_reaches_a_coding_plan_two_formats_away() {
    let harness = kimi_harness().await;

    let response = reqwest::Client::new()
        .post(format!("http://{}/v1/responses", harness.gateway))
        .header("x-api-key", &harness.key)
        .json(&json!({
            "model": "kimi-for-coding",
            "stream": true,
            "instructions": "Be terse.",
            "input": [
                { "role": "user", "content": [{ "type": "input_text", "text": "list the files" }] },
            ],
            "tools": [{
                "type": "function",
                "name": "bash",
                "parameters": { "type": "object", "properties": {} },
            }],
        }))
        .send()
        .await
        .unwrap();

    assert_eq!(response.status(), 200);
    assert_eq!(
        harness.reached().as_deref(),
        Some("/coding/v1/chat/completions")
    );

    let sent: Value =
        serde_json::from_slice(&harness.seen.body.lock().unwrap().clone().unwrap()).unwrap();
    assert_eq!(
        sent["messages"][0]["role"], "system",
        "the instructions have to survive both hops"
    );
    assert_eq!(sent["tools"][0]["function"]["name"], "bash");

    let body = response.text().await.unwrap();
    assert!(body.contains("response.created"), "{body}");
    assert!(body.contains("response.completed"), "{body}");
}

#[tokio::test]
async fn a_translated_request_is_in_flight_while_the_provider_thinks() {
    let release = Arc::new(tokio::sync::Notify::new());
    let held = release.clone();
    let upstream = serve(Router::new().route(
        "/v1/responses",
        post(move || {
            let held = held.clone();
            async move {
                held.notified().await;
                Response::builder()
                    .status(200)
                    .header("content-type", "text/event-stream")
                    .body(Body::from(
                        "event: response.completed\ndata: {\"type\":\"response.completed\",\"response\":{}}\n\n",
                    ))
                    .unwrap()
            }
        }),
    ))
    .await;

    let store = Store::in_memory(&[7u8; 32]).unwrap();
    store
        .add_account("work", "openai", "api_key", b"sk")
        .unwrap();
    let user = store.add_user("jmo").unwrap();
    let key = store.issue_key(&user.id, "test").unwrap().secret;
    let catalog = Catalog::builtin().with_base_url("openai", &format!("http://{upstream}"));
    let gateway = serve(App::new(store.clone(), [1u8; 32], catalog).router()).await;

    let sending = tokio::spawn(async move {
        reqwest::Client::new()
            .post(format!("http://{gateway}/v1/messages"))
            .header("x-api-key", &key)
            .json(&json!({ "model": "gpt-5", "max_tokens": 8, "stream": true, "messages": [] }))
            .send()
            .await
            .unwrap()
            .bytes()
            .await
            .unwrap()
    });

    let mut seen = None;
    for _ in 0..50 {
        seen = store.recent_requests(1).unwrap().into_iter().next();
        if seen.is_some() {
            break;
        }
        tokio::time::sleep(std::time::Duration::from_millis(20)).await;
    }
    let row = seen.expect("in flight is a real number only if the row exists before the answer");
    assert_eq!(row.status, "in_flight");
    assert!(row.translated);

    release.notify_one();
    sending.await.unwrap();
}

#[tokio::test]
async fn the_paths_clients_build_from_a_base_url_all_arrive() {
    let harness = harness(&[("personal", "anthropic")]).await;
    for path in ["/messages", "/v1/messages", "/v1/v1/messages"] {
        *harness.seen.path.lock().unwrap() = None;
        let (status, body) = harness
            .post(
                path,
                json!({ "model": "claude-sonnet-5", "max_tokens": 8, "messages": [] }),
            )
            .await;
        assert_eq!(status, 200, "{path}: {body}");
        assert_eq!(harness.reached().as_deref(), Some("/v1/messages"), "{path}");
    }
}

#[tokio::test]
async fn a_client_is_told_which_models_it_can_reach() {
    let harness = harness(&[("personal", "anthropic")]).await;
    let listed: Value = reqwest::Client::new()
        .get(format!("http://{}/v1/models", harness.gateway))
        .header("x-api-key", &harness.key)
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();

    let ids: Vec<&str> = listed["data"]
        .as_array()
        .unwrap()
        .iter()
        .filter_map(|model| model["id"].as_str())
        .collect();
    assert!(ids.contains(&"claude-sonnet-5"), "{ids:?}");
    assert!(
        !ids.contains(&"glm-4.7"),
        "a model no registered account can serve is not on offer: {ids:?}"
    );
    assert_eq!(listed["object"], "list");
    assert_eq!(listed["has_more"], false);
    assert_eq!(listed["data"][0]["type"], "model");
    assert_eq!(listed["data"][0]["object"], "model");
}

#[tokio::test]
async fn counting_tokens_asks_the_provider_that_would_serve_the_message() {
    let harness = harness(&[("personal", "anthropic")]).await;
    let (status, body) = harness
        .post(
            "/v1/messages/count_tokens",
            json!({ "model": "claude-sonnet-5", "messages": [{ "role": "user", "content": "hi" }] }),
        )
        .await;
    assert_eq!(status, 200, "{body}");
    assert_eq!(body, r#"{"input_tokens":12}"#);
    assert_eq!(
        harness.reached().as_deref(),
        Some("/v1/messages/count_tokens")
    );
    assert_eq!(harness.credential().as_deref(), Some("key-personal"));
}

#[tokio::test]
async fn counting_tokens_for_a_model_served_in_another_format_says_so() {
    let harness = harness(&[("work", "openai")]).await;
    let (status, body) = harness
        .post(
            "/v1/messages/count_tokens",
            json!({ "model": "gpt-5", "messages": [] }),
        )
        .await;
    assert_eq!(status, 400, "{body}");
    assert!(body.contains("Messages"), "{body}");
    assert_eq!(harness.reached(), None, "nothing should be sent anywhere");
}

#[tokio::test]
async fn a_client_checking_whether_the_gateway_is_up_is_answered() {
    let harness = harness(&[]).await;
    let status = reqwest::Client::new()
        .head(format!("http://{}/api/hello", harness.gateway))
        .send()
        .await
        .unwrap()
        .status();
    assert_eq!(status, 200);
}

#[tokio::test]
async fn claude_code_can_be_served_by_a_chatgpt_plan() {
    const CODEX_SSE: &str = concat!(
        "event: response.created\ndata: {\"type\":\"response.created\",\"response\":{\"id\":\"resp_1\"}}\n\n",
        "event: response.output_item.added\ndata: {\"type\":\"response.output_item.added\",\"item\":{\"type\":\"message\",\"id\":\"msg_1\"}}\n\n",
        "event: response.content_part.added\ndata: {\"type\":\"response.content_part.added\",\"part\":{\"type\":\"output_text\",\"text\":\"\"}}\n\n",
        "event: response.output_text.delta\ndata: {\"type\":\"response.output_text.delta\",\"delta\":\"Hello from Codex\"}\n\n",
        "event: response.output_item.done\ndata: {\"type\":\"response.output_item.done\",\"item\":{\"type\":\"message\",\"id\":\"msg_1\"}}\n\n",
        "event: response.completed\ndata: {\"type\":\"response.completed\",\"response\":{\"usage\":{\"input_tokens\":12,\"output_tokens\":4}}}\n\n",
    );
    let sent = Arc::new(Mutex::new(None::<Value>));
    let kept = sent.clone();
    let upstream = serve(Router::new().route(
        "/backend-api/codex/responses",
        post(move |body: Bytes| {
            let kept = kept.clone();
            async move {
                *kept.lock().unwrap() = serde_json::from_slice(&body).ok();
                Response::builder()
                    .status(200)
                    .header("content-type", "text/event-stream")
                    .body(Body::from(CODEX_SSE))
                    .unwrap()
            }
        }),
    ))
    .await;

    let store = Store::in_memory(&[7u8; 32]).unwrap();
    let mino = store.add_user("mino").unwrap();
    let plan = goat_gateway::oauth::Stored {
        access: "at".into(),
        refresh: None,
        expires_at: None,
        account_id: Some("acct_1".into()),
        client_id: None,
    };
    store
        .add_account("ChatGPT Pro", "chatgpt", "oauth", &plan.to_bytes())
        .unwrap();
    store.set_owner("ChatGPT Pro", Some(&mino.id)).unwrap();
    let key = store.issue_key(&mino.id, "Claude Code").unwrap().secret;
    let catalog = Catalog::builtin().with_base_url("chatgpt", &format!("http://{upstream}"));
    let gateway = serve(App::new(store.clone(), [1u8; 32], catalog).router()).await;

    let answer = reqwest::Client::new()
        .post(format!("http://{gateway}/v1/messages"))
        .header("x-api-key", &key)
        .json(&json!({
            "model": "gpt-5-codex",
            "max_tokens": 1024,
            "stream": true,
            "system": [{ "type": "text", "text": "You are terse." }],
            "messages": [{ "role": "user", "content": "hi" }],
        }))
        .send()
        .await
        .unwrap();
    assert_eq!(answer.status(), 200);
    let text = answer.text().await.unwrap();
    assert!(text.contains("message_start"), "{text}");
    assert!(text.contains("Hello from Codex"), "{text}");
    assert!(text.contains("message_stop"), "{text}");

    let body = sent.lock().unwrap().clone().unwrap();
    assert_eq!(body["instructions"], "You are terse.");
    assert_eq!(body["store"], false);
    assert_eq!(body["stream"], true);
    assert!(
        body.get("max_output_tokens").is_none(),
        "the codex backend refuses max_output_tokens: {body}"
    );

    let row = &store.recent_requests(1).unwrap()[0];
    assert!(row.translated);
    assert_eq!(row.egress, "responses");
    let evidence = row.evidence.clone().unwrap().to_string();
    assert!(
        evidence.contains("/max_output_tokens"),
        "what the provider refused is written down: {evidence}"
    );
}

#[tokio::test]
async fn a_quiet_translated_stream_keeps_claude_code_from_giving_up() {
    let upstream = serve(Router::new().route(
        "/coding/v1/chat/completions",
        post(|| async {
            let pieces = async_stream::stream! {
                yield Ok::<_, std::io::Error>(Bytes::from_static(
                    b"data: {\"choices\":[{\"index\":0,\"delta\":{\"role\":\"assistant\",\"content\":\"thinking it over\"}}]}\n\n",
                ));
                tokio::time::sleep(std::time::Duration::from_millis(400)).await;
                yield Ok(Bytes::from_static(
                    b"data: {\"choices\":[{\"index\":0,\"delta\":{},\"finish_reason\":\"stop\"}]}\n\ndata: [DONE]\n\n",
                ));
            };
            Response::builder()
                .status(200)
                .header("content-type", "text/event-stream")
                .body(Body::from_stream(pieces))
                .unwrap()
        }),
    ))
    .await;

    let store = Store::in_memory(&[7u8; 32]).unwrap();
    store
        .add_account("plan", "kimi", "api_key", b"sk-kimi")
        .unwrap();
    let user = store.add_user("mino").unwrap();
    let key = store.issue_key(&user.id, "Claude Code").unwrap().secret;
    let catalog = Catalog::builtin().with_base_url("kimi", &format!("http://{upstream}"));
    let app =
        App::new(store, [1u8; 32], catalog).pinging_every(std::time::Duration::from_millis(100));
    let gateway = serve(app.router()).await;

    let text = reqwest::Client::new()
        .post(format!("http://{gateway}/v1/messages"))
        .header("x-api-key", &key)
        .json(&json!({
            "model": "kimi-for-coding",
            "max_tokens": 64,
            "stream": true,
            "messages": [{ "role": "user", "content": "hi" }],
        }))
        .send()
        .await
        .unwrap()
        .text()
        .await
        .unwrap();

    assert!(text.contains("event: ping"), "{text}");
    let ping = text.find("event: ping").unwrap();
    let after = &text[ping..];
    assert!(
        after.starts_with("event: ping\ndata: {\"type\":\"ping\"}\n\n"),
        "a ping is a whole frame on its own: {after}"
    );
    assert!(text.contains("message_stop"));
}

async fn kimi_answering(
    content_type: &'static str,
    body: &'static str,
) -> (SocketAddr, String, Store) {
    let upstream = serve(Router::new().route(
        "/coding/v1/chat/completions",
        post(move || async move {
            Response::builder()
                .status(200)
                .header("content-type", content_type)
                .body(Body::from(body))
                .unwrap()
        }),
    ))
    .await;

    let store = Store::in_memory(&[7u8; 32]).unwrap();
    store
        .add_account("coding plan", "kimi", "api_key", b"key-kimi")
        .unwrap();
    let user = store.add_user("jmo").unwrap();
    let key = store.issue_key(&user.id, "test").unwrap().secret;
    let catalog = Catalog::builtin().with_base_url("kimi", &format!("http://{upstream}"));
    let gateway = serve(App::new(store.clone(), [1u8; 32], catalog).router()).await;
    (gateway, key, store)
}

async fn claude_code_asks(gateway: SocketAddr, key: &str, stream: bool) -> (u16, String) {
    let response = reqwest::Client::new()
        .post(format!("http://{gateway}/v1/messages"))
        .header("x-api-key", key)
        .json(&claude_code_turn(stream))
        .send()
        .await
        .unwrap();
    let status = response.status().as_u16();
    (status, response.text().await.unwrap())
}

#[tokio::test]
async fn a_provider_that_ignores_the_stream_it_was_asked_for_fails_rather_than_answering_empty() {
    let (gateway, key, store) = kimi_answering(
        "application/json",
        r#"{"choices":[{"index":0,"message":{"role":"assistant","content":"hello"},"finish_reason":"stop"}]}"#,
    )
    .await;

    for stream in [false, true] {
        let (status, body) = claude_code_asks(gateway, &key, stream).await;
        assert_ne!(
            status, 200,
            "an answer the gateway could not read must not reach the client as a complete one: {body}"
        );
        assert!(body.contains("application/json"), "{body}");

        let row = store
            .recent_requests(1)
            .unwrap()
            .into_iter()
            .next()
            .unwrap();
        assert_eq!(row.status, "error", "stream {stream}");
        assert!(
            row.error_message
                .as_deref()
                .is_some_and(|said| said.contains("hello")),
            "what the provider sent is the only explanation, so it is kept: {:?}",
            row.error_message
        );
    }
}
