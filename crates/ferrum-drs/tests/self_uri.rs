// SPDX-License-Identifier: BUSL-1.1
use ferrum_drs::types::DrsObject;

#[test]
fn test_self_uri_always_drs_scheme() {
    let obj = DrsObject {
        id: "object-123".to_string(),
        self_uri: "https://example.org/ga4gh/drs/v1/objects/object-123".to_string(),
        size: 0,
        created_time: "2026-01-01T00:00:00Z".to_string(),
        checksums: vec![],
        name: None,
        updated_time: None,
        version: None,
        mime_type: None,
        access_methods: None,
        contents: None,
        description: None,
        aliases: None,
        ont_metrics: None,
        gisaid_metadata: None,
        metadata_ref: None,
        storage_backend: None,
        is_encrypted: None,
        workspace_id: None,
        checksum_status: None,
    };

    let canonical = obj.canonical_self_uri("drs.example.test");
    assert_eq!(canonical, "drs://drs.example.test/object-123");
    assert!(!canonical.starts_with("http://"));
    assert!(!canonical.starts_with("https://"));
}

#[test]
fn official_drs_object_json_omits_null_optionals() {
    let obj = DrsObject {
        id: "test-object-1".to_string(),
        self_uri: "drs://localhost/test-object-1".to_string(),
        size: 12,
        created_time: "2026-08-16T15:11:26.878Z".to_string(),
        checksums: vec![ferrum_core::Checksum {
            r#type: "sha256".into(),
            checksum: "abc".into(),
        }],
        name: Some("HelixTest object".into()),
        updated_time: None,
        version: None,
        mime_type: None,
        access_methods: Some(vec![ferrum_core::AccessMethod {
            access_type: ferrum_core::AccessType::Https,
            access_url: Some(ferrum_core::AccessUrl::String(
                "http://localhost:8080/ga4gh/drs/v1/objects/test-object-1/stream".into(),
            )),
            access_id: Some("default".into()),
            region: None,
        }]),
        contents: None,
        description: None,
        aliases: None,
        ont_metrics: None,
        gisaid_metadata: None,
        metadata_ref: None,
        storage_backend: None,
        is_encrypted: None,
        workspace_id: None,
        checksum_status: None,
    };
    let v = serde_json::to_value(&obj).expect("serialize");
    for key in [
        "version",
        "contents",
        "description",
        "mime_type",
        "aliases",
        "updated_time",
    ] {
        assert!(v.get(key).is_none(), "{key} must be omitted, not null");
    }
    let region = v["access_methods"][0].get("region");
    assert!(
        region.is_none(),
        "AccessMethod.region must be omitted, not null"
    );
    let created = v["created_time"].as_str().expect("created_time");
    assert!(
        created.contains('T') && created.ends_with('Z'),
        "created_time must be RFC3339 with Z: {created}"
    );
}
