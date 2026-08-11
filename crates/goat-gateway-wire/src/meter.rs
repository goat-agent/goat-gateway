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

    fn absorb(&mut self, usage: &Value) {
        let read = |key: &str| usage.get(key).and_then(Value::as_i64);
        let keep = |slot: &mut Option<i64>, seen: Option<i64>| {
            if let Some(seen) = seen {
                *slot = Some(seen);
            }
        };

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
}

#[derive(Debug, Default)]
pub struct Meter {
    parser: Parser,
    usage: Usage,
    failure: Option<String>,
}

impl Meter {
    pub fn observe(&mut self, chunk: &[u8]) {
        for frame in self.parser.push(chunk) {
            let Some(data) = frame.json() else { continue };
            match data.get("type").and_then(Value::as_str) {
                Some("message_start") => {
                    if let Some(usage) = data.pointer("/message/usage") {
                        self.usage.absorb(usage);
                    }
                }
                Some("message_delta") => {
                    if let Some(usage) = data.get("usage") {
                        self.usage.absorb(usage);
                    }
                }
                Some("error") => {
                    self.failure = Some(
                        data.pointer("/error/type")
                            .and_then(Value::as_str)
                            .unwrap_or("error")
                            .to_owned(),
                    );
                }
                _ => {}
            }
        }
    }

    pub fn usage(&self) -> Usage {
        self.usage
    }

    pub fn failure(&self) -> Option<&str> {
        self.failure.as_deref()
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
    fn a_clean_stream_reports_no_failure() {
        let meter = drive(&["event: message_stop\ndata: {\"type\":\"message_stop\"}\n\n"]);
        assert!(meter.failure().is_none());
        assert!(meter.usage().is_empty());
    }
}
