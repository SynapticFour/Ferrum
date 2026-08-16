// SPDX-License-Identifier: BUSL-1.1
//! Local operator accounts for shared Edge devices (PIN + field role).

use crate::error::{FerrumError, Result};
use crate::pool::FerrumPool;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

pub const ROLE_COLLECTOR: &str = "collector";
pub const ROLE_ANALYST: &str = "analyst";
pub const ROLE_SYNC_OPERATOR: &str = "sync_operator";

const VALID_ROLES: &[&str] = &[ROLE_COLLECTOR, ROLE_ANALYST, ROLE_SYNC_OPERATOR];

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EdgeOperatorAccount {
    pub id: String,
    pub username: String,
    pub role: String,
    pub created_time: String,
    pub disabled: bool,
}

pub fn normalize_role(role: &str) -> Result<String> {
    let r = role.trim().to_ascii_lowercase();
    if VALID_ROLES.contains(&r.as_str()) {
        Ok(r)
    } else {
        Err(FerrumError::ValidationError(format!(
            "role must be one of: {}",
            VALID_ROLES.join(", ")
        )))
    }
}

pub fn visa_for_role(role: &str) -> &'static str {
    match role {
        ROLE_COLLECTOR => crate::auth::VISA_COLLECTOR,
        ROLE_ANALYST => crate::auth::VISA_ANALYST,
        ROLE_SYNC_OPERATOR => crate::auth::VISA_SYNC_OPERATOR,
        _ => "ferrum:unknown",
    }
}

/// Mint a local HS256 bearer token for a verified Edge operator (offline shared device).
pub fn mint_local_token(
    account: &EdgeOperatorAccount,
    jwt_secret: &[u8],
    ttl_hours: u64,
) -> Result<String> {
    use jsonwebtoken::{encode, Algorithm, EncodingKey, Header};
    #[derive(serde::Serialize)]
    struct LocalClaims {
        sub: String,
        iss: String,
        exp: i64,
        iat: i64,
        scope: String,
    }
    let now = chrono::Utc::now().timestamp();
    let claims = LocalClaims {
        sub: account.username.clone(),
        iss: "ferrum-edge-local".into(),
        exp: now + (ttl_hours as i64 * 3600),
        iat: now,
        scope: visa_for_role(&account.role).to_string(),
    };
    encode(
        &Header::new(Algorithm::HS256),
        &claims,
        &EncodingKey::from_secret(jwt_secret),
    )
    .map_err(|e| FerrumError::ValidationError(format!("mint token: {e}")))
}

fn hash_pin(pin: &str, salt: &str) -> String {
    let mut hasher = Sha256::new();
    hasher.update(salt.as_bytes());
    hasher.update(pin.as_bytes());
    hex::encode(hasher.finalize())
}

pub async fn create_account(
    pool: &FerrumPool,
    username: &str,
    role: &str,
    pin: &str,
) -> Result<()> {
    if pin.len() < 4 {
        return Err(FerrumError::ValidationError(
            "PIN must be at least 4 characters".into(),
        ));
    }
    let role = normalize_role(role)?;
    let id = ulid::Ulid::new().to_string();
    let salt = hex::encode(rand_bytes(16));
    let pin_hash = hash_pin(pin, &salt);

    let sql =
        "INSERT INTO edge_operator_accounts (id, username, role, pin_hash, pin_salt, disabled)
               VALUES ($1, $2, $3, $4, $5, $6)";
    match pool {
        FerrumPool::Postgres(p) => {
            sqlx::query(sql)
                .bind(&id)
                .bind(username)
                .bind(&role)
                .bind(&pin_hash)
                .bind(&salt)
                .bind(false)
                .execute(p)
                .await?;
        }
        FerrumPool::Sqlite(p) => {
            sqlx::query(sql)
                .bind(&id)
                .bind(username)
                .bind(&role)
                .bind(&pin_hash)
                .bind(&salt)
                .bind(0i32)
                .execute(p)
                .await?;
        }
    }
    Ok(())
}

pub async fn list_accounts(pool: &FerrumPool) -> Result<Vec<EdgeOperatorAccount>> {
    let sql = "SELECT id, username, role, created_time, disabled FROM edge_operator_accounts ORDER BY username";
    match pool {
        FerrumPool::Postgres(p) => {
            let rows: Vec<(String, String, String, chrono::DateTime<chrono::Utc>, bool)> =
                sqlx::query_as(sql).fetch_all(p).await?;
            Ok(rows
                .into_iter()
                .map(
                    |(id, username, role, created_time, disabled)| EdgeOperatorAccount {
                        id,
                        username,
                        role,
                        created_time: created_time.to_rfc3339(),
                        disabled,
                    },
                )
                .collect())
        }
        FerrumPool::Sqlite(p) => {
            let rows: Vec<(String, String, String, String, i32)> =
                sqlx::query_as(sql).fetch_all(p).await?;
            Ok(rows
                .into_iter()
                .map(
                    |(id, username, role, created_time, disabled)| EdgeOperatorAccount {
                        id,
                        username,
                        role,
                        created_time,
                        disabled: disabled != 0,
                    },
                )
                .collect())
        }
    }
}

#[allow(clippy::type_complexity)]
pub async fn verify_account_pin(
    pool: &FerrumPool,
    username: &str,
    pin: &str,
) -> Result<EdgeOperatorAccount> {
    let sql =
        "SELECT id, username, role, pin_hash, pin_salt, created_time, disabled FROM edge_operator_accounts WHERE username = $1 LIMIT 1";
    match pool {
        FerrumPool::Postgres(p) => {
            let row: Option<(
                String,
                String,
                String,
                String,
                String,
                chrono::DateTime<chrono::Utc>,
                bool,
            )> = sqlx::query_as(sql).bind(username).fetch_optional(p).await?;
            let Some((id, username, role, pin_hash, pin_salt, created_time, disabled)) = row else {
                return Err(FerrumError::ValidationError("unknown account".into()));
            };
            if disabled {
                return Err(FerrumError::ValidationError("account disabled".into()));
            }
            if hash_pin(pin, &pin_salt) != pin_hash {
                return Err(FerrumError::ValidationError("invalid PIN".into()));
            }
            Ok(EdgeOperatorAccount {
                id,
                username,
                role,
                created_time: created_time.to_rfc3339(),
                disabled,
            })
        }
        FerrumPool::Sqlite(p) => {
            let row: Option<(String, String, String, String, String, String, i32)> =
                sqlx::query_as(sql).bind(username).fetch_optional(p).await?;
            let Some((id, username, role, pin_hash, pin_salt, created_time, disabled)) = row else {
                return Err(FerrumError::ValidationError("unknown account".into()));
            };
            if disabled != 0 {
                return Err(FerrumError::ValidationError("account disabled".into()));
            }
            if hash_pin(pin, &pin_salt) != pin_hash {
                return Err(FerrumError::ValidationError("invalid PIN".into()));
            }
            Ok(EdgeOperatorAccount {
                id,
                username,
                role,
                created_time,
                disabled: false,
            })
        }
    }
}

fn rand_bytes(n: usize) -> Vec<u8> {
    use std::time::{SystemTime, UNIX_EPOCH};
    let seed = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_nanos())
        .unwrap_or(0);
    (0..n).map(|i| ((seed >> (i * 8)) & 0xFF) as u8).collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::pool::FerrumPool;

    async fn sqlite_pool() -> FerrumPool {
        let pool = sqlx::sqlite::SqlitePoolOptions::new()
            .connect("sqlite::memory:")
            .await
            .unwrap();
        sqlx::migrate!("../ferrum-embed/migrations")
            .run(&pool)
            .await
            .unwrap();
        FerrumPool::Sqlite(pool)
    }

    #[tokio::test]
    async fn create_and_verify_account() {
        let pool = sqlite_pool().await;
        create_account(&pool, "alice", ROLE_COLLECTOR, "1234")
            .await
            .expect("create");
        let acct = verify_account_pin(&pool, "alice", "1234")
            .await
            .expect("verify");
        assert_eq!(acct.role, ROLE_COLLECTOR);
    }
}
