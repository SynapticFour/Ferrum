//! Request extractors for workspace handlers.

use crate::error::WorkspaceError;
use axum::extract::FromRequestParts;
use axum::http::request::Parts;
use ferrum_core::AuthClaims;

/// Authenticated user claims (401 when missing — e.g. expired or invalid Passport).
pub struct RequireAuth(pub AuthClaims);

#[async_trait::async_trait]
impl<S> FromRequestParts<S> for RequireAuth
where
    S: Send + Sync,
{
    type Rejection = WorkspaceError;

    async fn from_request_parts(parts: &mut Parts, _state: &S) -> Result<Self, Self::Rejection> {
        parts
            .extensions
            .get::<AuthClaims>()
            .cloned()
            .map(RequireAuth)
            .ok_or_else(|| {
                WorkspaceError::Unauthorized(
                    "Sign in required — your session may have expired. Please log in again."
                        .to_string(),
                )
            })
    }
}
