use axum::{
    Extension,
    body::{Body, Bytes},
    extract::State,
    http::{HeaderMap, HeaderValue, StatusCode},
    response::Response,
};
use futures_util::StreamExt as _;
use goat_gateway_wire::{
    Provenance, StreamTarget, StreamTranslator,
    responses_to_messages::{Target, translate as into_messages},
};

use crate::{
    App, gateway_error,
    provider::{Model, Wire},
    serve::{self, Incoming, Ready},
};

pub async fn handle(
    State(app): State<App>,
    Extension(caller): Extension<crate::store::Caller>,
    headers: HeaderMap,
    body: Bytes,
) -> Response {
    serve::dispatch(app, caller, headers, body, Wire::Responses).await
}

pub(crate) async fn translate(incoming: Incoming, ready: Ready) -> Response {
    if !ready
        .request
        .get("stream")
        .and_then(serde_json::Value::as_bool)
        .unwrap_or(false)
    {
        return serve::reject(
            incoming.wire,
            StatusCode::BAD_REQUEST,
            "translating between formats needs a stream to translate; set stream: true".into(),
        );
    }

    let (Some(declared), Some(target)) = (
        ready.route.model.clone(),
        ready.route.model.as_ref().and_then(Model::target),
    ) else {
        return serve::reject(
            incoming.wire,
            StatusCode::BAD_REQUEST,
            "this model is not declared well enough to translate".into(),
        );
    };

    let provenance = Provenance {
        provider: ready.route.provider.clone(),
        account: ready.account.clone(),
        model: declared.name.clone(),
    };
    let target = Target {
        model: target,
        provenance: provenance.clone(),
        stream_thinking: true,
    };

    let translated = match into_messages(&incoming.body, &target, &incoming.app.inner.envelopes) {
        Ok(translated) => translated,
        Err(error) => {
            return serve::reject(incoming.wire, StatusCode::BAD_REQUEST, error.to_string());
        }
    };

    if translated.mapping.lost_anything() {
        tracing::warn!(
            dropped = translated.mapping.dropped.len(),
            "translation dropped state the provider cannot verify",
        );
    }

    let (mut headers, _) = serve::outgoing_headers(&incoming, &ready);
    headers.insert("content-type", HeaderValue::from_static("application/json"));
    headers.remove("accept");

    let sent = incoming
        .app
        .inner
        .client
        .post(serve::endpoint_url(&ready))
        .headers(headers)
        .body(translated.body)
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
        row.duration_ms = Some(crate::store::now() - ready.started);
        let _ = incoming.app.store().record_request(&row);
        return relayed;
    }

    let _ = incoming.app.store().record_request(&row);
    serve::announce_open(&incoming.app, &row);

    tracing::info!(
        moved = translated.mapping.moved,
        added = translated.mapping.added.len(),
        dropped = translated.mapping.dropped.len(),
        model = %provenance.model,
        "responses",
    );

    let settle = Some(crate::Settle {
        app: incoming.app.clone(),
        row,
        started: ready.started,
        price: ready.route.price(),
    });
    let translator = StreamTranslator::new(
        StreamTarget {
            response_id: format!("resp_{}", request_id()),
            model: provenance.model.clone(),
            provenance,
            nonce_seed: nonce_seed(),
        },
        incoming.app.inner.envelopes.clone(),
    );
    let meter = goat_gateway_wire::Meter::default();

    let stream = futures_util::stream::unfold(
        (upstream.bytes_stream(), translator, meter, settle, false),
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
                    meter.observe(&bytes);
                    let out = translator.push(&bytes);
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
                    close(&meter);
                    let tail = translator.finish();
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
