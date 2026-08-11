use axum::{
    body::Bytes,
    http::{HeaderMap, HeaderName, HeaderValue, StatusCode},
    response::{IntoResponse, Response},
};
use goat_gateway_wire::{BodyDigest, Record};
use serde_json::Value;

use crate::{
    App, gateway_error, pool,
    provider::{Route, Wire},
    store::{Caller, RequestRow, Usage, now},
    upstream::{Auth, forward_headers},
};

pub struct Incoming {
    pub app: App,
    pub caller: Caller,
    pub headers: HeaderMap,
    pub body: Bytes,
    pub wire: Wire,
}

pub struct Ready {
    pub started: i64,
    pub request: Value,
    pub route: Route,
    pub account: String,
    pub auth: Auth,
    pub base_url: Option<String>,
    pub conversation: Option<String>,
}

pub fn pinned_account(incoming: &Incoming) -> Option<String> {
    const OPAQUE: &[&str] = &["encrypted_content", "signature", "reasoning_state"];
    let request: Value = serde_json::from_slice(&incoming.body).ok()?;
    let mut found = None;
    let mut stack = vec![&request];
    while let Some(value) = stack.pop() {
        match value {
            Value::Object(map) => {
                for (name, child) in map {
                    if OPAQUE.contains(&name.as_str())
                        && let Some(blob) = child.as_str()
                        && let Ok(sealed) = incoming.app.inner.envelopes.open(blob)
                    {
                        found = Some(sealed.provenance.account);
                    }
                    stack.push(child);
                }
            }
            Value::Array(items) => stack.extend(items),
            _ => {}
        }
    }
    found
}

pub async fn admit(incoming: &Incoming, pinned: Option<&str>) -> Result<Ready, Response> {
    let Incoming {
        app,
        headers: _,
        body,
        wire,
        ..
    } = incoming;
    let started = now();

    let request: Value = serde_json::from_slice(body).map_err(|error| {
        reject(
            *wire,
            StatusCode::BAD_REQUEST,
            format!("the request body is not valid JSON: {error}"),
        )
    })?;

    let model = request
        .get("model")
        .and_then(Value::as_str)
        .unwrap_or_default();
    let registered = app
        .store()
        .accounts()
        .map(|accounts| {
            accounts
                .into_iter()
                .map(|account| account.provider)
                .collect::<Vec<_>>()
        })
        .unwrap_or_default();
    let route = app
        .catalog()
        .route(*wire, model, &registered)
        .map_err(|error| reject(*wire, StatusCode::BAD_REQUEST, error.to_string()))?;

    let conversation = goat_gateway_wire::identify(&request);
    let prefer = conversation
        .as_deref()
        .and_then(|conversation| app.store().account_that_served(conversation).ok().flatten());

    let chosen = pool::pick(
        app.store(),
        &pool::Want {
            provider: &route.provider,
            scope: route.limit_scope(),
            pinned,
            prefer: prefer.as_deref(),
        },
    )
    .map_err(|error| reject(*wire, StatusCode::SERVICE_UNAVAILABLE, error.to_string()))?;

    let prepared = crate::oauth::prepare(
        app.store(),
        &app.oauth_client(),
        &chosen.name,
        &route.provider,
        &chosen.credential_kind,
        &chosen.secret,
    )
    .await
    .map_err(|error| reject(*wire, StatusCode::SERVICE_UNAVAILABLE, error.to_string()))?;

    let auth = match prepared.auth {
        Auth::ApiKey(secret) => Auth::presented(route.key, secret),
        carried => carried,
    };

    Ok(Ready {
        started,
        request,
        route,
        account: chosen.name,
        auth,
        base_url: prepared.base_url.map(str::to_owned),
        conversation,
    })
}

pub fn outgoing_headers(
    incoming: &Incoming,
    ready: &Ready,
) -> (HeaderMap, Vec<goat_gateway_wire::HeaderEdit>) {
    let (mut headers, mut edits) = forward_headers(&incoming.headers, &ready.auth);
    for (name, value) in &ready.route.headers {
        let (Ok(name), Ok(value)) = (
            HeaderName::from_bytes(name.as_bytes()),
            HeaderValue::from_str(value),
        ) else {
            continue;
        };
        if headers.contains_key(&name) {
            continue;
        }
        edits.push(goat_gateway_wire::HeaderEdit::new(name.as_str()));
        headers.insert(name, value);
    }
    (headers, edits)
}

pub fn endpoint_url(ready: &Ready) -> String {
    match ready.base_url.as_deref() {
        Some(base) => format!(
            "{}{}",
            base.trim_end_matches('/'),
            path_of(&ready.route.endpoint.url)
        ),
        None => ready.route.endpoint.url.clone(),
    }
}

fn path_of(url: &str) -> &str {
    let rest = url.split_once("://").map_or(url, |(_, rest)| rest);
    rest.find('/').map_or("", |at| &rest[at..])
}

pub fn opened(incoming: &Incoming, ready: &Ready, record: Option<&Record>) -> RequestRow {
    RequestRow {
        id: crate::request_id(),
        started_at: ready.started,
        person: Some(incoming.caller.user_name.clone()),
        client: crate::messages::client_name(&incoming.headers),
        conversation: ready.conversation.clone(),
        provider: ready.route.provider.clone(),
        account: Some(ready.account.clone()),
        model: ready
            .request
            .get("model")
            .and_then(Value::as_str)
            .unwrap_or_default()
            .to_owned(),
        ingress: incoming.wire.id().to_owned(),
        egress: ready.route.endpoint.wire.id().to_owned(),
        translated: ready.route.translates_from(incoming.wire),
        status: "in_flight".into(),
        error_kind: None,
        error_message: None,
        ttft_ms: None,
        duration_ms: None,
        usage: Usage::default(),
        cost_micros: None,
        input_digest: record.map(|record| record.input.short()),
        output_digest: record.map(|record| record.output.short()),
        byte_identical: record.map(Record::is_byte_identical),
        evidence: record.map(|record| {
            serde_json::json!({
                "header_edits": record.header_edits,
                "body_edits": record.body_edits,
            })
        }),
        upstream_request_id: None,
    }
}

pub fn prepare_body(
    incoming: &Incoming,
    ready: &Ready,
) -> Result<(Vec<u8>, Vec<goat_gateway_wire::BodyEdit>), goat_gateway_wire::EditError> {
    let edits = goat_gateway_wire::breakpoints(&ready.request, ready.route.cache_min_tokens());
    let outgoing = goat_gateway_wire::apply(&incoming.body, &edits)?;
    Ok((outgoing, edits))
}

pub fn sealed_record(
    incoming: &Incoming,
    outgoing: &[u8],
    body_edits: Vec<goat_gateway_wire::BodyEdit>,
    header_edits: Vec<goat_gateway_wire::HeaderEdit>,
) -> Result<Record, goat_gateway_wire::VerifyError> {
    let record = Record {
        input: BodyDigest::of(&incoming.body),
        output: BodyDigest::of(outgoing),
        body_edits,
        header_edits,
    };
    record.verify(&incoming.body)?;
    Ok(record)
}

pub async fn dispatch(
    app: App,
    caller: crate::store::Caller,
    headers: HeaderMap,
    body: Bytes,
    wire: Wire,
) -> Response {
    let incoming = Incoming {
        app,
        caller,
        headers,
        body,
        wire,
    };
    let pinned = pinned_account(&incoming);
    let ready = match admit(&incoming, pinned.as_deref()).await {
        Ok(ready) => ready,
        Err(response) => return response,
    };

    match (wire, ready.route.endpoint.wire) {
        (from, to) if from == to => pass(incoming, ready).await,
        (Wire::Responses, Wire::Messages) => crate::responses::translate(incoming, ready).await,
        (Wire::Messages, Wire::Chat) => crate::chat::from_messages(incoming, ready).await,
        (from, to) => reject(
            from,
            StatusCode::BAD_REQUEST,
            format!("this gateway cannot yet turn {from} into {to}"),
        ),
    }
}

async fn pass(incoming: Incoming, ready: Ready) -> Response {
    let (outgoing, body_edits) = match prepare_body(&incoming, &ready) {
        Ok(prepared) => prepared,
        Err(error) => {
            return reject(
                incoming.wire,
                StatusCode::BAD_REQUEST,
                format!("could not prepare the request: {error}"),
            );
        }
    };
    let (headers, header_edits) = outgoing_headers(&incoming, &ready);
    let record = match sealed_record(&incoming, &outgoing, body_edits, header_edits) {
        Ok(record) => record,
        Err(error) => {
            return gateway_error(format!("refusing to send an unverified request: {error}"));
        }
    };

    let mut row = opened(&incoming, &ready, Some(&record));
    let _ = incoming.app.store().record_request(&row);
    announce_open(&incoming.app, &row);

    let sent = incoming
        .app
        .inner
        .client
        .post(endpoint_url(&ready))
        .headers(headers)
        .body(outgoing)
        .send()
        .await;

    let response = match sent {
        Ok(response) => response,
        Err(error) => return gateway_error(format!("upstream unreachable: {error}")),
    };

    let status = response.status().as_u16();
    crate::observe(&incoming.app, &ready.account, status, response.headers());

    row.ttft_ms = Some(now() - ready.started);
    row.upstream_request_id = response
        .headers()
        .get("request-id")
        .and_then(|value| value.to_str().ok())
        .map(str::to_owned);

    if !(200..300).contains(&status) {
        let (relayed, said) = crate::relay_failure(response).await;
        row.status = "error".into();
        row.error_kind = Some(format!("http_{status}"));
        row.error_message = said;
        row.duration_ms = Some(now() - ready.started);
        let _ = incoming.app.store().record_request(&row);
        return relayed;
    }

    let _ = incoming.app.store().record_request(&row);

    let price = ready.route.price();
    crate::relay_metered(
        response,
        Some(crate::Settle {
            app: incoming.app,
            row,
            started: ready.started,
            price,
        }),
    )
}

pub fn announce_open(app: &App, row: &RequestRow) {
    app.announcer()
        .say(crate::events::Happening::RequestOpened {
            id: row.id.clone(),
            provider: row.provider.clone(),
            account: row.account.clone(),
            model: row.model.clone(),
        });
}

pub fn reject(wire: Wire, status: StatusCode, message: String) -> Response {
    let kind = if status == StatusCode::SERVICE_UNAVAILABLE {
        "api_error"
    } else {
        "invalid_request_error"
    };
    let body = match wire {
        Wire::Messages => serde_json::json!({
            "type": "error",
            "error": { "type": kind, "message": message },
        }),
        Wire::Responses | Wire::Chat => serde_json::json!({
            "error": {
                "type": kind,
                "message": message,
                "code": Value::Null,
                "param": Value::Null,
            }
        }),
    };
    (status, axum::Json(body)).into_response()
}
