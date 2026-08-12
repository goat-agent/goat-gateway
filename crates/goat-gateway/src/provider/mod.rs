use std::collections::BTreeMap;

use goat_gateway_wire::responses_to_messages::{TargetModel, ThinkingStyle};
use serde::Deserialize;

const BUILTIN: &str = include_str!("builtin.toml");

#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Wire {
    Messages,
    Responses,
    Chat,
}

impl Wire {
    pub fn id(self) -> &'static str {
        match self {
            Self::Messages => "messages",
            Self::Responses => "responses",
            Self::Chat => "chat",
        }
    }

    pub fn label(self) -> &'static str {
        match self {
            Self::Messages => "Messages",
            Self::Responses => "Responses",
            Self::Chat => "Chat Completions",
        }
    }
}

impl std::fmt::Display for Wire {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.label())
    }
}

#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Endpoint {
    pub wire: Wire,
    pub url: String,
}

#[derive(Debug, Clone, Copy, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Price {
    pub input: i64,
    pub output: i64,
    pub cache_read: i64,
    pub cache_write: i64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Thinking {
    Adaptive,
    Budget,
    Unsupported,
}

impl From<Thinking> for ThinkingStyle {
    fn from(value: Thinking) -> Self {
        match value {
            Thinking::Adaptive => Self::Adaptive,
            Thinking::Budget => Self::Budget,
            Thinking::Unsupported => Self::Unsupported,
        }
    }
}

#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Model {
    pub name: String,
    #[serde(default)]
    pub limit_scope: Option<String>,
    #[serde(default)]
    pub cache_min_tokens: Option<u32>,
    #[serde(default)]
    pub price: Option<Price>,
    #[serde(default)]
    pub thinking: Option<Thinking>,
    #[serde(default)]
    pub max_tokens: Option<u32>,
    #[serde(default)]
    pub mid_conversation_system: bool,
}

impl Model {
    pub fn target(&self) -> Option<TargetModel> {
        Some(TargetModel {
            name: self.name.clone(),
            thinking: self.thinking?.into(),
            default_max_tokens: self.max_tokens?,
            mid_conversation_system: self.mid_conversation_system,
        })
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Key {
    Bearer,
    XApiKey,
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Limits {
    None,
    Headers,
    Endpoint { url: String },
}

#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Provider {
    #[serde(skip)]
    pub id: String,
    pub label: String,
    pub endpoints: Vec<Endpoint>,
    #[serde(default)]
    pub headers: BTreeMap<String, String>,
    #[serde(default = "no_limits")]
    pub limits: Limits,
    #[serde(default = "bearer")]
    pub key: Key,
    #[serde(default)]
    pub models: Vec<Model>,
}

fn no_limits() -> Limits {
    Limits::None
}

fn bearer() -> Key {
    Key::Bearer
}

pub fn translatable(from: Wire, to: Wire) -> bool {
    matches!(
        (from, to),
        (Wire::Responses, Wire::Messages)
            | (Wire::Messages, Wire::Chat)
            | (Wire::Chat, Wire::Messages)
    )
}

fn needs_thinking_declared(from: Wire, to: Wire) -> bool {
    matches!((from, to), (Wire::Responses, Wire::Messages))
}

fn path_of(url: &str) -> &str {
    let rest = url.split_once("://").map_or(url, |(_, rest)| rest);
    rest.find('/').map_or("", |at| &rest[at..])
}

impl Provider {
    pub fn endpoint(&self, wire: Wire) -> Option<&Endpoint> {
        self.endpoints.iter().find(|endpoint| endpoint.wire == wire)
    }

    pub fn speaks(&self, wire: Wire) -> bool {
        self.endpoint(wire).is_some()
    }

    pub fn model(&self, name: &str) -> Option<&Model> {
        self.models.iter().find(|model| model.name == name)
    }

    pub fn reachable(&self, ingress: Wire) -> bool {
        self.endpoints
            .iter()
            .any(|endpoint| endpoint.wire == ingress || translatable(ingress, endpoint.wire))
    }

    pub fn route(
        &self,
        ingress: Wire,
        model: &str,
        declared: Option<Model>,
    ) -> Result<Route, Unroutable> {
        let carry = |endpoint: &Endpoint| Route {
            provider: self.id.clone(),
            headers: self.headers.clone(),
            key: self.key,
            endpoint: endpoint.clone(),
            model: declared.clone(),
        };

        if let Some(endpoint) = self.endpoint(ingress) {
            return Ok(carry(endpoint));
        }
        let Some(endpoint) = self
            .endpoints
            .iter()
            .find(|endpoint| translatable(ingress, endpoint.wire))
        else {
            return Err(Unroutable::NoTranslation {
                provider: self.label.clone(),
                wire: ingress,
            });
        };
        if needs_thinking_declared(ingress, endpoint.wire)
            && declared.as_ref().and_then(Model::target).is_none()
        {
            return Err(Unroutable::Undeclared {
                model: model.to_owned(),
                wire: endpoint.wire,
                known: self.model_names().join(", "),
            });
        }
        Ok(carry(endpoint))
    }

    pub fn model_names(&self) -> Vec<&str> {
        self.models
            .iter()
            .map(|model| model.name.as_str())
            .collect()
    }
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct File {
    #[serde(default)]
    provider: BTreeMap<String, Provider>,
}

#[derive(Debug, thiserror::Error)]
pub enum CatalogError {
    #[error("{source_name} is not valid: {source}")]
    Parse {
        source_name: String,
        #[source]
        source: toml::de::Error,
    },
    #[error("{provider} declares no endpoint, so nothing could be routed to it")]
    NoEndpoints { provider: String },
}

#[derive(Debug, Clone, Default)]
pub struct Catalog {
    providers: BTreeMap<String, Provider>,
}

impl Catalog {
    pub fn builtin() -> Self {
        Self::parse(BUILTIN, "the built-in provider list").expect("built-in providers are valid")
    }

    pub fn with_overlay(overlay: &str) -> Result<Self, CatalogError> {
        let mut catalog = Self::builtin();
        let extra = Self::parse(overlay, "config.toml")?;
        catalog.providers.extend(extra.providers);
        Ok(catalog)
    }

    fn parse(text: &str, source_name: &str) -> Result<Self, CatalogError> {
        let file: File = toml::from_str(text).map_err(|source| CatalogError::Parse {
            source_name: source_name.to_owned(),
            source,
        })?;
        let mut providers = BTreeMap::new();
        for (id, mut provider) in file.provider {
            if provider.endpoints.is_empty() {
                return Err(CatalogError::NoEndpoints { provider: id });
            }
            provider.id = id.clone();
            providers.insert(id, provider);
        }
        Ok(Self { providers })
    }

    pub fn with_base_url(mut self, provider: &str, base_url: &str) -> Self {
        let base = base_url.trim_end_matches('/');
        if let Some(entry) = self.providers.get_mut(provider) {
            for endpoint in &mut entry.endpoints {
                endpoint.url = format!("{base}{}", path_of(&endpoint.url));
            }
        }
        self
    }

    pub fn get(&self, id: &str) -> Option<&Provider> {
        self.providers.get(id)
    }

    pub fn iter(&self) -> impl Iterator<Item = &Provider> {
        self.providers.values()
    }

    pub fn model(&self, provider: &str, model: &str) -> Option<&Model> {
        self.get(provider)?.model(model)
    }

    pub fn route(
        &self,
        ingress: Wire,
        model: &str,
        available: &[String],
    ) -> Result<Route, Unroutable> {
        if let Some((provider, declared)) = self
            .iter()
            .find_map(|provider| Some((provider, provider.model(model)?)))
        {
            return provider.route(ingress, model, Some(declared.clone()));
        }

        let registered: Vec<&Provider> = self
            .iter()
            .filter(|provider| available.contains(&provider.id))
            .collect();

        let native: Vec<&Provider> = registered
            .iter()
            .copied()
            .filter(|provider| provider.speaks(ingress))
            .collect();
        let reachable: Vec<&Provider> = registered
            .into_iter()
            .filter(|provider| provider.reachable(ingress))
            .collect();

        let shortlist = if native.is_empty() { reachable } else { native };
        match shortlist.as_slice() {
            [] => Err(Unroutable::NobodySpeaks { wire: ingress }),
            [only] => only.route(ingress, model, None),
            several => Err(Unroutable::Ambiguous {
                model: model.to_owned(),
                candidates: several
                    .iter()
                    .map(|provider| provider.label.as_str())
                    .collect::<Vec<_>>()
                    .join(" and "),
            }),
        }
    }
}

#[derive(Debug, Clone)]
pub struct Route {
    pub provider: String,
    pub headers: BTreeMap<String, String>,
    pub key: Key,
    pub endpoint: Endpoint,
    pub model: Option<Model>,
}

impl Route {
    pub fn translates_from(&self, ingress: Wire) -> bool {
        self.endpoint.wire != ingress
    }

    pub fn cache_min_tokens(&self) -> u32 {
        if self.endpoint.wire != Wire::Messages {
            return 0;
        }
        self.model
            .as_ref()
            .and_then(|model| model.cache_min_tokens)
            .unwrap_or(0)
    }

    pub fn limit_scope(&self) -> Option<&str> {
        self.model.as_ref()?.limit_scope.as_deref()
    }

    pub fn price(&self) -> Option<Price> {
        self.model.as_ref()?.price
    }
}

#[derive(Debug, thiserror::Error)]
pub enum Unroutable {
    #[error("no account is registered with a provider that serves the {wire} format")]
    NobodySpeaks { wire: Wire },
    #[error(
        "{provider} does not serve the {wire} format, and this gateway cannot yet translate into \
         anything it does serve"
    )]
    NoTranslation { provider: String, wire: Wire },
    #[error(
        "model {model:?} is not declared, and {candidates} could each serve it. \
         Declare the model in config.toml, or keep accounts for only one of them."
    )]
    Ambiguous { model: String, candidates: String },
    #[error(
        "model {model:?} has to be translated into {wire} to be served, and translating needs it \
         declared with thinking and max_tokens, so the gateway knows how it reasons and how much \
         it may write. Declared models: {known}"
    )]
    Undeclared {
        model: String,
        wire: Wire,
        known: String,
    },
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_built_in_list_parses() {
        let catalog = Catalog::builtin();
        assert!(catalog.get("anthropic").is_some());
        assert!(catalog.get("openai").is_some());
    }

    #[test]
    fn properties_are_declared_not_guessed_from_the_name() {
        let catalog = Catalog::builtin();
        let haiku = catalog.model("anthropic", "claude-haiku-4-5").unwrap();
        let sonnet = catalog.model("anthropic", "claude-sonnet-5").unwrap();

        assert_eq!(haiku.thinking, Some(Thinking::Budget));
        assert_eq!(sonnet.thinking, Some(Thinking::Adaptive));
        assert!(!sonnet.mid_conversation_system);
        assert!(
            catalog
                .model("anthropic", "claude-opus-5")
                .unwrap()
                .mid_conversation_system
        );
    }

    #[test]
    fn an_unknown_model_is_unknown_rather_than_assumed() {
        let catalog = Catalog::builtin();
        assert!(catalog.model("anthropic", "claude-sonnet-6").is_none());
        assert!(catalog.model("anthropic", "claudette-fast").is_none());
    }

    #[test]
    fn a_model_without_a_published_price_has_none() {
        let catalog = Catalog::builtin();
        assert!(
            catalog
                .model("anthropic", "claude-fable-5")
                .unwrap()
                .price
                .is_none()
        );
        assert!(
            catalog
                .model("anthropic", "claude-sonnet-5")
                .unwrap()
                .price
                .is_some()
        );
    }

    fn both() -> Vec<String> {
        vec!["anthropic".to_owned(), "openai".to_owned()]
    }

    #[test]
    fn a_provider_takes_its_key_the_way_it_asks_for_it() {
        let catalog = Catalog::builtin();
        assert_eq!(catalog.get("anthropic").unwrap().key, Key::XApiKey);
        assert_eq!(catalog.get("openai").unwrap().key, Key::Bearer);
        assert_eq!(catalog.get("zai").unwrap().key, Key::Bearer);
    }

    #[test]
    fn a_coding_plan_is_its_own_provider_because_it_is_its_own_host() {
        let catalog = Catalog::builtin();
        let kimi = catalog.get("kimi").unwrap();
        let moonshot = catalog.get("moonshot").unwrap();
        assert_ne!(
            kimi.endpoint(Wire::Chat).unwrap().url,
            moonshot.endpoint(Wire::Chat).unwrap().url,
            "a coding-plan key sent to the pay-per-token host fails quietly"
        );
    }

    #[test]
    fn an_override_replaces_one_provider_and_leaves_the_rest() {
        let catalog = Catalog::with_overlay(
            r#"
            [provider.anthropic]
            label = "Anthropic through a proxy"
            key = "x_api_key"
            endpoints = [{ wire = "messages", url = "http://10.0.0.2/v1/messages" }]
            "#,
        )
        .unwrap();

        assert_eq!(
            catalog
                .get("anthropic")
                .unwrap()
                .endpoint(Wire::Messages)
                .unwrap()
                .url,
            "http://10.0.0.2/v1/messages"
        );
        assert!(catalog.get("openai").is_some());
    }

    #[test]
    fn a_typo_in_an_override_is_refused_rather_than_half_applied() {
        let error = Catalog::with_overlay(
            r#"
            [provider.anthropic]
            label = "mine"
            endpoint = [{ wire = "messages", url = "http://10.0.0.2/v1/messages" }]
            "#,
        )
        .unwrap_err();
        assert!(matches!(error, CatalogError::Parse { .. }));
        assert!(error.to_string().contains("config.toml"));
    }

    #[test]
    fn a_model_goes_to_the_provider_that_declares_it() {
        let catalog = Catalog::builtin();
        let route = catalog
            .route(Wire::Responses, "gpt-5", &both())
            .expect("openai declares gpt-5");
        assert_eq!(route.provider, "openai");
        assert!(!route.translates_from(Wire::Responses));

        let route = catalog
            .route(Wire::Responses, "claude-sonnet-5", &both())
            .expect("anthropic declares claude-sonnet-5");
        assert_eq!(route.provider, "anthropic");
        assert!(
            route.translates_from(Wire::Responses),
            "anthropic serves Messages, so a Responses request has to be translated"
        );
    }

    #[test]
    fn a_model_that_shipped_after_us_still_reaches_the_provider_that_speaks_the_format() {
        let catalog = Catalog::builtin();
        let route = catalog
            .route(Wire::Responses, "gpt-6-that-shipped-today", &both())
            .expect("openai speaks Responses natively and anthropic does not");
        assert_eq!(route.provider, "openai");
        assert!(route.model.is_none());
        assert!(!route.translates_from(Wire::Responses));

        let route = catalog
            .route(Wire::Messages, "claude-sonnet-6", &both())
            .expect("only anthropic speaks Messages");
        assert_eq!(route.provider, "anthropic");
        assert!(!route.translates_from(Wire::Messages));
    }

    #[test]
    fn an_undeclared_model_that_needs_translating_is_refused_with_what_is_known() {
        let catalog = Catalog::builtin();
        let error = catalog
            .route(
                Wire::Responses,
                "claude-sonnet-6",
                &["anthropic".to_owned()],
            )
            .unwrap_err();
        let message = error.to_string();
        assert!(matches!(error, Unroutable::Undeclared { .. }));
        assert!(message.contains("claude-sonnet-6"), "{message}");
        assert!(message.contains("claude-sonnet-5"), "{message}");
    }

    #[test]
    fn a_format_no_registered_account_serves_says_so() {
        let catalog = Catalog::builtin();
        let error = catalog.route(Wire::Chat, "whatever", &[]).unwrap_err();
        assert!(matches!(error, Unroutable::NobodySpeaks { .. }));
    }

    #[test]
    fn one_anthropic_account_can_be_reached_from_any_of_the_three() {
        let catalog = Catalog::builtin();
        let registered = ["anthropic".to_owned()];

        for wire in [Wire::Messages, Wire::Responses, Wire::Chat] {
            let route = catalog
                .route(wire, "claude-sonnet-5", &registered)
                .unwrap_or_else(|error| panic!("{wire} could not be served: {error}"));
            assert_eq!(route.provider, "anthropic");
            assert_eq!(route.endpoint.wire, Wire::Messages);
        }
    }

    #[test]
    fn an_account_nobody_registered_never_wins_the_toss() {
        let catalog = Catalog::builtin();
        let route = catalog.route(Wire::Responses, "unknown", &["anthropic".to_owned()]);
        assert!(
            matches!(route, Err(Unroutable::Undeclared { .. })),
            "with no openai account the only candidate is anthropic, by translation"
        );
    }

    #[test]
    fn passthrough_is_computed_from_the_declared_endpoints() {
        let catalog = Catalog::builtin();
        assert!(catalog.get("anthropic").unwrap().speaks(Wire::Messages));
        assert!(!catalog.get("anthropic").unwrap().speaks(Wire::Responses));
        assert!(catalog.get("openai").unwrap().speaks(Wire::Responses));
    }

    #[test]
    fn an_overlay_replaces_a_provider_whole() {
        let catalog = Catalog::with_overlay(
            r#"
            [provider.anthropic]
            label = "Mine"
            endpoints = [{ wire = "messages", url = "http://127.0.0.1:9/v1/messages" }]
            "#,
        )
        .unwrap();

        let anthropic = catalog.get("anthropic").unwrap();
        assert_eq!(anthropic.label, "Mine");
        assert_eq!(
            anthropic.endpoint(Wire::Messages).unwrap().url,
            "http://127.0.0.1:9/v1/messages"
        );
        assert!(anthropic.models.is_empty());
        assert!(catalog.get("openai").is_some());
    }

    #[test]
    fn a_misspelled_key_is_refused_rather_than_ignored() {
        let error = Catalog::with_overlay(
            r#"
            [provider.anthropic]
            labl = "typo"
            endpoints = [{ wire = "messages", url = "https://example.test" }]
            "#,
        )
        .unwrap_err();

        assert!(matches!(error, CatalogError::Parse { .. }), "{error}");
        assert!(error.to_string().contains("config.toml"), "{error}");
    }

    #[test]
    fn a_provider_with_no_endpoint_is_refused() {
        let error = Catalog::with_overlay(
            r#"
            [provider.ghost]
            label = "Ghost"
            endpoints = []
            "#,
        )
        .unwrap_err();

        assert!(matches!(error, CatalogError::NoEndpoints { .. }), "{error}");
    }
}
