//! TES task repository.

use crate::error::Result;
use crate::types::TaskState;
use chrono::{DateTime, Utc};
use serde_json::Value;
use sqlx::PgPool;

pub struct TesRepo {
    pool: PgPool,
}

impl TesRepo {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }

    pub async fn create(
        &self,
        id: &str,
        name: Option<&str>,
        description: Option<&str>,
        inputs: &Value,
        outputs: &Value,
        executors: &Value,
        resources: Option<&Value>,
        volumes: Option<&Value>,
        tags: Option<&Value>,
    ) -> Result<()> {
        sqlx::query(
            r#"INSERT INTO tes_tasks (id, state, name, description, inputs, outputs, executors, resources, volumes, tags)
               VALUES ($1, 'QUEUED', $2, $3, $4, $5, $6, $7, $8, $9)"#,
        )
        .bind(id)
        .bind(name)
        .bind(description)
        .bind(inputs)
        .bind(outputs)
        .bind(executors)
        .bind(resources)
        .bind(volumes)
        .bind(tags.unwrap_or(&serde_json::json!({})))
        .execute(&self.pool)
        .await?;
        Ok(())
    }

    pub async fn get(
        &self,
        id: &str,
    ) -> Result<
        Option<(
            String,
            String,
            Option<String>,
            Option<String>,
            Value,
            Value,
            Value,
            Option<Value>,
            Option<Value>,
            Option<Value>,
            Option<DateTime<Utc>>,
            Option<DateTime<Utc>>,
            DateTime<Utc>,
            Option<String>,
            Option<String>,
            Option<Value>,
        )>,
    > {
        let row = sqlx::query_as::<_, (
            String,
            String,
            Option<String>,
            Option<String>,
            Value,
            Value,
            Value,
            Option<Value>,
            Option<Value>,
            Option<Value>,
            Option<DateTime<Utc>>,
            Option<DateTime<Utc>>,
            DateTime<Utc>,
            Option<String>,
            Option<String>,
            Option<Value>,
        )>(
            r#"SELECT id, state, name, description, inputs, outputs, executors, resources, volumes, tags,
               started_at, ended_at, created_at, external_id, backend, logs
               FROM tes_tasks WHERE id = $1"#,
        )
        .bind(id)
        .fetch_optional(&self.pool)
        .await?;
        Ok(row)
    }

    pub async fn update_state(&self, id: &str, state: TaskState) -> Result<()> {
        let s = state.as_str();
        sqlx::query(
            r#"UPDATE tes_tasks SET state = $1,
               started_at = CASE WHEN $1 = 'RUNNING' AND started_at IS NULL THEN NOW() ELSE started_at END,
               ended_at = CASE WHEN $1 IN ('COMPLETE', 'EXECUTOR_ERROR', 'SYSTEM_ERROR', 'CANCELED') THEN NOW() ELSE ended_at END
               WHERE id = $2"#,
        )
        .bind(s)
        .bind(id)
        .execute(&self.pool)
        .await?;
        Ok(())
    }

    pub async fn set_external_id(&self, id: &str, external_id: &str, backend: &str) -> Result<()> {
        sqlx::query("UPDATE tes_tasks SET external_id = $1, backend = $2 WHERE id = $3")
            .bind(external_id)
            .bind(backend)
            .bind(id)
            .execute(&self.pool)
            .await?;
        Ok(())
    }

    pub async fn set_logs(&self, id: &str, logs: &Value) -> Result<()> {
        sqlx::query("UPDATE tes_tasks SET logs = $1 WHERE id = $2")
            .bind(logs)
            .bind(id)
            .execute(&self.pool)
            .await?;
        Ok(())
    }

    pub async fn list(
        &self,
        page_size: i64,
        page_token: Option<&str>,
        state_filter: Option<&str>,
    ) -> Result<(Vec<(String, String)>, Option<String>)> {
        let offset: i64 = page_token.and_then(|t| t.parse().ok()).unwrap_or(0);
        let rows: Vec<(String, String)> = sqlx::query_as::<_, (String, String)>(
            r#"SELECT id, state FROM tes_tasks
               WHERE ($1::text IS NULL OR state = $1)
               ORDER BY created_at DESC LIMIT $2 OFFSET $3"#,
        )
        .bind(state_filter)
        .bind(page_size + 1)
        .bind(offset)
        .fetch_all(&self.pool)
        .await?;
        let has_more = rows.len() as i64 > page_size;
        let tasks = rows.into_iter().take(page_size as usize).collect();
        let next = if has_more {
            Some((offset + page_size).to_string())
        } else {
            None
        };
        Ok((tasks, next))
    }
}
