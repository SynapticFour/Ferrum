//! Regression: `GET /ga4gh/drs/v1/objects/{id}/access/{access_id}` reads `drs_access_methods.access_url`
//! from PostgreSQL as JSONB. Ingest/create often store `{"url": "https://..."}`; the handler must
//! resolve the same string [`ferrum_drs::access_url::parse_stored_access_url`] uses (before optional S3 presign).

use ferrum_drs::access_url::parse_stored_access_url;
use serde_json::json;

#[test]
fn stored_json_object_access_url_resolves_like_repo_get_access_url() {
    let stored = json!({"url": "https://gateway/ga4gh/drs/v1/objects/01HZ/acc-01"});
    let url = parse_stored_access_url(&stored).expect("object-with-url must resolve for GET access");
    assert!(url.ends_with("/acc-01"), "unexpected url: {url}");
}
