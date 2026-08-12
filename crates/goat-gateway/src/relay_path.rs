use goat_gateway_wire::{
    Envelopes, Mapping, TranslateError, Translated, chat_to_messages, messages_to_chat,
    messages_to_responses, responses_to_messages,
};

use crate::provider::Wire;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Hop {
    ResponsesToMessages,
    MessagesToChat,
    ChatToMessages,
}

pub fn path(from: Wire, to: Wire) -> Option<&'static [Hop]> {
    match (from, to) {
        (Wire::Responses, Wire::Messages) => Some(&[Hop::ResponsesToMessages]),
        (Wire::Messages, Wire::Chat) => Some(&[Hop::MessagesToChat]),
        (Wire::Chat, Wire::Messages) => Some(&[Hop::ChatToMessages]),
        (Wire::Responses, Wire::Chat) => Some(&[Hop::ResponsesToMessages, Hop::MessagesToChat]),
        _ => None,
    }
}

pub struct Ask<'a> {
    pub model: &'a str,
    pub reasoning: Option<responses_to_messages::Target>,
    pub default_max_tokens: u32,
    pub envelopes: &'a Envelopes,
}

pub fn needs_reasoning_declared(hops: &[Hop]) -> bool {
    hops.contains(&Hop::ResponsesToMessages)
}

pub fn carry(body: &[u8], hops: &[Hop], ask: &Ask<'_>) -> Result<Translated, TranslateError> {
    let mut carried = body.to_vec();
    let mut mapping = Mapping::default();

    for hop in hops {
        let step = match hop {
            Hop::ResponsesToMessages => {
                let Some(reasoning) = &ask.reasoning else {
                    return Err(TranslateError::NoCounterpart {
                        what: format!("model {:?}", ask.model),
                        wire: "Anthropic Messages",
                    });
                };
                responses_to_messages::translate(&carried, reasoning, ask.envelopes)?
            }
            Hop::MessagesToChat => messages_to_chat::translate(&carried, ask.model)?,
            Hop::ChatToMessages => chat_to_messages::translate(
                &carried,
                &chat_to_messages::Target {
                    model: ask.model.to_owned(),
                    default_max_tokens: ask.default_max_tokens,
                },
            )?,
        };
        carried = step.body;
        mapping.absorb(step.mapping);
    }

    Ok(Translated {
        body: carried,
        mapping,
    })
}

pub enum Back {
    ChatToMessages(chat_to_messages::StreamTranslator),
    MessagesToChat(messages_to_chat::StreamTranslator),
    MessagesToResponses(Box<messages_to_responses::StreamTranslator>),
    ChatToResponses(
        chat_to_messages::StreamTranslator,
        Box<messages_to_responses::StreamTranslator>,
    ),
}

impl Back {
    pub fn assembled(&self) -> Option<serde_json::Value> {
        match self {
            Self::ChatToMessages(one) => Some(one.assembled()),
            Self::MessagesToChat(one) => Some(one.assembled()),
            Self::MessagesToResponses(_) | Self::ChatToResponses(..) => None,
        }
    }

    pub fn push(&mut self, chunk: &[u8]) -> Vec<u8> {
        match self {
            Self::ChatToMessages(one) => one.push(chunk),
            Self::MessagesToChat(one) => one.push(chunk),
            Self::MessagesToResponses(one) => one.push(chunk),
            Self::ChatToResponses(first, then) => {
                let middle = first.push(chunk);
                then.push(&middle)
            }
        }
    }

    pub fn finish(&mut self) -> Vec<u8> {
        match self {
            Self::ChatToMessages(one) => one.finish(),
            Self::MessagesToChat(one) => one.finish(),
            Self::MessagesToResponses(one) => one.finish(),
            Self::ChatToResponses(first, then) => {
                let middle = first.finish();
                let mut out = then.push(&middle);
                out.extend(then.finish());
                out
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_provider_this_gateway_ships_can_be_reached_from_every_door() {
        for from in [Wire::Messages, Wire::Responses, Wire::Chat] {
            for serves in [Wire::Messages, Wire::Chat] {
                assert!(
                    from == serves || path(from, serves).is_some(),
                    "a {from} request has no way to reach a provider that serves {serves}"
                );
            }
        }
    }

    #[test]
    fn a_responses_only_provider_is_reached_by_speaking_responses() {
        assert_eq!(path(Wire::Chat, Wire::Responses), None);
        assert_eq!(path(Wire::Messages, Wire::Responses), None);
    }

    #[test]
    fn a_pair_with_no_direct_translator_goes_through_messages() {
        assert_eq!(
            path(Wire::Responses, Wire::Chat),
            Some(&[Hop::ResponsesToMessages, Hop::MessagesToChat][..])
        );
    }

    #[test]
    fn a_format_that_is_already_right_needs_no_hops() {
        assert_eq!(path(Wire::Messages, Wire::Messages), None);
    }
}
