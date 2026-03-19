//! OWASP security: event logging, path sanitization, resource authorization.

use crate::error::{FerrumError, Result};
use async_trait::async_trait;
use serde::Serialize;
use std::path::{Path, PathBuf};

/// Security event for audit log (A09).
#[derive(Debug, Clone, Serialize)]
pub struct SecurityEvent {
    pub event_type: String,
    pub severity: String,
    pub sub: Option<String>,
    pub ip_address: Option<String>,
    pub resource_id: Option<String>,
    pub details: Option<serde_json::Value>,
}

impl SecurityEvent {
    pub fn access_denied(resource_id: &str, sub: Option<&str>, ip: Option<&str>) -> Self {
        Self {
            event_type: "access_denied".to_string(),
            severity: "warning".to_string(),
            sub: sub.map(String::from),
            ip_address: ip.map(String::from),
            resource_id: Some(resource_id.to_string()),
            details: None,
        }
    }
    pub fn auth_failure(ip: Option<&str>, details: Option<serde_json::Value>) -> Self {
        Self {
            event_type: "auth_failure".to_string(),
            severity: "warning".to_string(),
            sub: None,
            ip_address: ip.map(String::from),
            resource_id: None,
            details,
        }
    }
    pub fn path_traversal_attempt(path: &str, ip: Option<&str>) -> Self {
        Self {
            event_type: "path_traversal_attempt".to_string(),
            severity: "critical".to_string(),
            sub: None,
            ip_address: ip.map(String::from),
            resource_id: Some(path.to_string()),
            details: None,
        }
    }
    pub fn ssrf_attempt(url: &str, ip: Option<&str>) -> Self {
        Self {
            event_type: "ssrf_attempt".to_string(),
            severity: "critical".to_string(),
            sub: None,
            ip_address: ip.map(String::from),
            resource_id: Some(url.to_string()),
            details: None,
        }
    }
}

/// Logger for security events (persists to DB, optional webhook).
pub struct SecurityEventLogger {
    pool: sqlx::PgPool,
}

impl SecurityEventLogger {
    pub fn new(pool: sqlx::PgPool) -> Self {
        Self { pool }
    }

    pub async fn log(&self, event: SecurityEvent) -> Result<()> {
        let id = ulid::Ulid::new().to_string();
        let details = event.details.as_ref();
        sqlx::query(
            r#"INSERT INTO security_events (id, event_type, severity, sub, ip_address, resource_id, details)
               VALUES ($1, $2, $3, $4, $5, $6, $7)"#,
        )
        .bind(&id)
        .bind(&event.event_type)
        .bind(&event.severity)
        .bind(&event.sub)
        .bind(&event.ip_address)
        .bind(&event.resource_id)
        .bind(details)
        .execute(&self.pool)
        .await?;
        if event.severity == "critical" {
            tracing::error!(event_type = %event.event_type, resource_id = ?event.resource_id, "security critical event");
        }
        Ok(())
    }

    pub async fn critical(&self, event: SecurityEvent) -> Result<()> {
        let mut e = event;
        e.severity = "critical".to_string();
        self.log(e).await
    }
}

/// Path sanitization to prevent traversal (A03). Returns canonical path if within base.
pub fn safe_join(base: &Path, user_segment: &str) -> Result<PathBuf> {
    // Reject segments that could escape
    if user_segment.contains("..") || user_segment.contains('\0') {
        return Err(FerrumError::PathTraversal);
    }
    if user_segment.starts_with('/') || user_segment.contains('\\') {
        return Err(FerrumError::PathTraversal);
    }
    let joined = base.join(user_segment);
    let canonical_base = base
        .canonicalize()
        .map_err(|_| FerrumError::PathTraversal)?;
    let canonical = joined
        .canonicalize()
        .map_err(|_| FerrumError::PathTraversal)?;
    if !canonical.starts_with(&canonical_base) {
        return Err(FerrumError::PathTraversal);
    }
    Ok(canonical)
}

/// Validate DRS-style name/alias: no null bytes, limited charset, length (A03).
pub fn validate_drs_name(name: &str) -> Result<()> {
    if name.is_empty() || name.len() > 255 {
        return Err(FerrumError::ValidationError(
            "name length must be 1-255".to_string(),
        ));
    }
    if name.contains('\0') || name.chars().any(|c| c.is_control()) {
        return Err(FerrumError::ValidationError(
            "name must not contain control characters".to_string(),
        ));
    }
    if !name
        .chars()
        .all(|c| c.is_ascii_alphanumeric() || "._-/: ".contains(c))
    {
        return Err(FerrumError::ValidationError(
            "name has invalid characters".to_string(),
        ));
    }
    Ok(())
}

/// Resource authorization (A01). Implement per service.
#[async_trait]
pub trait ResourceAuthorizer: Send + Sync {
    async fn can_read(&self, sub: &str, resource_id: &str) -> Result<bool>;
    async fn can_write(&self, sub: &str, resource_id: &str) -> Result<bool>;
    async fn can_admin(&self, sub: &str) -> Result<bool>;
}
