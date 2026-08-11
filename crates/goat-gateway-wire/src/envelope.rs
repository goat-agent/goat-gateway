use base64::{Engine as _, engine::general_purpose::URL_SAFE_NO_PAD as B64};
use chacha20poly1305::{
    ChaCha20Poly1305, Key, Nonce,
    aead::{Aead, KeyInit},
};
use serde::{Deserialize, Serialize};

pub const PREFIX: &str = "gwe1_";

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Provenance {
    pub provider: String,
    pub account: String,
    pub model: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Portability {
    Endpoint,
    Account,
    Session,
    Portable,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "t", rename_all = "snake_case")]
pub enum Payload {
    Thinking { thinking: String, signature: String },
    RedactedThinking { data: String },
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Sealed {
    pub provenance: Provenance,
    pub portability: Portability,
    pub payload: Payload,
}

#[derive(Debug, thiserror::Error)]
pub enum EnvelopeError {
    #[error("not one of our envelopes")]
    NotOurs,
    #[error("envelope is not valid base64")]
    Base64,
    #[error("envelope is too short to contain a nonce")]
    Truncated,
    #[error("envelope failed authentication; it was not sealed with this key")]
    Authentication,
    #[error("envelope contents are malformed: {0}")]
    Malformed(#[from] serde_json::Error),
    #[error(
        "envelope was minted by {minted:?} but this request goes to {target:?}; it cannot be replayed there"
    )]
    WrongIdentity { minted: String, target: String },
}

#[derive(Clone)]
pub struct Envelopes {
    cipher: ChaCha20Poly1305,
}

impl std::fmt::Debug for Envelopes {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("Envelopes(***)")
    }
}

impl Envelopes {
    pub fn new(key: &[u8; 32]) -> Self {
        Self {
            cipher: ChaCha20Poly1305::new(Key::from_slice(key)),
        }
    }

    pub fn seal(&self, nonce: [u8; 12], sealed: &Sealed) -> String {
        let plaintext = serde_json::to_vec(sealed).expect("Sealed is always serializable");
        let ciphertext = self
            .cipher
            .encrypt(Nonce::from_slice(&nonce), plaintext.as_ref())
            .expect("ChaCha20Poly1305 encryption is infallible for in-memory input");

        let mut blob = Vec::with_capacity(12 + ciphertext.len());
        blob.extend_from_slice(&nonce);
        blob.extend_from_slice(&ciphertext);
        format!("{PREFIX}{}", B64.encode(blob))
    }

    pub fn open(&self, blob: &str) -> Result<Sealed, EnvelopeError> {
        let encoded = blob.strip_prefix(PREFIX).ok_or(EnvelopeError::NotOurs)?;
        let raw = B64.decode(encoded).map_err(|_| EnvelopeError::Base64)?;
        if raw.len() < 12 {
            return Err(EnvelopeError::Truncated);
        }
        let (nonce, ciphertext) = raw.split_at(12);
        let plaintext = self
            .cipher
            .decrypt(Nonce::from_slice(nonce), ciphertext)
            .map_err(|_| EnvelopeError::Authentication)?;
        Ok(serde_json::from_slice(&plaintext)?)
    }

    pub fn open_for(&self, blob: &str, target: &Provenance) -> Result<Sealed, EnvelopeError> {
        let sealed = self.open(blob)?;
        if sealed.is_replayable_at(target) {
            Ok(sealed)
        } else {
            Err(EnvelopeError::WrongIdentity {
                minted: sealed.provenance.identity(),
                target: target.identity(),
            })
        }
    }
}

impl Provenance {
    pub fn identity(&self) -> String {
        format!("{}/{}/{}", self.provider, self.account, self.model)
    }
}

impl Sealed {
    pub fn is_replayable_at(&self, target: &Provenance) -> bool {
        match self.portability {
            Portability::Portable => true,
            Portability::Session | Portability::Account => {
                self.provenance.provider == target.provider
                    && self.provenance.account == target.account
            }
            Portability::Endpoint => self.provenance == *target,
        }
    }
}

pub fn is_ours(blob: &str) -> bool {
    blob.starts_with(PREFIX)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn envelopes() -> Envelopes {
        Envelopes::new(&[7u8; 32])
    }

    fn provenance(account: &str) -> Provenance {
        Provenance {
            provider: "anthropic".into(),
            account: account.into(),
            model: "claude-sonnet-5".into(),
        }
    }

    fn sealed(account: &str, portability: Portability) -> Sealed {
        Sealed {
            provenance: provenance(account),
            portability,
            payload: Payload::Thinking {
                thinking: "weighing the options".into(),
                signature: "EvpRCokBCBAYAipADlQ397V3hO8sP2WxUUU8".into(),
            },
        }
    }

    #[test]
    fn payload_round_trips_byte_exact() {
        let envelopes = envelopes();
        let original = sealed("personal", Portability::Account);
        let blob = envelopes.seal([1u8; 12], &original);
        assert_eq!(envelopes.open(&blob).unwrap(), original);
    }

    #[test]
    fn a_huge_signature_survives() {
        let envelopes = envelopes();
        let original = Sealed {
            provenance: provenance("personal"),
            portability: Portability::Account,
            payload: Payload::Thinking {
                thinking: String::new(),
                signature: "A".repeat(76_412),
            },
        };
        let blob = envelopes.seal([2u8; 12], &original);
        assert_eq!(envelopes.open(&blob).unwrap(), original);
    }

    #[test]
    fn redacted_thinking_survives() {
        let envelopes = envelopes();
        let original = Sealed {
            provenance: provenance("personal"),
            portability: Portability::Account,
            payload: Payload::RedactedThinking {
                data: "opaque-vendor-bytes".into(),
            },
        };
        let blob = envelopes.seal([3u8; 12], &original);
        assert_eq!(envelopes.open(&blob).unwrap(), original);
    }

    #[test]
    fn a_tampered_envelope_is_rejected_not_silently_dropped() {
        let envelopes = envelopes();
        let blob = envelopes.seal([4u8; 12], &sealed("personal", Portability::Account));
        let mut corrupted = blob.clone();
        corrupted.pop();
        corrupted.push(if blob.ends_with('A') { 'B' } else { 'A' });

        assert!(matches!(
            envelopes.open(&corrupted),
            Err(EnvelopeError::Authentication | EnvelopeError::Base64)
        ));
    }

    #[test]
    fn another_gateways_key_cannot_open_ours() {
        let blob = envelopes().seal([5u8; 12], &sealed("personal", Portability::Account));
        let stranger = Envelopes::new(&[9u8; 32]);
        assert!(matches!(
            stranger.open(&blob),
            Err(EnvelopeError::Authentication)
        ));
    }

    #[test]
    fn account_bound_state_refuses_a_different_account() {
        let envelopes = envelopes();
        let blob = envelopes.seal([6u8; 12], &sealed("personal", Portability::Account));

        envelopes.open_for(&blob, &provenance("personal")).unwrap();

        let err = envelopes.open_for(&blob, &provenance("work")).unwrap_err();
        assert!(matches!(err, EnvelopeError::WrongIdentity { .. }));
    }

    #[test]
    fn portable_state_crosses_accounts() {
        let envelopes = envelopes();
        let blob = envelopes.seal([7u8; 12], &sealed("personal", Portability::Portable));
        envelopes.open_for(&blob, &provenance("work")).unwrap();
    }

    #[test]
    fn endpoint_bound_state_refuses_a_different_model() {
        let envelopes = envelopes();
        let blob = envelopes.seal([8u8; 12], &sealed("personal", Portability::Endpoint));
        let mut target = provenance("personal");
        target.model = "claude-opus-5".into();
        assert!(matches!(
            envelopes.open_for(&blob, &target),
            Err(EnvelopeError::WrongIdentity { .. })
        ));
    }

    #[test]
    fn a_vendor_blob_is_not_mistaken_for_ours() {
        assert!(!is_ours("gAAAAABqegSk-JVUzE1Tb7LXXvQc0JY4mFPnWTYW"));
        assert!(is_ours(
            &envelopes().seal([0u8; 12], &sealed("p", Portability::Account))
        ));
        assert!(matches!(
            envelopes().open("gAAAAABqegSk-JVUzE1Tb7LX"),
            Err(EnvelopeError::NotOurs)
        ));
    }
}
