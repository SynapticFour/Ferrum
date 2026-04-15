use ferrum_mii_connect::{load_manifest, validate_payload, MiiValidationConfig};
use serde_json::Value;
use std::fs;
use std::path::PathBuf;

fn repo_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("..")
        .join("..")
}

#[test]
fn default17_bundle_matches_golden_summary_shape() {
    let root = repo_root();
    let manifest_path = root.join("profiles/mii/manifest.json");
    let sample_path = root.join("profiles/mii/samples/default17-bundle.json");
    let golden_path =
        root.join("crates/ferrum-mii-connect/tests/golden/default17-report.golden.json");

    let (manifest, sha) = load_manifest(&manifest_path).expect("manifest");
    let payload = fs::read_to_string(sample_path).expect("sample payload");
    let report = validate_payload(
        &payload,
        &MiiValidationConfig {
            enabled: true,
            strict_mode: true,
            ..Default::default()
        },
        &manifest,
        &sha,
    )
    .expect("report");

    let mut actual = serde_json::to_value(report).expect("serialize report");
    if let Some(obj) = actual.as_object_mut() {
        obj.remove("generated_at");
    }

    let expected: Value =
        serde_json::from_str(&fs::read_to_string(golden_path).expect("golden json")).expect("json");
    assert_eq!(actual, expected);
}
