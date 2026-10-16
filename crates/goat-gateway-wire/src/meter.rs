use serde_json::Value;

use crate::sse::Parser;

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct Usage {
    pub input: Option<i64>,
    pub output: Option<i64>,
    pub cache_read: Option<i64>,
    pub cache_write: Option<i64>,
    pub reasoning: Option<i64>,
}

impl Usage {
    pub fn is_empty(&self) -> bool {
        *self == Self::default()
    }

    fn absorb(&mut self, usage: &Value, format: Format) {
        match format {
            Format::Messages => self.absorb_counted_apart(usage),
            Format::Responses => self.absorb_counted_with_cache(
                usage,
                "input_tokens",
                "/input_tokens_details/cached_tokens",
                "output_tokens",
                "/output_tokens_details/reasoning_tokens",
            ),
            Format::Chat => self.absorb_counted_with_cache(
                usage,
                "prompt_tokens",
                "/prompt_tokens_details/cached_tokens",
                "completion_tokens",
                "/completion_tokens_details/reasoning_tokens",
            ),
        }
    }

    fn absorb_counted_apart(&mut self, usage: &Value) {
        let read = |key: &str| usage.get(key).and_then(Value::as_i64);
        keep(&mut self.input, read("input_tokens"));
        keep(&mut self.output, read("output_tokens"));
        keep(&mut self.cache_read, read("cache_read_input_tokens"));
        keep(&mut self.cache_write, read("cache_creation_input_tokens"));
        keep(
            &mut self.reasoning,
            usage
                .pointer("/output_tokens_details/thinking_tokens")
                .and_then(Value::as_i64),
        );
    }

    fn absorb_counted_with_cache(
        &mut self,
        usage: &Value,
        prompt: &str,
        cached: &str,
        output: &str,
        reasoning: &str,
    ) {
        let cached = usage.pointer(cached).and_then(Value::as_i64);
        keep(&mut self.cache_read, cached);
        keep(
            &mut self.input,
            usage
                .get(prompt)
                .and_then(Value::as_i64)
                .map(|prompt| (prompt - cached.unwrap_or(0)).max(0)),
        );
        keep(&mut self.output, usage.get(output).and_then(Value::as_i64));
        keep(
            &mut self.reasoning,
            usage.pointer(reasoning).and_then(Value::as_i64),
        );
    }
}

#[derive(Debug, Clone, Copy)]
enum Format {
    Messages,
    Responses,
    Chat,
}

impl Format {
    fn of_whole(answer: &Value) -> Self {
        match answer.get("object").and_then(Value::as_str) {
            Some("response") => Self::Responses,
            Some(object) if object.starts_with("chat.completion") => Self::Chat,
            _ if answer.get("choices").is_some() => Self::Chat,
            _ => Self::Messages,
        }
    }
}

fn keep(slot: &mut Option<i64>, seen: Option<i64>) {
    if let Some(seen) = seen {
        *slot = Some(seen);
    }
}

const LONGEST_WHOLE_ANSWER: usize = 8 * 1024 * 1024;

#[derive(Debug, Default)]
pub struct Meter {
    parser: Parser,
    usage: Usage,
    failure: Option<String>,
    opened: bool,
    closed: bool,
    framed: bool,
    whole: Vec<u8>,
}

impl Meter {
    pub fn observe(&mut self, chunk: &[u8]) {
        if !self.framed && self.whole.len() + chunk.len() <= LONGEST_WHOLE_ANSWER {
            self.whole.extend_from_slice(chunk);
        }
        for frame in self.parser.push(chunk) {
            if !self.framed {
                self.framed = true;
                self.whole = Vec::new();
            }
            if frame.data.trim() == "[DONE]" {
                self.closed = true;
                continue;
            }
            let Some(data) = frame.json() else { continue };
            self.heard(&data);
        }
    }

    fn heard(&mut self, data: &Value) {
        match data.get("type").and_then(Value::as_str) {
            Some("message_start") => {
                self.opened = true;
                if let Some(usage) = data.pointer("/message/usage") {
                    self.usage.absorb(usage, Format::Messages);
                }
            }
            Some("message_delta") => {
                if let Some(usage) = data.get("usage") {
                    self.usage.absorb(usage, Format::Messages);
                }
            }
            Some("message_stop") => self.closed = true,
            Some("response.created") => self.opened = true,
            Some("response.completed" | "response.incomplete") => {
                self.closed = true;
                if let Some(usage) = data.pointer("/response/usage") {
                    self.usage.absorb(usage, Format::Responses);
                }
            }
            Some("response.failed") => {
                self.closed = true;
                if let Some(usage) = data.pointer("/response/usage") {
                    self.usage.absorb(usage, Format::Responses);
                }
                self.failure = Some(
                    data.pointer("/response/error/code")
                        .and_then(Value::as_str)
                        .unwrap_or("failed")
                        .to_owned(),
                );
            }
            Some("error") => {
                self.failure = Some(
                    data.pointer("/error/type")
                        .or_else(|| data.get("code"))
                        .and_then(Value::as_str)
                        .unwrap_or("error")
                        .to_owned(),
                );
            }
            Some(_) => {}
            None => self.heard_a_chat_chunk(data),
        }
    }

    fn heard_a_chat_chunk(&mut self, data: &Value) {
        if let Some(error) = data.get("error") {
            self.failure = Some(
                error
                    .get("type")
                    .or_else(|| error.get("code"))
                    .and_then(Value::as_str)
                    .unwrap_or("error")
                    .to_owned(),
            );
            return;
        }
        if let Some(choices) = data.get("choices").and_then(Value::as_array) {
            self.opened = true;
            if choices.iter().any(|choice| {
                choice
                    .get("finish_reason")
                    .is_some_and(|reason| !reason.is_null())
            }) {
                self.closed = true;
            }
        }
        if let Some(usage) = data.get("usage").filter(|usage| !usage.is_null()) {
            self.usage.absorb(usage, Format::Chat);
        }
    }

    fn answered_whole(&self) -> Option<Value> {
        if self.framed || self.whole.is_empty() {
            return None;
        }
        serde_json::from_slice(&self.whole).ok()
    }

    pub fn usage(&self) -> Usage {
        let mut usage = self.usage;
        if let Some(answer) = self.answered_whole()
            && let Some(counted) = answer.get("usage")
        {
            usage.absorb(counted, Format::of_whole(&answer));
        }
        usage
    }

    pub fn failure(&self) -> Option<&str> {
        self.failure
            .as_deref()
            .or((self.opened && !self.closed).then_some("cut_short"))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn drive(chunks: &[&str]) -> Meter {
        let mut meter = Meter::default();
        for chunk in chunks {
            meter.observe(chunk.as_bytes());
        }
        meter
    }

    #[test]
    fn input_arrives_at_the_start_and_output_at_the_end() {
        let meter = drive(&[
            "event: message_start\ndata: {\"type\":\"message_start\",\"message\":{\"usage\":{\"input_tokens\":10,\"cache_read_input_tokens\":200}}}\n\n",
            "event: message_delta\ndata: {\"type\":\"message_delta\",\"usage\":{\"output_tokens\":8}}\n\n",
        ]);

        assert_eq!(
            meter.usage(),
            Usage {
                input: Some(10),
                output: Some(8),
                cache_read: Some(200),
                cache_write: None,
                reasoning: None,
            }
        );
    }

    #[test]
    fn a_frame_split_across_chunks_still_counts() {
        let whole = drive(&[
            "event: message_delta\ndata: {\"type\":\"message_delta\",\"usage\":{\"output_tokens\":8}}\n\n",
        ]);
        let split = drive(&[
            "event: message_delta\ndata: {\"type\":\"message_delta\",\"usa",
            "ge\":{\"output_tokens\":8}}\n\n",
        ]);
        assert_eq!(whole.usage(), split.usage());
    }

    #[test]
    fn a_stream_that_dies_after_a_success_status_is_not_a_success() {
        let meter = drive(&[
            "event: message_start\ndata: {\"type\":\"message_start\",\"message\":{\"usage\":{\"input_tokens\":10}}}\n\n",
            "event: error\ndata: {\"type\":\"error\",\"error\":{\"type\":\"overloaded_error\",\"message\":\"Overloaded\"}}\n\n",
        ]);

        assert_eq!(meter.failure(), Some("overloaded_error"));
        assert_eq!(meter.usage().input, Some(10));
    }

    #[test]
    fn a_responses_stream_is_counted_in_the_same_terms() {
        let meter = drive(&[
            "event: response.created\ndata: {\"type\":\"response.created\",\"response\":{}}\n\n",
            "event: response.completed\ndata: {\"type\":\"response.completed\",\"response\":{\"usage\":{\"input_tokens\":1000,\"input_tokens_details\":{\"cached_tokens\":900},\"output_tokens\":50,\"output_tokens_details\":{\"reasoning_tokens\":20}}}}\n\n",
        ]);

        assert_eq!(
            meter.usage(),
            Usage {
                input: Some(100),
                output: Some(50),
                cache_read: Some(900),
                cache_write: None,
                reasoning: Some(20),
            },
            "input means what Anthropic means by it: the part not read from cache"
        );
        assert!(meter.failure().is_none());
    }

    #[test]
    fn a_chat_stream_is_counted_in_the_same_terms() {
        let meter = drive(&[
            "data: {\"choices\":[{\"index\":0,\"delta\":{\"content\":\"hi\"}}]}\n\n",
            "data: {\"choices\":[{\"index\":0,\"delta\":{},\"finish_reason\":\"stop\"}]}\n\n",
            "data: {\"choices\":[],\"usage\":{\"prompt_tokens\":1000,\"prompt_tokens_details\":{\"cached_tokens\":900},\"completion_tokens\":50}}\n\n",
            "data: [DONE]\n\n",
        ]);

        assert_eq!(meter.usage().input, Some(100));
        assert_eq!(meter.usage().cache_read, Some(900));
        assert_eq!(meter.usage().output, Some(50));
        assert!(meter.failure().is_none());
    }

    #[test]
    fn a_stream_cut_short_is_a_failure_in_every_format() {
        for (format, cut) in [
            (
                "messages",
                "event: message_start\ndata: {\"type\":\"message_start\",\"message\":{}}\n\n",
            ),
            (
                "responses",
                "event: response.created\ndata: {\"type\":\"response.created\",\"response\":{}}\n\n",
            ),
            (
                "chat",
                "data: {\"choices\":[{\"index\":0,\"delta\":{\"content\":\"Half\"}}]}\n\n",
            ),
        ] {
            assert!(
                drive(&[cut]).failure().is_some(),
                "a {format} stream that never said it was done is not a success"
            );
        }
    }

    #[test]
    fn an_answer_that_was_not_streamed_is_counted_too() {
        for (format, body) in [
            (
                "messages",
                r#"{"type":"message","usage":{"input_tokens":100,"cache_read_input_tokens":900,"output_tokens":50}}"#,
            ),
            (
                "responses",
                r#"{"object":"response","usage":{"input_tokens":1000,"input_tokens_details":{"cached_tokens":900},"output_tokens":50}}"#,
            ),
            (
                "chat",
                r#"{"object":"chat.completion","usage":{"prompt_tokens":1000,"prompt_tokens_details":{"cached_tokens":900},"completion_tokens":50}}"#,
            ),
        ] {
            let (head, tail) = body.split_at(body.len() / 2);
            let meter = drive(&[head, tail]);
            assert_eq!(meter.usage().input, Some(100), "{format}");
            assert_eq!(meter.usage().cache_read, Some(900), "{format}");
            assert_eq!(meter.usage().output, Some(50), "{format}");
            assert!(meter.failure().is_none(), "{format}");
        }
    }

    #[test]
    fn a_clean_stream_reports_no_failure() {
        let meter = drive(&["event: message_stop\ndata: {\"type\":\"message_stop\"}\n\n"]);
        assert!(meter.failure().is_none());
        assert!(meter.usage().is_empty());
    }
}
