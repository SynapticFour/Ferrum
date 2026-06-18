-- Shared-device operator accounts for Edge mode (Phase 3 / T2 hardened).

CREATE TABLE IF NOT EXISTS edge_operator_accounts (
    id              TEXT PRIMARY KEY,
    username        TEXT NOT NULL UNIQUE,
    role            TEXT NOT NULL,
    pin_hash        TEXT NOT NULL,
    pin_salt        TEXT NOT NULL,
    created_time    TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    disabled        BOOLEAN NOT NULL DEFAULT FALSE
);

CREATE INDEX IF NOT EXISTS idx_edge_operator_accounts_username ON edge_operator_accounts(username);
