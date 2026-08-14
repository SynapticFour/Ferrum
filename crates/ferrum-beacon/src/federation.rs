//! Federated Beacon query helpers.

use crate::handlers::{
    AppState, VariantQueryResponse, VariantQueryResult, BEACON_RESPONSE_META_SCHEMA,
};
use ferrum_federation::{query_envelope_from_params, BeaconQueryParams};
use serde_json::{json, Value};

pub async fn maybe_federate_get(
    state: &AppState,
    params: BeaconQueryParams,
    federate: bool,
    local_exists: Option<bool>,
    local_count: Option<i64>,
    requester: Option<&str>,
) -> VariantQueryResponse {
    let mut meta = json!({
        "$schema": BEACON_RESPONSE_META_SCHEMA,
        "apiVersion": "v2.0.0",
        "requestedSchemas": [],
    });
    if !federate {
        return VariantQueryResponse {
            meta,
            response: VariantQueryResult {
                exists: local_exists,
                count: local_count,
            },
        };
    }
    let Some(ref federation) = state.federation else {
        return VariantQueryResponse {
            meta,
            response: VariantQueryResult {
                exists: local_exists,
                count: local_count,
            },
        };
    };
    if !federation.is_enabled() {
        return VariantQueryResponse {
            meta,
            response: VariantQueryResult {
                exists: local_exists,
                count: local_count,
            },
        };
    }

    let envelope = query_envelope_from_params(&params);
    let peer_results = federation.query_peers(&envelope).await;

    if let Some(ref audit) = state.residency_audit {
        for pr in &peer_results {
            if pr.error.is_none() {
                audit
                    .append_warn(
                        "peer_query_sent",
                        None,
                        requester,
                        Some(&pr.peer_name),
                        false,
                        None,
                    )
                    .await;
            }
        }
    }

    let (exists, count, warnings) = federation.aggregate(local_exists, local_count, &peer_results);

    if !warnings.is_empty() {
        meta["warnings"] = json!(warnings);
    }
    if federation.runtime().aggregate_strategy == ferrum_core::AggregateStrategy::LocalFirst {
        let peer_payload: Vec<Value> = peer_results
            .iter()
            .map(|p| {
                json!({
                    "peer": p.peer_name,
                    "exists": p.exists,
                    "count": p.count,
                    "error": p.error,
                })
            })
            .collect();
        meta["peerResults"] = json!(peer_payload);
    }

    VariantQueryResponse {
        meta,
        response: VariantQueryResult { exists, count },
    }
}
