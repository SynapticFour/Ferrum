//! Reference genome types.

use serde::{Deserialize, Serialize};
use url::Url;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum PopulationScope {
    Global,
    AfricanPangenome,
    Pathogen { taxon_id: u32 },
}

impl PopulationScope {
    pub fn to_db_string(&self) -> String {
        match self {
            PopulationScope::Global => "Global".into(),
            PopulationScope::AfricanPangenome => "AfricanPangenome".into(),
            PopulationScope::Pathogen { taxon_id } => format!("Pathogen:{taxon_id}"),
        }
    }

    pub fn from_db_string(s: &str) -> Option<Self> {
        if s == "Global" {
            return Some(PopulationScope::Global);
        }
        if s == "AfricanPangenome" {
            return Some(PopulationScope::AfricanPangenome);
        }
        if let Some(rest) = s.strip_prefix("Pathogen:") {
            let taxon_id = rest.parse().ok()?;
            return Some(PopulationScope::Pathogen { taxon_id });
        }
        None
    }

    pub fn is_global(&self) -> bool {
        matches!(self, PopulationScope::Global)
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ReferenceGenome {
    pub id: String,
    pub display_name: String,
    pub organism: String,
    pub population_scope: PopulationScope,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub source_url: Option<Url>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub fasta_drs_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub index_drs_id: Option<String>,
    #[serde(default)]
    pub is_default: bool,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct RegisterReferenceRequest {
    pub id: String,
    pub display_name: String,
    pub organism: String,
    pub population_scope: PopulationScope,
    #[serde(default)]
    pub source_url: Option<Url>,
    #[serde(default)]
    pub is_default: bool,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct LoadReferenceRequest {
    pub fasta_drs_id: String,
    #[serde(default)]
    pub index_drs_id: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
pub struct WesReferenceWarning {
    pub code: String,
    pub message: String,
    pub reference_used: String,
    pub suggested_alternatives: Vec<String>,
}
