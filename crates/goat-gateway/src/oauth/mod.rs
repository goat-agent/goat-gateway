mod account;
mod client;
mod loopback;
mod session;

pub use account::{KIND, PrepareError, Prepared, Stored, prepare};
pub use client::{Client, DeviceStart, OauthError, Tokens};
pub use loopback::{Callback, Listening, listen};
pub use session::{Sessions, SignInError, Started, Status, begin, finish_paste};

use std::collections::BTreeMap;

use base64::{Engine as _, engine::general_purpose::URL_SAFE_NO_PAD};
use rand::RngCore as _;
use serde::Deserialize;
use sha2::{Digest as _, Sha256};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum BodyFormat {
    Json,
    Form,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum StateSource {
    #[default]
    Random,
    Verifier,
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Loopback {
    pub host: String,
    pub port: u16,
    #[serde(default)]
    pub fallback_port: Option<u16>,
    pub path: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Device {
    pub user_code_url: String,
    pub poll_url: String,
    pub verification_url: String,
    pub redirect_uri: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Identity {
    pub claims: Vec<Vec<String>>,
    pub header: String,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Flow {
    #[serde(skip)]
    pub provider: String,
    pub label: String,
    pub client_id: String,
    #[serde(default)]
    pub issues_client: bool,
    pub authorize_url: String,
    pub token_url: String,
    pub scopes: String,
    #[serde(default)]
    pub must_grant: Vec<String>,
    pub body: BodyFormat,
    #[serde(default)]
    pub refresh_body: Option<BodyFormat>,
    #[serde(default)]
    pub state: StateSource,
    #[serde(default)]
    pub sends_state: bool,
    #[serde(default)]
    pub extra: BTreeMap<String, String>,
    #[serde(default)]
    pub first_only: Vec<String>,
    #[serde(default)]
    pub token_extra: BTreeMap<String, String>,
    #[serde(default)]
    pub loopback: Option<Loopback>,
    #[serde(default)]
    pub paste: Option<String>,
    #[serde(default)]
    pub device: Option<Device>,
    #[serde(default)]
    pub identity: Option<Identity>,
    #[serde(default)]
    pub merge_beta: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Mode {
    Loopback,
    Paste,
    Device,
}

pub struct Asking<'a> {
    pub install: &'a str,
    pub nonce: &'a str,
    pub first: bool,
}

impl Flow {
    pub fn supports(&self, mode: Mode) -> bool {
        match mode {
            Mode::Loopback => self.loopback.is_some(),
            Mode::Paste => self.paste.is_some() || self.loopback.is_some(),
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
        let loopback = |port: Option<u16>| {
            self.loopback.as_ref().map(|loopback| {
                format!(
                    "http://{}:{}{}",
                    loopback.host,
                    port.unwrap_or(loopback.port),
                    loopback.path
                )
            })
        };
        match mode {
            Mode::Loopback => loopback(bound_port),
            Mode::Paste => self.paste.clone().or_else(|| loopback(None)),
            Mode::Device => None,
        }
    }

    pub fn refresh_format(&self) -> BodyFormat {
        self.refresh_body.unwrap_or(self.body)
    }

    fn extra_for(&self, asking: &Asking<'_>) -> Vec<(String, String)> {
        self.extra
            .iter()
            .filter(|(name, _)| asking.first || !self.first_only.contains(name))
            .map(|(name, value)| {
                (
                    name.clone(),
                    value
                        .replace("{install}", asking.install)
                        .replace("{nonce}", asking.nonce),
                )
            })
            .collect()
    }

    pub fn granted(&self, scope: Option<&str>) -> Result<(), Vec<String>> {
        let granted: Vec<&str> = scope.unwrap_or_default().split_whitespace().collect();
        let missing: Vec<String> = self
            .must_grant
            .iter()
            .filter(|wanted| !granted.contains(&wanted.as_str()))
            .cloned()
            .collect();
        if missing.is_empty() {
            Ok(())
        } else {
            Err(missing)
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

pub fn authorize_url(
    flow: &Flow,
    client_id: &str,
    redirect_uri: &str,
    challenge: &str,
    state: &str,
    asking: &Asking<'_>,
) -> String {
    let mut query = vec![
        ("client_id".to_owned(), client_id.to_owned()),
        ("response_type".to_owned(), "code".to_owned()),
        ("redirect_uri".to_owned(), redirect_uri.to_owned()),
        ("scope".to_owned(), flow.scopes.clone()),
        ("code_challenge".to_owned(), challenge.to_owned()),
        ("code_challenge_method".to_owned(), "S256".to_owned()),
        ("state".to_owned(), state.to_owned()),
    ];
    query.extend(flow.extra_for(asking));

    let encoded = query
        .into_iter()
        .map(|(name, value)| format!("{}={}", encode(&name), encode(&value)))
        .collect::<Vec<_>>()
        .join("&");

    format!("{}?{encoded}", flow.authorize_url)
}

pub fn exchange_fields(
    flow: &Flow,
    client_id: &str,
    code: &str,
    state: &str,
    verifier: &str,
    redirect_uri: &str,
) -> Vec<(String, String)> {
    let mut fields = vec![
        ("grant_type".to_owned(), "authorization_code".to_owned()),
        ("code".to_owned(), code.to_owned()),
        ("redirect_uri".to_owned(), redirect_uri.to_owned()),
        ("client_id".to_owned(), client_id.to_owned()),
        ("code_verifier".to_owned(), verifier.to_owned()),
    ];
    if flow.sends_state {
        fields.push(("state".to_owned(), state.to_owned()));
    }
    fields.extend(flow.token_extra.clone());
    fields
}

pub fn refresh_fields(flow: &Flow, client_id: &str, refresh_token: &str) -> Vec<(String, String)> {
    let mut fields = vec![
        ("grant_type".to_owned(), "refresh_token".to_owned()),
        ("refresh_token".to_owned(), refresh_token.to_owned()),
        ("client_id".to_owned(), client_id.to_owned()),
    ];
    fields.extend(flow.token_extra.clone());
    fields
}

pub fn account_from_id_token(id_token: &str, identity: &Identity) -> Option<String> {
    let claims = decode_jwt_payload(id_token)?;
    for path in &identity.claims {
        let mut cursor = &claims;
        let mut found = true;
        for segment in path {
            match cursor.get(segment.as_str()) {
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

    fn flow(provider: &str) -> Flow {
        crate::provider::Catalog::builtin()
            .flow(provider)
            .cloned()
            .unwrap_or_else(|| panic!("{provider} declares a sign-in"))
    }

    fn asking() -> Asking<'static> {
        Asking {
            install: "urn:uuid:1",
            nonce: "n1",
            first: true,
        }
    }

    #[test]
    fn the_challenge_is_the_sha256_of_the_verifier() {
        let pkce = Pkce::generate();
        let expected = URL_SAFE_NO_PAD.encode(Sha256::digest(pkce.verifier.as_bytes()));
        assert_eq!(pkce.challenge, expected);
        assert_ne!(pkce.verifier, pkce.challenge);
    }

    #[test]
    fn scopes_keep_their_spaces_as_percent_twenty() {
        let anthropic = flow("anthropic");
        let url = authorize_url(
            &anthropic,
            &anthropic.client_id,
            "http://localhost:54545/callback",
            "ch",
            "st",
            &asking(),
        );
        assert!(url.contains("scope=org%3Acreate_api_key%20user%3Aprofile%20user%3Ainference"));
        assert!(url.contains("redirect_uri=http%3A%2F%2Flocalhost%3A54545%2Fcallback"));
    }

    #[test]
    fn each_flow_carries_its_own_extra_parameters() {
        let anthropic = flow("anthropic");
        let chatgpt = flow("chatgpt");
        let anthropic = authorize_url(&anthropic, "c", "r", "c", "s", &asking());
        let chatgpt = authorize_url(&chatgpt, "c", "r", "c", "s", &asking());
        assert!(anthropic.contains("code=true"));
        assert!(!anthropic.contains("codex_cli_simplified_flow"));
        assert!(chatgpt.contains("codex_cli_simplified_flow=true"));
        assert!(!chatgpt.contains("code=true"));
    }

    #[test]
    fn a_per_install_value_is_filled_in_and_a_first_time_hint_is_sent_only_the_first_time() {
        let official = flow("chatgpt_signin");
        let first = authorize_url(&official, "dynamic_agent_client", "r", "c", "s", &asking());
        assert!(
            first.contains("ext_agent_host_id=urn%3Auuid%3A1"),
            "{first}"
        );
        assert!(first.contains("nonce=n1"), "{first}");
        assert!(first.contains("agent_name_hint="), "{first}");

        let again = authorize_url(
            &official,
            "oaiapp_1",
            "r",
            "c",
            "s",
            &Asking {
                first: false,
                ..asking()
            },
        );
        assert!(!again.contains("agent_name_hint"), "{again}");
        assert!(again.contains("ext_agent_host_id=urn%3Auuid%3A1"));
    }

    #[test]
    fn state_is_sent_only_where_the_provider_expects_it() {
        let anthropic = exchange_fields(&flow("anthropic"), "c", "code", "state", "v", "r");
        let chatgpt = exchange_fields(&flow("chatgpt"), "c", "code", "state", "v", "r");
        assert!(anthropic.iter().any(|(name, _)| name == "state"));
        assert!(!chatgpt.iter().any(|(name, _)| name == "state"));
    }

    #[test]
    fn a_plan_that_was_not_granted_is_named() {
        let official = flow("chatgpt_signin");
        assert!(
            official
                .granted(Some("openid email chatgpt.tokens.use.direct"))
                .is_ok()
        );
        assert_eq!(
            official.granted(Some("openid email")),
            Err(vec!["chatgpt.tokens.use.direct".to_owned()])
        );
        assert!(flow("anthropic").granted(None).is_ok());
    }

    #[test]
    fn the_account_id_is_found_under_either_claim_shape() {
        let flat = payload(serde_json::json!({ "chatgpt_account_id": "acct_flat" }));
        let nested = payload(serde_json::json!({
            "https://api.openai.com/auth": { "chatgpt_account_id": "acct_nested" }
        }));
        let chatgpt = flow("chatgpt");
        let identity = chatgpt.identity.as_ref().unwrap();

        assert_eq!(
            account_from_id_token(&flat, identity).as_deref(),
            Some("acct_flat")
        );
        assert_eq!(
            account_from_id_token(&nested, identity).as_deref(),
            Some("acct_nested")
        );
        assert_eq!(
            account_from_id_token(&payload(serde_json::json!({ "sub": "x" })), identity),
            None
        );
    }

    #[test]
    fn the_loopback_port_follows_the_socket_we_actually_bound() {
        let chatgpt = flow("chatgpt");
        assert_eq!(
            chatgpt.redirect_uri(Mode::Loopback, Some(1457)).as_deref(),
            Some("http://127.0.0.1:1457/auth/callback")
        );
        assert_eq!(
            chatgpt.redirect_uri(Mode::Loopback, None).as_deref(),
            Some("http://127.0.0.1:1455/auth/callback")
        );
    }

    #[test]
    fn every_sign_in_works_when_the_browser_cannot_reach_the_gateway() {
        for flow in crate::provider::Catalog::builtin().flows() {
            assert!(
                flow.supports(Mode::Paste) || flow.supports(Mode::Device),
                "{} has only a loopback redirect, so a server or a container could never sign in",
                flow.provider
            );
        }
    }

    #[test]
    fn a_loopback_only_flow_can_finish_by_pasting_the_address_the_browser_landed_on() {
        let official = flow("chatgpt_signin");
        assert!(official.supports(Mode::Paste));
        assert_eq!(
            official.redirect_uri(Mode::Paste, None),
            official.redirect_uri(Mode::Loopback, None),
            "the provider only knows the loopback address, so pasting must use the same one"
        );
    }

    fn payload(claims: serde_json::Value) -> String {
        format!(
            "header.{}.signature",
            URL_SAFE_NO_PAD.encode(serde_json::to_vec(&claims).unwrap())
        )
    }
}
