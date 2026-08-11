use axum::{
    Extension,
    body::Bytes,
    extract::State,
    http::{HeaderMap, StatusCode},
    response::Response,
};
use goat_gateway_wire::{BodyDigest, Record};
use serde_json::Value;

use crate::{
    App, anthropic_error, gateway_error, pool,
    store::{RequestRow, Usage, now},
    upstream::forward_headers,
};

pub async fn handle(
    State(app): State<App>,
    Extension(caller): Extension<crate::store::Caller>,
    headers: HeaderMap,
    body: Bytes,
) -> Response {
    let started = now();
    let request: Value = serde_json::from_slice(&body).unwrap_or(Value::Null);
    let model = request
        .get("model")
        .and_then(Value::as_str)
        .unwrap_or_default()
        .to_owned();
    let declared = app.catalog().model("anthropic", &model).cloned();
    let price = declared.as_ref().and_then(|declared| declared.price);
    let limit_scope = declared
        .as_ref()
        .and_then(|declared| declared.limit_scope.clone());
    let conversation = goat_gateway_wire::identify(&request);
    let prefer = conversation.as_deref().and_then(|conversation| {
        app.inner
            .store
            .account_that_served(conversation)
            .ok()
            .flatten()
    });

    let chosen = match pool::pick(
        &app.inner.store,
        &pool::Want {
            provider: "anthropic",
            scope: limit_scope.as_deref(),
            pinned: None,
            prefer: prefer.as_deref(),
        },
    ) {
        Ok(chosen) => chosen,
        Err(error) => {
            return anthropic_error(
                StatusCode::SERVICE_UNAVAILABLE,
                "api_error",
                error.to_string(),
            );
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
            return anthropic_error(
                StatusCode::SERVICE_UNAVAILABLE,
                "authentication_error",
                error.to_string(),
            );
        }
    };
    let auth = prepared.auth;
    let cache_min_tokens = declared
        .as_ref()
        .and_then(|declared| declared.cache_min_tokens)
        .unwrap_or(0);
    let edits = goat_gateway_wire::breakpoints(&request, cache_min_tokens);

    let outgoing = match goat_gateway_wire::apply(&body, &edits) {
        Ok(bytes) => bytes,
        Err(error) => {
            return anthropic_error(
                StatusCode::BAD_REQUEST,
                "invalid_request_error",
                format!("could not prepare request: {error}"),
            );
        }
    };

    let (upstream_headers, header_edits) = forward_headers(&headers, &auth);
    let record = Record {
        input: BodyDigest::of(&body),
        output: BodyDigest::of(&outgoing),
        body_edits: edits,
        header_edits,
    };

    if let Err(error) = record.verify(&body) {
        return gateway_error(format!("refusing to send an unverified request: {error}"));
    }

    let sent = app
        .inner
        .client
        .post(crate::responses::upstream_url(&app, prepared.base_url))
        .headers(upstream_headers)
        .body(outgoing)
        .send()
        .await;

    let response = match sent {
        Ok(response) => response,
        Err(error) => return gateway_error(format!("upstream unreachable: {error}")),
    };

    let status = response.status().as_u16();
    crate::observe(&app, &chosen.name, status, response.headers());

    let row = RequestRow {
        id: crate::request_id(),
        started_at: started,
        person: Some(caller.user_name.clone()),
        client: client_name(&headers),
        conversation: conversation.clone(),
        provider: "anthropic".into(),
        account: Some(chosen.name.clone()),
        model,
        ingress: "messages".into(),
        egress: "messages".into(),
        translated: false,
        status: if (200..300).contains(&status) {
            "ok".into()
        } else {
            "error".into()
        },
        error_kind: (!(200..300).contains(&status)).then(|| format!("http_{status}")),
        ttft_ms: Some(now() - started),
        duration_ms: None,
        usage: Usage::default(),
        cost_micros: None,
        input_digest: Some(record.input.short()),
        output_digest: Some(record.output.short()),
        byte_identical: Some(record.is_byte_identical()),
        evidence: Some(serde_json::json!({
            "header_edits": record.header_edits,
            "body_edits": record.body_edits,
        })),
        upstream_request_id: response
            .headers()
            .get("request-id")
            .and_then(|value| value.to_str().ok())
            .map(str::to_owned),
    };
    let _ = app.inner.store.record_request(&row);

    if !(200..300).contains(&status) {
        return crate::relay(response);
    }

    crate::relay_metered(
        response,
        Some(crate::Settle {
            app: app.clone(),
            row,
            started,
            price,
        }),
    )
}

pub(crate) fn client_name(headers: &HeaderMap) -> Option<String> {
    let agent = headers.get("user-agent")?.to_str().ok()?;
    Some(match agent {
        agent if agent.contains("claude-cli") => "Claude Code".to_owned(),
        agent if agent.contains("codex") => "Codex".to_owned(),
        agent => agent.split('/').next().unwrap_or(agent).to_owned(),
    })
}
