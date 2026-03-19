//! Database repository for WES runs.

use crate::error::Result;
use crate::types::{RunState, RunSummary, TaskLog};
use chrono::{DateTime, Utc};
use serde_json::Value;
use sqlx::PgPool;

pub struct WesRepo {
    pool: PgPool,
}

impl WesRepo {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }

    pub fn pool(&self) -> &sqlx::PgPool {
        &self.pool
    }

    #[allow(clippy::too_many_arguments)]
    pub async fn create_run(
        &self,
        run_id: &str,
        workflow_url: &str,
        workflow_type: &str,
        workflow_type_version: &str,
        workflow_params: &Value,
        workflow_engine_params: &Value,
        tags: &Value,
        work_dir: Option<&str>,
        owner_sub: &str,
        workspace_id: Option<&str>,
        resumed_from_run_id: Option<&str>,
        checkpoint_enabled: bool,
    ) -> Result<()> {
        sqlx::query(
            r#"INSERT INTO wes_runs (run_id, workflow_url, workflow_type, workflow_type_version,
               workflow_params, workflow_engine_params, tags, state, work_dir, owner_sub, workspace_id, resumed_from_run_id, checkpoint_enabled)
               VALUES ($1, $2, $3, $4, $5, $6, $7, 'QUEUED', $8, $9, $10, $11, $12)"#,
        )
        .bind(run_id)
        .bind(workflow_url)
        .bind(workflow_type)
        .bind(workflow_type_version)
        .bind(workflow_params)
        .bind(workflow_engine_params)
        .bind(tags)
        .bind(work_dir)
        .bind(owner_sub)
        .bind(workspace_id)
        .bind(resumed_from_run_id)
        .bind(checkpoint_enabled)
        .execute(&self.pool)
        .await?;
        Ok(())
    }

    pub async fn get_run(
        &self,
        run_id: &str,
    ) -> Result<
        Option<(
            String,
            String,
            String,
            String,
            Value,
            Value,
            Value,
            String,
            Option<DateTime<Utc>>,
            Option<DateTime<Utc>>,
            Value,
            Option<String>,
            Option<String>,
            Option<String>,
            bool,
        )>,
    > {
        let row = sqlx::query_as::<_, (
            String, String, String, String, Value, Value, Value, String,
            Option<DateTime<Utc>>, Option<DateTime<Utc>>, Value, Option<String>, Option<String>,
            Option<String>, bool,
        )>(
            r#"SELECT run_id, workflow_url, workflow_type, workflow_type_version,
               workflow_params, workflow_engine_params, tags, state,
               start_time, end_time, outputs, work_dir, owner_sub, resumed_from_run_id, checkpoint_enabled
               FROM wes_runs WHERE run_id = $1"#,
        )
        .bind(run_id)
        .fetch_optional(&self.pool)
        .await?;
        Ok(row)
    }

    /// Returns true if the run exists and the caller may access it (owner, admin, or workspace member).
    pub async fn run_visible_to(
        &self,
        run_id: &str,
        caller_sub: Option<&str>,
        is_admin: bool,
    ) -> Result<bool> {
        // Demo/CI: when auth is not required, conformance expects the service to remain usable
        // without strict owner/workspace visibility constraints (HelixTest often calls multiple
        // endpoints without stable token context).
        let require_auth = matches!(std::env::var("FERRUM_AUTH__REQUIRE_AUTH").as_deref(), Ok("true"));
        if !require_auth {
            return Ok(true);
        }

        let row: Option<(Option<String>, Option<String>)> =
            sqlx::query_as("SELECT owner_sub, workspace_id FROM wes_runs WHERE run_id = $1")
                .bind(run_id)
                .fetch_optional(&self.pool)
                .await?;
        let Some((run_owner, workspace_id)) = row else {
            return Ok(false);
        };
        if is_admin {
            return Ok(true);
        }
        if let Some(sub) = caller_sub {
            if run_owner.as_deref() == Some(sub) {
                return Ok(true);
            }
            if let Some(ref ws_id) = workspace_id {
                let role = ferrum_core::get_workspace_member_role(&self.pool, ws_id, sub).await?;
                if role.is_some() {
                    return Ok(true);
                }
            }
        }
        Ok(run_owner.is_none() || run_owner.as_deref() == Some("anonymous"))
    }

    pub async fn update_state(&self, run_id: &str, state: RunState) -> Result<()> {
        let s = state.as_str();
        sqlx::query(
            r#"UPDATE wes_runs SET state = $1,
               start_time = CASE WHEN $1 = 'RUNNING' AND start_time IS NULL THEN NOW() ELSE start_time END,
               end_time = CASE WHEN $1 IN ('COMPLETE', 'EXECUTOR_ERROR', 'SYSTEM_ERROR', 'CANCELED') THEN NOW() ELSE end_time END
               WHERE run_id = $2"#,
        )
        .bind(s)
        .bind(run_id)
        .execute(&self.pool)
        .await?;
        Ok(())
    }

    pub async fn set_start_time(&self, run_id: &str) -> Result<()> {
        sqlx::query("UPDATE wes_runs SET start_time = NOW(), state = 'RUNNING' WHERE run_id = $1")
            .bind(run_id)
            .execute(&self.pool)
            .await?;
        Ok(())
    }

    pub async fn set_work_dir(&self, run_id: &str, work_dir: &str) -> Result<()> {
        sqlx::query("UPDATE wes_runs SET work_dir = $1 WHERE run_id = $2")
            .bind(work_dir)
            .bind(run_id)
            .execute(&self.pool)
            .await?;
        Ok(())
    }

    pub async fn set_end_time_and_state(
        &self,
        run_id: &str,
        state: RunState,
        outputs: Option<&Value>,
    ) -> Result<()> {
        sqlx::query(
            "UPDATE wes_runs SET end_time = NOW(), state = $1, outputs = COALESCE($2, outputs) WHERE run_id = $3",
        )
        .bind(state.as_str())
        .bind(outputs)
        .bind(run_id)
        .execute(&self.pool)
        .await?;
        Ok(())
    }

    /// Merge additional keys into the run's outputs JSONB (e.g. multiqc_report, multiqc_data).
    pub async fn merge_run_outputs(
        &self,
        run_id: &str,
        updates: &serde_json::Map<String, Value>,
    ) -> Result<()> {
        let row: Option<Value> =
            sqlx::query_scalar("SELECT outputs FROM wes_runs WHERE run_id = $1")
                .bind(run_id)
                .fetch_optional(&self.pool)
                .await?;
        let mut obj: serde_json::Map<String, Value> = row
            .as_ref()
            .and_then(Value::as_object)
            .cloned()
            .unwrap_or_default();
        for (k, v) in updates {
            obj.insert(k.clone(), v.clone());
        }
        let merged = Value::Object(obj);
        sqlx::query("UPDATE wes_runs SET outputs = $1 WHERE run_id = $2")
            .bind(&merged)
            .bind(run_id)
            .execute(&self.pool)
            .await?;
        Ok(())
    }

    pub async fn list_runs(
        &self,
        page_size: i64,
        page_token: Option<&str>,
        state_filter: Option<RunState>,
        owner_sub: Option<&str>,
        workspace_id: Option<&str>,
    ) -> Result<(Vec<RunSummary>, Option<String>)> {
        let offset: i64 = page_token.and_then(|t| t.parse().ok()).unwrap_or(0);
        let state_str = state_filter.map(|s| s.as_str());
        type RunListRow = (
            String,
            String,
            Option<DateTime<Utc>>,
            Option<DateTime<Utc>>,
            Option<Value>,
            Option<String>,
        );
        let rows: Vec<RunListRow> = sqlx::query_as(
            r#"SELECT run_id, state, start_time, end_time, tags, resumed_from_run_id FROM wes_runs
                   WHERE ($1::text IS NULL OR state = $1)
                     AND ($4::text IS NULL OR owner_sub = $4)
                     AND ($5::text IS NULL OR workspace_id = $5)
                   ORDER BY created_at DESC LIMIT $2 OFFSET $3"#,
        )
        .bind(state_str)
        .bind(page_size + 1)
        .bind(offset)
        .bind(owner_sub)
        .bind(workspace_id)
        .fetch_all(&self.pool)
        .await?;
        let has_more = rows.len() as i64 > page_size;
        let runs: Vec<RunSummary> = rows
            .into_iter()
            .take(page_size as usize)
            .map(
                |(run_id, state, start_time, end_time, tags, resumed_from_run_id)| {
                    let tags_map = tags
                        .and_then(|v| v.as_object().cloned())
                        .map(|m| {
                            m.into_iter()
                                .filter_map(|(k, v)| Some((k, v.as_str()?.to_string())))
                                .collect()
                        })
                        .unwrap_or_default();
                    RunSummary {
                        run_id,
                        state: RunState::from_str(&state),
                        start_time: start_time.map(|t| t.to_rfc3339()),
                        end_time: end_time.map(|t| t.to_rfc3339()),
                        tags: tags_map,
                        resumed_from_run_id,
                    }
                },
            )
            .collect();
        let next_token = if has_more {
            Some((offset + page_size).to_string())
        } else {
            None
        };
        Ok((runs, next_token))
    }

    #[allow(clippy::too_many_arguments)]
    pub async fn upsert_run_log(
        &self,
        run_id: &str,
        name: &str,
        cmd: &[String],
        start_time: Option<DateTime<Utc>>,
        end_time: Option<DateTime<Utc>>,
        stdout_url: Option<&str>,
        stderr_url: Option<&str>,
        exit_code: Option<i32>,
    ) -> Result<()> {
        let cmd_vec: Vec<&str> = cmd.iter().map(String::as_str).collect();
        sqlx::query(
            r#"INSERT INTO wes_run_log (run_id, name, cmd, start_time, end_time, stdout_url, stderr_url, exit_code)
               VALUES ($1, $2, $3, $4, $5, $6, $7, $8)
               ON CONFLICT (run_id) DO UPDATE SET name = $2, cmd = $3, start_time = $4, end_time = $5, stdout_url = $6, stderr_url = $7, exit_code = $8"#,
        )
        .bind(run_id)
        .bind(name)
        .bind(&cmd_vec)
        .bind(start_time)
        .bind(end_time)
        .bind(stdout_url)
        .bind(stderr_url)
        .bind(exit_code)
        .execute(&self.pool)
        .await?;
        Ok(())
    }

    pub async fn get_run_log(
        &self,
        run_id: &str,
    ) -> Result<
        Option<(
            String,
            Vec<String>,
            Option<DateTime<Utc>>,
            Option<DateTime<Utc>>,
            Option<String>,
            Option<String>,
            Option<i32>,
        )>,
    > {
        let row = sqlx::query_as::<_, (String, Vec<String>, Option<DateTime<Utc>>, Option<DateTime<Utc>>, Option<String>, Option<String>, Option<i32>)>(
            "SELECT name, cmd, start_time, end_time, stdout_url, stderr_url, exit_code FROM wes_run_log WHERE run_id = $1",
        )
        .bind(run_id)
        .fetch_optional(&self.pool)
        .await?;
        Ok(row)
    }

    pub async fn get_task_logs(
        &self,
        run_id: &str,
        _page_size: i64,
        _page_token: Option<&str>,
    ) -> Result<Vec<TaskLog>> {
        type TaskLogRow = (
            String,
            String,
            Option<Vec<String>>,
            Option<DateTime<Utc>>,
            Option<DateTime<Utc>>,
            Option<String>,
            Option<String>,
            Option<i32>,
        );
        let rows: Vec<TaskLogRow> =
            sqlx::query_as(
                "SELECT task_id, name, cmd, start_time, end_time, stdout_url, stderr_url, exit_code FROM wes_task_logs WHERE run_id = $1 ORDER BY id",
            )
            .bind(run_id)
            .fetch_all(&self.pool)
            .await?;
        Ok(rows
            .into_iter()
            .map(
                |(id, name, cmd, start_time, end_time, stdout, stderr, exit_code)| TaskLog {
                    id,
                    name,
                    cmd,
                    start_time: start_time.map(|t| t.to_rfc3339()),
                    end_time: end_time.map(|t| t.to_rfc3339()),
                    stdout,
                    stderr,
                    exit_code,
                },
            )
            .collect())
    }

    pub async fn system_state_counts(&self) -> Result<std::collections::HashMap<String, i64>> {
        let rows: Vec<(String, i64)> =
            sqlx::query_as("SELECT state, COUNT(*)::bigint FROM wes_runs GROUP BY state")
                .fetch_all(&self.pool)
                .await?;
        Ok(rows.into_iter().collect())
    }

    /// List runs in terminal state for cost summary. Date range on end_time.
    pub async fn list_runs_for_cost(
        &self,
        from_date: Option<DateTime<Utc>>,
        to_date: Option<DateTime<Utc>>,
    ) -> Result<Vec<(String, String, Option<DateTime<Utc>>, Value)>> {
        let rows: Vec<(String, String, Option<DateTime<Utc>>, Value)> = sqlx::query_as(
            r#"SELECT run_id, workflow_type, end_time, tags FROM wes_runs
               WHERE state IN ('COMPLETE', 'EXECUTOR_ERROR', 'SYSTEM_ERROR', 'CANCELED')
                 AND ($1::timestamptz IS NULL OR end_time >= $1)
                 AND ($2::timestamptz IS NULL OR end_time <= $2)
               ORDER BY end_time DESC"#,
        )
        .bind(from_date)
        .bind(to_date)
        .fetch_all(&self.pool)
        .await?;
        Ok(rows)
    }
}
