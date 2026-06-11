//! Shared DRS access checks (DAC, outbreak download approval).

use crate::error::{DrsError, Result};
use crate::state::AppState;
use ferrum_core::{AuthClaims, OutbreakService};
use std::sync::Arc;

/// Enforce dataset DAC and outbreak download approval before byte access.
pub async fn check_object_byte_access(
    state: &AppState,
    canonical_object_id: &str,
    auth: Option<&AuthClaims>,
) -> Result<()> {
    if let Some(dataset_id) = state.repo.get_dataset_id(canonical_object_id).await? {
        let claims = auth.ok_or_else(|| {
            DrsError::Forbidden("authentication required for this dataset".into())
        })?;
        if !claims.has_dataset_grant(&dataset_id) && !claims.is_admin() {
            return Err(DrsError::Forbidden("dataset access not granted".into()));
        }
    }
    enforce_outbreak_download(
        state.outbreak.as_ref(),
        &state.repo,
        canonical_object_id,
        auth,
    )
    .await
}

/// Metadata GET uses the same rules as stream/download for controlled objects.
pub async fn check_object_metadata_access(
    state: &AppState,
    canonical_object_id: &str,
    auth: Option<&AuthClaims>,
) -> Result<()> {
    check_object_byte_access(state, canonical_object_id, auth).await
}

async fn enforce_outbreak_download(
    outbreak: Option<&Arc<OutbreakService>>,
    repo: &crate::repo::DrsRepo,
    object_id: &str,
    auth: Option<&AuthClaims>,
) -> Result<()> {
    let Some(outbreak) = outbreak else {
        return Ok(());
    };
    if !outbreak.is_enabled() {
        return Ok(());
    }
    let Some(organism) = repo.pathogen_organism(object_id).await? else {
        return Ok(());
    };
    let claims = auth.ok_or_else(|| {
        DrsError::Forbidden("authentication required for outbreak-controlled pathogen data".into())
    })?;
    let recipient = claims
        .recipient_identity()
        .unwrap_or_else(|| claims.sub().unwrap_or("unknown"));
    let active = outbreak.active_policies().await?;
    for policy_name in active {
        let Some(policy) = outbreak.policy(&policy_name) else {
            continue;
        };
        if !organism_matches(&policy.trigger_pathogen, &organism) {
            continue;
        }
        if !outbreak
            .emergency_beacon_access(recipient, &organism)
            .await
            .unwrap_or(false)
        {
            continue;
        }
        if !outbreak
            .has_download_approval(&policy_name, object_id, recipient)
            .await?
        {
            return Err(DrsError::Forbidden(format!(
                "outbreak mode: DRS download for '{object_id}' requires POST /api/v1/outbreak/approve-download/{object_id}"
            )));
        }
    }
    Ok(())
}

fn organism_matches(policy_pathogen: &str, object_pathogen: &str) -> bool {
    policy_pathogen.eq_ignore_ascii_case(object_pathogen)
        || object_pathogen.contains(policy_pathogen)
        || policy_pathogen.contains(object_pathogen)
}
