// SPDX-License-Identifier: BUSL-1.1
//! WES reference mismatch warnings for African population data.

use crate::registry::ReferenceRegistry;
use crate::types::{PopulationScope, WesReferenceWarning};
use ferrum_core::{FerrumPool, Result};

pub fn extract_drs_ids_from_params(params: &serde_json::Value) -> Vec<String> {
    let mut ids = Vec::new();
    collect_drs_ids(params, &mut ids);
    ids.sort();
    ids.dedup();
    ids
}

fn collect_drs_ids(value: &serde_json::Value, out: &mut Vec<String>) {
    match value {
        serde_json::Value::String(s) => {
            if let Some(id) = s.strip_prefix("drs://") {
                let id = id.split('/').next().unwrap_or(id).trim();
                if !id.is_empty() {
                    out.push(id.to_string());
                }
            }
        }
        serde_json::Value::Array(arr) => {
            for v in arr {
                collect_drs_ids(v, out);
            }
        }
        serde_json::Value::Object(map) => {
            for v in map.values() {
                collect_drs_ids(v, out);
            }
        }
        _ => {}
    }
}

pub fn params_suggest_african_origin(params: &serde_json::Value) -> bool {
    if let Some(scope) = params.get("population_scope").and_then(|v| v.as_str()) {
        if scope.eq_ignore_ascii_case("AfricanPangenome") {
            return true;
        }
    }
    if let Some(origin) = params.get("geo_origin").and_then(|v| v.as_str()) {
        if origin.to_ascii_lowercase().contains("africa") {
            return true;
        }
    }
    false
}

pub async fn drs_object_suggests_african_origin(pool: &FerrumPool, drs_id: &str) -> Result<bool> {
    let sql = "SELECT name, description FROM drs_objects WHERE id = $1";
    let row: Option<(Option<String>, Option<String>)> = match pool {
        FerrumPool::Postgres(p) => sqlx::query_as(sql).bind(drs_id).fetch_optional(p).await?,
        FerrumPool::Sqlite(p) => sqlx::query_as(sql).bind(drs_id).fetch_optional(p).await?,
    };
    if let Some((name, description)) = row {
        if text_suggests_african_origin(name.as_deref()) {
            return Ok(true);
        }
        if text_suggests_african_origin(description.as_deref()) {
            return Ok(true);
        }
    }

    let org_sql = "SELECT organism FROM pathogen_annotations WHERE drs_object_id = $1 LIMIT 1";
    let organism: Option<String> = match pool {
        FerrumPool::Postgres(p) => {
            sqlx::query_scalar(org_sql)
                .bind(drs_id)
                .fetch_optional(p)
                .await?
        }
        FerrumPool::Sqlite(p) => {
            sqlx::query_scalar(org_sql)
                .bind(drs_id)
                .fetch_optional(p)
                .await?
        }
    };
    if organism.as_deref() == Some("Homo_sapiens") {
        return Ok(true);
    }
    Ok(false)
}

fn text_suggests_african_origin(text: Option<&str>) -> bool {
    let Some(t) = text else {
        return false;
    };
    let lower = t.to_ascii_lowercase();
    lower.contains("africa")
        || lower.contains("h3africa")
        || lower.contains("awi-gen")
        || lower.contains("awi_gen")
}

pub async fn check_reference_mismatch(
    registry: &ReferenceRegistry,
    reference_genome: Option<&str>,
    workflow_params: &serde_json::Value,
) -> Result<Option<WesReferenceWarning>> {
    let ref_id = match reference_genome.filter(|s| !s.is_empty()) {
        Some(id) => id.to_string(),
        None => registry
            .default_reference()
            .await?
            .map(|r| r.id)
            .unwrap_or_else(|| "GRCh38".into()),
    };

    let reference = registry.get(&ref_id).await?;
    let scope = reference
        .as_ref()
        .map(|r| r.population_scope.clone())
        .unwrap_or(PopulationScope::Global);
    if !scope.is_global() {
        return Ok(None);
    }

    let mut african = params_suggest_african_origin(workflow_params);
    if !african {
        for drs_id in extract_drs_ids_from_params(workflow_params) {
            if drs_object_suggests_african_origin(registry.pool(), &drs_id).await? {
                african = true;
                break;
            }
        }
    }
    if !african {
        return Ok(None);
    }

    let alternatives = registry.african_pangenome_alternatives().await?;
    Ok(Some(WesReferenceWarning {
        code: "REFERENCE_MISMATCH".into(),
        message: "Input data may have African population origin. Consider using H3Africa_v1 or AWI-GEN_panel for improved variant calling accuracy.".into(),
        reference_used: ref_id,
        suggested_alternatives: alternatives,
    }))
}
