use axum::http::HeaderMap;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Window {
    pub label: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub scope: Option<String>,
    pub used_percent: f64,
    pub resets_at_ms: Option<i64>,
}

impl Window {
    fn applies_to(&self, scope: Option<&str>) -> bool {
        match self.scope.as_deref() {
            None => true,
            Some(mine) => scope == Some(mine),
        }
    }
}

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct Snapshot {
    pub windows: Vec<Window>,
    pub binding: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub said: Option<String>,
}

impl Snapshot {
    pub fn is_empty(&self) -> bool {
        self.windows.is_empty()
    }

    pub fn pressure_for(&self, scope: Option<&str>) -> Option<f64> {
        self.windows
            .iter()
            .filter(|window| window.applies_to(scope))
            .map(|window| window.used_percent)
            .fold(None, |worst: Option<f64>, used| {
                Some(worst.map_or(used, |value| value.max(used)))
            })
    }

    pub fn soonest_reset(&self) -> Option<i64> {
        self.windows.iter().filter_map(|w| w.resets_at_ms).min()
    }
}

pub fn from_answer(
    body: &serde_json::Value,
    asked: &[crate::provider::WindowAt],
    now_ms: i64,
) -> Snapshot {
    let number = |pointer: &Option<String>| {
        pointer
            .as_deref()
            .and_then(|pointer| body.pointer(pointer))
            .and_then(reading)
    };

    let windows = asked
        .iter()
        .filter_map(|want| {
            let used = reading(body.pointer(&want.used_percent)?)?;
            let resets_at_ms = number(&want.resets_at)
                .map(|at| {
                    if at > 1_000_000_000_000.0 {
                        at
                    } else {
                        at * 1000.0
                    }
                })
                .or_else(|| {
                    number(&want.resets_in_seconds).map(|left| now_ms as f64 + left * 1000.0)
                })
                .map(|at| at as i64);
            Some(Window {
                label: want.label.clone(),
                scope: want.scope.clone(),
                used_percent: if used <= 1.0 { used * 100.0 } else { used },
                resets_at_ms,
            })
        })
        .collect::<Vec<_>>();

    let said = windows.is_empty().then(|| {
        let text = body.to_string();
        text.chars().take(2000).collect()
    });

    Snapshot {
        windows,
        binding: None,
        said,
    }
}

fn reading(value: &serde_json::Value) -> Option<f64> {
    value
        .as_f64()
        .or_else(|| value.as_str().and_then(parse_utilization))
}

pub fn parse(headers: &HeaderMap, now_ms: i64) -> Snapshot {
    let mut windows = Vec::new();

    for (name, value) in headers {
        let name = name.as_str();
        let Some(period) = name
            .strip_prefix("anthropic-ratelimit-unified-")
            .and_then(|rest| rest.strip_suffix("-utilization"))
        else {
            continue;
        };
        let Ok(text) = value.to_str() else { continue };
        let Some(used) = parse_utilization(text) else {
            continue;
        };

        let reset_header = format!("anthropic-ratelimit-unified-{period}-reset");
        let resets_at_ms = headers
            .get(reset_header.as_str())
            .and_then(|value| value.to_str().ok())
            .and_then(|text| parse_reset(text, now_ms));

        let (period, scope) = match period.split_once('_') {
            Some((period, scope)) => (period, Some(scope.to_owned())),
            None => (period, None),
        };

        windows.push(Window {
            label: label_for_period(period),
            scope,
            used_percent: used,
            resets_at_ms,
        });
    }

    for bucket in codex_buckets(headers) {
        for half in ["primary", "secondary"] {
            let used = headers
                .get(format!("x-{bucket}-{half}-used-percent").as_str())
                .and_then(|value| value.to_str().ok())
                .and_then(parse_utilization);
            let Some(used) = used else { continue };

            let resets_at_ms = headers
                .get(format!("x-{bucket}-{half}-reset-at").as_str())
                .and_then(|value| value.to_str().ok())
                .and_then(|text| text.parse::<i64>().ok())
                .map(|seconds| seconds * 1000)
                .or_else(|| {
                    headers
                        .get(format!("x-{bucket}-{half}-reset-after-seconds").as_str())
                        .and_then(|value| value.to_str().ok())
                        .and_then(|text| text.parse::<i64>().ok())
                        .map(|seconds| now_ms + seconds * 1000)
                });

            let label = headers
                .get(format!("x-{bucket}-{half}-window-minutes").as_str())
                .and_then(|value| value.to_str().ok())
                .and_then(|text| text.trim().parse::<i64>().ok())
                .map_or_else(
                    || {
                        if half == "primary" {
                            "5h".to_owned()
                        } else {
                            "weekly".to_owned()
                        }
                    },
                    label_for_minutes,
                );

            windows.push(Window {
                label,
                scope: (bucket != "codex").then(|| bucket.replace('-', "_")),
                used_percent: used,
                resets_at_ms,
            });
        }
    }

    windows.extend(budgets(headers, now_ms));

    let binding = headers
        .get("anthropic-ratelimit-unified-representative-claim")
        .and_then(|value| value.to_str().ok())
        .map(|claim| match claim {
            "five_hour" => "5h".to_owned(),
            "seven_day" => "weekly".to_owned(),
            other => other.to_owned(),
        });

    windows.sort_by(|a, b| a.label.cmp(&b.label));
    Snapshot {
        windows,
        binding,
        said: None,
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Verdict {
    Fine,
    Transient,
    Exhausted { until: i64 },
    SignedOut,
}

pub fn classify(status: u16, headers: &HeaderMap, snapshot: &Snapshot) -> Verdict {
    match status {
        401 | 403 => Verdict::SignedOut,
        429 if snapshot.is_empty() && !headers.contains_key("retry-after") => Verdict::Transient,
        429 => Verdict::Exhausted {
            until: retry_after_ms(headers, now()).unwrap_or_else(|| now() + 60_000),
        },
        _ => Verdict::Fine,
    }
}

fn now() -> i64 {
    crate::store::now()
}

pub fn retry_after_ms(headers: &HeaderMap, now_ms: i64) -> Option<i64> {
    let text = |name: &str| headers.get(name).and_then(|value| value.to_str().ok());
    if let Some(millis) = text("retry-after-ms").and_then(|text| text.trim().parse::<i64>().ok()) {
        return Some(now_ms + millis);
    }
    if let Some(said) = text("retry-after").map(str::trim) {
        if let Ok(seconds) = said.parse::<i64>() {
            return Some(now_ms + seconds * 1000);
        }
        if let Ok(when) =
            time::OffsetDateTime::parse(said, &time::format_description::well_known::Rfc2822)
        {
            return Some((when.unix_timestamp_nanos() / 1_000_000) as i64);
        }
    }
    parse(headers, now_ms).soonest_reset()
}

fn budgets(headers: &HeaderMap, now_ms: i64) -> Vec<Window> {
    let text = |name: &str| headers.get(name).and_then(|value| value.to_str().ok());
    let mut found = Vec::new();

    for name in headers.keys().map(|name| name.as_str()) {
        let named = if let Some(dimension) = name.strip_prefix("x-ratelimit-remaining-") {
            Some((
                dimension,
                format!("x-ratelimit-limit-{dimension}"),
                format!("x-ratelimit-reset-{dimension}"),
            ))
        } else {
            name.strip_prefix("anthropic-ratelimit-")
                .and_then(|rest| rest.strip_suffix("-remaining"))
                .filter(|dimension| !dimension.starts_with("unified"))
                .map(|dimension| {
                    (
                        dimension,
                        format!("anthropic-ratelimit-{dimension}-limit"),
                        format!("anthropic-ratelimit-{dimension}-reset"),
                    )
                })
        };
        let Some((dimension, limit_name, reset_name)) = named else {
            continue;
        };

        let number = |name: &str| text(name).and_then(|text| text.trim().parse::<f64>().ok());
        let (Some(limit), Some(remaining)) = (number(&limit_name), number(name)) else {
            continue;
        };
        if limit <= 0.0 {
            continue;
        }

        found.push(Window {
            label: dimension.to_owned(),
            scope: None,
            used_percent: ((limit - remaining) / limit * 100.0).clamp(0.0, 100.0),
            resets_at_ms: text(&reset_name).and_then(|text| {
                duration_ms(text)
                    .map(|left| now_ms + left)
                    .or_else(|| parse_reset(text, now_ms))
            }),
        });
    }
    found
}

fn duration_ms(text: &str) -> Option<i64> {
    let mut rest = text.trim();
    if rest.is_empty() {
        return None;
    }
    let mut total = 0.0;
    while !rest.is_empty() {
        let digits = rest.find(|c: char| !(c.is_ascii_digit() || c == '.'))?;
        if digits == 0 {
            return None;
        }
        let (number, tail) = rest.split_at(digits);
        let unit_ends = tail
            .find(|c: char| c.is_ascii_digit() || c == '.')
            .unwrap_or(tail.len());
        let (unit, next) = tail.split_at(unit_ends);
        let scale = match unit {
            "h" => 3_600_000.0,
            "m" => 60_000.0,
            "s" => 1_000.0,
            "ms" => 1.0,
            "us" | "µs" => 0.001,
            "ns" => 0.000_001,
            _ => return None,
        };
        total += number.parse::<f64>().ok()? * scale;
        rest = next;
    }
    Some(total.round() as i64)
}

fn codex_buckets(headers: &HeaderMap) -> Vec<String> {
    let mut found: Vec<String> = headers
        .keys()
        .filter_map(|name| {
            name.as_str()
                .strip_suffix("-primary-used-percent")?
                .strip_prefix("x-")
                .map(str::to_owned)
        })
        .filter(|bucket| !bucket.starts_with("ratelimit"))
        .collect();
    found.sort();
    found.dedup();
    found
}

fn label_for_period(period: &str) -> String {
    match period {
        "5h" => "5h".to_owned(),
        "7d" => "weekly".to_owned(),
        other => other.to_owned(),
    }
}

fn label_for_minutes(minutes: i64) -> String {
    match minutes {
        300 => "5h".to_owned(),
        1440 => "daily".to_owned(),
        10080 => "weekly".to_owned(),
        43200 => "monthly".to_owned(),
        other => format!("{other}m"),
    }
}

fn parse_utilization(text: &str) -> Option<f64> {
    let text = text.trim();
    if let Some(percent) = text.strip_suffix('%') {
        return percent.trim().parse::<f64>().ok();
    }
    let value = text.parse::<f64>().ok()?;
    Some(if value <= 1.0 { value * 100.0 } else { value })
}

fn parse_reset(text: &str, now_ms: i64) -> Option<i64> {
    let text = text.trim();
    if let Ok(seconds) = text.parse::<i64>() {
        return Some(if seconds > 1_000_000_000 {
            seconds * 1000
        } else {
            now_ms + seconds * 1000
        });
    }
    time::OffsetDateTime::parse(text, &time::format_description::well_known::Rfc3339)
        .ok()
        .map(|when| (when.unix_timestamp_nanos() / 1_000_000) as i64)
}

#[cfg(test)]
mod tests {
    use super::*;
    use axum::http::{HeaderName, HeaderValue};

    const NOW: i64 = 1_700_000_000_000;

    fn headers(pairs: &[(&str, &str)]) -> HeaderMap {
        let mut map = HeaderMap::new();
        for (name, value) in pairs {
            map.insert(
                HeaderName::from_bytes(name.as_bytes()).unwrap(),
                HeaderValue::from_str(value).unwrap(),
            );
        }
        map
    }

    #[test]
    fn a_rejection_wearing_a_429_does_not_cost_the_account_its_place() {
        let bare = headers(&[]);
        assert_eq!(
            classify(429, &bare, &Snapshot::default()),
            Verdict::Transient,
            "a limit the provider did not report is not a limit we can schedule against"
        );

        let reported = headers(&[("anthropic-ratelimit-unified-5h-utilization", "1.0")]);
        let snapshot = parse(&reported, NOW);
        assert!(matches!(
            classify(429, &reported, &snapshot),
            Verdict::Exhausted { .. }
        ));

        let before = crate::store::now();
        let verdict = classify(
            429,
            &headers(&[("retry-after", "30")]),
            &Snapshot::default(),
        );
        let Verdict::Exhausted { until } = verdict else {
            panic!("an explicit wait is a reported limit: {verdict:?}");
        };
        assert!((before + 30_000..=crate::store::now() + 30_000).contains(&until));
    }

    #[test]
    fn a_model_scoped_window_carries_its_scope() {
        let snapshot = parse(
            &headers(&[
                ("anthropic-ratelimit-unified-5h-utilization", "0.20"),
                ("anthropic-ratelimit-unified-7d_fable-utilization", "1.0"),
            ]),
            NOW,
        );

        let fable = snapshot
            .windows
            .iter()
            .find(|window| window.scope.as_deref() == Some("fable"))
            .expect("the scoped window");
        assert_eq!(fable.label, "weekly");
        assert_eq!(fable.used_percent, 100.0);

        assert_eq!(snapshot.pressure_for(Some("fable")), Some(100.0));
        assert_eq!(snapshot.pressure_for(Some("sonnet")), Some(20.0));
    }

    #[test]
    fn codex_buckets_are_discovered_by_suffix_not_by_a_fixed_list() {
        let snapshot = parse(
            &headers(&[
                ("x-codex-primary-used-percent", "10"),
                ("x-codex-primary-window-minutes", "300"),
                ("x-codex-secondary-used-percent", "40"),
                ("x-codex-secondary-window-minutes", "10080"),
                ("x-codex-bengalfox-primary-used-percent", "95"),
                ("x-codex-bengalfox-primary-window-minutes", "300"),
            ]),
            NOW,
        );

        let scopes: Vec<_> = snapshot
            .windows
            .iter()
            .map(|window| {
                (
                    window.scope.clone(),
                    window.label.clone(),
                    window.used_percent,
                )
            })
            .collect();

        assert!(
            scopes.contains(&(None, "5h".to_owned(), 10.0)),
            "the default bucket keeps both halves: {scopes:?}"
        );
        assert!(
            scopes.contains(&(None, "weekly".to_owned(), 40.0)),
            "x-codex-secondary-* is the default bucket's second window, not a bucket: {scopes:?}"
        );
        assert!(
            scopes.contains(&(Some("codex_bengalfox".to_owned()), "5h".to_owned(), 95.0)),
            "only -primary-used-percent declares a bucket: {scopes:?}"
        );

        assert_eq!(snapshot.pressure_for(Some("codex_bengalfox")), Some(95.0));
        assert_eq!(snapshot.pressure_for(None), Some(40.0));
    }

    #[test]
    fn anthropic_windows_are_discovered_not_hardcoded() {
        let snapshot = parse(
            &headers(&[
                ("anthropic-ratelimit-unified-5h-utilization", "0.62"),
                ("anthropic-ratelimit-unified-5h-reset", "1700000600"),
                ("anthropic-ratelimit-unified-7d-utilization", "31%"),
                (
                    "anthropic-ratelimit-unified-representative-claim",
                    "five_hour",
                ),
            ]),
            NOW,
        );

        assert_eq!(snapshot.windows.len(), 2);
        let five_hour = snapshot.windows.iter().find(|w| w.label == "5h").unwrap();
        assert_eq!(five_hour.used_percent, 62.0);
        assert_eq!(five_hour.resets_at_ms, Some(1_700_000_600_000));
        assert_eq!(
            snapshot
                .windows
                .iter()
                .find(|w| w.label == "weekly")
                .unwrap()
                .used_percent,
            31.0
        );
        assert_eq!(snapshot.binding.as_deref(), Some("5h"));
    }

    #[test]
    fn a_window_we_have_never_seen_still_shows_up() {
        let snapshot = parse(
            &headers(&[("anthropic-ratelimit-unified-30d-utilization", "0.05")]),
            NOW,
        );
        assert_eq!(snapshot.windows[0].label, "30d");
    }

    #[test]
    fn codex_headers_are_read_in_both_shapes() {
        let absolute = parse(
            &headers(&[
                ("x-codex-primary-used-percent", "18"),
                ("x-codex-primary-reset-at", "1700000600"),
            ]),
            NOW,
        );
        assert_eq!(absolute.windows[0].resets_at_ms, Some(1_700_000_600_000));

        let relative = parse(
            &headers(&[
                ("x-codex-primary-used-percent", "18"),
                ("x-codex-primary-reset-after-seconds", "600"),
            ]),
            NOW,
        );
        assert_eq!(relative.windows[0].resets_at_ms, Some(NOW + 600_000));
    }

    #[test]
    fn pressure_is_the_worst_window_not_the_average() {
        let snapshot = parse(
            &headers(&[
                ("anthropic-ratelimit-unified-5h-utilization", "0.95"),
                ("anthropic-ratelimit-unified-7d-utilization", "0.05"),
            ]),
            NOW,
        );
        assert_eq!(snapshot.pressure_for(None), Some(95.0));
    }

    #[test]
    fn no_headers_means_unknown_not_zero() {
        let snapshot = parse(&headers(&[]), NOW);
        assert!(snapshot.is_empty());
        assert_eq!(snapshot.pressure_for(None), None);
    }

    #[test]
    fn retry_after_wins_over_the_snapshot() {
        let at = retry_after_ms(
            &headers(&[
                ("retry-after", "30"),
                ("anthropic-ratelimit-unified-5h-reset", "1700009999"),
                ("anthropic-ratelimit-unified-5h-utilization", "1.0"),
            ]),
            NOW,
        );
        assert_eq!(at, Some(NOW + 30_000));
    }

    #[test]
    fn a_request_and_token_budget_reads_as_one_window_per_dimension() {
        let snapshot = parse(
            &headers(&[
                ("x-ratelimit-limit-requests", "500"),
                ("x-ratelimit-remaining-requests", "499"),
                ("x-ratelimit-reset-requests", "120ms"),
                ("x-ratelimit-limit-tokens", "30000"),
                ("x-ratelimit-remaining-tokens", "6000"),
                ("x-ratelimit-reset-tokens", "6m0s"),
            ]),
            NOW,
        );

        let tokens = snapshot
            .windows
            .iter()
            .find(|window| window.label == "tokens")
            .expect("the token budget");
        assert_eq!(tokens.used_percent, 80.0);
        assert_eq!(tokens.resets_at_ms, Some(NOW + 360_000));

        let requests = snapshot
            .windows
            .iter()
            .find(|window| window.label == "requests")
            .expect("the request budget");
        assert_eq!(requests.resets_at_ms, Some(NOW + 120));
        assert_eq!(snapshot.pressure_for(None), Some(80.0));
    }

    #[test]
    fn a_dimension_nobody_listed_is_still_read() {
        let snapshot = parse(
            &headers(&[
                ("x-ratelimit-limit-req-minute", "60"),
                ("x-ratelimit-remaining-req-minute", "15"),
            ]),
            NOW,
        );
        assert_eq!(snapshot.windows[0].label, "req-minute");
        assert_eq!(snapshot.windows[0].used_percent, 75.0);
        assert_eq!(snapshot.windows[0].resets_at_ms, None);
    }

    #[test]
    fn an_api_key_tier_on_anthropic_is_read_beside_the_subscription_windows() {
        let snapshot = parse(
            &headers(&[
                ("anthropic-ratelimit-input-tokens-limit", "400000"),
                ("anthropic-ratelimit-input-tokens-remaining", "100000"),
                (
                    "anthropic-ratelimit-input-tokens-reset",
                    "2026-08-10T12:00:00Z",
                ),
                ("anthropic-ratelimit-unified-5h-utilization", "0.10"),
            ]),
            NOW,
        );
        let input = snapshot
            .windows
            .iter()
            .find(|window| window.label == "input-tokens")
            .expect("the input token budget");
        assert_eq!(input.used_percent, 75.0);
        assert_eq!(input.resets_at_ms, Some(1_786_363_200_000));
        assert!(
            snapshot.windows.iter().any(|window| window.label == "5h"),
            "the unified headers must not be mistaken for a tier budget"
        );
        assert!(
            !snapshot
                .windows
                .iter()
                .any(|window| window.label.contains("unified"))
        );
    }

    #[test]
    fn a_budget_with_no_limit_is_not_a_full_one() {
        let snapshot = parse(
            &headers(&[
                ("x-ratelimit-limit-tokens", "0"),
                ("x-ratelimit-remaining-tokens", "0"),
            ]),
            NOW,
        );
        assert!(
            snapshot.is_empty(),
            "a limit of zero says nothing about how much is used"
        );
    }

    #[test]
    fn retry_after_is_understood_as_a_date_and_in_milliseconds() {
        assert_eq!(
            retry_after_ms(&headers(&[("retry-after-ms", "1500")]), NOW),
            Some(NOW + 1_500)
        );
        assert_eq!(
            retry_after_ms(
                &headers(&[("retry-after", "Mon, 10 Aug 2026 12:00:00 GMT")]),
                NOW
            ),
            Some(1_786_363_200_000)
        );
    }

    #[test]
    fn a_429_on_a_spent_minute_parks_the_account_until_that_minute_turns() {
        let spent = headers(&[
            ("x-ratelimit-limit-tokens", "30000"),
            ("x-ratelimit-remaining-tokens", "0"),
            ("x-ratelimit-reset-tokens", "6s"),
        ]);
        let before = crate::store::now();
        let verdict = classify(429, &spent, &parse(&spent, before));
        let Verdict::Exhausted { until } = verdict else {
            panic!("a reported budget at zero is spent, not a transient throttle: {verdict:?}");
        };
        assert!((before + 5_000..=crate::store::now() + 7_000).contains(&until));
    }

    #[test]
    fn rfc3339_resets_are_understood() {
        let snapshot = parse(
            &headers(&[
                ("anthropic-ratelimit-unified-5h-utilization", "0.5"),
                (
                    "anthropic-ratelimit-unified-5h-reset",
                    "2026-08-10T12:00:00Z",
                ),
            ]),
            NOW,
        );
        assert_eq!(snapshot.windows[0].resets_at_ms, Some(1_786_363_200_000));
    }
}
