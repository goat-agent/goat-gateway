use axum::{
    Extension, Json,
    extract::{Path, State},
    http::{HeaderMap, StatusCode, header::SET_COOKIE},
    response::{IntoResponse, Response},
};
use serde::Deserialize;
use serde_json::json;

use super::{bad_request, failed, not_found};
use crate::{
    App, auth,
    store::{Person, Role},
};

const SHORTEST_PASSWORD: usize = 8;

pub async fn health(State(app): State<App>) -> Response {
    let nobody_yet = app
        .store()
        .users()
        .map(|users| users.is_empty())
        .unwrap_or(false);
    Json(json!({ "nobody_yet": nobody_yet })).into_response()
}

#[derive(Deserialize)]
pub struct Credentials {
    #[serde(default)]
    key: Option<String>,
    #[serde(default)]
    name: Option<String>,
    #[serde(default)]
    password: Option<String>,
}

pub async fn open_session(State(app): State<App>, Json(body): Json<Credentials>) -> Response {
    let store = app.store();
    let opened = match (
        body.key.as_deref(),
        body.name
            .as_deref()
            .map(str::trim)
            .filter(|name| !name.is_empty()),
        body.password.as_deref(),
    ) {
        (Some(key), Some(name), Some(password)) => {
            match store.admin_key_matches(key.trim()) {
                Ok(true) => {}
                Ok(false) => {
                    return refused("that admin key is not the one this gateway was started with");
                }
                Err(error) => return failed(error.to_string()),
            }
            if let Some(refusal) = too_short(password) {
                return refusal;
            }
            match store.recover(name, password) {
                Ok(person) => store.open_session(&person.id),
                Err(error) => return failed(error.to_string()),
            }
        }
        (Some(_), _, _) => {
            return bad_request(
                "the admin key only recovers a person: give a name and a new password with it. \
                 A name nobody has yet makes a new admin",
            );
        }
        (None, Some(name), Some(password)) => match store.signed_in(name, password) {
            Ok(Some(person)) => store.open_session(&person.id),
            Ok(None) => return refused("that name and password do not match anyone here"),
            Err(error) => return failed(error.to_string()),
        },
        _ => return bad_request("sign in with a name and password"),
    };
    match opened {
        Ok(token) => signed_in(&token),
        Err(error) => failed(error.to_string()),
    }
}

pub async fn close_session(State(app): State<App>, headers: HeaderMap) -> Response {
    if let Some(token) = auth::session_token(&headers) {
        let _ = app.store().close_session(&token);
    }
    (
        StatusCode::NO_CONTENT,
        [(
            SET_COOKIE,
            format!(
                "{}=; Path=/; HttpOnly; SameSite=Lax; Max-Age=0",
                auth::COOKIE
            ),
        )],
    )
        .into_response()
}

pub async fn me(Extension(viewer): Extension<Person>) -> Response {
    Json(json!({
        "person": viewer,
        "admin": viewer.is_admin(),
    }))
    .into_response()
}

#[derive(Deserialize)]
pub struct NewPassword {
    password: String,
}

pub async fn set_own_password(
    State(app): State<App>,
    Extension(viewer): Extension<Person>,
    Json(body): Json<NewPassword>,
) -> Response {
    if let Some(refusal) = too_short(&body.password) {
        return refusal;
    }
    match app.store().set_password(&viewer.id, &body.password) {
        Ok(()) => StatusCode::NO_CONTENT.into_response(),
        Err(error) => failed(error.to_string()),
    }
}

pub async fn users(State(app): State<App>) -> Response {
    match app.store().users() {
        Ok(users) => Json(json!({ "users": users })).into_response(),
        Err(error) => failed(error.to_string()),
    }
}

#[derive(Deserialize)]
pub struct NewUser {
    name: String,
    #[serde(default)]
    role: Option<Role>,
}

pub async fn add_user(State(app): State<App>, Json(body): Json<NewUser>) -> Response {
    let name = body.name.trim();
    if name.is_empty() {
        return bad_request("a person needs a name");
    }
    let store = app.store();
    let user = match store.add_user(name) {
        Ok(user) => user,
        Err(_) => return bad_request(format!("there is already a person called {name:?}")),
    };
    if let Some(role) = body.role
        && let Err(error) = store.set_role(&user.id, role)
    {
        return failed(error.to_string());
    }
    match store.invite(&user.id) {
        Ok(invite) => (
            StatusCode::CREATED,
            Json(json!({ "user": user, "invite": invite })),
        )
            .into_response(),
        Err(error) => failed(error.to_string()),
    }
}

pub async fn remove_user(State(app): State<App>, Path(id): Path<String>) -> Response {
    match app.store().remove_user(&id) {
        Ok(()) => StatusCode::NO_CONTENT.into_response(),
        Err(error) => failed(error.to_string()),
    }
}

pub async fn invite(State(app): State<App>, Path(id): Path<String>) -> Response {
    match app.store().person(&id) {
        Ok(Some(_)) => {}
        Ok(None) => return not_found(format!("nobody here has the id {id}")),
        Err(error) => return failed(error.to_string()),
    }
    match app.store().invite(&id) {
        Ok(invite) => Json(json!({ "invite": invite })).into_response(),
        Err(error) => failed(error.to_string()),
    }
}

#[derive(Deserialize)]
pub struct NewRole {
    role: Role,
}

pub async fn set_role(
    State(app): State<App>,
    Path(id): Path<String>,
    Json(body): Json<NewRole>,
) -> Response {
    match app.store().set_role(&id, body.role) {
        Ok(()) => StatusCode::NO_CONTENT.into_response(),
        Err(error) => failed(error.to_string()),
    }
}

pub async fn invited(State(app): State<App>, Path(token): Path<String>) -> Response {
    match app.store().invited(&token) {
        Ok(Some(person)) => Json(json!({ "name": person.name })).into_response(),
        Ok(None) => not_found(spent_invite()),
        Err(error) => failed(error.to_string()),
    }
}

pub async fn accept_invite(
    State(app): State<App>,
    Path(token): Path<String>,
    Json(body): Json<NewPassword>,
) -> Response {
    if let Some(refusal) = too_short(&body.password) {
        return refusal;
    }
    let store = app.store();
    match store.accept_invite(&token, &body.password) {
        Ok(Some(person)) => match store.open_session(&person.id) {
            Ok(session) => signed_in(&session),
            Err(error) => failed(error.to_string()),
        },
        Ok(None) => not_found(spent_invite()),
        Err(error) => failed(error.to_string()),
    }
}

fn signed_in(token: &str) -> Response {
    (
        StatusCode::NO_CONTENT,
        [(
            SET_COOKIE,
            format!(
                "{}={token}; Path=/; HttpOnly; SameSite=Lax; Max-Age=2592000",
                auth::COOKIE
            ),
        )],
    )
        .into_response()
}

fn refused(message: &str) -> Response {
    (StatusCode::UNAUTHORIZED, Json(json!({ "error": message }))).into_response()
}

fn too_short(password: &str) -> Option<Response> {
    (password.chars().count() < SHORTEST_PASSWORD).then(|| {
        bad_request(format!(
            "a password needs at least {SHORTEST_PASSWORD} characters"
        ))
    })
}

fn spent_invite() -> &'static str {
    "that invite link has expired or was already used. Ask an admin for a new one"
}
