use rusqlite::{OptionalExtension as _, params};
use serde::{Deserialize, Serialize};

use crate::store::{Store, StoreError, now};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AccountState {
    Active,
    RateLimited,
    SignInExpired,
    Disabled,
}

impl AccountState {
    fn as_str(self) -> &'static str {
        match self {
            Self::Active => "active",
            Self::RateLimited => "rate_limited",
            Self::SignInExpired => "sign_in_expired",
            Self::Disabled => "disabled",
        }
    }

    fn parse(text: &str) -> Self {
        match text {
            "rate_limited" => Self::RateLimited,
            "sign_in_expired" => Self::SignInExpired,
            "disabled" => Self::Disabled,
            _ => Self::Active,
        }
    }
}

#[derive(Debug, Clone, Serialize)]
pub struct AccountRow {
    pub name: String,
    pub provider: String,
    pub credential_kind: String,
    pub state: AccountState,
    pub cooldown_until: Option<i64>,
    pub created_at: i64,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct Usage {
    pub input_tokens: Option<i64>,
    pub output_tokens: Option<i64>,
    pub cache_read_tokens: Option<i64>,
    pub cache_write_tokens: Option<i64>,
    pub reasoning_tokens: Option<i64>,
}

#[derive(Debug, Clone, Serialize)]
pub struct RequestRow {
    pub id: String,
    pub started_at: i64,
    pub person: Option<String>,
    pub client: Option<String>,
    pub provider: String,
    pub account: Option<String>,
    pub model: String,
    pub ingress: String,
    pub egress: String,
    pub translated: bool,
    pub status: String,
    pub error_kind: Option<String>,
    pub ttft_ms: Option<i64>,
    pub duration_ms: Option<i64>,
    pub usage: Usage,
    pub cost_micros: Option<i64>,
    pub input_digest: Option<String>,
    pub output_digest: Option<String>,
    pub byte_identical: Option<bool>,
    pub evidence: Option<serde_json::Value>,
    pub upstream_request_id: Option<String>,
}

impl Store {
    pub fn add_account(
        &self,
        name: &str,
        provider: &str,
        credential_kind: &str,
        secret: &[u8],
    ) -> Result<(), StoreError> {
        let (ciphertext, nonce) = self.seal(secret);
        let connection = self.connection.lock().expect("store mutex");
        connection.execute(
            "INSERT INTO accounts (name, provider, credential_kind, secret, nonce, state, created_at)
             VALUES (?1, ?2, ?3, ?4, ?5, 'active', ?6)
             ON CONFLICT(name) DO UPDATE SET
                 provider = excluded.provider,
                 credential_kind = excluded.credential_kind,
                 secret = excluded.secret,
                 nonce = excluded.nonce,
                 state = 'active',
                 cooldown_until = NULL",
            params![name, provider, credential_kind, ciphertext, nonce, now()],
        )?;
        Ok(())
    }

    pub fn secret(&self, name: &str) -> Result<Vec<u8>, StoreError> {
        let connection = self.connection.lock().expect("store mutex");
        let row: Option<(Vec<u8>, Vec<u8>)> = connection
            .query_row(
                "SELECT secret, nonce FROM accounts WHERE name = ?1",
                params![name],
                |row| Ok((row.get(0)?, row.get(1)?)),
            )
            .optional()?;
        drop(connection);

        let (ciphertext, nonce) = row.ok_or_else(|| StoreError::UnknownAccount(name.to_owned()))?;
        self.unseal(name, &ciphertext, &nonce)
    }

    pub fn accounts(&self) -> Result<Vec<AccountRow>, StoreError> {
        let connection = self.connection.lock().expect("store mutex");
        let mut statement = connection.prepare(
            "SELECT name, provider, credential_kind, state, cooldown_until, created_at
             FROM accounts ORDER BY provider, name",
        )?;
        let rows = statement
            .query_map([], |row| {
                Ok(AccountRow {
                    name: row.get(0)?,
                    provider: row.get(1)?,
                    credential_kind: row.get(2)?,
                    state: AccountState::parse(&row.get::<_, String>(3)?),
                    cooldown_until: row.get(4)?,
                    created_at: row.get(5)?,
                })
            })?
            .collect::<Result<Vec<_>, _>>()?;
        Ok(rows)
    }

    pub fn remove_account(&self, name: &str) -> Result<(), StoreError> {
        let connection = self.connection.lock().expect("store mutex");
        connection.execute("DELETE FROM accounts WHERE name = ?1", params![name])?;
        Ok(())
    }

    pub fn set_state(
        &self,
        name: &str,
        state: AccountState,
        cooldown_until: Option<i64>,
    ) -> Result<(), StoreError> {
        let connection = self.connection.lock().expect("store mutex");
        connection.execute(
            "UPDATE accounts SET state = ?2, cooldown_until = ?3 WHERE name = ?1",
            params![name, state.as_str(), cooldown_until],
        )?;
        Ok(())
    }

    pub fn clear_expired_cooldowns(&self) -> Result<usize, StoreError> {
        let connection = self.connection.lock().expect("store mutex");
        let changed = connection.execute(
            "UPDATE accounts SET state = 'active', cooldown_until = NULL
             WHERE state = 'rate_limited' AND cooldown_until IS NOT NULL AND cooldown_until <= ?1",
            params![now()],
        )?;
        Ok(changed)
    }

    pub fn record_request(&self, row: &RequestRow) -> Result<(), StoreError> {
        let connection = self.connection.lock().expect("store mutex");
        connection.execute(
            "INSERT INTO requests (
                 id, started_at, person, client, provider, account, model,
                 ingress, egress, translated, status, error_kind,
                 ttft_ms, duration_ms,
                 input_tokens, output_tokens, cache_read_tokens, cache_write_tokens, reasoning_tokens,
                 cost_micros, input_digest, output_digest, byte_identical, evidence, upstream_request_id
             ) VALUES (
                 ?1, ?2, ?3, ?4, ?5, ?6, ?7,
                 ?8, ?9, ?10, ?11, ?12,
                 ?13, ?14,
                 ?15, ?16, ?17, ?18, ?19,
                 ?20, ?21, ?22, ?23, ?24, ?25
             )
             ON CONFLICT(id) DO UPDATE SET
                 status = excluded.status,
                 error_kind = excluded.error_kind,
                 ttft_ms = excluded.ttft_ms,
                 duration_ms = excluded.duration_ms,
                 input_tokens = excluded.input_tokens,
                 output_tokens = excluded.output_tokens,
                 cache_read_tokens = excluded.cache_read_tokens,
                 cache_write_tokens = excluded.cache_write_tokens,
                 reasoning_tokens = excluded.reasoning_tokens,
                 cost_micros = excluded.cost_micros,
                 evidence = excluded.evidence,
                 upstream_request_id = excluded.upstream_request_id",
            params![
                row.id,
                row.started_at,
                row.person,
                row.client,
                row.provider,
                row.account,
                row.model,
                row.ingress,
                row.egress,
                row.translated as i64,
                row.status,
                row.error_kind,
                row.ttft_ms,
                row.duration_ms,
                row.usage.input_tokens,
                row.usage.output_tokens,
                row.usage.cache_read_tokens,
                row.usage.cache_write_tokens,
                row.usage.reasoning_tokens,
                row.cost_micros,
                row.input_digest,
                row.output_digest,
                row.byte_identical.map(|flag| flag as i64),
                row.evidence.as_ref().map(ToString::to_string),
                row.upstream_request_id,
            ],
        )?;
        Ok(())
    }

    pub fn recent_requests(&self, limit: usize) -> Result<Vec<RequestRow>, StoreError> {
        let connection = self.connection.lock().expect("store mutex");
        let mut statement = connection.prepare(
            "SELECT id, started_at, person, client, provider, account, model,
                    ingress, egress, translated, status, error_kind,
                    ttft_ms, duration_ms,
                    input_tokens, output_tokens, cache_read_tokens, cache_write_tokens, reasoning_tokens,
                    cost_micros, input_digest, output_digest, byte_identical, evidence, upstream_request_id
             FROM requests ORDER BY started_at DESC LIMIT ?1",
        )?;
        let rows = statement
            .query_map(params![limit as i64], |row| {
                Ok(RequestRow {
                    id: row.get(0)?,
                    started_at: row.get(1)?,
                    person: row.get(2)?,
                    client: row.get(3)?,
                    provider: row.get(4)?,
                    account: row.get(5)?,
                    model: row.get(6)?,
                    ingress: row.get(7)?,
                    egress: row.get(8)?,
                    translated: row.get::<_, i64>(9)? != 0,
                    status: row.get(10)?,
                    error_kind: row.get(11)?,
                    ttft_ms: row.get(12)?,
                    duration_ms: row.get(13)?,
                    usage: Usage {
                        input_tokens: row.get(14)?,
                        output_tokens: row.get(15)?,
                        cache_read_tokens: row.get(16)?,
                        cache_write_tokens: row.get(17)?,
                        reasoning_tokens: row.get(18)?,
                    },
                    cost_micros: row.get(19)?,
                    input_digest: row.get(20)?,
                    output_digest: row.get(21)?,
                    byte_identical: row.get::<_, Option<i64>>(22)?.map(|flag| flag != 0),
                    evidence: row
                        .get::<_, Option<String>>(23)?
                        .and_then(|text| serde_json::from_str(&text).ok()),
                    upstream_request_id: row.get(24)?,
                })
            })?
            .collect::<Result<Vec<_>, _>>()?;
        Ok(rows)
    }
}

impl Store {
    pub fn set_rate_limits(
        &self,
        account: &str,
        snapshot: &crate::limits::Snapshot,
    ) -> Result<(), StoreError> {
        let json = serde_json::to_string(snapshot).unwrap_or_else(|_| "{}".to_owned());
        let connection = self.connection.lock().expect("store mutex");
        connection.execute(
            "INSERT INTO rate_limits (account, snapshot, updated_at) VALUES (?1, ?2, ?3)
             ON CONFLICT(account) DO UPDATE SET snapshot = excluded.snapshot,
                                                updated_at = excluded.updated_at",
            params![account, json, now()],
        )?;
        Ok(())
    }

    pub fn rate_limits(
        &self,
        account: &str,
    ) -> Result<Option<crate::limits::Snapshot>, StoreError> {
        let connection = self.connection.lock().expect("store mutex");
        let json: Option<String> = connection
            .query_row(
                "SELECT snapshot FROM rate_limits WHERE account = ?1",
                params![account],
                |row| row.get(0),
            )
            .optional()?;
        Ok(json.and_then(|text| serde_json::from_str(&text).ok()))
    }

    pub fn all_rate_limits(&self) -> Result<Vec<(String, crate::limits::Snapshot)>, StoreError> {
        let connection = self.connection.lock().expect("store mutex");
        let mut statement = connection.prepare("SELECT account, snapshot FROM rate_limits")?;
        let rows = statement
            .query_map([], |row| {
                let account: String = row.get(0)?;
                let json: String = row.get(1)?;
                Ok((account, json))
            })?
            .collect::<Result<Vec<_>, _>>()?;
        Ok(rows
            .into_iter()
            .filter_map(|(account, json)| {
                serde_json::from_str(&json)
                    .ok()
                    .map(|snapshot| (account, snapshot))
            })
            .collect())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn store() -> Store {
        Store::in_memory(&[2u8; 32]).unwrap()
    }

    #[test]
    fn a_secret_round_trips_through_encryption() {
        let store = store();
        store
            .add_account("Claude personal", "anthropic", "api_key", b"sk-secret")
            .unwrap();
        assert_eq!(store.secret("Claude personal").unwrap(), b"sk-secret");
    }

    #[test]
    fn the_secret_is_not_stored_in_the_clear() {
        let store = store();
        store
            .add_account("a", "anthropic", "api_key", b"sk-plaintext-marker")
            .unwrap();

        let connection = store.connection.lock().unwrap();
        let stored: Vec<u8> = connection
            .query_row("SELECT secret FROM accounts WHERE name = 'a'", [], |row| {
                row.get(0)
            })
            .unwrap();
        assert!(
            !stored.windows(19).any(|w| w == b"sk-plaintext-marker"),
            "the plaintext secret must not appear in the database"
        );
    }

    #[test]
    fn another_master_key_cannot_read_the_secrets() {
        let store = store();
        store
            .add_account("a", "anthropic", "api_key", b"sk")
            .unwrap();

        let stolen = Store {
            connection: store.connection.clone(),
            cipher: std::sync::Arc::new({
                use chacha20poly1305::{ChaCha20Poly1305, Key, aead::KeyInit};
                ChaCha20Poly1305::new(Key::from_slice(&[9u8; 32]))
            }),
        };
        assert!(matches!(
            stolen.secret("a"),
            Err(StoreError::Undecryptable { .. })
        ));
    }

    #[test]
    fn accounts_list_without_exposing_secrets() {
        let store = store();
        store
            .add_account("b", "anthropic", "api_key", b"x")
            .unwrap();
        store.add_account("a", "openai", "oauth", b"y").unwrap();

        let accounts = store.accounts().unwrap();
        assert_eq!(accounts.len(), 2);
        assert_eq!(accounts[0].provider, "anthropic");
        assert_eq!(accounts[0].state, AccountState::Active);

        let json = serde_json::to_string(&accounts).unwrap();
        assert!(!json.contains("secret"));
    }

    #[test]
    fn a_cooldown_expires_on_its_own() {
        let store = store();
        store
            .add_account("a", "anthropic", "api_key", b"x")
            .unwrap();
        store
            .set_state("a", AccountState::RateLimited, Some(now() - 1))
            .unwrap();

        assert_eq!(store.clear_expired_cooldowns().unwrap(), 1);
        assert_eq!(store.accounts().unwrap()[0].state, AccountState::Active);
    }

    #[test]
    fn a_cooldown_still_running_is_left_alone() {
        let store = store();
        store
            .add_account("a", "anthropic", "api_key", b"x")
            .unwrap();
        store
            .set_state("a", AccountState::RateLimited, Some(now() + 60_000))
            .unwrap();

        assert_eq!(store.clear_expired_cooldowns().unwrap(), 0);
        assert_eq!(
            store.accounts().unwrap()[0].state,
            AccountState::RateLimited
        );
    }

    #[test]
    fn requests_come_back_newest_first() {
        let store = store();
        for (index, id) in ["a", "b", "c"].iter().enumerate() {
            store
                .record_request(&RequestRow {
                    id: (*id).to_owned(),
                    started_at: index as i64,
                    person: None,
                    client: None,
                    provider: "anthropic".into(),
                    account: None,
                    model: "claude-sonnet-5".into(),
                    ingress: "responses".into(),
                    egress: "messages".into(),
                    translated: true,
                    status: "ok".into(),
                    error_kind: None,
                    ttft_ms: Some(300),
                    duration_ms: Some(1100),
                    usage: Usage::default(),
                    cost_micros: None,
                    input_digest: None,
                    output_digest: None,
                    byte_identical: Some(false),
                    evidence: None,
                    upstream_request_id: None,
                })
                .unwrap();
        }
        let rows = store.recent_requests(10).unwrap();
        assert_eq!(
            rows.iter().map(|r| r.id.as_str()).collect::<Vec<_>>(),
            vec!["c", "b", "a"]
        );
    }
}
