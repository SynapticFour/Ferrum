//! OWASP A03: Input validation for workspace and invite data (injection, header injection, length).

use crate::error::{Result, WorkspaceError};

/// Max length for an email address (RFC 5321).
const EMAIL_MAX_LEN: usize = 254;

/// Invite token is 32 hex chars; reject anything else to avoid probing.
const INVITE_TOKEN_LEN: usize = 32;

/// Validates email for invite: length, no control chars or newlines (SMTP header injection).
pub fn validate_invite_email(email: &str) -> Result<()> {
    let trimmed = email.trim();
    if trimmed.is_empty() {
        return Err(WorkspaceError::Validation("email required".to_string()));
    }
    if trimmed.len() > EMAIL_MAX_LEN {
        return Err(WorkspaceError::Validation("email too long".to_string()));
    }
    if trimmed.contains('\0') || trimmed.contains('\n') || trimmed.contains('\r') {
        return Err(WorkspaceError::Validation("email contains invalid characters".to_string()));
    }
    if trimmed.chars().any(|c| c.is_control()) {
        return Err(WorkspaceError::Validation("email contains invalid characters".to_string()));
    }
    // Basic format: at least one @
    if !trimmed.contains('@') {
        return Err(WorkspaceError::Validation("invalid email format".to_string()));
    }
    Ok(())
}

/// Validates workspace slug when user-provided: alphanumeric, dash, underscore only; 1–64 chars.
pub fn validate_workspace_slug(slug: &str) -> Result<()> {
    let s = slug.trim();
    if s.is_empty() {
        return Err(WorkspaceError::Validation("slug cannot be empty".to_string()));
    }
    if s.len() > 64 {
        return Err(WorkspaceError::Validation("slug too long".to_string()));
    }
    if !s
        .chars()
        .all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_')
    {
        return Err(WorkspaceError::Validation(
            "slug must contain only letters, numbers, dash and underscore".to_string(),
        ));
    }
    Ok(())
}

/// Returns a string safe for use in plain-text email body: no control chars, truncated length.
pub fn sanitize_for_email_body(s: &str, max_len: usize) -> String {
    let out: String = s
        .chars()
        .filter(|c| !c.is_control() && *c != '\0')
        .take(max_len)
        .collect();
    out.trim_end().to_string()
}

/// Validates invite token path segment: exact length and hex charset to avoid injection/probing.
pub fn validate_invite_token(token: &str) -> Result<()> {
    if token.len() != INVITE_TOKEN_LEN {
        return Err(WorkspaceError::NotFound("invite expired or invalid".to_string()));
    }
    if !token.chars().all(|c| c.is_ascii_hexdigit()) {
        return Err(WorkspaceError::NotFound("invite expired or invalid".to_string()));
    }
    Ok(())
}
