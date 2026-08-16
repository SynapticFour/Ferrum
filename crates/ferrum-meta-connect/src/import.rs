// SPDX-License-Identifier: BUSL-1.1
//! CSV import for paper → digital metadata (Phase 2.7).

use crate::init::InitParams;
use crate::profiles::{validate_submission, MetaProfile};
use crate::MetaValidationReport;
use serde_json::Value;
use std::io::BufRead;

/// Expected CSV header (case-insensitive):
/// study_title,sample_alias,individual_alias,collection_date,collection_site,country,consent_type,pathogen_organism,data_use_conditions
pub fn import_csv_to_submission(
    profile: MetaProfile,
    reader: impl BufRead,
) -> Result<(Value, MetaValidationReport), String> {
    let mut lines = reader.lines();
    let header = lines
        .next()
        .ok_or("CSV is empty")?
        .map_err(|e| e.to_string())?;
    let cols: Vec<String> = header
        .split(',')
        .map(|s| s.trim().to_ascii_lowercase())
        .collect();
    let row = lines
        .next()
        .ok_or("CSV needs a data row after header")?
        .map_err(|e| e.to_string())?;
    let values: Vec<String> = parse_csv_row(&row);
    let get = |name: &str| -> Option<String> {
        cols.iter()
            .position(|c| c == name)
            .and_then(|i| values.get(i))
            .map(|s| s.trim().to_string())
            .filter(|s| !s.is_empty())
    };

    let duo_raw = get("data_use_conditions").unwrap_or_default();
    let duo: Vec<String> = if duo_raw.is_empty() {
        vec![]
    } else {
        duo_raw.split(';').map(|s| s.trim().to_string()).collect()
    };

    let params = InitParams {
        study_title: get("study_title"),
        study_alias: get("study_alias"),
        sample_alias: get("sample_alias"),
        individual_alias: get("individual_alias"),
        collection_site: get("collection_site"),
        collection_date: get("collection_date"),
        country: get("country"),
        consent_type: get("consent_type"),
        pathogen_organism: get("pathogen_organism"),
        data_use_conditions: duo,
    };

    let doc = crate::init::build_init_template(profile, &params);
    let report = validate_submission(&doc, Some(profile));
    if !report.valid {
        return Err(format!(
            "imported CSV produced invalid submission ({} errors)",
            report.error_count()
        ));
    }
    Ok((doc, report))
}

fn parse_csv_row(row: &str) -> Vec<String> {
    let mut out = Vec::new();
    let mut cur = String::new();
    let mut in_quotes = false;
    for ch in row.chars() {
        match ch {
            '"' => in_quotes = !in_quotes,
            ',' if !in_quotes => {
                out.push(cur.trim().to_string());
                cur.clear();
            }
            _ => cur.push(ch),
        }
    }
    out.push(cur.trim().to_string());
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Cursor;

    #[test]
    fn csv_import_pathogen() {
        let csv = Cursor::new(
            "study_title,sample_alias,collection_date,collection_site,data_use_conditions\n\
             Outbreak pilot,s001,2026-06-01,Monrovia,DUO:0000007\n",
        );
        let (doc, report) = import_csv_to_submission(MetaProfile::Pathogen, csv).expect("import");
        assert!(report.valid);
        assert_eq!(
            doc.get("samples")
                .and_then(|v| v.as_array())
                .and_then(|a| a.first())
                .and_then(|s| s.get("collection_site"))
                .and_then(|v| v.as_str()),
            Some("Monrovia")
        );
    }
}
