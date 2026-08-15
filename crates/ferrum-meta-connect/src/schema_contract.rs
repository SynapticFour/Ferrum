//! Compile-time bind to vendored [ferrum-meta](https://github.com/SynapticFour/ferrum-meta)
//! LinkML YAML. This crate is still a structural subset (not a LinkML runtime);
//! missing these files must fail `cargo check`, not only a docs note.

/// Vendored `schema/core/ferrum-core.yaml` from ferrum-meta (see `profiles/meta/schema/`).
pub(crate) const CORE_SCHEMA_YAML: &str =
    include_str!("../../../profiles/meta/schema/ferrum-core.yaml");

/// Vendored pathogen profile YAML.
pub(crate) const PATHOGEN_SCHEMA_YAML: &str =
    include_str!("../../../profiles/meta/schema/pathogen-profile.yaml");

/// Vendored H3Africa profile YAML.
pub(crate) const H3AFRICA_SCHEMA_YAML: &str =
    include_str!("../../../profiles/meta/schema/h3africa-profile.yaml");

#[cfg(test)]
mod tests {
    use super::*;
    use crate::FERRUM_META_VERSION;
    use serde_yaml::Value;

    fn parse(yaml: &str) -> Value {
        serde_yaml::from_str(yaml).expect("vendored LinkML YAML must parse")
    }

    fn required_slot_names(class: &Value) -> Vec<String> {
        let Some(usage) = class.get("slot_usage").and_then(Value::as_mapping) else {
            return Vec::new();
        };
        usage
            .iter()
            .filter_map(|(k, v)| {
                if v.get("required").and_then(Value::as_bool) == Some(true) {
                    k.as_str().map(str::to_string)
                } else {
                    None
                }
            })
            .collect()
    }

    fn class<'a>(root: &'a Value, name: &str) -> &'a Value {
        root.get("classes")
            .and_then(Value::as_mapping)
            .and_then(|m| m.get(Value::String(name.into())))
            .unwrap_or_else(|| panic!("LinkML class `{name}` missing from vendored schema"))
    }

    #[test]
    fn vendored_schema_versions_match_crate_const() {
        for (label, yaml) in [
            ("core", CORE_SCHEMA_YAML),
            ("pathogen", PATHOGEN_SCHEMA_YAML),
            ("h3africa", H3AFRICA_SCHEMA_YAML),
        ] {
            let root = parse(yaml);
            let version = root
                .get("version")
                .and_then(Value::as_str)
                .unwrap_or("(missing)");
            assert_eq!(
                version, FERRUM_META_VERSION,
                "{label} schema version {version} != FERRUM_META_VERSION {FERRUM_META_VERSION}"
            );
        }
    }

    #[test]
    fn core_submission_required_collections_match_validator() {
        let root = parse(CORE_SCHEMA_YAML);
        let submission = class(&root, "FerrumCoreSubmission");
        let slots: Vec<&str> = submission
            .get("slots")
            .and_then(Value::as_sequence)
            .expect("FerrumCoreSubmission.slots")
            .iter()
            .filter_map(Value::as_str)
            .collect();
        for name in [
            "ferrum_meta_version",
            "studies",
            "individuals",
            "samples",
            "experiments",
            "files",
            "datasets",
        ] {
            assert!(
                slots.contains(&name),
                "FerrumCoreSubmission.slots missing `{name}` (validator requires it)"
            );
        }
        let required = required_slot_names(submission);
        for name in [
            "ferrum_meta_version",
            "studies",
            "individuals",
            "samples",
            "experiments",
            "files",
            "datasets",
        ] {
            assert!(
                required.iter().any(|s| s == name),
                "FerrumCoreSubmission.slot_usage.{name} is not required in LinkML"
            );
        }
    }

    #[test]
    fn core_entity_required_fields_match_validator() {
        let root = parse(CORE_SCHEMA_YAML);
        let study_req = required_slot_names(class(&root, "Study"));
        for name in ["title", "description", "type"] {
            assert!(
                study_req.iter().any(|s| s == name),
                "Study.{name} is not required in LinkML but validate_core_submission checks it"
            );
        }
        let sample_req = required_slot_names(class(&root, "Sample"));
        assert!(
            sample_req.iter().any(|s| s == "individual_alias"),
            "Sample.individual_alias must stay required"
        );
        let exp_req = required_slot_names(class(&root, "Experiment"));
        assert!(
            exp_req.iter().any(|s| s == "sample_alias"),
            "Experiment.sample_alias must stay required"
        );
        let ds_req = required_slot_names(class(&root, "Dataset"));
        for name in ["title", "file_aliases"] {
            assert!(
                ds_req.iter().any(|s| s == name),
                "Dataset.{name} is not required in LinkML but the validator checks it"
            );
        }
    }

    #[test]
    fn pathogen_linkml_uses_collection_country_not_site() {
        let root = parse(PATHOGEN_SCHEMA_YAML);
        let sample = class(&root, "PathogenSample");
        let required = required_slot_names(sample);
        assert!(
            required.iter().any(|s| s == "collection_country"),
            "PathogenSample.collection_country drifted in LinkML"
        );
        assert!(
            required.iter().any(|s| s == "collection_date"),
            "PathogenSample.collection_date drifted in LinkML"
        );
        assert!(
            !required.iter().any(|s| s == "collection_site"),
            "LinkML must not grow a collection_site slot without updating the validator"
        );
    }

    #[test]
    fn h3africa_linkml_requires_consent_records() {
        let root = parse(H3AFRICA_SCHEMA_YAML);
        let consent = class(&root, "ConsentRecord");
        let required = required_slot_names(consent);
        assert!(
            required.iter().any(|s| s == "consent_type"),
            "ConsentRecord.consent_type drifted in LinkML"
        );
    }
}
