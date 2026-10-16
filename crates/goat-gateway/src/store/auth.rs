use rand::RngCore as _;
use rusqlite::{OptionalExtension as _, params};
use serde::{Deserialize, Serialize};
use sha2::{Digest as _, Sha256};

use crate::store::{Store, StoreError, encode_hex, now};

pub const KEY_PREFIX: &str = "gwk_";
pub const ADMIN_PREFIX: &str = "gwa_";
const SESSION_PREFIX: &str = "gws_";
const INVITE_PREFIX: &str = "gwi_";
const ADMIN_HASH: &str = "admin_key_hash";
const INSTALL: &str = "install_id";
const SESSION_LASTS_MS: i64 = 30 * 86_400_000;
const INVITE_LASTS_MS: i64 = 7 * 86_400_000;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Role {
    Admin,
    Member,
}

impl Role {
    fn as_str(self) -> &'static str {
        match self {
            Self::Admin => "admin",
            Self::Member => "member",
        }
    }

    fn parse(text: &str) -> Self {
        if text == "admin" {
            Self::Admin
        } else {
            Self::Member
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Person {
    pub id: String,
    pub name: String,
    pub role: Role,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Viewer {
    Root,
    Person(Person),
}

impl Viewer {
    pub fn is_admin(&self) -> bool {
        match self {
            Self::Root => true,
            Self::Person(person) => person.role == Role::Admin,
        }
    }

    pub fn person(&self) -> Option<&Person> {
        match self {
            Self::Root => None,
            Self::Person(person) => Some(person),
        }
    }
}

#[derive(Debug, Clone, Serialize)]
pub struct UserRow {
    pub id: String,
    pub name: String,
    pub role: Role,
    pub can_sign_in: bool,
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
            role: Role::Member,
            can_sign_in: false,
            created_at,
        })
    }

    pub fn users(&self) -> Result<Vec<UserRow>, StoreError> {
        let connection = self.connection.lock().expect("store mutex");
        let mut statement = connection.prepare(
            "SELECT id, name, role, password IS NOT NULL, created_at FROM users ORDER BY name",
        )?;
        let rows = statement
            .query_map([], |row| {
                Ok(UserRow {
                    id: row.get(0)?,
                    name: row.get(1)?,
                    role: Role::parse(&row.get::<_, String>(2)?),
                    can_sign_in: row.get(3)?,
                    created_at: row.get(4)?,
                })
            })?
            .collect::<Result<Vec<_>, _>>()?;
        Ok(rows)
    }

    pub fn person(&self, id: &str) -> Result<Option<Person>, StoreError> {
        let connection = self.connection.lock().expect("store mutex");
        let found = connection
            .query_row(
                "SELECT id, name, role FROM users WHERE id = ?1",
                params![id],
                read_person,
            )
            .optional()?;
        Ok(found)
    }

    pub fn set_role(&self, id: &str, role: Role) -> Result<(), StoreError> {
        let connection = self.connection.lock().expect("store mutex");
        connection.execute(
            "UPDATE users SET role = ?2 WHERE id = ?1",
            params![id, role.as_str()],
        )?;
        Ok(())
    }

    pub fn set_password(&self, id: &str, password: &str) -> Result<(), StoreError> {
        let protected = protect(password)?;
        let connection = self.connection.lock().expect("store mutex");
        connection.execute(
            "UPDATE users SET password = ?2 WHERE id = ?1",
            params![id, protected],
        )?;
        Ok(())
    }

    pub fn signed_in(&self, name: &str, password: &str) -> Result<Option<Person>, StoreError> {
        let connection = self.connection.lock().expect("store mutex");
        let found: Option<(Person, Option<String>)> = connection
            .query_row(
                "SELECT id, name, role, password FROM users WHERE name = ?1",
                params![name.trim()],
                |row| Ok((read_person(row)?, row.get(3)?)),
            )
            .optional()?;
        drop(connection);

        Ok(found.and_then(|(person, stored)| {
            stored
                .filter(|stored| matches(password, stored))
                .map(|_| person)
        }))
    }

    pub fn open_session(&self, user_id: Option<&str>) -> Result<String, StoreError> {
        let token = mint(SESSION_PREFIX);
        let connection = self.connection.lock().expect("store mutex");
        connection.execute(
            "INSERT INTO sessions (hash, user_id, created_at, expires_at) VALUES (?1, ?2, ?3, ?4)",
            params![digest(&token), user_id, now(), now() + SESSION_LASTS_MS],
        )?;
        Ok(token)
    }

    pub fn viewer_for(&self, token: &str) -> Result<Option<Viewer>, StoreError> {
        if !token.starts_with(SESSION_PREFIX) {
            return Ok(None);
        }
        let connection = self.connection.lock().expect("store mutex");
        let found: Option<Option<String>> = connection
            .query_row(
                "SELECT user_id FROM sessions WHERE hash = ?1 AND expires_at > ?2",
                params![digest(token), now()],
                |row| row.get(0),
            )
            .optional()?;
        drop(connection);

        match found {
            None => Ok(None),
            Some(None) => Ok(Some(Viewer::Root)),
            Some(Some(user_id)) => Ok(self.person(&user_id)?.map(Viewer::Person)),
        }
    }

    pub fn claim_session(&self, token: &str, user_id: &str) -> Result<(), StoreError> {
        let connection = self.connection.lock().expect("store mutex");
        connection.execute(
            "UPDATE sessions SET user_id = ?2 WHERE hash = ?1",
            params![digest(token), user_id],
        )?;
        Ok(())
    }

    pub fn close_session(&self, token: &str) -> Result<(), StoreError> {
        let connection = self.connection.lock().expect("store mutex");
        connection.execute(
            "DELETE FROM sessions WHERE hash = ?1",
            params![digest(token)],
        )?;
        Ok(())
    }

    pub fn invite(&self, user_id: &str) -> Result<String, StoreError> {
        let token = mint(INVITE_PREFIX);
        let connection = self.connection.lock().expect("store mutex");
        connection.execute(
            "INSERT INTO invites (hash, user_id, expires_at) VALUES (?1, ?2, ?3)",
            params![digest(&token), user_id, now() + INVITE_LASTS_MS],
        )?;
        Ok(token)
    }

    pub fn invited(&self, token: &str) -> Result<Option<Person>, StoreError> {
        let connection = self.connection.lock().expect("store mutex");
        let found = connection
            .query_row(
                "SELECT u.id, u.name, u.role FROM invites i JOIN users u ON u.id = i.user_id
                 WHERE i.hash = ?1 AND i.expires_at > ?2",
                params![digest(token), now()],
                read_person,
            )
            .optional()?;
        Ok(found)
    }

    pub fn accept_invite(&self, token: &str, password: &str) -> Result<Option<Person>, StoreError> {
        let Some(person) = self.invited(token)? else {
            return Ok(None);
        };
        self.set_password(&person.id, password)?;
        let connection = self.connection.lock().expect("store mutex");
        connection.execute(
            "DELETE FROM invites WHERE hash = ?1",
            params![digest(token)],
        )?;
        Ok(Some(person))
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

    pub fn install_id(&self) -> Result<String, StoreError> {
        let connection = self.connection.lock().expect("store mutex");
        let found: Option<String> = connection
            .query_row(
                "SELECT value FROM settings WHERE name = ?1",
                params![INSTALL],
                |row| row.get(0),
            )
            .optional()?;
        if let Some(found) = found {
            return Ok(found);
        }
        let mut bytes = [0u8; 16];
        rand::rng().fill_bytes(&mut bytes);
        bytes[6] = (bytes[6] & 0x0f) | 0x40;
        bytes[8] = (bytes[8] & 0x3f) | 0x80;
        let hex = encode_hex(&bytes);
        let made = format!(
            "urn:uuid:{}-{}-{}-{}-{}",
            &hex[0..8],
            &hex[8..12],
            &hex[12..16],
            &hex[16..20],
            &hex[20..32]
        );
        connection.execute(
            "INSERT INTO settings (name, value) VALUES (?1, ?2)",
            params![INSTALL, made],
        )?;
        Ok(made)
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

fn read_person(row: &rusqlite::Row<'_>) -> rusqlite::Result<Person> {
    Ok(Person {
        id: row.get(0)?,
        name: row.get(1)?,
        role: Role::parse(&row.get::<_, String>(2)?),
    })
}

fn protect(password: &str) -> Result<String, StoreError> {
    use argon2::{Argon2, PasswordHasher as _, password_hash::SaltString};
    let mut salt = [0u8; 16];
    rand::rng().fill_bytes(&mut salt);
    let salt =
        SaltString::encode_b64(&salt).map_err(|error| StoreError::Password(error.to_string()))?;
    Argon2::default()
        .hash_password(password.as_bytes(), &salt)
        .map(|hash| hash.to_string())
        .map_err(|error| StoreError::Password(error.to_string()))
}

fn matches(password: &str, stored: &str) -> bool {
    use argon2::{Argon2, PasswordVerifier as _, password_hash::PasswordHash};
    PasswordHash::new(stored)
        .map(|hash| {
            Argon2::default()
                .verify_password(password.as_bytes(), &hash)
                .is_ok()
        })
        .unwrap_or(false)
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
    fn a_password_lets_its_person_in_and_nobody_else() {
        let store = store();
        let mino = store.add_user("mino").unwrap();
        store.set_password(&mino.id, "correct horse").unwrap();

        assert_eq!(
            store
                .signed_in("mino", "correct horse")
                .unwrap()
                .map(|p| p.id),
            Some(mino.id.clone())
        );
        assert!(store.signed_in("mino", "wrong horse").unwrap().is_none());
        assert!(
            store
                .signed_in("nobody", "correct horse")
                .unwrap()
                .is_none()
        );

        let connection = store.connection.lock().unwrap();
        let stored: String = connection
            .query_row("SELECT password FROM users", [], |row| row.get(0))
            .unwrap();
        assert!(!stored.contains("correct horse"));
    }

    #[test]
    fn a_session_resolves_until_it_is_closed() {
        let store = store();
        let mino = store.add_user("mino").unwrap();
        let token = store.open_session(Some(&mino.id)).unwrap();

        let Some(Viewer::Person(person)) = store.viewer_for(&token).unwrap() else {
            panic!("a person's session must resolve to that person");
        };
        assert_eq!(person.name, "mino");

        store.close_session(&token).unwrap();
        assert!(store.viewer_for(&token).unwrap().is_none());
    }

    #[test]
    fn the_admin_key_opens_a_session_that_answers_for_everyone() {
        let store = store();
        let token = store.open_session(None).unwrap();
        assert_eq!(store.viewer_for(&token).unwrap(), Some(Viewer::Root));
        assert!(Viewer::Root.is_admin());
    }

    #[test]
    fn a_session_token_is_not_stored_in_the_clear() {
        let store = store();
        let token = store.open_session(None).unwrap();
        let connection = store.connection.lock().unwrap();
        let stored: Vec<u8> = connection
            .query_row("SELECT hash FROM sessions", [], |row| row.get(0))
            .unwrap();
        assert_eq!(stored, digest(&token));
    }

    #[test]
    fn an_invite_is_spent_by_the_first_password_set_with_it() {
        let store = store();
        let ana = store.add_user("ana").unwrap();
        let invite = store.invite(&ana.id).unwrap();

        assert_eq!(
            store.invited(&invite).unwrap().map(|p| p.name),
            Some("ana".into())
        );
        let accepted = store.accept_invite(&invite, "a long password").unwrap();
        assert_eq!(accepted.map(|p| p.id), Some(ana.id.clone()));

        assert!(store.invited(&invite).unwrap().is_none());
        assert!(
            store
                .accept_invite(&invite, "someone else")
                .unwrap()
                .is_none(),
            "a link that was already used must not reset the password"
        );
        assert!(store.signed_in("ana", "a long password").unwrap().is_some());
    }

    #[test]
    fn an_expired_invite_opens_nothing() {
        let store = store();
        let ana = store.add_user("ana").unwrap();
        let invite = store.invite(&ana.id).unwrap();
        store
            .connection
            .lock()
            .unwrap()
            .execute("UPDATE invites SET expires_at = 0", [])
            .unwrap();
        assert!(store.invited(&invite).unwrap().is_none());
        assert!(store.accept_invite(&invite, "whatever!").unwrap().is_none());
    }

    #[test]
    fn a_new_person_is_a_member_until_made_an_admin() {
        let store = store();
        let ana = store.add_user("ana").unwrap();
        assert_eq!(ana.role, Role::Member);
        store.set_role(&ana.id, Role::Admin).unwrap();
        assert_eq!(store.users().unwrap()[0].role, Role::Admin);
    }

    #[test]
    fn this_gateway_keeps_one_identity_for_every_sign_in() {
        let store = store();
        let first = store.install_id().unwrap();
        assert!(first.starts_with("urn:uuid:"));
        assert_eq!(first.len(), "urn:uuid:".len() + 36);
        assert_eq!(
            store.install_id().unwrap(),
            first,
            "a provider that binds a client to this host must see the same host every time"
        );
    }

    #[test]
    fn two_users_cannot_share_a_name() {
        let store = store();
        store.add_user("isac").unwrap();
        assert!(store.add_user("isac").is_err());
    }
}
