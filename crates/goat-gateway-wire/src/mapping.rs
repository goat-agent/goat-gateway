use serde::{Deserialize, Serialize};

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
