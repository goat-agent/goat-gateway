pub const MIGRATIONS: &[(&str, &str)] = &[
    (
        "0001_initial",
        r#"
CREATE TABLE accounts (
    name            TEXT PRIMARY KEY,
    provider        TEXT NOT NULL,
    credential_kind TEXT NOT NULL,
    secret          BLOB NOT NULL,
    nonce           BLOB NOT NULL,
    state           TEXT NOT NULL DEFAULT 'active',
    cooldown_until  INTEGER,
    note            TEXT,
    created_at      INTEGER NOT NULL
);

CREATE TABLE rate_limits (
    account    TEXT PRIMARY KEY REFERENCES accounts(name) ON DELETE CASCADE,
    snapshot   TEXT NOT NULL,
    updated_at INTEGER NOT NULL
);

CREATE TABLE requests (
    id                  TEXT PRIMARY KEY,
    started_at          INTEGER NOT NULL,
    person              TEXT,
    client              TEXT,
    conversation        TEXT,
    provider            TEXT NOT NULL,
    account             TEXT,
    model               TEXT NOT NULL,
    ingress             TEXT NOT NULL,
    egress              TEXT NOT NULL,
    translated          INTEGER NOT NULL DEFAULT 0,
    status              TEXT NOT NULL,
    error_kind          TEXT,
    error_message       TEXT,
    ttft_ms             INTEGER,
    duration_ms         INTEGER,
    input_tokens        INTEGER,
    output_tokens       INTEGER,
    cache_read_tokens   INTEGER,
    cache_write_tokens  INTEGER,
    reasoning_tokens    INTEGER,
    cost_micros         INTEGER,
    input_digest        TEXT,
    output_digest       TEXT,
    byte_identical      INTEGER,
    evidence            TEXT,
    upstream_request_id TEXT
);

CREATE INDEX requests_by_time    ON requests(started_at DESC);
CREATE INDEX requests_by_account ON requests(account, started_at DESC);
CREATE INDEX requests_by_model   ON requests(model, started_at DESC);

CREATE TABLE people (
    name       TEXT PRIMARY KEY,
    is_owner   INTEGER NOT NULL DEFAULT 0,
    created_at INTEGER NOT NULL
);

CREATE TABLE api_keys (
    id         TEXT PRIMARY KEY,
    person     TEXT NOT NULL REFERENCES people(name) ON DELETE CASCADE,
    prefix     TEXT NOT NULL,
    hash       BLOB NOT NULL,
    created_at INTEGER NOT NULL,
    revoked_at INTEGER
);

CREATE TABLE passkeys (
    id           TEXT PRIMARY KEY,
    person       TEXT NOT NULL REFERENCES people(name) ON DELETE CASCADE,
    public_key   BLOB NOT NULL,
    sign_count   INTEGER NOT NULL DEFAULT 0,
    created_at   INTEGER NOT NULL
);
"#,
    ),
    (
        "0002_auth",
        r#"
DROP TABLE IF EXISTS passkeys;
DROP TABLE IF EXISTS api_keys;
DROP TABLE IF EXISTS people;

CREATE TABLE users (
    id         TEXT PRIMARY KEY,
    name       TEXT NOT NULL UNIQUE,
    created_at INTEGER NOT NULL
);

CREATE TABLE keys (
    id           TEXT PRIMARY KEY,
    user_id      TEXT NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    label        TEXT NOT NULL,
    prefix       TEXT NOT NULL,
    hash         BLOB NOT NULL UNIQUE,
    created_at   INTEGER NOT NULL,
    last_used_at INTEGER,
    revoked_at   INTEGER
);

CREATE INDEX keys_by_user ON keys(user_id);

CREATE TABLE settings (
    name  TEXT PRIMARY KEY,
    value TEXT NOT NULL
);
"#,
    ),
    (
        "0003_conversations",
        r#"
CREATE INDEX requests_by_conversation ON requests(conversation, started_at DESC);
"#,
    ),
    (
        "0004_chatgpt_is_its_own_provider",
        r#"
UPDATE accounts SET provider = 'chatgpt' WHERE provider = 'openai' AND credential_kind = 'oauth';
"#,
    ),
    (
        "0005_people_own_what_they_sign_in_to",
        r#"
ALTER TABLE users ADD COLUMN role TEXT NOT NULL DEFAULT 'member';
ALTER TABLE users ADD COLUMN password TEXT;

CREATE TABLE sessions (
    hash       BLOB PRIMARY KEY,
    user_id    TEXT REFERENCES users(id) ON DELETE CASCADE,
    created_at INTEGER NOT NULL,
    expires_at INTEGER NOT NULL
);

CREATE TABLE invites (
    hash       BLOB PRIMARY KEY,
    user_id    TEXT NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    expires_at INTEGER NOT NULL
);

ALTER TABLE accounts ADD COLUMN owner TEXT REFERENCES users(id) ON DELETE CASCADE;

ALTER TABLE requests ADD COLUMN user_id TEXT;
ALTER TABLE requests ADD COLUMN key_id TEXT;
ALTER TABLE requests ADD COLUMN credential TEXT;
ALTER TABLE requests ADD COLUMN attempts TEXT;
CREATE INDEX requests_by_user ON requests(user_id, started_at DESC);

CREATE TABLE demotions (
    key_id  TEXT NOT NULL REFERENCES keys(id) ON DELETE CASCADE,
    account TEXT NOT NULL REFERENCES accounts(name) ON DELETE CASCADE ON UPDATE CASCADE,
    until   INTEGER,
    cause   TEXT NOT NULL,
    set_at  INTEGER NOT NULL,
    PRIMARY KEY (key_id, account)
);

UPDATE users SET role = 'admin' WHERE (SELECT COUNT(*) FROM users) = 1;
UPDATE accounts SET owner = (SELECT id FROM users)
 WHERE credential_kind = 'oauth' AND (SELECT COUNT(*) FROM users) = 1;
"#,
    ),
    (
        "0006_keys_are_labels",
        r#"
DROP TABLE demotions;
"#,
    ),
    (
        "0007_a_request_names_its_key",
        r#"
ALTER TABLE requests DROP COLUMN client;
"#,
    ),
    (
        "0008_every_session_is_a_person",
        r#"
DELETE FROM sessions WHERE user_id IS NULL;
"#,
    ),
];
