mod accounts;
mod history;
mod keys;
mod people;
mod providers;

use axum::{
    Json, Router,
    http::StatusCode,
    response::{IntoResponse, Response},
    routing::{delete, get, post},
};
use serde_json::json;

use crate::App;

pub fn open_router() -> Router<App> {
    Router::new()
        .route(
            "/api/session",
            post(people::open_session).delete(people::close_session),
        )
        .route("/api/health", get(people::health))
        .route("/api/hello", get(|| async { StatusCode::OK }))
        .route(
            "/api/invites/{token}",
            get(people::invited).post(people::accept_invite),
        )
}

pub fn router() -> Router<App> {
    Router::new()
        .route("/api/me", get(people::me))
        .route("/api/me/claim", post(people::claim))
        .route("/api/me/password", post(people::set_own_password))
        .route("/api/overview", get(history::overview))
        .route("/api/requests", get(history::requests))
        .route("/api/requests/{id}", get(history::one_request))
        .route("/api/usage", get(history::usage))
        .route("/api/models", get(history::models))
        .route("/api/pricing", get(history::price_table))
        .route("/api/accounts", get(accounts::list).post(accounts::add))
        .route("/api/accounts/{name}", delete(accounts::remove))
        .route("/api/accounts/{name}/state", post(accounts::set_state))
        .route("/api/accounts/{name}/test", post(accounts::test))
        .route("/api/signin/providers", get(accounts::signin_providers))
        .route("/api/signin", post(accounts::signin_begin))
        .route(
            "/api/signin/{session}",
            get(accounts::signin_status).post(accounts::signin_paste),
        )
        .route("/api/providers", get(providers::list))
        .route("/api/keys", get(keys::list).post(keys::issue))
        .route("/api/keys/{id}", delete(keys::revoke))
}

pub fn admin_router() -> Router<App> {
    Router::new()
        .route("/api/users", get(people::users).post(people::add_user))
        .route("/api/users/{id}", delete(people::remove_user))
        .route("/api/users/{id}/invite", post(people::invite))
        .route("/api/users/{id}/role", post(people::set_role))
        .route("/api/accounts/{name}/owner", post(accounts::set_owner))
        .route("/api/providers/discover", post(providers::discover))
        .route(
            "/api/providers/{id}",
            get(providers::declaration)
                .put(providers::save)
                .delete(providers::remove),
        )
}

fn answer(status: StatusCode, message: impl Into<String>) -> Response {
    (status, Json(json!({ "error": message.into() }))).into_response()
}

fn failed(message: impl Into<String>) -> Response {
    answer(StatusCode::INTERNAL_SERVER_ERROR, message)
}

fn bad_request(message: impl Into<String>) -> Response {
    answer(StatusCode::BAD_REQUEST, message)
}

fn forbidden(message: impl Into<String>) -> Response {
    answer(StatusCode::FORBIDDEN, message)
}

fn not_found(message: impl Into<String>) -> Response {
    answer(StatusCode::NOT_FOUND, message)
}
