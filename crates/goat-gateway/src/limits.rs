use axum::http::HeaderMap;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Window {
    pub label: String,
    pub used_percent: f64,
    pub resets_at_ms: Option<i64>,
}

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct Snapshot {
    pub windows: Vec<Window>,
    pub binding: Option<String>,
}

impl Snapshot {
    pub fn is_empty(&self) -> bool {
        self.windows.is_empty()
    }

    pub fn pressure(&self) -> Option<f64> {
        self.windows
            .iter()
            .map(|window| window.used_percent)
            .fold(None, |worst: Option<f64>, used| {
                Some(worst.map_or(used, |value| value.max(used)))
            })
    }

    pub fn soonest_reset(&self) -> Option<i64> {
        self.windows.iter().filter_map(|w| w.resets_at_ms).min()
    }
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

        windows.push(Window {
            label: match period {
                "5h" => "5h".to_owned(),
                "7d" => "weekly".to_owned(),
                other => other.to_owned(),
            },
            used_percent: used,
            resets_at_ms,
        });
    }

    for (prefix, label) in [("primary", "5h"), ("secondary", "weekly")] {
        let used = headers
            .get(format!("x-codex-{prefix}-used-percent").as_str())
            .and_then(|value| value.to_str().ok())
            .and_then(parse_utilization);
        let Some(used) = used else { continue };

        let resets_at_ms = headers
            .get(format!("x-codex-{prefix}-reset-at").as_str())
            .and_then(|value| value.to_str().ok())
            .and_then(|text| text.parse::<i64>().ok())
            .map(|seconds| seconds * 1000)
            .or_else(|| {
                headers
                    .get(format!("x-codex-{prefix}-reset-after-seconds").as_str())
                    .and_then(|value| value.to_str().ok())
                    .and_then(|text| text.parse::<i64>().ok())
                    .map(|seconds| now_ms + seconds * 1000)
            });

        windows.push(Window {
            label: label.to_owned(),
            used_percent: used,
            resets_at_ms,
        });
    }

    let binding = headers
        .get("anthropic-ratelimit-unified-representative-claim")
        .and_then(|value| value.to_str().ok())
        .map(|claim| match claim {
            "five_hour" => "5h".to_owned(),
            "seven_day" => "weekly".to_owned(),
            other => other.to_owned(),
        });

    windows.sort_by(|a, b| a.label.cmp(&b.label));
    Snapshot { windows, binding }
}

pub fn retry_after_ms(headers: &HeaderMap, now_ms: i64) -> Option<i64> {
    if let Some(seconds) = headers
        .get("retry-after")
        .and_then(|value| value.to_str().ok())
        .and_then(|text| text.trim().parse::<i64>().ok())
    {
        return Some(now_ms + seconds * 1000);
    }
    parse(headers, now_ms).soonest_reset()
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
        assert_eq!(snapshot.pressure(), Some(95.0));
    }

    #[test]
    fn no_headers_means_unknown_not_zero() {
        let snapshot = parse(&headers(&[]), NOW);
        assert!(snapshot.is_empty());
        assert_eq!(snapshot.pressure(), None);
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
