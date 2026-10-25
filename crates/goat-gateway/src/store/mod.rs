mod auth;
mod schema;

use std::{
    path::{Path, PathBuf},
    sync::{Arc, Mutex},
};

use chacha20poly1305::{
    ChaCha20Poly1305, Key, Nonce,
    aead::{Aead, KeyInit},
};
use rand::RngCore as _;
use rusqlite::{Connection, OptionalExtension as _, params};

pub use crate::store::auth::{
    ADMIN_PREFIX, Caller, Issued, KeyRow, Person, Role, UserRow, Viewer, mint,
};
pub use crate::store::records::{AccountRow, AccountState, RequestRow, Usage};

pub mod insight;
mod records;

#[derive(Debug, thiserror::Error)]
pub enum StoreError {
    #[error("database: {0}")]
    Sqlite(#[from] rusqlite::Error),
    #[error("io: {0}")]
    Io(#[from] std::io::Error),
    #[error("stored credential for {account:?} could not be decrypted with this master key")]
    Undecryptable { account: String },
    #[error("no account named {0:?}")]
    UnknownAccount(String),
    #[error("could not protect the password: {0}")]
    Password(String),
}

#[derive(Clone)]
pub struct Store {
    connection: Arc<Mutex<Connection>>,
    cipher: Arc<ChaCha20Poly1305>,
}

impl std::fmt::Debug for Store {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("Store")
    }
}

impl Store {
    pub fn open(path: &Path, master_key: &[u8; 32]) -> Result<Self, StoreError> {
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        let connection = Connection::open(path)?;
        connection.pragma_update(None, "journal_mode", "WAL")?;
        connection.pragma_update(None, "foreign_keys", "ON")?;
        connection.pragma_update(None, "synchronous", "NORMAL")?;

        let store = Self {
            connection: Arc::new(Mutex::new(connection)),
            cipher: Arc::new(ChaCha20Poly1305::new(Key::from_slice(master_key))),
        };
        store.migrate()?;
        Ok(store)
    }

    pub fn in_memory(master_key: &[u8; 32]) -> Result<Self, StoreError> {
        let connection = Connection::open_in_memory()?;
        connection.pragma_update(None, "foreign_keys", "ON")?;
        let store = Self {
            connection: Arc::new(Mutex::new(connection)),
            cipher: Arc::new(ChaCha20Poly1305::new(Key::from_slice(master_key))),
        };
        store.migrate()?;
        Ok(store)
    }

    #[cfg(test)]
    pub(crate) fn open_in_memory_before(
        master_key: &[u8; 32],
        stop: &str,
    ) -> Result<Self, StoreError> {
        let connection = Connection::open_in_memory()?;
        connection.pragma_update(None, "foreign_keys", "ON")?;
        let store = Self {
            connection: Arc::new(Mutex::new(connection)),
            cipher: Arc::new(ChaCha20Poly1305::new(Key::from_slice(master_key))),
        };
        store.migrate_until(Some(stop))?;
        Ok(store)
    }

    fn migrate(&self) -> Result<(), StoreError> {
        self.migrate_until(None)
    }

    fn migrate_until(&self, stop: Option<&str>) -> Result<(), StoreError> {
        let connection = self.connection.lock().expect("store mutex");
        connection.execute_batch(
            "CREATE TABLE IF NOT EXISTS migrations (
                 name       TEXT PRIMARY KEY,
                 applied_at INTEGER NOT NULL
             )",
        )?;

        for (name, sql) in schema::MIGRATIONS {
            if stop == Some(*name) {
                break;
            }
            let applied: Option<String> = connection
                .query_row(
                    "SELECT name FROM migrations WHERE name = ?1",
                    params![name],
                    |row| row.get(0),
                )
                .optional()?;
            if applied.is_some() {
                continue;
            }
            connection.execute_batch(sql)?;
            connection.execute(
                "INSERT INTO migrations (name, applied_at) VALUES (?1, ?2)",
                params![name, now()],
            )?;
        }
        Ok(())
    }

    fn seal(&self, plaintext: &[u8]) -> (Vec<u8>, Vec<u8>) {
        let mut nonce = [0u8; 12];
        rand::rng().fill_bytes(&mut nonce);
        let ciphertext = self
            .cipher
            .encrypt(Nonce::from_slice(&nonce), plaintext)
            .expect("in-memory encryption cannot fail");
        (ciphertext, nonce.to_vec())
    }

    fn unseal(
        &self,
        account: &str,
        ciphertext: &[u8],
        nonce: &[u8],
    ) -> Result<Vec<u8>, StoreError> {
        self.cipher
            .decrypt(Nonce::from_slice(nonce), ciphertext)
            .map_err(|_| StoreError::Undecryptable {
                account: account.to_owned(),
            })
    }
}

pub fn now() -> i64 {
    use std::time::{SystemTime, UNIX_EPOCH};
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_millis() as i64)
        .unwrap_or_default()
}

pub fn data_dir() -> PathBuf {
    if let Ok(dir) = std::env::var("GOAT_DATA_DIR") {
        return PathBuf::from(dir);
    }
    let home = std::env::var("HOME").unwrap_or_else(|_| ".".to_owned());
    PathBuf::from(home).join(".goat-gateway")
}

pub fn load_or_create_master_key(dir: &Path) -> Result<[u8; 32], StoreError> {
    if let Ok(hex) = std::env::var("GOAT_MASTER_KEY")
        && hex.len() == 64
        && let Some(key) = decode_hex(&hex)
    {
        return Ok(key);
    }

    std::fs::create_dir_all(dir)?;
    let path = dir.join("master.key");
    if let Ok(contents) = std::fs::read_to_string(&path)
        && let Some(key) = decode_hex(contents.trim())
    {
        return Ok(key);
    }

    let mut key = [0u8; 32];
    rand::rng().fill_bytes(&mut key);
    write_private(&path, &encode_hex(&key))?;
    Ok(key)
}

fn write_private(path: &Path, contents: &str) -> Result<(), StoreError> {
    std::fs::write(path, contents)?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt as _;
        std::fs::set_permissions(path, std::fs::Permissions::from_mode(0o600))?;
    }
    Ok(())
}

pub fn encode_hex(bytes: &[u8]) -> String {
    use std::fmt::Write as _;
    let mut out = String::with_capacity(bytes.len() * 2);
    for byte in bytes {
        let _ = write!(out, "{byte:02x}");
    }
    out
}

fn decode_hex(text: &str) -> Option<[u8; 32]> {
    if text.len() != 64 {
        return None;
    }
    let mut key = [0u8; 32];
    for (index, slot) in key.iter_mut().enumerate() {
        *slot = u8::from_str_radix(text.get(index * 2..index * 2 + 2)?, 16).ok()?;
    }
    Some(key)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn with_one_person_their_sign_ins_become_theirs_and_shared_keys_stay_shared() {
        let store =
            Store::open_in_memory_before(&[3u8; 32], "0005_people_own_what_they_sign_in_to")
                .unwrap();
        let connection = store.connection.lock().unwrap();
        connection
            .execute_batch(
                "INSERT INTO users (id, name, created_at) VALUES ('usr_1', 'mino', 0);
                 INSERT INTO accounts (name, provider, credential_kind, secret, nonce, created_at)
                   VALUES ('Max', 'anthropic', 'oauth', x'00', x'00', 0),
                          ('Team key', 'anthropic', 'api_key', x'00', x'00', 0);",
            )
            .unwrap();
        drop(connection);

        store.migrate().unwrap();

        let owner_of = |name: &str| {
            store
                .accounts()
                .unwrap()
                .into_iter()
                .find(|account| account.name == name)
                .and_then(|account| account.owner)
        };
        assert_eq!(owner_of("Max").as_deref(), Some("usr_1"));
        assert_eq!(owner_of("Team key"), None, "an API key stays everyone's");
        assert_eq!(store.users().unwrap()[0].role, auth::Role::Admin);
    }

    #[test]
    fn with_several_people_nobody_is_handed_someone_elses_plan() {
        let store =
            Store::open_in_memory_before(&[3u8; 32], "0005_people_own_what_they_sign_in_to")
                .unwrap();
        store
            .connection
            .lock()
            .unwrap()
            .execute_batch(
                "INSERT INTO users (id, name, created_at) VALUES ('usr_1', 'mino', 0), ('usr_2', 'ana', 0);
                 INSERT INTO accounts (name, provider, credential_kind, secret, nonce, created_at)
                   VALUES ('Max', 'anthropic', 'oauth', x'00', x'00', 0);",
            )
            .unwrap();

        store.migrate().unwrap();

        assert_eq!(store.accounts().unwrap()[0].owner, None);
        assert!(
            store
                .users()
                .unwrap()
                .iter()
                .all(|user| user.role == auth::Role::Member),
            "which of several people is the admin is not something a migration can know"
        );
    }

    #[test]
    fn a_conversation_prefers_the_account_that_served_the_same_person() {
        let store = Store::in_memory(&[3u8; 32]).unwrap();
        let mut row = records::tests::row("req_1", "anthropic", "ok");
        row.conversation = Some("conv_same".into());
        row.account = Some("Ana's Max".into());
        row.user_id = Some("usr_ana".into());
        store.record_request(&row).unwrap();

        assert_eq!(
            store
                .account_that_served("conv_same", "usr_ana")
                .unwrap()
                .as_deref(),
            Some("Ana's Max")
        );
        assert_eq!(
            store.account_that_served("conv_same", "usr_mino").unwrap(),
            None,
            "two people opening the same way must not be steered onto each other's accounts"
        );
    }

    #[test]
    fn a_chatgpt_sign_in_registered_under_openai_moves_to_chatgpt_and_keys_stay() {
        let store = Store::in_memory(&[3u8; 32]).unwrap();
        store
            .add_account("ChatGPT Pro", "openai", "oauth", b"{}")
            .unwrap();
        store
            .add_account("OpenAI key", "openai", "api_key", b"sk-1")
            .unwrap();
        store
            .connection
            .lock()
            .unwrap()
            .execute(
                "DELETE FROM migrations WHERE name = '0004_chatgpt_is_its_own_provider'",
                [],
            )
            .unwrap();

        store.migrate().unwrap();

        let provider_of = |name: &str| {
            store
                .accounts()
                .unwrap()
                .into_iter()
                .find(|account| account.name == name)
                .map(|account| account.provider)
        };
        assert_eq!(provider_of("ChatGPT Pro").as_deref(), Some("chatgpt"));
        assert_eq!(
            provider_of("OpenAI key").as_deref(),
            Some("openai"),
            "an API key belongs on the pay-per-token host"
        );
    }
}
