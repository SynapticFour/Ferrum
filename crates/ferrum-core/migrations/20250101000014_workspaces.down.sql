DROP INDEX IF EXISTS idx_wes_workspace;
DROP INDEX IF EXISTS idx_drs_workspace;
DROP INDEX IF EXISTS idx_ws_activity;
DROP INDEX IF EXISTS idx_ws_members_sub;

DROP TABLE IF EXISTS workspace_activity;
ALTER TABLE wes_runs     DROP COLUMN IF EXISTS workspace_id;
ALTER TABLE drs_objects  DROP COLUMN IF EXISTS workspace_id;

DROP TABLE IF EXISTS workspace_invites;
DROP TABLE IF EXISTS workspace_members;
DROP TABLE IF EXISTS workspaces;
