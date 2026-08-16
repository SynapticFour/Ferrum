// SPDX-License-Identifier: BUSL-1.1
//! Federated Beacon query fan-out.

use crate::rate_limit::PeerRateLimiter;
use crate::types::{FederationRuntime, PeerQueryResult};
use ferrum_core::config::AggregateStrategy;
use reqwest::Client;
use serde_json::{json, Value};
use std::sync::Arc;
use std::time::Duration;

pub struct FederationClient {
    runtime: FederationRuntime,
    http: Client,
    rate_limiter: Arc<PeerRateLimiter>,
}

impl FederationClient {
    pub fn new(runtime: FederationRuntime) -> Self {
        let budget = runtime.peer_requests_per_minute;
        Self {
            runtime,
            http: Client::builder()
                .timeout(Duration::from_secs(30))
                .build()
                .expect("reqwest client"),
            rate_limiter: Arc::new(PeerRateLimiter::new(budget)),
        }
    }

    pub fn is_enabled(&self) -> bool {
        self.runtime.enabled && !self.runtime.peers.is_empty()
    }

    pub fn runtime(&self) -> &FederationRuntime {
        &self.runtime
    }

    /// Fan out a Beacon g_variants query envelope to configured peers.
    pub async fn query_peers(&self, envelope: &Value) -> Vec<PeerQueryResult> {
        if !self.is_enabled() {
            return Vec::new();
        }
        if self.runtime.fan_out_parallel {
            let mut handles = Vec::new();
            for peer in &self.runtime.peers {
                let peer = peer.clone();
                let envelope = envelope.clone();
                let http = self.http.clone();
                let limiter = Arc::clone(&self.rate_limiter);
                handles.push(tokio::spawn(async move {
                    query_one_peer(&http, &limiter, &peer, &envelope).await
                }));
            }
            let mut results = Vec::new();
            for h in handles {
                if let Ok(r) = h.await {
                    results.push(r);
                }
            }
            results
        } else {
            let mut results = Vec::new();
            for peer in &self.runtime.peers {
                results.push(query_one_peer(&self.http, &self.rate_limiter, peer, envelope).await);
            }
            results
        }
    }

    pub fn aggregate(
        &self,
        local_exists: Option<bool>,
        local_count: Option<i64>,
        peer_results: &[PeerQueryResult],
    ) -> (Option<bool>, Option<i64>, Vec<String>) {
        let mut warnings = Vec::new();
        for pr in peer_results {
            if let Some(ref err) = pr.error {
                warnings.push(format!("peer '{}' unavailable: {}", pr.peer_name, err));
            }
        }
        let peer_exists: Vec<bool> = peer_results
            .iter()
            .filter(|p| p.error.is_none())
            .filter_map(|p| p.exists)
            .collect();
        let peer_counts: Vec<i64> = peer_results
            .iter()
            .filter(|p| p.error.is_none())
            .filter_map(|p| p.count)
            .collect();

        let exists = match self.runtime.aggregate_strategy {
            AggregateStrategy::Union => {
                let local = local_exists.unwrap_or(false);
                let any_peer = peer_exists.iter().any(|&e| e);
                Some(local || any_peer)
            }
            AggregateStrategy::Intersection => {
                let local = local_exists.unwrap_or(false);
                if peer_exists.is_empty() {
                    Some(local)
                } else {
                    Some(local && peer_exists.iter().all(|&e| e))
                }
            }
            AggregateStrategy::LocalFirst => local_exists,
        };

        let count = match self.runtime.aggregate_strategy {
            AggregateStrategy::Union => {
                let lc = local_count.unwrap_or(0);
                let pc: i64 = peer_counts.iter().sum();
                if local_count.is_some() || !peer_counts.is_empty() {
                    Some(lc + pc)
                } else {
                    None
                }
            }
            AggregateStrategy::Intersection => {
                if peer_counts.is_empty() {
                    local_count
                } else {
                    let mut min = local_count.unwrap_or(i64::MAX);
                    for c in peer_counts {
                        min = min.min(c);
                    }
                    Some(min)
                }
            }
            AggregateStrategy::LocalFirst => local_count,
        };

        (exists, count, warnings)
    }
}

async fn query_one_peer(
    http: &Client,
    limiter: &PeerRateLimiter,
    peer: &crate::types::FerrumPeer,
    envelope: &Value,
) -> PeerQueryResult {
    if !limiter.allow(&peer.name) {
        return PeerQueryResult {
            peer_name: peer.name.clone(),
            exists: None,
            count: None,
            error: Some("rate limit exceeded".into()),
        };
    }
    let url = peer
        .beacon_endpoint
        .join("g_variants/query")
        .unwrap_or_else(|_| peer.beacon_endpoint.clone());
    let timeout = Duration::from_millis(peer.timeout_ms.max(100));
    let mut req = http.post(url).json(envelope).timeout(timeout);
    if let Some(ref token) = peer.service_token {
        req = req.bearer_auth(token);
    }
    match req.send().await {
        Ok(resp) if resp.status().is_success() => match resp.json::<Value>().await {
            Ok(body) => {
                let exists = body.pointer("/response/exists").and_then(|v| v.as_bool());
                let count = body.pointer("/response/count").and_then(|v| v.as_i64());
                PeerQueryResult {
                    peer_name: peer.name.clone(),
                    exists,
                    count,
                    error: None,
                }
            }
            Err(e) => PeerQueryResult {
                peer_name: peer.name.clone(),
                exists: None,
                count: None,
                error: Some(format!("invalid JSON: {e}")),
            },
        },
        Ok(resp) => PeerQueryResult {
            peer_name: peer.name.clone(),
            exists: None,
            count: None,
            error: Some(format!("HTTP {}", resp.status())),
        },
        Err(e) => PeerQueryResult {
            peer_name: peer.name.clone(),
            exists: None,
            count: None,
            error: Some(e.to_string()),
        },
    }
}

/// Build a minimal Beacon query envelope for federation fan-out.
pub fn query_envelope_from_params(params: &BeaconQueryParams) -> Value {
    json!({
        "meta": { "apiVersion": "v2.0.0" },
        "query": {
            "requestParameters": {
                "assemblyId": params.assembly_id,
                "referenceName": params.reference_name,
                "start": params.start,
                "end": params.end,
                "referenceBases": params.reference_bases,
                "alternateBases": params.alternate_bases,
                "requestedGranularity": params.granularity,
                "organism": params.organism,
                "amrGene": params.amr_gene,
                "serotype": params.serotype,
                "minQscore": params.min_qscore,
            }
        }
    })
}

#[derive(Debug, Clone, Default)]
pub struct BeaconQueryParams {
    pub assembly_id: Option<String>,
    pub reference_name: Option<String>,
    pub start: Option<i64>,
    pub end: Option<i64>,
    pub reference_bases: Option<String>,
    pub alternate_bases: Option<String>,
    pub granularity: Option<String>,
    pub organism: Option<String>,
    pub amr_gene: Option<String>,
    pub serotype: Option<String>,
    pub min_qscore: Option<f32>,
}
