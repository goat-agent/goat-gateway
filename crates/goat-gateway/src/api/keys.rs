use axum::{
    Extension, Json,
    extract::{Path, State},
    http::StatusCode,
    response::{IntoResponse, Response},
};
use serde::Deserialize;
use serde_json::json;

use super::{bad_request, failed, forbidden, not_found};
use crate::{
    App,
    store::{KeyRow, Viewer},
};

pub const KEPT_OFF_BY_HAND: &str = "kept off by hand";

fn yours(viewer: &Viewer, key: &KeyRow) -> bool {
    viewer.is_admin()
        || viewer
            .person()
            .is_some_and(|person| person.id == key.user_id)
}

fn may_manage(app: &App, viewer: &Viewer, id: &str) -> Result<KeyRow, Box<Response>> {
    let found = app
        .store()
        .keys()
        .map_err(|error| Box::new(failed(error.to_string())))?
        .into_iter()
        .find(|key| key.id == id);
    match found {
        None => Err(Box::new(not_found(format!("no key has the id {id}")))),
        Some(key) if yours(viewer, &key) => Ok(key),
        Some(_) => Err(Box::new(forbidden("that key is someone else's"))),
    }
}

pub async fn list(State(app): State<App>, Extension(viewer): Extension<Viewer>) -> Response {
    match app.store().keys() {
        Ok(keys) => {
            let shown: Vec<_> = keys.into_iter().filter(|key| yours(&viewer, key)).collect();
            Json(json!({ "keys": shown })).into_response()
        }
        Err(error) => failed(error.to_string()),
    }
}

#[derive(Deserialize)]
pub struct NewKey {
    #[serde(default)]
    user_id: Option<String>,
    label: String,
}

pub async fn issue(
    State(app): State<App>,
    Extension(viewer): Extension<Viewer>,
    Json(body): Json<NewKey>,
) -> Response {
    let label = body.label.trim();
    if label.is_empty() {
        return bad_request("a key needs a label so you can tell your programs apart");
    }
    let holder = match (&viewer, body.user_id) {
        (Viewer::Person(person), _) if !viewer.is_admin() => person.id.clone(),
        (_, Some(id)) => id,
        (Viewer::Person(person), None) => person.id.clone(),
        (Viewer::Root, None) => return bad_request("say whose key this is"),
    };
    match app.store().person(&holder) {
        Ok(Some(_)) => {}
        Ok(None) => return bad_request(format!("nobody here has the id {holder}")),
        Err(error) => return failed(error.to_string()),
    }
    match app.store().issue_key(&holder, label) {
        Ok(issued) => (
            StatusCode::CREATED,
            Json(json!({ "id": issued.id, "key": issued.secret })),
        )
            .into_response(),
        Err(error) => failed(error.to_string()),
    }
}

pub async fn revoke(
    State(app): State<App>,
    Extension(viewer): Extension<Viewer>,
    Path(id): Path<String>,
) -> Response {
    if let Err(refusal) = may_manage(&app, &viewer, &id) {
        return *refusal;
    }
    match app.store().revoke_key(&id) {
        Ok(()) => StatusCode::NO_CONTENT.into_response(),
        Err(error) => failed(error.to_string()),
    }
}

pub async fn demotions(
    State(app): State<App>,
    Extension(viewer): Extension<Viewer>,
    Path(id): Path<String>,
) -> Response {
    if let Err(refusal) = may_manage(&app, &viewer, &id) {
        return *refusal;
    }
    match app.store().demotions(&id) {
        Ok(demotions) => Json(json!({ "demotions": demotions })).into_response(),
        Err(error) => failed(error.to_string()),
    }
}

#[derive(Deserialize)]
pub struct KeepOff {
    account: String,
}

pub async fn keep_off(
    State(app): State<App>,
    Extension(viewer): Extension<Viewer>,
    Path(id): Path<String>,
    Json(body): Json<KeepOff>,
) -> Response {
    if let Err(refusal) = may_manage(&app, &viewer, &id) {
        return *refusal;
    }
    let exists = app
        .store()
        .accounts()
        .map(|accounts| accounts.iter().any(|account| account.name == body.account));
    match exists {
        Ok(true) => {}
        Ok(false) => return not_found(format!("no account is called {:?}", body.account)),
        Err(error) => return failed(error.to_string()),
    }
    match app
        .store()
        .demote(&id, &body.account, None, KEPT_OFF_BY_HAND)
    {
        Ok(()) => StatusCode::NO_CONTENT.into_response(),
        Err(error) => failed(error.to_string()),
    }
}

pub async fn use_again(
    State(app): State<App>,
    Extension(viewer): Extension<Viewer>,
    Path((id, account)): Path<(String, String)>,
) -> Response {
    if let Err(refusal) = may_manage(&app, &viewer, &id) {
        return *refusal;
    }
    match app.store().undemote(&id, &account) {
        Ok(()) => StatusCode::NO_CONTENT.into_response(),
        Err(error) => failed(error.to_string()),
    }
}
