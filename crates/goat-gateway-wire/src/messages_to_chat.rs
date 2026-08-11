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

    Ok(Translated {
        body: serde_json::to_vec(&Value::Object(out))?,
        mapping,
    })
}

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
