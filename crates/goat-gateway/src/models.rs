use axum::{
    Json,
    extract::State,
    response::{IntoResponse, Response},
};
use serde_json::{Value, json};

use crate::{App, store::AccountState};

pub async fn offered(State(app): State<App>) -> Response {
    let serving: Vec<String> = app
        .store()
        .accounts()
        .unwrap_or_default()
        .into_iter()
        .filter(|account| account.state != AccountState::Disabled)
        .map(|account| account.provider)
        .collect();

    let catalog = app.catalog();
    let mut seen: Vec<&str> = Vec::new();
    let mut data: Vec<Value> = Vec::new();
    for provider in catalog
        .iter()
        .filter(|provider| serving.contains(&provider.id))
    {
        for model in &provider.models {
            if seen.contains(&model.name.as_str()) {
                continue;
            }
            seen.push(&model.name);
            data.push(json!({
                "id": model.name,
                "object": "model",
                "type": "model",
                "display_name": model.name,
                "owned_by": provider.id,
            }));
        }
    }

    Json(json!({
        "object": "list",
        "data": data,
        "has_more": false,
        "first_id": seen.first(),
        "last_id": seen.last(),
    }))
    .into_response()
}
