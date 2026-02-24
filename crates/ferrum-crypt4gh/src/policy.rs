//! Layer 3: Policy engine — DataAccessPolicy, visa checks, ControlledAccessGrants, cache with TTL.

use ferrum_core::auth::VisaObject;
use serde::{Deserialize, Serialize};
use std::time::{Duration, Instant};
use dashmap::DashMap;

/// GA4GH ControlledAccessGrants visa type (value often encodes dataset/grant).
pub const VISA_TYPE_CONTROLLED_ACCESS_GRANTS: &str = "ControlledAccessGrants";

/// Policy for a single object: which visa type and optional dataset/purposes.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DataAccessPolicy {
    pub object_id: String,
    /// Required visa type (e.g. ControlledAccessGrants).
    pub required_visa_type: String,
    /// If set, visa value must match this dataset identifier.
    pub required_dataset: Option<String>,
    /// Allowed purposes (empty = any). If non-empty, visa conditions or value must align.
    pub allowed_purposes: Vec<String>,
}

/// Result of a policy check (cached with expiry).
#[derive(Clone)]
struct CachedDecision {
    allowed: bool,
    expires_at: Instant,
}

/// Policy engine: checks Passport Visas against policies, with optional TTL cache.
pub struct PolicyEngine {
    policies: DashMap<String, DataAccessPolicy>,
    cache: DashMap<(String, String), CachedDecision>,
    cache_ttl: Duration,
}

impl PolicyEngine {
    pub fn new(cache_ttl_secs: u64) -> Self {
        Self {
            policies: DashMap::new(),
            cache: DashMap::new(),
            cache_ttl: Duration::from_secs(cache_ttl_secs),
        }
    }

    pub fn add_policy(&self, policy: DataAccessPolicy) {
        self.policies.insert(policy.object_id.clone(), policy);
    }

    pub fn remove_policy(&self, object_id: &str) -> Option<DataAccessPolicy> {
        self.policies.remove(object_id).map(|(_, v)| v)
    }

    pub fn get_policy(&self, object_id: &str) -> Option<DataAccessPolicy> {
        self.policies.get(object_id).map(|r| r.clone())
    }

    /// Check whether the given visas satisfy the policy for the object.
    /// Supports ControlledAccessGrants: visa type must match and (if required_dataset set) value/dataset must match.
    pub fn check(&self, object_id: &str, visas: &[VisaObject], subject_id: &str) -> bool {
        let cache_key = (object_id.to_string(), subject_id.to_string());
        if let Some(cached) = self.cache.get(&cache_key) {
            if cached.expires_at > Instant::now() {
                return cached.allowed;
            }
            self.cache.remove(&cache_key);
        }

        let allowed = self.check_uncached(object_id, visas);
        self.cache.insert(
            cache_key,
            CachedDecision {
                allowed,
                expires_at: Instant::now() + self.cache_ttl,
            },
        );
        allowed
    }

    fn check_uncached(&self, object_id: &str, visas: &[VisaObject]) -> bool {
        let Some(policy) = self.policies.get(object_id) else {
            return false;
        };

        for visa in visas {
            if visa.r#type != policy.required_visa_type {
                continue;
            }
            if let Some(ref required) = policy.required_dataset {
                if !visa.value.contains(required) && visa.value != *required {
                    continue;
                }
            }
            if !policy.allowed_purposes.is_empty() {
                // Optional: check visa conditions for purpose
                if let Some(ref conds) = visa.conditions {
                    let has_purpose = policy.allowed_purposes.iter().any(|p| {
                        conds.iter().any(|c| {
                            c.get("type").and_then(|t| t.as_str()) == Some("Purpose")
                                && c.get("value").and_then(|v| v.as_str()) == Some(p.as_str())
                        })
                    });
                    if !has_purpose {
                        continue;
                    }
                }
            }
            return true;
        }
        false
    }
}

impl Default for PolicyEngine {
    fn default() -> Self {
        Self::new(300)
    }
}
