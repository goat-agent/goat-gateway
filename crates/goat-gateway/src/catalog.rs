use goat_gateway_wire::responses_to_messages::{TargetModel, ThinkingStyle};

#[derive(Debug, Clone)]
pub struct ModelRow {
    pub name: &'static str,
    pub thinking: ThinkingStyle,
    pub default_max_tokens: u32,
    pub mid_conversation_system: bool,
}

const ANTHROPIC: &[ModelRow] = &[
    ModelRow {
        name: "claude-opus-5",
        thinking: ThinkingStyle::Adaptive,
        default_max_tokens: 32_000,
        mid_conversation_system: true,
    },
    ModelRow {
        name: "claude-sonnet-5",
        thinking: ThinkingStyle::Adaptive,
        default_max_tokens: 32_000,
        mid_conversation_system: false,
    },
    ModelRow {
        name: "claude-fable-5",
        thinking: ThinkingStyle::Adaptive,
        default_max_tokens: 32_000,
        mid_conversation_system: true,
    },
    ModelRow {
        name: "claude-opus-4-8",
        thinking: ThinkingStyle::Adaptive,
        default_max_tokens: 32_000,
        mid_conversation_system: true,
    },
    ModelRow {
        name: "claude-haiku-4-5",
        thinking: ThinkingStyle::Budget,
        default_max_tokens: 8_192,
        mid_conversation_system: false,
    },
];

pub fn anthropic(model: &str) -> Option<TargetModel> {
    ANTHROPIC
        .iter()
        .find(|row| row.name == model)
        .map(|row| TargetModel {
            name: row.name.to_owned(),
            thinking: row.thinking,
            default_max_tokens: row.default_max_tokens,
            mid_conversation_system: row.mid_conversation_system,
        })
}

pub fn known_anthropic_models() -> Vec<&'static str> {
    ANTHROPIC.iter().map(|row| row.name).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn properties_are_declared_not_guessed_from_the_name() {
        assert_eq!(
            anthropic("claude-haiku-4-5").unwrap().thinking,
            ThinkingStyle::Budget
        );
        assert_eq!(
            anthropic("claude-sonnet-5").unwrap().thinking,
            ThinkingStyle::Adaptive
        );
        assert!(
            !anthropic("claude-sonnet-5")
                .unwrap()
                .mid_conversation_system
        );
        assert!(anthropic("claude-opus-5").unwrap().mid_conversation_system);
    }

    #[test]
    fn an_unknown_model_is_unknown_rather_than_assumed() {
        assert!(anthropic("claude-sonnet-6").is_none());
        assert!(anthropic("claudette-fast").is_none());
    }
}
