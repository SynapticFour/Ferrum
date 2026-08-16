// SPDX-License-Identifier: BUSL-1.1
//! Shared DRS access checks (workspace-private, published dataset grants, outbreak approval).

use crate::error::{DrsError, Result};
use crate::state::AppState;
use ferrum_core::{is_workspace_member, AuthClaims, OutbreakService};
use std::sync::Arc;

/// Enforce workspace-private, dataset DAC, and outbreak download approval before byte access.
pub async fn check_object_byte_access(
    state: &AppState,
    canonical_object_id: &str,
    auth: Option<&AuthClaims>,
    federated_ads_base: Option<&str>,
) -> Result<()> {
    let dataset_id = state.repo.get_dataset_id(canonical_object_id).await?;
    let workspace_id = state.repo.get_workspace_id(canonical_object_id).await?;

    if let Some(dataset_id) = dataset_id {
        let claims = auth.ok_or_else(|| {
            DrsError::Forbidden("authentication required for this dataset".into())
        })?;
        if let Some(client) = state.ads_introspect.as_ref() {
            enforce_ads_dataset_access(
                client,
                &dataset_id,
                canonical_object_id,
                claims,
                federated_ads_base,
            )
            .await?;
        } else if !claims.has_published_dataset_access(&dataset_id, canonical_object_id)
            && !claims.is_admin()
        {
            return Err(DrsError::Forbidden("dataset access not granted".into()));
        }
    } else if let Some(ws_id) = workspace_id {
        let claims = auth.ok_or_else(|| {
            DrsError::Forbidden("authentication required for workspace-private data".into())
        })?;
        if !claims.is_admin() {
            let sub = claims
                .sub()
                .ok_or_else(|| DrsError::Forbidden("missing subject in token".into()))?;
            if !is_workspace_member(state.repo.pool(), &ws_id, sub).await? {
                return Err(DrsError::Forbidden(
                    "workspace-private object: not a workspace member".into(),
                ));
            }
        }
    }

    enforce_outbreak_download(
        state.outbreak.as_ref(),
        &state.repo,
        canonical_object_id,
        auth,
    )
    .await?;

    enforce_solum_object_consent(state, canonical_object_id).await
}

async fn enforce_solum_object_consent(state: &AppState, object_id: &str) -> Result<()> {
    let Some(client) = state.solum_consent.as_ref() else {
        return Ok(());
    };
    let metadata = state.repo.get_metadata(object_id).await?;
    let Some((subject, purpose)) = client.binding_from_metadata(&metadata) else {
        return Ok(());
    };
    client
        .require_granted(&subject, &purpose)
        .await
        .map_err(|e| DrsError::Forbidden(format!("solum consent: {e}")))
}

async fn enforce_ads_dataset_access(
    client: &ferrum_core::AdsIntrospectClient,
    dataset_id: &str,
    object_id: &str,
    claims: &AuthClaims,
    federated_ads_base: Option<&str>,
) -> Result<()> {
    if claims.is_admin() || claims.has_published_dataset_access(dataset_id, object_id) {
        return Ok(());
    }
    let token = claims
        .raw_token()
        .ok_or_else(|| DrsError::Forbidden("Bearer token required for ADS access check".into()))?;
    let resource = format!("drs:{object_id}");
    let active = if let Some(ads_base) = federated_ads_base {
        let policy = ferrum_core::SsrfPolicy {
            allow_private_networks: false,
            allowed_schemes: vec!["https".into()],
            ..Default::default()
        };
        ferrum_core::validate_url_ssrf_resolved(ads_base, &policy)
            .await
            .map_err(|e| DrsError::Forbidden(format!("ADS base URL rejected: {e}")))?;
        client
            .introspect_at_base(ads_base, token, &resource, dataset_id)
            .await
    } else {
        client
            .is_dataset_access_active(token, &resource, dataset_id)
            .await
    }
    .map_err(|e| DrsError::Forbidden(format!("ADS access check failed: {e}")))?;
    if !active {
        return Err(DrsError::Forbidden("dataset access not granted".into()));
    }
    Ok(())
}

/// Metadata GET uses the same rules as stream/download for controlled objects.
pub async fn check_object_metadata_access(
    state: &AppState,
    canonical_object_id: &str,
    auth: Option<&AuthClaims>,
    federated_ads_base: Option<&str>,
) -> Result<()> {
    check_object_byte_access(state, canonical_object_id, auth, federated_ads_base).await
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
