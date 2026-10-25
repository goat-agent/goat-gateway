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
    oauth::Stored,
    store::{AccountRow, AccountState, Person},
};

const SETUP_TOKEN_PREFIX: &str = "sk-ant-oat";
const EVERYONE: &str = "everyone";

pub fn visible(viewer: &Person, account: &AccountRow) -> bool {
    viewer.is_admin() || account.owner.is_none() || owns(viewer, account)
}

fn manageable(viewer: &Person, account: &AccountRow) -> bool {
    viewer.is_admin() || owns(viewer, account)
}

fn owns(viewer: &Person, account: &AccountRow) -> bool {
    account.owner.as_deref() == Some(viewer.id.as_str())
}

fn find(app: &App, name: &str) -> Result<Option<AccountRow>, Box<Response>> {
    app.store()
        .accounts()
        .map(|accounts| accounts.into_iter().find(|account| account.name == name))
        .map_err(|error| Box::new(failed(error.to_string())))
}

fn may_manage(app: &App, viewer: &Person, name: &str) -> Result<(), Box<Response>> {
    match find(app, name)? {
        None => Err(Box::new(not_found(format!(
            "no account is called {name:?}"
        )))),
        Some(account) if manageable(viewer, &account) => Ok(()),
        Some(_) => Err(Box::new(forbidden(format!(
            "{name:?} is not yours. Only its owner or an admin can change it"
        )))),
    }
}

fn may_take_name(app: &App, viewer: &Person, name: &str) -> Result<(), Box<Response>> {
    match find(app, name)? {
        Some(account) if !manageable(viewer, &account) => Err(Box::new(bad_request(format!(
            "an account called {name:?} already exists. Pick another name"
        )))),
        _ => Ok(()),
    }
}

fn owner_for(
    app: &App,
    viewer: &Person,
    asked: Option<&str>,
    personal: bool,
) -> Result<Option<String>, Box<Response>> {
    let chosen = match asked {
        _ if !viewer.is_admin() => Some(viewer.id.clone()),
        Some(EVERYONE) => None,
        Some(id) => Some(id.to_owned()),
        None if personal => Some(viewer.id.clone()),
        None => None,
    };
    match chosen {
        None if personal => Err(Box::new(bad_request(
            "a plan belongs to one person: choose whose it is. \
             Anthropic's and OpenAI's terms do not allow sharing a subscription",
        ))),
        None => Ok(None),
        Some(id) => match app.store().person(&id) {
            Ok(Some(_)) => Ok(Some(id)),
            Ok(None) => Err(Box::new(bad_request(format!(
                "nobody here has the id {id}"
            )))),
            Err(error) => Err(Box::new(failed(error.to_string()))),
        },
    }
}

pub async fn list(State(app): State<App>, Extension(viewer): Extension<Person>) -> Response {
    match app.store().accounts() {
        Ok(accounts) => {
            let shown: Vec<_> = accounts
                .into_iter()
                .filter(|account| visible(&viewer, account))
                .collect();
            Json(json!({ "accounts": shown })).into_response()
        }
        Err(error) => failed(error.to_string()),
    }
}

#[derive(Deserialize)]
pub struct NewAccount {
    name: String,
    provider: String,
    #[serde(default = "api_key")]
    credential_kind: String,
    secret: String,
    #[serde(default)]
    owner: Option<String>,
}

fn api_key() -> String {
    "api_key".to_owned()
}

pub async fn add(
    State(app): State<App>,
    Extension(viewer): Extension<Person>,
    Json(body): Json<NewAccount>,
) -> Response {
    let name = body.name.trim();
    let secret = body.secret.trim();
    if name.is_empty() {
        return bad_request("an account needs a name");
    }
    if secret.is_empty() {
        return bad_request("an account needs a credential");
    }
    let takes_keys = match app.catalog().get(&body.provider) {
        Some(provider) => provider.api_key,
        None => return bad_request(format!("no provider is declared as {:?}", body.provider)),
    };
    if body.credential_kind == "api_key" && !takes_keys {
        return bad_request(format!(
            "{:?} takes no API key. Sign in to it instead",
            body.provider
        ));
    }

    let (kind, stored, personal) = match body.credential_kind.as_str() {
        "api_key" => ("api_key", secret.as_bytes().to_vec(), false),
        "setup_token" => {
            if body.provider != "anthropic" || !secret.starts_with(SETUP_TOKEN_PREFIX) {
                return bad_request(
                    "a setup token comes from `claude setup-token` and starts with sk-ant-oat. \
                     Run it on a machine where Claude Code is signed in and paste what it prints",
                );
            }
            let token = Stored {
                access: secret.to_owned(),
                refresh: None,
                expires_at: None,
                account_id: None,
                client_id: None,
            };
            (crate::oauth::KIND, token.to_bytes(), true)
        }
        other => {
            return bad_request(format!(
                "{other:?} cannot be pasted. Sign in from the console instead"
            ));
        }
    };

    if let Err(refusal) = may_take_name(&app, &viewer, name) {
        return *refusal;
    }
    let owner = match owner_for(&app, &viewer, body.owner.as_deref(), personal) {
        Ok(owner) => owner,
        Err(refusal) => return *refusal,
    };

    let store = app.store();
    match store
        .add_account(name, &body.provider, kind, &stored)
        .and_then(|()| store.set_owner(name, owner.as_deref()))
    {
        Ok(()) => (StatusCode::CREATED, Json(json!({ "name": name }))).into_response(),
        Err(error) => failed(error.to_string()),
    }
}

pub async fn remove(
    State(app): State<App>,
    Extension(viewer): Extension<Person>,
    Path(name): Path<String>,
) -> Response {
    if let Err(refusal) = may_manage(&app, &viewer, &name) {
        return *refusal;
    }
    match app.store().remove_account(&name) {
        Ok(()) => StatusCode::NO_CONTENT.into_response(),
        Err(error) => failed(error.to_string()),
    }
}

#[derive(Deserialize)]
pub struct SetState {
    state: AccountState,
}

pub async fn set_state(
    State(app): State<App>,
    Extension(viewer): Extension<Person>,
    Path(name): Path<String>,
    Json(body): Json<SetState>,
) -> Response {
    if let Err(refusal) = may_manage(&app, &viewer, &name) {
        return *refusal;
    }
    match app.store().set_state(&name, body.state, None) {
        Ok(()) => StatusCode::NO_CONTENT.into_response(),
        Err(error) => failed(error.to_string()),
    }
}

pub async fn test(
    State(app): State<App>,
    Extension(viewer): Extension<Person>,
    Path(name): Path<String>,
) -> Response {
    if let Err(refusal) = may_manage(&app, &viewer, &name) {
        return *refusal;
    }
    match crate::probe::reach(&app, &name).await {
        Ok(reached) => Json(json!(reached)).into_response(),
        Err(error) => failed(error.to_string()),
    }
}

#[derive(Deserialize)]
pub struct NewOwner {
    #[serde(default)]
    owner: Option<String>,
}

pub async fn set_owner(
    State(app): State<App>,
    Extension(viewer): Extension<Person>,
    Path(name): Path<String>,
    Json(body): Json<NewOwner>,
) -> Response {
    let account = match find(&app, &name) {
        Ok(Some(account)) => account,
        Ok(None) => return not_found(format!("no account is called {name:?}")),
        Err(refusal) => return *refusal,
    };
    let personal = account.credential_kind == crate::oauth::KIND;
    let owner = match owner_for(
        &app,
        &viewer,
        Some(body.owner.as_deref().unwrap_or(EVERYONE)),
        personal,
    ) {
        Ok(owner) => owner,
        Err(refusal) => return *refusal,
    };
    match app.store().set_owner(&name, owner.as_deref()) {
        Ok(()) => StatusCode::NO_CONTENT.into_response(),
        Err(error) => failed(error.to_string()),
    }
}

pub async fn signin_providers(State(app): State<App>) -> Response {
    let providers: Vec<_> = app
        .catalog()
        .flows()
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
pub struct BeginSignIn {
    provider: String,
    name: String,
    mode: crate::oauth::Mode,
    #[serde(default)]
    owner: Option<String>,
}

pub async fn signin_begin(
    State(app): State<App>,
    Extension(viewer): Extension<Person>,
    Json(body): Json<BeginSignIn>,
) -> Response {
    if let Err(refusal) = may_take_name(&app, &viewer, body.name.trim()) {
        return *refusal;
    }
    let owner = match owner_for(&app, &viewer, body.owner.as_deref(), true) {
        Ok(Some(owner)) => owner,
        Ok(None) => return bad_request("a plan belongs to one person: choose whose it is"),
        Err(refusal) => return *refusal,
    };
    let Some(flow) = app.catalog().flow(&body.provider).cloned() else {
        return bad_request(format!(
            "{:?} declares no sign-in. Add it with an API key instead",
            body.provider
        ));
    };
    match crate::oauth::begin(
        app.store(),
        &app.oauth_client(),
        app.sessions(),
        &flow,
        &body.name,
        &owner,
        body.mode,
    )
    .await
    {
        Ok(started) => Json(started).into_response(),
        Err(error) => bad_request(error.to_string()),
    }
}

#[derive(Deserialize)]
pub struct PastedCode {
    code: String,
}

pub async fn signin_paste(
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
        Err(error) => bad_request(error.to_string()),
    }
}

pub async fn signin_status(State(app): State<App>, Path(session): Path<String>) -> Response {
    match app.sessions().status(&session) {
        Some(status) => Json(status).into_response(),
        None => not_found("that sign-in is not open anymore. Start again"),
    }
}
