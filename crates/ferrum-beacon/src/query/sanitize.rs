use crate::error::BeaconError;

/// Sanitized / normalized query params for Beacon `variant_exists`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SanitizedVariantQuery {
    pub assembly_id: Option<String>,
    pub reference_name: String, // normalized, e.g. chr1 / chrX / chrM
    pub start: i64,
    pub end: i64,
}

const MAX_GENOMIC_COORD: i64 = 3_100_000_000; // ~3.1x10^9 as a realistic bound

const ASSEMBLY_WHITELIST: &[&str] = &[
    "GRCh37",
    "GRCh38",
    "hg19",
    "hg38",
    "T2T-CHM13v2.0",
];

/// Reject obvious injection vectors before any DB interaction.
fn reject_for_injection(s: &str) -> Result<(), BeaconError> {
    // Learned from EGA beacon2-pi-api hardening: never pass raw strings
    // containing control characters into query-builder paths.
    if s.contains('$') || s.contains('{') || s.contains('}') || s.contains(';') {
        return Err(BeaconError::Validation(format!(
            "invalid characters in input; forbidden one of [$ {{ }} ;]"
        )));
    }
    Ok(())
}

pub fn sanitize_bases(bases: Option<&str>) -> Result<Option<String>, BeaconError> {
    // For Beacon v2, bases are modelled as strings (e.g. "A", "T", "ACGT").
    // Learned from production hardening: reject obvious query-injection characters
    // before any DB interaction. We intentionally do *not* over-restrict allowed
    // alphabet here to avoid rejecting legitimate IUPAC/extended base encodings.
    let Some(raw) = bases.map(|s| s.trim()).filter(|s| !s.is_empty()) else {
        return Ok(None);
    };
    reject_for_injection(raw)?;
    Ok(Some(raw.to_string()))
}

pub fn sanitize_filter_id(filter_id: &str) -> Result<String, BeaconError> {
    // Learned from EGA production hardening: never allow control characters / injection
    // patterns in any query-builder path, including Beacon `filters[].id`.
    let raw = filter_id.trim();
    if raw.is_empty() {
        return Err(BeaconError::Validation("filter id must not be empty".into()));
    }
    reject_for_injection(raw)?;
    Ok(raw.to_string())
}

pub fn sanitize_reference_name(reference_name: Option<&str>) -> Result<String, BeaconError> {
    let raw = reference_name.unwrap_or("1").trim();
    if raw.is_empty() {
        return Err(BeaconError::Validation("reference_name must not be empty".into()));
    }
    reject_for_injection(raw)?;

    // Normalize forms to `chr<id>` where id is one of:
    // 1..22, X, Y, M
    let (chr_prefix, id) = match raw {
        // already-prefixed
        r if r.starts_with("chr") => {
            let tail = &r[3..];
            (true, tail)
        }
        _ => (false, raw),
    };

    let normalized_id = match id {
        "1" | "2" | "3" | "4" | "5" | "6" | "7" | "8" | "9" | "10" | "11" | "12" | "13" | "14"
        | "15" | "16" | "17" | "18" | "19" | "20" | "21" | "22" => id.to_string(),
        "X" | "x" => "X".to_string(),
        "Y" | "y" => "Y".to_string(),
        "M" | "m" | "MT" | "Mt" | "mt" | "mT" => "M".to_string(),
        other => {
            // Allow already-prefixed `chrX/chrY/chrM`.
            // Otherwise reject.
            if chr_prefix {
                return Err(BeaconError::Validation(format!(
                    "invalid reference_name: {other}"
                )));
            }
            return Err(BeaconError::Validation(format!(
                "invalid reference_name: {other}"
            )));
        }
    };

    Ok(format!("chr{}", normalized_id))
}

pub fn sanitize_assembly_id(assembly_id: Option<&str>) -> Result<Option<String>, BeaconError> {
    let Some(raw) = assembly_id.map(|s| s.trim()).filter(|s| !s.is_empty()) else {
        return Ok(None);
    };
    reject_for_injection(raw)?;
    if ASSEMBLY_WHITELIST
        .iter()
        .any(|w| w.eq_ignore_ascii_case(raw))
    {
        // Preserve canonical casing from whitelist.
        let canonical = ASSEMBLY_WHITELIST
            .iter()
            .find(|w| w.eq_ignore_ascii_case(raw))
            .map(|s| s.to_string())
            .expect("whitelisted");
        return Ok(Some(canonical));
    }
    Err(BeaconError::Validation(format!(
        "invalid assembly_id: {}",
        raw
    )))
}

pub fn sanitize_range(start: Option<i64>, end: Option<i64>) -> Result<(i64, i64), BeaconError> {
    let s = start.unwrap_or(0);
    let e = end.unwrap_or(999_999_999);

    // Range bounds (GA4GH Beacon v2 query semantics).
    if s < 0 || e < 0 || s > MAX_GENOMIC_COORD || e > MAX_GENOMIC_COORD {
        return Err(BeaconError::Validation(
            "start/end out of realistic genomic bounds".into(),
        ));
    }
    if s > e {
        return Err(BeaconError::Validation(
            "start must be <= end".into(),
        ));
    }
    Ok((s, e))
}

pub fn sanitize_query_params(
    assembly_id: Option<&str>,
    reference_name: Option<&str>,
    start: Option<i64>,
    end: Option<i64>,
) -> Result<SanitizedVariantQuery, BeaconError> {
    let assembly_id = sanitize_assembly_id(assembly_id)?;
    let reference_name = sanitize_reference_name(reference_name)?;
    let (start, end) = sanitize_range(start, end)?;
    Ok(SanitizedVariantQuery {
        assembly_id,
        reference_name,
        start,
        end,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_reference_name_normalization_numeric() {
        assert_eq!(
            sanitize_reference_name(Some("1")).unwrap(),
            "chr1".to_string()
        );
        assert_eq!(
            sanitize_reference_name(Some("chr2")).unwrap(),
            "chr2".to_string()
        );
    }

    #[test]
    fn test_reference_name_normalization_x_y_m() {
        assert_eq!(sanitize_reference_name(Some("X")).unwrap(), "chrX");
        assert_eq!(sanitize_reference_name(Some("chrY")).unwrap(), "chrY");
        assert_eq!(sanitize_reference_name(Some("MT")).unwrap(), "chrM");
        assert_eq!(sanitize_reference_name(Some("m")).unwrap(), "chrM");
    }

    #[test]
    fn test_invalid_reference_name_rejected() {
        let err = sanitize_reference_name(Some("chrZ")).unwrap_err();
        match err {
            BeaconError::Validation(_) => {}
            _ => panic!("expected validation error"),
        }
    }

    #[test]
    fn test_assembly_id_whitelist() {
        assert!(sanitize_assembly_id(Some("GRCh38")).unwrap().is_some());
        assert!(sanitize_assembly_id(Some("grch37")).unwrap().is_some());
        assert!(sanitize_assembly_id(Some("unknown")).is_err());
    }

    #[test]
    fn test_start_end_bounds() {
        assert!(sanitize_range(Some(10), Some(20)).is_ok());
        assert!(sanitize_range(Some(20), Some(10)).is_err());
    }

    #[test]
    fn test_bases_injection_rejected() {
        assert!(sanitize_bases(Some("$")).is_err());
        assert!(sanitize_bases(Some("{")).is_err());
        assert!(sanitize_bases(Some("A")).unwrap().as_deref() == Some("A"));
        assert!(sanitize_bases(None).unwrap().is_none());
    }

    #[test]
    fn test_filter_id_injection_rejected() {
        assert!(sanitize_filter_id("$").is_err());
        assert!(sanitize_filter_id("HP:{bad}").is_err());
        assert_eq!(sanitize_filter_id("SNV").unwrap(), "SNV");
    }
}

