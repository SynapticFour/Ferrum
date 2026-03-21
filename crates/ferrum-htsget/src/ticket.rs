//! Ticket building: DRS object classification, URL to `GET .../drs/v1/objects/{id}/stream`.

use percent_encoding::{utf8_percent_encode, NON_ALPHANUMERIC};

/// What kind of genomics file the DRS object represents (from mime + name).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FileKind {
    ReadsBam,
    ReadsCram,
    VariantsVcf,
    VariantsBcf,
    Other,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EndpointKind {
    Reads,
    Variants,
}

pub fn classify_object(mime_type: Option<&str>, name: Option<&str>) -> FileKind {
    let mime = mime_type
        .map(|s| s.to_ascii_lowercase())
        .unwrap_or_default();
    let name_l = name.map(|s| s.to_ascii_lowercase()).unwrap_or_default();

    if mime.contains("cram") || name_l.ends_with(".cram") {
        return FileKind::ReadsCram;
    }
    if mime.contains("bam") || mime.contains("vnd.ga4gh.bam") || name_l.ends_with(".bam") {
        return FileKind::ReadsBam;
    }
    if mime.contains("bcf") || name_l.ends_with(".bcf") {
        return FileKind::VariantsBcf;
    }
    if mime.contains("vcf") || name_l.ends_with(".vcf") || name_l.ends_with(".vcf.gz") {
        return FileKind::VariantsVcf;
    }
    FileKind::Other
}

pub fn endpoint_matches_file(endpoint: EndpointKind, kind: FileKind) -> bool {
    match endpoint {
        EndpointKind::Reads => matches!(kind, FileKind::ReadsBam | FileKind::ReadsCram),
        EndpointKind::Variants => matches!(kind, FileKind::VariantsVcf | FileKind::VariantsBcf),
    }
}

/// Default response format for endpoint + stored file.
pub fn default_format_for(endpoint: EndpointKind, kind: FileKind) -> &'static str {
    match endpoint {
        EndpointKind::Reads => match kind {
            FileKind::ReadsCram => "CRAM",
            _ => "BAM",
        },
        EndpointKind::Variants => match kind {
            FileKind::VariantsBcf => "BCF",
            _ => "VCF",
        },
    }
}

/// Normalize requested format to uppercase BAM/CRAM/VCF/BCF or None if missing.
pub fn normalize_format(s: Option<&str>) -> Option<String> {
    s.map(|x| x.trim().to_ascii_uppercase())
        .filter(|x| !x.is_empty())
}

/// Returns true if the requested format is compatible with the stored file kind.
pub fn format_matches_file(requested: &str, kind: FileKind) -> bool {
    match kind {
        FileKind::ReadsBam => requested == "BAM",
        FileKind::ReadsCram => requested == "CRAM",
        FileKind::VariantsVcf => requested == "VCF",
        FileKind::VariantsBcf => requested == "BCF",
        FileKind::Other => false,
    }
}

/// HTTPS URL to DRS stream (object id percent-encoded for path safety).
pub fn drs_stream_url(public_base: &str, object_id: &str) -> String {
    let base = public_base.trim_end_matches('/');
    let enc = utf8_percent_encode(object_id, NON_ALPHANUMERIC);
    format!("{base}/ga4gh/drs/v1/objects/{enc}/stream")
}
