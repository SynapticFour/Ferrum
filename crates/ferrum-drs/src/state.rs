//! App state for DRS (repo, optional storage for ingest, optional S3 presigner, optional provenance).
//!
//! **Crypt4GH integration:** When `storage_references.is_encrypted = true`, bytes are stored in
//! Crypt4GH form. `GET /objects/{id}/stream` can decrypt server-side and stream **plaintext** when
//! `crypt4gh_decrypt_stream` is enabled and a node key is configured. Optional: wrap stream routes
//! with `Crypt4GHLayer` in the gateway to re-encrypt for the client's public key.

use crate::presign::S3Presigner;
use crate::repo::DrsRepo;
use ferrum_core::{
    BackgroundWorkGate, IngestConfig, OutbreakService, PipelineConfig, ProvenanceStore,
    ResidencyAuditLog,
};
use ferrum_storage::{BandwidthMonitor, ObjectStorage, TransferQueue};
use std::path::PathBuf;
use std::sync::Arc;

#[derive(Clone)]
pub struct AppState {
    pub repo: Arc<DrsRepo>,
    pub storage: Option<Arc<dyn ObjectStorage>>,
    /// When set, GET .../access/{access_id} for objects with storage_backend s3/minio returns a presigned URL.
    pub s3_presigner: Option<Arc<dyn S3Presigner>>,
    /// When set, provenance/lineage is recorded and GET /objects/{id}/provenance is available.
    pub provenance_store: Option<Arc<ProvenanceStore>>,
    /// Directory with `{crypt4gh_master_key_id}.sec` for decrypting at-rest Crypt4GH objects on `/stream`.
    pub crypt4gh_key_dir: Option<PathBuf>,
    pub crypt4gh_master_key_id: String,
    /// When true, encrypted objects are decrypted when using `GET .../objects/{id}/stream`.
    pub crypt4gh_decrypt_stream: bool,
    /// Ingest API defaults and limits.
    pub ingest: IngestConfig,
    /// Value stored in `storage_references.storage_backend` for bytes written via ingest (e.g. `local`, `s3`).
    pub object_storage_backend: String,
    /// Outbreak Mode download approval (optional; set by gateway when `[outbreak] enabled`).
    pub outbreak: Option<Arc<OutbreakService>>,
    /// Rolling bandwidth estimation for adaptive chunk sizes and compression.
    pub bandwidth: Option<Arc<BandwidthMonitor>>,
    /// Large transfer deferral queue for VeryLow bandwidth links.
    pub transfer_queue: Option<Arc<TransferQueue>>,
    /// Append-only residency audit log (optional; set by gateway).
    pub residency_audit: Option<Arc<ResidencyAuditLog>>,
    /// Solar/battery gate for pausing background checksum/index work (optional; set by gateway).
    pub background_gate: Option<Arc<BackgroundWorkGate>>,
    /// Optional ADS introspection for published datasets (ga4gh-infra co-deploy).
    pub ads_introspect: Option<Arc<ferrum_core::AdsIntrospectClient>>,
    /// Optional Solum consent status client (H2.1 Teeth).
    pub solum_consent: Option<Arc<ferrum_core::SolumConsentClient>>,
    /// When true, ingest routes require collector/admin role (set by gateway from auth config).
    pub ingest_require_auth: bool,
    /// When true, `/api/v1/metadata/*` is enabled (set by gateway from `[metadata_store]`).
    pub metadata_store_enabled: bool,
    /// Post-ingest QC / Beacon / htsget automation (Phase 5).
    pub pipeline: PipelineConfig,
}
