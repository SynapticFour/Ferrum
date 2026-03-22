//! Normalization of `drs_access_methods.access_url` JSONB for resolving download URLs.
//!
//! Write paths (create/ingest) store either a JSON string or a GA4GH-style object
//! `{"url": "https://..."}`. Two read paths share this module:
//!
//! - **[`parse_stored_access_url`]** — resolve a **single URL string** for
//!   [`crate::repo::DrsRepo::get_access_url`] / `GET .../access/{access_id}`.
//! - **[`jsonb_to_core_access_url_for_listing`]** — map JSONB into
//!   [`ferrum_core::AccessUrl`] for `GET .../objects/{id}` access method listings
//!   (string or object shape preserved).
//!
//! **Invariant:** Any JSONB shape accepted here for URL resolution must stay aligned with
//! [`DrsRepo::get_access_url`](crate::repo::DrsRepo::get_access_url) and ingest/create writers
//! (see unit tests).

use ferrum_core::AccessUrl;
use serde_json::Value;

/// Map stored `access_url` JSONB to GA4GH `AccessMethod.access_url` for object metadata (`GET object`).
///
/// Accepts a JSON **string** or **object** (including `{"url": "..."}` with optional extra keys).
/// This does **not** validate that a `url` field exists inside objects; use [`parse_stored_access_url`]
/// when a concrete download URL is required.
pub fn jsonb_to_core_access_url_for_listing(value: &Value) -> Option<AccessUrl> {
    match value {
        Value::String(s) => Some(AccessUrl::String(s.clone())),
        Value::Object(map) => Some(AccessUrl::Object(map.clone())),
        _ => None,
    }
}

/// Extract the URL string from stored `access_url` JSONB.
///
/// Supported shapes:
/// - JSON string: `"https://host/path"`
/// - JSON object with `url` key: `{"url": "https://host/path", ...}` (Ferrum default on insert)
pub fn parse_stored_access_url(value: &Value) -> Option<String> {
    if let Some(s) = value.as_str() {
        return Some(s.to_string());
    }
    value
        .as_object()?
        .get("url")
        .and_then(|u| u.as_str())
        .map(String::from)
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn parses_string_and_object_with_url() {
        assert_eq!(
            parse_stored_access_url(&json!("https://a/b")).as_deref(),
            Some("https://a/b")
        );
        assert_eq!(
            parse_stored_access_url(&json!({"url": "https://x/y"})).as_deref(),
            Some("https://x/y")
        );
    }

    #[test]
    fn rejects_object_without_url_or_non_string_url() {
        assert!(parse_stored_access_url(&json!({})).is_none());
        assert!(parse_stored_access_url(&json!({"url": 1})).is_none());
        assert!(parse_stored_access_url(&json!([])).is_none());
    }

    #[test]
    fn listing_mapper_accepts_string_and_object() {
        assert!(matches!(
            jsonb_to_core_access_url_for_listing(&json!("https://a")),
            Some(ferrum_core::AccessUrl::String(s)) if s == "https://a"
        ));
        let m = jsonb_to_core_access_url_for_listing(&json!({"url": "https://b"})).unwrap();
        assert!(matches!(m, ferrum_core::AccessUrl::Object(_)));
    }

    #[test]
    fn listing_mapper_returns_none_for_array_and_null() {
        assert!(jsonb_to_core_access_url_for_listing(&json!([])).is_none());
        assert!(jsonb_to_core_access_url_for_listing(&Value::Null).is_none());
    }

    /// Regression: same JSON object shape as create/ingest must resolve in the `get_access_url` path.
    #[test]
    fn get_access_pipeline_resolves_stored_object_url() {
        let stored = json!({"url": "https://drs.example/ga4gh/drs/v1/objects/o1/access/access-o1"});
        let resolved = parse_stored_access_url(&stored).expect("GET access must accept object url");
        assert_eq!(
            resolved,
            "https://drs.example/ga4gh/drs/v1/objects/o1/access/access-o1"
        );
    }
}
