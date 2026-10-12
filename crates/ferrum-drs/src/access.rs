// SPDX-License-Identifier: BUSL-1.1
//! Shared DRS access checks (workspace-private, published dataset grants, outbreak approval).

use crate::error::{DrsError, Result};
use crate::state::AppState;
use ferrum_core::{is_workspace_member, AuthClaims, DeniedReason, OutbreakService};
use std::sync::Arc;

fn record_denied(state: &AppState, reason: DeniedReason, auth: Option<&AuthClaims>) {
    if let Some(sink) = state.denied_audit.as_ref() {
        sink.record_spawn(
            reason,
            auth.and_then(|a| a.sub().map(str::to_string)),
            None,
            None,
        );
    }
}

fn forbidden(state: &AppState, auth: Option<&AuthClaims>, msg: impl Into<String>) -> DrsError {
    record_denied(state, DeniedReason::Forbidden, auth);
    DrsError::Forbidden(msg.into())
}

fn missing_bearer(state: &AppState, msg: impl Into<String>) -> DrsError {
    record_denied(state, DeniedReason::MissingBearer, None);
    DrsError::Unauthorized(msg.into())
}

/// Enforce workspace-private, dataset DAC, and outbreak download approval before byte access.
pub async fn check_object_byte_access(
    state: &AppState,
    canonical_object_id: &str,
    auth: Option<&AuthClaims>,
    federated_ads_base: Option<&str>,
) -> Result<()> {
    let dataset_id = state.repo.get_dataset_id(canonical_object_id).await?;
    let workspace_id = state.repo.get_workspace_id(canonical_object_id).await?;

    // ADR-030: optional Passport for all byte paths (default off).
    if state.require_passport_for_bytes && auth.is_none() {
        return Err(missing_bearer(
            state,
            "Bearer authentication required for object byte access",
        ));
    }

    if let Some(dataset_id) = dataset_id {
        let claims =
            auth.ok_or_else(|| forbidden(state, None, "authentication required for this dataset"))?;
        if let Some(client) = state.ads_introspect.as_ref() {
            enforce_ads_dataset_access(
                state,
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
            return Err(forbidden(state, Some(claims), "dataset access not granted"));
        }
    } else if let Some(ws_id) = workspace_id {
        let claims = auth.ok_or_else(|| {
            forbidden(
                state,
                None,
                "authentication required for workspace-private data",
            )
        })?;
        if !claims.is_admin() {
            let sub = claims
                .sub()
                .ok_or_else(|| forbidden(state, Some(claims), "missing subject in token"))?;
            if !is_workspace_member(state.repo.pool(), &ws_id, sub).await? {
                return Err(forbidden(
                    state,
                    Some(claims),
                    "workspace-private object: not a workspace member",
                ));
            }
        }
    }

    enforce_outbreak_download(
        state,
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
        .map_err(|e| forbidden(state, None, format!("solum consent: {e}")))
}

async fn enforce_ads_dataset_access(
    state: &AppState,
    client: &ferrum_core::AdsIntrospectClient,
    dataset_id: &str,
    object_id: &str,
    claims: &AuthClaims,
    federated_ads_base: Option<&str>,
) -> Result<()> {
    if claims.is_admin() || claims.has_published_dataset_access(dataset_id, object_id) {
        return Ok(());
    }
    let token = claims.raw_token().ok_or_else(|| {
        forbidden(
            state,
            Some(claims),
            "Bearer token required for ADS access check",
        )
    })?;
    let resource = format!("drs:{object_id}");
    let active = if let Some(ads_base) = federated_ads_base {
        let policy = ferrum_core::SsrfPolicy {
            allow_private_networks: false,
            allowed_schemes: vec!["https".into()],
            ..Default::default()
        };
        ferrum_core::validate_url_ssrf_resolved(ads_base, &policy)
            .await
            .map_err(|e| forbidden(state, Some(claims), format!("ADS base URL rejected: {e}")))?;
        client
            .introspect_at_base(ads_base, token, &resource, dataset_id)
            .await
    } else {
        client
            .is_dataset_access_active(token, &resource, dataset_id)
            .await
    }
    .map_err(|e| forbidden(state, Some(claims), format!("ADS access check failed: {e}")))?;
    if !active {
        return Err(forbidden(state, Some(claims), "dataset access not granted"));
    }
    Ok(())
}

/// Metadata GET: controlled objects use the same rules as bytes, but
/// `require_passport_for_bytes` does **not** close anonymous catalog/metadata
/// for unbound objects (ADR-030).
pub async fn check_object_metadata_access(
    state: &AppState,
    canonical_object_id: &str,
    auth: Option<&AuthClaims>,
    federated_ads_base: Option<&str>,
) -> Result<()> {
    let dataset_id = state.repo.get_dataset_id(canonical_object_id).await?;
    let workspace_id = state.repo.get_workspace_id(canonical_object_id).await?;
    if dataset_id.is_none() && workspace_id.is_none() {
        enforce_outbreak_download(
            state,
            state.outbreak.as_ref(),
            &state.repo,
            canonical_object_id,
            auth,
        )
        .await?;
        return enforce_solum_object_consent(state, canonical_object_id).await;
    }
    // Controlled objects already require auth for dataset/workspace; byte flag is a no-op here.
    check_object_byte_access(state, canonical_object_id, auth, federated_ads_base).await
}

async fn enforce_outbreak_download(
    state: &AppState,
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
        forbidden(
            state,
            None,
            "authentication required for outbreak-controlled pathogen data",
        )
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
            return Err(forbidden(
                state,
                Some(claims),
                format!(
                    "outbreak mode: DRS download for '{object_id}' requires POST /api/v1/outbreak/approve-download/{object_id}"
                ),
            ));
        }
    }
    Ok(())
}

fn organism_matches(policy_pathogen: &str, object_pathogen: &str) -> bool {
    policy_pathogen.eq_ignore_ascii_case(object_pathogen)
        || object_pathogen.contains(policy_pathogen)
        || policy_pathogen.contains(object_pathogen)
}
