use std::collections::{BTreeMap, HashMap};

use axum::{
    Extension, Json,
    extract::{Path, Query, State},
    response::{IntoResponse, Response},
};
use serde::Deserialize;
use serde_json::json;

use super::{failed, not_found};
use crate::{
    App, pricing,
    store::{
        AccountState, Person,
        insight::{By, Filter},
    },
};

type Asked = HashMap<String, String>;

fn scoped(viewer: &Person, mut filter: Filter) -> Filter {
    if !viewer.is_admin() {
        filter.user_id = Some(viewer.id.clone());
    }
    filter
}

#[derive(Deserialize)]
pub struct Window {
    window_ms: Option<i64>,
}

pub async fn overview(
    State(app): State<App>,
    Extension(viewer): Extension<Person>,
    Query(window): Query<Window>,
) -> Response {
    let store = app.store();
    let _ = store.clear_expired_cooldowns();

    let window_ms = window.window_ms.unwrap_or(86_400_000).max(60_000);
    let filter = scoped(&viewer, Filter::since(window_ms));

    let (accounts, limits, totals) = match (
        store.accounts(),
        store.all_rate_limits(),
        store.totals(&filter),
    ) {
        (Ok(accounts), Ok(limits), Ok(totals)) => (accounts, limits, totals),
        _ => return failed("could not read the database"),
    };
    let accounts: Vec<_> = accounts
        .into_iter()
        .filter(|account| super::accounts::visible(&viewer, account))
        .collect();

    let seen = |name: &str| limits.iter().find(|(account, ..)| account == name);
    let mut providers: Vec<serde_json::Value> = Vec::new();
    let catalog = app.catalog();

    for provider in unique(accounts.iter().map(|account| account.provider.clone())) {
        let declared = catalog.get(&provider);
        let mine: Vec<_> = accounts
            .iter()
            .filter(|account| account.provider == provider)
            .collect();

        providers.push(json!({
            "provider": provider,
            "label": declared.map_or(provider.clone(), |declared| declared.label.clone()),
            "accounts": mine.len(),
            "usable": mine.iter().filter(|a| a.state == AccountState::Active).count(),
            "soonest_reset_ms": mine.iter().filter_map(|a| a.cooldown_until).min(),
            "reports_limits": declared
                .is_none_or(|declared| declared.limits != crate::provider::Limits::None),
            "limits": mine.iter().filter_map(|account| {
                seen(&account.name).map(|(name, snapshot, observed_at)| json!({
                    "account": name,
                    "windows": snapshot.windows,
                    "said": snapshot.said,
                    "observed_at": observed_at,
                }))
            }).collect::<Vec<_>>(),
        }));
    }

    let attention: Vec<_> = accounts
        .iter()
        .filter(|account| account.state != AccountState::Active)
        .map(|account| {
            json!({
                "account": account.name,
                "provider": account.provider,
                "state": account.state,
                "until": account.cooldown_until,
            })
        })
        .collect();

    Json(json!({
        "now": crate::store::now(),
        "window_ms": window_ms,
        "totals": totals,
        "cache_hit_ratio": totals.cache_hit_ratio(),
        "error_ratio": (totals.requests > 0)
            .then(|| totals.errors as f64 / totals.requests as f64),
        "requests_per_hour": totals.requests as f64 / (window_ms as f64 / 3_600_000.0),
        "providers": providers,
        "attention": attention,
        "pricing_as_of": pricing::AS_OF,
    }))
    .into_response()
}

pub async fn requests(
    State(app): State<App>,
    Extension(viewer): Extension<Person>,
    Query(asked): Query<Asked>,
) -> Response {
    let filter = scoped(&viewer, Filter::from_query(&asked));
    let limit = crate::store::insight::number(&asked, "limit")
        .unwrap_or(100)
        .clamp(1, 500) as usize;
    let before = crate::store::insight::number(&asked, "before");

    match app.store().find_requests(&filter, before, limit + 1) {
        Ok(mut found) => {
            let more = found.len() > limit;
            found.truncate(limit);
            let next = more
                .then(|| found.last().map(|row| row.started_at))
                .flatten();
            Json(json!({ "requests": found, "next_before": next })).into_response()
        }
        Err(error) => failed(error.to_string()),
    }
}

pub async fn one_request(
    State(app): State<App>,
    Extension(viewer): Extension<Person>,
    Path(id): Path<String>,
) -> Response {
    let mine =
        |owner: &Option<String>| viewer.is_admin() || owner.as_deref() == Some(viewer.id.as_str());
    match app.store().request(&id) {
        Ok(Some(row)) if mine(&row.user_id) => Json(json!({ "request": row })).into_response(),
        Ok(_) => not_found(format!("no request {id}")),
        Err(error) => failed(error.to_string()),
    }
}

pub async fn usage(
    State(app): State<App>,
    Extension(viewer): Extension<Person>,
    Query(asked): Query<Asked>,
) -> Response {
    let store = app.store();
    let by = By::named(asked.get("by"));
    let bucket = crate::store::insight::number(&asked, "bucket_ms")
        .unwrap_or(3_600_000)
        .max(60_000);
    let filter = &scoped(&viewer, Filter::from_query(&asked));

    match (
        store.totals(filter),
        store.breakdown(filter, by),
        store.series(filter, by, bucket),
    ) {
        (Ok(totals), Ok(breakdown), Ok(series)) => Json(json!({
            "totals": totals,
            "cache_hit_ratio": totals.cache_hit_ratio(),
            "by": by,
            "bucket_ms": bucket,
            "breakdown": breakdown,
            "series": series.into_iter().map(|(at, slices)| json!({
                "at": at,
                "slices": slices,
            })).collect::<Vec<_>>(),
        }))
        .into_response(),
        _ => failed("could not read the request history"),
    }
}

pub async fn models(State(app): State<App>) -> Response {
    let catalog = app.catalog();
    let by_provider: BTreeMap<&str, Vec<&str>> = catalog
        .iter()
        .map(|provider| (provider.id.as_str(), provider.model_names()))
        .collect();
    Json(json!(by_provider)).into_response()
}

pub async fn price_table(State(app): State<App>) -> Response {
    let prices: Vec<_> = app
        .catalog()
        .iter()
        .flat_map(|provider| {
            provider.models.iter().filter_map(move |model| {
                model.price.map(|price| {
                    json!({
                        "provider": provider.id,
                        "model": model.name,
                        "input_per_mtok_micros": price.input,
                        "output_per_mtok_micros": price.output,
                        "cache_read_per_mtok_micros": price.cache_read,
                        "cache_write_per_mtok_micros": price.cache_write,
                    })
                })
            })
        })
        .collect();
    Json(json!({ "as_of": pricing::AS_OF, "prices": prices })).into_response()
}

fn unique(values: impl Iterator<Item = String>) -> Vec<String> {
    let mut seen = Vec::new();
    for value in values {
        if !seen.contains(&value) {
            seen.push(value);
        }
    }
    seen
}
