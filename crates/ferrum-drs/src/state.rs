//! App state for DRS (repo, optional storage for ingest, optional S3 presigner, optional provenance).
//!
//! **Crypt4GH integration:** When `storage_references.is_encrypted = true`, bytes are stored in
//! Crypt4GH form. `GET /objects/{id}/stream` can decrypt server-side and stream **plaintext** when
//! `crypt4gh_decrypt_stream` is enabled and a node key is configured. Optional: wrap stream routes
//! with `Crypt4GHLayer` in the gateway to re-encrypt for the client's public key.

use crate::presign::S3Presigner;
use crate::repo::DrsRepo;
use ferrum_core::{ObjectStorage, ProvenanceStore};
use std::path::PathBuf;
use std::sync::Arc;

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
}
