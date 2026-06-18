CREATE TABLE IF NOT EXISTS edge_operator_accounts (
    id              TEXT PRIMARY KEY,
    username        TEXT NOT NULL UNIQUE,
    role            TEXT NOT NULL,
    pin_hash        TEXT NOT NULL,
    pin_salt        TEXT NOT NULL,
    created_time    TEXT NOT NULL DEFAULT (datetime('now')),
    disabled        INTEGER NOT NULL DEFAULT 0
);

CREATE INDEX IF NOT EXISTS idx_edge_operator_accounts_username ON edge_operator_accounts(username);
