mod account;
mod client;
mod loopback;
mod session;

pub use account::{PrepareError, Prepared, Stored, prepare};
pub use client::{Client, DeviceStart, OauthError, Tokens};
pub use loopback::{Callback, Listening, listen};
pub use session::{Sessions, SignInError, Started, Status, begin, finish_paste};

use base64::{Engine as _, engine::general_purpose::URL_SAFE_NO_PAD};
use rand::RngCore as _;
use sha2::{Digest as _, Sha256};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BodyFormat {
    Json,
    Form,
}

#[derive(Debug, Clone, Copy)]
pub struct Loopback {
    pub port: u16,
    pub fallback_port: Option<u16>,
    pub path: &'static str,
}

#[derive(Debug, Clone, Copy)]
pub struct Device {
    pub user_code_url: &'static str,
    pub poll_url: &'static str,
    pub verification_url: &'static str,
    pub redirect_uri: &'static str,
}

#[derive(Debug, Clone, Copy)]
pub struct Identity {
    pub claims: &'static [&'static [&'static str]],
    pub header: &'static str,
}

#[derive(Debug, Clone, Copy)]
pub struct Flow {
    pub provider: &'static str,
    pub label: &'static str,
    pub client_id: &'static str,
    pub authorize_url: &'static str,
    pub token_url: &'static str,
    pub scopes: &'static str,
    pub body: BodyFormat,
    pub sends_state: bool,
    pub extra: &'static [(&'static str, &'static str)],
    pub loopback: Option<Loopback>,
    pub paste: Option<&'static str>,
    pub device: Option<Device>,
    pub identity: Option<Identity>,
    pub base_url: Option<&'static str>,
    pub merge_beta: Option<&'static str>,
}

pub const ANTHROPIC: Flow = Flow {
    provider: "anthropic",
    label: "Claude Pro/Max",
    client_id: "9d1c250a-e61b-44d9-88ed-5944d1962f5e",
    authorize_url: "https://claude.ai/oauth/authorize",
    token_url: "https://api.anthropic.com/v1/oauth/token",
    scopes: "org:create_api_key user:profile user:inference",
    body: BodyFormat::Json,
    sends_state: true,
    extra: &[("code", "true")],
    loopback: Some(Loopback {
        port: 54545,
        fallback_port: None,
        path: "/callback",
    }),
    paste: Some("https://console.anthropic.com/oauth/code/callback"),
    device: None,
    identity: None,
    base_url: None,
    merge_beta: Some("oauth-2025-04-20"),
};

pub const OPENAI: Flow = Flow {
    provider: "openai",
    label: "ChatGPT Plus/Pro",
    client_id: "app_EMoamEEZ73f0CkXaXp7hrann",
    authorize_url: "https://auth.openai.com/oauth/authorize",
    token_url: "https://auth.openai.com/oauth/token",
    scopes: "openid profile email offline_access",
    body: BodyFormat::Form,
    sends_state: false,
    extra: &[
        ("id_token_add_organizations", "true"),
        ("codex_cli_simplified_flow", "true"),
    ],
    loopback: Some(Loopback {
        port: 1455,
        fallback_port: Some(1457),
        path: "/auth/callback",
    }),
    paste: None,
    device: Some(Device {
        user_code_url: "https://auth.openai.com/api/accounts/deviceauth/usercode",
        poll_url: "https://auth.openai.com/api/accounts/deviceauth/token",
        verification_url: "https://auth.openai.com/codex/device",
        redirect_uri: "https://auth.openai.com/deviceauth/callback",
    }),
    identity: Some(Identity {
        claims: &[
            &["chatgpt_account_id"],
            &["https://api.openai.com/auth", "chatgpt_account_id"],
        ],
        header: "chatgpt-account-id",
    }),
    base_url: Some("https://chatgpt.com/backend-api/codex"),
    merge_beta: None,
};

pub const FLOWS: &[Flow] = &[ANTHROPIC, OPENAI];

pub fn flow(provider: &str) -> Option<&'static Flow> {
    FLOWS.iter().find(|flow| flow.provider == provider)
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Mode {
    Loopback,
    Paste,
    Device,
}

impl Flow {
    pub fn supports(&self, mode: Mode) -> bool {
        match mode {
            Mode::Loopback => self.loopback.is_some(),
            Mode::Paste => self.paste.is_some(),
            Mode::Device => self.device.is_some(),
        }
    }

    pub fn modes(&self) -> Vec<Mode> {
        [Mode::Loopback, Mode::Paste, Mode::Device]
            .into_iter()
            .filter(|mode| self.supports(*mode))
            .collect()
    }

    pub fn redirect_uri(&self, mode: Mode, bound_port: Option<u16>) -> Option<String> {
        match mode {
            Mode::Loopback => self.loopback.map(|loopback| {
                let port = bound_port.unwrap_or(loopback.port);
                format!("http://localhost:{port}{}", loopback.path)
            }),
            Mode::Paste => self.paste.map(str::to_owned),
            Mode::Device => None,
        }
    }
}

pub struct Pkce {
    pub verifier: String,
    pub challenge: String,
}

impl Pkce {
    pub fn generate() -> Self {
        let mut bytes = [0u8; 32];
        rand::rng().fill_bytes(&mut bytes);
        let verifier = URL_SAFE_NO_PAD.encode(bytes);
        let challenge = URL_SAFE_NO_PAD.encode(Sha256::digest(verifier.as_bytes()));
        Self {
            verifier,
            challenge,
        }
    }
}

pub fn random_state() -> String {
    let mut bytes = [0u8; 32];
    rand::rng().fill_bytes(&mut bytes);
    URL_SAFE_NO_PAD.encode(bytes)
}

pub fn authorize_url(flow: &Flow, redirect_uri: &str, challenge: &str, state: &str) -> String {
    let mut query = vec![
        ("client_id", flow.client_id.to_owned()),
        ("response_type", "code".to_owned()),
        ("redirect_uri", redirect_uri.to_owned()),
        ("scope", flow.scopes.to_owned()),
        ("code_challenge", challenge.to_owned()),
        ("code_challenge_method", "S256".to_owned()),
        ("state", state.to_owned()),
    ];
    for (name, value) in flow.extra {
        query.push((name, (*value).to_owned()));
    }

    let encoded = query
        .into_iter()
        .map(|(name, value)| format!("{}={}", encode(name), encode(&value)))
        .collect::<Vec<_>>()
        .join("&");

    format!("{}?{encoded}", flow.authorize_url)
}

pub fn split_pasted(pasted: &str) -> (String, Option<String>) {
    let trimmed = pasted.trim();
    let raw = trimmed
        .split_once("code=")
        .map(|(_, rest)| rest.split('&').next().unwrap_or(rest))
        .unwrap_or(trimmed);
    match raw.split_once('#') {
        Some((code, state)) => (code.to_owned(), Some(state.to_owned())),
        None => (raw.to_owned(), None),
    }
}

pub fn exchange_fields(
    flow: &Flow,
    code: &str,
    state: &str,
    verifier: &str,
    redirect_uri: &str,
) -> Vec<(&'static str, String)> {
    let mut fields = vec![
        ("grant_type", "authorization_code".to_owned()),
        ("code", code.to_owned()),
        ("redirect_uri", redirect_uri.to_owned()),
        ("client_id", flow.client_id.to_owned()),
        ("code_verifier", verifier.to_owned()),
    ];
    if flow.sends_state {
        fields.push(("state", state.to_owned()));
    }
    fields
}

pub fn refresh_fields(flow: &Flow, refresh_token: &str) -> Vec<(&'static str, String)> {
    vec![
        ("grant_type", "refresh_token".to_owned()),
        ("refresh_token", refresh_token.to_owned()),
        ("client_id", flow.client_id.to_owned()),
    ]
}

pub fn account_from_id_token(id_token: &str, identity: &Identity) -> Option<String> {
    let claims = decode_jwt_payload(id_token)?;
    for path in identity.claims {
        let mut cursor = &claims;
        let mut found = true;
        for segment in *path {
            match cursor.get(*segment) {
                Some(next) => cursor = next,
                None => {
                    found = false;
                    break;
                }
            }
        }
        if found && let Some(value) = cursor.as_str() {
            return Some(value.to_owned());
        }
    }
    None
}

pub fn decode_jwt_payload(token: &str) -> Option<serde_json::Value> {
    let payload = token.split('.').nth(1)?;
    let bytes = URL_SAFE_NO_PAD.decode(payload).ok()?;
    serde_json::from_slice(&bytes).ok()
}

fn encode(value: &str) -> String {
    let mut out = String::with_capacity(value.len());
    for byte in value.bytes() {
        match byte {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' => {
                out.push(byte as char);
            }
            _ => out.push_str(&format!("%{byte:02X}")),
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_challenge_is_the_sha256_of_the_verifier() {
        let pkce = Pkce::generate();
        let expected = URL_SAFE_NO_PAD.encode(Sha256::digest(pkce.verifier.as_bytes()));
        assert_eq!(pkce.challenge, expected);
        assert_ne!(pkce.verifier, pkce.challenge);
    }

    #[test]
    fn scopes_keep_their_spaces_as_percent_twenty() {
        let url = authorize_url(&ANTHROPIC, "http://localhost:54545/callback", "ch", "st");
        assert!(url.contains("scope=org%3Acreate_api_key%20user%3Aprofile%20user%3Ainference"));
        assert!(url.contains("redirect_uri=http%3A%2F%2Flocalhost%3A54545%2Fcallback"));
    }

    #[test]
    fn each_flow_carries_its_own_extra_parameters() {
        let anthropic = authorize_url(&ANTHROPIC, "r", "c", "s");
        let openai = authorize_url(&OPENAI, "r", "c", "s");
        assert!(anthropic.contains("code=true"));
        assert!(!anthropic.contains("codex_cli_simplified_flow"));
        assert!(openai.contains("id_token_add_organizations=true"));
        assert!(openai.contains("codex_cli_simplified_flow=true"));
        assert!(!openai.contains("code=true"));
    }

    #[test]
    fn state_is_sent_only_where_the_provider_expects_it() {
        let anthropic = exchange_fields(&ANTHROPIC, "code", "state", "verifier", "redirect");
        let openai = exchange_fields(&OPENAI, "code", "state", "verifier", "redirect");
        assert!(anthropic.iter().any(|(name, _)| *name == "state"));
        assert!(!openai.iter().any(|(name, _)| *name == "state"));
    }

    #[test]
    fn the_pasted_code_carries_the_state_behind_a_hash() {
        assert_eq!(
            split_pasted("  ac_123#st_456 "),
            ("ac_123".to_owned(), Some("st_456".to_owned()))
        );
        assert_eq!(split_pasted("ac_123"), ("ac_123".to_owned(), None));
    }

    #[test]
    fn a_whole_redirect_url_can_be_pasted_instead_of_the_code() {
        assert_eq!(
            split_pasted("https://console.anthropic.com/oauth/code/callback?code=ac_1#st_2&x=1"),
            ("ac_1".to_owned(), Some("st_2".to_owned()))
        );
    }

    #[test]
    fn the_account_id_is_found_under_either_claim_shape() {
        let flat = payload(serde_json::json!({ "chatgpt_account_id": "acct_flat" }));
        let nested = payload(serde_json::json!({
            "https://api.openai.com/auth": { "chatgpt_account_id": "acct_nested" }
        }));
        let identity = OPENAI.identity.unwrap();

        assert_eq!(
            account_from_id_token(&flat, &identity).as_deref(),
            Some("acct_flat")
        );
        assert_eq!(
            account_from_id_token(&nested, &identity).as_deref(),
            Some("acct_nested")
        );
        assert_eq!(
            account_from_id_token(&payload(serde_json::json!({ "sub": "x" })), &identity),
            None
        );
    }

    #[test]
    fn a_claim_name_containing_dots_is_not_split() {
        let identity = OPENAI.identity.unwrap();
        assert!(
            identity
                .claims
                .iter()
                .any(|path| path.len() == 2 && path[0].contains('.')),
            "the nested claim is a URL, so a dotted path string would tear it apart"
        );
    }

    #[test]
    fn the_loopback_port_follows_the_socket_we_actually_bound() {
        assert_eq!(
            OPENAI.redirect_uri(Mode::Loopback, Some(1457)).as_deref(),
            Some("http://localhost:1457/auth/callback")
        );
        assert_eq!(
            OPENAI.redirect_uri(Mode::Loopback, None).as_deref(),
            Some("http://localhost:1455/auth/callback")
        );
    }

    #[test]
    fn every_provider_offers_a_way_in_when_the_browser_cannot_reach_us() {
        for flow in FLOWS {
            let offline = flow.supports(Mode::Paste) || flow.supports(Mode::Device);
            assert!(
                offline,
                "{} has only a loopback redirect, so a headless box could never sign in",
                flow.provider
            );
        }
    }

    #[test]
    fn a_flow_never_claims_a_mode_it_cannot_build_a_redirect_for() {
        for flow in FLOWS {
            for mode in flow.modes() {
                if mode != Mode::Device {
                    assert!(flow.redirect_uri(mode, None).is_some());
                }
            }
        }
    }

    fn payload(claims: serde_json::Value) -> String {
        format!(
            "header.{}.signature",
            URL_SAFE_NO_PAD.encode(serde_json::to_vec(&claims).unwrap())
        )
    }
}
