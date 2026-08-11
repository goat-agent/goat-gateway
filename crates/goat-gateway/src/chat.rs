use axum::{Extension, body::Bytes, extract::State, http::HeaderMap, response::Response};

use crate::{App, provider::Wire, serve};

pub async fn handle(
    State(app): State<App>,
    Extension(caller): Extension<crate::store::Caller>,
    headers: HeaderMap,
    body: Bytes,
) -> Response {
    serve::dispatch(app, caller, headers, body, Wire::Chat).await
}
