use axum::{
    Extension,
    body::{Body, Bytes},
    extract::State,
    http::{HeaderMap, HeaderValue, StatusCode},
    response::{IntoResponse, Response},
};
use futures_util::StreamExt as _;
use goat_gateway_wire::{
    Provenance, StreamTarget, StreamTranslator,
    responses_to_messages::{Target, translate},
};
use serde_json::Value;

use crate::{
    App, catalog, gateway_error, pool,
    store::now,
    upstream::forward_headers,
};

pub async fn handle(
    State(app): State<App>,
    Extension(caller): Extension<crate::store::Caller>,
    headers: HeaderMap,
    body: Bytes,
) -> Response {
    let request: Value = match serde_json::from_slice(&body) {
        Ok(value) => value,
        Err(error) => {
            return responses_error(StatusCode::BAD_REQUEST, format!("invalid JSON: {error}"));
        }
    };

    let model = request
        .get("model")
        .and_then(Value::as_str)
        .unwrap_or_default();
    let Some(target_model) = catalog::anthropic(model) else {
        return responses_error(
            StatusCode::BAD_REQUEST,
            format!(
                "model {model:?} is not registered on this gateway. Known models: {}",
                catalog::known_anthropic_models().join(", ")
            ),
        );
    };

    if !request
        .get("stream")
        .and_then(Value::as_bool)
        .unwrap_or(false)
    {
        return responses_error(
            StatusCode::BAD_REQUEST,
            "this gateway serves the Responses API in streaming mode only; set stream: true".into(),
        );
    }

    let pinned = pinned_account(&request, &app);
    let chosen = match pool::pick(&app.inner.store, "anthropic", pinned.as_deref()) {
        Ok(chosen) => chosen,
        Err(error) => {
            return responses_error(StatusCode::SERVICE_UNAVAILABLE, error.to_string());
        }
    };
    let prepared = match crate::oauth::prepare(
        &app.inner.store,
        &crate::oauth::Client::with_http(app.inner.client.clone()),
        &chosen.name,
        "anthropic",
        &chosen.credential_kind,
        &chosen.secret,
    )
    .await
    {
        Ok(prepared) => prepared,
        Err(error) => {
            return responses_error(StatusCode::SERVICE_UNAVAILABLE, error.to_string());
        }
    };
    let auth = prepared.auth;

    let provenance = Provenance {
        provider: "anthropic".into(),
        account: chosen.name.clone(),
        model: target_model.name.clone(),
    };

    let target = Target {
        model: target_model,
        provenance: provenance.clone(),
        stream_thinking: true,
    };

    let translated = match translate(&body, &target, &app.inner.envelopes) {
        Ok(translated) => translated,
        Err(error) => return responses_error(StatusCode::BAD_REQUEST, error.to_string()),
    };

    if translated.mapping.lost_anything() {
        tracing::warn!(
            dropped = translated.mapping.dropped.len(),
            "translation dropped state the provider cannot verify",
        );
    }

    let (mut upstream_headers, _) = forward_headers(&headers, &auth);
    upstream_headers.insert("content-type", HeaderValue::from_static("application/json"));
    upstream_headers.insert("anthropic-version", HeaderValue::from_static("2023-06-01"));
    upstream_headers.remove("accept");

    let sent = app
        .inner
        .client
        .post(format!(
            "{}/v1/messages",
            prepared
                .base_url
                .unwrap_or(&app.inner.anthropic_base_url)
                .trim_end_matches('/')
        ))
        .headers(upstream_headers)
        .body(translated.body)
        .send()
        .await;

    let upstream = match sent {
        Ok(response) => response,
        Err(error) => return gateway_error(format!("upstream unreachable: {error}")),
    };

    crate::observe(
        &app,
        &chosen.name,
        upstream.status().as_u16(),
        upstream.headers(),
    );

    if !upstream.status().is_success() {
        return crate::relay(upstream);
    }

    let _ = app.inner.store.record_request(&crate::store::RequestRow {
        id: crate::request_id(),
        started_at: now(),
        person: Some(caller.user_name.clone()),
        client: crate::messages::client_name(&headers),
        provider: "anthropic".into(),
        account: Some(chosen.name.clone()),
        model: provenance.model.clone(),
        ingress: "responses".into(),
        egress: "messages".into(),
        translated: true,
        status: "ok".into(),
        error_kind: None,
        ttft_ms: None,
        duration_ms: None,
        usage: crate::store::Usage::default(),
        cost_micros: None,
        input_digest: None,
        output_digest: None,
        byte_identical: Some(false),
        evidence: Some(serde_json::json!({
            "moved": translated.mapping.moved,
            "added": translated.mapping.added,
            "dropped": translated.mapping.dropped,
        })),
        upstream_request_id: upstream
            .headers()
            .get("request-id")
            .and_then(|value| value.to_str().ok())
            .map(str::to_owned),
    });

    tracing::info!(
        moved = translated.mapping.moved,
        added = translated.mapping.added.len(),
        dropped = translated.mapping.dropped.len(),
        model = %provenance.model,
        "responses",
    );

    let translator = StreamTranslator::new(
        StreamTarget {
            response_id: format!("resp_{}", request_id()),
            model: provenance.model.clone(),
            provenance,
            nonce_seed: nonce_seed(),
        },
        app.inner.envelopes.clone(),
    );

    let stream = futures_util::stream::unfold(
        (upstream.bytes_stream(), translator, false),
        |(mut upstream, mut translator, done)| async move {
            if done {
                return None;
            }
            match upstream.next().await {
                Some(Ok(bytes)) => {
                    let out = translator.push(&bytes);
                    Some((Ok(Bytes::from(out)), (upstream, translator, false)))
                }
                Some(Err(error)) => Some((
                    Err(std::io::Error::other(error)),
                    (upstream, translator, true),
                )),
                None => {
                    let tail = translator.finish();
                    Some((Ok(Bytes::from(tail)), (upstream, translator, true)))
                }
            }
        },
    )
    .filter(|chunk: &Result<Bytes, std::io::Error>| {
        let keep = !matches!(chunk, Ok(bytes) if bytes.is_empty());
        async move { keep }
    });

    Response::builder()
        .status(StatusCode::OK)
        .header("content-type", "text/event-stream")
        .header("cache-control", "no-store")
        .body(Body::from_stream(stream))
        .unwrap_or_else(|error| gateway_error(format!("could not open the stream: {error}")))
}

fn responses_error(status: StatusCode, message: String) -> Response {
    let body = serde_json::json!({
        "error": {
            "type": "invalid_request_error",
            "message": message,
            "code": Value::Null,
            "param": Value::Null,
        }
    });
    (status, axum::Json(body)).into_response()
}

fn request_id() -> String {
    use std::time::{SystemTime, UNIX_EPOCH};
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_nanos())
        .unwrap_or_default();
    format!("{nanos:x}")
}

fn nonce_seed() -> [u8; 8] {
    use std::time::{SystemTime, UNIX_EPOCH};
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_nanos() as u64)
        .unwrap_or_default();
    nanos.to_le_bytes()
}

fn pinned_account(request: &Value, app: &App) -> Option<String> {
    let items = request.get("input")?.as_array()?;
    for item in items.iter().rev() {
        let Some(blob) = item.get("encrypted_content").and_then(Value::as_str) else {
            continue;
        };
        if let Ok(sealed) = app.inner.envelopes.open(blob) {
            return Some(sealed.provenance.account);
        }
    }
    None
}
