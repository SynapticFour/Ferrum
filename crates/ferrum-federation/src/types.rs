//! Federation configuration types.

use ferrum_core::config::{AggregateStrategy, FederationConfig, FerrumPeerConfig};
use url::Url;

/// A configured Beacon peer for federated queries.
#[derive(Debug, Clone)]
pub struct FerrumPeer {
    pub name: String,
    pub beacon_endpoint: Url,
    pub public_key: Option<String>,
    pub timeout_ms: u64,
    pub service_token: Option<String>,
}

impl FerrumPeer {
    pub fn from_config(cfg: &FerrumPeerConfig) -> Result<Self, url::ParseError> {
        Ok(Self {
            name: cfg.name.clone(),
            beacon_endpoint: Url::parse(&cfg.beacon_endpoint)?,
            public_key: cfg.public_key.clone(),
            timeout_ms: cfg.timeout_ms,
            service_token: cfg.service_token.clone(),
        })
    }
}

/// Resolved federation settings used at runtime.
#[derive(Debug, Clone)]
pub struct FederationRuntime {
    pub enabled: bool,
    pub peers: Vec<FerrumPeer>,
    pub fan_out_parallel: bool,
    pub aggregate_strategy: AggregateStrategy,
    pub peer_requests_per_minute: u32,
}

impl FederationRuntime {
    pub fn from_config(cfg: &FederationConfig) -> Result<Self, url::ParseError> {
        let peers = cfg
            .peers
            .iter()
            .map(FerrumPeer::from_config)
            .collect::<Result<Vec<_>, _>>()?;
        Ok(Self {
            enabled: cfg.enabled,
            peers,
            fan_out_parallel: cfg.fan_out_parallel,
            aggregate_strategy: cfg.aggregate_strategy.clone(),
            peer_requests_per_minute: cfg.peer_requests_per_minute,
        })
    }

    pub fn disabled() -> Self {
        Self {
            enabled: false,
            peers: Vec::new(),
            fan_out_parallel: true,
            aggregate_strategy: AggregateStrategy::default(),
            peer_requests_per_minute: 10,
        }
    }
}

/// Result from a single peer query.
#[derive(Debug, Clone)]
pub struct PeerQueryResult {
    pub peer_name: String,
    pub exists: Option<bool>,
    pub count: Option<i64>,
    pub error: Option<String>,
}
