use axum::{
    Extension,
    body::{Body, Bytes},
    extract::State,
    http::{HeaderMap, HeaderValue, StatusCode},
    response::{IntoResponse, Response},
};
use futures_util::StreamExt as _;
use goat_gateway_wire::{chat_to_messages, chat_to_messages::StreamTarget, messages_to_chat};
use serde_json::Value;

use crate::{
    App, gateway_error,
    provider::Wire,
    serve::{self, Incoming, Ready},
    store::now,
};

pub async fn handle(
    State(app): State<App>,
    Extension(caller): Extension<crate::store::Caller>,
    headers: HeaderMap,
    body: Bytes,
) -> Response {
    serve::dispatch(app, caller, headers, body, Wire::Chat).await
}

pub(crate) async fn from_messages(incoming: Incoming, ready: Ready) -> Response {
    let asked_to_stream = ready
        .request
        .get("stream")
        .and_then(Value::as_bool)
        .unwrap_or(false);

    let model = ready
        .route
        .model
        .as_ref()
        .map_or_else(|| requested(&ready), |declared| declared.name.clone());

    let translated = match messages_to_chat::translate(&incoming.body, &model) {
        Ok(translated) => translated,
        Err(error) => {
            return serve::reject(incoming.wire, StatusCode::BAD_REQUEST, error.to_string());
        }
    };

    let mut going = match serde_json::from_slice::<Value>(&translated.body) {
        Ok(Value::Object(going)) => going,
        _ => return gateway_error("the translated request is not an object".to_owned()),
    };
    going.insert("stream".into(), Value::Bool(true));
    going.insert(
        "stream_options".into(),
        serde_json::json!({ "include_usage": true }),
    );

    let (mut headers, _) = serve::outgoing_headers(&incoming, &ready);
    headers.insert("content-type", HeaderValue::from_static("application/json"));
    headers.insert("accept", HeaderValue::from_static("text/event-stream"));

    let sent = incoming
        .app
        .inner
        .client
        .post(serve::endpoint_url(&ready))
        .headers(headers)
        .body(serde_json::to_vec(&Value::Object(going)).unwrap_or_default())
        .send()
        .await;

    let upstream = match sent {
        Ok(response) => response,
        Err(error) => return gateway_error(format!("upstream unreachable: {error}")),
    };

    crate::observe(
        &incoming.app,
        &ready.account,
        upstream.status().as_u16(),
        upstream.headers(),
    );

    let mut row = serve::opened(&incoming, &ready, None);
    row.byte_identical = Some(false);
    row.ttft_ms = Some(now() - ready.started);
    row.evidence = Some(serde_json::json!({
        "moved": translated.mapping.moved,
        "added": translated.mapping.added,
        "dropped": translated.mapping.dropped,
    }));
    row.upstream_request_id = upstream
        .headers()
        .get("x-request-id")
        .and_then(|value| value.to_str().ok())
        .map(str::to_owned);

    let status = upstream.status().as_u16();
    if !(200..300).contains(&status) {
        let (relayed, said) = crate::relay_failure(upstream).await;
        row.status = "error".into();
        row.error_kind = Some(format!("http_{status}"));
        row.error_message = said;
        row.duration_ms = Some(now() - ready.started);
        let _ = incoming.app.store().record_request(&row);
        return relayed;
    }
    let _ = incoming.app.store().record_request(&row);
    serve::announce_open(&incoming.app, &row);

    let target = StreamTarget {
        message_id: format!("msg_{}", crate::request_id()),
        model,
    };
    let settle = crate::Settle {
        app: incoming.app.clone(),
        row,
        started: ready.started,
        price: ready.route.price(),
    };

    if asked_to_stream {
        return streamed(upstream, target, settle);
    }
    whole(upstream, target, settle).await
}

fn requested(ready: &Ready) -> String {
    ready
        .request
        .get("model")
        .and_then(Value::as_str)
        .unwrap_or_default()
        .to_owned()
}

fn streamed(upstream: reqwest::Response, target: StreamTarget, settle: crate::Settle) -> Response {
    let translator = goat_gateway_wire::chat_to_messages::StreamTranslator::new(target);
    let state = (
        upstream.bytes_stream(),
        translator,
        goat_gateway_wire::Meter::default(),
        Some(settle),
        false,
    );

    let stream = futures_util::stream::unfold(
        state,
        |(mut upstream, mut translator, mut meter, mut settle, done)| async move {
            if done {
                return None;
            }
            let mut close = |meter: &goat_gateway_wire::Meter| {
                if let Some(settle) = settle.take() {
                    settle.finish(meter);
                }
            };
            match upstream.next().await {
                Some(Ok(bytes)) => {
                    let out = translator.push(&bytes);
                    meter.observe(&out);
                    Some((
                        Ok(Bytes::from(out)),
                        (upstream, translator, meter, settle, false),
                    ))
                }
                Some(Err(error)) => {
                    close(&meter);
                    Some((
                        Err(std::io::Error::other(error)),
                        (upstream, translator, meter, settle, true),
                    ))
                }
                None => {
                    let tail = translator.finish();
                    meter.observe(&tail);
                    close(&meter);
                    Some((
                        Ok(Bytes::from(tail)),
                        (upstream, translator, meter, settle, true),
                    ))
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

async fn whole(
    upstream: reqwest::Response,
    target: StreamTarget,
    settle: crate::Settle,
) -> Response {
    let mut translator = goat_gateway_wire::chat_to_messages::StreamTranslator::new(target);
    let mut meter = goat_gateway_wire::Meter::default();
    let mut bytes = upstream.bytes_stream();

    while let Some(piece) = bytes.next().await {
        match piece {
            Ok(piece) => meter.observe(&translator.push(&piece)),
            Err(error) => return gateway_error(format!("the provider's stream broke: {error}")),
        }
    }
    meter.observe(&translator.finish());
    settle.finish(&meter);

    axum::Json(translator.assembled()).into_response()
}

pub(crate) async fn to_messages(incoming: Incoming, ready: Ready) -> Response {
    let asked_to_stream = ready
        .request
        .get("stream")
        .and_then(Value::as_bool)
        .unwrap_or(false);

    let declared = ready.route.model.as_ref();
    let target = chat_to_messages::Target {
        model: declared.map_or_else(|| requested(&ready), |model| model.name.clone()),
        default_max_tokens: declared.and_then(|model| model.max_tokens).unwrap_or(4096),
    };

    let translated = match chat_to_messages::translate(&incoming.body, &target) {
        Ok(translated) => translated,
        Err(error) => {
            return serve::reject(incoming.wire, StatusCode::BAD_REQUEST, error.to_string());
        }
    };

    let mut going = match serde_json::from_slice::<Value>(&translated.body) {
        Ok(Value::Object(going)) => going,
        _ => return gateway_error("the translated request is not an object".to_owned()),
    };
    going.insert("stream".into(), Value::Bool(true));

    let (mut headers, _) = serve::outgoing_headers(&incoming, &ready);
    headers.insert("content-type", HeaderValue::from_static("application/json"));
    headers.insert("accept", HeaderValue::from_static("text/event-stream"));

    let sent = incoming
        .app
        .inner
        .client
        .post(serve::endpoint_url(&ready))
        .headers(headers)
        .body(serde_json::to_vec(&Value::Object(going)).unwrap_or_default())
        .send()
        .await;

    let upstream = match sent {
        Ok(response) => response,
        Err(error) => return gateway_error(format!("upstream unreachable: {error}")),
    };

    crate::observe(
        &incoming.app,
        &ready.account,
        upstream.status().as_u16(),
        upstream.headers(),
    );

    let mut row = serve::opened(&incoming, &ready, None);
    row.byte_identical = Some(false);
    row.ttft_ms = Some(now() - ready.started);
    row.evidence = Some(serde_json::json!({
        "moved": translated.mapping.moved,
        "added": translated.mapping.added,
        "dropped": translated.mapping.dropped,
    }));
    row.upstream_request_id = upstream
        .headers()
        .get("request-id")
        .and_then(|value| value.to_str().ok())
        .map(str::to_owned);

    let status = upstream.status().as_u16();
    if !(200..300).contains(&status) {
        let (relayed, said) = crate::relay_failure(upstream).await;
        row.status = "error".into();
        row.error_kind = Some(format!("http_{status}"));
        row.error_message = said;
        row.duration_ms = Some(now() - ready.started);
        let _ = incoming.app.store().record_request(&row);
        return relayed;
    }
    let _ = incoming.app.store().record_request(&row);
    serve::announce_open(&incoming.app, &row);

    let settle = crate::Settle {
        app: incoming.app.clone(),
        row,
        started: ready.started,
        price: ready.route.price(),
    };
    let out = messages_to_chat::StreamTranslator::new(messages_to_chat::StreamTarget {
        completion_id: format!("chatcmpl_{}", crate::request_id()),
        model: target.model,
        created: now() / 1000,
    });

    if asked_to_stream {
        return as_chat_stream(upstream, out, settle);
    }
    as_one_completion(upstream, out, settle).await
}

fn as_chat_stream(
    upstream: reqwest::Response,
    out: messages_to_chat::StreamTranslator,
    settle: crate::Settle,
) -> Response {
    let state = (
        upstream.bytes_stream(),
        out,
        goat_gateway_wire::Meter::default(),
        Some(settle),
        false,
    );

    let stream = futures_util::stream::unfold(
        state,
        |(mut upstream, mut out, mut meter, mut settle, done)| async move {
            if done {
                return None;
            }
            let mut close = |meter: &goat_gateway_wire::Meter| {
                if let Some(settle) = settle.take() {
                    settle.finish(meter);
                }
            };
            match upstream.next().await {
                Some(Ok(bytes)) => {
                    meter.observe(&bytes);
                    let carried = out.push(&bytes);
                    Some((
                        Ok(Bytes::from(carried)),
                        (upstream, out, meter, settle, false),
                    ))
                }
                Some(Err(error)) => {
                    close(&meter);
                    Some((
                        Err(std::io::Error::other(error)),
                        (upstream, out, meter, settle, true),
                    ))
                }
                None => {
                    let tail = out.finish();
                    close(&meter);
                    Some((Ok(Bytes::from(tail)), (upstream, out, meter, settle, true)))
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

async fn as_one_completion(
    upstream: reqwest::Response,
    mut out: messages_to_chat::StreamTranslator,
    settle: crate::Settle,
) -> Response {
    let mut meter = goat_gateway_wire::Meter::default();
    let mut bytes = upstream.bytes_stream();

    while let Some(piece) = bytes.next().await {
        match piece {
            Ok(piece) => {
                meter.observe(&piece);
                out.push(&piece);
            }
            Err(error) => return gateway_error(format!("the provider's stream broke: {error}")),
        }
    }
    out.finish();
    settle.finish(&meter);

    axum::Json(out.assembled()).into_response()
}
