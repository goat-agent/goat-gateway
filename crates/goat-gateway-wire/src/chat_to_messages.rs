use serde_json::{Map, Value, json};

use crate::{
    mapping::{Mapping, TranslateError, Translated},
    sse::{Frame, Parser},
};

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

const WIRE: &str = "Anthropic Messages";

const CARRIED: &[&str] = &[
    "model",
    "messages",
    "tools",
    "tool_choice",
    "max_tokens",
    "max_completion_tokens",
    "temperature",
    "top_p",
    "stream",
    "stream_options",
    "stop",
    "reasoning_effort",
];

#[derive(Debug, Clone)]
pub struct Target {
    pub model: String,
    pub default_max_tokens: u32,
}

pub fn translate(input: &[u8], target: &Target) -> Result<Translated, TranslateError> {
    let source: Value = serde_json::from_slice(input)?;
    let mut mapping = Mapping::default();
    let mut out = Map::new();

    out.insert("model".into(), json!(target.model));
    mapping.moved();

    let mut system = Vec::new();
    let mut messages: Vec<Value> = Vec::new();

    for (index, message) in source
        .get("messages")
        .and_then(Value::as_array)
        .map_or(&[][..], Vec::as_slice)
        .iter()
        .enumerate()
    {
        carry_back(message, index, &mut system, &mut messages, &mut mapping)?;
    }

    if !system.is_empty() {
        out.insert("system".into(), Value::Array(system));
    }
    out.insert("messages".into(), Value::Array(messages));

    if let Some(tools) = source.get("tools").and_then(Value::as_array) {
        let declared: Result<Vec<Value>, TranslateError> = tools.iter().map(as_tool).collect();
        out.insert("tools".into(), Value::Array(declared?));
        mapping.moved();
    }
    if let Some(choice) = source.get("tool_choice") {
        out.insert("tool_choice".into(), as_anthropic_choice(choice)?);
        mapping.moved();
    }

    let ceiling = source
        .get("max_tokens")
        .or_else(|| source.get("max_completion_tokens"))
        .and_then(Value::as_u64);
    out.insert(
        "max_tokens".into(),
        json!(ceiling.unwrap_or(u64::from(target.default_max_tokens))),
    );
    if ceiling.is_some() {
        mapping.moved();
    } else {
        mapping.added(
            "/max_tokens",
            "the Messages format requires a ceiling and the request named none",
        );
    }

    for field in ["temperature", "top_p", "stream"] {
        if let Some(value) = source.get(field) {
            out.insert(field.into(), value.clone());
            mapping.moved();
        }
    }
    if let Some(stop) = source.get("stop") {
        out.insert(
            "stop_sequences".into(),
            match stop {
                Value::String(one) => json!([one]),
                other => other.clone(),
            },
        );
        mapping.moved();
    }
    if source.get("stream_options").is_some() {
        mapping.dropped(
            "/stream_options",
            "the Messages format always reports usage, so there is nothing to ask for",
        );
    }
    if source.get("reasoning_effort").is_some() {
        mapping.dropped(
            "/reasoning_effort",
            "how hard a model thinks is asked for as a token budget here, which this request did not give",
        );
    }

    mapping.mind_the_rest(&source, CARRIED, WIRE);

    Ok(Translated {
        body: serde_json::to_vec(&Value::Object(out))?,
        mapping,
    })
}

fn carry_back(
    message: &Value,
    index: usize,
    system: &mut Vec<Value>,
    out: &mut Vec<Value>,
    mapping: &mut Mapping,
) -> Result<(), TranslateError> {
    let role = message
        .get("role")
        .and_then(Value::as_str)
        .unwrap_or("user");
    let at = format!("/messages/{index}");

    if role == "system" || role == "developer" {
        for block in spoken_blocks(message.get("content").unwrap_or(&Value::Null)) {
            system.push(block);
        }
        mapping.moved();
        return Ok(());
    }

    if role == "tool" {
        let result = json!({
            "type": "tool_result",
            "tool_use_id": message.get("tool_call_id").cloned().unwrap_or(Value::Null),
            "content": joined(message.get("content").unwrap_or(&Value::Null)),
        });
        mapping.moved();
        if let Some(last) = out.last_mut()
            && last.get("role").and_then(Value::as_str) == Some("user")
            && let Some(blocks) = last.get_mut("content").and_then(Value::as_array_mut)
        {
            blocks.insert(0, result);
            return Ok(());
        }
        out.push(json!({ "role": "user", "content": [result] }));
        return Ok(());
    }

    let mut blocks = spoken_blocks(message.get("content").unwrap_or(&Value::Null));
    if !blocks.is_empty() {
        mapping.moved();
    }

    for (position, call) in message
        .get("tool_calls")
        .and_then(Value::as_array)
        .map_or(&[][..], Vec::as_slice)
        .iter()
        .enumerate()
    {
        let function = call.get("function").unwrap_or(&Value::Null);
        let arguments = function
            .get("arguments")
            .and_then(Value::as_str)
            .unwrap_or("{}");
        blocks.push(json!({
            "type": "tool_use",
            "id": call.get("id").cloned().unwrap_or(Value::Null),
            "name": function.get("name").cloned().unwrap_or(Value::Null),
            "input": serde_json::from_str::<Value>(arguments).map_err(|error| {
                TranslateError::Malformed {
                    what: format!("{at}/tool_calls/{position}/function/arguments"),
                    detail: format!("a tool call carries its arguments as JSON text: {error}"),
                }
            })?,
        }));
        mapping.moved();
    }

    if blocks.is_empty() {
        return Ok(());
    }
    out.push(json!({ "role": role, "content": blocks }));
    Ok(())
}

fn spoken_blocks(content: &Value) -> Vec<Value> {
    match content {
        Value::String(text) if !text.is_empty() => {
            vec![json!({ "type": "text", "text": text })]
        }
        Value::Array(parts) => parts.iter().filter_map(as_block).collect(),
        _ => Vec::new(),
    }
}

fn as_block(part: &Value) -> Option<Value> {
    match part.get("type").and_then(Value::as_str) {
        Some("text") => Some(json!({
            "type": "text",
            "text": part.get("text").and_then(Value::as_str).unwrap_or_default(),
        })),
        Some("image_url") => {
            let url = part.get("image_url")?.get("url")?.as_str()?;
            let (media, data) = url
                .strip_prefix("data:")?
                .split_once(";base64,")
                .unwrap_or(("image/png", url));
            Some(json!({
                "type": "image",
                "source": { "type": "base64", "media_type": media, "data": data },
            }))
        }
        _ => None,
    }
}

fn joined(content: &Value) -> String {
    match content {
        Value::String(text) => text.clone(),
        Value::Array(parts) => parts
            .iter()
            .filter_map(|part| part.get("text").and_then(Value::as_str))
            .collect::<Vec<_>>()
            .join("\n\n"),
        _ => String::new(),
    }
}

fn as_tool(tool: &Value) -> Result<Value, TranslateError> {
    let function = tool.get("function").unwrap_or(&Value::Null);
    let Some(name) = function.get("name").and_then(Value::as_str) else {
        return Err(TranslateError::NoCounterpart {
            what: format!(
                "a {:?} tool",
                tool.get("type")
                    .and_then(Value::as_str)
                    .unwrap_or("nameless")
            ),
            wire: WIRE,
        });
    };
    let mut declared = Map::new();
    declared.insert("name".into(), json!(name));
    if let Some(description) = function.get("description") {
        declared.insert("description".into(), description.clone());
    }
    declared.insert(
        "input_schema".into(),
        function
            .get("parameters")
            .cloned()
            .unwrap_or_else(|| json!({ "type": "object", "properties": {} })),
    );
    Ok(Value::Object(declared))
}

fn as_anthropic_choice(choice: &Value) -> Result<Value, TranslateError> {
    match choice {
        Value::String(named) => match named.as_str() {
            "auto" => Ok(json!({ "type": "auto" })),
            "required" => Ok(json!({ "type": "any" })),
            "none" => Ok(json!({ "type": "none" })),
            other => Err(TranslateError::NoCounterpart {
                what: format!("tool_choice {other:?}"),
                wire: WIRE,
            }),
        },
        Value::Object(_) => Ok(json!({
            "type": "tool",
            "name": choice
                .get("function")
                .and_then(|function| function.get("name"))
                .cloned()
                .unwrap_or(Value::Null),
        })),
        other => Err(TranslateError::NoCounterpart {
            what: format!("tool_choice {other}"),
            wire: WIRE,
        }),
    }
}

#[cfg(test)]
mod request_tests {
    use super::*;

    fn target() -> Target {
        Target {
            model: "claude-sonnet-5".into(),
            default_max_tokens: 32000,
        }
    }

    fn sent(request: Value) -> Value {
        let out = translate(&serde_json::to_vec(&request).unwrap(), &target()).unwrap();
        serde_json::from_slice(&out.body).unwrap()
    }

    #[test]
    fn a_system_message_becomes_the_system_prompt() {
        let out = sent(json!({
            "model": "gpt-5",
            "messages": [
                { "role": "system", "content": "Be terse." },
                { "role": "user", "content": "hi" },
            ],
        }));

        assert_eq!(out["model"], "claude-sonnet-5");
        assert_eq!(out["system"][0]["text"], "Be terse.");
        assert_eq!(out["messages"].as_array().unwrap().len(), 1);
        assert_eq!(out["messages"][0]["content"][0]["text"], "hi");
    }

    #[test]
    fn a_developer_message_is_a_system_prompt_by_another_name() {
        let out = sent(json!({
            "messages": [{ "role": "developer", "content": "Be terse." }],
        }));
        assert_eq!(out["system"][0]["text"], "Be terse.");
    }

    #[test]
    fn a_tool_call_and_its_result_come_back_as_blocks() {
        let out = sent(json!({
            "messages": [
                { "role": "assistant", "content": "on it", "tool_calls": [{
                    "id": "call_9",
                    "type": "function",
                    "function": { "name": "bash", "arguments": "{\"cmd\":\"ls\"}" },
                }]},
                { "role": "tool", "tool_call_id": "call_9", "content": "a.txt" },
            ],
        }));

        assert_eq!(out["messages"][0]["role"], "assistant");
        assert_eq!(out["messages"][0]["content"][1]["type"], "tool_use");
        assert_eq!(out["messages"][0]["content"][1]["id"], "call_9");
        assert_eq!(out["messages"][0]["content"][1]["input"]["cmd"], "ls");

        assert_eq!(out["messages"][1]["role"], "user");
        assert_eq!(out["messages"][1]["content"][0]["type"], "tool_result");
        assert_eq!(out["messages"][1]["content"][0]["tool_use_id"], "call_9");
    }

    #[test]
    fn two_results_for_one_turn_share_a_single_user_message() {
        let out = sent(json!({
            "messages": [
                { "role": "assistant", "tool_calls": [
                    { "id": "a", "function": { "name": "one", "arguments": "{}" } },
                    { "id": "b", "function": { "name": "two", "arguments": "{}" } },
                ]},
                { "role": "tool", "tool_call_id": "a", "content": "first" },
                { "role": "tool", "tool_call_id": "b", "content": "second" },
            ],
        }));

        let roles: Vec<&str> = out["messages"]
            .as_array()
            .unwrap()
            .iter()
            .map(|message| message["role"].as_str().unwrap())
            .collect();
        assert_eq!(
            roles,
            ["assistant", "user"],
            "the Messages format wants every result for a turn in one message"
        );
        assert_eq!(out["messages"][1]["content"].as_array().unwrap().len(), 2);
    }

    #[test]
    fn a_ceiling_is_required_so_one_is_supplied_and_said_so() {
        let out = translate(
            &serde_json::to_vec(&json!({ "messages": [] })).unwrap(),
            &target(),
        )
        .unwrap();
        let body: Value = serde_json::from_slice(&out.body).unwrap();

        assert_eq!(body["max_tokens"], 32000);
        assert!(
            out.mapping
                .added
                .iter()
                .any(|note| note.pointer == "/max_tokens")
        );
    }

    #[test]
    fn max_completion_tokens_counts_as_a_ceiling() {
        let out = sent(json!({ "messages": [], "max_completion_tokens": 512 }));
        assert_eq!(out["max_tokens"], 512);
    }

    #[test]
    fn a_tool_declaration_comes_back_out_of_its_function_wrapper() {
        let out = sent(json!({
            "messages": [],
            "tools": [{
                "type": "function",
                "function": {
                    "name": "bash",
                    "description": "run it",
                    "parameters": { "type": "object", "properties": {} },
                },
            }],
            "tool_choice": "required",
        }));

        assert_eq!(out["tools"][0]["name"], "bash");
        assert_eq!(out["tools"][0]["input_schema"]["type"], "object");
        assert!(out["tools"][0].get("function").is_none());
        assert_eq!(out["tool_choice"]["type"], "any");
    }

    #[test]
    fn naming_one_tool_picks_that_tool() {
        let out = sent(json!({
            "messages": [],
            "tool_choice": { "type": "function", "function": { "name": "bash" } },
        }));
        assert_eq!(
            out["tool_choice"],
            json!({ "type": "tool", "name": "bash" })
        );
    }

    #[test]
    fn a_single_stop_string_becomes_a_list_of_one() {
        assert_eq!(
            sent(json!({ "messages": [], "stop": "\n\n" }))["stop_sequences"],
            json!(["\n\n"])
        );
        assert_eq!(
            sent(json!({ "messages": [], "stop": ["a", "b"] }))["stop_sequences"],
            json!(["a", "b"])
        );
    }

    #[test]
    fn an_image_comes_back_out_of_its_data_url() {
        let out = sent(json!({
            "messages": [{ "role": "user", "content": [
                { "type": "text", "text": "what is this" },
                { "type": "image_url", "image_url": { "url": "data:image/png;base64,AAAA" } },
            ]}],
        }));

        assert_eq!(out["messages"][0]["content"][1]["type"], "image");
        assert_eq!(
            out["messages"][0]["content"][1]["source"]["media_type"],
            "image/png"
        );
        assert_eq!(out["messages"][0]["content"][1]["source"]["data"], "AAAA");
    }

    #[test]
    fn arguments_that_are_not_json_stop_the_request() {
        let error = translate(
            &serde_json::to_vec(&json!({
                "messages": [{ "role": "assistant", "tool_calls": [
                    { "id": "a", "function": { "name": "bash", "arguments": "not json" } },
                ]}],
            }))
            .unwrap(),
            &target(),
        )
        .unwrap_err();
        assert!(matches!(error, TranslateError::Malformed { .. }));
    }

    #[test]
    fn a_field_we_do_not_understand_is_written_down() {
        let out = translate(
            &serde_json::to_vec(&json!({
                "messages": [],
                "logit_bias": { "50256": -100 },
                "seed": 7,
            }))
            .unwrap(),
            &target(),
        )
        .unwrap();

        let named: Vec<&str> = out
            .mapping
            .dropped
            .iter()
            .map(|note| note.pointer.as_str())
            .collect();
        assert!(named.contains(&"/logit_bias"), "{named:?}");
        assert!(named.contains(&"/seed"), "{named:?}");
    }
}
