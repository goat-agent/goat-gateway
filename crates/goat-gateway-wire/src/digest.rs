use std::fmt;

use sha2::{Digest as _, Sha256};

#[derive(Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct BodyDigest {
    bytes: [u8; 32],
    len: usize,
}

impl BodyDigest {
    pub fn of(body: &[u8]) -> Self {
        let mut hasher = Sha256::new();
        hasher.update(body);
        Self {
            bytes: hasher.finalize().into(),
            len: body.len(),
        }
    }

    pub fn len(&self) -> usize {
        self.len
    }

    pub fn is_empty(&self) -> bool {
        self.len == 0
    }

    pub fn hex(&self) -> String {
        let mut out = String::with_capacity(64);
        for byte in self.bytes {
            use fmt::Write as _;
            let _ = write!(out, "{byte:02x}");
        }
        out
    }

    pub fn short(&self) -> String {
        let hex = self.hex();
        format!("sha256:{}…{}", &hex[..4], &hex[hex.len() - 4..])
    }
}

impl fmt::Debug for BodyDigest {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{} ({} bytes)", self.short(), self.len)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn identical_bytes_produce_identical_digests() {
        assert_eq!(BodyDigest::of(b"{\"a\":1}"), BodyDigest::of(b"{\"a\":1}"));
    }

    #[test]
    fn reformatting_is_a_change() {
        assert_ne!(BodyDigest::of(b"{\"a\":1}"), BodyDigest::of(b"{\"a\": 1}"));
    }

    #[test]
    fn short_form_is_readable() {
        let digest = BodyDigest::of(b"hello");
        let hex = digest.hex();
        assert_eq!(hex.len(), 64);
        assert_eq!(
            digest.short(),
            format!("sha256:{}…{}", &hex[..4], &hex[60..])
        );
    }
}
