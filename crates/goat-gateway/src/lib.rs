pub mod api;
pub mod auth;
pub mod chat;
pub mod events;
pub mod limits;
pub mod messages;
pub mod oauth;
pub mod pool;
pub mod pricing;
pub mod probe;
pub mod provider;
pub mod quota;
pub mod relay_path;
pub mod responses;
pub mod serve;
pub mod store;
pub mod translate;
pub mod turns;
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
    pub(crate) catalog: crate::provider::Catalog,
    pub(crate) sessions: oauth::Sessions,
    pub(crate) announcer: events::Announcer,
    pub(crate) turns: turns::Turns,
    pub(crate) asked: quota::Asked,
}

impl App {
    pub fn new(store: Store, envelope_key: [u8; 32], catalog: crate::provider::Catalog) -> Self {
        Self {
            inner: Arc::new(Inner {
                client: reqwest::Client::new(),
                store,
                envelopes: Envelopes::new(&envelope_key),
                catalog,
                sessions: oauth::Sessions::default(),
                announcer: events::Announcer::default(),
                turns: turns::Turns::default(),
                asked: quota::Asked::default(),
            }),
        }
    }

    pub fn store(&self) -> &Store {
        &self.inner.store
    }

    pub fn turns(&self) -> &turns::Turns {
        &self.inner.turns
    }

    pub fn announcer(&self) -> &events::Announcer {
        &self.inner.announcer
    }

    pub fn catalog(&self) -> &crate::provider::Catalog {
        &self.inner.catalog
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
            .route("/v1/chat/completions", post(chat::handle))
            .route_layer(axum::middleware::from_fn_with_state(
                self.clone(),
                auth::gateway,
            ));

        let admin = api::router()
            .route("/api/events", axum::routing::get(events::stream))
            .route_layer(axum::middleware::from_fn_with_state(
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

pub(crate) async fn relay_failure(response: reqwest::Response) -> (Response, Option<String>) {
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

    let body = response.bytes().await.unwrap_or_default();
    let said = complaint(&body);
    let relayed = builder
        .body(axum::body::Body::from(body))
        .unwrap_or_else(|error| gateway_error(format!("could not relay response: {error}")));
    (relayed, said)
}

fn complaint(body: &[u8]) -> Option<String> {
    let text = std::str::from_utf8(body).ok()?.trim();
    if text.is_empty() {
        return None;
    }
    let spoken = serde_json::from_str::<serde_json::Value>(text)
        .ok()
        .and_then(|parsed| {
            parsed
                .get("error")
                .and_then(|error| error.get("message"))
                .or_else(|| parsed.get("message"))
                .and_then(|message| message.as_str())
                .map(str::to_owned)
        })
        .unwrap_or_else(|| text.to_owned());
    Some(spoken.chars().take(2000).collect())
}

pub(crate) fn relay_metered(response: reqwest::Response, settle: Option<Settle>) -> Response {
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

    let body = match settle {
        None => axum::body::Body::from_stream(response.bytes_stream()),
        Some(settle) => axum::body::Body::from_stream(metered(response.bytes_stream(), settle)),
    };

    builder
        .body(body)
        .unwrap_or_else(|error| gateway_error(format!("could not relay response: {error}")))
}

pub(crate) struct Settle {
    pub app: App,
    pub row: store::RequestRow,
    pub started: i64,
    pub price: Option<provider::Price>,
}

impl Settle {
    fn finish(mut self, meter: &goat_gateway_wire::Meter) {
        let seen = meter.usage();
        self.row.usage = store::Usage {
            input_tokens: seen.input,
            output_tokens: seen.output,
            cache_read_tokens: seen.cache_read,
            cache_write_tokens: seen.cache_write,
            reasoning_tokens: seen.reasoning,
        };
        self.row.cost_micros = self
            .price
            .filter(|_| !seen.is_empty())
            .map(|price| pricing::cost_micros(price, &self.row.usage));
        self.row.duration_ms = Some(store::now() - self.started);

        match meter.failure() {
            Some(kind) => {
                self.row.status = "error".into();
                self.row.error_kind = Some(kind.to_owned());
            }
            None => self.row.status = "ok".into(),
        }

        let _ = self.app.inner.store.record_request(&self.row);
        self.app.announcer().say(events::Happening::RequestSettled {
            id: self.row.id.clone(),
            status: self.row.status.clone(),
            duration_ms: self.row.duration_ms,
            cost_micros: self.row.cost_micros,
        });
    }
}

fn metered(
    upstream: impl futures_util::Stream<Item = reqwest::Result<bytes::Bytes>> + Send + 'static,
    settle: Settle,
) -> impl futures_util::Stream<Item = Result<bytes::Bytes, std::io::Error>> + Send + 'static {
    use futures_util::StreamExt as _;

    let state = (
        Box::pin(upstream),
        goat_gateway_wire::Meter::default(),
        Some(settle),
    );
    futures_util::stream::unfold(state, |(mut upstream, mut meter, settle)| async move {
        match upstream.next().await {
            Some(Ok(bytes)) => {
                meter.observe(&bytes);
                Some((Ok(bytes), (upstream, meter, settle)))
            }
            Some(Err(error)) => {
                if let Some(settle) = settle {
                    settle.finish(&meter);
                }
                Some((Err(std::io::Error::other(error)), (upstream, meter, None)))
            }
            None => {
                settle?.finish(&meter);
                None
            }
        }
    })
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

pub fn load_catalog(dir: &std::path::Path) -> Result<provider::Catalog, provider::CatalogError> {
    let path = dir.join("config.toml");
    let catalog = match std::fs::read_to_string(&path) {
        Ok(overlay) => {
            let catalog = provider::Catalog::with_overlay(&overlay)?;
            tracing::info!(path = %path.display(), "loaded provider overrides");
            catalog
        }
        Err(_) => provider::Catalog::builtin(),
    };
    Ok(match std::env::var("GOAT_ANTHROPIC_BASE_URL") {
        Ok(base) if !base.is_empty() => catalog.with_base_url("anthropic", &base),
        _ => catalog,
    })
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
    app.turns().minted(headers, account);
    let snapshot = limits::parse(headers, store::now());
    if !snapshot.is_empty() {
        let _ = app.inner.store.set_rate_limits(account, &snapshot);
        app.announcer().say(events::Happening::LimitsObserved {
            account: account.to_owned(),
        });
    }

    let moved_to = match limits::classify(status, headers, &snapshot) {
        limits::Verdict::Exhausted { until } => {
            Some((store::AccountState::RateLimited, Some(until)))
        }
        limits::Verdict::SignedOut => Some((store::AccountState::SignInExpired, None)),
        limits::Verdict::Transient | limits::Verdict::Fine => None,
    };

    if let Some((state, until)) = moved_to {
        let _ = app.inner.store.set_state(account, state, until);
        app.announcer().say(events::Happening::AccountChanged {
            account: account.to_owned(),
            state,
            until,
        });
    }
}
