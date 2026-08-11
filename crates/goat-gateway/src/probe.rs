use serde::Serialize;
use serde_json::{Value, json};

use crate::{
    App,
    provider::{Endpoint, Wire},
    store::{AccountState, StoreError, now},
};

#[derive(Debug, Serialize)]
pub struct Reached {
    pub ok: bool,
    pub status: u16,
    pub model: Option<String>,
    pub said: Option<String>,
    pub took_ms: i64,
}

#[derive(Debug, thiserror::Error)]
pub enum ProbeError {
    #[error(transparent)]
    Store(#[from] StoreError),
    #[error("no account is registered as {0:?}")]
    Unknown(String),
    #[error("{provider} declares no endpoint to try")]
    Nowhere { provider: String },
    #[error("{0}")]
    Credential(String),
    #[error("could not reach the provider: {0}")]
    Unreachable(String),
}

pub async fn reach(app: &App, account: &str) -> Result<Reached, ProbeError> {
    let started = now();
    let row = app
        .store()
        .accounts()?
        .into_iter()
        .find(|row| row.name == account)
        .ok_or_else(|| ProbeError::Unknown(account.to_owned()))?;

    let provider = app
        .catalog()
        .get(&row.provider)
        .ok_or_else(|| ProbeError::Nowhere {
            provider: row.provider.clone(),
        })?;
    let endpoint = provider
        .endpoints
        .first()
        .ok_or_else(|| ProbeError::Nowhere {
            provider: provider.label.clone(),
        })?;
    let model = provider.models.first().map(|model| model.name.clone());

    let secret = app.store().secret(account)?;
    let prepared = crate::oauth::prepare(
        app.store(),
        &app.oauth_client(),
        account,
        &row.provider,
        &row.credential_kind,
        &secret,
    )
    .await
    .map_err(|error| ProbeError::Credential(error.to_string()))?;

    let auth = match prepared.auth {
        crate::upstream::Auth::ApiKey(secret) => {
            crate::upstream::Auth::presented(provider.key, secret)
        }
        carried => carried,
    };
    let (mut headers, _) = crate::upstream::forward_headers(&Default::default(), &auth);
    for (name, value) in &provider.headers {
        if let (Ok(name), Ok(value)) = (
            axum::http::HeaderName::from_bytes(name.as_bytes()),
            axum::http::HeaderValue::from_str(value),
        ) {
            headers.insert(name, value);
        }
    }
    headers.insert(
        "content-type",
        axum::http::HeaderValue::from_static("application/json"),
    );

    let url = match prepared.base_url {
        Some(base) => format!("{}{}", base.trim_end_matches('/'), path_of(&endpoint.url)),
        None => endpoint.url.clone(),
    };

    let sent = app
        .inner
        .client
        .post(url)
        .headers(headers)
        .body(serde_json::to_vec(&knock(endpoint, model.as_deref())).unwrap_or_default())
        .send()
        .await
        .map_err(|error| ProbeError::Unreachable(error.to_string()))?;

    let status = sent.status().as_u16();
    crate::observe(app, account, status, sent.headers());

    let body = sent.bytes().await.unwrap_or_default();
    let ok = (200..300).contains(&status);
    if ok && row.state == AccountState::SignInExpired {
        let _ = app.store().set_state(account, AccountState::Active, None);
    }

    Ok(Reached {
        ok,
        status,
        model,
        said: (!ok).then(|| complaint(&body)).flatten(),
        took_ms: now() - started,
    })
}

fn knock(endpoint: &Endpoint, model: Option<&str>) -> Value {
    let model = model.unwrap_or("");
    match endpoint.wire {
        Wire::Messages => json!({
            "model": model,
            "max_tokens": 1,
            "messages": [{ "role": "user", "content": "." }],
        }),
        Wire::Responses => json!({ "model": model, "input": ".", "max_output_tokens": 16 }),
        Wire::Chat => json!({
            "model": model,
            "max_tokens": 1,
            "messages": [{ "role": "user", "content": "." }],
        }),
    }
}

fn path_of(url: &str) -> &str {
    let rest = url.split_once("://").map_or(url, |(_, rest)| rest);
    rest.find('/').map_or("", |at| &rest[at..])
}

fn complaint(body: &[u8]) -> Option<String> {
    let text = std::str::from_utf8(body).ok()?.trim();
    if text.is_empty() {
        return None;
    }
    let spoken = serde_json::from_str::<Value>(text)
        .ok()
        .and_then(|parsed| {
            parsed
                .get("error")
                .and_then(|error| error.get("message"))
                .or_else(|| parsed.get("message"))
                .and_then(Value::as_str)
                .map(str::to_owned)
        })
        .unwrap_or_else(|| text.to_owned());
    Some(spoken.chars().take(400).collect())
}
