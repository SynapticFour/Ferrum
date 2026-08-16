// SPDX-License-Identifier: BUSL-1.1
//! RO-Crate enrichment helpers (ONT metrics, pathogen annotations, reference genome).

use serde_json::{json, Value};

#[derive(Debug, Clone, Default)]
pub struct DrsCrateExtensions {
    pub ont_metrics: Option<Value>,
    pub gisaid_metadata: Option<Value>,
    pub pathogen_annotation: Option<PathogenAnnotationSummary>,
}

#[derive(Debug, Clone, serde::Serialize)]
pub struct PathogenAnnotationSummary {
    pub organism: String,
    pub amr_genes: Option<Value>,
    pub serotype: Option<String>,
    pub ont_qscore_min: Option<f32>,
}

pub fn reference_genome_from_engine_params(engine_params: &Value) -> Option<String> {
    engine_params
        .get("reference_genome")
        .and_then(|v| v.as_str())
        .filter(|s| !s.is_empty())
        .map(str::to_string)
}

pub fn enrich_file_node(base: Value, ext: &DrsCrateExtensions) -> Value {
    let mut node = base;
    if let Some(ref metrics) = ext.ont_metrics {
        node["ont_metrics"] = metrics.clone();
    }
    if let Some(ref gm) = ext.gisaid_metadata {
        node["gisaid_metadata"] = gm.clone();
    }
    if let Some(ref pa) = ext.pathogen_annotation {
        node["pathogen_annotations"] = json!(pa);
    }
    node
}

pub fn reference_genome_entity(reference_genome: &str) -> Value {
    json!({
        "@type": "ReferenceGenome",
        "@id": format!("#reference-genome-{}", reference_genome),
        "identifier": reference_genome,
        "name": reference_genome
    })
}

type PathogenRow = (String, Option<Value>, Option<String>, Option<f32>);

pub async fn load_drs_extensions(
    pool: &sqlx::PgPool,
    object_id: &str,
) -> Result<DrsCrateExtensions, sqlx::Error> {
    let row: Option<(Option<serde_json::Value>, Option<serde_json::Value>)> =
        sqlx::query_as("SELECT ont_metrics, gisaid_metadata FROM drs_objects WHERE id = $1")
            .bind(object_id)
            .fetch_optional(pool)
            .await?;

    let pathogen: Option<PathogenRow> = sqlx::query_as(
        "SELECT organism, amr_genes, serotype, ont_qscore_min
             FROM pathogen_annotations WHERE drs_object_id = $1 LIMIT 1",
    )
    .bind(object_id)
    .fetch_optional(pool)
    .await?;

    Ok(DrsCrateExtensions {
        ont_metrics: row.as_ref().and_then(|r| r.0.clone()),
        gisaid_metadata: row.as_ref().and_then(|r| r.1.clone()),
        pathogen_annotation: pathogen.map(|(organism, amr_genes, serotype, ont_qscore_min)| {
            PathogenAnnotationSummary {
                organism,
                amr_genes,
                serotype,
                ont_qscore_min,
            }
        }),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_ro_crate_ont_fields() {
        let ext = DrsCrateExtensions {
            ont_metrics: Some(json!({"mean_qscore": 12.5, "read_count": 100})),
            pathogen_annotation: Some(PathogenAnnotationSummary {
                organism: "Plasmodium_falciparum".into(),
                amr_genes: None,
                serotype: None,
                ont_qscore_min: Some(10.0),
            }),
            ..Default::default()
        };
        let node = enrich_file_node(
            json!({"@type": "File", "@id": "drs://ferrum/obj-1", "identifier": "obj-1"}),
            &ext,
        );
        assert!(node.get("ont_metrics").is_some());
        assert!(node.get("pathogen_annotations").is_some());
        let rg = reference_genome_from_engine_params(&json!({"reference_genome": "H3Africa_v1"}));
        assert_eq!(rg.as_deref(), Some("H3Africa_v1"));
        let entity = reference_genome_entity("H3Africa_v1");
        assert_eq!(entity["identifier"], "H3Africa_v1");
    }
}
