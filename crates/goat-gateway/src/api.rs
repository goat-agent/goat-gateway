use axum::{
    Json, Router,
    extract::{Path, Query, State},
    http::StatusCode,
    response::{IntoResponse, Response},
    routing::{delete, get, post},
};
use std::collections::BTreeMap;

use serde::Deserialize;
use serde_json::json;

use crate::{
    App, auth, pricing,
    store::{
        AccountState,
        insight::{By, Filter},
    },
};

async fn signin_providers() -> Response {
    let providers: Vec<_> = crate::oauth::FLOWS
        .iter()
        .map(|flow| {
            json!({
                "provider": flow.provider,
                "label": flow.label,
                "modes": flow.modes(),
            })
        })
        .collect();
    Json(json!({ "providers": providers })).into_response()
}

#[derive(Deserialize)]
struct BeginSignIn {
    provider: String,
    name: String,
    mode: crate::oauth::Mode,
}

async fn signin_begin(State(app): State<App>, Json(body): Json<BeginSignIn>) -> Response {
    match crate::oauth::begin(
        app.store(),
        &app.oauth_client(),
        app.sessions(),
        &body.provider,
        &body.name,
        body.mode,
    )
    .await
    {
        Ok(started) => Json(started).into_response(),
        Err(error) => refused(&error.to_string()),
    }
}

#[derive(Deserialize)]
struct PastedCode {
    code: String,
}

async fn signin_paste(
    State(app): State<App>,
    Path(session): Path<String>,
    Json(body): Json<PastedCode>,
) -> Response {
    match crate::oauth::finish_paste(
        app.store(),
        &app.oauth_client(),
        app.sessions(),
        &session,
        &body.code,
    )
    .await
    {
        Ok(()) => StatusCode::NO_CONTENT.into_response(),
        Err(error) => refused(&error.to_string()),
    }
}

async fn signin_status(State(app): State<App>, Path(session): Path<String>) -> Response {
    match app.sessions().status(&session) {
        Some(status) => Json(status).into_response(),
        None => (
            StatusCode::NOT_FOUND,
            Json(json!({ "error": "that sign-in is not open anymore. Start again" })),
        )
            .into_response(),
    }
}

fn refused(message: &str) -> Response {
    (StatusCode::BAD_REQUEST, Json(json!({ "error": message }))).into_response()
}

pub fn open_router() -> Router<App> {
    Router::new()
        .route("/api/session", post(open_session).delete(close_session))
        .route("/api/health", get(health))
}

async fn health(State(app): State<App>) -> Response {
    let configured = app.store().has_admin_key().unwrap_or(false);
    Json(json!({ "needs_admin_key": configured })).into_response()
}

#[derive(Deserialize)]
struct SessionRequest {
    key: String,
}

async fn open_session(State(app): State<App>, Json(body): Json<SessionRequest>) -> Response {
    match app.store().admin_key_matches(body.key.trim()) {
        Ok(true) => (
            StatusCode::NO_CONTENT,
            [(
                axum::http::header::SET_COOKIE,
                format!(
                    "{}={}; Path=/; HttpOnly; SameSite=Lax; Max-Age=2592000",
                    auth::COOKIE,
                    body.key.trim()
                ),
            )],
        )
            .into_response(),
        Ok(false) => (
            StatusCode::UNAUTHORIZED,
            Json(json!({ "error": "that admin key is not the one this gateway was started with" })),
        )
            .into_response(),
        Err(error) => failed(error.to_string()),
    }
}

async fn close_session() -> Response {
    (
        StatusCode::NO_CONTENT,
        [(
            axum::http::header::SET_COOKIE,
            format!(
                "{}=; Path=/; HttpOnly; SameSite=Lax; Max-Age=0",
                auth::COOKIE
            ),
        )],
    )
        .into_response()
}

pub fn router() -> Router<App> {
    Router::new()
        .route("/api/overview", get(overview))
        .route("/api/users", get(users).post(add_user))
        .route("/api/users/{id}", delete(remove_user))
        .route("/api/keys", get(keys).post(issue_key))
        .route("/api/keys/{id}", delete(revoke_key))
        .route("/api/accounts", get(accounts).post(add_account))
        .route("/api/accounts/{name}", delete(remove_account))
        .route("/api/accounts/{name}/state", post(set_state))
        .route("/api/requests", get(requests))
        .route("/api/requests/{id}", get(one_request))
        .route("/api/usage", get(usage))
        .route("/api/models", get(models))
        .route("/api/pricing", get(price_table))
        .route("/api/signin/providers", get(signin_providers))
        .route("/api/signin", post(signin_begin))
        .route(
            "/api/signin/{session}",
            get(signin_status).post(signin_paste),
        )
}

#[derive(Deserialize)]
struct Window {
    window_ms: Option<i64>,
}

async fn overview(State(app): State<App>, Query(window): Query<Window>) -> Response {
    let store = app.store();
    let _ = store.clear_expired_cooldowns();

    let window_ms = window.window_ms.unwrap_or(86_400_000).max(60_000);
    let filter = Filter::since(window_ms);

    let (accounts, limits, totals) = match (
        store.accounts(),
        store.all_rate_limits(),
        store.totals(&filter),
    ) {
        (Ok(accounts), Ok(limits), Ok(totals)) => (accounts, limits, totals),
        _ => return failed("could not read the database".to_owned()),
    };

    let seen = |name: &str| limits.iter().find(|(account, ..)| account == name);
    let mut providers: Vec<serde_json::Value> = Vec::new();

    for provider in unique(accounts.iter().map(|account| account.provider.clone())) {
        let declared = app.catalog().get(&provider);
        let mine: Vec<_> = accounts
            .iter()
            .filter(|account| account.provider == provider)
            .collect();

        providers.push(json!({
            "provider": provider,
            "label": declared.map_or(provider.clone(), |declared| declared.label.clone()),
            "accounts": mine.len(),
            "usable": mine.iter().filter(|a| a.state == AccountState::Active).count(),
            "soonest_reset_ms": mine.iter().filter_map(|a| a.cooldown_until).min(),
            "reports_limits": declared
                .is_none_or(|declared| declared.limits != crate::provider::Limits::None),
            "limits": mine.iter().filter_map(|account| {
                seen(&account.name).map(|(name, snapshot, observed_at)| json!({
                    "account": name,
                    "windows": snapshot.windows,
                    "observed_at": observed_at,
                }))
            }).collect::<Vec<_>>(),
        }));
    }

    let attention: Vec<_> = accounts
        .iter()
        .filter(|account| account.state != AccountState::Active)
        .map(|account| {
            json!({
                "account": account.name,
                "provider": account.provider,
                "state": account.state,
                "until": account.cooldown_until,
            })
        })
        .collect();

    Json(json!({
        "now": crate::store::now(),
        "window_ms": window_ms,
        "totals": totals,
        "cache_hit_ratio": totals.cache_hit_ratio(),
        "error_ratio": (totals.requests > 0)
            .then(|| totals.errors as f64 / totals.requests as f64),
        "requests_per_hour": totals.requests as f64 / (window_ms as f64 / 3_600_000.0),
        "providers": providers,
        "attention": attention,
        "pricing_as_of": pricing::AS_OF,
    }))
    .into_response()
}

async fn accounts(State(app): State<App>) -> Response {
    match app.store().accounts() {
        Ok(accounts) => Json(json!({ "accounts": accounts })).into_response(),
        Err(error) => failed(error.to_string()),
    }
}

#[derive(Deserialize)]
struct NewAccount {
    name: String,
    provider: String,
    #[serde(default = "api_key")]
    credential_kind: String,
    secret: String,
}

fn api_key() -> String {
    "api_key".to_owned()
}

async fn add_account(State(app): State<App>, Json(body): Json<NewAccount>) -> Response {
    if body.name.trim().is_empty() {
        return bad_request("an account needs a name");
    }
    if body.secret.trim().is_empty() {
        return bad_request("an account needs a credential");
    }
    match app.store().add_account(
        body.name.trim(),
        &body.provider,
        &body.credential_kind,
        body.secret.trim().as_bytes(),
    ) {
        Ok(()) => (StatusCode::CREATED, Json(json!({ "name": body.name }))).into_response(),
        Err(error) => failed(error.to_string()),
    }
}

async fn remove_account(State(app): State<App>, Path(name): Path<String>) -> Response {
    match app.store().remove_account(&name) {
        Ok(()) => StatusCode::NO_CONTENT.into_response(),
        Err(error) => failed(error.to_string()),
    }
}

#[derive(Deserialize)]
struct SetState {
    state: AccountState,
}

async fn set_state(
    State(app): State<App>,
    Path(name): Path<String>,
    Json(body): Json<SetState>,
) -> Response {
    match app.store().set_state(&name, body.state, None) {
        Ok(()) => StatusCode::NO_CONTENT.into_response(),
        Err(error) => failed(error.to_string()),
    }
}

type Asked = std::collections::HashMap<String, String>;

async fn requests(State(app): State<App>, Query(asked): Query<Asked>) -> Response {
    let filter = Filter::from_query(&asked);
    let limit = crate::store::insight::number(&asked, "limit")
        .unwrap_or(100)
        .clamp(1, 500) as usize;
    let before = crate::store::insight::number(&asked, "before");

    match app.store().find_requests(&filter, before, limit + 1) {
        Ok(mut found) => {
            let more = found.len() > limit;
            found.truncate(limit);
            let next = more
                .then(|| found.last().map(|row| row.started_at))
                .flatten();
            Json(json!({ "requests": found, "next_before": next })).into_response()
        }
        Err(error) => failed(error.to_string()),
    }
}

async fn one_request(State(app): State<App>, Path(id): Path<String>) -> Response {
    match app.store().request(&id) {
        Ok(Some(row)) => Json(json!({ "request": row })).into_response(),
        Ok(None) => (
            StatusCode::NOT_FOUND,
            Json(json!({ "error": format!("no request {id}") })),
        )
            .into_response(),
        Err(error) => failed(error.to_string()),
    }
}

async fn usage(State(app): State<App>, Query(asked): Query<Asked>) -> Response {
    let store = app.store();
    let by = By::named(asked.get("by"));
    let bucket = crate::store::insight::number(&asked, "bucket_ms")
        .unwrap_or(3_600_000)
        .max(60_000);
    let filter = &Filter::from_query(&asked);

    match (
        store.totals(filter),
        store.breakdown(filter, by),
        store.series(filter, by, bucket),
    ) {
        (Ok(totals), Ok(breakdown), Ok(series)) => Json(json!({
            "totals": totals,
            "cache_hit_ratio": totals.cache_hit_ratio(),
            "by": by,
            "bucket_ms": bucket,
            "breakdown": breakdown,
            "series": series.into_iter().map(|(at, slices)| json!({
                "at": at,
                "slices": slices,
            })).collect::<Vec<_>>(),
        }))
        .into_response(),
        _ => failed("could not read the request history".to_owned()),
    }
}

async fn models(State(app): State<App>) -> Response {
    let by_provider: BTreeMap<&str, Vec<&str>> = app
        .catalog()
        .iter()
        .map(|provider| (provider.id.as_str(), provider.model_names()))
        .collect();
    Json(json!(by_provider)).into_response()
}

async fn price_table(State(app): State<App>) -> Response {
    let prices: Vec<_> = app
        .catalog()
        .iter()
        .flat_map(|provider| {
            provider.models.iter().filter_map(move |model| {
                model.price.map(|price| {
                    json!({
                        "provider": provider.id,
                        "model": model.name,
                        "input_per_mtok_micros": price.input,
                        "output_per_mtok_micros": price.output,
                        "cache_read_per_mtok_micros": price.cache_read,
                        "cache_write_per_mtok_micros": price.cache_write,
                    })
                })
            })
        })
        .collect();
    Json(json!({ "as_of": pricing::AS_OF, "prices": prices })).into_response()
}

fn unique(values: impl Iterator<Item = String>) -> Vec<String> {
    let mut seen = Vec::new();
    for value in values {
        if !seen.contains(&value) {
            seen.push(value);
        }
    }
    seen
}

fn failed(message: impl Into<String>) -> Response {
    (
        StatusCode::INTERNAL_SERVER_ERROR,
        Json(json!({ "error": message.into() })),
    )
        .into_response()
}

fn bad_request(message: impl Into<String>) -> Response {
    (
        StatusCode::BAD_REQUEST,
        Json(json!({ "error": message.into() })),
    )
        .into_response()
}

async fn users(State(app): State<App>) -> Response {
    match app.store().users() {
        Ok(users) => Json(json!({ "users": users })).into_response(),
        Err(error) => failed(error.to_string()),
    }
}

#[derive(Deserialize)]
struct NewUser {
    name: String,
}

async fn add_user(State(app): State<App>, Json(body): Json<NewUser>) -> Response {
    let name = body.name.trim();
    if name.is_empty() {
        return bad_request("a user needs a name");
    }
    match app.store().add_user(name) {
        Ok(user) => (StatusCode::CREATED, Json(json!(user))).into_response(),
        Err(_) => bad_request(format!("there is already a user called {name:?}")),
    }
}

async fn remove_user(State(app): State<App>, Path(id): Path<String>) -> Response {
    match app.store().remove_user(&id) {
        Ok(()) => StatusCode::NO_CONTENT.into_response(),
        Err(error) => failed(error.to_string()),
    }
}

async fn keys(State(app): State<App>) -> Response {
    match app.store().keys() {
        Ok(keys) => Json(json!({ "keys": keys })).into_response(),
        Err(error) => failed(error.to_string()),
    }
}

#[derive(Deserialize)]
struct NewKey {
    user_id: String,
    label: String,
}

async fn issue_key(State(app): State<App>, Json(body): Json<NewKey>) -> Response {
    let label = body.label.trim();
    if label.is_empty() {
        return bad_request("a key needs a label so you can tell your machines apart");
    }
    match app.store().issue_key(&body.user_id, label) {
        Ok(issued) => (
            StatusCode::CREATED,
            Json(json!({ "id": issued.id, "key": issued.secret })),
        )
            .into_response(),
        Err(error) => failed(error.to_string()),
    }
}

async fn revoke_key(State(app): State<App>, Path(id): Path<String>) -> Response {
    match app.store().revoke_key(&id) {
        Ok(()) => StatusCode::NO_CONTENT.into_response(),
        Err(error) => failed(error.to_string()),
    }
}
