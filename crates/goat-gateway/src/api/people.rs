use axum::{
    Extension, Json,
    extract::{Path, State},
    http::{HeaderMap, StatusCode, header::SET_COOKIE},
    response::{IntoResponse, Response},
};
use serde::Deserialize;
use serde_json::json;

use super::{bad_request, failed, forbidden, not_found};
use crate::{
    App, auth,
    store::{Role, Viewer},
};

const SHORTEST_PASSWORD: usize = 8;

pub async fn health(State(app): State<App>) -> Response {
    let configured = app.store().has_admin_key().unwrap_or(false);
    Json(json!({ "needs_admin_key": configured })).into_response()
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
        body.name.as_deref(),
        body.password.as_deref(),
    ) {
        (Some(key), _, _) => match store.admin_key_matches(key.trim()) {
            Ok(true) => store.open_session(None),
            Ok(false) => {
                return refused("that admin key is not the one this gateway was started with");
            }
            Err(error) => return failed(error.to_string()),
        },
        (None, Some(name), Some(password)) => match store.signed_in(name, password) {
            Ok(Some(person)) => store.open_session(Some(&person.id)),
            Ok(None) => return refused("that name and password do not match anyone here"),
            Err(error) => return failed(error.to_string()),
        },
        _ => return bad_request("sign in with a name and password, or with the admin key"),
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

pub async fn me(Extension(viewer): Extension<Viewer>) -> Response {
    Json(json!({
        "person": viewer.person(),
        "admin": viewer.is_admin(),
    }))
    .into_response()
}

#[derive(Deserialize)]
pub struct Claim {
    #[serde(default)]
    user_id: Option<String>,
    #[serde(default)]
    name: Option<String>,
}

pub async fn claim(
    State(app): State<App>,
    Extension(viewer): Extension<Viewer>,
    headers: HeaderMap,
    Json(body): Json<Claim>,
) -> Response {
    if viewer != Viewer::Root {
        return forbidden("this session already belongs to someone");
    }
    let Some(token) = auth::session_token(&headers) else {
        return bad_request(
            "claiming needs a console session. Sign in with the admin key in the console first",
        );
    };
    let store = app.store();
    let id = match (body.user_id, body.name.as_deref().map(str::trim)) {
        (Some(id), _) => id,
        (None, Some(name)) if !name.is_empty() => match store.add_user(name) {
            Ok(user) => user.id,
            Err(_) => return bad_request(format!("there is already a person called {name:?}")),
        },
        _ => return bad_request("say who you are: pick a person, or give a name"),
    };
    let claimed = store
        .set_role(&id, Role::Admin)
        .and_then(|()| store.claim_session(&token, &id))
        .and_then(|()| store.person(&id));
    match claimed {
        Ok(Some(person)) => Json(json!({ "person": person })).into_response(),
        Ok(None) => not_found(format!("nobody here has the id {id}")),
        Err(error) => failed(error.to_string()),
    }
}

#[derive(Deserialize)]
pub struct NewPassword {
    password: String,
}

pub async fn set_own_password(
    State(app): State<App>,
    Extension(viewer): Extension<Viewer>,
    Json(body): Json<NewPassword>,
) -> Response {
    let Some(person) = viewer.person() else {
        return bad_request("the admin key has no password. Say who you are first");
    };
    if let Some(refusal) = too_short(&body.password) {
        return refusal;
    }
    match app.store().set_password(&person.id, &body.password) {
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
        Ok(Some(person)) => match store.open_session(Some(&person.id)) {
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
