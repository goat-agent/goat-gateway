use axum::http::HeaderMap;
use serde::Serialize;
use serde_json::{Value, json};

use crate::{App, provider::Wire};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Style {
    Bearer,
    XApiKey,
}

#[derive(Debug, Clone, Serialize)]
pub struct Finding {
    pub wire: &'static str,
    pub url: String,
    pub status: Option<u16>,
    pub serves: bool,
    pub said: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
pub struct Discovered {
    pub suggested_id: String,
    pub root: String,
    pub key: Option<Style>,
    pub models: Vec<String>,
    pub findings: Vec<Finding>,
    pub reports_limits: bool,
    pub declaration: String,
}

pub async fn discover(
    app: &App,
    base_url: &str,
    secret: &str,
    model: Option<&str>,
) -> Result<Discovered, String> {
    let base = base_url.trim().trim_end_matches('/');
    if !base.starts_with("http://") && !base.starts_with("https://") {
        return Err("the address has to start with http:// or https://".to_owned());
    }
    let roots: Vec<String> = if base.ends_with("/v1") {
        vec![base.to_owned()]
    } else {
        vec![format!("{base}/v1"), base.to_owned()]
    };

    let client = &app.inner.client;
    let mut root = roots[0].clone();
    let mut key = None;
    let mut models = Vec::new();
    'listing: for candidate in &roots {
        for style in [Style::Bearer, Style::XApiKey] {
            let Ok(answer) = client
                .get(format!("{candidate}/models"))
                .headers(authorised(style, secret))
                .send()
                .await
            else {
                continue;
            };
            if answer.status().is_success() {
                root = candidate.clone();
                key = Some(style);
                models = listed(&answer.json::<Value>().await.unwrap_or_default());
                break 'listing;
            }
        }
    }

    let tried = model
        .map(str::to_owned)
        .or_else(|| models.first().cloned())
        .unwrap_or_default();
    let mut findings = Vec::new();
    let mut reports_limits = false;
    for wire in [Wire::Messages, Wire::Chat, Wire::Responses] {
        let (path, body) = knock(wire, &tried);
        let url = format!("{root}{path}");
        let styles = match key {
            Some(style) => vec![style],
            None => vec![Style::Bearer, Style::XApiKey],
        };
        let mut finding = Finding {
            wire: wire.id(),
            url: url.clone(),
            status: None,
            serves: false,
            said: None,
        };
        for style in styles {
            let mut headers = authorised(style, secret);
            headers.insert(
                "anthropic-version",
                axum::http::HeaderValue::from_static("2023-06-01"),
            );
            let Ok(answer) = client.post(&url).headers(headers).json(&body).send().await else {
                finding.said = Some("could not connect".to_owned());
                continue;
            };
            let status = answer.status().as_u16();
            reports_limits |= limited(answer.headers());
            let text = answer.text().await.unwrap_or_default();
            finding.status = Some(status);
            finding.serves = matches!(status, 200..=299 | 400 | 422 | 429);
            finding.said =
                (!(200..300).contains(&status)).then(|| text.chars().take(300).collect());
            if !matches!(status, 401 | 403) {
                if key.is_none() {
                    key = Some(style);
                }
                break;
            }
        }
        findings.push(finding);
    }

    let suggested_id = suggested(base);
    let declaration = declared(&suggested_id, &findings, key, reports_limits, &models);
    Ok(Discovered {
        suggested_id,
        root,
        key,
        models,
        findings,
        reports_limits,
        declaration,
    })
}

fn authorised(style: Style, secret: &str) -> HeaderMap {
    let mut headers = HeaderMap::new();
    let (name, value) = match style {
        Style::Bearer => ("authorization", format!("Bearer {secret}")),
        Style::XApiKey => ("x-api-key", secret.to_owned()),
    };
    if let (Ok(name), Ok(value)) = (
        axum::http::HeaderName::from_bytes(name.as_bytes()),
        axum::http::HeaderValue::from_str(&value),
    ) {
        headers.insert(name, value);
    }
    headers
}

fn listed(body: &Value) -> Vec<String> {
    ["data", "models"]
        .iter()
        .filter_map(|field| body.get(*field).and_then(Value::as_array))
        .flatten()
        .filter_map(|model| {
            ["id", "slug", "name"]
                .iter()
                .find_map(|field| model.get(*field).and_then(Value::as_str))
        })
        .map(|name| name.trim_start_matches("models/").to_owned())
        .collect()
}

fn knock(wire: Wire, model: &str) -> (&'static str, Value) {
    match wire {
        Wire::Messages => (
            "/messages",
            json!({ "model": model, "max_tokens": 1, "messages": [{ "role": "user", "content": "." }] }),
        ),
        Wire::Chat => (
            "/chat/completions",
            json!({ "model": model, "max_tokens": 1, "messages": [{ "role": "user", "content": "." }] }),
        ),
        Wire::Responses => (
            "/responses",
            json!({ "model": model, "input": ".", "max_output_tokens": 16 }),
        ),
    }
}

fn limited(headers: &HeaderMap) -> bool {
    headers.keys().any(|name| {
        let name = name.as_str();
        name.starts_with("x-ratelimit-") || name.starts_with("anthropic-ratelimit-")
    })
}

fn suggested(base: &str) -> String {
    let host = base
        .split_once("://")
        .map_or(base, |(_, rest)| rest)
        .split(['/', ':'])
        .next()
        .unwrap_or("custom");
    let mut id: String = host
        .trim_start_matches("api.")
        .split('.')
        .next()
        .unwrap_or("custom")
        .chars()
        .map(|c| {
            if c.is_ascii_alphanumeric() {
                c.to_ascii_lowercase()
            } else {
                '-'
            }
        })
        .collect();
    if !id.starts_with(|c: char| c.is_ascii_lowercase()) {
        id.insert_str(0, "custom-");
    }
    id
}

fn declared(
    id: &str,
    findings: &[Finding],
    key: Option<Style>,
    reports_limits: bool,
    models: &[String],
) -> String {
    let quoted = |text: &str| Value::String(text.to_owned()).to_string();
    let mut lines = vec![format!("label = {}", quoted(id))];
    if key == Some(Style::XApiKey) {
        lines.push("key = \"x_api_key\"".to_owned());
    }
    lines.push(format!(
        "limits = \"{}\"",
        if reports_limits { "headers" } else { "none" }
    ));
    let served: Vec<String> = findings
        .iter()
        .filter(|finding| finding.serves)
        .map(|finding| {
            format!(
                "  {{ wire = \"{}\", url = {} }},",
                finding.wire,
                quoted(&finding.url)
            )
        })
        .collect();
    lines.push(format!("endpoints = [\n{}\n]", served.join("\n")));
    if findings
        .iter()
        .any(|finding| finding.serves && finding.wire == "messages")
    {
        lines.push("headers = { \"anthropic-version\" = \"2023-06-01\" }".to_owned());
    }
    for model in models {
        lines.push(format!("\n[[models]]\nname = {}", quoted(model)));
    }
    lines.join("\n") + "\n"
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn both_listing_shapes_name_their_models() {
        let openai = json!({ "data": [{ "id": "gpt-5" }, { "id": "o4" }] });
        let codex = json!({ "models": [{ "slug": "gpt-6.1-sol" }] });
        let gemini = json!({ "models": [{ "name": "models/gemini-3-pro" }] });
        assert_eq!(listed(&openai), ["gpt-5", "o4"]);
        assert_eq!(listed(&codex), ["gpt-6.1-sol"]);
        assert_eq!(listed(&gemini), ["gemini-3-pro"]);
    }

    #[test]
    fn a_suggested_name_is_one_config_toml_accepts() {
        for base in [
            "https://api.deepseek.com",
            "http://10.0.0.9:8000/v1",
            "https://llm.office.example",
        ] {
            let id = suggested(base);
            assert!(crate::config::named(&id).is_ok(), "{base} gave {id}");
        }
        assert_eq!(suggested("https://api.deepseek.com"), "deepseek");
    }

    #[test]
    fn what_was_found_becomes_a_declaration_the_catalog_accepts() {
        let findings = vec![
            Finding {
                wire: "messages",
                url: "https://llm.example/v1/messages".into(),
                status: Some(400),
                serves: true,
                said: Some("model required".into()),
            },
            Finding {
                wire: "chat",
                url: "https://llm.example/v1/chat/completions".into(),
                status: Some(200),
                serves: true,
                said: None,
            },
            Finding {
                wire: "responses",
                url: "https://llm.example/v1/responses".into(),
                status: Some(404),
                serves: false,
                said: None,
            },
        ];
        let text = declared(
            "llm",
            &findings,
            Some(Style::XApiKey),
            true,
            &["big".into(), "small".into()],
        );
        let written = crate::config::declared("", "llm", &text).unwrap();
        let catalog = crate::provider::Catalog::with_overlay(&written).unwrap();
        let llm = catalog.get("llm").unwrap();
        assert!(llm.speaks(Wire::Messages) && llm.speaks(Wire::Chat));
        assert!(
            !llm.speaks(Wire::Responses),
            "a 404 is not a format it serves"
        );
        assert_eq!(llm.key, crate::provider::Key::XApiKey);
        assert_eq!(llm.model_names(), ["big", "small"]);
    }
}
