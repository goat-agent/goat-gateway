use serde_json::{Value, json};

use crate::{
    envelope::{Envelopes, Payload, Portability, Provenance, Sealed},
    sse::{Frame, Parser},
};

#[derive(Debug, Clone)]
pub struct StreamTarget {
    pub response_id: String,
    pub model: String,
    pub provenance: Provenance,
    pub nonce_seed: [u8; 8],
}

pub struct StreamTranslator {
    target: StreamTarget,
    envelopes: Envelopes,
    parser: Parser,
    sequence: u64,
    output_index: u64,
    nonce_counter: u32,
    open: Option<OpenBlock>,
    items: Vec<Value>,
    usage: Option<Value>,
    stop_reason: Option<String>,
    started: bool,
    finished: bool,
}

enum OpenBlock {
    Reasoning {
        item_id: String,
        thinking: String,
        signature: String,
        redacted: Option<String>,
        summary_open: bool,
    },
    Text {
        item_id: String,
        text: String,
    },
    ToolCall {
        item_id: String,
        call_id: String,
        name: String,
        arguments: String,
    },
    Ignored,
}

impl StreamTranslator {
    pub fn new(target: StreamTarget, envelopes: Envelopes) -> Self {
        Self {
            target,
            envelopes,
            parser: Parser::default(),
            sequence: 0,
            output_index: 0,
            nonce_counter: 0,
            open: None,
            items: Vec::new(),
            usage: None,
            stop_reason: None,
            started: false,
            finished: false,
        }
    }

    pub fn push(&mut self, chunk: &[u8]) -> Vec<u8> {
        let frames = self.parser.push(chunk);
        let mut out = String::new();
        for frame in frames {
            self.handle(&frame, &mut out);
        }
        out.into_bytes()
    }

    pub fn finish(&mut self) -> Vec<u8> {
        if self.finished {
            return Vec::new();
        }
        let mut out = String::new();
        self.complete(&mut out, "incomplete");
        out.into_bytes()
    }

    fn emit(&mut self, out: &mut String, event: &str, mut data: Value) {
        data["type"] = json!(event);
        data["sequence_number"] = json!(self.sequence);
        self.sequence += 1;
        out.push_str(&Frame::encode(event, &data));
    }

    fn handle(&mut self, frame: &Frame, out: &mut String) {
        let Some(data) = frame.json() else { return };
        let kind = frame
            .event
            .as_deref()
            .or_else(|| data.get("type").and_then(Value::as_str))
            .unwrap_or_default()
            .to_owned();

        match kind.as_str() {
            "message_start" => self.start(out),
            "content_block_start" => self.block_start(&data, out),
            "content_block_delta" => self.block_delta(&data, out),
            "content_block_stop" => self.block_stop(out),
            "message_delta" => {
                if let Some(reason) = data.pointer("/delta/stop_reason").and_then(Value::as_str) {
                    self.stop_reason = Some(reason.to_owned());
                }
                if let Some(usage) = data.get("usage") {
                    self.usage = Some(usage.clone());
                }
            }
            "message_stop" => self.complete(out, "completed"),
            "error" => {
                let message = data
                    .pointer("/error/message")
                    .and_then(Value::as_str)
                    .unwrap_or("upstream error")
                    .to_owned();
                let kind = data
                    .pointer("/error/type")
                    .and_then(Value::as_str)
                    .unwrap_or("api_error")
                    .to_owned();
                self.emit(
                    out,
                    "error",
                    json!({ "code": kind, "message": message, "param": Value::Null }),
                );
            }
            _ => {}
        }
    }

    fn start(&mut self, out: &mut String) {
        if self.started {
            return;
        }
        self.started = true;
        let skeleton = self.response(json!("in_progress"), Vec::new(), None);
        self.emit(out, "response.created", json!({ "response": skeleton }));
        let skeleton = self.response(json!("in_progress"), Vec::new(), None);
        self.emit(out, "response.in_progress", json!({ "response": skeleton }));
    }

    fn block_start(&mut self, data: &Value, out: &mut String) {
        self.start(out);
        let block = data.get("content_block").cloned().unwrap_or(Value::Null);
        let kind = block
            .get("type")
            .and_then(Value::as_str)
            .unwrap_or_default();
        let item_id = format!("{}_{}", self.target.response_id, self.output_index);

        match kind {
            "thinking" | "redacted_thinking" => {
                let item = json!({
                    "type": "reasoning",
                    "id": item_id,
                    "summary": [],
                });
                let index = self.output_index;
                self.emit(
                    out,
                    "response.output_item.added",
                    json!({ "output_index": index, "item": item }),
                );
                self.open = Some(OpenBlock::Reasoning {
                    item_id,
                    thinking: String::new(),
                    signature: String::new(),
                    redacted: block.get("data").and_then(Value::as_str).map(str::to_owned),
                    summary_open: false,
                });
            }
            "text" => {
                let item = json!({
                    "type": "message",
                    "id": item_id,
                    "status": "in_progress",
                    "role": "assistant",
                    "content": [],
                });
                let index = self.output_index;
                self.emit(
                    out,
                    "response.output_item.added",
                    json!({ "output_index": index, "item": item }),
                );
                self.emit(
                    out,
                    "response.content_part.added",
                    json!({
                        "item_id": item_id,
                        "output_index": index,
                        "content_index": 0,
                        "part": { "type": "output_text", "text": "", "annotations": [] },
                    }),
                );
                self.open = Some(OpenBlock::Text {
                    item_id,
                    text: String::new(),
                });
            }
            "tool_use" => {
                let call_id = block
                    .get("id")
                    .and_then(Value::as_str)
                    .unwrap_or_default()
                    .to_owned();
                let name = block
                    .get("name")
                    .and_then(Value::as_str)
                    .unwrap_or_default()
                    .to_owned();
                let item = json!({
                    "type": "function_call",
                    "id": item_id,
                    "call_id": call_id,
                    "name": name,
                    "arguments": "",
                });
                let index = self.output_index;
                self.emit(
                    out,
                    "response.output_item.added",
                    json!({ "output_index": index, "item": item }),
                );
                self.open = Some(OpenBlock::ToolCall {
                    item_id,
                    call_id,
                    name,
                    arguments: String::new(),
                });
            }
            _ => self.open = Some(OpenBlock::Ignored),
        }
    }

    fn block_delta(&mut self, data: &Value, out: &mut String) {
        let Some(delta) = data.get("delta") else {
            return;
        };
        let kind = delta
            .get("type")
            .and_then(Value::as_str)
            .unwrap_or_default();

        let (event, payload, index) = match (&mut self.open, kind) {
            (
                Some(OpenBlock::Reasoning {
                    item_id,
                    thinking,
                    summary_open,
                    ..
                }),
                "thinking_delta",
            ) => {
                let text = delta.get("thinking").and_then(Value::as_str).unwrap_or("");
                thinking.push_str(text);
                let item_id = item_id.clone();
                let first = !*summary_open;
                *summary_open = true;
                let index = self.output_index;
                if first {
                    self.emit(
                        out,
                        "response.reasoning_summary_part.added",
                        json!({
                            "item_id": item_id,
                            "output_index": index,
                            "summary_index": 0,
                            "part": { "type": "summary_text", "text": "" },
                        }),
                    );
                }
                (
                    "response.reasoning_summary_text.delta",
                    json!({
                        "item_id": item_id,
                        "output_index": index,
                        "summary_index": 0,
                        "delta": text,
                    }),
                    index,
                )
            }
            (Some(OpenBlock::Reasoning { signature, .. }), "signature_delta") => {
                signature.push_str(delta.get("signature").and_then(Value::as_str).unwrap_or(""));
                return;
            }
            (Some(OpenBlock::Text { item_id, text }), "text_delta") => {
                let chunk = delta.get("text").and_then(Value::as_str).unwrap_or("");
                text.push_str(chunk);
                let item_id = item_id.clone();
                let index = self.output_index;
                (
                    "response.output_text.delta",
                    json!({
                        "item_id": item_id,
                        "output_index": index,
                        "content_index": 0,
                        "delta": chunk,
                    }),
                    index,
                )
            }
            (
                Some(OpenBlock::ToolCall {
                    item_id, arguments, ..
                }),
                "input_json_delta",
            ) => {
                let chunk = delta
                    .get("partial_json")
                    .and_then(Value::as_str)
                    .unwrap_or("");
                arguments.push_str(chunk);
                let item_id = item_id.clone();
                let index = self.output_index;
                (
                    "response.function_call_arguments.delta",
                    json!({
                        "item_id": item_id,
                        "output_index": index,
                        "delta": chunk,
                    }),
                    index,
                )
            }
            _ => return,
        };

        let _ = index;
        self.emit(out, event, payload);
    }

    fn block_stop(&mut self, out: &mut String) {
        let Some(open) = self.open.take() else { return };
        let index = self.output_index;

        match open {
            OpenBlock::Reasoning {
                item_id,
                thinking,
                signature,
                redacted,
                summary_open,
            } => {
                if summary_open {
                    let text = thinking.clone();
                    self.emit(
                        out,
                        "response.reasoning_summary_text.done",
                        json!({
                            "item_id": item_id,
                            "output_index": index,
                            "summary_index": 0,
                            "text": text,
                        }),
                    );
                    let text = thinking.clone();
                    self.emit(
                        out,
                        "response.reasoning_summary_part.done",
                        json!({
                            "item_id": item_id,
                            "output_index": index,
                            "summary_index": 0,
                            "part": { "type": "summary_text", "text": text },
                        }),
                    );
                }

                let payload = match redacted {
                    Some(data) => Payload::RedactedThinking { data },
                    None => Payload::Thinking {
                        thinking: thinking.clone(),
                        signature,
                    },
                };
                let nonce = self.next_nonce();
                let blob = self.envelopes.seal(
                    nonce,
                    &Sealed {
                        provenance: self.target.provenance.clone(),
                        portability: Portability::Account,
                        payload,
                    },
                );

                let summary = if summary_open {
                    json!([{ "type": "summary_text", "text": thinking }])
                } else {
                    json!([])
                };
                let item = json!({
                    "type": "reasoning",
                    "id": item_id,
                    "summary": summary,
                    "encrypted_content": blob,
                });
                self.items.push(item.clone());
                self.emit(
                    out,
                    "response.output_item.done",
                    json!({ "output_index": index, "item": item }),
                );
                self.output_index += 1;
            }

            OpenBlock::Text { item_id, text } => {
                let done = text.clone();
                self.emit(
                    out,
                    "response.output_text.done",
                    json!({
                        "item_id": item_id,
                        "output_index": index,
                        "content_index": 0,
                        "text": done,
                    }),
                );
                let done = text.clone();
                self.emit(
                    out,
                    "response.content_part.done",
                    json!({
                        "item_id": item_id,
                        "output_index": index,
                        "content_index": 0,
                        "part": { "type": "output_text", "text": done, "annotations": [] },
                    }),
                );
                let item = json!({
                    "type": "message",
                    "id": item_id,
                    "status": "completed",
                    "role": "assistant",
                    "content": [{ "type": "output_text", "text": text, "annotations": [] }],
                });
                self.items.push(item.clone());
                self.emit(
                    out,
                    "response.output_item.done",
                    json!({ "output_index": index, "item": item }),
                );
                self.output_index += 1;
            }

            OpenBlock::ToolCall {
                item_id,
                call_id,
                name,
                arguments,
            } => {
                let done = arguments.clone();
                self.emit(
                    out,
                    "response.function_call_arguments.done",
                    json!({
                        "item_id": item_id,
                        "output_index": index,
                        "arguments": done,
                    }),
                );
                let item = json!({
                    "type": "function_call",
                    "id": item_id,
                    "call_id": call_id,
                    "name": name,
                    "arguments": arguments,
                    "status": "completed",
                });
                self.items.push(item.clone());
                self.emit(
                    out,
                    "response.output_item.done",
                    json!({ "output_index": index, "item": item }),
                );
                self.output_index += 1;
            }

            OpenBlock::Ignored => {}
        }
    }

    fn complete(&mut self, out: &mut String, status: &str) {
        if self.finished {
            return;
        }
        self.finished = true;
        let items = self.items.clone();
        let usage = self.usage.clone().map(|usage| convert_usage(&usage));
        let response = self.response(json!(status), items, usage);
        let event = if status == "completed" {
            "response.completed"
        } else {
            "response.incomplete"
        };
        self.emit(out, event, json!({ "response": response }));
    }

    fn response(&self, status: Value, output: Vec<Value>, usage: Option<Value>) -> Value {
        let mut response = json!({
            "id": self.target.response_id,
            "object": "response",
            "status": status,
            "model": self.target.model,
            "output": output,
        });
        if let Some(usage) = usage {
            response["usage"] = usage;
        }
        if let Some(reason) = &self.stop_reason
            && reason == "max_tokens"
        {
            response["incomplete_details"] = json!({ "reason": "max_output_tokens" });
        }
        response
    }

    fn next_nonce(&mut self) -> [u8; 12] {
        let mut nonce = [0u8; 12];
        nonce[..8].copy_from_slice(&self.target.nonce_seed);
        nonce[8..].copy_from_slice(&self.nonce_counter.to_le_bytes());
        self.nonce_counter += 1;
        nonce
    }
}

fn convert_usage(usage: &Value) -> Value {
    let number = |key: &str| usage.get(key).and_then(Value::as_u64).unwrap_or(0);
    let input = number("input_tokens");
    let cache_read = number("cache_read_input_tokens");
    let cache_write = number("cache_creation_input_tokens");
    let output = number("output_tokens");
    let reasoning = usage
        .pointer("/output_tokens_details/thinking_tokens")
        .and_then(Value::as_u64)
        .unwrap_or(0);

    json!({
        "input_tokens": input + cache_read + cache_write,
        "input_tokens_details": {
            "cached_tokens": cache_read,
            "cache_write_tokens": cache_write,
        },
        "output_tokens": output,
        "output_tokens_details": { "reasoning_tokens": reasoning },
        "total_tokens": input + cache_read + cache_write + output,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::sse::Parser as SseParser;

    const SIGNATURE: &str = "EqQBCgIYAhIM1gbcDa9GJwZA2b3hGgxBdjrkzLoky3dl1pki";

    fn envelopes() -> Envelopes {
        Envelopes::new(&[5u8; 32])
    }

    fn provenance() -> Provenance {
        Provenance {
            provider: "anthropic".into(),
            account: "personal".into(),
            model: "claude-sonnet-5".into(),
        }
    }

    fn translator() -> StreamTranslator {
        StreamTranslator::new(
            StreamTarget {
                response_id: "resp_1".into(),
                model: "claude-sonnet-5".into(),
                provenance: provenance(),
                nonce_seed: [9u8; 8],
            },
            envelopes(),
        )
    }

    fn frames(bytes: &[u8]) -> Vec<(String, Value)> {
        let mut parser = SseParser::default();
        parser
            .push(bytes)
            .into_iter()
            .map(|frame| {
                let data = frame.json().unwrap();
                (frame.event.unwrap_or_default(), data)
            })
            .collect()
    }

    fn anthropic_stream() -> Vec<&'static str> {
        vec![
            r#"event: message_start
data: {"type":"message_start","message":{"id":"msg_1","role":"assistant","content":[],"model":"claude-sonnet-5"}}

"#,
            r#"event: content_block_start
data: {"type":"content_block_start","index":0,"content_block":{"type":"thinking","thinking":"","signature":""}}

"#,
            r#"event: content_block_delta
data: {"type":"content_block_delta","index":0,"delta":{"type":"thinking_delta","thinking":"weighing "}}

"#,
            r#"event: content_block_delta
data: {"type":"content_block_delta","index":0,"delta":{"type":"thinking_delta","thinking":"the options"}}

"#,
            r#"event: content_block_delta
data: {"type":"content_block_delta","index":0,"delta":{"type":"signature_delta","signature":"EqQBCgIYAhIM1gbcDa9GJwZA2b3hGgxBdjrkzLoky3dl1pki"}}

"#,
            r#"event: content_block_stop
data: {"type":"content_block_stop","index":0}

"#,
            r#"event: content_block_start
data: {"type":"content_block_start","index":1,"content_block":{"type":"text","text":""}}

"#,
            r#"event: content_block_delta
data: {"type":"content_block_delta","index":1,"delta":{"type":"text_delta","text":"Checking."}}

"#,
            r#"event: content_block_stop
data: {"type":"content_block_stop","index":1}

"#,
            r#"event: content_block_start
data: {"type":"content_block_start","index":2,"content_block":{"type":"tool_use","id":"toolu_1","name":"shell","input":{}}}

"#,
            r#"event: content_block_delta
data: {"type":"content_block_delta","index":2,"delta":{"type":"input_json_delta","partial_json":"{\"cmd\":"}}

"#,
            r#"event: content_block_delta
data: {"type":"content_block_delta","index":2,"delta":{"type":"input_json_delta","partial_json":"\"ls\"}"}}

"#,
            r#"event: content_block_stop
data: {"type":"content_block_stop","index":2}

"#,
            r#"event: message_delta
data: {"type":"message_delta","delta":{"stop_reason":"tool_use"},"usage":{"input_tokens":10,"cache_read_input_tokens":200,"cache_creation_input_tokens":5,"output_tokens":42,"output_tokens_details":{"thinking_tokens":30}}}

"#,
            r#"event: message_stop
data: {"type":"message_stop"}

"#,
        ]
    }

    fn run_all() -> Vec<(String, Value)> {
        let mut translator = translator();
        let mut out = Vec::new();
        for chunk in anthropic_stream() {
            out.extend(translator.push(chunk.as_bytes()));
        }
        frames(&out)
    }

    #[test]
    fn thinking_reaches_the_client_before_the_block_closes() {
        let mut translator = translator();
        let stream = anthropic_stream();

        let mut emitted = Vec::new();
        for chunk in &stream[..4] {
            emitted.extend(translator.push(chunk.as_bytes()));
        }

        let events: Vec<String> = frames(&emitted).into_iter().map(|(kind, _)| kind).collect();
        assert!(
            events.contains(&"response.reasoning_summary_text.delta".to_owned()),
            "thinking must stream live, not be buffered until the block closes: {events:?}"
        );

        let deltas: Vec<String> = frames(&emitted)
            .into_iter()
            .filter(|(kind, _)| kind == "response.reasoning_summary_text.delta")
            .map(|(_, data)| data["delta"].as_str().unwrap().to_owned())
            .collect();
        assert_eq!(deltas, vec!["weighing ", "the options"]);
    }

    #[test]
    fn the_event_sequence_is_what_a_responses_client_expects() {
        let events: Vec<String> = run_all().into_iter().map(|(kind, _)| kind).collect();
        assert_eq!(
            events,
            vec![
                "response.created",
                "response.in_progress",
                "response.output_item.added",
                "response.reasoning_summary_part.added",
                "response.reasoning_summary_text.delta",
                "response.reasoning_summary_text.delta",
                "response.reasoning_summary_text.done",
                "response.reasoning_summary_part.done",
                "response.output_item.done",
                "response.output_item.added",
                "response.content_part.added",
                "response.output_text.delta",
                "response.output_text.done",
                "response.content_part.done",
                "response.output_item.done",
                "response.output_item.added",
                "response.function_call_arguments.delta",
                "response.function_call_arguments.delta",
                "response.function_call_arguments.done",
                "response.output_item.done",
                "response.completed",
            ]
        );
    }

    #[test]
    fn sequence_numbers_are_dense_and_ordered() {
        let numbers: Vec<u64> = run_all()
            .into_iter()
            .map(|(_, data)| data["sequence_number"].as_u64().unwrap())
            .collect();
        assert_eq!(numbers, (0..numbers.len() as u64).collect::<Vec<_>>());
    }

    #[test]
    fn output_indexes_advance_once_per_item() {
        let indexes: Vec<u64> = run_all()
            .into_iter()
            .filter(|(kind, _)| kind == "response.output_item.done")
            .map(|(_, data)| data["output_index"].as_u64().unwrap())
            .collect();
        assert_eq!(indexes, vec![0, 1, 2]);
    }

    #[test]
    fn the_signature_is_recoverable_from_the_emitted_envelope() {
        let done = run_all()
            .into_iter()
            .find(|(kind, data)| {
                kind == "response.output_item.done" && data["item"]["type"] == "reasoning"
            })
            .unwrap()
            .1;

        let blob = done["item"]["encrypted_content"].as_str().unwrap();
        let sealed = envelopes().open(blob).unwrap();
        assert_eq!(
            sealed.payload,
            Payload::Thinking {
                thinking: "weighing the options".into(),
                signature: SIGNATURE.into(),
            }
        );
        assert_eq!(sealed.provenance, provenance());
    }

    #[test]
    fn a_full_round_trip_restores_the_signature_byte_for_byte() {
        let done = run_all()
            .into_iter()
            .find(|(kind, data)| {
                kind == "response.output_item.done" && data["item"]["type"] == "reasoning"
            })
            .unwrap()
            .1;
        let reasoning_item = done["item"].clone();

        let target = crate::responses_to_messages::Target {
            model: crate::responses_to_messages::TargetModel {
                name: "claude-sonnet-5".into(),
                thinking: crate::responses_to_messages::ThinkingStyle::Adaptive,
                default_max_tokens: 8192,
                mid_conversation_system: false,
            },
            provenance: provenance(),
            stream_thinking: true,
        };
        let next_turn = json!({
            "input": [
                { "type": "message", "role": "user", "content": "go on" },
                reasoning_item,
            ],
        });
        let translated = crate::responses_to_messages::translate(
            serde_json::to_vec(&next_turn).unwrap().as_slice(),
            &target,
            &envelopes(),
        )
        .unwrap();

        let body: Value = serde_json::from_slice(&translated.body).unwrap();
        let block = &body["messages"][1]["content"][0];
        assert_eq!(block["type"], "thinking");
        assert_eq!(block["thinking"], "weighing the options");
        assert_eq!(block["signature"], SIGNATURE);
        assert!(!translated.mapping.lost_anything());
    }

    #[test]
    fn usage_is_mapped_without_double_counting_cache() {
        let completed = run_all()
            .into_iter()
            .find(|(kind, _)| kind == "response.completed")
            .unwrap()
            .1;
        let usage = &completed["response"]["usage"];

        assert_eq!(usage["input_tokens"], 215);
        assert_eq!(usage["input_tokens_details"]["cached_tokens"], 200);
        assert_eq!(usage["input_tokens_details"]["cache_write_tokens"], 5);
        assert_eq!(usage["output_tokens"], 42);
        assert_eq!(usage["output_tokens_details"]["reasoning_tokens"], 30);
        assert_eq!(usage["total_tokens"], 257);
    }

    #[test]
    fn tool_arguments_arrive_whole() {
        let done = run_all()
            .into_iter()
            .find(|(kind, data)| {
                kind == "response.output_item.done" && data["item"]["type"] == "function_call"
            })
            .unwrap()
            .1;
        assert_eq!(done["item"]["arguments"], r#"{"cmd":"ls"}"#);
        assert_eq!(done["item"]["call_id"], "toolu_1");
        assert_eq!(done["item"]["name"], "shell");
    }

    #[test]
    fn a_mid_stream_error_is_relayed_not_swallowed() {
        let mut translator = translator();
        translator.push(anthropic_stream()[0].as_bytes());
        let out = translator.push(
            b"event: error\ndata: {\"type\":\"error\",\"error\":{\"type\":\"overloaded_error\",\"message\":\"Overloaded\"}}\n\n",
        );
        let events = frames(&out);
        assert_eq!(events[0].0, "error");
        assert_eq!(events[0].1["code"], "overloaded_error");
        assert_eq!(events[0].1["message"], "Overloaded");
    }

    #[test]
    fn a_stream_cut_short_still_closes_the_response() {
        let mut translator = translator();
        translator.push(anthropic_stream()[0].as_bytes());
        let tail = translator.finish();
        let events = frames(&tail);
        assert_eq!(events[0].0, "response.incomplete");
    }

    #[test]
    fn redacted_thinking_is_sealed_rather_than_dropped() {
        let mut translator = translator();
        translator.push(anthropic_stream()[0].as_bytes());
        let out = translator.push(
            b"event: content_block_start\ndata: {\"type\":\"content_block_start\",\"index\":0,\"content_block\":{\"type\":\"redacted_thinking\",\"data\":\"vendor-opaque\"}}\n\nevent: content_block_stop\ndata: {\"type\":\"content_block_stop\",\"index\":0}\n\n",
        );
        let done = frames(&out)
            .into_iter()
            .find(|(kind, _)| kind == "response.output_item.done")
            .unwrap()
            .1;
        let blob = done["item"]["encrypted_content"].as_str().unwrap();
        assert_eq!(
            envelopes().open(blob).unwrap().payload,
            Payload::RedactedThinking {
                data: "vendor-opaque".into()
            }
        );
    }

    #[test]
    fn chunk_boundaries_do_not_change_the_output() {
        let whole: String = anthropic_stream().concat();

        let mut one_shot = translator();
        let a = one_shot.push(whole.as_bytes());

        let mut byte_at_a_time = translator();
        let mut b = Vec::new();
        for byte in whole.as_bytes() {
            b.extend(byte_at_a_time.push(&[*byte]));
        }

        assert_eq!(
            a, b,
            "SSE framing must not depend on how the network split it"
        );
    }
}
