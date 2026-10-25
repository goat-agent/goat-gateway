use axum::{
    body::Bytes,
    http::{HeaderMap, HeaderName, HeaderValue, StatusCode},
    response::{IntoResponse, Response},
};
use goat_gateway_wire::{BodyDigest, Record};
use serde_json::{Value, json};

use crate::{
    App, gateway_error, pool,
    provider::{CarriedCredential, Route, Wire},
    store::{Caller, RequestRow, Usage, now},
    upstream::{Auth, GATEWAY_KEY, forward_headers},
};

const TRIES: usize = 2;

pub struct Incoming {
    pub app: App,
    pub caller: Caller,
    pub headers: HeaderMap,
    pub body: Bytes,
    pub wire: Wire,
    pub id: String,
    pub carried: Option<CarriedCredential>,
}

pub struct Plan {
    pub started: i64,
    pub request: Value,
    pub conversation: Option<String>,
    pub choices: Vec<Choice>,
    pub set_aside: Option<String>,
}

pub enum Choice {
    Carried(CarriedCredential, Route),
    Account(pool::Candidate, Route),
}

impl Choice {
    pub fn route(&self) -> &Route {
        match self {
            Self::Carried(_, route) | Self::Account(_, route) => route,
        }
    }
}

pub struct Ready {
    pub started: i64,
    pub request: Value,
    pub route: Route,
    pub account: Option<String>,
    pub credential: String,
    pub auth: Auth,
    pub conversation: Option<String>,
    pub attempts: Vec<Value>,
}

impl Ready {
    pub fn first_try(&self) -> bool {
        !self
            .attempts
            .iter()
            .any(|attempt| attempt.get("account").is_some())
    }
}

pub enum Outcome {
    Served(Response),
    Declined { response: Response, note: Value },
}

pub fn pinned_account(incoming: &Incoming) -> Option<String> {
    if let Some(account) = incoming.app.turns().account_for(&incoming.headers) {
        return Some(account);
    }
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

pub fn arrive(
    app: App,
    caller: Caller,
    headers: HeaderMap,
    body: Bytes,
    wire: Wire,
) -> Result<Incoming, Box<Response>> {
    let carried = if headers.contains_key(GATEWAY_KEY) {
        match app.catalog().recognize(&headers) {
            None => None,
            Some(Ok(carried)) => Some(carried),
            Some(Err(header)) => {
                return Err(Box::new(reject(
                    wire,
                    StatusCode::UNAUTHORIZED,
                    format!(
                        "the {header} sent beside your gateway key is not a login this gateway \
                         knows how to forward, so it was not sent anywhere. Remove it, or send \
                         only the gateway key"
                    ),
                )));
            }
        }
    } else {
        None
    };
    Ok(Incoming {
        app,
        caller,
        headers,
        body,
        wire,
        id: crate::request_id(),
        carried,
    })
}

pub fn plan(incoming: &Incoming, pinned: Option<&str>) -> Result<Plan, Box<Response>> {
    let Incoming {
        app,
        caller,
        body,
        wire,
        carried,
        ..
    } = incoming;
    let refuse = |status: StatusCode, message: String| Box::new(reject(*wire, status, message));

    let request: Value = serde_json::from_slice(body).map_err(|error| {
        refuse(
            StatusCode::BAD_REQUEST,
            format!("the request body is not valid JSON: {error}"),
        )
    })?;
    let model = request
        .get("model")
        .and_then(Value::as_str)
        .unwrap_or_default();

    let accounts = app.store().accounts().unwrap_or_default();
    let mut available = Vec::new();
    let mut preferred = Vec::new();
    if let Some(carried) = carried {
        available.push(carried.provider.clone());
        preferred.push(carried.provider.clone());
    }
    for account in &accounts {
        let yours = account.owner.as_deref() == Some(caller.user_id.as_str());
        if (yours || account.owner.is_none()) && !available.contains(&account.provider) {
            available.push(account.provider.clone());
        }
        if yours && !preferred.contains(&account.provider) {
            preferred.push(account.provider.clone());
        }
    }

    let routes = app
        .catalog()
        .routes_for(*wire, model, &available, &preferred)
        .map_err(|error| refuse(StatusCode::BAD_REQUEST, error.to_string()))?;
    let conversation = goat_gateway_wire::identify(&request);

    let mut choices = Vec::new();
    let mut set_aside = None;
    if let Some(carried) = carried {
        match routes
            .iter()
            .find(|route| route.provider == carried.provider)
        {
            Some(route) if carried.may_reach(&route.endpoint.url) => {
                choices.push(Choice::Carried(carried.clone(), route.clone()));
            }
            Some(route) => {
                set_aside = Some(format!(
                    "the login sent beside the gateway key may only go to {}, and {} is declared \
                     elsewhere",
                    carried.hosts.join(", "),
                    route.provider
                ));
            }
            None => {
                set_aside = Some(format!(
                    "the login sent beside the gateway key is for {}, and this model is served by \
                     {}",
                    carried.provider,
                    providers_of(&routes).join(" or ")
                ));
            }
        }
    }

    let prefer = conversation.as_deref().and_then(|conversation| {
        app.store()
            .account_that_served(conversation, &caller.user_id)
            .ok()
            .flatten()
    });
    let providers = providers_of(&routes);
    let candidates = pool::candidates(
        app.store(),
        &pool::Want {
            providers: &providers,
            scope: routes.iter().find_map(Route::limit_scope),
            pinned,
            prefer: prefer.as_deref(),
            person: Some(&caller.user_id),
        },
    );
    match candidates {
        Ok(candidates) => choices.extend(candidates.into_iter().filter_map(|candidate| {
            let route = routes
                .iter()
                .find(|route| route.provider == candidate.provider)?
                .clone();
            Some(Choice::Account(candidate, route))
        })),
        Err(error) if choices.is_empty() => {
            return Err(refuse(StatusCode::SERVICE_UNAVAILABLE, error.to_string()));
        }
        Err(_) => {}
    }

    Ok(Plan {
        started: now(),
        request,
        conversation,
        choices,
        set_aside,
    })
}

fn providers_of(routes: &[Route]) -> Vec<&str> {
    routes.iter().map(|route| route.provider.as_str()).collect()
}

pub async fn prepare(
    incoming: &Incoming,
    plan: &Plan,
    choice: &Choice,
    attempts: Vec<Value>,
) -> Result<Ready, Box<Response>> {
    let route = choice.route();
    let (account, credential, auth) = match choice {
        Choice::Carried(carried, _) => (
            None,
            "carried".to_owned(),
            Auth::Carried {
                header: carried.header.clone(),
                value: carried.value.clone(),
            },
        ),
        Choice::Account(candidate, _) => {
            let unavailable = |message: String| {
                Box::new(reject(
                    incoming.wire,
                    StatusCode::SERVICE_UNAVAILABLE,
                    message,
                ))
            };
            let chosen = pool::load(incoming.app.store(), &candidate.name)
                .map_err(|error| unavailable(error.to_string()))?;
            let prepared = crate::oauth::prepare(
                incoming.app.store(),
                &incoming.app.oauth_client(),
                incoming.app.catalog().flow(&route.provider),
                &chosen.name,
                &route.provider,
                &chosen.credential_kind,
                &chosen.secret,
            )
            .await
            .map_err(|error| unavailable(error.to_string()))?;
            let auth = match prepared.auth {
                Auth::ApiKey(secret) => Auth::presented(route.key, secret),
                carried => carried,
            };
            (Some(chosen.name), chosen.credential_kind, auth)
        }
    };

    Ok(Ready {
        started: plan.started,
        request: plan.request.clone(),
        route: route.clone(),
        account,
        credential,
        auth,
        conversation: plan.conversation.clone(),
        attempts,
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

pub fn opened(incoming: &Incoming, ready: &Ready, record: Option<&Record>) -> RequestRow {
    RequestRow {
        id: incoming.id.clone(),
        started_at: ready.started,
        person: Some(incoming.caller.user_name.clone()),
        conversation: ready.conversation.clone(),
        provider: ready.route.provider.clone(),
        account: ready.account.clone(),
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
            json!({
                "header_edits": record.header_edits,
                "body_edits": record.body_edits,
            })
        }),
        upstream_request_id: None,
        user_id: Some(incoming.caller.user_id.clone()),
        key_id: Some(incoming.caller.key_id.clone()),
        credential: Some(
            if ready.account.is_some() {
                "account"
            } else {
                "carried"
            }
            .to_owned(),
        ),
        attempts: (!ready.attempts.is_empty()).then(|| Value::Array(ready.attempts.clone())),
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
    caller: Caller,
    headers: HeaderMap,
    body: Bytes,
    wire: Wire,
) -> Response {
    let incoming = match arrive(app, caller, headers, body, wire) {
        Ok(incoming) => incoming,
        Err(response) => return *response,
    };
    let pinned = pinned_account(&incoming);
    let plan = match plan(&incoming, pinned.as_deref()) {
        Ok(plan) => plan,
        Err(response) => return *response,
    };

    let mut attempts: Vec<Value> = plan
        .set_aside
        .iter()
        .map(|why| json!({ "carried": "set aside", "why": why }))
        .collect();
    let tries = plan.choices.len().min(TRIES);
    for (index, choice) in plan.choices.iter().take(TRIES).enumerate() {
        let egress = choice.route().endpoint.wire;
        let hops = if wire == egress {
            None
        } else {
            match crate::relay_path::path(wire, egress) {
                Some(hops) => Some(hops),
                None => {
                    return reject(
                        wire,
                        StatusCode::BAD_REQUEST,
                        format!("this gateway cannot turn {wire} into {egress}"),
                    );
                }
            }
        };
        let ready = match prepare(&incoming, &plan, choice, attempts.clone()).await {
            Ok(ready) => ready,
            Err(response) => return *response,
        };
        let outcome = match hops {
            None => pass(&incoming, ready).await,
            Some(hops) => crate::translate::run(&incoming, ready, hops).await,
        };
        match outcome {
            Outcome::Served(response) => return response,
            Outcome::Declined { response, note } => {
                if index + 1 >= tries {
                    return response;
                }
                attempts.push(note);
            }
        }
    }
    gateway_error("no account was left to try".to_owned())
}

pub async fn count_tokens(app: App, caller: Caller, headers: HeaderMap, body: Bytes) -> Response {
    let incoming = match arrive(app, caller, headers, body, Wire::Messages) {
        Ok(incoming) => incoming,
        Err(response) => return *response,
    };
    let plan = match plan(&incoming, None) {
        Ok(plan) => plan,
        Err(response) => return *response,
    };
    let Some(first) = plan.choices.first() else {
        return gateway_error("no account was left to try".to_owned());
    };
    let Some(choice) = plan
        .choices
        .iter()
        .find(|choice| choice.route().endpoint.wire == Wire::Messages)
    else {
        return reject(
            Wire::Messages,
            StatusCode::BAD_REQUEST,
            format!(
                "counting tokens needs a provider that serves the Messages format, and {} serves \
                 this model in the {} format",
                first.route().provider,
                first.route().endpoint.wire
            ),
        );
    };
    let ready = match prepare(&incoming, &plan, choice, Vec::new()).await {
        Ok(ready) => ready,
        Err(response) => return *response,
    };

    let (headers, _) = outgoing_headers(&incoming, &ready);
    let sent = incoming
        .app
        .inner
        .client
        .post(format!(
            "{}/count_tokens",
            ready.route.endpoint.url.trim_end_matches('/')
        ))
        .headers(headers)
        .body(incoming.body.clone())
        .send()
        .await;
    match sent {
        Ok(response) => crate::relay_metered(response, None),
        Err(error) => gateway_error(format!("upstream unreachable: {error}")),
    }
}

async fn pass(incoming: &Incoming, ready: Ready) -> Outcome {
    let (outgoing, body_edits) = match prepare_body(incoming, &ready) {
        Ok(prepared) => prepared,
        Err(error) => {
            return Outcome::Served(reject(
                incoming.wire,
                StatusCode::BAD_REQUEST,
                format!("could not prepare the request: {error}"),
            ));
        }
    };
    let (headers, header_edits) = outgoing_headers(incoming, &ready);
    let record = match sealed_record(incoming, &outgoing, body_edits, header_edits) {
        Ok(record) => record,
        Err(error) => {
            return Outcome::Served(gateway_error(format!(
                "refusing to send an unverified request: {error}"
            )));
        }
    };

    let mut row = opened(incoming, &ready, Some(&record));
    let _ = incoming.app.store().record_request(&row);
    if ready.first_try() {
        announce_open(&incoming.app, &row);
    }

    let sent = incoming
        .app
        .inner
        .client
        .post(&ready.route.endpoint.url)
        .headers(headers)
        .body(outgoing)
        .send()
        .await;

    let response = match sent {
        Ok(response) => response,
        Err(error) => {
            return Outcome::Served(unreachable(&incoming.app, row, ready.started, error));
        }
    };

    row.ttft_ms = Some(now() - ready.started);
    row.upstream_request_id = response
        .headers()
        .get("request-id")
        .and_then(|value| value.to_str().ok())
        .map(str::to_owned);

    let status = response.status().as_u16();
    if !(200..300).contains(&status) {
        return answered_with_failure(incoming, &ready, row, response).await;
    }

    if let Some(account) = &ready.account {
        crate::observe(&incoming.app, account, status, response.headers());
        crate::quota::refresh_soon(&incoming.app, account, &ready.route.provider);
    }
    let _ = incoming.app.store().record_request(&row);

    Outcome::Served(crate::relay_metered(
        response,
        Some(crate::Settle {
            app: incoming.app.clone(),
            row,
            started: ready.started,
            price: ready.route.price(),
        }),
    ))
}

pub async fn answered_with_failure(
    incoming: &Incoming,
    ready: &Ready,
    mut row: RequestRow,
    response: reqwest::Response,
) -> Outcome {
    let status = response.status().as_u16();
    let headers = response.headers().clone();
    let failure = crate::relay_failure(response).await;

    row.status = "error".into();
    row.error_kind = Some(format!("http_{status}"));
    row.error_message = failure.said.clone();
    row.duration_ms = Some(now() - ready.started);

    let declined = |kind: &str| {
        json!({
            "account": ready.account,
            "credential": kind,
            "status": status,
            "said": failure.said,
        })
    };

    let Some(account) = &ready.account else {
        settle_failure(&incoming.app, &row);
        let snapshot = crate::limits::parse(&headers, now());
        return match crate::limits::classify(status, &headers, &snapshot) {
            crate::limits::Verdict::Exhausted { .. } => Outcome::Declined {
                note: declined("carried"),
                response: failure.relayed,
            },
            _ => Outcome::Served(failure.relayed),
        };
    };

    let snapshot = crate::observe_limits(&incoming.app, account, &headers);
    let refused = incoming
        .app
        .catalog()
        .get(&ready.route.provider)
        .and_then(|provider| {
            provider.refusal(
                status,
                &failure.body,
                failure.said.as_deref(),
                &ready.credential,
            )
        })
        .is_some();
    if refused {
        row.error_kind = Some("refused".into());
    }
    let taken_out = !refused && crate::judge(&incoming.app, account, status, &headers, &snapshot);
    settle_failure(&incoming.app, &row);

    if refused || taken_out {
        Outcome::Declined {
            note: declined(&ready.credential),
            response: failure.relayed,
        }
    } else {
        Outcome::Served(failure.relayed)
    }
}

pub fn settle_failure(app: &App, row: &RequestRow) {
    let _ = app.store().record_request(row);
    app.announcer()
        .say(crate::events::Happening::RequestSettled {
            id: row.id.clone(),
            status: row.status.clone(),
            duration_ms: row.duration_ms,
            cost_micros: None,
        });
}

pub fn unreachable(
    app: &App,
    mut row: RequestRow,
    started: i64,
    error: reqwest::Error,
) -> Response {
    let message = format!("upstream unreachable: {error}");
    row.status = "error".into();
    row.error_kind = Some("unreachable".into());
    row.error_message = Some(message.clone());
    row.duration_ms = Some(now() - started);
    settle_failure(app, &row);
    gateway_error(message)
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
    let kind = match status {
        StatusCode::SERVICE_UNAVAILABLE | StatusCode::BAD_GATEWAY => "api_error",
        StatusCode::UNAUTHORIZED => "authentication_error",
        _ => "invalid_request_error",
    };
    let body = match wire {
        Wire::Messages => json!({
            "type": "error",
            "error": { "type": kind, "message": message },
        }),
        Wire::Responses | Wire::Chat => json!({
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
