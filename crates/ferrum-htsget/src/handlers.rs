// SPDX-License-Identifier: Apache-2.0
#![allow(clippy::result_large_err)] // axum Response as Err (Clippy 1.99)
//! htsget HTTP handlers (GA4GH htsget 1.3.0-style tickets over DRS stream).

use crate::error::{htsget_error_response, HTSGET_JSON};
use crate::ticket::{
    classify_object, default_format_for, drs_stream_url, endpoint_matches_file,
    format_matches_file, normalize_format, EndpointKind, FileKind,
};
use crate::HtsgetState;
use axum::{
    body::Body,
    extract::{Extension, Path, Query, State},
    http::{HeaderMap, HeaderValue, Request, StatusCode},
    response::{IntoResponse, Response},
    Json,
};
use ferrum_core::AuthClaims;
use serde::Deserialize;
use serde_json::json;
use std::collections::HashSet;
use std::sync::Arc;
use utoipa::IntoParams;

#[derive(Debug, Deserialize, IntoParams)]
pub struct HtsgetGetQuery {
    pub format: Option<String>,
    pub class: Option<String>,
    #[serde(rename = "referenceName")]
    pub reference_name: Option<String>,
    pub start: Option<u64>,
    pub end: Option<u64>,
    pub fields: Option<String>,
    pub tags: Option<String>,
    pub notags: Option<String>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PostTicketBody {
    pub format: Option<String>,
    pub class: Option<String>,
    pub fields: Option<Vec<String>>,
    pub tags: Option<Vec<String>>,
    pub notags: Option<Vec<String>>,
    pub regions: Option<Vec<PostRegion>>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PostRegion {
    pub reference_name: String,
    pub start: Option<u64>,
    pub end: Option<u64>,
}

#[derive(Debug, Clone)]
struct NormalizedParams {
    format: Option<String>,
    class: Option<String>,
    /// `(referenceName, start, end)` per requested region; empty = whole object.
    regions: Vec<(String, Option<u64>, Option<u64>)>,
    reference_name: Option<String>,
    start: Option<u64>,
    end: Option<u64>,
    fields: Option<Vec<String>>,
    tags: Option<Vec<String>>,
    notags: Option<Vec<String>>,
}

fn split_csv(s: &str) -> Vec<String> {
    s.split(',')
        .map(|t| t.trim().to_string())
        .filter(|t| !t.is_empty())
        .collect()
}

fn tags_intersect(a: &[String], b: &[String]) -> bool {
    let sa: HashSet<&str> = a.iter().map(String::as_str).collect();
    b.iter().any(|x| sa.contains(x.as_str()))
}

#[allow(clippy::result_large_err)]
fn validate_range(start: Option<u64>, end: Option<u64>) -> Result<(), Response> {
    match (start, end) {
        (Some(s), Some(e)) if s > e => Err(htsget_error_response(
            StatusCode::BAD_REQUEST,
            "InvalidRange",
            "start must be <= end",
        )),
        _ => Ok(()),
    }
}

/// class=header: only `format` may be set (htsget 1.3.0).
#[allow(clippy::result_large_err)]
fn validate_class_header(p: &NormalizedParams) -> Result<(), Response> {
    if p.class.as_deref() != Some("header") {
        return Ok(());
    }
    let extra = p.reference_name.is_some()
        || p.start.is_some()
        || p.end.is_some()
        || p.fields.as_ref().is_some_and(|v| !v.is_empty())
        || p.tags.as_ref().is_some_and(|v| !v.is_empty())
        || p.notags.as_ref().is_some_and(|v| !v.is_empty());
    if extra {
        return Err(htsget_error_response(
            StatusCode::BAD_REQUEST,
            "InvalidInput",
            "when class=header, only format may be specified",
        ));
    }
    Ok(())
}

#[allow(clippy::result_large_err)]
fn validate_start_end_with_ref(
    endpoint: EndpointKind,
    reference_name: Option<&str>,
    start: Option<u64>,
    end: Option<u64>,
) -> Result<(), Response> {
    if start.is_none() && end.is_none() {
        return Ok(());
    }
    let refn = match reference_name {
        Some(r) => r,
        None => {
            return Err(htsget_error_response(
                StatusCode::BAD_REQUEST,
                "InvalidInput",
                "start/end require referenceName",
            ));
        }
    };
    if endpoint == EndpointKind::Reads && refn == "*" {
        return Err(htsget_error_response(
            StatusCode::BAD_REQUEST,
            "InvalidInput",
            "start/end cannot be used with referenceName=*",
        ));
    }
    Ok(())
}

#[allow(clippy::result_large_err)]
fn validate_get_params(
    endpoint: EndpointKind,
    q: &HtsgetGetQuery,
) -> Result<NormalizedParams, Response> {
    let fields = q.fields.as_ref().map(|s| split_csv(s));
    let tags = q.tags.as_ref().map(|s| split_csv(s));
    let notags = q.notags.as_ref().map(|s| split_csv(s));

    if let (Some(ta), Some(nt)) = (&tags, &notags) {
        if tags_intersect(ta, nt) {
            return Err(htsget_error_response(
                StatusCode::BAD_REQUEST,
                "InvalidInput",
                "tags and notags must not intersect",
            ));
        }
    }

    validate_range(q.start, q.end)?;

    let p = NormalizedParams {
        format: normalize_format(q.format.as_deref()),
        class: q.class.as_ref().map(|c| c.to_ascii_lowercase()),
        regions: q
            .reference_name
            .clone()
            .map(|r| vec![(r, q.start, q.end)])
            .unwrap_or_default(),
        reference_name: q.reference_name.clone(),
        start: q.start,
        end: q.end,
        fields,
        tags,
        notags,
    };

    validate_start_end_with_ref(endpoint, p.reference_name.as_deref(), p.start, p.end)?;

    validate_class(&p)?;
    if p.class.as_deref() == Some("header") {
        validate_class_header(&p)?;
    }

    Ok(p)
}

#[allow(clippy::result_large_err)]
fn validate_post_params(
    endpoint: EndpointKind,
    body: PostTicketBody,
) -> Result<NormalizedParams, Response> {
    if let (Some(ta), Some(nt)) = (&body.tags, &body.notags) {
        if tags_intersect(ta, nt) {
            return Err(htsget_error_response(
                StatusCode::BAD_REQUEST,
                "InvalidInput",
                "tags and notags must not intersect",
            ));
        }
    }

    if let Some(regions) = &body.regions {
        if regions.is_empty() {
            return Err(htsget_error_response(
                StatusCode::BAD_REQUEST,
                "InvalidInput",
                "regions must not be empty",
            ));
        }
        for r in regions {
            validate_range(r.start, r.end)?;
            validate_start_end_with_ref(endpoint, Some(&r.reference_name), r.start, r.end)?;
        }
    }

    let regions = body
        .regions
        .as_ref()
        .map(|rs| {
            rs.iter()
                .map(|r| (r.reference_name.clone(), r.start, r.end))
                .collect()
        })
        .unwrap_or_default();
    let p = NormalizedParams {
        format: normalize_format(body.format.as_deref()),
        class: body.class.as_ref().map(|c| c.to_ascii_lowercase()),
        regions,
        reference_name: None,
        start: None,
        end: None,
        fields: body.fields,
        tags: body.tags,
        notags: body.notags,
    };

    if p.class.as_deref() == Some("header") {
        let extra = body.regions.is_some()
            || p.fields.as_ref().is_some_and(|v| !v.is_empty())
            || p.tags.as_ref().is_some_and(|v| !v.is_empty())
            || p.notags.as_ref().is_some_and(|v| !v.is_empty());
        if extra {
            return Err(htsget_error_response(
                StatusCode::BAD_REQUEST,
                "InvalidInput",
                "when class=header, only format may be specified",
            ));
        }
    }
    validate_class(&p)?;

    Ok(p)
}

/// htsget 1.3 knows `header` and `body` (default).
#[allow(clippy::result_large_err)]
fn validate_class(p: &NormalizedParams) -> Result<(), Response> {
    match p.class.as_deref() {
        None | Some("header") | Some("body") => Ok(()),
        Some(other) => Err(htsget_error_response(
            StatusCode::BAD_REQUEST,
            "InvalidInput",
            format!("unknown class {other:?}; use header or body"),
        )),
    }
}

async fn ticket_for_object(
    state: &HtsgetState,
    endpoint: EndpointKind,
    raw_id: &str,
    params: NormalizedParams,
    auth: Option<&AuthClaims>,
    headers: &HeaderMap,
) -> Result<Response, Response> {
    let resolved = state
        .repo
        .resolve_id_or_uri(raw_id)
        .await
        .map_err(map_drs_err)?;
    let canonical = resolved.ok_or_else(|| {
        htsget_error_response(
            StatusCode::NOT_FOUND,
            "NotFound",
            format!("no object for id {:?}", raw_id),
        )
    })?;

    if let Some(dataset_id) = state
        .repo
        .get_dataset_id(&canonical)
        .await
        .map_err(map_drs_err)?
    {
        let claims = auth.ok_or_else(|| {
            htsget_error_response(
                StatusCode::FORBIDDEN,
                "PermissionDenied",
                "authentication required for this dataset",
            )
        })?;
        if !claims.has_dataset_grant(&dataset_id) && !claims.is_admin() {
            return Err(htsget_error_response(
                StatusCode::FORBIDDEN,
                "PermissionDenied",
                "dataset access not granted",
            ));
        }
    }

    let obj = state
        .repo
        .get_object(&canonical, false)
        .await
        .map_err(map_drs_err)?
        .ok_or_else(|| {
            htsget_error_response(StatusCode::NOT_FOUND, "NotFound", "object not found")
        })?;

    let kind = classify_object(obj.mime_type.as_deref(), obj.name.as_deref());
    if kind == FileKind::Other {
        return Err(htsget_error_response(
            StatusCode::NOT_FOUND,
            "NotFound",
            "object mime type is not recognized as reads or variants data",
        ));
    }
    if !endpoint_matches_file(endpoint, kind) {
        return Err(htsget_error_response(
            StatusCode::NOT_FOUND,
            "NotFound",
            "object type does not match this htsget endpoint (reads vs variants)",
        ));
    }

    let storage_ref = state
        .repo
        .get_storage_ref(&canonical)
        .await
        .map_err(map_drs_err)?
        .ok_or_else(|| {
            htsget_error_response(
                StatusCode::NOT_FOUND,
                "NotFound",
                "no storage reference for object",
            )
        })?;

    let default_fmt = default_format_for(endpoint, kind);
    let fmt = params.format.as_deref().unwrap_or(default_fmt);
    if !format_matches_file(fmt, kind) {
        return Err(htsget_error_response(
            StatusCode::BAD_REQUEST,
            "UnsupportedFormat",
            format!(
                "requested format {} is not available for this object (expected {})",
                fmt, default_fmt
            ),
        ));
    }

    let wants_slice = !params.regions.is_empty() || params.class.as_deref() == Some("header");
    if wants_slice {
        return region_ticket(
            state,
            &canonical,
            obj.name.as_deref(),
            fmt,
            storage_ref,
            &params,
            headers,
        )
        .await;
    }

    let url = drs_stream_url(&state.public_base_url, &canonical);
    let ticket = json!({
        "htsget": {
            "format": fmt,
            "urls": [{ "url": url }]
        }
    });

    Ok((
        StatusCode::OK,
        [(
            axum::http::header::CONTENT_TYPE,
            HeaderValue::from_static(HTSGET_JSON),
        )],
        Json(ticket),
    )
        .into_response())
}

/// Index file name suffixes per format. htsget-rs reads BAI for BAM and TBI for VCF.gz (not CSI),
/// CRAI for CRAM and CSI for BCF.
fn index_suffixes(fmt: &str) -> &'static [&'static str] {
    match fmt {
        "BAM" => &[".bai"],
        "CRAM" => &[".crai"],
        "VCF" => &[".tbi"],
        "BCF" => &[".csi"],
        _ => &[],
    }
}

/// Index object for a data object: metadata `htsget.index` (an object id) first, otherwise an
/// object named `<data name><suffix>` in the same dataset.
async fn find_index_object(
    state: &HtsgetState,
    canonical: &str,
    data_name: Option<&str>,
    fmt: &str,
) -> Result<Option<String>, Response> {
    let meta = state
        .repo
        .get_metadata(canonical)
        .await
        .map_err(map_drs_err)?;
    if let Some((_, id)) = meta.iter().find(|(k, _)| k == "htsget.index") {
        if let Some(c) = state.repo.resolve_id(id).await.map_err(map_drs_err)? {
            return Ok(Some(c));
        }
    }
    let Some(name) = data_name else {
        return Ok(None);
    };
    let dataset = state
        .repo
        .get_dataset_id(canonical)
        .await
        .map_err(map_drs_err)?;
    for suffix in index_suffixes(fmt) {
        let candidates = state
            .repo
            .find_object_ids_by_name(&format!("{name}{suffix}"))
            .await
            .map_err(map_drs_err)?;
        for id in candidates {
            let ds = state.repo.get_dataset_id(&id).await.map_err(map_drs_err)?;
            if ds == dataset {
                return Ok(Some(id));
            }
        }
    }
    Ok(None)
}

fn htsget_err(e: htsget_config::types::HtsGetError) -> Response {
    use htsget_config::types::HtsGetError as E;
    match e {
        E::NotFound(m) => htsget_error_response(StatusCode::NOT_FOUND, "NotFound", m),
        E::UnsupportedFormat(m) => {
            htsget_error_response(StatusCode::BAD_REQUEST, "UnsupportedFormat", m)
        }
        E::InvalidInput(m) => htsget_error_response(StatusCode::BAD_REQUEST, "InvalidInput", m),
        E::InvalidRange(m) => htsget_error_response(StatusCode::BAD_REQUEST, "InvalidRange", m),
        other => {
            tracing::warn!(error = %other, "htsget region search failed");
            htsget_error_response(
                StatusCode::INTERNAL_SERVER_ERROR,
                "InternalError",
                "could not compute the ticket for this object",
            )
        }
    }
}

/// Region and `class=header` tickets via htsget-rs (see `region.rs`).
#[allow(clippy::too_many_arguments)]
async fn region_ticket(
    state: &HtsgetState,
    canonical: &str,
    data_name: Option<&str>,
    fmt: &str,
    storage_ref: (String, String, bool),
    params: &NormalizedParams,
    headers: &HeaderMap,
) -> Result<Response, Response> {
    use crate::region::{self, DrsObjects, RegionRequest, StoredFile};

    let storage = state.storage.clone().ok_or_else(|| {
        htsget_error_response(
            StatusCode::NOT_IMPLEMENTED,
            "NotImplemented",
            "region tickets need object storage configured on this node",
        )
    })?;
    let format = region::htsget_format(fmt).ok_or_else(|| {
        htsget_error_response(
            StatusCode::BAD_REQUEST,
            "UnsupportedFormat",
            format!("format {fmt} cannot be sliced"),
        )
    })?;
    let (backend, data_key, encrypted) = storage_ref;
    if !matches!(
        backend.to_ascii_lowercase().as_str(),
        "local" | "s3" | "minio"
    ) {
        return Err(htsget_error_response(
            StatusCode::BAD_REQUEST,
            "InvalidInput",
            format!(
                "region tickets need the object in this node's storage (local, s3 or minio); this object is stored as '{backend}'"
            ),
        ));
    }

    let index_id = find_index_object(state, canonical, data_name, fmt)
        .await?
        .ok_or_else(|| {
            htsget_error_response(
                StatusCode::NOT_FOUND,
                "NotFound",
                format!(
                    "no index for this object: register one named <object name>{} in the same dataset, or set metadata htsget.index",
                    index_suffixes(fmt).join(" or ")
                ),
            )
        })?;
    let (index_backend, index_key, index_encrypted) = state
        .repo
        .get_storage_ref(&index_id)
        .await
        .map_err(map_drs_err)?
        .ok_or_else(|| {
            htsget_error_response(
                StatusCode::NOT_FOUND,
                "NotFound",
                "index object has no storage reference",
            )
        })?;

    if !matches!(
        index_backend.to_ascii_lowercase().as_str(),
        "local" | "s3" | "minio"
    ) {
        return Err(htsget_error_response(
            StatusCode::BAD_REQUEST,
            "InvalidInput",
            "the index object is not in this node's storage (local, s3 or minio)",
        ));
    }

    let base = &state.public_base_url;
    let mut data_url = drs_stream_url(base, canonical);
    if encrypted {
        data_url.push_str("?raw=true");
    }
    let mut index_url = drs_stream_url(base, &index_id);
    if index_encrypted {
        index_url.push_str("?raw=true");
    }
    let mut files = std::collections::HashMap::new();
    files.insert(
        region::data_key(format, encrypted),
        StoredFile {
            storage_key: data_key.trim().trim_start_matches('/').to_string(),
            stream_url: data_url,
        },
    );
    files.insert(
        region::index_key(format, index_encrypted),
        StoredFile {
            storage_key: index_key.trim().trim_start_matches('/').to_string(),
            stream_url: index_url,
        },
    );
    let objects = DrsObjects::new(storage, files);

    let c4gh = if encrypted {
        let pk = headers
            .get("x-crypt4gh-public-key")
            .or_else(|| headers.get("htsget-context-public-key"))
            .and_then(|v| v.to_str().ok())
            .ok_or_else(|| {
                htsget_error_response(
                    StatusCode::BAD_REQUEST,
                    "InvalidInput",
                    "this object is Crypt4GH-encrypted: send your public key (base64) in X-Crypt4GH-Public-Key; the ticket is encrypted for that key",
                )
            })?;
        let client_public_key = region::parse_client_public_key(pk).ok_or_else(|| {
            htsget_error_response(
                StatusCode::BAD_REQUEST,
                "InvalidInput",
                "X-Crypt4GH-Public-Key is not a base64 X25519 public key",
            )
        })?;
        let key_dir = state.crypt4gh_key_dir.as_ref().ok_or_else(|| {
            htsget_error_response(
                StatusCode::NOT_IMPLEMENTED,
                "NotImplemented",
                "Crypt4GH region tickets need the node key (encryption.crypt4gh_key_dir)",
            )
        })?;
        use ferrum_crypt4gh::KeyStore;
        let server = ferrum_crypt4gh::LocalKeyStore::new(key_dir.as_path())
            .get_private_key(&state.crypt4gh_master_key_id)
            .await
            .map_err(|e| {
                tracing::warn!(error = %e, "Crypt4GH node key could not be read");
                htsget_error_response(
                    StatusCode::INTERNAL_SERVER_ERROR,
                    "InternalError",
                    "node Crypt4GH key could not be read",
                )
            })?
            .ok_or_else(|| {
                htsget_error_response(
                    StatusCode::NOT_IMPLEMENTED,
                    "NotImplemented",
                    "node Crypt4GH key is missing",
                )
            })?;
        Some(region::C4ghKeys {
            server,
            client_public_key,
        })
    } else {
        None
    };

    let req = RegionRequest {
        class_header: params.class.as_deref() == Some("header"),
        regions: params.regions.clone(),
    };
    let response = region::search(&objects, format, &req, c4gh.as_ref())
        .await
        .map_err(htsget_err)?;
    let body =
        serde_json::to_value(htsget_config::types::JsonResponse::from(response)).map_err(|_| {
            htsget_error_response(
                StatusCode::INTERNAL_SERVER_ERROR,
                "InternalError",
                "could not encode the ticket",
            )
        })?;
    Ok((
        StatusCode::OK,
        [(
            axum::http::header::CONTENT_TYPE,
            HeaderValue::from_static(HTSGET_JSON),
        )],
        Json(body),
    )
        .into_response())
}

fn map_drs_err(e: ferrum_drs::error::DrsError) -> Response {
    match e {
        ferrum_drs::error::DrsError::NotFound(m) => {
            htsget_error_response(StatusCode::NOT_FOUND, "NotFound", m)
        }
        ferrum_drs::error::DrsError::Forbidden(m) => {
            htsget_error_response(StatusCode::FORBIDDEN, "PermissionDenied", m)
        }
        ferrum_drs::error::DrsError::Validation(m) => {
            htsget_error_response(StatusCode::BAD_REQUEST, "InvalidInput", m)
        }
        _ => htsget_error_response(
            StatusCode::INTERNAL_SERVER_ERROR,
            "InvalidInput",
            "internal server error",
        ),
    }
}

#[utoipa::path(
    get,
    path = "/reads/service-info",
    responses((status = 200, description = "htsget reads service-info"))
)]
pub async fn reads_service_info() -> impl IntoResponse {
    let doc = json!({
        "id": "ferrum-htsget-reads",
        "name": "Ferrum htsget (reads)",
        "version": env!("CARGO_PKG_VERSION"),
        "type": {
            "group": "org.ga4gh",
            "artifact": "htsget",
            "version": "1.3.0"
        },
        "description": "Tickets for alignment data as DRS stream URLs. Region requests (referenceName, start, end, POST regions) and class=header use the object's index (BAI/CSI for BAM, CRAI for CRAM) via htsget-rs; without an index only whole-object tickets are available. Crypt4GH objects need the requester's public key (X-Crypt4GH-Public-Key) and one region per request.",
        "organization": { "name": "Ferrum", "url": null },
        "htsget": {
            "datatype": "reads",
            "formats": ["BAM", "CRAM"],
            "fieldsParameterEffective": false,
            "tagsParametersEffective": false
        }
    });
    Json(doc)
}

#[utoipa::path(
    get,
    path = "/variants/service-info",
    responses((status = 200, description = "htsget variants service-info"))
)]
pub async fn variants_service_info() -> impl IntoResponse {
    let doc = json!({
        "id": "ferrum-htsget-variants",
        "name": "Ferrum htsget (variants)",
        "version": env!("CARGO_PKG_VERSION"),
        "type": {
            "group": "org.ga4gh",
            "artifact": "htsget",
            "version": "1.3.0"
        },
        "description": "Tickets for variant data as DRS stream URLs. Region requests (referenceName, start, end, POST regions) and class=header use the object's index (TBI/CSI for VCF.gz, CSI for BCF) via htsget-rs; without an index only whole-object tickets are available. Crypt4GH objects need the requester's public key (X-Crypt4GH-Public-Key) and one region per request; Crypt4GH BCF supports whole-object tickets only.",
        "organization": { "name": "Ferrum", "url": null },
        "htsget": {
            "datatype": "variants",
            "formats": ["VCF", "BCF"],
            "fieldsParameterEffective": false,
            "tagsParametersEffective": false
        }
    });
    Json(doc)
}

#[utoipa::path(
    get,
    path = "/reads/{id}",
    params(("id" = String, Path, description = "DRS object id"), HtsgetGetQuery),
    responses(
        (status = 200, description = "htsget ticket; urls point at DRS /stream (with Range headers for regions) or are data: URLs"),
        (status = 400, description = "InvalidInput, InvalidRange or UnsupportedFormat"),
        (status = 404, description = "NotFound (object, reference, or index for a region request)")
    )
)]
pub async fn get_reads_ticket(
    State(state): State<Arc<HtsgetState>>,
    Path(id): Path<String>,
    Query(q): Query<HtsgetGetQuery>,
    auth: Option<Extension<AuthClaims>>,
    headers: HeaderMap,
) -> Result<Response, Response> {
    let params = validate_get_params(EndpointKind::Reads, &q)?;
    ticket_for_object(
        &state,
        EndpointKind::Reads,
        &id,
        params,
        auth.as_ref().map(|e| &e.0),
        &headers,
    )
    .await
}

#[utoipa::path(
    get,
    path = "/variants/{id}",
    params(("id" = String, Path, description = "DRS object id"), HtsgetGetQuery),
    responses(
        (status = 200, description = "htsget ticket; urls point at DRS /stream (with Range headers for regions) or are data: URLs"),
        (status = 400, description = "InvalidInput, InvalidRange or UnsupportedFormat"),
        (status = 404, description = "NotFound (object, reference, or index for a region request)")
    )
)]
pub async fn get_variants_ticket(
    State(state): State<Arc<HtsgetState>>,
    Path(id): Path<String>,
    Query(q): Query<HtsgetGetQuery>,
    auth: Option<Extension<AuthClaims>>,
    headers: HeaderMap,
) -> Result<Response, Response> {
    let params = validate_get_params(EndpointKind::Variants, &q)?;
    ticket_for_object(
        &state,
        EndpointKind::Variants,
        &id,
        params,
        auth.as_ref().map(|e| &e.0),
        &headers,
    )
    .await
}

/// Max POST body size for ticket JSON (htsget multi-region requests stay small).
const MAX_POST_BODY: usize = 4 * 1024 * 1024;

async fn parse_post_ticket_body(
    req: Request<Body>,
) -> std::result::Result<(PostTicketBody, Option<AuthClaims>, HeaderMap), Response> {
    let (parts, body) = req.into_parts();
    let headers = parts.headers.clone();
    if parts.uri.query().is_some_and(|q| !q.is_empty()) {
        return Err(htsget_error_response(
            StatusCode::BAD_REQUEST,
            "InvalidInput",
            "POST must not include query parameters; use JSON body",
        ));
    }
    let auth = parts.extensions.get::<AuthClaims>().cloned();
    let bytes = axum::body::to_bytes(body, MAX_POST_BODY)
        .await
        .map_err(|e| {
            let msg = e.to_string();
            if msg.contains("length limit") || msg.contains("LengthLimitError") {
                return htsget_error_response(
                    StatusCode::PAYLOAD_TOO_LARGE,
                    "PayloadTooLarge",
                    "request body exceeds limit",
                );
            }
            htsget_error_response(
                StatusCode::BAD_REQUEST,
                "InvalidInput",
                format!("could not read body: {}", e),
            )
        })?;
    let body: PostTicketBody = serde_json::from_slice(&bytes).map_err(|e| {
        htsget_error_response(
            StatusCode::BAD_REQUEST,
            "InvalidInput",
            format!("invalid JSON: {}", e),
        )
    })?;
    Ok((body, auth, headers))
}

#[utoipa::path(
    post,
    path = "/reads/{id}",
    params(("id" = String, Path, description = "DRS object id")),
    responses(
        (status = 200, description = "htsget ticket; urls point at DRS /stream (with Range headers for regions) or are data: URLs"),
        (status = 400, description = "InvalidInput, InvalidRange or UnsupportedFormat"),
        (status = 404, description = "NotFound (object, reference, or index for a region request)")
    )
)]
pub async fn post_reads_ticket(
    State(state): State<Arc<HtsgetState>>,
    Path(id): Path<String>,
    req: Request<Body>,
) -> Result<Response, Response> {
    let (body, auth, headers) = parse_post_ticket_body(req).await?;
    let params = validate_post_params(EndpointKind::Reads, body)?;
    ticket_for_object(
        &state,
        EndpointKind::Reads,
        &id,
        params,
        auth.as_ref(),
        &headers,
    )
    .await
}

#[utoipa::path(
    post,
    path = "/variants/{id}",
    params(("id" = String, Path, description = "DRS object id")),
    responses(
        (status = 200, description = "htsget ticket; urls point at DRS /stream (with Range headers for regions) or are data: URLs"),
        (status = 400, description = "InvalidInput, InvalidRange or UnsupportedFormat"),
        (status = 404, description = "NotFound (object, reference, or index for a region request)")
    )
)]
pub async fn post_variants_ticket(
    State(state): State<Arc<HtsgetState>>,
    Path(id): Path<String>,
    req: Request<Body>,
) -> Result<Response, Response> {
    let (body, auth, headers) = parse_post_ticket_body(req).await?;
    let params = validate_post_params(EndpointKind::Variants, body)?;
    ticket_for_object(
        &state,
        EndpointKind::Variants,
        &id,
        params,
        auth.as_ref(),
        &headers,
    )
    .await
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn get_region_query_is_accepted() {
        let q = HtsgetGetQuery {
            format: None,
            class: None,
            reference_name: Some("chr1".into()),
            start: Some(0),
            end: Some(100),
            fields: None,
            tags: None,
            notags: None,
        };
        let p = validate_get_params(EndpointKind::Reads, &q).expect("region accepted");
        assert_eq!(p.regions, vec![("chr1".to_string(), Some(0), Some(100))]);
    }

    #[test]
    fn region_with_start_after_end_is_rejected() {
        let q = HtsgetGetQuery {
            format: None,
            class: None,
            reference_name: Some("chr1".into()),
            start: Some(100),
            end: Some(10),
            fields: None,
            tags: None,
            notags: None,
        };
        let err = validate_get_params(EndpointKind::Reads, &q).unwrap_err();
        assert_eq!(err.status(), StatusCode::BAD_REQUEST);
    }

    #[test]
    fn start_without_reference_and_unknown_class_are_rejected() {
        let mut q = HtsgetGetQuery {
            format: None,
            class: None,
            reference_name: None,
            start: Some(5),
            end: None,
            fields: None,
            tags: None,
            notags: None,
        };
        assert!(validate_get_params(EndpointKind::Reads, &q).is_err());
        q.start = None;
        q.class = Some("everything".into());
        assert!(validate_get_params(EndpointKind::Reads, &q).is_err());
        q.class = Some("header".into());
        assert!(validate_get_params(EndpointKind::Reads, &q).is_ok());
        q.reference_name = Some("chr1".into());
        assert!(
            validate_get_params(EndpointKind::Reads, &q).is_err(),
            "header + region"
        );
    }

    #[test]
    fn post_regions_are_accepted() {
        let body = PostTicketBody {
            format: None,
            class: None,
            fields: None,
            tags: None,
            notags: None,
            regions: Some(vec![PostRegion {
                reference_name: "chr1".into(),
                start: Some(0),
                end: Some(10),
            }]),
        };
        let p = validate_post_params(EndpointKind::Variants, body).expect("regions accepted");
        assert_eq!(p.regions, vec![("chr1".to_string(), Some(0), Some(10))]);
    }

    #[test]
    fn whole_object_get_is_ok() {
        let q = HtsgetGetQuery {
            format: Some("BAM".into()),
            class: None,
            reference_name: None,
            start: None,
            end: None,
            fields: None,
            tags: None,
            notags: None,
        };
        assert!(validate_get_params(EndpointKind::Reads, &q).is_ok());
    }
}
