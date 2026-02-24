//! App state for DRS (repo, optional storage for ingest, optional S3 presigner, optional provenance).
//!
//! **Crypt4GH integration:** When `storage_references.is_encrypted = true`, the DRS access URL
//! points to the same DRS server. Route GET /objects/{id}/access/{access_id} through
//! ferrum-crypt4gh's Crypt4GHLayer (e.g. in ferrum-gateway) so that responses are re-encrypted
//! for the requester's key (X-Crypt4GH-Public-Key header or Passport).

use crate::presign::S3Presigner;
use crate::repo::DrsRepo;
use ferrum_core::{ObjectStorage, ProvenanceStore};
use std::sync::Arc;

pub struct AppState {
    pub repo: Arc<DrsRepo>,
    pub storage: Option<Arc<dyn ObjectStorage>>,
    /// When set, GET .../access/{access_id} for objects with storage_backend s3/minio returns a presigned URL.
    pub s3_presigner: Option<Arc<dyn S3Presigner>>,
    /// When set, provenance/lineage is recorded and GET /objects/{id}/provenance is available.
    pub provenance_store: Option<Arc<ProvenanceStore>>,
}
