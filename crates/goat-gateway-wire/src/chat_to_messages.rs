use serde_json::{Value, json};

use crate::sse::{Frame, Parser};

#[derive(Debug, Clone)]
pub struct StreamTarget {
    pub message_id: String,
    pub model: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Kind {
    Thinking,
    Text,
    Tool,
}

#[derive(Debug)]
struct Open {
    index: usize,
    kind: Kind,
    slot: Option<usize>,
    id: Option<String>,
    name: Option<String>,
    written: String,
}

#[derive(Debug, Default, Clone, Copy)]
struct Counted {
    input: i64,
    output: i64,
    cache_read: i64,
    reasoning: i64,
}

pub struct StreamTranslator {
    parser: Parser,
    target: StreamTarget,
    started: bool,
    finished: bool,
    open: Option<Open>,
    next_index: usize,
    stop_reason: Option<&'static str>,
    usage: Counted,
    blocks: Vec<Value>,
}

impl StreamTranslator {
    pub fn new(target: StreamTarget) -> Self {
        Self {
            parser: Parser::default(),
            target,
            started: false,
            finished: false,
            open: None,
            next_index: 0,
            stop_reason: None,
            usage: Counted::default(),
            blocks: Vec::new(),
        }
    }

    pub fn assembled(&self) -> Value {
        json!({
            "id": self.target.message_id,
            "type": "message",
            "role": "assistant",
            "model": self.target.model,
            "content": self.blocks,
            "stop_reason": self.stop_reason.unwrap_or("end_turn"),
            "stop_sequence": Value::Null,
            "usage": {
                "input_tokens": self.usage.input,
                "output_tokens": self.usage.output,
                "cache_read_input_tokens": self.usage.cache_read,
            },
        })
    }

    pub fn push(&mut self, chunk: &[u8]) -> Vec<u8> {
        let mut out = String::new();
        for frame in self.parser.push(chunk) {
            self.take(&frame, &mut out);
        }
        out.into_bytes()
    }

    pub fn finish(&mut self) -> Vec<u8> {
        let mut out = String::new();
        self.close_open(&mut out);
        if self.started && !self.finished {
            self.finished = true;
            out.push_str(&Frame::encode(
                "message_delta",
                &json!({
                    "type": "message_delta",
                    "delta": {
                        "stop_reason": self.stop_reason.unwrap_or("end_turn"),
                        "stop_sequence": Value::Null,
                    },
                    "usage": { "output_tokens": self.usage.output },
                }),
            ));
            out.push_str(&Frame::encode(
                "message_stop",
                &json!({ "type": "message_stop" }),
            ));
        }
        out.into_bytes()
    }

    fn take(&mut self, frame: &Frame, out: &mut String) {
        if frame.data.trim() == "[DONE]" {
            return;
        }
        let Some(chunk) = frame.json() else { return };

        if let Some(error) = chunk.get("error") {
            self.relay_error(error, out);
            return;
        }

        self.start(out);
        self.count(&chunk);

        let choice = chunk
            .get("choices")
            .and_then(Value::as_array)
            .and_then(|choices| choices.first());
        let Some(choice) = choice else { return };

        if let Some(delta) = choice.get("delta") {
            self.spoken(delta, "reasoning_content", Kind::Thinking, out);
            self.spoken(delta, "content", Kind::Text, out);
            if let Some(calls) = delta.get("tool_calls").and_then(Value::as_array) {
                for call in calls {
                    self.called(call, out);
                }
            }
        }

        if let Some(reason) = choice.get("finish_reason").and_then(Value::as_str) {
            self.stop_reason = Some(match reason {
                "length" => "max_tokens",
                "tool_calls" | "function_call" => "tool_use",
                "content_filter" => "refusal",
                _ => "end_turn",
            });
            self.close_open(out);
        }
    }

    fn start(&mut self, out: &mut String) {
        if self.started {
            return;
        }
        self.started = true;
        out.push_str(&Frame::encode(
            "message_start",
            &json!({
                "type": "message_start",
                "message": {
                    "id": self.target.message_id,
                    "type": "message",
                    "role": "assistant",
                    "model": self.target.model,
                    "content": [],
                    "stop_reason": Value::Null,
                    "stop_sequence": Value::Null,
                    "usage": {
                        "input_tokens": self.usage.input,
                        "output_tokens": 0,
                        "cache_read_input_tokens": self.usage.cache_read,
                    },
                },
            }),
        ));
    }

    fn spoken(&mut self, delta: &Value, field: &str, kind: Kind, out: &mut String) {
        let Some(text) = delta.get(field).and_then(Value::as_str) else {
            return;
        };
        if text.is_empty() {
            return;
        }
        self.open_block(kind, None, out);
        let name = if kind == Kind::Thinking {
            "thinking_delta"
        } else {
            "text_delta"
        };
        let body = if kind == Kind::Thinking {
            json!({ "type": name, "thinking": text })
        } else {
            json!({ "type": name, "text": text })
        };
        self.delta(body, out);
    }

    fn called(&mut self, call: &Value, out: &mut String) {
        let slot = call
            .get("index")
            .and_then(Value::as_u64)
            .map(|index| index as usize)
            .unwrap_or(0);
        let function = call.get("function").unwrap_or(&Value::Null);

        let fresh =
            !matches!(&self.open, Some(open) if open.kind == Kind::Tool && open.slot == Some(slot));
        if fresh {
            self.close_open(out);
            let index = self.claim(Kind::Tool, Some(slot));
            if let Some(open) = &mut self.open {
                open.id = call.get("id").and_then(Value::as_str).map(str::to_owned);
                open.name = function
                    .get("name")
                    .and_then(Value::as_str)
                    .map(str::to_owned);
            }
            out.push_str(&Frame::encode(
                "content_block_start",
                &json!({
                    "type": "content_block_start",
                    "index": index,
                    "content_block": {
                        "type": "tool_use",
                        "id": call.get("id").cloned().unwrap_or(Value::Null),
                        "name": function.get("name").cloned().unwrap_or(Value::Null),
                        "input": {},
                    },
                }),
            ));
        }

        if let Some(fragment) = function.get("arguments").and_then(Value::as_str)
            && !fragment.is_empty()
        {
            self.delta(
                json!({ "type": "input_json_delta", "partial_json": fragment }),
                out,
            );
        }
    }

    fn open_block(&mut self, kind: Kind, slot: Option<usize>, out: &mut String) {
        if matches!(&self.open, Some(open) if open.kind == kind && open.slot == slot) {
            return;
        }
        self.close_open(out);
        let index = self.claim(kind, slot);
        let block = match kind {
            Kind::Thinking => json!({ "type": "thinking", "thinking": "" }),
            Kind::Text => json!({ "type": "text", "text": "" }),
            Kind::Tool => json!({ "type": "tool_use", "input": {} }),
        };
        out.push_str(&Frame::encode(
            "content_block_start",
            &json!({ "type": "content_block_start", "index": index, "content_block": block }),
        ));
    }

    fn claim(&mut self, kind: Kind, slot: Option<usize>) -> usize {
        let index = self.next_index;
        self.next_index += 1;
        self.open = Some(Open {
            index,
            kind,
            slot,
            id: None,
            name: None,
            written: String::new(),
        });
        index
    }

    fn delta(&mut self, body: Value, out: &mut String) {
        let Some(open) = &mut self.open else { return };
        for field in ["text", "thinking", "partial_json"] {
            if let Some(fragment) = body.get(field).and_then(Value::as_str) {
                open.written.push_str(fragment);
            }
        }
        out.push_str(&Frame::encode(
            "content_block_delta",
            &json!({ "type": "content_block_delta", "index": open.index, "delta": body }),
        ));
    }

    fn close_open(&mut self, out: &mut String) {
        let Some(open) = self.open.take() else { return };
        self.blocks.push(match open.kind {
            Kind::Thinking => json!({ "type": "thinking", "thinking": open.written }),
            Kind::Text => json!({ "type": "text", "text": open.written }),
            Kind::Tool => json!({
                "type": "tool_use",
                "id": open.id,
                "name": open.name,
                "input": serde_json::from_str::<Value>(&open.written).unwrap_or_else(|_| json!({})),
            }),
        });
        out.push_str(&Frame::encode(
            "content_block_stop",
            &json!({ "type": "content_block_stop", "index": open.index }),
        ));
    }

    fn count(&mut self, chunk: &Value) {
        let Some(usage) = chunk.get("usage").filter(|usage| !usage.is_null()) else {
            return;
        };
        let read = |name: &str| usage.get(name).and_then(Value::as_i64).unwrap_or(0);
        self.usage.output = read("completion_tokens").max(self.usage.output);
        self.usage.cache_read = usage
            .get("prompt_tokens_details")
            .and_then(|details| details.get("cached_tokens"))
            .and_then(Value::as_i64)
            .unwrap_or(self.usage.cache_read);
        self.usage.input = (read("prompt_tokens") - self.usage.cache_read).max(0);
        self.usage.reasoning = usage
            .get("completion_tokens_details")
            .and_then(|details| details.get("reasoning_tokens"))
            .and_then(Value::as_i64)
            .unwrap_or(self.usage.reasoning);
    }

    fn relay_error(&mut self, error: &Value, out: &mut String) {
        self.close_open(out);
        self.finished = true;
        out.push_str(&Frame::encode(
            "error",
            &json!({
                "type": "error",
                "error": {
                    "type": error.get("type").and_then(Value::as_str).unwrap_or("api_error"),
                    "message": error
                        .get("message")
                        .and_then(Value::as_str)
                        .unwrap_or("the provider stopped the stream without saying why"),
                },
            }),
        ));
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn target() -> StreamTarget {
        StreamTarget {
            message_id: "msg_1".into(),
            model: "kimi-for-coding".into(),
        }
    }

    fn chunk(delta: Value, finish: Option<&str>) -> String {
        format!(
            "data: {}\n\n",
            json!({
                "object": "chat.completion.chunk",
                "choices": [{ "index": 0, "delta": delta, "finish_reason": finish }],
            })
        )
    }

    fn run(chunks: &[String]) -> Vec<Frame> {
        let mut translator = StreamTranslator::new(target());
        let mut out = Vec::new();
        for chunk in chunks {
            out.extend(translator.push(chunk.as_bytes()));
        }
        out.extend(translator.finish());
        Parser::default().push(&out)
    }

    fn events(frames: &[Frame]) -> Vec<String> {
        frames
            .iter()
            .filter_map(|frame| frame.event.clone())
            .collect()
    }

    #[test]
    fn plain_text_becomes_one_block_from_start_to_stop() {
        let frames = run(&[
            chunk(json!({ "role": "assistant", "content": "" }), None),
            chunk(json!({ "content": "Here" }), None),
            chunk(json!({ "content": " you go." }), None),
            chunk(json!({}), Some("stop")),
        ]);

        assert_eq!(
            events(&frames),
            [
                "message_start",
                "content_block_start",
                "content_block_delta",
                "content_block_delta",
                "content_block_stop",
                "message_delta",
                "message_stop",
            ]
        );
        let spoken: String = frames
            .iter()
            .filter_map(|frame| frame.json())
            .filter_map(|value| value["delta"]["text"].as_str().map(str::to_owned))
            .collect();
        assert_eq!(spoken, "Here you go.");
    }

    #[test]
    fn thinking_arrives_as_thinking_and_never_grows_a_signature() {
        let frames = run(&[
            chunk(json!({ "reasoning_content": "weighing" }), None),
            chunk(json!({ "content": "yes" }), None),
            chunk(json!({}), Some("stop")),
        ]);

        let kinds: Vec<String> = frames
            .iter()
            .filter_map(|frame| frame.json())
            .filter_map(|value| value["content_block"]["type"].as_str().map(str::to_owned))
            .collect();
        assert_eq!(kinds, ["thinking", "text"]);

        let raw: String = frames.iter().map(|frame| frame.data.clone()).collect();
        assert!(
            !raw.contains("signature"),
            "a signature this provider never minted must not be invented"
        );
    }

    #[test]
    fn a_tool_call_arrives_as_one_block_with_its_arguments_streamed() {
        let frames = run(&[
            chunk(
                json!({ "tool_calls": [{ "index": 0, "id": "call_9", "function": { "name": "bash", "arguments": "" } }] }),
                None,
            ),
            chunk(
                json!({ "tool_calls": [{ "index": 0, "function": { "arguments": "{\"cmd\":" } }] }),
                None,
            ),
            chunk(
                json!({ "tool_calls": [{ "index": 0, "function": { "arguments": "\"ls\"}" } }] }),
                None,
            ),
            chunk(json!({}), Some("tool_calls")),
        ]);

        let started = frames
            .iter()
            .filter_map(|frame| frame.json())
            .find(|value| value["type"] == "content_block_start")
            .unwrap();
        assert_eq!(started["content_block"]["type"], "tool_use");
        assert_eq!(started["content_block"]["id"], "call_9");
        assert_eq!(started["content_block"]["name"], "bash");

        let arguments: String = frames
            .iter()
            .filter_map(|frame| frame.json())
            .filter_map(|value| value["delta"]["partial_json"].as_str().map(str::to_owned))
            .collect();
        assert_eq!(arguments, r#"{"cmd":"ls"}"#);

        let ended = frames
            .iter()
            .filter_map(|frame| frame.json())
            .find(|value| value["type"] == "message_delta")
            .unwrap();
        assert_eq!(ended["delta"]["stop_reason"], "tool_use");
    }

    #[test]
    fn two_tool_calls_get_two_blocks() {
        let frames = run(&[
            chunk(
                json!({ "tool_calls": [{ "index": 0, "id": "a", "function": { "name": "one", "arguments": "{}" } }] }),
                None,
            ),
            chunk(
                json!({ "tool_calls": [{ "index": 1, "id": "b", "function": { "name": "two", "arguments": "{}" } }] }),
                None,
            ),
            chunk(json!({}), Some("tool_calls")),
        ]);

        let ids: Vec<String> = frames
            .iter()
            .filter_map(|frame| frame.json())
            .filter_map(|value| value["content_block"]["id"].as_str().map(str::to_owned))
            .collect();
        assert_eq!(ids, ["a", "b"]);
    }

    #[test]
    fn usage_is_reported_without_counting_the_cache_twice() {
        let frames = run(&[
            chunk(json!({ "content": "hi" }), Some("stop")),
            format!(
                "data: {}\n\n",
                json!({
                    "choices": [],
                    "usage": {
                        "prompt_tokens": 1000,
                        "completion_tokens": 42,
                        "prompt_tokens_details": { "cached_tokens": 900 },
                    },
                })
            ),
            "data: [DONE]\n\n".to_owned(),
        ]);

        let ended = frames
            .iter()
            .filter_map(|frame| frame.json())
            .find(|value| value["type"] == "message_delta")
            .unwrap();
        assert_eq!(ended["usage"]["output_tokens"], 42);
    }

    #[test]
    fn a_stream_that_dies_says_so_rather_than_ending_quietly() {
        let frames = run(&[
            chunk(json!({ "content": "half" }), None),
            format!(
                "data: {}\n\n",
                json!({ "error": { "type": "server_error", "message": "upstream fell over" } })
            ),
        ]);

        let complaint = frames
            .iter()
            .filter_map(|frame| frame.json())
            .find(|value| value["type"] == "error")
            .expect("the client has to be told");
        assert_eq!(complaint["error"]["message"], "upstream fell over");
        assert!(!events(&frames).contains(&"message_stop".to_owned()));
    }

    #[test]
    fn a_client_that_did_not_ask_for_a_stream_gets_the_same_message_whole() {
        let mut translator = StreamTranslator::new(target());
        for piece in [
            chunk(json!({ "reasoning_content": "weighing" }), None),
            chunk(json!({ "content": "run it" }), None),
            chunk(
                json!({ "tool_calls": [{ "index": 0, "id": "call_9", "function": { "name": "bash", "arguments": "{\"cmd\":\"ls\"}" } }] }),
                None,
            ),
            chunk(json!({}), Some("tool_calls")),
        ] {
            translator.push(piece.as_bytes());
        }
        translator.finish();

        let whole = translator.assembled();
        assert_eq!(whole["role"], "assistant");
        assert_eq!(whole["stop_reason"], "tool_use");
        assert_eq!(whole["content"][0]["thinking"], "weighing");
        assert_eq!(whole["content"][1]["text"], "run it");
        assert_eq!(whole["content"][2]["type"], "tool_use");
        assert_eq!(whole["content"][2]["id"], "call_9");
        assert_eq!(whole["content"][2]["input"]["cmd"], "ls");
    }

    #[test]
    fn nothing_at_all_produces_nothing_at_all() {
        let mut translator = StreamTranslator::new(target());
        assert!(translator.push(b"").is_empty());
        assert!(translator.finish().is_empty());
    }
}
