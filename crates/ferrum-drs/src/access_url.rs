//! Normalization of `drs_access_methods.access_url` JSONB for resolving download URLs.
//!
//! Write paths (create/ingest) store either a JSON string or a GA4GH-style object
//! `{"url": "https://..."}`. Read paths that return plain `AccessUrl` JSON must accept both so
//! `GET .../access/{access_id}` works for all clients.

use serde_json::Value;

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
}
