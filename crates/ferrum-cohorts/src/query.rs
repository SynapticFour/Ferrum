//! Cohort query engine: filter samples by phenotype and return facets.

use crate::error::Result;
use crate::types::{QueryFacets, QueryResult};
use serde::{Deserialize, Serialize};
use sqlx::PgPool;
use std::collections::HashMap;
use utoipa::ToSchema;

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct CohortQuery {
    #[serde(default)]
    pub filters: Vec<Filter>,
    #[serde(default)]
    pub logic: QueryLogic,
}

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
#[serde(tag = "type")]
pub enum Filter {
    Phenotype {
        field: String,
        operator: String,
        value: serde_json::Value,
    },
    SampleIdIn(Vec<String>),
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "UPPERCASE")]
pub enum QueryLogic {
    #[default]
    And,
    Or,
}

impl CohortQuery {
    /// Execute query against a cohort; returns matched sample IDs and facets.
    pub async fn execute(&self, cohort_id: &str, pool: &PgPool) -> Result<QueryResult> {
        let rows = sqlx::query_as::<_, (String, serde_json::Value)>(
            "SELECT sample_id, phenotype FROM cohort_samples WHERE cohort_id = $1",
        )
        .bind(cohort_id)
        .fetch_all(pool)
        .await?;
        let mut matched = Vec::new();
        let mut by_sex: HashMap<String, usize> = HashMap::new();
        let mut by_diagnosis: HashMap<String, usize> = HashMap::new();
        let mut by_sequencing_type: HashMap<String, usize> = HashMap::new();
        let by_data_type: HashMap<String, usize> = HashMap::new();
        for (sample_id, phenotype) in rows {
            if self.matches(&phenotype, &sample_id) {
                matched.push(sample_id.clone());
                if let Some(obj) = phenotype.as_object() {
                    if let Some(v) = obj.get("sex").and_then(|v| v.as_str()) {
                        *by_sex.entry(v.to_string()).or_insert(0) += 1;
                    }
                    if let Some(v) = obj.get("diagnosis").and_then(|v| v.as_str()) {
                        *by_diagnosis.entry(v.to_string()).or_insert(0) += 1;
                    }
                    if let Some(v) = obj.get("sequencing_type").and_then(|v| v.as_str()) {
                        *by_sequencing_type.entry(v.to_string()).or_insert(0) += 1;
                    }
                }
            }
        }
        Ok(QueryResult {
            total_count: matched.len(),
            matched_sample_ids: matched,
            facets: QueryFacets {
                by_sex,
                by_diagnosis,
                by_sequencing_type,
                by_data_type,
            },
        })
    }

    fn matches(&self, phenotype: &serde_json::Value, sample_id: &str) -> bool {
        if self.filters.is_empty() {
            return true;
        }
        let outcomes: Vec<bool> = self
            .filters
            .iter()
            .map(|f| self.filter_matches(f, phenotype, sample_id))
            .collect();
        match self.logic {
            QueryLogic::And => outcomes.iter().all(|&x| x),
            QueryLogic::Or => outcomes.iter().any(|&x| x),
        }
    }

    fn filter_matches(&self, filter: &Filter, phenotype: &serde_json::Value, sample_id: &str) -> bool {
        match filter {
            Filter::SampleIdIn(ids) => ids.contains(&sample_id.to_string()),
            Filter::Phenotype { field, operator, value } => {
                let v = phenotype.get(field);
                match operator.as_str() {
                    "eq" | "=" => v == Some(value),
                    "ne" | "!=" => v != Some(value),
                    "in" => value
                        .as_array()
                        .map(|a| a.iter().any(|x| v == Some(x)))
                        .unwrap_or(false),
                    "contains" => value
                        .as_str()
                        .map(|s| v.and_then(|x| x.as_str()).map(|x| x.contains(s)).unwrap_or(false))
                        .unwrap_or(false),
                    _ => v == Some(value),
                }
            }
        }
    }
}
