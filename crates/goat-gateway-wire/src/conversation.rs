use serde_json::Value;

use crate::digest::BodyDigest;

pub fn identify(request: &Value) -> Option<String> {
    let opening = opening(request)?;
    let mut seed = Vec::new();
    for part in [request.get("tools"), request.get("system"), Some(opening)]
        .into_iter()
        .flatten()
    {
        seed.extend_from_slice(part.to_string().as_bytes());
    }
    Some(format!("conv_{}", &BodyDigest::of(&seed).hex()[..24]))
}

fn opening(request: &Value) -> Option<&Value> {
    ["messages", "input"]
        .into_iter()
        .find_map(|field| request.get(field)?.as_array()?.first())
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn turn(question: &str, answers: usize) -> Value {
        let mut messages = vec![json!({ "role": "user", "content": question })];
        for index in 0..answers {
            messages.push(json!({ "role": "assistant", "content": index.to_string() }));
            messages.push(json!({ "role": "user", "content": "and then?" }));
        }
        json!({
            "model": "claude-sonnet-5",
            "system": [{ "type": "text", "text": "You are helpful." }],
            "messages": messages,
        })
    }

    #[test]
    fn a_conversation_keeps_its_identity_as_it_grows() {
        let first = identify(&turn("how do i sort in rust", 0)).unwrap();
        let later = identify(&turn("how do i sort in rust", 6)).unwrap();
        assert_eq!(first, later);
    }

    #[test]
    fn a_different_opening_is_a_different_conversation() {
        assert_ne!(
            identify(&turn("how do i sort in rust", 0)),
            identify(&turn("how do i sort in go", 0))
        );
    }

    #[test]
    fn changing_the_tools_starts_a_new_conversation() {
        let mut with_tools = turn("hello", 2);
        with_tools["tools"] = json!([{ "name": "bash" }]);
        assert_ne!(identify(&with_tools), identify(&turn("hello", 2)));
    }

    #[test]
    fn a_request_with_nothing_to_hash_has_no_identity() {
        assert_eq!(identify(&json!({ "model": "claude-sonnet-5" })), None);
        assert_eq!(identify(&json!({ "messages": [] })), None);
    }

    #[test]
    fn the_responses_shape_is_identified_the_same_way() {
        let request = json!({
            "model": "gpt-5",
            "input": [{ "role": "user", "content": "hello" }],
        });
        assert!(identify(&request).is_some());
    }
}
