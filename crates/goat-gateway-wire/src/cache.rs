use serde_json::{Value, json};

use crate::edit::BodyEdit;

const BYTES_PER_TOKEN: usize = 4;
const LOOKBACK_BLOCKS: usize = 20;

pub fn breakpoints(request: &Value, min_tokens: u32) -> Vec<BodyEdit> {
    if min_tokens == 0 || already_marked(request) {
        return Vec::new();
    }

    let floor = min_tokens as usize * BYTES_PER_TOKEN;
    points(request)
        .into_iter()
        .filter(|(_, carried)| *carried >= floor)
        .map(|(pointer, _)| BodyEdit::Insert {
            pointer,
            value: json!({ "type": "ephemeral" }),
        })
        .collect()
}

fn points(request: &Value) -> Vec<(String, usize)> {
    let mut points = Vec::new();
    let mut carried = 0usize;

    for field in ["tools", "system"] {
        let Some(value) = request.get(field) else {
            continue;
        };
        carried += value.to_string().len();
        if let Some(last) = value.as_array().and_then(|items| plain_last(items)) {
            points.push((format!("/{field}/{last}/cache_control"), carried));
        }
    }

    let Some(messages) = request.get("messages").and_then(Value::as_array) else {
        return points;
    };
    let Some(last) = messages.len().checked_sub(1) else {
        return points;
    };
    let previous = messages
        .len()
        .checked_sub(3)
        .filter(|&start| blocks_in(&messages[start..]) <= LOOKBACK_BLOCKS);

    let mut from = 0;
    for index in [previous, Some(last)].into_iter().flatten() {
        carried += weigh(&messages[from..=index]);
        from = index + 1;
        if let Some(pointer) = block_pointer(&messages[index], index) {
            points.push((pointer, carried));
        }
    }

    points
}

fn block_pointer(message: &Value, index: usize) -> Option<String> {
    let content = message.get("content")?.as_array()?;
    let block = plain_last(content)?;
    Some(format!("/messages/{index}/content/{block}/cache_control"))
}

fn plain_last(items: &[Value]) -> Option<usize> {
    let last = items.len().checked_sub(1)?;
    items[last].get("cache_control").is_none().then_some(last)
}

fn weigh(values: &[Value]) -> usize {
    values.iter().map(|value| value.to_string().len()).sum()
}

fn blocks_in(messages: &[Value]) -> usize {
    messages
        .iter()
        .map(|message| {
            message
                .get("content")
                .and_then(Value::as_array)
                .map_or(1, Vec::len)
        })
        .sum()
}

fn already_marked(value: &Value) -> bool {
    match value {
        Value::Object(map) => map.contains_key("cache_control") || map.values().any(already_marked),
        Value::Array(items) => items.iter().any(already_marked),
        _ => false,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn text(size: usize) -> Value {
        json!({ "type": "text", "text": "x".repeat(size) })
    }

    fn message(role: &str, size: usize) -> Value {
        json!({ "role": role, "content": [text(size)] })
    }

    fn pointers(request: &Value, min_tokens: u32) -> Vec<String> {
        breakpoints(request, min_tokens)
            .into_iter()
            .map(|edit| match edit {
                BodyEdit::Insert { pointer, .. } => pointer,
            })
            .collect()
    }

    #[test]
    fn a_prompt_too_small_to_cache_is_left_alone() {
        let request = json!({
            "system": [text(40)],
            "messages": [message("user", 40)],
        });
        assert!(pointers(&request, 1024).is_empty());
    }

    #[test]
    fn the_stable_prefix_and_both_turn_boundaries_get_marked() {
        let request = json!({
            "tools": [json!({ "name": "bash", "description": "x".repeat(8000) })],
            "system": [text(8000)],
            "messages": [
                message("user", 8000),
                message("assistant", 8000),
                message("user", 8000),
                message("assistant", 8000),
                message("user", 8000),
            ],
        });

        assert_eq!(
            pointers(&request, 1024),
            [
                "/tools/0/cache_control",
                "/system/0/cache_control",
                "/messages/2/content/0/cache_control",
                "/messages/4/content/0/cache_control",
            ]
        );
    }

    #[test]
    fn four_is_the_most_anthropic_accepts_and_the_most_we_ask_for() {
        let messages: Vec<Value> = (0..40).map(|_| message("user", 8000)).collect();
        let request = json!({
            "tools": [json!({ "name": "bash", "description": "x".repeat(8000) })],
            "system": [text(8000)],
            "messages": messages,
        });
        assert!(pointers(&request, 1024).len() <= 4);
    }

    #[test]
    fn a_client_that_manages_its_own_cache_is_not_second_guessed() {
        let request = json!({
            "system": [json!({
                "type": "text",
                "text": "x".repeat(8000),
                "cache_control": { "type": "ephemeral" },
            })],
            "messages": [message("user", 8000), message("user", 8000)],
        });
        assert!(pointers(&request, 1024).is_empty());
    }

    #[test]
    fn a_far_older_boundary_is_out_of_reach_and_left_unmarked() {
        let long: Value = json!({
            "role": "user",
            "content": (0..30).map(|_| text(400)).collect::<Vec<_>>(),
        });
        let request = json!({
            "messages": [message("user", 8000), long, message("user", 8000)],
        });
        assert_eq!(
            pointers(&request, 1024),
            ["/messages/2/content/0/cache_control"]
        );
    }

    #[test]
    fn a_model_that_cannot_cache_is_never_edited() {
        let request = json!({
            "system": [text(80000)],
            "messages": [message("user", 80000)],
        });
        assert!(pointers(&request, 0).is_empty());
    }

    #[test]
    fn a_string_system_prompt_has_nowhere_to_put_a_breakpoint() {
        let request = json!({
            "system": "x".repeat(8000),
            "messages": [message("user", 8000)],
        });
        assert_eq!(
            pointers(&request, 1024),
            ["/messages/0/content/0/cache_control"]
        );
    }
}
