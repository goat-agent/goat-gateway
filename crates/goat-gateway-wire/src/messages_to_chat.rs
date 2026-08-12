use serde_json::{Map, Value, json};

use crate::mapping::{Mapping, TranslateError, Translated};

const WIRE: &str = "Chat Completions";

pub fn translate(input: &[u8], model: &str) -> Result<Translated, TranslateError> {
    let source: Value = serde_json::from_slice(input)?;
    let mut mapping = Mapping::default();
    let mut out = Map::new();

    out.insert("model".into(), json!(model));
    mapping.moved();

    let mut messages = Vec::new();
    if let Some(system) = source.get("system") {
        let spoken = joined_text(system);
        if !spoken.is_empty() {
            messages.push(json!({ "role": "system", "content": spoken }));
            mapping.moved();
        }
    }

    for (index, message) in array(source.get("messages")).iter().enumerate() {
        carry(message, index, &mut messages, &mut mapping)?;
    }
    out.insert("messages".into(), Value::Array(messages));

    if let Some(tools) = source.get("tools").and_then(Value::as_array) {
        let declared: Result<Vec<Value>, TranslateError> = tools.iter().map(as_function).collect();
        out.insert("tools".into(), Value::Array(declared?));
        mapping.moved();
    }
    if let Some(choice) = source.get("tool_choice") {
        out.insert("tool_choice".into(), as_choice(choice)?);
        mapping.moved();
    }

    for (from, to) in [
        ("max_tokens", "max_tokens"),
        ("temperature", "temperature"),
        ("top_p", "top_p"),
        ("stream", "stream"),
    ] {
        if let Some(value) = source.get(from) {
            out.insert(to.into(), value.clone());
            mapping.moved();
        }
    }
    if let Some(stop) = source.get("stop_sequences") {
        out.insert("stop".into(), stop.clone());
        mapping.moved();
    }
    if source.get("top_k").is_some() {
        mapping.dropped("/top_k", format!("{WIRE} has no top_k; sent without it"));
    }
    if source.get("thinking").is_some() {
        mapping.dropped(
            "/thinking",
            "how much a model may think is asked for differently by every provider that serves this format",
        );
    }
    if source
        .get("stream")
        .and_then(Value::as_bool)
        .unwrap_or(false)
    {
        out.insert("stream_options".into(), json!({ "include_usage": true }));
        mapping.added(
            "/stream_options/include_usage",
            "without it the provider reports no token counts at all",
        );
    }

    mapping.mind_the_rest(&source, CARRIED, WIRE);

    Ok(Translated {
        body: serde_json::to_vec(&Value::Object(out))?,
        mapping,
    })
}

const CARRIED: &[&str] = &[
    "model",
    "system",
    "messages",
    "tools",
    "tool_choice",
    "max_tokens",
    "temperature",
    "top_p",
    "top_k",
    "stream",
    "stop_sequences",
    "thinking",
];

fn carry(
    message: &Value,
    index: usize,
    out: &mut Vec<Value>,
    mapping: &mut Mapping,
) -> Result<(), TranslateError> {
    let role = message
        .get("role")
        .and_then(Value::as_str)
        .unwrap_or("user");
    let at = |part: &str| format!("/messages/{index}{part}");

    let Some(blocks) = message.get("content").and_then(Value::as_array) else {
        out.push(json!({ "role": role, "content": message.get("content").cloned() }));
        mapping.moved();
        return Ok(());
    };

    let mut spoken = Vec::new();
    let mut calls = Vec::new();

    for (position, block) in blocks.iter().enumerate() {
        let where_ = at(&format!("/content/{position}"));
        match block
            .get("type")
            .and_then(Value::as_str)
            .unwrap_or_default()
        {
            "text" => {
                spoken.push(json!({ "type": "text", "text": text_of(block) }));
                mapping.moved();
            }
            "image" => {
                spoken.push(as_image(block, &where_)?);
                mapping.moved();
            }
            "tool_use" => {
                calls.push(json!({
                    "id": block.get("id").cloned().unwrap_or(Value::Null),
                    "type": "function",
                    "function": {
                        "name": block.get("name").cloned().unwrap_or(Value::Null),
                        "arguments": block.get("input").map_or_else(
                            || "{}".to_owned(),
                            ToString::to_string,
                        ),
                    },
                }));
                mapping.moved();
            }
            "tool_result" => {
                out.push(json!({
                    "role": "tool",
                    "tool_call_id": block.get("tool_use_id").cloned().unwrap_or(Value::Null),
                    "content": joined_text(block.get("content").unwrap_or(&Value::Null)),
                }));
                mapping.moved();
            }
            "thinking" | "redacted_thinking" => mapping.dropped(
                where_,
                "the provider serving this format cannot verify reasoning minted elsewhere",
            ),
            other => {
                return Err(TranslateError::NoCounterpart {
                    what: format!("a {other:?} block at {where_}"),
                    wire: WIRE,
                });
            }
        }
    }

    if spoken.is_empty() && calls.is_empty() {
        return Ok(());
    }

    let mut carried = Map::new();
    carried.insert("role".into(), json!(role));
    carried.insert("content".into(), narrow(spoken));
    if !calls.is_empty() {
        carried.insert("tool_calls".into(), Value::Array(calls));
    }
    out.push(Value::Object(carried));
    Ok(())
}

fn narrow(spoken: Vec<Value>) -> Value {
    match spoken.as_slice() {
        [] => Value::Null,
        [only] if only.get("type").and_then(Value::as_str) == Some("text") => {
            json!(text_of(only))
        }
        _ => Value::Array(spoken),
    }
}

fn as_image(block: &Value, where_: &str) -> Result<Value, TranslateError> {
    let source = block.get("source").unwrap_or(&Value::Null);
    match source.get("type").and_then(Value::as_str) {
        Some("base64") => {
            let media = source
                .get("media_type")
                .and_then(Value::as_str)
                .unwrap_or("image/png");
            let data = source
                .get("data")
                .and_then(Value::as_str)
                .unwrap_or_default();
            Ok(json!({
                "type": "image_url",
                "image_url": { "url": format!("data:{media};base64,{data}") },
            }))
        }
        Some("url") => Ok(json!({
            "type": "image_url",
            "image_url": { "url": source.get("url").cloned().unwrap_or(Value::Null) },
        })),
        other => Err(TranslateError::Malformed {
            what: format!("the image at {where_}"),
            detail: format!("its source is {other:?}, which this gateway cannot carry across"),
        }),
    }
}

fn as_function(tool: &Value) -> Result<Value, TranslateError> {
    let Some(name) = tool.get("name").and_then(Value::as_str) else {
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
    let mut function = Map::new();
    function.insert("name".into(), json!(name));
    if let Some(description) = tool.get("description") {
        function.insert("description".into(), description.clone());
    }
    function.insert(
        "parameters".into(),
        tool.get("input_schema")
            .cloned()
            .unwrap_or_else(|| json!({ "type": "object", "properties": {} })),
    );
    Ok(json!({ "type": "function", "function": Value::Object(function) }))
}

fn as_choice(choice: &Value) -> Result<Value, TranslateError> {
    match choice.get("type").and_then(Value::as_str) {
        Some("auto") => Ok(json!("auto")),
        Some("any") => Ok(json!("required")),
        Some("none") => Ok(json!("none")),
        Some("tool") => Ok(json!({
            "type": "function",
            "function": { "name": choice.get("name").cloned().unwrap_or(Value::Null) },
        })),
        other => Err(TranslateError::NoCounterpart {
            what: format!("tool_choice {other:?}"),
            wire: WIRE,
        }),
    }
}

fn array(value: Option<&Value>) -> &[Value] {
    value.and_then(Value::as_array).map_or(&[], Vec::as_slice)
}

fn text_of(block: &Value) -> String {
    block
        .get("text")
        .and_then(Value::as_str)
        .unwrap_or_default()
        .to_owned()
}

fn joined_text(value: &Value) -> String {
    match value {
        Value::String(text) => text.clone(),
        Value::Array(blocks) => blocks
            .iter()
            .filter(|block| block.get("type").and_then(Value::as_str) != Some("image"))
            .map(text_of)
            .collect::<Vec<_>>()
            .join("\n\n"),
        _ => String::new(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sent(request: Value) -> Value {
        let out = translate(&serde_json::to_vec(&request).unwrap(), "kimi-for-coding").unwrap();
        serde_json::from_slice(&out.body).unwrap()
    }

    #[test]
    fn a_system_prompt_becomes_the_first_message() {
        let out = sent(json!({
            "model": "claude-sonnet-5",
            "system": [{ "type": "text", "text": "You are terse." }],
            "messages": [{ "role": "user", "content": "hi" }],
        }));

        assert_eq!(out["model"], "kimi-for-coding");
        assert_eq!(out["messages"][0]["role"], "system");
        assert_eq!(out["messages"][0]["content"], "You are terse.");
        assert_eq!(out["messages"][1]["content"], "hi");
    }

    #[test]
    fn a_tool_call_keeps_the_id_it_was_given() {
        let out = sent(json!({
            "messages": [
                { "role": "assistant", "content": [
                    { "type": "text", "text": "let me look" },
                    { "type": "tool_use", "id": "toolu_01ABC", "name": "bash", "input": { "cmd": "ls" } },
                ]},
                { "role": "user", "content": [
                    { "type": "tool_result", "tool_use_id": "toolu_01ABC", "content": "a.txt" },
                ]},
            ],
        }));

        let call = &out["messages"][0]["tool_calls"][0];
        assert_eq!(call["id"], "toolu_01ABC");
        assert_eq!(call["function"]["name"], "bash");
        assert_eq!(call["function"]["arguments"], r#"{"cmd":"ls"}"#);

        assert_eq!(out["messages"][1]["role"], "tool");
        assert_eq!(
            out["messages"][1]["tool_call_id"], "toolu_01ABC",
            "rewriting a tool id makes the conversation unresumable"
        );
    }

    #[test]
    fn a_tool_result_lands_right_after_the_call_that_asked_for_it() {
        let out = sent(json!({
            "messages": [
                { "role": "assistant", "content": [
                    { "type": "tool_use", "id": "t1", "name": "bash", "input": {} },
                ]},
                { "role": "user", "content": [
                    { "type": "tool_result", "tool_use_id": "t1", "content": "done" },
                    { "type": "text", "text": "and now?" },
                ]},
            ],
        }));

        let roles: Vec<&str> = out["messages"]
            .as_array()
            .unwrap()
            .iter()
            .map(|m| m["role"].as_str().unwrap())
            .collect();
        assert_eq!(roles, ["assistant", "tool", "user"]);
        assert_eq!(out["messages"][2]["content"], "and now?");
    }

    #[test]
    fn thinking_from_another_provider_is_dropped_and_said_so() {
        let out = translate(
            &serde_json::to_vec(&json!({
                "messages": [{ "role": "assistant", "content": [
                    { "type": "thinking", "thinking": "hmm", "signature": "EqQBCg" },
                    { "type": "text", "text": "yes" },
                ]}],
            }))
            .unwrap(),
            "kimi-for-coding",
        )
        .unwrap();

        assert!(out.mapping.lost_anything());
        let body: Value = serde_json::from_slice(&out.body).unwrap();
        assert_eq!(body["messages"][0]["content"], "yes");
        assert!(
            !out.body.windows(6).any(|w| w == b"EqQBCg"),
            "a signature the provider cannot verify must not be sent"
        );
    }

    #[test]
    fn a_streaming_request_asks_for_the_usage_it_needs() {
        let out = sent(json!({ "stream": true, "messages": [] }));
        assert_eq!(out["stream_options"]["include_usage"], true);
    }

    #[test]
    fn a_tool_declaration_becomes_a_function() {
        let out = sent(json!({
            "messages": [],
            "tools": [{
                "name": "bash",
                "description": "run it",
                "input_schema": { "type": "object", "properties": { "cmd": { "type": "string" } } },
            }],
            "tool_choice": { "type": "any" },
        }));

        assert_eq!(out["tools"][0]["type"], "function");
        assert_eq!(out["tools"][0]["function"]["name"], "bash");
        assert_eq!(out["tools"][0]["function"]["parameters"]["type"], "object");
        assert_eq!(out["tool_choice"], "required");
    }

    #[test]
    fn a_block_with_no_counterpart_stops_the_request() {
        let error = translate(
            &serde_json::to_vec(&json!({
                "messages": [{ "role": "user", "content": [{ "type": "document", "source": {} }] }],
            }))
            .unwrap(),
            "kimi-for-coding",
        )
        .unwrap_err();
        assert!(matches!(error, TranslateError::NoCounterpart { .. }));
    }

    #[test]
    fn an_image_travels_as_a_data_url() {
        let out = sent(json!({
            "messages": [{ "role": "user", "content": [
                { "type": "image", "source": { "type": "base64", "media_type": "image/png", "data": "AAAA" } },
            ]}],
        }));
        assert_eq!(
            out["messages"][0]["content"][0]["image_url"]["url"],
            "data:image/png;base64,AAAA"
        );
    }
}

#[derive(Debug, Clone)]
pub struct StreamTarget {
    pub completion_id: String,
    pub model: String,
    pub created: i64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Speaking {
    Text,
    Thinking,
    Tool(usize),
}

pub struct StreamTranslator {
    parser: crate::sse::Parser,
    target: StreamTarget,
    opened: bool,
    finished: bool,
    speaking: Option<Speaking>,
    calls: usize,
    finish_reason: Option<&'static str>,
    usage: Option<Value>,
    spoken: String,
    thought: String,
    tools: Vec<Value>,
}

impl StreamTranslator {
    pub fn new(target: StreamTarget) -> Self {
        Self {
            parser: crate::sse::Parser::default(),
            target,
            opened: false,
            finished: false,
            speaking: None,
            calls: 0,
            finish_reason: None,
            usage: None,
            spoken: String::new(),
            thought: String::new(),
            tools: Vec::new(),
        }
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
        if self.opened && !self.finished {
            self.finished = true;
            out.push_str(&self.chunk(json!({}), self.finish_reason.or(Some("stop"))));
            if let Some(usage) = self.usage.clone() {
                out.push_str(&self.tally(&usage));
            }
            out.push_str("data: [DONE]\n\n");
        }
        out.into_bytes()
    }

    pub fn assembled(&self) -> Value {
        let mut message = Map::new();
        message.insert("role".into(), json!("assistant"));
        message.insert(
            "content".into(),
            if self.spoken.is_empty() {
                Value::Null
            } else {
                json!(self.spoken)
            },
        );
        if !self.thought.is_empty() {
            message.insert("reasoning_content".into(), json!(self.thought));
        }
        if !self.tools.is_empty() {
            message.insert("tool_calls".into(), Value::Array(self.tools.clone()));
        }

        json!({
            "id": self.target.completion_id,
            "object": "chat.completion",
            "created": self.target.created,
            "model": self.target.model,
            "choices": [{
                "index": 0,
                "message": Value::Object(message),
                "finish_reason": self.finish_reason.unwrap_or("stop"),
            }],
            "usage": self.usage.clone().unwrap_or(Value::Null),
        })
    }

    fn take(&mut self, frame: &crate::sse::Frame, out: &mut String) {
        let Some(event) = frame.json() else { return };
        match event
            .get("type")
            .and_then(Value::as_str)
            .unwrap_or_default()
        {
            "message_start" => {
                self.opened = true;
                out.push_str(&self.chunk(json!({ "role": "assistant", "content": "" }), None));
                self.note_usage(event.get("message").and_then(|m| m.get("usage")));
            }
            "content_block_start" => self.opened_block(&event, out),
            "content_block_delta" => self.wrote(&event, out),
            "content_block_stop" => self.speaking = None,
            "message_delta" => {
                if let Some(reason) = event
                    .get("delta")
                    .and_then(|delta| delta.get("stop_reason"))
                    .and_then(Value::as_str)
                {
                    self.finish_reason = Some(match reason {
                        "max_tokens" => "length",
                        "tool_use" => "tool_calls",
                        "refusal" => "content_filter",
                        _ => "stop",
                    });
                }
                self.note_usage(event.get("usage"));
            }
            "error" => {
                self.finished = true;
                out.push_str(&format!(
                    "data: {}\n\n",
                    json!({ "error": event.get("error").cloned().unwrap_or(Value::Null) })
                ));
            }
            _ => {}
        }
    }

    fn opened_block(&mut self, event: &Value, out: &mut String) {
        let block = event.get("content_block").unwrap_or(&Value::Null);
        match block
            .get("type")
            .and_then(Value::as_str)
            .unwrap_or_default()
        {
            "text" => self.speaking = Some(Speaking::Text),
            "thinking" | "redacted_thinking" => self.speaking = Some(Speaking::Thinking),
            "tool_use" => {
                let slot = self.calls;
                self.calls += 1;
                self.speaking = Some(Speaking::Tool(slot));
                self.tools.push(json!({
                    "index": slot,
                    "id": block.get("id").cloned().unwrap_or(Value::Null),
                    "type": "function",
                    "function": {
                        "name": block.get("name").cloned().unwrap_or(Value::Null),
                        "arguments": "",
                    },
                }));
                out.push_str(&self.chunk(
                    json!({ "tool_calls": [{
                        "index": slot,
                        "id": block.get("id").cloned().unwrap_or(Value::Null),
                        "type": "function",
                        "function": {
                            "name": block.get("name").cloned().unwrap_or(Value::Null),
                            "arguments": "",
                        },
                    }] }),
                    None,
                ));
            }
            _ => self.speaking = None,
        }
    }

    fn wrote(&mut self, event: &Value, out: &mut String) {
        let delta = event.get("delta").unwrap_or(&Value::Null);
        match self.speaking {
            Some(Speaking::Text) => {
                if let Some(text) = delta.get("text").and_then(Value::as_str) {
                    self.spoken.push_str(text);
                    out.push_str(&self.chunk(json!({ "content": text }), None));
                }
            }
            Some(Speaking::Thinking) => {
                if let Some(text) = delta.get("thinking").and_then(Value::as_str) {
                    self.thought.push_str(text);
                    out.push_str(&self.chunk(json!({ "reasoning_content": text }), None));
                }
            }
            Some(Speaking::Tool(slot)) => {
                if let Some(fragment) = delta.get("partial_json").and_then(Value::as_str) {
                    if let Some(call) = self.tools.get_mut(slot) {
                        let held = call["function"]["arguments"]
                            .as_str()
                            .unwrap_or_default()
                            .to_owned();
                        call["function"]["arguments"] = json!(format!("{held}{fragment}"));
                    }
                    out.push_str(&self.chunk(
                        json!({ "tool_calls": [{
                            "index": slot,
                            "function": { "arguments": fragment },
                        }] }),
                        None,
                    ));
                }
            }
            None => {}
        }
    }

    fn note_usage(&mut self, usage: Option<&Value>) {
        let Some(usage) = usage.filter(|usage| !usage.is_null()) else {
            return;
        };
        let read = |name: &str| usage.get(name).and_then(Value::as_i64);
        let held = self.usage.get_or_insert_with(
            || json!({ "prompt_tokens": 0, "completion_tokens": 0, "total_tokens": 0 }),
        );

        let cached = read("cache_read_input_tokens").unwrap_or(0);
        if let Some(input) = read("input_tokens") {
            held["prompt_tokens"] = json!(input + cached);
            held["prompt_tokens_details"] = json!({ "cached_tokens": cached });
        }
        if let Some(output) = read("output_tokens") {
            held["completion_tokens"] = json!(output);
        }
        let prompt = held["prompt_tokens"].as_i64().unwrap_or(0);
        let completion = held["completion_tokens"].as_i64().unwrap_or(0);
        held["total_tokens"] = json!(prompt + completion);
    }

    fn chunk(&self, delta: Value, finish: Option<&str>) -> String {
        format!(
            "data: {}\n\n",
            json!({
                "id": self.target.completion_id,
                "object": "chat.completion.chunk",
                "created": self.target.created,
                "model": self.target.model,
                "choices": [{ "index": 0, "delta": delta, "finish_reason": finish }],
            })
        )
    }

    fn tally(&self, usage: &Value) -> String {
        format!(
            "data: {}\n\n",
            json!({
                "id": self.target.completion_id,
                "object": "chat.completion.chunk",
                "created": self.target.created,
                "model": self.target.model,
                "choices": [],
                "usage": usage,
            })
        )
    }
}

#[cfg(test)]
mod stream_tests {
    use super::*;
    use crate::sse::{Frame, Parser};

    fn target() -> StreamTarget {
        StreamTarget {
            completion_id: "chatcmpl_1".into(),
            model: "gpt-5".into(),
            created: 1_700_000_000,
        }
    }

    fn anthropic(events: &[(&str, Value)]) -> String {
        events
            .iter()
            .map(|(event, data)| format!("event: {event}\ndata: {data}\n\n"))
            .collect()
    }

    fn run(text: &str) -> (Vec<Frame>, StreamTranslator) {
        let mut translator = StreamTranslator::new(target());
        let mut out = translator.push(text.as_bytes());
        out.extend(translator.finish());
        (Parser::default().push(&out), translator)
    }

    fn deltas(frames: &[Frame], field: &str) -> String {
        frames
            .iter()
            .filter_map(|frame| frame.json())
            .filter_map(|chunk| {
                chunk["choices"][0]["delta"][field]
                    .as_str()
                    .map(str::to_owned)
            })
            .collect()
    }

    const SPEAKING: &[(&str, &str)] = &[];

    #[test]
    fn text_and_thinking_arrive_on_the_fields_chat_clients_read() {
        let text = anthropic(&[
            (
                "message_start",
                json!({ "type": "message_start", "message": { "usage": { "input_tokens": 10 } } }),
            ),
            (
                "content_block_start",
                json!({ "type": "content_block_start", "index": 0, "content_block": { "type": "thinking" } }),
            ),
            (
                "content_block_delta",
                json!({ "type": "content_block_delta", "index": 0, "delta": { "type": "thinking_delta", "thinking": "weigh" } }),
            ),
            (
                "content_block_delta",
                json!({ "type": "content_block_delta", "index": 0, "delta": { "type": "thinking_delta", "thinking": "ing" } }),
            ),
            (
                "content_block_stop",
                json!({ "type": "content_block_stop", "index": 0 }),
            ),
            (
                "content_block_start",
                json!({ "type": "content_block_start", "index": 1, "content_block": { "type": "text" } }),
            ),
            (
                "content_block_delta",
                json!({ "type": "content_block_delta", "index": 1, "delta": { "type": "text_delta", "text": "Here." } }),
            ),
            (
                "content_block_stop",
                json!({ "type": "content_block_stop", "index": 1 }),
            ),
            (
                "message_delta",
                json!({ "type": "message_delta", "delta": { "stop_reason": "end_turn" }, "usage": { "output_tokens": 8 } }),
            ),
            ("message_stop", json!({ "type": "message_stop" })),
        ]);

        let (frames, _) = run(&text);
        assert_eq!(deltas(&frames, "reasoning_content"), "weighing");
        assert_eq!(deltas(&frames, "content"), "Here.");

        let ended = frames
            .iter()
            .filter_map(|frame| frame.json())
            .find(|chunk| chunk["choices"][0]["finish_reason"] == "stop")
            .expect("a chat client waits for a finish_reason");
        assert_eq!(ended["object"], "chat.completion.chunk");

        let raw: String = frames.iter().map(|frame| frame.data.clone()).collect();
        assert!(raw.contains("[DONE]"), "a chat stream ends with [DONE]");
        let _ = SPEAKING;
    }

    #[test]
    fn a_tool_use_block_becomes_an_indexed_tool_call() {
        let text = anthropic(&[
            (
                "message_start",
                json!({ "type": "message_start", "message": {} }),
            ),
            (
                "content_block_start",
                json!({ "type": "content_block_start", "index": 0, "content_block": { "type": "tool_use", "id": "toolu_01A", "name": "bash" } }),
            ),
            (
                "content_block_delta",
                json!({ "type": "content_block_delta", "index": 0, "delta": { "type": "input_json_delta", "partial_json": "{\"cmd\"" } }),
            ),
            (
                "content_block_delta",
                json!({ "type": "content_block_delta", "index": 0, "delta": { "type": "input_json_delta", "partial_json": ":\"ls\"}" } }),
            ),
            (
                "content_block_stop",
                json!({ "type": "content_block_stop", "index": 0 }),
            ),
            (
                "message_delta",
                json!({ "type": "message_delta", "delta": { "stop_reason": "tool_use" } }),
            ),
        ]);

        let (frames, translator) = run(&text);

        let opened = frames
            .iter()
            .filter_map(|frame| frame.json())
            .find(|chunk| chunk["choices"][0]["delta"]["tool_calls"][0]["id"] == "toolu_01A")
            .expect("the id has to reach the client unchanged");
        assert_eq!(opened["choices"][0]["delta"]["tool_calls"][0]["index"], 0);
        assert_eq!(
            opened["choices"][0]["delta"]["tool_calls"][0]["function"]["name"],
            "bash"
        );

        let arguments: String = frames
            .iter()
            .filter_map(|frame| frame.json())
            .filter_map(|chunk| {
                chunk["choices"][0]["delta"]["tool_calls"][0]["function"]["arguments"]
                    .as_str()
                    .map(str::to_owned)
            })
            .collect();
        assert_eq!(arguments, r#"{"cmd":"ls"}"#);

        let ended = frames
            .iter()
            .filter_map(|frame| frame.json())
            .find(|chunk| !chunk["choices"][0]["finish_reason"].is_null())
            .unwrap();
        assert_eq!(ended["choices"][0]["finish_reason"], "tool_calls");

        let whole = translator.assembled();
        assert_eq!(
            whole["choices"][0]["message"]["tool_calls"][0]["id"],
            "toolu_01A"
        );
        assert_eq!(
            whole["choices"][0]["message"]["tool_calls"][0]["function"]["arguments"],
            r#"{"cmd":"ls"}"#
        );
    }

    #[test]
    fn the_cache_is_counted_once_in_the_prompt_total() {
        let text = anthropic(&[
            (
                "message_start",
                json!({ "type": "message_start", "message": { "usage": { "input_tokens": 100, "cache_read_input_tokens": 900 } } }),
            ),
            (
                "message_delta",
                json!({ "type": "message_delta", "delta": { "stop_reason": "end_turn" }, "usage": { "output_tokens": 42 } }),
            ),
        ]);

        let (frames, translator) = run(&text);
        let counted = frames
            .iter()
            .filter_map(|frame| frame.json())
            .find(|chunk| !chunk["usage"].is_null())
            .expect("a chat client reads usage off the last chunk");

        assert_eq!(counted["usage"]["prompt_tokens"], 1000);
        assert_eq!(
            counted["usage"]["prompt_tokens_details"]["cached_tokens"],
            900
        );
        assert_eq!(counted["usage"]["completion_tokens"], 42);
        assert_eq!(counted["usage"]["total_tokens"], 1042);
        assert_eq!(translator.assembled()["usage"]["total_tokens"], 1042);
    }

    #[test]
    fn a_stream_that_dies_is_relayed_rather_than_ended_cleanly() {
        let text = anthropic(&[
            (
                "message_start",
                json!({ "type": "message_start", "message": {} }),
            ),
            (
                "error",
                json!({ "type": "error", "error": { "type": "overloaded_error", "message": "Overloaded" } }),
            ),
        ]);

        let (frames, _) = run(&text);
        let raw: String = frames.iter().map(|frame| frame.data.clone()).collect();
        assert!(raw.contains("Overloaded"));
        assert!(
            !raw.contains("[DONE]"),
            "a stream that failed did not finish"
        );
    }
}
