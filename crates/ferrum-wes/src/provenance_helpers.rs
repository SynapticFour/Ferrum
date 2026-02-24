//! Helpers for extracting DRS URIs from workflow params for provenance.

use serde_json::Value;

/// Extract object IDs from drs:// URIs in a JSON value (e.g. workflow_params).
/// Returns the last path segment of each drs:// URI as the object_id.
pub fn extract_drs_object_ids_from_json(v: &Value) -> Vec<String> {
    let mut ids = Vec::new();
    collect_drs_ids(v, &mut ids);
    ids
}

fn collect_drs_ids(v: &Value, out: &mut Vec<String>) {
    match v {
        Value::String(s) => {
            let s = s.trim();
            if s.starts_with("drs://") {
                if let Some(rest) = s.strip_prefix("drs://") {
                    if let Some((_host, path)) = rest.split_once('/') {
                        let object_id = path.split('/').next_back().unwrap_or(path).to_string();
                        if !object_id.is_empty() && !out.contains(&object_id) {
                            out.push(object_id);
                        }
                    }
                }
            }
        }
        Value::Array(arr) => {
            for item in arr {
                collect_drs_ids(item, out);
            }
        }
        Value::Object(map) => {
            for (_, val) in map {
                collect_drs_ids(val, out);
            }
        }
        _ => {}
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_extract() {
        let j = serde_json::json!({
            "input_bam": "drs://drs.example.org/01HXYZ123",
            "nested": { "ref": "drs://other/ga4gh/drs/v1/objects/01HABC" }
        });
        let ids = extract_drs_object_ids_from_json(&j);
        assert!(ids.contains(&"01HXYZ123".to_string()));
        assert!(ids.contains(&"01HABC".to_string()));
    }
}
