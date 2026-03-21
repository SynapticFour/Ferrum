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
    http::{HeaderValue, Request, StatusCode},
    response::{IntoResponse, Response},
    Json,
};
use ferrum_core::AuthClaims;
use serde::Deserialize;
use serde_json::json;
use std::collections::HashSet;
use std::sync::Arc;

#[derive(Debug, Deserialize)]
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
    Err(htsget_error_response(
        StatusCode::BAD_REQUEST,
        "InvalidInput",
        "class=header is not supported; request full data stream (omit class)",
    ))
}

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
        reference_name: q.reference_name.clone(),
        start: q.start,
        end: q.end,
        fields,
        tags,
        notags,
    };

    validate_start_end_with_ref(endpoint, p.reference_name.as_deref(), p.start, p.end)?;

    if p.class.as_deref() == Some("header") {
        validate_class_header(&p)?;
    }

    Ok(p)
}

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

    let p = NormalizedParams {
        format: normalize_format(body.format.as_deref()),
        class: body.class.as_ref().map(|c| c.to_ascii_lowercase()),
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
        return Err(htsget_error_response(
            StatusCode::BAD_REQUEST,
            "InvalidInput",
            "class=header is not supported; request full data stream (omit class)",
        ));
    }

    Ok(p)
}

async fn ticket_for_object(
    state: &HtsgetState,
    endpoint: EndpointKind,
    raw_id: &str,
    params: NormalizedParams,
    auth: Option<&AuthClaims>,
) -> Result<Response, Response> {
    let resolved = state
        .repo
        .resolve_id_or_uri(raw_id)
        .await
        .map_err(|e| map_drs_err(e))?;
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
        .map_err(|e| map_drs_err(e))?
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
        .map_err(|e| map_drs_err(e))?
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

    let storage = state
        .repo
        .get_storage_ref(&canonical)
        .await
        .map_err(|e| map_drs_err(e))?;
    if storage.is_none() {
        return Err(htsget_error_response(
            StatusCode::NOT_FOUND,
            "NotFound",
            "no storage reference for object",
        ));
    }

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
        "description": "htsget tickets for alignment data; data blocks are served via DRS stream",
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
        "description": "htsget tickets for variant data; data blocks are served via DRS stream",
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

pub async fn get_reads_ticket(
    State(state): State<Arc<HtsgetState>>,
    Path(id): Path<String>,
    Query(q): Query<HtsgetGetQuery>,
    auth: Option<Extension<AuthClaims>>,
) -> Result<Response, Response> {
    let params = validate_get_params(EndpointKind::Reads, &q)?;
    ticket_for_object(
        &state,
        EndpointKind::Reads,
        &id,
        params,
        auth.as_ref().map(|e| &e.0),
    )
    .await
}

pub async fn get_variants_ticket(
    State(state): State<Arc<HtsgetState>>,
    Path(id): Path<String>,
    Query(q): Query<HtsgetGetQuery>,
    auth: Option<Extension<AuthClaims>>,
) -> Result<Response, Response> {
    let params = validate_get_params(EndpointKind::Variants, &q)?;
    ticket_for_object(
        &state,
        EndpointKind::Variants,
        &id,
        params,
        auth.as_ref().map(|e| &e.0),
    )
    .await
}

/// Max POST body size for ticket JSON (htsget multi-region requests stay small).
const MAX_POST_BODY: usize = 4 * 1024 * 1024;

async fn parse_post_ticket_body(
    req: Request<Body>,
) -> std::result::Result<(PostTicketBody, Option<AuthClaims>), Response> {
    let (parts, body) = req.into_parts();
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
    Ok((body, auth))
}

pub async fn post_reads_ticket(
    State(state): State<Arc<HtsgetState>>,
    Path(id): Path<String>,
    req: Request<Body>,
) -> Result<Response, Response> {
    let (body, auth) = parse_post_ticket_body(req).await?;
    let params = validate_post_params(EndpointKind::Reads, body)?;
    ticket_for_object(&state, EndpointKind::Reads, &id, params, auth.as_ref()).await
}

pub async fn post_variants_ticket(
    State(state): State<Arc<HtsgetState>>,
    Path(id): Path<String>,
    req: Request<Body>,
) -> Result<Response, Response> {
    let (body, auth) = parse_post_ticket_body(req).await?;
    let params = validate_post_params(EndpointKind::Variants, body)?;
    ticket_for_object(&state, EndpointKind::Variants, &id, params, auth.as_ref()).await
}
