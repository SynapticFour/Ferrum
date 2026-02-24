//! A03/A08: workflow_url validation, SSRF URL validation.

#[cfg(test)]
use ferrum_core::{validate_url_ssrf, SsrfPolicy};

#[test]
fn ssrf_rejects_metadata_host() {
    let policy = SsrfPolicy::default();
    assert!(validate_url_ssrf("https://169.254.169.254/latest/meta-data/", &policy).is_err());
    assert!(validate_url_ssrf("https://metadata.google.internal/", &policy).is_err());
}

#[test]
fn ssrf_rejects_http_scheme_when_only_https_allowed() {
    let policy = SsrfPolicy::default();
    assert!(validate_url_ssrf("http://example.com/", &policy).is_err());
}

#[test]
fn ssrf_allows_https() {
    let policy = SsrfPolicy::default();
    assert!(validate_url_ssrf("https://example.com/path", &policy).is_ok());
}

#[test]
fn validate_drs_name_rejects_empty() {
    assert!(ferrum_core::validate_drs_name("").is_err());
}

#[test]
fn validate_drs_name_rejects_control_chars() {
    assert!(ferrum_core::validate_drs_name("a\x00b").is_err());
}

#[test]
fn validate_drs_name_accepts_simple() {
    assert!(ferrum_core::validate_drs_name("sample-name_1").is_ok());
}
