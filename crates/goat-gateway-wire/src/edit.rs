use serde_json::Value;

use crate::digest::BodyDigest;

#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum BodyEdit {
    Insert { pointer: String, value: Value },
}

impl BodyEdit {
    pub fn describe(&self) -> String {
        match self {
            Self::Insert { pointer, .. } => format!("inserted {pointer}"),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct HeaderEdit {
    pub name: String,
}

impl HeaderEdit {
    pub fn new(name: impl Into<String>) -> Self {
        Self { name: name.into() }
    }
}

#[derive(Debug, thiserror::Error)]
pub enum EditError {
    #[error("body is not valid JSON: {0}")]
    Json(#[from] serde_json::Error),
    #[error("pointer {pointer:?} has no parent to insert into")]
    NoParent { pointer: String },
    #[error("pointer {pointer:?} does not resolve")]
    Unresolvable { pointer: String },
    #[error("pointer {pointer:?} already exists; insert would overwrite")]
    WouldOverwrite { pointer: String },
}

pub fn apply(input: &[u8], edits: &[BodyEdit]) -> Result<Vec<u8>, EditError> {
    if edits.is_empty() {
        return Ok(input.to_vec());
    }

    let mut doc: Value = serde_json::from_slice(input)?;
    for edit in edits {
        match edit {
            BodyEdit::Insert { pointer, value } => insert(&mut doc, pointer, value.clone())?,
        }
    }
    Ok(serde_json::to_vec(&doc)?)
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct Record {
    pub input: BodyDigest,
    pub output: BodyDigest,
    pub body_edits: Vec<BodyEdit>,
    pub header_edits: Vec<HeaderEdit>,
}

impl Record {
    pub fn is_byte_identical(&self) -> bool {
        self.body_edits.is_empty() && self.input == self.output
    }

    pub fn verify(&self, input_body: &[u8]) -> Result<(), VerifyError> {
        if BodyDigest::of(input_body) != self.input {
            return Err(VerifyError::InputMismatch);
        }
        let replayed = apply(input_body, &self.body_edits).map_err(VerifyError::Replay)?;
        if BodyDigest::of(&replayed) != self.output {
            return Err(VerifyError::Unrecorded);
        }
        Ok(())
    }
}

#[derive(Debug, thiserror::Error)]
pub enum VerifyError {
    #[error("the body handed to verify() is not the one that was recorded")]
    InputMismatch,
    #[error("recorded edits could not be replayed: {0}")]
    Replay(#[source] EditError),
    #[error("replaying the recorded edits does not reproduce what was sent")]
    Unrecorded,
}

fn insert(doc: &mut Value, pointer: &str, value: Value) -> Result<(), EditError> {
    let (parent_pointer, key) = split_pointer(pointer)?;
    let parent = doc
        .pointer_mut(&parent_pointer)
        .ok_or_else(|| EditError::Unresolvable {
            pointer: parent_pointer.clone(),
        })?;

    match parent {
        Value::Object(map) => {
            if map.contains_key(&key) {
                return Err(EditError::WouldOverwrite {
                    pointer: pointer.to_owned(),
                });
            }
            map.insert(key, value);
            Ok(())
        }
        Value::Array(items) => {
            let index = if key == "-" {
                items.len()
            } else {
                key.parse::<usize>().map_err(|_| EditError::Unresolvable {
                    pointer: pointer.to_owned(),
                })?
            };
            if index > items.len() {
                return Err(EditError::Unresolvable {
                    pointer: pointer.to_owned(),
                });
            }
            items.insert(index, value);
            Ok(())
        }
        _ => Err(EditError::Unresolvable {
            pointer: parent_pointer,
        }),
    }
}

fn split_pointer(pointer: &str) -> Result<(String, String), EditError> {
    let cut = pointer.rfind('/').ok_or_else(|| EditError::NoParent {
        pointer: pointer.to_owned(),
    })?;
    let (parent, last) = pointer.split_at(cut);
    let key = last[1..].replace("~1", "/").replace("~0", "~");
    Ok((parent.to_owned(), key))
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn cache_control_at(pointer: &str) -> BodyEdit {
        BodyEdit::Insert {
            pointer: pointer.into(),
            value: json!({ "type": "ephemeral" }),
        }
    }

    fn record(input: &[u8], edits: Vec<BodyEdit>) -> Record {
        let output = apply(input, &edits).unwrap();
        Record {
            input: BodyDigest::of(input),
            output: BodyDigest::of(&output),
            body_edits: edits,
            header_edits: vec![HeaderEdit::new("x-api-key")],
        }
    }

    #[test]
    fn no_edits_means_the_original_bytes_go_out() {
        let input = br#"{ "model" :"claude-sonnet-5",  "max_tokens":1 }"#;
        assert_eq!(apply(input, &[]).unwrap(), input);

        let record = record(input, vec![]);
        assert!(record.is_byte_identical());
        record.verify(input).unwrap();
    }

    #[test]
    fn undocumented_fields_survive_an_edit() {
        let input = br#"{"model":"m","namespace":"collaboration","phase":"final_answer","system":[{"type":"text","text":"hi"}]}"#;
        let out = apply(input, &[cache_control_at("/system/0/cache_control")]).unwrap();
        let parsed: Value = serde_json::from_slice(&out).unwrap();

        assert_eq!(parsed["namespace"], "collaboration");
        assert_eq!(parsed["phase"], "final_answer");
        assert_eq!(parsed["system"][0]["cache_control"]["type"], "ephemeral");
    }

    #[test]
    fn verify_accepts_a_faithful_record() {
        let input = br#"{"system":[{"type":"text","text":"hi"}]}"#;
        let record = record(input, vec![cache_control_at("/system/0/cache_control")]);
        assert!(!record.is_byte_identical());
        record.verify(input).unwrap();
    }

    #[test]
    fn verify_catches_a_change_we_did_not_write_down() {
        let input = br#"{"system":[{"type":"text","text":"hi"}],"extra":1}"#;

        let mut doc: Value = serde_json::from_slice(input).unwrap();
        doc["system"][0]["cache_control"] = json!({ "type": "ephemeral" });
        doc.as_object_mut().unwrap().remove("extra");
        let actually_sent = serde_json::to_vec(&doc).unwrap();

        let record = Record {
            input: BodyDigest::of(input),
            output: BodyDigest::of(&actually_sent),
            body_edits: vec![cache_control_at("/system/0/cache_control")],
            header_edits: vec![],
        };

        assert!(matches!(record.verify(input), Err(VerifyError::Unrecorded)));
    }

    #[test]
    fn insert_refuses_to_overwrite() {
        let input = br#"{"system":[{"type":"text","cache_control":{"type":"ephemeral"}}]}"#;
        let err = apply(input, &[cache_control_at("/system/0/cache_control")]).unwrap_err();
        assert!(matches!(err, EditError::WouldOverwrite { .. }));
    }

    #[test]
    fn key_order_is_preserved_through_an_edit() {
        let input = br#"{"z":1,"a":2,"m":3}"#;
        let out = apply(
            input,
            &[BodyEdit::Insert {
                pointer: "/new".into(),
                value: json!(true),
            }],
        )
        .unwrap();
        assert_eq!(
            String::from_utf8(out).unwrap(),
            r#"{"z":1,"a":2,"m":3,"new":true}"#
        );
    }

    #[test]
    fn pointer_escapes_are_honored() {
        let input = br#"{"a/b":{}}"#;
        let out = apply(
            input,
            &[BodyEdit::Insert {
                pointer: "/a~1b/x".into(),
                value: json!(1),
            }],
        )
        .unwrap();
        let parsed: Value = serde_json::from_slice(&out).unwrap();
        assert_eq!(parsed["a/b"]["x"], 1);
    }
}
