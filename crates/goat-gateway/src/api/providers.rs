use axum::{
    Json,
    extract::{Path, State},
    http::StatusCode,
    response::{IntoResponse, Response},
};
use serde::Deserialize;
use serde_json::json;

use super::{bad_request, failed, not_found};
use crate::{App, config, provider::Limits};

fn declared_in_config(app: &App) -> Vec<String> {
    let Some(path) = app.config_path() else {
        return Vec::new();
    };
    config::read(path)
        .parse::<toml_edit::DocumentMut>()
        .ok()
        .and_then(|document| {
            document
                .get("provider")
                .and_then(|item| item.as_table_like())
                .map(|table| table.iter().map(|(id, _)| id.to_owned()).collect())
        })
        .unwrap_or_default()
}

pub async fn list(State(app): State<App>) -> Response {
    let catalog = app.catalog();
    let configured = declared_in_config(&app);
    let builtin = crate::provider::Catalog::builtin();
    let accounts = app.store().accounts().unwrap_or_default();

    let providers: Vec<_> = catalog
        .iter()
        .map(|provider| {
            let origin = match (
                configured.contains(&provider.id),
                builtin.get(&provider.id).is_some(),
            ) {
                (true, true) => "overridden",
                (true, false) => "config",
                _ => "built_in",
            };
            json!({
                "id": provider.id,
                "label": provider.label,
                "origin": origin,
                "key": provider.key,
                "endpoints": provider.endpoints.iter().map(|endpoint| json!({
                    "wire": endpoint.wire.id(),
                    "url": endpoint.url,
                })).collect::<Vec<_>>(),
                "limits": match &provider.limits {
                    Limits::None => "none",
                    Limits::Headers => "headers",
                    Limits::Endpoint(_) => "endpoint",
                },
                "models": provider.models.iter().map(|model| json!({
                    "name": model.name,
                    "priced": model.price.is_some(),
                    "thinking": model.thinking,
                    "max_tokens": model.max_tokens,
                    "cache_min_tokens": model.cache_min_tokens,
                })).collect::<Vec<_>>(),
                "sign_in": provider.sign_in.as_ref().map(|flow| json!({
                    "label": flow.label,
                    "modes": flow.modes(),
                })),
                "carries_own_login": !provider.carried.is_empty(),
                "api_key": provider.api_key,
                "accounts": accounts.iter().filter(|account| account.provider == provider.id).count(),
            })
        })
        .collect();
    Json(json!({
        "providers": providers,
        "writable": app.config_path().is_some(),
    }))
    .into_response()
}

pub async fn declaration(State(app): State<App>, Path(id): Path<String>) -> Response {
    let configured = app.config_path().map(config::read).unwrap_or_default();
    let sources = [
        configured.as_str(),
        crate::provider::Catalog::builtin_text(),
    ];
    for source in sources {
        let Ok(document) = source.parse::<toml_edit::DocumentMut>() else {
            continue;
        };
        if let Some(table) = document
            .get("provider")
            .and_then(|item| item.get(&id))
            .and_then(|item| item.as_table())
        {
            let mut alone = toml_edit::DocumentMut::new();
            for (key, item) in table.iter() {
                alone.insert(key, item.clone());
            }
            return Json(json!({ "id": id, "declaration": alone.to_string() })).into_response();
        }
    }
    not_found(format!("no provider is declared as {id:?}"))
}

#[derive(Deserialize)]
pub struct Probe {
    base_url: String,
    secret: String,
    #[serde(default)]
    model: Option<String>,
}

pub async fn discover(State(app): State<App>, Json(body): Json<Probe>) -> Response {
    match crate::discover::discover(
        &app,
        &body.base_url,
        body.secret.trim(),
        body.model
            .as_deref()
            .filter(|model| !model.trim().is_empty()),
    )
    .await
    {
        Ok(found) => Json(found).into_response(),
        Err(message) => bad_request(message),
    }
}

#[derive(Deserialize)]
pub struct Declaration {
    declaration: String,
}

pub async fn save(
    State(app): State<App>,
    Path(id): Path<String>,
    Json(body): Json<Declaration>,
) -> Response {
    let Some(path) = app.config_path().map(std::path::Path::to_path_buf) else {
        return bad_request(
            "this gateway was started without a data directory, so it has no config.toml to write",
        );
    };
    let written = match config::declared(&config::read(&path), &id, &body.declaration) {
        Ok(written) => written,
        Err(error) => return bad_request(error.to_string()),
    };
    reload(&app, &path, &written, StatusCode::OK)
}

pub async fn remove(State(app): State<App>, Path(id): Path<String>) -> Response {
    let Some(path) = app.config_path().map(std::path::Path::to_path_buf) else {
        return bad_request("this gateway has no config.toml to remove a provider from");
    };
    let written = match config::undeclared(&config::read(&path), &id) {
        Ok(written) => written,
        Err(error) => return bad_request(error.to_string()),
    };
    reload(&app, &path, &written, StatusCode::NO_CONTENT)
}

fn reload(app: &App, path: &std::path::Path, written: &str, status: StatusCode) -> Response {
    let catalog = match crate::catalog_from(Some(written)) {
        Ok(catalog) => catalog,
        Err(error) => return bad_request(error.to_string()),
    };
    if let Err(error) = config::write(path, written) {
        return failed(error.to_string());
    }
    app.replace_catalog(catalog);
    status.into_response()
}
