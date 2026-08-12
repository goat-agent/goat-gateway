use serde::{Deserialize, Serialize};

#[derive(Debug)]
pub struct Translated {
    pub body: Vec<u8>,
    pub mapping: Mapping,
}

#[derive(Debug, thiserror::Error)]
pub enum TranslateError {
    #[error("request is not valid JSON: {0}")]
    Json(#[from] serde_json::Error),
    #[error(
        "{what} has no counterpart in the {wire} format; refusing to send a request that silently loses it"
    )]
    NoCounterpart { what: String, wire: &'static str },
    #[error("{what} is malformed: {detail}")]
    Malformed { what: String, detail: String },
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Added {
    pub pointer: String,
    pub why: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Dropped {
    pub pointer: String,
    pub why: String,
    pub reissued_as: Option<String>,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Mapping {
    pub moved: usize,
    pub added: Vec<Added>,
    pub dropped: Vec<Dropped>,
}

impl Mapping {
    pub fn mind_the_rest(&mut self, source: &serde_json::Value, consumed: &[&str], wire: &str) {
        let Some(fields) = source.as_object() else {
            return;
        };
        for name in fields.keys() {
            if consumed.contains(&name.as_str()) {
                continue;
            }
            self.dropped(
                format!("/{name}"),
                format!("this gateway does not know what {name} means in the {wire} format"),
            );
        }
    }

    pub fn absorb(&mut self, other: Self) {
        self.moved += other.moved;
        self.added.extend(other.added);
        self.dropped.extend(other.dropped);
    }

    pub fn moved(&mut self) {
        self.moved += 1;
    }

    pub fn added(&mut self, pointer: impl Into<String>, why: impl Into<String>) {
        self.added.push(Added {
            pointer: pointer.into(),
            why: why.into(),
        });
    }

    pub fn dropped(&mut self, pointer: impl Into<String>, why: impl Into<String>) {
        self.dropped.push(Dropped {
            pointer: pointer.into(),
            why: why.into(),
            reissued_as: None,
        });
    }

    pub fn reissued(
        &mut self,
        pointer: impl Into<String>,
        why: impl Into<String>,
        as_: impl Into<String>,
    ) {
        self.dropped.push(Dropped {
            pointer: pointer.into(),
            why: why.into(),
            reissued_as: Some(as_.into()),
        });
    }

    pub fn lost_anything(&self) -> bool {
        self.dropped.iter().any(|d| d.reissued_as.is_none())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_reissue_is_not_a_loss() {
        let mut mapping = Mapping::default();
        mapping.reissued(
            "/input/2/encrypted_content",
            "organization-bound at the source",
            "/messages/1/content/0/signature",
        );
        assert!(!mapping.lost_anything());
    }

    #[test]
    fn a_plain_drop_is_a_loss() {
        let mut mapping = Mapping::default();
        mapping.dropped("/input/2/something", "no counterpart");
        assert!(mapping.lost_anything());
    }
}
