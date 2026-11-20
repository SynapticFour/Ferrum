// SPDX-License-Identifier: Apache-2.0
//! Region tickets (htsget 1.3 `referenceName` / `start` / `end`, POST `regions`, `class=header`)
//! computed by [htsget-rs](https://github.com/umccr/htsget-rs) over Ferrum DRS objects.
//!
//! htsget-rs reads the index (BAI/CSI/TBI/CRAI) and the file header through [`DrsObjects`], a
//! [`StorageTrait`] adapter over Ferrum's [`ObjectStorage`]. Ticket URLs point at DRS
//! `GET /objects/{id}/stream` with an HTTP `Range` header, so clients fetch bytes through the
//! same access checks as any DRS download.
//!
//! Crypt4GH objects: htsget-rs' `C4GHStorage` decrypts only the header region with the node key
//! and builds tickets whose first URLs are a Crypt4GH header re-encrypted for the requester's
//! public key (with a data edit list), followed by ranges of the stored ciphertext
//! (`/stream?raw=true`). The client decrypts the concatenation with its own key.
//!
//! Plain BAM, VCF.gz and BCF (not the unplaced `*` reference): the ticket comes from
//! [`crate::bgzf_edges`], which reads the same index with noodles and cuts the header end and the
//! first and last block of each index chunk at the record boundary the index records.
//! htsget-rs' block-aligned ranges break when records span BGZF blocks (every htslib BCF) and
//! when a header shares its last block with the first records.

use crate::bgzf_edges::{merge_chunks, range_urls, BgzfFormat, Located};
use async_trait::async_trait;
use ferrum_storage::ObjectStorage;
use htsget_config::encryption_scheme::EncryptionScheme;
use htsget_config::types::{Class, Format, HtsGetError, Interval, Query, Request, Response, Url};
use htsget_search::from_storage::HtsGetFromStorage;
use htsget_search::HtsGet;
use htsget_storage::c4gh::storage::C4GHStorage;
use htsget_storage::types::{GetOptions, HeadOptions, RangeUrlOptions};
use htsget_storage::{Storage, StorageMiddleware, StorageTrait, Streamable};
use noodles::bgzf::VirtualPosition;
use std::collections::HashMap;
use std::sync::Arc;
use tokio::io::AsyncRead;

/// Internal id used for htsget-rs keys (`obj.bam`, `obj.bam.bai`, `obj.bam.c4gh`, ...).
const KEY_ID: &str = "obj";

/// One stored object as htsget-rs sees it.
#[derive(Clone, Debug)]
pub struct StoredFile {
    /// Key in Ferrum object storage.
    pub storage_key: String,
    /// DRS stream URL clients fetch ranges from (already includes `?raw=true` where needed).
    pub stream_url: String,
}

/// [`StorageTrait`] over Ferrum object storage for one ticket request.
#[derive(Clone)]
pub struct DrsObjects {
    storage: Arc<dyn ObjectStorage>,
    /// htsget-rs key → stored file.
    files: Arc<HashMap<String, StoredFile>>,
}

impl DrsObjects {
    pub fn new(storage: Arc<dyn ObjectStorage>, files: HashMap<String, StoredFile>) -> Self {
        Self {
            storage,
            files: Arc::new(files),
        }
    }

    fn file(&self, key: &str) -> htsget_storage::error::Result<&StoredFile> {
        self.files
            .get(key)
            .ok_or_else(|| htsget_storage::error::StorageError::KeyNotFound(key.to_string()))
    }
}

fn storage_err(e: ferrum_core::FerrumError) -> htsget_storage::error::StorageError {
    match e {
        ferrum_core::FerrumError::NotFound(k) => {
            htsget_storage::error::StorageError::KeyNotFound(k)
        }
        other => htsget_storage::error::StorageError::IoError(
            "object storage".to_string(),
            std::io::Error::other(other.to_string()),
        ),
    }
}

impl StorageMiddleware for DrsObjects {}

#[async_trait]
impl StorageTrait for DrsObjects {
    async fn get(
        &self,
        key: &str,
        options: GetOptions<'_>,
    ) -> htsget_storage::error::Result<Streamable> {
        let file = self.file(key)?;
        let range = options.range();
        let reader: Box<dyn AsyncRead + Send + Unpin> = match (range.get_start(), range.get_end()) {
            (None, None) => self
                .storage
                .get(&file.storage_key)
                .await
                .map_err(storage_err)?,
            (start, end) => {
                let start = start.unwrap_or(0);
                // BytesPosition ends are exclusive; get_range takes an inclusive end.
                let end = match end {
                    Some(e) if e > start => e - 1,
                    Some(_) => {
                        return Ok(Streamable::from_async_read(
                            std::io::Cursor::new(Vec::new()),
                        ))
                    }
                    None => u64::MAX,
                };
                let size = self
                    .storage
                    .size(&file.storage_key)
                    .await
                    .map_err(storage_err)?;
                if start >= size {
                    return Ok(Streamable::from_async_read(
                        std::io::Cursor::new(Vec::new()),
                    ));
                }
                self.storage
                    .get_range(&file.storage_key, start, end.min(size - 1))
                    .await
                    .map_err(storage_err)?
            }
        };
        Ok(Streamable::from_async_read(SyncReader(
            std::sync::Mutex::new(reader),
        )))
    }

    async fn range_url(
        &self,
        key: &str,
        options: RangeUrlOptions<'_>,
    ) -> htsget_storage::error::Result<Url> {
        let file = self.file(key)?;
        options.apply(Url::new(file.stream_url.clone()))
    }

    async fn head(
        &self,
        key: &str,
        _options: HeadOptions<'_>,
    ) -> htsget_storage::error::Result<u64> {
        let file = self.file(key)?;
        self.storage
            .size(&file.storage_key)
            .await
            .map_err(storage_err)
    }
}

/// `Streamable::from_async_read` wants `Sync`; Ferrum readers are `Send` only. The reader is
/// only polled through `&mut self`, so the Mutex is never contended; it just makes the type
/// `Sync` without unsafe code.
struct SyncReader(std::sync::Mutex<Box<dyn AsyncRead + Send + Unpin>>);

impl AsyncRead for SyncReader {
    fn poll_read(
        mut self: std::pin::Pin<&mut Self>,
        cx: &mut std::task::Context<'_>,
        buf: &mut tokio::io::ReadBuf<'_>,
    ) -> std::task::Poll<std::io::Result<()>> {
        let inner = self
            .0
            .get_mut()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        std::pin::Pin::new(inner).poll_read(cx, buf)
    }
}

/// What the client asked for, already validated by the handler.
#[derive(Clone, Debug, Default)]
pub struct RegionRequest {
    pub class_header: bool,
    /// `(referenceName, start, end)`; `None` for a header-only request.
    pub regions: Vec<(String, Option<u64>, Option<u64>)>,
}

/// Crypt4GH material for an encrypted object.
pub struct C4ghKeys {
    /// Node key(s) that decrypt the stored header.
    pub server: Vec<crypt4gh::Keys>,
    /// Requester's X25519 public key (32 bytes).
    pub client_public_key: Vec<u8>,
}

pub fn htsget_format(fmt: &str) -> Option<Format> {
    match fmt {
        "BAM" => Some(Format::Bam),
        "CRAM" => Some(Format::Cram),
        "VCF" => Some(Format::Vcf),
        "BCF" => Some(Format::Bcf),
        _ => None,
    }
}

/// htsget-rs keys for the data file and its index.
pub fn data_key(format: Format, encrypted: bool) -> String {
    let k = format.fmt_file(KEY_ID);
    if encrypted {
        C4GHStorage::format_key(&k)
    } else {
        k
    }
}

pub fn index_key(format: Format, encrypted: bool) -> String {
    let k = format.fmt_index(KEY_ID);
    if encrypted {
        C4GHStorage::format_key(&k)
    } else {
        k
    }
}

fn to_u32(v: u64, what: &str) -> Result<u32, HtsGetError> {
    u32::try_from(v).map_err(|_| HtsGetError::InvalidRange(format!("{what} {v} is out of range")))
}

fn query_for(
    format: Format,
    class_header: bool,
    region: Option<&(String, Option<u64>, Option<u64>)>,
    encrypted: bool,
) -> Result<Query, HtsGetError> {
    let mut q = Query::new(KEY_ID, format, Request::new_with_id(KEY_ID.to_string()));
    if class_header {
        q = q.with_class(Class::Header);
    }
    if let Some((reference, start, end)) = region {
        q = q.with_reference_name(reference.clone());
        let start = start.map(|s| to_u32(s, "start")).transpose()?;
        let end = end.map(|e| to_u32(e, "end")).transpose()?;
        q = q.with_interval(Interval::new(start, end));
    }
    if encrypted {
        q = q.with_encryption_scheme(EncryptionScheme::C4GH);
    }
    Ok(q)
}

fn storage_for(objects: &DrsObjects, c4gh: Option<&C4ghKeys>) -> Storage {
    match c4gh {
        None => Storage::new(objects.clone()),
        Some(keys) => {
            // Re-encrypt the header for the requester: sender = node key, recipient = client.
            let client = keys
                .server
                .iter()
                .map(|k| crypt4gh::Keys {
                    method: 0,
                    privkey: k.privkey.clone(),
                    recipient_pubkey: keys.client_public_key.clone(),
                })
                .collect();
            Storage::new(C4GHStorage::new(
                keys.server.clone(),
                client,
                objects.clone(),
                false,
                String::new(),
            ))
        }
    }
}

/// Run htsget-rs for one or more regions and merge the tickets.
///
/// Several regions on a plain object: header once, body ranges of all regions (overlapping or
/// adjacent ranges on the same URL merged), EOF once. Several regions on a Crypt4GH object are
/// rejected: each htsget-rs ticket carries its own header and edit list.
pub async fn search(
    objects: &DrsObjects,
    format: Format,
    req: &RegionRequest,
    c4gh: Option<&C4ghKeys>,
) -> Result<Response, HtsGetError> {
    let encrypted = c4gh.is_some();
    let bgzf = match format {
        Format::Bam => Some(BgzfFormat::Bam),
        Format::Vcf => Some(BgzfFormat::Vcf),
        Format::Bcf => Some(BgzfFormat::Bcf),
        _ => None,
    };
    let header_only = req.class_header || req.regions.is_empty();
    let unplaced = req.regions.iter().any(|(r, _, _)| r == "*");
    if let (Some(bgzf), false, false) = (bgzf, encrypted, unplaced) {
        // Plain BAM / VCF.gz / BCF: record-exact ticket from the index (bgzf_edges.rs).
        let regions: &[_] = if header_only { &[] } else { &req.regions };
        return record_exact(objects, format, bgzf, regions).await;
    }
    if header_only {
        let q = query_for(format, true, None, encrypted)?;
        let mut response = HtsGetFromStorage::new(storage_for(objects, c4gh))
            .search(q)
            .await?;
        if !encrypted {
            // htsget-rs returns the header bytes only. Append the format's end-of-file marker so
            // the concatenated ticket is a complete file (CRAM readers need the EOF container).
            // Crypt4GH tickets are left as they are: their bytes are one encrypted stream.
            response.urls.push(eof_url(format));
        }
        return Ok(response);
    }
    if encrypted && format == Format::Bcf {
        // htslib writes BCF records across BGZF blocks; a block-aligned Crypt4GH ticket would
        // start or end inside a record (see bgzf_edges.rs). Not served rather than served broken.
        return Err(HtsGetError::UnsupportedFormat(
            "region requests on Crypt4GH-encrypted BCF are not supported; request the whole object or use VCF.gz"
                .into(),
        ));
    }
    if encrypted && req.regions.len() > 1 {
        return Err(HtsGetError::InvalidInput(
            "several regions in one request are not supported for Crypt4GH objects; send one request per region"
                .into(),
        ));
    }
    let mut responses = Vec::with_capacity(req.regions.len());
    for region in &req.regions {
        let q = query_for(format, false, Some(region), encrypted)?;
        responses.push(
            HtsGetFromStorage::new(storage_for(objects, c4gh))
                .search(q)
                .await?,
        );
    }
    if responses.len() == 1 {
        return Ok(responses.remove(0));
    }
    Ok(merge_plain_responses(format, responses))
}

/// Plain BGZF ticket: header `[0, first record)`, the regions' index chunks, EOF marker; every
/// edge cut at the record boundary the index records (see bgzf_edges.rs).
async fn record_exact(
    objects: &DrsObjects,
    format: Format,
    bgzf: BgzfFormat,
    regions: &[(String, Option<u64>, Option<u64>)],
) -> Result<Response, HtsGetError> {
    let to_err = |e: htsget_storage::error::StorageError| HtsGetError::InternalError(e.to_string());
    let data = objects.file(&data_key(format, false)).map_err(to_err)?;
    let index = objects.file(&index_key(format, false)).map_err(to_err)?;
    let storage = objects.storage.as_ref();
    let located = Located::load(storage, bgzf, &index.storage_key, &data.storage_key).await?;
    let mut urls = range_urls(
        storage,
        &data.storage_key,
        &data.stream_url,
        &[(VirtualPosition::default(), located.header_end())],
        Class::Header,
    )
    .await?;
    let mut chunks = Vec::new();
    for (reference, start, end) in regions {
        chunks.extend(located.chunks(reference, *start, *end)?);
    }
    urls.extend(
        range_urls(
            storage,
            &data.storage_key,
            &data.stream_url,
            &merge_chunks(chunks),
            Class::Body,
        )
        .await?,
    );
    urls.push(eof_url(format));
    Ok(Response::new(format, urls))
}

/// BGZF end-of-file block (SAM/BAM spec § 4.1.2), used by BAM, VCF.gz and BCF.
const BGZF_EOF: [u8; 28] = [
    0x1f, 0x8b, 0x08, 0x04, 0x00, 0x00, 0x00, 0x00, 0x00, 0xff, 0x06, 0x00, 0x42, 0x43, 0x02, 0x00,
    0x1b, 0x00, 0x03, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
];

/// CRAM 3.x end-of-file container (CRAM spec § 9).
const CRAM_EOF: [u8; 38] = [
    0x0f, 0x00, 0x00, 0x00, 0xff, 0xff, 0xff, 0xff, 0x0f, 0xe0, 0x45, 0x4f, 0x46, 0x00, 0x00, 0x00,
    0x00, 0x01, 0x00, 0x05, 0xbd, 0xd9, 0x4f, 0x00, 0x01, 0x00, 0x06, 0x06, 0x01, 0x00, 0x01, 0x00,
    0x01, 0x00, 0xee, 0x63, 0x01, 0x4b,
];

fn eof_url(format: Format) -> Url {
    use base64::Engine;
    let bytes: &[u8] = match format {
        Format::Cram => &CRAM_EOF,
        _ => &BGZF_EOF,
    };
    Url::new(format!(
        "data:;base64,{}",
        base64::engine::general_purpose::STANDARD.encode(bytes)
    ))
}

fn parse_range(url: &Url) -> Option<(u64, u64)> {
    let v = url.headers.as_ref()?.as_ref_inner().get("Range")?;
    let (a, b) = v.strip_prefix("bytes=")?.split_once('-')?;
    Some((a.parse().ok()?, b.parse().ok()?))
}

/// Merge tickets of several regions on one plain object.
fn merge_plain_responses(format: Format, responses: Vec<Response>) -> Response {
    let mut header: Vec<Url> = Vec::new();
    let mut eof: Option<Url> = None;
    let mut body: Vec<(String, u64, u64, Option<Class>)> = Vec::new();
    let mut other: Vec<Url> = Vec::new();
    for (i, r) in responses.into_iter().enumerate() {
        let n = r.urls.len();
        for (j, u) in r.urls.into_iter().enumerate() {
            if u.class == Some(Class::Header) {
                if i == 0 {
                    header.push(u);
                }
                continue;
            }
            if u.url.starts_with("data:") && j + 1 == n {
                eof.get_or_insert(u);
                continue;
            }
            match parse_range(&u) {
                Some((a, b)) => body.push((u.url.clone(), a, b, u.class)),
                None => other.push(u),
            }
        }
    }
    body.sort_by(|x, y| (x.0.as_str(), x.1).cmp(&(y.0.as_str(), y.1)));
    let mut merged: Vec<(String, u64, u64, Option<Class>)> = Vec::new();
    for b in body {
        match merged.last_mut() {
            Some(m) if m.0 == b.0 && b.1 <= m.2.saturating_add(1) => m.2 = m.2.max(b.2),
            _ => merged.push(b),
        }
    }
    let mut urls = header;
    urls.extend(other);
    for (url, a, b, class) in merged {
        let mut u = Url::new(url).add_headers(
            htsget_config::types::Headers::default().with_header("Range", format!("bytes={a}-{b}")),
        );
        u = u.set_class(class);
        urls.push(u);
    }
    if let Some(e) = eof {
        urls.push(e);
    }
    Response::new(format, urls)
}

/// Requester public key from `X-Crypt4GH-Public-Key` (Ferrum) or `Htsget-Context-Public-Key`
/// (htsget-rs): base64 of either the raw 32-byte X25519 key or a Crypt4GH public key file.
pub fn parse_client_public_key(value: &str) -> Option<Vec<u8>> {
    use base64::Engine;
    let b64 = base64::engine::general_purpose::STANDARD;
    let decoded = b64.decode(value.trim()).ok()?;
    if decoded.len() == 32 {
        return Some(decoded);
    }
    let text = String::from_utf8(decoded).ok()?;
    let inner: String = text
        .lines()
        .map(str::trim)
        .filter(|l| !l.is_empty() && !l.starts_with("-----"))
        .collect();
    let raw = b64.decode(inner).ok()?;
    (raw.len() == 32).then_some(raw)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn keys_follow_htsget_rs_naming() {
        assert_eq!(data_key(Format::Bam, false), "obj.bam");
        assert_eq!(index_key(Format::Bam, false), "obj.bam.bai");
        assert_eq!(data_key(Format::Vcf, true), "obj.vcf.gz.c4gh");
        assert_eq!(index_key(Format::Vcf, false), "obj.vcf.gz.tbi");
        assert_eq!(index_key(Format::Bcf, false), "obj.bcf.csi");
    }

    #[test]
    fn client_key_raw_and_pem() {
        use base64::Engine;
        let b64 = base64::engine::general_purpose::STANDARD;
        let raw = [7u8; 32];
        assert_eq!(parse_client_public_key(&b64.encode(raw)).unwrap(), raw);
        let pem = format!(
            "-----BEGIN CRYPT4GH PUBLIC KEY-----\n{}\n-----END CRYPT4GH PUBLIC KEY-----\n",
            b64.encode(raw)
        );
        assert_eq!(parse_client_public_key(&b64.encode(pem)).unwrap(), raw);
        assert!(parse_client_public_key("not base64!").is_none());
        assert!(parse_client_public_key(&b64.encode([1u8; 31])).is_none());
    }

    #[test]
    fn merging_regions_keeps_one_header_and_one_eof() {
        let range = |a: u64, b: u64| {
            Url::new("https://h/s")
                .add_headers(
                    htsget_config::types::Headers::default()
                        .with_header("Range", format!("bytes={a}-{b}")),
                )
                .set_class(Some(Class::Body))
        };
        let hdr = || {
            Url::new("https://h/s")
                .add_headers(
                    htsget_config::types::Headers::default().with_header("Range", "bytes=0-99"),
                )
                .set_class(Some(Class::Header))
        };
        let eof = || Url::new("data:;base64,EOF").set_class(Some(Class::Body));
        let r1 = Response::new(Format::Bam, vec![hdr(), range(100, 199), eof()]);
        let r2 = Response::new(
            Format::Bam,
            vec![hdr(), range(150, 299), range(500, 599), eof()],
        );
        let m = merge_plain_responses(Format::Bam, vec![r1, r2]);
        let got: Vec<(String, Option<(u64, u64)>)> = m
            .urls
            .iter()
            .map(|u| (u.url.clone(), parse_range(u)))
            .collect();
        assert_eq!(
            got,
            vec![
                ("https://h/s".into(), Some((0, 99))),
                ("https://h/s".into(), Some((100, 299))),
                ("https://h/s".into(), Some((500, 599))),
                ("data:;base64,EOF".into(), None),
            ]
        );
    }
}
