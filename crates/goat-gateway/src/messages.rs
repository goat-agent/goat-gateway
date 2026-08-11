use axum::{Extension, body::Bytes, extract::State, http::HeaderMap, response::Response};

use crate::{App, provider::Wire, serve};

pub async fn handle(
    State(app): State<App>,
    Extension(caller): Extension<crate::store::Caller>,
    headers: HeaderMap,
    body: Bytes,
) -> Response {
    serve::dispatch(app, caller, headers, body, Wire::Messages).await
}

pub(crate) fn client_name(headers: &HeaderMap) -> Option<String> {
    let agent = headers.get("user-agent")?.to_str().ok()?;
    Some(match agent {
        agent if agent.contains("claude-cli") => "Claude Code".to_owned(),
        agent if agent.contains("codex") => "Codex".to_owned(),
        agent => agent.split('/').next().unwrap_or(agent).to_owned(),
    })
}
