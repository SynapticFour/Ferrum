// SPDX-License-Identifier: Apache-2.0
//! End-to-end region tickets: real BAM/VCF files with indexes are stored as DRS objects, the
//! htsget API returns a ticket, the test fetches every ticket URL through the DRS stream route
//! (with the Range headers from the ticket), concatenates the bytes and parses the result.
//!
//! The expected records come from noodles' own indexed query on the original file plus a plain
//! overlap filter, not from htsget-rs. A ticket may return records just outside the region
//! (BGZF block granularity, allowed by htsget); it must return every record inside it, parse as
//! a valid file, and be much smaller than the whole object for a small region.

use base64::Engine;
use ferrum_core::FerrumPool;
use ferrum_drs::handlers::get_object_stream;
use ferrum_drs::state::AppState;
use ferrum_htsget::HtsgetState;
use ferrum_storage::{LocalStorage, ObjectStorage};
use http::{Method, Request, StatusCode};
use noodles::core::Position;
use noodles::sam::alignment::io::Write as _;
use noodles::sam::alignment::record::cigar::op::{Kind, Op};
use noodles::sam::alignment::record::Flags;
use noodles::sam::alignment::record_buf::{QualityScores, Sequence};
use noodles::sam::alignment::RecordBuf;
use noodles::sam::header::record::value::map::header::{sort_order, tag as header_tag};
use noodles::sam::header::record::value::map::{Header as HeaderMap, ReferenceSequence};
use noodles::sam::header::record::value::Map;
use noodles::vcf::variant::io::Write as _;
use noodles::vcf::variant::record::Ids as _;
use noodles::{bam, bgzf, sam, tabix, vcf};
use std::collections::{BTreeSet, HashSet};
use std::num::NonZeroUsize;
use std::sync::Arc;
use tower::ServiceExt;

const BASE: &str = "http://ferrum.test";
const REFS: [(&str, usize); 2] = [("chr1", 5_000_000), ("chr2", 3_000_000)];

struct Fixture {
    app: axum::Router,
    repo: Arc<ferrum_drs::repo::DrsRepo>,
    storage: Arc<LocalStorage>,
    _tmp: tempfile::TempDir,
    key_dir: std::path::PathBuf,
}

async fn fixture() -> Fixture {
    std::env::set_var("FERRUM_AUTH__REQUIRE_AUTH", "false");
    let pool = sqlx::sqlite::SqlitePoolOptions::new()
        .max_connections(1)
        .connect("sqlite::memory:")
        .await
        .expect("pool");
    sqlx::migrate!("../ferrum-embed/migrations")
        .run(&pool)
        .await
        .expect("migrate");
    let fp = FerrumPool::Sqlite(pool);
    let repo = Arc::new(ferrum_drs::repo::DrsRepo::new(fp, "ferrum.test".into()));
    let tmp = tempfile::tempdir().unwrap();
    let storage = Arc::new(LocalStorage::new(tmp.path().join("objects")).expect("storage"));
    let key_dir = tmp.path().join("keys");
    std::fs::create_dir_all(&key_dir).unwrap();
    ferrum_crypt4gh::generate_keypair(&key_dir.join("node.sec"), &key_dir.join("node.pub"), None)
        .expect("node keypair");

    let drs = AppState {
        repo: repo.clone(),
        storage: Some(storage.clone()),
        s3_presigner: None,
        provenance_store: None,
        crypt4gh_key_dir: Some(key_dir.clone()),
        crypt4gh_master_key_id: "node".into(),
        crypt4gh_decrypt_stream: true,
        ingest: Default::default(),
        object_storage_backend: "local".into(),
        outbreak: None,
        bandwidth: None,
        transfer_queue: None,
        residency_audit: None,
        denied_audit: None,
        background_gate: None,
        ads_introspect: None,
        solum_consent: None,
        ingest_require_auth: false,
        require_passport_for_bytes: false,
        metadata_store_enabled: false,
        pipeline: ferrum_core::PipelineConfig::default(),
    };
    let htsget = Arc::new(HtsgetState {
        repo: repo.clone(),
        public_base_url: BASE.into(),
        storage: Some(storage.clone()),
        crypt4gh_key_dir: Some(key_dir.clone()),
        crypt4gh_master_key_id: "node".into(),
    });
    let app = axum::Router::new()
        .route(
            "/ga4gh/drs/v1/objects/:object_id/stream",
            axum::routing::get(get_object_stream),
        )
        .with_state(Arc::new(drs))
        .nest("/ga4gh/htsget/v1", ferrum_htsget::router(htsget));
    Fixture {
        app,
        repo,
        storage,
        _tmp: tmp,
        key_dir,
    }
}

async fn register(f: &Fixture, id: &str, name: &str, mime: &str, bytes: &[u8], encrypted: bool) {
    let key = format!("drs/{id}");
    f.storage.put_bytes(&key, bytes).await.unwrap();
    f.repo
        .create_object_with_id(
            &ferrum_drs::types::CreateObjectRequest {
                name: Some(name.into()),
                description: None,
                mime_type: Some(mime.into()),
                size: bytes.len() as i64,
                checksums: vec![],
                aliases: None,
                storage_backend: "local".into(),
                storage_key: key,
                is_encrypted: Some(encrypted),
                workspace_id: None,
                ont_metrics: None,
                gisaid_metadata: None,
                metadata_ref: None,
            },
            Some(id.into()),
        )
        .await
        .unwrap();
}

/// How records sit in BGZF blocks.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum Layout {
    /// Like htslib BAM and bgzip VCF: the header ends its own block and no record spans two
    /// blocks. Block-aligned byte ranges are then valid files (what Crypt4GH tickets rely on).
    Htslib,
    /// Blocks are filled to the limit, so records span blocks (like htslib BCF, or BAM/VCF with
    /// records longer than a block), and the header shares its last block with the first
    /// records. Plain tickets must cut blocks at record boundaries.
    Spanning,
}

/// (reference index, 1-based start, 1-based end, name) of every record written.
type Expect = Vec<(usize, usize, usize, String)>;

fn sam_header() -> sam::Header {
    let mut b = sam::Header::builder().set_header(
        Map::<HeaderMap>::builder()
            .insert(header_tag::SORT_ORDER, sort_order::COORDINATE)
            .build()
            .unwrap(),
    );
    for (name, len) in REFS {
        b = b.add_reference_sequence(
            name,
            Map::<ReferenceSequence>::new(NonZeroUsize::new(len).unwrap()),
        );
    }
    b.build()
}

/// Deterministic, poorly compressible read content so files span many BGZF blocks and
/// Crypt4GH segments, like real data.
fn lcg(seed: u64) -> impl Iterator<Item = u64> {
    let mut x = seed
        .wrapping_mul(6364136223846793005)
        .wrapping_add(1442695040888963407);
    std::iter::from_fn(move || {
        x = x
            .wrapping_mul(6364136223846793005)
            .wrapping_add(1442695040888963407);
        Some(x >> 33)
    })
}

fn pseudo_bases(ref_id: usize, i: usize) -> Vec<u8> {
    lcg((ref_id as u64) << 32 | i as u64)
        .take(100)
        .map(|v| b"ACGT"[(v % 4) as usize])
        .collect()
}

fn pseudo_quals(ref_id: usize, i: usize) -> Vec<u8> {
    lcg(((ref_id as u64) << 32 | i as u64) ^ 0x5eed)
        .take(100)
        .map(|v| 2 + (v % 40) as u8)
        .collect()
}

/// A coordinate-sorted BAM with `per_ref` 100-bp reads per reference, every `step` bp, and its BAI.
/// See [`Layout`] for how records sit in BGZF blocks.
fn make_bam(per_ref: usize, step: usize, layout: Layout) -> (Vec<u8>, Vec<u8>, Expect) {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("t.bam");
    let header = sam_header();
    let mut expect = Vec::new();
    {
        let mut w = bam::io::Writer::new(std::fs::File::create(&path).unwrap());
        w.write_header(&header).unwrap();
        if layout == Layout::Htslib {
            std::io::Write::flush(w.get_mut()).unwrap();
        }
        for (ref_id, _) in REFS.iter().enumerate() {
            for i in 0..per_ref {
                let start = 1 + i * step;
                let name = format!("r{ref_id}_{i}");
                let rec = RecordBuf::builder()
                    .set_name(name.as_bytes())
                    .set_flags(Flags::empty())
                    .set_reference_sequence_id(ref_id)
                    .set_alignment_start(Position::try_from(start).unwrap())
                    .set_cigar([Op::new(Kind::Match, 100)].into_iter().collect())
                    .set_sequence(Sequence::from(pseudo_bases(ref_id, i)))
                    .set_quality_scores(QualityScores::from(pseudo_quals(ref_id, i)))
                    .build();
                w.write_alignment_record(&header, &rec).unwrap();
                expect.push((ref_id, start, start + 99, name));
                if layout == Layout::Htslib && i % 100 == 99 {
                    std::io::Write::flush(w.get_mut()).unwrap();
                }
            }
        }
        w.try_finish().unwrap();
    }
    let index = bam::fs::index(&path).unwrap();
    let bai_path = dir.path().join("t.bam.bai");
    bam::bai::fs::write(&bai_path, &index).unwrap();
    (
        std::fs::read(&path).unwrap(),
        std::fs::read(&bai_path).unwrap(),
        expect,
    )
}

/// bgzipped VCF with `per_ref` SNVs per contig every `step` bp, and its TBI.
fn make_vcf(per_ref: usize, step: usize, layout: Layout) -> (Vec<u8>, Vec<u8>, Expect) {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("t.vcf.gz");
    let mut hb = vcf::Header::builder();
    for (name, len) in REFS {
        hb = hb.add_contig(
            name,
            vcf::header::record::value::Map::<vcf::header::record::value::map::Contig>::builder()
                .set_length(len)
                .build()
                .unwrap(),
        );
    }
    let header = hb.build();
    let mut expect = Vec::new();
    {
        let mut w =
            vcf::io::Writer::new(bgzf::io::Writer::new(std::fs::File::create(&path).unwrap()));
        w.write_header(&header).unwrap();
        if layout == Layout::Htslib {
            std::io::Write::flush(w.get_mut()).unwrap();
        }
        for (ref_id, (name, _)) in REFS.iter().enumerate() {
            for i in 0..per_ref {
                let pos = 1 + i * step;
                let id = format!("v{ref_id}_{i}");
                let rec = vcf::variant::RecordBuf::builder()
                    .set_reference_sequence_name(*name)
                    .set_variant_start(Position::try_from(pos).unwrap())
                    .set_ids([id.clone()].into_iter().collect())
                    .set_reference_bases("A")
                    .set_alternate_bases(vec![String::from("C")].into())
                    .build();
                w.write_variant_record(&header, &rec).unwrap();
                expect.push((ref_id, pos, pos, id));
                if layout == Layout::Htslib && i % 500 == 499 {
                    std::io::Write::flush(w.get_mut()).unwrap();
                }
            }
        }
        w.into_inner().finish().unwrap();
    }
    let index = vcf::fs::index(&path).unwrap();
    let tbi = dir.path().join("t.vcf.gz.tbi");
    tabix::fs::write(&tbi, &index).unwrap();
    (
        std::fs::read(&path).unwrap(),
        std::fs::read(&tbi).unwrap(),
        expect,
    )
}

async fn send(app: &axum::Router, req: Request<axum::body::Body>) -> (StatusCode, Vec<u8>) {
    let resp = app.clone().oneshot(req).await.unwrap();
    let status = resp.status();
    let body = axum::body::to_bytes(resp.into_body(), usize::MAX)
        .await
        .unwrap()
        .to_vec();
    (status, body)
}

async fn get_ticket(
    app: &axum::Router,
    path: &str,
    headers: &[(&str, &str)],
) -> (StatusCode, serde_json::Value) {
    let mut req = Request::builder().method(Method::GET).uri(path);
    for (k, v) in headers {
        req = req.header(*k, *v);
    }
    let (status, body) = send(app, req.body(axum::body::Body::empty()).unwrap()).await;
    (
        status,
        serde_json::from_slice(&body).unwrap_or(serde_json::Value::Null),
    )
}

async fn post_ticket(
    app: &axum::Router,
    path: &str,
    body: serde_json::Value,
) -> (StatusCode, serde_json::Value) {
    let req = Request::builder()
        .method(Method::POST)
        .uri(path)
        .header("content-type", "application/json")
        .body(axum::body::Body::from(body.to_string()))
        .unwrap();
    let (status, body) = send(app, req).await;
    (
        status,
        serde_json::from_slice(&body).unwrap_or(serde_json::Value::Null),
    )
}

/// Fetch every ticket URL like an htsget client and concatenate the bytes.
async fn fetch(app: &axum::Router, ticket: &serde_json::Value) -> (Vec<u8>, usize) {
    let urls = ticket["htsget"]["urls"].as_array().expect("urls");
    let mut out = Vec::new();
    for u in urls {
        let url = u["url"].as_str().unwrap();
        if let Some(data) = url.strip_prefix("data:") {
            let (_, b64) = data.split_once(";base64,").expect("base64 data url");
            out.extend(
                base64::engine::general_purpose::STANDARD
                    .decode(b64)
                    .unwrap(),
            );
            continue;
        }
        let path = url.strip_prefix(BASE).expect("ticket url on this node");
        let mut req = Request::builder().method(Method::GET).uri(path);
        if let Some(h) = u["headers"].as_object() {
            for (k, v) in h {
                req = req.header(k.as_str(), v.as_str().unwrap());
            }
        }
        let (status, body) = send(app, req.body(axum::body::Body::empty()).unwrap()).await;
        assert!(
            status == StatusCode::OK || status == StatusCode::PARTIAL_CONTENT,
            "fetch {url}: {status}"
        );
        out.extend(body);
    }
    (out, urls.len())
}

fn bam_names(bytes: &[u8]) -> Vec<String> {
    let mut r = bam::io::Reader::new(std::io::Cursor::new(bytes));
    r.read_header().expect("BAM header");
    r.records()
        .map(|rec| String::from_utf8(rec.expect("BAM record").name().unwrap().to_vec()).unwrap())
        .collect()
}

fn vcf_ids(bytes: &[u8]) -> Vec<String> {
    let mut r = vcf::io::Reader::new(bgzf::io::Reader::new(std::io::Cursor::new(bytes)));
    let header = r.read_header().expect("VCF header");
    let mut ids = Vec::new();
    for rec in r.records() {
        let rec = rec.expect("VCF record");
        for id in rec.ids().iter() {
            ids.push(id.to_string());
        }
    }
    let _ = header;
    ids
}

/// Names whose interval overlaps `[start, end)` (0-based half-open, htsget) on `ref_id`.
fn expected(expect: &Expect, ref_id: usize, start: usize, end: usize) -> BTreeSet<String> {
    expect
        .iter()
        .filter(|(r, s, e, _)| *r == ref_id && *s <= end && *e > start)
        .map(|(_, _, _, n)| n.clone())
        .collect()
}

/// Cross-check `expected` against noodles' indexed BAM query on the original file.
fn indexed_bam_names(
    bam_bytes: &[u8],
    bai: &[u8],
    reference: &str,
    start: usize,
    end: usize,
) -> BTreeSet<String> {
    let index = bam::bai::io::Reader::new(std::io::Cursor::new(bai))
        .read_index()
        .unwrap();
    let mut r = bam::io::Reader::new(std::io::Cursor::new(bam_bytes));
    let header = r.read_header().unwrap();
    let region: noodles::core::Region = format!("{reference}:{}-{}", start + 1, end)
        .parse()
        .unwrap();
    r.query(&header, &index, &region)
        .unwrap()
        .records()
        .map(|rec| String::from_utf8(rec.unwrap().name().unwrap().to_vec()).unwrap())
        .collect()
}

#[tokio::test]
async fn bam_region_returns_every_record_in_the_region_and_little_else() {
    let f = fixture().await;
    let (bam_bytes, bai, expect) = make_bam(20_000, 200, Layout::Htslib);
    register(
        &f,
        "bam1",
        "sample.bam",
        "application/vnd.ga4gh.bam",
        &bam_bytes,
        false,
    )
    .await;
    register(
        &f,
        "bai1",
        "sample.bam.bai",
        "application/octet-stream",
        &bai,
        false,
    )
    .await;
    let total = expect.len();

    for (reference, ref_id, start, end) in [
        ("chr1", 0usize, 1_000_000usize, 1_010_000usize),
        ("chr2", 1, 0, 5_000),
        ("chr1", 0, 3_999_000, 5_000_000),
    ] {
        let (status, ticket) = get_ticket(
            &f.app,
            &format!(
                "/ga4gh/htsget/v1/reads/bam1?referenceName={reference}&start={start}&end={end}"
            ),
            &[],
        )
        .await;
        assert_eq!(status, StatusCode::OK, "{ticket}");
        assert_eq!(ticket["htsget"]["format"], "BAM");
        let (bytes, _) = fetch(&f.app, &ticket).await;
        let got: Vec<String> = bam_names(&bytes);
        let got_set: BTreeSet<String> = got.iter().cloned().collect();
        assert_eq!(got.len(), got_set.len(), "no duplicate records");
        let want = expected(&expect, ref_id, start, end);
        assert_eq!(
            want,
            indexed_bam_names(&bam_bytes, &bai, reference, start, end),
            "oracle agrees with noodles' indexed query"
        );
        assert!(!want.is_empty());
        let missing: Vec<_> = want.difference(&got_set).collect();
        assert!(
            missing.is_empty(),
            "{reference}:{start}-{end} missing {missing:?}"
        );
        assert!(
            got.len() < total / 4,
            "small region returned {} of {total} records",
            got.len()
        );
        assert!(bytes.len() < bam_bytes.len() / 2);
    }
}

#[tokio::test]
async fn bam_whole_reference_and_header_only() {
    let f = fixture().await;
    let (bam_bytes, bai, expect) = make_bam(5_000, 500, Layout::Htslib);
    register(
        &f,
        "bam2",
        "s2.bam",
        "application/vnd.ga4gh.bam",
        &bam_bytes,
        false,
    )
    .await;
    register(
        &f,
        "bai2",
        "s2.bam.bai",
        "application/octet-stream",
        &bai,
        false,
    )
    .await;

    let (status, ticket) = get_ticket(
        &f.app,
        "/ga4gh/htsget/v1/reads/bam2?referenceName=chr2",
        &[],
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{ticket}");
    let got: HashSet<String> = bam_names(&fetch(&f.app, &ticket).await.0)
        .into_iter()
        .collect();
    let want: HashSet<String> = expect
        .iter()
        .filter(|e| e.0 == 1)
        .map(|e| e.3.clone())
        .collect();
    assert!(want.is_subset(&got));
    assert!(
        got.iter().all(|n| n.starts_with("r1_")),
        "only chr2 records"
    );

    let (status, ticket) =
        get_ticket(&f.app, "/ga4gh/htsget/v1/reads/bam2?class=header", &[]).await;
    assert_eq!(status, StatusCode::OK, "{ticket}");
    let (bytes, _) = fetch(&f.app, &ticket).await;
    let mut r = bam::io::Reader::new(std::io::Cursor::new(&bytes[..]));
    let header = r.read_header().expect("header parses");
    assert_eq!(header.reference_sequences().len(), 2);
    assert_eq!(r.records().count(), 0, "header ticket carries no records");
    assert!(bytes.len() < 1_000);
}

#[tokio::test]
async fn post_two_regions_merges_ranges_without_duplicates() {
    let f = fixture().await;
    let (bam_bytes, bai, expect) = make_bam(20_000, 200, Layout::Htslib);
    register(
        &f,
        "bam3",
        "s3.bam",
        "application/vnd.ga4gh.bam",
        &bam_bytes,
        false,
    )
    .await;
    register(
        &f,
        "bai3",
        "s3.bam.bai",
        "application/octet-stream",
        &bai,
        false,
    )
    .await;
    let (status, ticket) = post_ticket(
        &f.app,
        "/ga4gh/htsget/v1/reads/bam3",
        serde_json::json!({ "regions": [
            { "referenceName": "chr1", "start": 500000, "end": 501000 },
            { "referenceName": "chr1", "start": 500500, "end": 502000 },
            { "referenceName": "chr2", "start": 100000, "end": 100500 }
        ]}),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{ticket}");
    let got = bam_names(&fetch(&f.app, &ticket).await.0);
    let got_set: BTreeSet<String> = got.iter().cloned().collect();
    assert_eq!(
        got.len(),
        got_set.len(),
        "overlapping regions do not duplicate records"
    );
    for (ref_id, a, b) in [(0, 500_000, 502_000), (1, 100_000, 100_500)] {
        let want = expected(&expect, ref_id, a, b);
        assert!(
            want.is_subset(&got_set),
            "missing records for {ref_id}:{a}-{b}"
        );
    }
}

#[tokio::test]
async fn vcf_region_with_tabix() {
    let f = fixture().await;
    let (vcf_bytes, tbi, expect) = make_vcf(30_000, 100, Layout::Htslib);
    register(
        &f,
        "vcf1",
        "calls.vcf.gz",
        "application/vnd.ga4gh.vcf",
        &vcf_bytes,
        false,
    )
    .await;
    register(
        &f,
        "tbi1",
        "calls.vcf.gz.tbi",
        "application/octet-stream",
        &tbi,
        false,
    )
    .await;
    let (status, ticket) = get_ticket(
        &f.app,
        "/ga4gh/htsget/v1/variants/vcf1?referenceName=chr2&start=1500000&end=1502000",
        &[],
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{ticket}");
    assert_eq!(ticket["htsget"]["format"], "VCF");
    let (bytes, _) = fetch(&f.app, &ticket).await;
    let got: BTreeSet<String> = vcf_ids(&bytes).into_iter().collect();
    let want = expected(&expect, 1, 1_500_000, 1_502_000);
    assert_eq!(want.len(), 20);
    assert!(
        want.is_subset(&got),
        "missing {:?}",
        want.difference(&got).collect::<Vec<_>>()
    );
    assert!(got.len() < expect.len() / 4);
}

#[tokio::test]
async fn region_errors_are_htsget_errors() {
    let f = fixture().await;
    let (bam_bytes, bai, _) = make_bam(1_000, 1_000, Layout::Htslib);
    register(
        &f,
        "bam4",
        "s4.bam",
        "application/vnd.ga4gh.bam",
        &bam_bytes,
        false,
    )
    .await;
    // No index registered yet → NotFound with a hint.
    let (status, body) = get_ticket(
        &f.app,
        "/ga4gh/htsget/v1/reads/bam4?referenceName=chr1",
        &[],
    )
    .await;
    assert_eq!(status, StatusCode::NOT_FOUND);
    assert!(body["htsget"]["message"]
        .as_str()
        .unwrap()
        .contains("index"));
    register(
        &f,
        "bai4",
        "s4.bam.bai",
        "application/octet-stream",
        &bai,
        false,
    )
    .await;
    // Unknown reference.
    let (status, body) = get_ticket(
        &f.app,
        "/ga4gh/htsget/v1/reads/bam4?referenceName=chrX&start=0&end=10",
        &[],
    )
    .await;
    assert!(status.is_client_error(), "{status} {body}");
    // Whole-object tickets keep working without touching the index.
    let (status, ticket) = get_ticket(&f.app, "/ga4gh/htsget/v1/reads/bam4", &[]).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(ticket["htsget"]["urls"].as_array().unwrap().len(), 1);
}

#[tokio::test]
async fn crypt4gh_region_ticket_decrypts_with_the_client_key_only() {
    let f = fixture().await;
    let (bam_bytes, bai, expect) = make_bam(20_000, 200, Layout::Htslib);

    // Store the BAM encrypted for the node key; the index stays plain.
    let node_pub = crypt4gh::keys::get_public_key(f.key_dir.join("node.pub")).unwrap();
    let mut recipients = std::collections::HashSet::new();
    recipients.insert(ferrum_crypt4gh::recipient_keys_from_pubkey(&node_pub));
    let mut encrypted = Vec::new();
    crypt4gh::encrypt(
        &recipients,
        &mut std::io::Cursor::new(&bam_bytes),
        &mut encrypted,
        0,
        None,
    )
    .unwrap();
    register(
        &f,
        "c4gh1",
        "enc.bam",
        "application/vnd.ga4gh.bam",
        &encrypted,
        true,
    )
    .await;
    register(
        &f,
        "c4bai",
        "enc.bam.bai",
        "application/octet-stream",
        &bai,
        false,
    )
    .await;

    let path = "/ga4gh/htsget/v1/reads/c4gh1?referenceName=chr1&start=2000000&end=2004000";
    // Without a public key the request is refused (the ticket must be encrypted for someone).
    let (status, _) = get_ticket(&f.app, path, &[]).await;
    assert_eq!(status, StatusCode::BAD_REQUEST);

    let client = crypt4gh::keys::generate_private_key().unwrap();
    let (client_sk, client_pk) = client.split_at(32);
    let pk_b64 = base64::engine::general_purpose::STANDARD.encode(client_pk);
    let (status, ticket) = get_ticket(&f.app, path, &[("X-Crypt4GH-Public-Key", &pk_b64)]).await;
    assert_eq!(status, StatusCode::OK, "{ticket}");
    let (ciphertext, _) = fetch(&f.app, &ticket).await;
    assert!(
        ciphertext.starts_with(b"crypt4gh"),
        "client receives Crypt4GH"
    );

    let decrypt_with = |sk: &[u8]| {
        let keys = vec![crypt4gh::Keys {
            method: 0,
            privkey: sk.to_vec(),
            recipient_pubkey: vec![],
        }];
        let mut plain = Vec::new();
        crypt4gh::decrypt(
            &keys,
            &mut std::io::Cursor::new(&ciphertext),
            &mut plain,
            0,
            None,
            &None,
        )
        .map(|_| plain)
    };
    let plain = decrypt_with(client_sk).expect("client key decrypts the ticket");
    let got: BTreeSet<String> = bam_names(&plain).into_iter().collect();
    let want = expected(&expect, 0, 2_000_000, 2_004_000);
    assert!(!want.is_empty());
    assert!(
        want.is_subset(&got),
        "missing {:?}",
        want.difference(&got).collect::<Vec<_>>()
    );
    assert!(got.len() < expect.len() / 4);
    assert!(ciphertext.len() < encrypted.len() / 2);

    // Another key cannot read it.
    let other = crypt4gh::keys::generate_private_key().unwrap();
    assert!(decrypt_with(&other[..32]).is_err());
}

// ---------------------------------------------------------------------------------------------
// CRAM and BCF: fixtures written by htslib (pysam), see tests/data/make_fixtures.py. Their
// layout (CRAM containers with one slice each, BCF with CSI) is what samtools/bcftools produce.

const FIXTURE_REFS: [(&str, usize); 2] = [("chr1", 120_000), ("chr2", 60_000)];
const FIXTURE_READ_LEN: usize = 50;
const FIXTURE_READ_STEP: usize = 20;
const FIXTURE_VARIANT_STEP: usize = 40;

fn fixture_file(name: &str) -> Vec<u8> {
    std::fs::read(
        std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("tests/data")
            .join(name),
    )
    .unwrap_or_else(|e| panic!("fixture {name}: {e}"))
}

/// Same formula as `ref_base` in make_fixtures.py.
fn fixture_ref_base(ref_id: usize, i: usize) -> u8 {
    b"ACGT"[(i * 7 + i / 13 + ref_id * 3) % 4]
}

fn fixture_repository() -> noodles::fasta::Repository {
    use noodles::fasta;
    let records = FIXTURE_REFS
        .iter()
        .enumerate()
        .map(|(ref_id, (name, len))| {
            let seq: Vec<u8> = (0..*len).map(|i| fixture_ref_base(ref_id, i)).collect();
            fasta::Record::new(
                fasta::record::Definition::new(*name, None),
                fasta::record::Sequence::from(seq),
            )
        })
        .collect::<Vec<_>>();
    fasta::Repository::new(records)
}

/// Read names in a CRAM byte stream (decoded against the synthetic reference).
fn cram_names(bytes: &[u8]) -> Vec<String> {
    let mut r = noodles::cram::io::reader::Builder::default()
        .set_reference_sequence_repository(fixture_repository())
        .build_from_reader(std::io::Cursor::new(bytes));
    let header = r.read_header().expect("CRAM header");
    r.records(&header)
        .map(|rec| {
            let rec = rec.expect("CRAM record");
            String::from_utf8(rec.name().expect("read name").to_vec()).unwrap()
        })
        .collect()
}

/// Every read the fixture generator wrote: (ref, 0-based start, 0-based exclusive end, name).
fn fixture_reads() -> Vec<(usize, usize, usize, String)> {
    let mut out = Vec::new();
    for (ref_id, (_, len)) in FIXTURE_REFS.iter().enumerate() {
        for (n, pos) in (0..len - FIXTURE_READ_LEN)
            .step_by(FIXTURE_READ_STEP)
            .enumerate()
        {
            out.push((
                ref_id,
                pos,
                pos + FIXTURE_READ_LEN,
                format!("c{ref_id}_{n}"),
            ));
        }
    }
    out
}

#[tokio::test]
async fn cram_region_with_crai() {
    let f = fixture().await;
    let cram = fixture_file("sample.cram");
    let crai = fixture_file("sample.cram.crai");
    register(
        &f,
        "cram1",
        "sample.cram",
        "application/vnd.ga4gh.cram",
        &cram,
        false,
    )
    .await;
    register(
        &f,
        "crai1",
        "sample.cram.crai",
        "application/octet-stream",
        &crai,
        false,
    )
    .await;
    let all = fixture_reads();
    assert_eq!(
        cram_names(&cram).len(),
        all.len(),
        "fixture decodes completely (reference formula matches the generator)"
    );

    for (reference, ref_id, start, end) in [
        ("chr1", 0usize, 50_000usize, 52_000usize),
        ("chr2", 1, 0, 1_000),
        ("chr2", 1, 59_000, 60_000),
    ] {
        let (status, ticket) = get_ticket(
            &f.app,
            &format!(
                "/ga4gh/htsget/v1/reads/cram1?format=CRAM&referenceName={reference}&start={start}&end={end}"
            ),
            &[],
        )
        .await;
        assert_eq!(status, StatusCode::OK, "{ticket}");
        assert_eq!(ticket["htsget"]["format"], "CRAM");
        let (bytes, _) = fetch(&f.app, &ticket).await;
        let got = cram_names(&bytes);
        let got_set: BTreeSet<String> = got.iter().cloned().collect();
        assert_eq!(got.len(), got_set.len(), "no duplicate records");
        let want: BTreeSet<String> = all
            .iter()
            .filter(|(r, s, e, _)| *r == ref_id && *s < end && *e > start)
            .map(|(_, _, _, n)| n.clone())
            .collect();
        assert!(!want.is_empty());
        let missing: Vec<_> = want.difference(&got_set).collect();
        assert!(
            missing.is_empty(),
            "{reference}:{start}-{end} missing {missing:?}"
        );
        assert!(got.len() < all.len() / 4, "{} of {}", got.len(), all.len());
        assert!(bytes.len() < cram.len() / 2);
    }

    let (status, ticket) = get_ticket(
        &f.app,
        "/ga4gh/htsget/v1/reads/cram1?format=CRAM&class=header",
        &[],
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{ticket}");
    let bytes = fetch(&f.app, &ticket).await.0;
    assert!(cram_names(&bytes).is_empty(), "header ticket has no reads");
}

#[tokio::test]
async fn bcf_region_with_csi() {
    let f = fixture().await;
    let bcf_bytes = fixture_file("calls.bcf");
    let csi = fixture_file("calls.bcf.csi");
    register(
        &f,
        "bcf1",
        "calls.bcf",
        "application/vnd.ga4gh.bcf",
        &bcf_bytes,
        false,
    )
    .await;
    register(
        &f,
        "csi1",
        "calls.bcf.csi",
        "application/octet-stream",
        &csi,
        false,
    )
    .await;

    let ids = |bytes: &[u8]| -> Vec<String> {
        let mut r = noodles::bcf::io::Reader::new(std::io::Cursor::new(bytes));
        let header = r.read_header().expect("BCF header");
        r.record_bufs(&header)
            .map(|rec| {
                rec.expect("BCF record")
                    .ids()
                    .as_ref()
                    .iter()
                    .next()
                    .expect("id")
                    .to_string()
            })
            .collect()
    };
    let total = ids(&bcf_bytes).len();
    let (status, ticket) = get_ticket(
        &f.app,
        "/ga4gh/htsget/v1/variants/bcf1?format=BCF&referenceName=chr1&start=80000&end=81000",
        &[],
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{ticket}");
    assert_eq!(ticket["htsget"]["format"], "BCF");
    let got: Vec<String> = ids(&fetch(&f.app, &ticket).await.0);
    let got_set: BTreeSet<String> = got.iter().cloned().collect();
    assert_eq!(got.len(), got_set.len());
    // Variants sit at 0-based 10, 50, 90, … (make_fixtures.py); 80_000..81_000 holds 25 of them.
    let want: BTreeSet<String> = (10..FIXTURE_REFS[0].1)
        .step_by(FIXTURE_VARIANT_STEP)
        .enumerate()
        .filter(|(_, pos)| (80_000..81_000).contains(pos))
        .map(|(n, _)| format!("v0_{n}"))
        .collect();
    assert_eq!(want.len(), 25);
    assert!(
        want.is_subset(&got_set),
        "missing {:?}",
        want.difference(&got_set).collect::<Vec<_>>()
    );
    assert!(got.len() < total / 2, "{} of {total}", got.len());
}

/// Records spanning BGZF blocks: every region must still parse, with every record in it, for
/// BAM and VCF (the BCF fixture above spans blocks too, as htslib writes it).
#[tokio::test]
async fn plain_tickets_cut_blocks_at_record_boundaries() {
    let f = fixture().await;
    let (bam_bytes, bai, bam_expect) = make_bam(20_000, 200, Layout::Spanning);
    register(
        &f,
        "sb",
        "span.bam",
        "application/vnd.ga4gh.bam",
        &bam_bytes,
        false,
    )
    .await;
    register(
        &f,
        "sbi",
        "span.bam.bai",
        "application/octet-stream",
        &bai,
        false,
    )
    .await;
    let (vcf_bytes, tbi, vcf_expect) = make_vcf(30_000, 100, Layout::Spanning);
    register(
        &f,
        "sv",
        "span.vcf.gz",
        "application/vnd.ga4gh.vcf",
        &vcf_bytes,
        false,
    )
    .await;
    register(
        &f,
        "svi",
        "span.vcf.gz.tbi",
        "application/octet-stream",
        &tbi,
        false,
    )
    .await;

    // Several regions, including ones that start and end inside blocks and one whole reference.
    for (reference, ref_id, start, end) in [
        ("chr1", 0usize, 0usize, 1_000usize),
        ("chr1", 0, 1_000_000, 1_010_000),
        ("chr1", 0, 1_234_567, 1_234_890),
        ("chr2", 1, 2_000_000, 2_050_000),
    ] {
        let (status, ticket) = get_ticket(
            &f.app,
            &format!("/ga4gh/htsget/v1/reads/sb?referenceName={reference}&start={start}&end={end}"),
            &[],
        )
        .await;
        assert_eq!(status, StatusCode::OK, "{ticket}");
        let got = bam_names(&fetch(&f.app, &ticket).await.0);
        let got_set: BTreeSet<String> = got.iter().cloned().collect();
        assert_eq!(got.len(), got_set.len(), "no duplicates");
        let want = expected(&bam_expect, ref_id, start, end);
        assert!(!want.is_empty());
        assert!(
            want.is_subset(&got_set),
            "BAM {reference}:{start}-{end} missing {:?}",
            want.difference(&got_set).collect::<Vec<_>>()
        );
        assert!(got.len() < bam_expect.len() / 4);
    }

    for (reference, ref_id, start, end) in [
        ("chr1", 0usize, 0usize, 2_000usize),
        ("chr1", 0, 777_777, 790_001),
        ("chr2", 1, 0, 3_000),
    ] {
        let (status, ticket) = get_ticket(
            &f.app,
            &format!(
                "/ga4gh/htsget/v1/variants/sv?referenceName={reference}&start={start}&end={end}"
            ),
            &[],
        )
        .await;
        assert_eq!(status, StatusCode::OK, "{ticket}");
        let got: BTreeSet<String> = vcf_ids(&fetch(&f.app, &ticket).await.0)
            .into_iter()
            .collect();
        let want = expected(&vcf_expect, ref_id, start, end);
        assert!(!want.is_empty());
        assert!(
            want.is_subset(&got),
            "VCF {reference}:{start}-{end} missing {:?}",
            want.difference(&got).collect::<Vec<_>>()
        );
        assert!(got.len() < vcf_expect.len() / 4);
    }

    let whole = get_ticket(&f.app, "/ga4gh/htsget/v1/reads/sb?referenceName=chr2", &[]).await;
    assert_eq!(whole.0, StatusCode::OK);
    let got: BTreeSet<String> = bam_names(&fetch(&f.app, &whole.1).await.0)
        .into_iter()
        .collect();
    let want: BTreeSet<String> = bam_expect
        .iter()
        .filter(|e| e.0 == 1)
        .map(|e| e.3.clone())
        .collect();
    assert_eq!(got, want, "whole chr2, nothing from chr1");

    // Header tickets: the header shares a block with records, so it must be cut there.
    let (status, ticket) = get_ticket(&f.app, "/ga4gh/htsget/v1/reads/sb?class=header", &[]).await;
    assert_eq!(status, StatusCode::OK, "{ticket}");
    let bytes = fetch(&f.app, &ticket).await.0;
    let mut r = bam::io::Reader::new(std::io::Cursor::new(&bytes[..]));
    assert_eq!(
        r.read_header().expect("header").reference_sequences().len(),
        2
    );
    assert_eq!(r.records().count(), 0, "header ticket carries no records");
    let (status, ticket) =
        get_ticket(&f.app, "/ga4gh/htsget/v1/variants/sv?class=header", &[]).await;
    assert_eq!(status, StatusCode::OK, "{ticket}");
    assert!(vcf_ids(&fetch(&f.app, &ticket).await.0).is_empty());
}

#[tokio::test]
async fn crypt4gh_bcf_regions_are_refused_not_served_broken() {
    let f = fixture().await;
    let bcf_bytes = fixture_file("calls.bcf");
    let node_pub = crypt4gh::keys::get_public_key(f.key_dir.join("node.pub")).unwrap();
    let mut recipients = std::collections::HashSet::new();
    recipients.insert(ferrum_crypt4gh::recipient_keys_from_pubkey(&node_pub));
    let mut encrypted = Vec::new();
    crypt4gh::encrypt(
        &recipients,
        &mut std::io::Cursor::new(&bcf_bytes),
        &mut encrypted,
        0,
        None,
    )
    .unwrap();
    register(
        &f,
        "ebcf",
        "enc.bcf",
        "application/vnd.ga4gh.bcf",
        &encrypted,
        true,
    )
    .await;
    let csi = fixture_file("calls.bcf.csi");
    register(
        &f,
        "ecsi",
        "enc.bcf.csi",
        "application/octet-stream",
        &csi,
        false,
    )
    .await;
    let client = crypt4gh::keys::generate_private_key().unwrap();
    let pk_b64 = base64::engine::general_purpose::STANDARD.encode(&client[32..]);
    let (status, body) = get_ticket(
        &f.app,
        "/ga4gh/htsget/v1/variants/ebcf?format=BCF&referenceName=chr1&start=1000&end=2000",
        &[("X-Crypt4GH-Public-Key", &pk_b64)],
    )
    .await;
    assert_eq!(status, StatusCode::BAD_REQUEST, "{body}");
    assert_eq!(body["htsget"]["error"], "UnsupportedFormat");
    // The whole object is still available.
    let (status, _) = get_ticket(&f.app, "/ga4gh/htsget/v1/variants/ebcf?format=BCF", &[]).await;
    assert_eq!(status, StatusCode::OK);
}
