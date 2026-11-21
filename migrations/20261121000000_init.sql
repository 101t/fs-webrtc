CREATE TABLE users (
    id            INTEGER PRIMARY KEY,
    username      TEXT NOT NULL UNIQUE,     -- also the SIP user name
    display_name  TEXT NOT NULL,
    password_hash TEXT NOT NULL,            -- argon2id PHC string
    sip_password  TEXT NOT NULL,            -- random, server-generated; handed to the user's own browser only
    created_at    INTEGER NOT NULL,         -- unix epoch seconds
    last_seen     INTEGER NOT NULL DEFAULT 0
);

-- Login sessions. Only the SHA-256 of a token is stored.
CREATE TABLE sessions (
    token_hash BLOB PRIMARY KEY,
    user_id    INTEGER NOT NULL REFERENCES users (id) ON DELETE CASCADE,
    expires_at INTEGER NOT NULL
);
CREATE INDEX sessions_user ON sessions (user_id);

-- A row (a, b) means "a added b". Both rows present = contacts who may call each other;
-- one row alone is a pending request.
CREATE TABLE contacts (
    user_id    INTEGER NOT NULL REFERENCES users (id) ON DELETE CASCADE,
    contact_id INTEGER NOT NULL REFERENCES users (id) ON DELETE CASCADE,
    created_at INTEGER NOT NULL,
    PRIMARY KEY (user_id, contact_id)
);
CREATE INDEX contacts_reverse ON contacts (contact_id, user_id);
