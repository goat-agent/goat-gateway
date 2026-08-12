use axum::{
    body::{Body, Bytes},
    http::{HeaderValue, StatusCode},
    response::{IntoResponse, Response},
};
use futures_util::StreamExt as _;
use goat_gateway_wire::{
    Provenance, chat_to_messages, messages_to_chat, messages_to_responses, responses_to_messages,
};
use serde_json::Value;

use crate::{
    gateway_error,
    provider::Wire,
    relay_path::{Ask, Back, Hop},
    serve::{self, Incoming, Ready},
    store::now,
};

pub(crate) async fn run(incoming: Incoming, ready: Ready, hops: &'static [Hop]) -> Response {
    let asked_to_stream = ready
        .request
        .get("stream")
        .and_then(Value::as_bool)
        .unwrap_or(false);

    let declared = ready.route.model.as_ref();
    let model = declared.map_or_else(
        || {
            ready
                .request
                .get("model")
                .and_then(Value::as_str)
                .unwrap_or_default()
                .to_owned()
        },
        |model| model.name.clone(),
    );
    let provenance = Provenance {
        provider: ready.route.provider.clone(),
        account: ready.account.clone(),
        model: model.clone(),
    };

    let needs_reasoning = crate::relay_path::needs_reasoning_declared(hops);

    if !asked_to_stream && incoming.wire == Wire::Responses {
        return serve::reject(
            incoming.wire,
            StatusCode::BAD_REQUEST,
            "translating into the Responses format needs a stream to translate; set stream: true"
                .into(),
        );
    }

    let target = declared.and_then(|model| model.target());
    if needs_reasoning && target.is_none() {
        return serve::reject(
            incoming.wire,
            StatusCode::BAD_REQUEST,
            format!(
                "serving {} from a provider that speaks {} needs the model declared with thinking \
                 and max_tokens, so the gateway knows how it reasons and how much it may write",
                incoming.wire, ready.route.endpoint.wire
            ),
        );
    }

    let default_max_tokens = declared.and_then(|model| model.max_tokens).unwrap_or(4096);
    let ask = Ask {
        model: &model,
        reasoning: target.map(|target| responses_to_messages::Target {
            model: target,
            provenance: provenance.clone(),
            stream_thinking: true,
        }),
        default_max_tokens,
        envelopes: &incoming.app.inner.envelopes,
    };

    let translated = match crate::relay_path::carry(&incoming.body, hops, &ask) {
        Ok(translated) => translated,
        Err(error) => {
            return serve::reject(incoming.wire, StatusCode::BAD_REQUEST, error.to_string());
        }
    };

    if translated.mapping.lost_anything() {
        tracing::warn!(
            dropped = translated.mapping.dropped.len(),
            from = %incoming.wire,
            to = %ready.route.endpoint.wire,
            "translation dropped state the provider cannot carry",
        );
    }

    let mut going = match serde_json::from_slice::<Value>(&translated.body) {
        Ok(Value::Object(going)) => going,
        _ => return gateway_error("the translated request is not an object".to_owned()),
    };
    going.insert("stream".into(), Value::Bool(true));
    if ready.route.endpoint.wire == Wire::Chat {
        going.insert(
            "stream_options".into(),
            serde_json::json!({ "include_usage": true }),
        );
    }

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
        .or_else(|| upstream.headers().get("x-request-id"))
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
    let back = coming_back(&incoming, ready.route.endpoint.wire, &provenance, &model);

    if asked_to_stream {
        return streamed(upstream, back, settle);
    }
    whole(upstream, back, incoming.wire, settle).await
}

fn coming_back(incoming: &Incoming, egress: Wire, provenance: &Provenance, model: &str) -> Back {
    let responses = || {
        Box::new(messages_to_responses::StreamTranslator::new(
            messages_to_responses::StreamTarget {
                response_id: format!("resp_{}", crate::request_id()),
                model: model.to_owned(),
                provenance: provenance.clone(),
                nonce_seed: nonce_seed(),
            },
            incoming.app.inner.envelopes.clone(),
        ))
    };
    let messages = || {
        chat_to_messages::StreamTranslator::new(chat_to_messages::StreamTarget {
            message_id: format!("msg_{}", crate::request_id()),
            model: model.to_owned(),
        })
    };
    let chat = || {
        messages_to_chat::StreamTranslator::new(messages_to_chat::StreamTarget {
            completion_id: format!("chatcmpl_{}", crate::request_id()),
            model: model.to_owned(),
            created: now() / 1000,
        })
    };

    match (egress, incoming.wire) {
        (Wire::Messages, Wire::Responses) => Back::MessagesToResponses(responses()),
        (Wire::Messages, Wire::Chat) => Back::MessagesToChat(chat()),
        (Wire::Chat, Wire::Messages) => Back::ChatToMessages(messages()),
        (Wire::Chat, Wire::Responses) => Back::ChatToResponses(messages(), responses()),
        _ => Back::ChatToMessages(messages()),
    }
}

fn streamed(upstream: reqwest::Response, back: Back, settle: crate::Settle) -> Response {
    let state = (
        upstream.bytes_stream(),
        back,
        goat_gateway_wire::Meter::default(),
        Some(settle),
        false,
    );

    let stream = futures_util::stream::unfold(
        state,
        |(mut upstream, mut back, mut meter, mut settle, done)| async move {
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
                    let carried = back.push(&bytes);
                    Some((
                        Ok(Bytes::from(carried)),
                        (upstream, back, meter, settle, false),
                    ))
                }
                Some(Err(error)) => {
                    close(&meter);
                    Some((
                        Err(std::io::Error::other(error)),
                        (upstream, back, meter, settle, true),
                    ))
                }
                None => {
                    let tail = back.finish();
                    close(&meter);
                    Some((Ok(Bytes::from(tail)), (upstream, back, meter, settle, true)))
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
    mut back: Back,
    wire: Wire,
    settle: crate::Settle,
) -> Response {
    let mut meter = goat_gateway_wire::Meter::default();
    let mut bytes = upstream.bytes_stream();

    while let Some(piece) = bytes.next().await {
        match piece {
            Ok(piece) => {
                meter.observe(&piece);
                back.push(&piece);
            }
            Err(error) => return gateway_error(format!("the provider's stream broke: {error}")),
        }
    }
    back.finish();
    settle.finish(&meter);

    match back.assembled() {
        Some(whole) => axum::Json(whole).into_response(),
        None => gateway_error(format!(
            "the {wire} format cannot be assembled without a stream, which should have been \
             refused before the request was sent"
        )),
    }
}

fn nonce_seed() -> [u8; 8] {
    use std::time::{SystemTime, UNIX_EPOCH};
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|since| since.as_nanos() as u64)
        .unwrap_or_default()
        .to_le_bytes()
}
