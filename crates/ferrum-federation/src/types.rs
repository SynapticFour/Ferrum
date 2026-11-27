// SPDX-License-Identifier: Apache-2.0
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
            beacon_endpoint: parse_beacon_endpoint(&cfg.beacon_endpoint)?,
            public_key: cfg.public_key.clone(),
            timeout_ms: cfg.timeout_ms,
            service_token: cfg.service_token.clone(),
        })
    }
}

/// Parses a peer Beacon base URL and makes sure its path ends with `/`.
///
/// `Url::join` replaces the last path segment when the base has no trailing
/// slash. Without this, `https://peer/ga4gh/beacon/v2` joined with
/// `g_variants/query` became `https://peer/ga4gh/beacon/g_variants/query`, and
/// the peer answered 404. docs/FEDERATION.md writes the endpoint without the slash.
pub fn parse_beacon_endpoint(raw: &str) -> Result<Url, url::ParseError> {
    let mut url = Url::parse(raw)?;
    if !url.path().ends_with('/') {
        let path = format!("{}/", url.path());
        url.set_path(&path);
    }
    Ok(url)
}

#[cfg(test)]
mod tests {
    use super::parse_beacon_endpoint;

    #[test]
    fn endpoint_without_trailing_slash_keeps_v2_segment() {
        let base = parse_beacon_endpoint("https://peer.example/ga4gh/beacon/v2").unwrap();
        assert_eq!(
            base.join("g_variants/query").unwrap().as_str(),
            "https://peer.example/ga4gh/beacon/v2/g_variants/query"
        );
    }

    #[test]
    fn endpoint_with_trailing_slash_is_unchanged() {
        let base = parse_beacon_endpoint("https://peer.example/ga4gh/beacon/v2/").unwrap();
        assert_eq!(base.as_str(), "https://peer.example/ga4gh/beacon/v2/");
    }

    #[test]
    fn endpoint_query_string_is_kept() {
        let base = parse_beacon_endpoint("https://peer.example/beacon?x=1").unwrap();
        assert_eq!(base.path(), "/beacon/");
        assert_eq!(base.query(), Some("x=1"));
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
