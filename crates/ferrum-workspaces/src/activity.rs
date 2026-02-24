use crate::error::Result;

pub struct ActivityLogger {
    pool: sqlx::PgPool,
}

impl ActivityLogger {
    pub fn new(pool: sqlx::PgPool) -> Self {
        Self { pool }
    }

    pub async fn log(
        &self,
        workspace_id: &str,
        sub: &str,
        action: &str,
        resource_type: Option<&str>,
        resource_id: Option<&str>,
        details: serde_json::Value,
    ) -> Result<()> {
        let id = ulid::Ulid::new().to_string();
        sqlx::query(
            r#"INSERT INTO workspace_activity (id, workspace_id, sub, action, resource_type, resource_id, details)
               VALUES ($1, $2, $3, $4, $5, $6, $7)"#,
        )
        .bind(&id)
        .bind(workspace_id)
        .bind(sub)
        .bind(action)
        .bind(resource_type)
        .bind(resource_id)
        .bind(&details)
        .execute(&self.pool)
        .await?;
        Ok(())
    }
}
