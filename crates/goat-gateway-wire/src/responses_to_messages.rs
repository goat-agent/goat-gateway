use serde_json::{Map, Value, json};

use crate::{
    envelope::{Envelopes, Payload, Provenance},
    mapping::Mapping,
};

pub use crate::mapping::{TranslateError, Translated};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ThinkingStyle {
    Adaptive,
    Budget,
    Unsupported,
}

#[derive(Debug, Clone)]
pub struct TargetModel {
    pub name: String,
    pub thinking: ThinkingStyle,
    pub default_max_tokens: u32,
    pub mid_conversation_system: bool,
}

#[derive(Debug, Clone)]
pub struct Target {
    pub model: TargetModel,
    pub provenance: Provenance,
    pub stream_thinking: bool,
}

pub fn translate(
    input: &[u8],
    target: &Target,
    envelopes: &Envelopes,
) -> Result<Translated, TranslateError> {
    let source: Value = serde_json::from_slice(input)?;
    let mut mapping = Mapping::default();
    let mut out = Map::new();

    out.insert("model".into(), json!(target.model.name));
    mapping.moved();

    let mut system = Vec::new();
    if let Some(instructions) = source.get("instructions") {
        system.extend(text_blocks(instructions));
        mapping.moved();
    }

    let mut messages = Builder::default();
    match source.get("input") {
        Some(Value::String(text)) => {
            messages.push("user", json!({ "type": "text", "text": text }));
            mapping.moved();
        }
        Some(Value::Array(items)) => {
            for (index, item) in items.iter().enumerate() {
                convert_item(
                    item,
                    index,
                    target,
                    envelopes,
                    &mut system,
                    &mut messages,
                    &mut mapping,
                )?;
            }
        }
        Some(other) => {
            return Err(TranslateError::Malformed {
                what: "input".into(),
                detail: format!("expected a string or an array, found {other}"),
            });
        }
        None => {}
    }

    if !system.is_empty() {
        out.insert("system".into(), Value::Array(system));
    }
    out.insert("messages".into(), Value::Array(messages.finish()));

    let max_tokens = source
        .get("max_output_tokens")
        .and_then(Value::as_u64)
        .inspect(|_n| {
            mapping.moved();
        })
        .unwrap_or_else(|| {
            mapping.added(
                "/max_tokens",
                "Anthropic requires max_tokens; used the value declared for this model",
            );
            u64::from(target.model.default_max_tokens)
        });
    out.insert("max_tokens".into(), json!(max_tokens));

    if let Some(tools) = source.get("tools") {
        let converted = convert_tools(tools, &mut mapping)?;
        if !converted.is_empty() {
            out.insert("tools".into(), Value::Array(converted));
        }
    }
    if let Some(choice) = source.get("tool_choice") {
        out.insert("tool_choice".into(), convert_tool_choice(choice)?);
        mapping.moved();
    }

    if let Some(reasoning) = source.get("reasoning") {
        apply_thinking(reasoning, target, &mut out, &mut mapping)?;
    }

    for (from, to) in [
        ("temperature", "temperature"),
        ("top_p", "top_p"),
        ("stream", "stream"),
    ] {
        if let Some(value) = source.get(from) {
            out.insert(to.into(), value.clone());
            mapping.moved();
        }
    }

    if let Some(Value::Object(metadata)) = source.get("metadata")
        && let Some(user) = metadata.get("user_id")
    {
        out.insert("metadata".into(), json!({ "user_id": user }));
        mapping.moved();
    }

    for ignorable in ["store", "previous_response_id", "include", "background"] {
        if source.get(ignorable).is_some() {
            mapping.dropped(
                format!("/{ignorable}"),
                "state is held by the gateway, not by the provider",
            );
        }
    }

    Ok(Translated {
        body: serde_json::to_vec(&Value::Object(out))?,
        mapping,
    })
}

#[derive(Default)]
struct Builder {
    messages: Vec<Value>,
    role: Option<String>,
    blocks: Vec<Value>,
}

impl Builder {
    fn push(&mut self, role: &str, block: Value) {
        if self.role.as_deref() != Some(role) {
            self.flush();
            self.role = Some(role.to_owned());
        }
        self.blocks.push(block);
    }

    fn flush(&mut self) {
        if let Some(role) = self.role.take()
            && !self.blocks.is_empty()
        {
            self.messages.push(json!({
                "role": role,
                "content": std::mem::take(&mut self.blocks),
            }));
        }
        self.blocks.clear();
    }

    fn finish(mut self) -> Vec<Value> {
        self.flush();
        self.messages
    }
}

#[allow(clippy::too_many_arguments)]
fn convert_item(
    item: &Value,
    index: usize,
    target: &Target,
    envelopes: &Envelopes,
    system: &mut Vec<Value>,
    messages: &mut Builder,
    mapping: &mut Mapping,
) -> Result<(), TranslateError> {
    let at = |suffix: &str| format!("/input/{index}{suffix}");
    let kind = item
        .get("type")
        .and_then(Value::as_str)
        .unwrap_or("message");

    match kind {
        "message" => {
            let role = item.get("role").and_then(Value::as_str).unwrap_or("user");
            let content = item.get("content").unwrap_or(&Value::Null);
            match role {
                "system" | "developer" => {
                    if messages.messages.is_empty() && messages.blocks.is_empty() {
                        system.extend(text_blocks(content));
                    } else if target.model.mid_conversation_system {
                        for block in text_blocks(content) {
                            messages.push("system", block);
                        }
                    } else {
                        return Err(TranslateError::NoCounterpart {
                            what: format!(
                                "a {role} message in the middle of the conversation ({})",
                                at("")
                            ),
                            wire: "Anthropic Messages",
                        });
                    }
                }
                "assistant" => {
                    for block in text_blocks(content) {
                        messages.push("assistant", block);
                    }
                }
                _ => {
                    for block in content_blocks(content, &at("/content"))? {
                        messages.push("user", block);
                    }
                }
            }
            mapping.moved();
        }

        "function_call" | "custom_tool_call" => {
            let name = item.get("name").and_then(Value::as_str).unwrap_or_default();
            let id = item
                .get("call_id")
                .and_then(Value::as_str)
                .unwrap_or_default();
            let input = if kind == "function_call" {
                let raw = item
                    .get("arguments")
                    .and_then(Value::as_str)
                    .unwrap_or("{}");
                serde_json::from_str::<Value>(raw).map_err(|error| TranslateError::Malformed {
                    what: at("/arguments"),
                    detail: error.to_string(),
                })?
            } else {
                json!({ "input": item.get("input").and_then(Value::as_str).unwrap_or_default() })
            };
            messages.push(
                "assistant",
                json!({ "type": "tool_use", "id": id, "name": name, "input": input }),
            );
            mapping.moved();
        }

        "function_call_output" | "custom_tool_call_output" => {
            let id = item
                .get("call_id")
                .and_then(Value::as_str)
                .unwrap_or_default();
            let output = item.get("output").unwrap_or(&Value::Null);
            let content = match output {
                Value::String(text) => json!([{ "type": "text", "text": text }]),
                Value::Array(_) => Value::Array(content_blocks(output, &at("/output"))?),
                other => json!([{ "type": "text", "text": other.to_string() }]),
            };
            messages.push(
                "user",
                json!({ "type": "tool_result", "tool_use_id": id, "content": content }),
            );
            mapping.moved();
        }

        "reasoning" => {
            let blob = item.get("encrypted_content").and_then(Value::as_str);
            match blob {
                Some(blob) if crate::envelope::is_ours(blob) => {
                    let sealed = envelopes.open_for(blob, &target.provenance).map_err(|e| {
                        TranslateError::NoCounterpart {
                            what: format!("{} ({e})", at("/encrypted_content")),
                            wire: "Anthropic Messages",
                        }
                    })?;
                    let block = match sealed.payload {
                        Payload::Thinking {
                            thinking,
                            signature,
                        } => json!({
                            "type": "thinking",
                            "thinking": thinking,
                            "signature": signature,
                        }),
                        Payload::RedactedThinking { data } => {
                            json!({ "type": "redacted_thinking", "data": data })
                        }
                    };
                    messages.push("assistant", block);
                    mapping.moved();
                }
                Some(_) => {
                    mapping.dropped(
                        at("/encrypted_content"),
                        "minted by another provider and organization-bound; it cannot be decrypted here",
                    );
                }
                None => {
                    mapping.dropped(
                        at(""),
                        "a reasoning item with no encrypted_content carries nothing Anthropic can verify",
                    );
                }
            }
        }

        other => {
            return Err(TranslateError::NoCounterpart {
                what: format!("input item type {other:?} at {}", at("")),
                wire: "Anthropic Messages",
            });
        }
    }

    Ok(())
}

fn text_blocks(content: &Value) -> Vec<Value> {
    match content {
        Value::String(text) => vec![json!({ "type": "text", "text": text })],
        Value::Array(parts) => parts
            .iter()
            .filter_map(|part| {
                part.get("text")
                    .and_then(Value::as_str)
                    .map(|text| json!({ "type": "text", "text": text }))
            })
            .collect(),
        _ => Vec::new(),
    }
}

fn content_blocks(content: &Value, at: &str) -> Result<Vec<Value>, TranslateError> {
    let parts = match content {
        Value::String(text) => return Ok(vec![json!({ "type": "text", "text": text })]),
        Value::Array(parts) => parts,
        Value::Null => return Ok(Vec::new()),
        other => {
            return Err(TranslateError::Malformed {
                what: at.to_owned(),
                detail: format!("expected string or array, found {other}"),
            });
        }
    };

    let mut blocks = Vec::with_capacity(parts.len());
    for part in parts {
        let kind = part.get("type").and_then(Value::as_str).unwrap_or_default();
        match kind {
            "input_text" | "output_text" | "text" => blocks.push(json!({
                "type": "text",
                "text": part.get("text").and_then(Value::as_str).unwrap_or_default(),
            })),
            "input_image" | "image" => blocks.push(image_block(part, at)?),
            other => {
                return Err(TranslateError::NoCounterpart {
                    what: format!("content part {other:?} at {at}"),
                    wire: "Anthropic Messages",
                });
            }
        }
    }
    Ok(blocks)
}

fn image_block(part: &Value, at: &str) -> Result<Value, TranslateError> {
    let url = part
        .get("image_url")
        .and_then(Value::as_str)
        .ok_or_else(|| TranslateError::Malformed {
            what: at.to_owned(),
            detail: "image part without image_url".into(),
        })?;

    if let Some(rest) = url.strip_prefix("data:") {
        let (media_type, data) =
            rest.split_once(";base64,")
                .ok_or_else(|| TranslateError::Malformed {
                    what: at.to_owned(),
                    detail: "data URL is not base64-encoded".into(),
                })?;
        Ok(json!({
            "type": "image",
            "source": { "type": "base64", "media_type": media_type, "data": data },
        }))
    } else {
        Ok(json!({
            "type": "image",
            "source": { "type": "url", "url": url },
        }))
    }
}

fn convert_tools(tools: &Value, mapping: &mut Mapping) -> Result<Vec<Value>, TranslateError> {
    let Value::Array(tools) = tools else {
        return Err(TranslateError::Malformed {
            what: "/tools".into(),
            detail: "expected an array".into(),
        });
    };

    let mut out = Vec::with_capacity(tools.len());
    for (index, tool) in tools.iter().enumerate() {
        let kind = tool.get("type").and_then(Value::as_str).unwrap_or_default();
        match kind {
            "function" => {
                out.push(json!({
                    "name": tool.get("name").and_then(Value::as_str).unwrap_or_default(),
                    "description": tool.get("description").and_then(Value::as_str).unwrap_or_default(),
                    "input_schema": tool.get("parameters").cloned().unwrap_or_else(|| json!({ "type": "object" })),
                }));
                mapping.moved();
            }
            "custom" => {
                out.push(json!({
                    "name": tool.get("name").and_then(Value::as_str).unwrap_or_default(),
                    "description": tool.get("description").and_then(Value::as_str).unwrap_or_default(),
                    "input_schema": { "type": "object", "properties": { "input": { "type": "string" } } },
                }));
                mapping.moved();
            }
            "web_search" | "web_search_preview" => {
                out.push(json!({
                    "type": "web_search_20250305",
                    "name": "web_search",
                }));
                mapping.moved();
            }
            other => {
                return Err(TranslateError::NoCounterpart {
                    what: format!("tool type {other:?} at /tools/{index}"),
                    wire: "Anthropic Messages",
                });
            }
        }
    }
    Ok(out)
}

fn convert_tool_choice(choice: &Value) -> Result<Value, TranslateError> {
    Ok(match choice {
        Value::String(mode) => match mode.as_str() {
            "auto" => json!({ "type": "auto" }),
            "required" => json!({ "type": "any" }),
            "none" => json!({ "type": "none" }),
            other => {
                return Err(TranslateError::NoCounterpart {
                    what: format!("tool_choice {other:?}"),
                    wire: "Anthropic Messages",
                });
            }
        },
        Value::Object(map) => {
            let name = map.get("name").and_then(Value::as_str).ok_or_else(|| {
                TranslateError::Malformed {
                    what: "/tool_choice".into(),
                    detail: "expected a tool name".into(),
                }
            })?;
            json!({ "type": "tool", "name": name })
        }
        other => {
            return Err(TranslateError::Malformed {
                what: "/tool_choice".into(),
                detail: format!("unexpected {other}"),
            });
        }
    })
}

fn apply_thinking(
    reasoning: &Value,
    target: &Target,
    out: &mut Map<String, Value>,
    mapping: &mut Mapping,
) -> Result<(), TranslateError> {
    let effort = reasoning
        .get("effort")
        .and_then(Value::as_str)
        .unwrap_or("medium");

    match target.model.thinking {
        ThinkingStyle::Unsupported => {
            mapping.dropped(
                "/reasoning",
                "this model does not expose a thinking control",
            );
            return Ok(());
        }
        ThinkingStyle::Adaptive => {
            let mut thinking = json!({ "type": "adaptive" });
            if target.stream_thinking {
                thinking["display"] = json!("summarized");
                mapping.added(
                    "/thinking/display",
                    "asked for summarized thinking so the client receives thinking deltas live",
                );
            }
            out.insert("thinking".into(), thinking);
            out.insert("output_config".into(), json!({ "effort": effort }));
        }
        ThinkingStyle::Budget => {
            let budget = match effort {
                "none" | "minimal" => 0,
                "low" => 4_096,
                "high" | "xhigh" | "max" => 16_384,
                _ => 8_192,
            };
            if budget == 0 {
                out.insert("thinking".into(), json!({ "type": "disabled" }));
            } else {
                let mut thinking = json!({ "type": "enabled", "budget_tokens": budget });
                if target.stream_thinking {
                    thinking["display"] = json!("summarized");
                    mapping.added(
                        "/thinking/display",
                        "asked for summarized thinking so the client receives thinking deltas live",
                    );
                }
                out.insert("thinking".into(), thinking);
            }
        }
    }
    mapping.moved();
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::envelope::{Portability, Sealed};

    fn envelopes() -> Envelopes {
        Envelopes::new(&[3u8; 32])
    }

    fn provenance() -> Provenance {
        Provenance {
            provider: "anthropic".into(),
            account: "personal".into(),
            model: "claude-sonnet-5".into(),
        }
    }

    fn target() -> Target {
        Target {
            model: TargetModel {
                name: "claude-sonnet-5".into(),
                thinking: ThinkingStyle::Adaptive,
                default_max_tokens: 8192,
                mid_conversation_system: false,
            },
            provenance: provenance(),
            stream_thinking: true,
        }
    }

    fn run(source: Value) -> Translated {
        translate(
            serde_json::to_vec(&source).unwrap().as_slice(),
            &target(),
            &envelopes(),
        )
        .unwrap()
    }

    fn body(translated: &Translated) -> Value {
        serde_json::from_slice(&translated.body).unwrap()
    }

    #[test]
    fn a_codex_shaped_turn_becomes_a_messages_request() {
        let translated = run(json!({
            "model": "gpt-5.6",
            "instructions": "You are a helpful assistant.",
            "input": [
                { "type": "message", "role": "developer",
                  "content": [{ "type": "input_text", "text": "app context" }] },
                { "type": "message", "role": "user",
                  "content": [{ "type": "input_text", "text": "list the files" }] },
                { "type": "function_call", "call_id": "call_1", "name": "shell",
                  "arguments": "{\"cmd\":\"ls\"}" },
                { "type": "function_call_output", "call_id": "call_1", "output": "a.rs\nb.rs" },
            ],
            "max_output_tokens": 1024,
            "stream": true,
        }));
        let body = body(&translated);

        assert_eq!(body["model"], "claude-sonnet-5");
        assert_eq!(body["max_tokens"], 1024);
        assert_eq!(body["stream"], true);
        assert_eq!(body["system"][0]["text"], "You are a helpful assistant.");
        assert_eq!(body["system"][1]["text"], "app context");

        let messages = body["messages"].as_array().unwrap();
        assert_eq!(messages.len(), 3);
        assert_eq!(messages[0]["role"], "user");
        assert_eq!(messages[0]["content"][0]["text"], "list the files");
        assert_eq!(messages[1]["role"], "assistant");
        assert_eq!(messages[1]["content"][0]["type"], "tool_use");
        assert_eq!(messages[1]["content"][0]["input"]["cmd"], "ls");
        assert_eq!(messages[2]["role"], "user");
        assert_eq!(messages[2]["content"][0]["type"], "tool_result");
        assert_eq!(messages[2]["content"][0]["tool_use_id"], "call_1");
        assert!(!translated.mapping.lost_anything());
    }

    #[test]
    fn our_envelope_restores_the_signature_byte_for_byte() {
        let signature = "EvpRCokBCBAYAipADlQ397V3hO8sP2WxUUU8".repeat(200);
        let blob = envelopes().seal(
            [1u8; 12],
            &Sealed {
                provenance: provenance(),
                portability: Portability::Account,
                payload: Payload::Thinking {
                    thinking: "weighing".into(),
                    signature: signature.clone(),
                },
            },
        );

        let translated = run(json!({
            "input": [
                { "type": "message", "role": "user", "content": "hi" },
                { "type": "reasoning", "id": "rs_1", "summary": [],
                  "encrypted_content": blob },
            ],
        }));
        let body = body(&translated);
        let block = &body["messages"][1]["content"][0];

        assert_eq!(block["type"], "thinking");
        assert_eq!(block["thinking"], "weighing");
        assert_eq!(block["signature"], signature);
        assert!(!translated.mapping.lost_anything());
    }

    #[test]
    fn redacted_thinking_survives_the_round_trip() {
        let blob = envelopes().seal(
            [2u8; 12],
            &Sealed {
                provenance: provenance(),
                portability: Portability::Account,
                payload: Payload::RedactedThinking {
                    data: "opaque".into(),
                },
            },
        );
        let translated = run(json!({
            "input": [{ "type": "reasoning", "encrypted_content": blob }],
        }));
        let block = &body(&translated)["messages"][0]["content"][0];
        assert_eq!(block["type"], "redacted_thinking");
        assert_eq!(block["data"], "opaque");
    }

    #[test]
    fn a_foreign_blob_is_dropped_with_a_reason_never_silently() {
        let translated = run(json!({
            "input": [
                { "type": "message", "role": "user", "content": "hi" },
                { "type": "reasoning", "encrypted_content": "gAAAAABqegSk-JVUzE1Tb7LX" },
            ],
        }));

        assert!(translated.mapping.lost_anything());
        let dropped = &translated.mapping.dropped[0];
        assert_eq!(dropped.pointer, "/input/1/encrypted_content");
        assert!(dropped.why.contains("organization-bound"));
        assert!(dropped.reissued_as.is_none());
    }

    #[test]
    fn an_envelope_from_another_account_is_refused() {
        let mut minted_elsewhere = provenance();
        minted_elsewhere.account = "work".into();
        let blob = envelopes().seal(
            [3u8; 12],
            &Sealed {
                provenance: minted_elsewhere,
                portability: Portability::Account,
                payload: Payload::Thinking {
                    thinking: String::new(),
                    signature: "sig".into(),
                },
            },
        );

        let err = translate(
            serde_json::to_vec(&json!({
                "input": [{ "type": "reasoning", "encrypted_content": blob }],
            }))
            .unwrap()
            .as_slice(),
            &target(),
            &envelopes(),
        )
        .unwrap_err();

        assert!(matches!(err, TranslateError::NoCounterpart { .. }));
    }

    #[test]
    fn max_tokens_is_declared_not_invented() {
        let translated = run(json!({ "input": "hi" }));
        assert_eq!(body(&translated)["max_tokens"], 8192);
        let added = &translated.mapping.added[0];
        assert_eq!(added.pointer, "/max_tokens");
        assert!(added.why.contains("declared"));
    }

    #[test]
    fn streaming_thinking_asks_the_provider_for_summarized_display() {
        let translated = run(json!({ "input": "hi", "reasoning": { "effort": "high" } }));
        let body = body(&translated);
        assert_eq!(body["thinking"]["display"], "summarized");
        assert_eq!(body["output_config"]["effort"], "high");
        assert!(
            translated
                .mapping
                .added
                .iter()
                .any(|a| a.pointer == "/thinking/display")
        );
    }

    #[test]
    fn without_streaming_we_do_not_touch_display() {
        let mut target = target();
        target.stream_thinking = false;
        let translated = translate(
            serde_json::to_vec(&json!({ "input": "hi", "reasoning": { "effort": "low" } }))
                .unwrap()
                .as_slice(),
            &target,
            &envelopes(),
        )
        .unwrap();
        let body: Value = serde_json::from_slice(&translated.body).unwrap();
        assert!(body["thinking"].get("display").is_none());
    }

    #[test]
    fn an_unmappable_tool_fails_before_we_call_the_provider() {
        let err = translate(
            serde_json::to_vec(&json!({
                "input": "draw me a cat",
                "tools": [{ "type": "image_generation" }],
            }))
            .unwrap()
            .as_slice(),
            &target(),
            &envelopes(),
        )
        .unwrap_err();

        let message = err.to_string();
        assert!(message.contains("image_generation"), "{message}");
        assert!(message.contains("no counterpart"), "{message}");
    }

    #[test]
    fn web_search_maps_to_the_anthropic_server_tool() {
        let translated = run(json!({
            "input": "what happened today",
            "tools": [{ "type": "web_search" }],
        }));
        assert_eq!(body(&translated)["tools"][0]["type"], "web_search_20250305");
    }

    #[test]
    fn a_mid_conversation_developer_message_fails_closed() {
        let err = translate(
            serde_json::to_vec(&json!({
                "input": [
                    { "type": "message", "role": "user", "content": "hi" },
                    { "type": "message", "role": "developer", "content": "new rules" },
                ],
            }))
            .unwrap()
            .as_slice(),
            &target(),
            &envelopes(),
        )
        .unwrap_err();
        assert!(matches!(err, TranslateError::NoCounterpart { .. }));
    }

    #[test]
    fn a_model_that_allows_it_keeps_the_mid_conversation_system_turn() {
        let mut target = target();
        target.model.mid_conversation_system = true;
        let translated = translate(
            serde_json::to_vec(&json!({
                "input": [
                    { "type": "message", "role": "user", "content": "hi" },
                    { "type": "message", "role": "developer", "content": "new rules" },
                ],
            }))
            .unwrap()
            .as_slice(),
            &target,
            &envelopes(),
        )
        .unwrap();
        let body: Value = serde_json::from_slice(&translated.body).unwrap();
        assert_eq!(body["messages"][1]["role"], "system");
    }

    #[test]
    fn a_data_url_image_becomes_a_base64_source() {
        let translated = run(json!({
            "input": [{ "type": "message", "role": "user", "content": [
                { "type": "input_image", "image_url": "data:image/png;base64,AAAA" },
            ]}],
        }));
        let source = &body(&translated)["messages"][0]["content"][0]["source"];
        assert_eq!(source["type"], "base64");
        assert_eq!(source["media_type"], "image/png");
        assert_eq!(source["data"], "AAAA");
    }

    #[test]
    fn malformed_tool_arguments_fail_rather_than_reach_the_provider() {
        let err = translate(
            serde_json::to_vec(&json!({
                "input": [{ "type": "function_call", "call_id": "c", "name": "n",
                            "arguments": "{\"broken\": " }],
            }))
            .unwrap()
            .as_slice(),
            &target(),
            &envelopes(),
        )
        .unwrap_err();
        assert!(matches!(err, TranslateError::Malformed { .. }));
    }
}
