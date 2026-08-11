pub mod api;
pub mod auth;
pub mod catalog;
pub mod limits;
pub mod messages;
pub mod oauth;
pub mod pool;
pub mod pricing;
pub mod responses;
pub mod store;
pub mod upstream;
pub mod web;

use std::sync::Arc;

use axum::{
    Router,
    http::StatusCode,
    response::{IntoResponse, Response},
    routing::post,
};
use goat_gateway_wire::Envelopes;

use crate::store::Store;

#[derive(Clone)]
pub struct App {
    pub(crate) inner: Arc<Inner>,
}

pub(crate) struct Inner {
    pub(crate) client: reqwest::Client,
    pub(crate) store: Store,
    pub(crate) envelopes: Envelopes,
    pub(crate) anthropic_base_url: String,
    pub(crate) sessions: oauth::Sessions,
}

impl App {
    pub fn new(
        store: Store,
        envelope_key: [u8; 32],
        anthropic_base_url: impl Into<String>,
    ) -> Self {
        Self {
            inner: Arc::new(Inner {
                client: reqwest::Client::new(),
                store,
                envelopes: Envelopes::new(&envelope_key),
                anthropic_base_url: anthropic_base_url.into().trim_end_matches('/').to_owned(),
                sessions: oauth::Sessions::default(),
            }),
        }
    }

    pub fn store(&self) -> &Store {
        &self.inner.store
    }

    pub(crate) fn sessions(&self) -> &oauth::Sessions {
        &self.inner.sessions
    }

    pub(crate) fn oauth_client(&self) -> oauth::Client {
        oauth::Client::with_http(self.inner.client.clone())
    }

    pub fn router(self) -> Router {
        let gateway = Router::new()
            .route("/v1/messages", post(messages::handle))
            .route("/v1/responses", post(responses::handle))
            .route_layer(axum::middleware::from_fn_with_state(
                self.clone(),
                auth::gateway,
            ));

        let admin = api::router().route_layer(axum::middleware::from_fn_with_state(
            self.clone(),
            auth::admin,
        ));

        Router::new()
            .merge(gateway)
            .merge(admin)
            .merge(api::open_router())
            .merge(web::router())
            .with_state(self)
    }
}

pub(crate) fn relay(response: reqwest::Response) -> Response {
    let status =
        StatusCode::from_u16(response.status().as_u16()).unwrap_or(StatusCode::BAD_GATEWAY);

    let mut builder = Response::builder().status(status);
    if let Some(headers) = builder.headers_mut() {
        for (name, value) in response.headers() {
            if name == "content-length" || name == "transfer-encoding" {
                continue;
            }
            headers.append(name.clone(), value.clone());
        }
    }

    builder
        .body(axum::body::Body::from_stream(response.bytes_stream()))
        .unwrap_or_else(|error| gateway_error(format!("could not relay response: {error}")))
}

pub(crate) fn gateway_error(message: String) -> Response {
    anthropic_error(StatusCode::BAD_GATEWAY, "api_error", message)
}

pub(crate) fn anthropic_error(status: StatusCode, kind: &str, message: String) -> Response {
    let body = serde_json::json!({
        "type": "error",
        "error": { "type": kind, "message": message },
    });
    (status, axum::Json(body)).into_response()
}

pub fn anthropic_base_url_from_env() -> String {
    std::env::var("GOAT_ANTHROPIC_BASE_URL")
        .unwrap_or_else(|_| "https://api.anthropic.com".to_owned())
}

pub fn envelope_key_from(master_key: &[u8; 32]) -> [u8; 32] {
    use sha2::{Digest as _, Sha256};
    let mut hasher = Sha256::new();
    hasher.update(b"goat-gateway/envelope/v1");
    hasher.update(master_key);
    hasher.finalize().into()
}

pub(crate) fn request_id() -> String {
    use rand::Rng as _;
    let mut bytes = [0u8; 12];
    rand::rng().fill(&mut bytes);
    format!("req_{}", store::encode_hex(&bytes))
}

pub(crate) fn observe(app: &App, account: &str, status: u16, headers: &axum::http::HeaderMap) {
    let snapshot = limits::parse(headers, store::now());
    if !snapshot.is_empty() {
        let _ = app.inner.store.set_rate_limits(account, &snapshot);
    }

    match status {
        429 => {
            let until = limits::retry_after_ms(headers, store::now())
                .unwrap_or_else(|| store::now() + 60_000);
            let _ =
                app.inner
                    .store
                    .set_state(account, store::AccountState::RateLimited, Some(until));
        }
        401 | 403 => {
            let _ = app
                .inner
                .store
                .set_state(account, store::AccountState::SignInExpired, None);
        }
        _ => {}
    }
}
