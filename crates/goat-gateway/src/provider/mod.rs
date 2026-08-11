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
    pub thinking: Thinking,
    pub max_tokens: u32,
    #[serde(default)]
    pub limit_scope: Option<String>,
    #[serde(default)]
    pub mid_conversation_system: bool,
    #[serde(default)]
    pub cache_min_tokens: Option<u32>,
    #[serde(default)]
    pub price: Option<Price>,
}

impl Model {
    pub fn target(&self) -> TargetModel {
        TargetModel {
            name: self.name.clone(),
            thinking: self.thinking.into(),
            default_max_tokens: self.max_tokens,
            mid_conversation_system: self.mid_conversation_system,
        }
    }
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
    #[serde(default)]
    pub models: Vec<Model>,
}

fn no_limits() -> Limits {
    Limits::None
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

        assert_eq!(haiku.thinking, Thinking::Budget);
        assert_eq!(sonnet.thinking, Thinking::Adaptive);
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
