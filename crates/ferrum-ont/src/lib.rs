// SPDX-License-Identifier: BUSL-1.1
//! Oxford Nanopore (ONT) ingestion types and canonical DRS mapping.
//!
//! Ferrum does not run basecalling (Dorado/Guppy are external). This crate accepts
//! raw POD5/FAST5/BLOW5 archives or pre-basecalled FASTQ and stores them as DRS objects.

pub mod error;
pub mod ingest;
pub mod types;

pub use error::{OntError, Result};
pub use ingest::{
    build_create_request, mime_for_format, synthetic_pod5_bytes, validate_ingest_request,
    OntCreateFields,
};
pub use types::{OntFormat, OntIngestRequest, OntQualityMetrics};
