use rand::RngCore as _;
use rusqlite::{OptionalExtension as _, params};
use serde::Serialize;
use sha2::{Digest as _, Sha256};

use crate::store::{Store, StoreError, encode_hex, now};

pub const KEY_PREFIX: &str = "gwk_";
pub const ADMIN_PREFIX: &str = "gwa_";
const ADMIN_HASH: &str = "admin_key_hash";

#[derive(Debug, Clone, Serialize)]
pub struct UserRow {
    pub id: String,
    pub name: String,
    pub created_at: i64,
}

#[derive(Debug, Clone, Serialize)]
pub struct KeyRow {
    pub id: String,
    pub user_id: String,
    pub user_name: String,
    pub label: String,
    pub prefix: String,
    pub created_at: i64,
    pub last_used_at: Option<i64>,
    pub revoked_at: Option<i64>,
}

#[derive(Debug, Clone)]
pub struct Issued {
    pub id: String,
    pub secret: String,
}

#[derive(Debug, Clone)]
pub struct Caller {
    pub key_id: String,
    pub user_id: String,
    pub user_name: String,
}

pub fn mint(prefix: &str) -> String {
    let mut bytes = [0u8; 24];
    rand::rng().fill_bytes(&mut bytes);
    use base64::{Engine as _, engine::general_purpose::URL_SAFE_NO_PAD as B64};
    format!("{prefix}{}", B64.encode(bytes))
}

pub fn digest(secret: &str) -> Vec<u8> {
    let mut hasher = Sha256::new();
    hasher.update(secret.as_bytes());
    hasher.finalize().to_vec()
}

fn visible_prefix(secret: &str) -> String {
    secret.chars().take(12).collect()
}

impl Store {
    pub fn add_user(&self, name: &str) -> Result<UserRow, StoreError> {
        let id = format!("usr_{}", short_id());
        let created_at = now();
        let connection = self.connection.lock().expect("store mutex");
        connection.execute(
            "INSERT INTO users (id, name, created_at) VALUES (?1, ?2, ?3)",
            params![id, name, created_at],
        )?;
        Ok(UserRow {
            id,
            name: name.to_owned(),
            created_at,
        })
    }

    pub fn users(&self) -> Result<Vec<UserRow>, StoreError> {
        let connection = self.connection.lock().expect("store mutex");
        let mut statement =
            connection.prepare("SELECT id, name, created_at FROM users ORDER BY name")?;
        let rows = statement
            .query_map([], |row| {
                Ok(UserRow {
                    id: row.get(0)?,
                    name: row.get(1)?,
                    created_at: row.get(2)?,
                })
            })?
            .collect::<Result<Vec<_>, _>>()?;
        Ok(rows)
    }

    pub fn remove_user(&self, id: &str) -> Result<(), StoreError> {
        let connection = self.connection.lock().expect("store mutex");
        connection.execute("DELETE FROM users WHERE id = ?1", params![id])?;
        Ok(())
    }

    pub fn issue_key(&self, user_id: &str, label: &str) -> Result<Issued, StoreError> {
        let secret = mint(KEY_PREFIX);
        let id = format!("key_{}", short_id());
        let connection = self.connection.lock().expect("store mutex");
        connection.execute(
            "INSERT INTO keys (id, user_id, label, prefix, hash, created_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
            params![
                id,
                user_id,
                label,
                visible_prefix(&secret),
                digest(&secret),
                now()
            ],
        )?;
        Ok(Issued { id, secret })
    }

    pub fn keys(&self) -> Result<Vec<KeyRow>, StoreError> {
        let connection = self.connection.lock().expect("store mutex");
        let mut statement = connection.prepare(
            "SELECT k.id, k.user_id, u.name, k.label, k.prefix, k.created_at,
                    k.last_used_at, k.revoked_at
             FROM keys k JOIN users u ON u.id = k.user_id
             ORDER BY u.name, k.label",
        )?;
        let rows = statement
            .query_map([], |row| {
                Ok(KeyRow {
                    id: row.get(0)?,
                    user_id: row.get(1)?,
                    user_name: row.get(2)?,
                    label: row.get(3)?,
                    prefix: row.get(4)?,
                    created_at: row.get(5)?,
                    last_used_at: row.get(6)?,
                    revoked_at: row.get(7)?,
                })
            })?
            .collect::<Result<Vec<_>, _>>()?;
        Ok(rows)
    }

    pub fn revoke_key(&self, id: &str) -> Result<(), StoreError> {
        let connection = self.connection.lock().expect("store mutex");
        connection.execute(
            "UPDATE keys SET revoked_at = ?2 WHERE id = ?1 AND revoked_at IS NULL",
            params![id, now()],
        )?;
        Ok(())
    }

    pub fn caller_for(&self, secret: &str) -> Result<Option<Caller>, StoreError> {
        if !secret.starts_with(KEY_PREFIX) {
            return Ok(None);
        }
        let hash = digest(secret);
        let connection = self.connection.lock().expect("store mutex");
        let found: Option<Caller> = connection
            .query_row(
                "SELECT k.id, k.user_id, u.name
                 FROM keys k JOIN users u ON u.id = k.user_id
                 WHERE k.hash = ?1 AND k.revoked_at IS NULL",
                params![hash],
                |row| {
                    Ok(Caller {
                        key_id: row.get(0)?,
                        user_id: row.get(1)?,
                        user_name: row.get(2)?,
                    })
                },
            )
            .optional()?;

        if let Some(caller) = &found {
            connection.execute(
                "UPDATE keys SET last_used_at = ?2 WHERE id = ?1",
                params![caller.key_id, now()],
            )?;
        }
        Ok(found)
    }

    pub fn set_admin_key(&self, secret: &str) -> Result<(), StoreError> {
        let connection = self.connection.lock().expect("store mutex");
        connection.execute(
            "INSERT INTO settings (name, value) VALUES (?1, ?2)
             ON CONFLICT(name) DO UPDATE SET value = excluded.value",
            params![ADMIN_HASH, encode_hex(&digest(secret))],
        )?;
        Ok(())
    }

    pub fn has_admin_key(&self) -> Result<bool, StoreError> {
        let connection = self.connection.lock().expect("store mutex");
        let found: Option<String> = connection
            .query_row(
                "SELECT value FROM settings WHERE name = ?1",
                params![ADMIN_HASH],
                |row| row.get(0),
            )
            .optional()?;
        Ok(found.is_some())
    }

    pub fn admin_key_matches(&self, secret: &str) -> Result<bool, StoreError> {
        let connection = self.connection.lock().expect("store mutex");
        let stored: Option<String> = connection
            .query_row(
                "SELECT value FROM settings WHERE name = ?1",
                params![ADMIN_HASH],
                |row| row.get(0),
            )
            .optional()?;
        Ok(match stored {
            Some(stored) => {
                constant_time_eq(stored.as_bytes(), encode_hex(&digest(secret)).as_bytes())
            }
            None => false,
        })
    }
}

fn constant_time_eq(a: &[u8], b: &[u8]) -> bool {
    if a.len() != b.len() {
        return false;
    }
    a.iter().zip(b).fold(0u8, |acc, (x, y)| acc | (x ^ y)) == 0
}

fn short_id() -> String {
    let mut bytes = [0u8; 8];
    rand::rng().fill_bytes(&mut bytes);
    encode_hex(&bytes)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn store() -> Store {
        Store::in_memory(&[4u8; 32]).unwrap()
    }

    #[test]
    fn a_key_is_only_ever_returned_once() {
        let store = store();
        let user = store.add_user("isac").unwrap();
        let issued = store.issue_key(&user.id, "맥북 Codex").unwrap();

        assert!(issued.secret.starts_with(KEY_PREFIX));

        let listed = store.keys().unwrap();
        assert_eq!(listed.len(), 1);
        assert_eq!(listed[0].user_name, "isac");
        assert_eq!(listed[0].prefix, issued.secret[..12].to_owned());

        let json = serde_json::to_string(&listed).unwrap();
        assert!(
            !json.contains(&issued.secret),
            "a listing must never carry the full key"
        );
    }

    #[test]
    fn the_secret_is_not_recoverable_from_the_database() {
        let store = store();
        let user = store.add_user("isac").unwrap();
        let issued = store.issue_key(&user.id, "laptop").unwrap();

        let connection = store.connection.lock().unwrap();
        let stored: Vec<u8> = connection
            .query_row("SELECT hash FROM keys", [], |row| row.get(0))
            .unwrap();
        assert!(!stored.windows(4).any(|w| w == b"gwk_"));
        assert_eq!(stored, digest(&issued.secret));
    }

    #[test]
    fn a_valid_key_resolves_to_its_user() {
        let store = store();
        let user = store.add_user("isac").unwrap();
        let issued = store.issue_key(&user.id, "laptop").unwrap();

        let caller = store.caller_for(&issued.secret).unwrap().unwrap();
        assert_eq!(caller.user_name, "isac");
        assert_eq!(caller.key_id, issued.id);
        assert!(store.keys().unwrap()[0].last_used_at.is_some());
    }

    #[test]
    fn a_revoked_key_stops_resolving_and_leaves_the_user_alone() {
        let store = store();
        let user = store.add_user("isac").unwrap();
        let laptop = store.issue_key(&user.id, "laptop").unwrap();
        let ci = store.issue_key(&user.id, "CI").unwrap();

        store.revoke_key(&laptop.id).unwrap();

        assert!(store.caller_for(&laptop.secret).unwrap().is_none());
        assert!(
            store.caller_for(&ci.secret).unwrap().is_some(),
            "revoking one device must not disturb the others"
        );
    }

    #[test]
    fn an_admin_key_is_not_an_api_key() {
        let store = store();
        let admin = mint(ADMIN_PREFIX);
        store.set_admin_key(&admin).unwrap();

        assert!(store.admin_key_matches(&admin).unwrap());
        assert!(
            store.caller_for(&admin).unwrap().is_none(),
            "the admin key must not authorize model calls"
        );
    }

    #[test]
    fn an_api_key_is_not_an_admin_key() {
        let store = store();
        store.set_admin_key(&mint(ADMIN_PREFIX)).unwrap();
        let user = store.add_user("isac").unwrap();
        let issued = store.issue_key(&user.id, "laptop").unwrap();

        assert!(!store.admin_key_matches(&issued.secret).unwrap());
    }

    #[test]
    fn removing_a_user_takes_their_keys_with_them() {
        let store = store();
        let user = store.add_user("isac").unwrap();
        let issued = store.issue_key(&user.id, "laptop").unwrap();

        store.remove_user(&user.id).unwrap();

        assert!(store.keys().unwrap().is_empty());
        assert!(store.caller_for(&issued.secret).unwrap().is_none());
    }

    #[test]
    fn a_made_up_key_resolves_to_nobody() {
        let store = store();
        store.add_user("isac").unwrap();
        assert!(store.caller_for("gwk_totally-made-up").unwrap().is_none());
        assert!(store.caller_for("not-even-our-prefix").unwrap().is_none());
    }

    #[test]
    fn two_users_cannot_share_a_name() {
        let store = store();
        store.add_user("isac").unwrap();
        assert!(store.add_user("isac").is_err());
    }
}
