use std::time::Duration;

use serde::Deserialize;

use crate::oauth::{
    BodyFormat, Flow, Mode, account_from_id_token, exchange_fields, refresh_fields,
};

#[derive(Debug, thiserror::Error)]
pub enum OauthError {
    #[error("could not reach {url}: {source}")]
    Unreachable {
        url: String,
        #[source]
        source: reqwest::Error,
    },
    #[error("{provider} refused the request ({status}): {body}")]
    Refused {
        provider: String,
        status: u16,
        body: String,
    },
    #[error("{provider} answered with something this flow does not understand: {body}")]
    Unreadable { provider: String, body: String },
    #[error("{provider} does not offer {mode:?} sign-in")]
    Unsupported { provider: String, mode: Mode },
    #[error("nobody finished the sign-in within {minutes} minutes")]
    NobodyCame { minutes: u64 },
}

#[derive(Debug, Clone)]
pub struct Tokens {
    pub access: String,
    pub refresh: Option<String>,
    pub id_token: Option<String>,
    pub expires_at: Option<i64>,
    pub account_id: Option<String>,
}

#[derive(Debug, Clone)]
pub struct DeviceStart {
    pub user_code: String,
    pub verification_url: String,
    device_auth_id: String,
    interval: u64,
}

#[derive(Deserialize)]
struct TokenResponse {
    access_token: String,
    #[serde(default)]
    refresh_token: Option<String>,
    #[serde(default)]
    id_token: Option<String>,
    #[serde(default)]
    expires_in: Option<i64>,
}

#[derive(Deserialize)]
struct UserCodeResponse {
    device_auth_id: String,
    #[serde(alias = "usercode")]
    user_code: String,
    #[serde(default, deserialize_with = "lenient_interval")]
    interval: u64,
}

#[derive(Deserialize)]
struct DeviceCodeResponse {
    authorization_code: String,
    code_verifier: String,
}

fn lenient_interval<'de, D>(deserializer: D) -> Result<u64, D::Error>
where
    D: serde::Deserializer<'de>,
{
    let value = serde_json::Value::deserialize(deserializer)?;
    Ok(match value {
        serde_json::Value::Number(number) => number.as_u64().unwrap_or(5),
        serde_json::Value::String(text) => text.trim().parse().unwrap_or(5),
        _ => 5,
    })
}

#[derive(Clone)]
pub struct Client {
    http: reqwest::Client,
    origin: Option<String>,
}

impl Default for Client {
    fn default() -> Self {
        Self::new()
    }
}

impl Client {
    pub fn new() -> Self {
        Self::with_http(reqwest::Client::new())
    }

    pub fn with_http(http: reqwest::Client) -> Self {
        Self { http, origin: None }
    }

    pub fn against(origin: impl Into<String>) -> Self {
        Self {
            http: reqwest::Client::new(),
            origin: Some(origin.into().trim_end_matches('/').to_owned()),
        }
    }

    fn at(&self, url: &str) -> String {
        let Some(origin) = self.origin.as_deref() else {
            return url.to_owned();
        };
        let path = url
            .split_once("://")
            .and_then(|(_, rest)| rest.split_once('/'))
            .map_or(String::new(), |(_, path)| format!("/{path}"));
        format!("{origin}{path}")
    }

    pub async fn exchange(
        &self,
        flow: &Flow,
        code: &str,
        state: &str,
        verifier: &str,
        redirect_uri: &str,
    ) -> Result<Tokens, OauthError> {
        let fields = exchange_fields(flow, code, state, verifier, redirect_uri);
        self.token_call(flow, &self.at(flow.token_url), fields).await
    }

    pub async fn refresh(&self, flow: &Flow, refresh_token: &str) -> Result<Tokens, OauthError> {
        let fields = refresh_fields(flow, refresh_token);
        let mut tokens = self.token_call(flow, &self.at(flow.token_url), fields).await?;
        if tokens.refresh.is_none() {
            tokens.refresh = Some(refresh_token.to_owned());
        }
        Ok(tokens)
    }

    pub async fn start_device(&self, flow: &Flow) -> Result<DeviceStart, OauthError> {
        let device = flow.device.ok_or_else(|| OauthError::Unsupported {
            provider: flow.provider.to_owned(),
            mode: Mode::Device,
        })?;

        let body = self
            .read_json(
                flow,
                self.http
                    .post(self.at(device.user_code_url))
                    .json(&serde_json::json!({ "client_id": flow.client_id })),
                device.user_code_url,
            )
            .await?;

        let parsed: UserCodeResponse =
            serde_json::from_str(&body).map_err(|_| OauthError::Unreadable {
                provider: flow.provider.to_owned(),
                body: body.clone(),
            })?;

        Ok(DeviceStart {
            user_code: parsed.user_code,
            verification_url: device.verification_url.to_owned(),
            device_auth_id: parsed.device_auth_id,
            interval: parsed.interval.max(1),
        })
    }

    pub async fn finish_device(
        &self,
        flow: &Flow,
        start: &DeviceStart,
    ) -> Result<Tokens, OauthError> {
        let device = flow.device.ok_or_else(|| OauthError::Unsupported {
            provider: flow.provider.to_owned(),
            mode: Mode::Device,
        })?;

        const WINDOW_MINUTES: u64 = 15;
        let deadline =
            tokio::time::Instant::now() + Duration::from_secs(WINDOW_MINUTES.saturating_mul(60));

        let issued = loop {
            let response = self
                .http
                .post(self.at(device.poll_url))
                .json(&serde_json::json!({
                    "device_auth_id": start.device_auth_id,
                    "user_code": start.user_code,
                }))
                .send()
                .await
                .map_err(|source| OauthError::Unreachable {
                    url: device.poll_url.to_owned(),
                    source,
                })?;

            let status = response.status();
            let body = response.text().await.unwrap_or_default();

            if status.is_success() {
                break serde_json::from_str::<DeviceCodeResponse>(&body).map_err(|_| {
                    OauthError::Unreadable {
                        provider: flow.provider.to_owned(),
                        body,
                    }
                })?;
            }

            let still_waiting = status == reqwest::StatusCode::FORBIDDEN
                || status == reqwest::StatusCode::NOT_FOUND;
            if !still_waiting {
                return Err(OauthError::Refused {
                    provider: flow.provider.to_owned(),
                    status: status.as_u16(),
                    body,
                });
            }

            let now = tokio::time::Instant::now();
            if now >= deadline {
                return Err(OauthError::NobodyCame {
                    minutes: WINDOW_MINUTES,
                });
            }
            tokio::time::sleep(Duration::from_secs(start.interval).min(deadline - now)).await;
        };

        self.exchange(
            flow,
            &issued.authorization_code,
            "",
            &issued.code_verifier,
            device.redirect_uri,
        )
        .await
    }

    async fn token_call(
        &self,
        flow: &Flow,
        url: &str,
        fields: Vec<(&'static str, String)>,
    ) -> Result<Tokens, OauthError> {
        let request = match flow.body {
            BodyFormat::Json => {
                let object = fields
                    .into_iter()
                    .map(|(name, value)| (name.to_owned(), serde_json::Value::String(value)))
                    .collect::<serde_json::Map<_, _>>();
                self.http.post(url).json(&object)
            }
            BodyFormat::Form => self.http.post(url).form(&fields),
        };

        let body = self.read_json(flow, request, url).await?;
        let parsed: TokenResponse =
            serde_json::from_str(&body).map_err(|_| OauthError::Unreadable {
                provider: flow.provider.to_owned(),
                body: body.clone(),
            })?;

        let account_id = match (flow.identity, parsed.id_token.as_deref()) {
            (Some(identity), Some(id_token)) => account_from_id_token(id_token, &identity),
            _ => None,
        };

        Ok(Tokens {
            access: parsed.access_token,
            refresh: parsed.refresh_token,
            id_token: parsed.id_token,
            expires_at: parsed
                .expires_in
                .map(|seconds| crate::store::now() + seconds.saturating_mul(1_000)),
            account_id,
        })
    }

    async fn read_json(
        &self,
        flow: &Flow,
        request: reqwest::RequestBuilder,
        url: &str,
    ) -> Result<String, OauthError> {
        let response = request
            .send()
            .await
            .map_err(|source| OauthError::Unreachable {
                url: url.to_owned(),
                source,
            })?;
        let status = response.status();
        let body = response.text().await.unwrap_or_default();

        if !status.is_success() {
            return Err(OauthError::Refused {
                provider: flow.provider.to_owned(),
                status: status.as_u16(),
                body,
            });
        }
        Ok(body)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::oauth::{ANTHROPIC, OPENAI};

    #[test]
    fn a_refusal_keeps_the_provider_message_instead_of_flattening_it() {
        let error = OauthError::Refused {
            provider: "anthropic".to_owned(),
            status: 400,
            body: r#"{"error":"invalid_grant"}"#.to_owned(),
        };
        let rendered = error.to_string();
        assert!(
            rendered.contains("invalid_grant"),
            "the reason the sign-in failed only exists in their body: {rendered}"
        );
    }

    #[test]
    fn an_interval_survives_being_sent_as_a_string() {
        #[derive(Deserialize)]
        struct Probe {
            #[serde(default, deserialize_with = "lenient_interval")]
            interval: u64,
        }
        let as_string: Probe = serde_json::from_str(r#"{"interval":"7"}"#).unwrap();
        let as_number: Probe = serde_json::from_str(r#"{"interval":3}"#).unwrap();
        let missing: Probe = serde_json::from_str("{}").unwrap();
        assert_eq!(as_string.interval, 7);
        assert_eq!(as_number.interval, 3);
        assert_eq!(missing.interval, 0);
    }

    #[tokio::test]
    async fn asking_for_a_mode_a_provider_lacks_names_the_provider() {
        let client = Client::new();
        let error = client.start_device(&ANTHROPIC).await.unwrap_err();
        assert!(matches!(
            error,
            OauthError::Unsupported {
                mode: Mode::Device,
                ..
            }
        ));
        assert!(error.to_string().contains("anthropic"));
    }

    #[test]
    fn the_two_providers_disagree_on_body_format_and_that_is_carried_as_data() {
        assert_eq!(ANTHROPIC.body, BodyFormat::Json);
        assert_eq!(OPENAI.body, BodyFormat::Form);
    }
}
