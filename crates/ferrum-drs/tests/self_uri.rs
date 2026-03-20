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
    };

    let canonical = obj.canonical_self_uri("drs.example.test");
    assert_eq!(canonical, "drs://drs.example.test/object-123");
    assert!(!canonical.starts_with("http://"));
    assert!(!canonical.starts_with("https://"));
}

