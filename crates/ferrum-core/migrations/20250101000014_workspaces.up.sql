-- Workspace and collaboration: named project containers with shared access control.
CREATE TABLE workspaces (
    id           TEXT PRIMARY KEY,
    name         TEXT NOT NULL,
    description  TEXT,
    owner_sub    TEXT NOT NULL,
    slug         TEXT NOT NULL UNIQUE,
    is_archived  BOOLEAN NOT NULL DEFAULT FALSE,
    settings     JSONB NOT NULL DEFAULT '{}',
    created_at   TIMESTAMPTZ DEFAULT now(),
    updated_at   TIMESTAMPTZ DEFAULT now()
);

CREATE TABLE workspace_members (
    workspace_id TEXT NOT NULL REFERENCES workspaces(id) ON DELETE CASCADE,
    sub          TEXT NOT NULL,
    role         TEXT NOT NULL,
    invited_by   TEXT NOT NULL,
    joined_at    TIMESTAMPTZ DEFAULT now(),
    PRIMARY KEY (workspace_id, sub)
);

CREATE TABLE workspace_invites (
    id           TEXT PRIMARY KEY,
    workspace_id TEXT NOT NULL REFERENCES workspaces(id) ON DELETE CASCADE,
    email        TEXT NOT NULL,
    role         TEXT NOT NULL,
    token        TEXT NOT NULL UNIQUE,
    invited_by   TEXT NOT NULL,
    expires_at   TIMESTAMPTZ NOT NULL,
    accepted_at  TIMESTAMPTZ
);

ALTER TABLE drs_objects  ADD COLUMN IF NOT EXISTS workspace_id TEXT REFERENCES workspaces(id);
ALTER TABLE wes_runs     ADD COLUMN IF NOT EXISTS workspace_id TEXT REFERENCES workspaces(id);

CREATE TABLE workspace_activity (
    id            TEXT PRIMARY KEY,
    workspace_id  TEXT NOT NULL REFERENCES workspaces(id) ON DELETE CASCADE,
    sub           TEXT NOT NULL,
    action        TEXT NOT NULL,
    resource_type TEXT,
    resource_id   TEXT,
    details       JSONB DEFAULT '{}',
    occurred_at   TIMESTAMPTZ DEFAULT now()
);

CREATE INDEX IF NOT EXISTS idx_ws_members_sub ON workspace_members(sub);
CREATE INDEX IF NOT EXISTS idx_ws_activity ON workspace_activity(workspace_id, occurred_at DESC);
CREATE INDEX IF NOT EXISTS idx_drs_workspace ON drs_objects(workspace_id) WHERE workspace_id IS NOT NULL;
CREATE INDEX IF NOT EXISTS idx_wes_workspace ON wes_runs(workspace_id) WHERE workspace_id IS NOT NULL;
