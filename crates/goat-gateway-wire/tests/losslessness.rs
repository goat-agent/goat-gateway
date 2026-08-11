use goat_gateway_wire::{
    BodyDigest, BodyEdit, Envelopes, HeaderEdit, Provenance, Record, apply,
    chat_to_messages::{StreamTarget, StreamTranslator},
    messages_to_chat,
    responses_to_messages::{self, Target, TargetModel, ThinkingStyle},
    sse::{Frame, Parser},
};
use serde_json::{Value, json};

fn undocumented() -> Vec<(&'static str, Value)> {
    vec![
        ("namespace", json!("collaboration")),
        ("phase", json!("final_answer")),
        ("obfuscation", json!("aXRzIGp1c3QgcGFkZGluZw")),
        ("fingerprint", json!({ "build": 4471, "surface": "cli" })),
        ("shipped_after_we_were_written", json!([1, 2, 3])),
    ]
}

fn unaccounted(source: &Value, dropped: &[String]) -> Vec<String> {
    source
        .as_object()
        .expect("a request is an object")
        .keys()
        .filter(|name| !dropped.iter().any(|note| note == &format!("/{name}")))
        .filter(|name| KNOWN.contains(&name.as_str()))
        .map(String::clone)
        .collect()
}

const KNOWN: &[&str] = &[
    "namespace",
    "phase",
    "obfuscation",
    "fingerprint",
    "shipped_after_we_were_written",
];

fn envelopes() -> Envelopes {
    Envelopes::new(&[9u8; 32])
}

fn anthropic_target() -> Target {
    Target {
        model: TargetModel {
            name: "claude-sonnet-5".into(),
            thinking: ThinkingStyle::Adaptive,
            default_max_tokens: 32000,
            mid_conversation_system: false,
        },
        provenance: Provenance {
            provider: "anthropic".into(),
            account: "personal".into(),
            model: "claude-sonnet-5".into(),
        },
        stream_thinking: true,
    }
}

#[test]
fn a_responses_request_loses_nothing_it_does_not_write_down() {
    let mut request = json!({
        "model": "claude-sonnet-5",
        "stream": true,
        "instructions": "Be terse.",
        "input": [
            { "role": "user", "content": [{ "type": "input_text", "text": "list the files" }] },
            {
                "type": "function_call",
                "call_id": "call_9",
                "name": "bash",
                "arguments": "{\"cmd\":\"ls\"}",
            },
            { "type": "function_call_output", "call_id": "call_9", "output": "a.txt" },
        ],
        "tools": [{
            "type": "function",
            "name": "bash",
            "description": "run it",
            "parameters": { "type": "object", "properties": {} },
        }],
        "max_output_tokens": 2048,
        "temperature": 0.4,
    });
    for (name, value) in undocumented() {
        request[name] = value;
    }

    let out = responses_to_messages::translate(
        &serde_json::to_vec(&request).unwrap(),
        &anthropic_target(),
        &envelopes(),
    )
    .unwrap();
    let sent: Value = serde_json::from_slice(&out.body).unwrap();

    let dropped: Vec<String> = out
        .mapping
        .dropped
        .iter()
        .map(|note| note.pointer.clone())
        .collect();

    let vanished = unaccounted(&request, &dropped);
    assert!(
        vanished.is_empty(),
        "these went missing without being recorded: {vanished:?}\nrecorded drops: {dropped:?}"
    );
    assert!(
        !String::from_utf8_lossy(&out.body).contains("obfuscation"),
        "a field we do not understand must not be forwarded as though we did"
    );
    assert_eq!(sent["messages"][1]["content"][0]["type"], "tool_use");
}

#[test]
fn a_messages_request_loses_nothing_it_does_not_write_down() {
    let mut request = json!({
        "model": "kimi-for-coding",
        "max_tokens": 4096,
        "stream": true,
        "temperature": 0.2,
        "stop_sequences": ["\n\nHuman:"],
        "system": [{ "type": "text", "text": "Be terse." }],
        "tools": [{
            "name": "bash",
            "description": "run it",
            "input_schema": { "type": "object", "properties": { "cmd": { "type": "string" } } },
        }],
        "tool_choice": { "type": "auto" },
        "messages": [
            { "role": "user", "content": "list the files" },
            { "role": "assistant", "content": [
                { "type": "text", "text": "on it" },
                { "type": "tool_use", "id": "toolu_01A", "name": "bash", "input": { "cmd": "ls" } },
            ]},
            { "role": "user", "content": [
                { "type": "tool_result", "tool_use_id": "toolu_01A", "content": "a.txt" },
            ]},
        ],
    });
    for (name, value) in undocumented() {
        request[name] = value;
    }

    let out =
        messages_to_chat::translate(&serde_json::to_vec(&request).unwrap(), "kimi-for-coding")
            .unwrap();
    let sent: Value = serde_json::from_slice(&out.body).unwrap();

    let dropped: Vec<String> = out
        .mapping
        .dropped
        .iter()
        .map(|note| note.pointer.clone())
        .collect();

    let vanished = unaccounted(&request, &dropped);
    assert!(
        vanished.is_empty(),
        "these went missing without being recorded: {vanished:?}\nrecorded drops: {dropped:?}"
    );
    assert_eq!(sent["messages"][0]["content"], "Be terse.");
    assert_eq!(sent["messages"][3]["role"], "tool");
}

#[test]
fn a_body_edit_that_was_not_written_down_cannot_be_replayed() {
    let input = br#"{"system":[{"type":"text","text":"hi"}],"phase":"final_answer"}"#;
    let honest = vec![BodyEdit::Insert {
        pointer: "/system/0/cache_control".into(),
        value: json!({ "type": "ephemeral" }),
    }];
    let sent = apply(input, &honest).unwrap();

    for corrupted in [
        Record {
            input: BodyDigest::of(input),
            output: BodyDigest::of(&sent),
            body_edits: Vec::new(),
            header_edits: vec![HeaderEdit::new("x-api-key")],
        },
        Record {
            input: BodyDigest::of(input),
            output: BodyDigest::of(br#"{"something":"else"}"#),
            body_edits: honest.clone(),
            header_edits: Vec::new(),
        },
        Record {
            input: BodyDigest::of(br#"{"a":1}"#),
            output: BodyDigest::of(&sent),
            body_edits: honest.clone(),
            header_edits: Vec::new(),
        },
    ] {
        assert!(
            corrupted.verify(input).is_err(),
            "a record that does not reproduce what was sent must not verify"
        );
    }

    Record {
        input: BodyDigest::of(input),
        output: BodyDigest::of(&sent),
        body_edits: honest,
        header_edits: vec![HeaderEdit::new("x-api-key")],
    }
    .verify(input)
    .expect("an honest record verifies");
}

fn chat_stream() -> String {
    [
        r#"data: {"choices":[{"index":0,"delta":{"role":"assistant","reasoning_content":"weigh"}}]}"#,
        r#"data: {"choices":[{"index":0,"delta":{"reasoning_content":"ing it"}}]}"#,
        r#"data: {"choices":[{"index":0,"delta":{"content":"Running"}}]}"#,
        r#"data: {"choices":[{"index":0,"delta":{"content":" it."}}]}"#,
        r#"data: {"choices":[{"index":0,"delta":{"tool_calls":[{"index":0,"id":"call_9","function":{"name":"bash","arguments":"{\"cmd\""}}]}}]}"#,
        r#"data: {"choices":[{"index":0,"delta":{"tool_calls":[{"index":0,"function":{"arguments":":\"ls\"}"}}]}}]}"#,
        r#"data: {"choices":[{"index":0,"delta":{},"finish_reason":"tool_calls"}]}"#,
        r#"data: {"choices":[],"usage":{"prompt_tokens":1000,"completion_tokens":42,"prompt_tokens_details":{"cached_tokens":900}}}"#,
        "data: [DONE]",
    ]
    .join("\n\n")
        + "\n\n"
}

fn translate_in_pieces(text: &str, at: usize) -> Vec<Frame> {
    let mut translator = StreamTranslator::new(StreamTarget {
        message_id: "msg_1".into(),
        model: "kimi-for-coding".into(),
    });
    let bytes = text.as_bytes();
    let mut out = Vec::new();
    for piece in bytes.chunks(at.max(1)) {
        out.extend(translator.push(piece));
    }
    out.extend(translator.finish());
    Parser::default().push(&out)
}

#[test]
fn where_the_network_split_the_bytes_changes_nothing() {
    let text = chat_stream();
    let whole = translate_in_pieces(&text, text.len());

    for at in [1, 2, 3, 7, 13, 64, 511] {
        assert_eq!(
            translate_in_pieces(&text, at),
            whole,
            "splitting the provider's stream every {at} bytes changed what the client received"
        );
    }
}

#[test]
fn what_the_stream_said_and_what_it_assembled_are_the_same_thing() {
    let text = chat_stream();
    let mut translator = StreamTranslator::new(StreamTarget {
        message_id: "msg_1".into(),
        model: "kimi-for-coding".into(),
    });
    let mut out = Vec::new();
    out.extend(translator.push(text.as_bytes()));
    out.extend(translator.finish());
    let whole = translator.assembled();

    let mut spoken = String::new();
    let mut thought = String::new();
    let mut arguments = String::new();
    let mut stop = None;

    for frame in Parser::default().push(&out) {
        let Some(event) = frame.json() else { continue };
        if let Some(text) = event["delta"]["text"].as_str() {
            spoken.push_str(text);
        }
        if let Some(text) = event["delta"]["thinking"].as_str() {
            thought.push_str(text);
        }
        if let Some(text) = event["delta"]["partial_json"].as_str() {
            arguments.push_str(text);
        }
        if let Some(reason) = event["delta"]["stop_reason"].as_str() {
            stop = Some(reason.to_owned());
        }
    }

    assert_eq!(whole["content"][0]["thinking"], thought);
    assert_eq!(whole["content"][1]["text"], spoken);
    assert_eq!(
        whole["content"][2]["input"],
        serde_json::from_str::<Value>(&arguments).unwrap()
    );
    assert_eq!(whole["stop_reason"], stop.unwrap());
}

#[test]
fn the_tokens_the_provider_counted_are_the_tokens_we_report() {
    let text = chat_stream();
    let mut translator = StreamTranslator::new(StreamTarget {
        message_id: "msg_1".into(),
        model: "kimi-for-coding".into(),
    });
    translator.push(text.as_bytes());
    translator.finish();

    let whole = translator.assembled();
    let input = whole["usage"]["input_tokens"].as_i64().unwrap();
    let cached = whole["usage"]["cache_read_input_tokens"].as_i64().unwrap();

    assert_eq!(
        input + cached,
        1000,
        "the provider billed 1000 prompt tokens; splitting them must not change the total"
    );
    assert_eq!(cached, 900);
    assert_eq!(whole["usage"]["output_tokens"], 42);
}

#[test]
fn a_tool_id_survives_the_round_trip_unchanged() {
    let request = json!({
        "messages": [
            { "role": "assistant", "content": [
                { "type": "tool_use", "id": "toolu_01XyZ-_9", "name": "bash", "input": {} },
            ]},
            { "role": "user", "content": [
                { "type": "tool_result", "tool_use_id": "toolu_01XyZ-_9", "content": "ok" },
            ]},
        ],
    });

    let out =
        messages_to_chat::translate(&serde_json::to_vec(&request).unwrap(), "kimi-for-coding")
            .unwrap();
    let sent: Value = serde_json::from_slice(&out.body).unwrap();

    assert_eq!(sent["messages"][0]["tool_calls"][0]["id"], "toolu_01XyZ-_9");
    assert_eq!(sent["messages"][1]["tool_call_id"], "toolu_01XyZ-_9");

    let coming_back = format!(
        "data: {}\n\ndata: [DONE]\n\n",
        json!({
            "choices": [{
                "index": 0,
                "delta": { "tool_calls": [{ "index": 0, "id": "call_returned_9", "function": { "name": "bash", "arguments": "{}" } }] },
                "finish_reason": "tool_calls",
            }],
        })
    );
    let mut translator = StreamTranslator::new(StreamTarget {
        message_id: "msg_1".into(),
        model: "kimi-for-coding".into(),
    });
    translator.push(coming_back.as_bytes());
    translator.finish();

    assert_eq!(
        translator.assembled()["content"][0]["id"],
        "call_returned_9",
        "an id the provider chose is what the client has to send back"
    );
}

#[test]
fn nothing_ever_invents_a_signature() {
    let request = json!({
        "messages": [{ "role": "assistant", "content": [
            { "type": "thinking", "thinking": "earlier", "signature": "EqQBCgIYAhIM1gbc" },
        ]}],
    });
    let out =
        messages_to_chat::translate(&serde_json::to_vec(&request).unwrap(), "kimi-for-coding")
            .unwrap();
    assert!(!String::from_utf8_lossy(&out.body).contains("signature"));

    let thinking = format!(
        "data: {}\n\ndata: [DONE]\n\n",
        json!({
            "choices": [{ "index": 0, "delta": { "reasoning_content": "fresh" }, "finish_reason": "stop" }],
        })
    );
    let mut translator = StreamTranslator::new(StreamTarget {
        message_id: "msg_1".into(),
        model: "kimi-for-coding".into(),
    });
    let emitted =
        String::from_utf8([translator.push(thinking.as_bytes()), translator.finish()].concat())
            .unwrap();

    assert!(emitted.contains("thinking_delta"));
    assert!(
        !emitted.contains("signature"),
        "a provider that mints no signature must not appear to have minted one"
    );
}
