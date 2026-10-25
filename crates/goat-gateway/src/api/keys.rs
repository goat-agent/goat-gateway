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
    store::{KeyRow, Person},
};

fn yours(viewer: &Person, key: &KeyRow) -> bool {
    viewer.is_admin() || viewer.id == key.user_id
}

fn may_manage(app: &App, viewer: &Person, id: &str) -> Result<KeyRow, Box<Response>> {
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

pub async fn list(State(app): State<App>, Extension(viewer): Extension<Person>) -> Response {
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
    Extension(viewer): Extension<Person>,
    Json(body): Json<NewKey>,
) -> Response {
    let label = body.label.trim();
    if label.is_empty() {
        return bad_request("a key needs a label so you can tell your programs apart");
    }
    let holder = match body.user_id {
        Some(id) if viewer.is_admin() => id,
        _ => viewer.id.clone(),
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
    Extension(viewer): Extension<Person>,
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
