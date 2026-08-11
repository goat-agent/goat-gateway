use axum::http::{HeaderMap, HeaderName, HeaderValue};
use goat_gateway_wire::HeaderEdit;

const NEVER_FORWARD: &[&str] = &[
    "connection",
    "keep-alive",
    "proxy-authenticate",
    "proxy-authorization",
    "te",
    "trailer",
    "transfer-encoding",
    "upgrade",
    "host",
    "content-length",
    "authorization",
    "x-api-key",
];

#[derive(Debug, Clone)]
pub struct Upstream {
    pub base_url: String,
    pub auth: Auth,
}

#[derive(Clone, Default)]
pub struct Oauth {
    pub token: String,
    pub extra: Vec<(String, String)>,
    pub merge_beta: Option<String>,
}

#[derive(Clone)]
pub enum Auth {
    ApiKey(String),
    Bearer(String),
    Oauth(Oauth),
}

impl std::fmt::Debug for Auth {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::ApiKey(_) => f.write_str("ApiKey(***)"),
            Self::Bearer(_) => f.write_str("Bearer(***)"),
            Self::Oauth(_) => f.write_str("Oauth(***)"),
        }
    }
}

impl Auth {
    pub fn presented(style: crate::provider::Key, secret: String) -> Self {
        match style {
            crate::provider::Key::Bearer => Self::Bearer(secret),
            crate::provider::Key::XApiKey => Self::ApiKey(secret),
        }
    }

    fn header(&self) -> (HeaderName, HeaderValue, HeaderEdit) {
        match self {
            Self::ApiKey(key) => (
                HeaderName::from_static("x-api-key"),
                HeaderValue::from_str(key).unwrap_or_else(|_| HeaderValue::from_static("")),
                HeaderEdit::new("x-api-key"),
            ),
            Self::Bearer(token) | Self::Oauth(Oauth { token, .. }) => (
                HeaderName::from_static("authorization"),
                HeaderValue::from_str(&format!("Bearer {token}"))
                    .unwrap_or_else(|_| HeaderValue::from_static("")),
                HeaderEdit::new("authorization"),
            ),
        }
    }
}

pub fn merge_beta(existing: Option<&str>, required: &str) -> String {
    let mut flags: Vec<&str> = existing
        .unwrap_or_default()
        .split(',')
        .map(str::trim)
        .filter(|flag| !flag.is_empty())
        .collect();
    if !flags.contains(&required) {
        flags.insert(0, required);
    }
    flags.join(",")
}

pub fn forward_headers(incoming: &HeaderMap, auth: &Auth) -> (HeaderMap, Vec<HeaderEdit>) {
    let mut out = HeaderMap::with_capacity(incoming.len() + 1);
    for (name, value) in incoming {
        if NEVER_FORWARD.contains(&name.as_str()) {
            continue;
        }
        out.append(name.clone(), value.clone());
    }

    let (name, value, edit) = auth.header();
    out.insert(name, value);
    let mut edits = vec![edit];

    if let Auth::Oauth(oauth) = auth {
        if let Some(required) = oauth.merge_beta.as_deref() {
            const BETA: &str = "anthropic-beta";
            let existing = out.get(BETA).and_then(|value| value.to_str().ok());
            let merged = merge_beta(existing, required);
            if existing != Some(merged.as_str())
                && let Ok(value) = HeaderValue::from_str(&merged)
            {
                out.insert(HeaderName::from_static(BETA), value);
                edits.push(HeaderEdit::new(BETA));
            }
        }
        for (name, value) in &oauth.extra {
            if let (Ok(header), Ok(value)) = (
                HeaderName::from_bytes(name.as_bytes()),
                HeaderValue::from_str(value),
            ) {
                out.insert(header, value);
                edits.push(HeaderEdit::new(name));
            }
        }
    }

    (out, edits)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn headers(pairs: &[(&str, &str)]) -> HeaderMap {
        let mut map = HeaderMap::new();
        for (name, value) in pairs {
            map.append(
                HeaderName::from_bytes(name.as_bytes()).unwrap(),
                HeaderValue::from_str(value).unwrap(),
            );
        }
        map
    }

    fn oauth(token: &str) -> Auth {
        Auth::Oauth(Oauth {
            token: token.into(),
            extra: Vec::new(),
            merge_beta: Some("oauth-2025-04-20".into()),
        })
    }

    #[test]
    fn the_oauth_beta_joins_the_flags_the_client_already_sent() {
        let incoming = headers(&[(
            "anthropic-beta",
            "claude-code-20250219,interleaved-thinking-2025-05-14",
        )]);
        let (out, _) = forward_headers(&incoming, &oauth("at"));
        assert_eq!(
            out["anthropic-beta"],
            "oauth-2025-04-20,claude-code-20250219,interleaved-thinking-2025-05-14",
            "dropping the client's own beta flags would silently turn off features it asked for"
        );
    }

    #[test]
    fn the_oauth_beta_is_not_repeated_when_the_client_already_sent_it() {
        let incoming = headers(&[("anthropic-beta", "oauth-2025-04-20,other")]);
        let (out, edits) = forward_headers(&incoming, &oauth("at"));
        assert_eq!(out["anthropic-beta"], "oauth-2025-04-20,other");
        assert!(
            !edits.iter().any(|edit| edit.name == "anthropic-beta"),
            "an edit that changes nothing must not be announced as an edit"
        );
    }

    #[test]
    fn the_oauth_beta_appears_even_when_the_client_sent_none() {
        let (out, edits) = forward_headers(&headers(&[]), &oauth("at"));
        assert_eq!(out["anthropic-beta"], "oauth-2025-04-20");
        assert!(edits.iter().any(|edit| edit.name == "anthropic-beta"));
    }

    #[test]
    fn an_oauth_call_carries_a_bearer_and_never_an_api_key() {
        let incoming = headers(&[("x-api-key", "client-key")]);
        let (out, _) = forward_headers(&incoming, &oauth("at_live"));
        assert_eq!(out["authorization"], "Bearer at_live");
        assert!(
            !out.contains_key("x-api-key"),
            "anthropic rejects an oauth call that also presents x-api-key"
        );
    }

    #[test]
    fn an_identity_header_is_announced_like_any_other_edit() {
        let auth = Auth::Oauth(Oauth {
            token: "at".into(),
            extra: vec![("chatgpt-account-id".into(), "acct_9".into())],
            merge_beta: None,
        });
        let (out, edits) = forward_headers(&headers(&[]), &auth);
        assert_eq!(out["chatgpt-account-id"], "acct_9");
        assert!(edits.iter().any(|edit| edit.name == "chatgpt-account-id"));
    }

    #[test]
    fn merging_ignores_blanks_and_stray_spacing() {
        assert_eq!(merge_beta(Some(" a , , b "), "x"), "x,a,b");
        assert_eq!(merge_beta(Some(""), "x"), "x");
        assert_eq!(merge_beta(None, "x"), "x");
    }

    #[test]
    fn unknown_headers_are_forwarded() {
        let incoming = headers(&[
            ("anthropic-beta", "some-future-thing-2027-01-01"),
            ("x-vendor-experiment", "on"),
        ]);
        let (out, _) = forward_headers(&incoming, &Auth::ApiKey("k".into()));
        assert_eq!(out["anthropic-beta"], "some-future-thing-2027-01-01");
        assert_eq!(out["x-vendor-experiment"], "on");
    }

    #[test]
    fn client_credentials_do_not_reach_the_provider() {
        let incoming = headers(&[("authorization", "Bearer client-key-for-us")]);
        let (out, edits) = forward_headers(&incoming, &Auth::ApiKey("provider-key".into()));
        assert!(!out.contains_key("authorization"));
        assert_eq!(out["x-api-key"], "provider-key");
        assert_eq!(edits, vec![HeaderEdit::new("x-api-key")]);
    }

    #[test]
    fn hop_by_hop_headers_are_dropped() {
        let incoming = headers(&[
            ("connection", "keep-alive"),
            ("transfer-encoding", "chunked"),
        ]);
        let (out, _) = forward_headers(&incoming, &Auth::ApiKey("k".into()));
        assert!(!out.contains_key("connection"));
        assert!(!out.contains_key("transfer-encoding"));
    }

    #[test]
    fn repeated_headers_keep_every_value() {
        let incoming = headers(&[("anthropic-beta", "a"), ("anthropic-beta", "b")]);
        let (out, _) = forward_headers(&incoming, &Auth::ApiKey("k".into()));
        assert_eq!(out.get_all("anthropic-beta").iter().count(), 2);
    }

    #[test]
    fn auth_never_prints_its_secret() {
        assert_eq!(
            format!("{:?}", Auth::ApiKey("sk-secret".into())),
            "ApiKey(***)"
        );
        assert_eq!(
            format!("{:?}", Auth::Bearer("sk-secret".into())),
            "Bearer(***)"
        );
    }
}
