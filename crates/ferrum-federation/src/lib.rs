//! P2P federated Beacon queries without a central coordinator.

pub mod client;
pub mod rate_limit;
pub mod types;

pub use client::{query_envelope_from_params, BeaconQueryParams, FederationClient};
pub use types::{FederationRuntime, FerrumPeer, PeerQueryResult};
