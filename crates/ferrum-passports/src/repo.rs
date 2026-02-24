//! Database access for visa grants and auth codes.

use crate::error::Result;
use crate::types::VisaGrantRow;
use sqlx::FromRow;
use sqlx::PgPool;
use uuid::Uuid;

pub struct PassportRepo {
    pool: PgPool,
}

#[derive(FromRow)]
struct VisaGrantRowQuery {
    id: Uuid,
    user_sub: String,
    user_iss: String,
    dataset_id: String,
    visa_type: String,
    value: String,
    source: String,
    conditions: Option<serde_json::Value>,
    expires_at: Option<chrono::DateTime<chrono::Utc>>,
}

#[derive(FromRow)]
struct AuthCodeRow {
    sub: String,
    iss: String,
    scope: String,
    redirect_uri: String,
}

impl PassportRepo {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }

    /// List visa grants for a user (sub, iss). Excludes expired.
    pub async fn list_visa_grants(&self, user_sub: &str, user_iss: &str) -> Result<Vec<VisaGrantRow>> {
        let rows = sqlx::query_as::<_, VisaGrantRowQuery>(
            r#"
            SELECT id, user_sub, user_iss, dataset_id, visa_type, value, source, conditions, expires_at
            FROM passport_visa_grants
            WHERE user_sub = $1 AND user_iss = $2 AND (expires_at IS NULL OR expires_at > NOW())
            ORDER BY created_at
            "#,
        )
        .bind(user_sub)
        .bind(user_iss)
        .fetch_all(&self.pool)
        .await?;
        Ok(rows
            .into_iter()
            .map(|r| VisaGrantRow {
                id: r.id,
                user_sub: r.user_sub,
                user_iss: r.user_iss,
                dataset_id: r.dataset_id,
                visa_type: r.visa_type,
                value: r.value,
                source: r.source,
                conditions: r.conditions,
                expires_at: r.expires_at,
            })
            .collect())
    }

    /// Create a visa grant (admin).
    #[allow(clippy::too_many_arguments)]
    pub async fn create_visa_grant(
        &self,
        user_sub: &str,
        user_iss: &str,
        dataset_id: &str,
        visa_type: &str,
        value: &str,
        source: &str,
        conditions: Option<serde_json::Value>,
        expires_at: Option<chrono::DateTime<chrono::Utc>>,
    ) -> Result<Uuid> {
        let id = Uuid::new_v4();
        sqlx::query(
            r#"
            INSERT INTO passport_visa_grants (id, user_sub, user_iss, dataset_id, visa_type, value, source, conditions, expires_at)
            VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9)
            "#,
        )
        .bind(id)
        .bind(user_sub)
        .bind(user_iss)
        .bind(dataset_id)
        .bind(visa_type)
        .bind(value)
        .bind(source)
        .bind(conditions)
        .bind(expires_at)
        .execute(&self.pool)
        .await?;
        Ok(id)
    }

    /// Delete a visa grant by id.
    pub async fn delete_visa_grant(&self, id: Uuid) -> Result<bool> {
        let r = sqlx::query("DELETE FROM passport_visa_grants WHERE id = $1")
            .bind(id)
            .execute(&self.pool)
            .await?;
        Ok(r.rows_affected() > 0)
    }

    /// List all visa grants (admin), with optional filters.
    pub async fn list_all_visa_grants(
        &self,
        user_sub: Option<&str>,
        dataset_id: Option<&str>,
    ) -> Result<Vec<VisaGrantRow>> {
        let rows = if let Some(sub) = user_sub {
            sqlx::query_as::<_, VisaGrantRowQuery>(
                "SELECT id, user_sub, user_iss, dataset_id, visa_type, value, source, conditions, expires_at FROM passport_visa_grants WHERE user_sub = $1 ORDER BY created_at",
            )
            .bind(sub)
            .fetch_all(&self.pool)
            .await?
        } else if let Some(ds) = dataset_id {
            sqlx::query_as::<_, VisaGrantRowQuery>(
                "SELECT id, user_sub, user_iss, dataset_id, visa_type, value, source, conditions, expires_at FROM passport_visa_grants WHERE dataset_id = $1 ORDER BY created_at",
            )
            .bind(ds)
            .fetch_all(&self.pool)
            .await?
        } else {
            sqlx::query_as::<_, VisaGrantRowQuery>(
                "SELECT id, user_sub, user_iss, dataset_id, visa_type, value, source, conditions, expires_at FROM passport_visa_grants ORDER BY created_at",
            )
            .fetch_all(&self.pool)
            .await?
        };
        Ok(rows
            .into_iter()
            .map(|r| VisaGrantRow {
                id: r.id,
                user_sub: r.user_sub,
                user_iss: r.user_iss,
                dataset_id: r.dataset_id,
                visa_type: r.visa_type,
                value: r.value,
                source: r.source,
                conditions: r.conditions,
                expires_at: r.expires_at,
            })
            .collect())
    }

    /// Store auth code for OAuth flow.
    #[allow(clippy::too_many_arguments)]
    pub async fn store_auth_code(
        &self,
        code: &str,
        client_id: &str,
        sub: &str,
        iss: &str,
        scope: &str,
        redirect_uri: &str,
        expires_at: chrono::DateTime<chrono::Utc>,
    ) -> Result<()> {
        sqlx::query(
            r#"
            INSERT INTO passport_auth_codes (code, client_id, sub, iss, scope, redirect_uri, expires_at)
            VALUES ($1, $2, $3, $4, $5, $6, $7)
            "#,
        )
        .bind(code)
        .bind(client_id)
        .bind(sub)
        .bind(iss)
        .bind(scope)
        .bind(redirect_uri)
        .bind(expires_at)
        .execute(&self.pool)
        .await?;
        Ok(())
    }

    /// Consume auth code and return (sub, iss, scope, redirect_uri).
    pub async fn consume_auth_code(
        &self,
        code: &str,
    ) -> Result<Option<(String, String, String, String)>> {
        let row = sqlx::query_as::<_, AuthCodeRow>(
            "SELECT sub, iss, scope, redirect_uri FROM passport_auth_codes WHERE code = $1 AND expires_at > NOW()",
        )
        .bind(code)
        .fetch_optional(&self.pool)
        .await?;
        if let Some(r) = row {
            sqlx::query("DELETE FROM passport_auth_codes WHERE code = $1")
                .bind(code)
                .execute(&self.pool)
                .await?;
            return Ok(Some((r.sub, r.iss, r.scope, r.redirect_uri)));
        }
        Ok(None)
    }
}
