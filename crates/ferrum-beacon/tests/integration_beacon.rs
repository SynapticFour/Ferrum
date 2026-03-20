use ferrum_beacon::router;
use http::{Method, Request, StatusCode};
use serde::Deserialize;
use sqlx::PgPool;
use tower::ServiceExt;

fn fixtures_dir() -> std::path::PathBuf {
    std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("tests")
        .join("fixtures")
}

#[derive(Debug, Deserialize)]
struct DatasetFixture {
    id: String,
    name: String,
    description: String,
    assembly_id: String,
}

#[derive(Debug, Deserialize)]
struct VariantFixture {
    dataset_id: String,
    chromosome: String,
    start: i64,
    end: i64,
    reference: String,
    alternate: String,
    variant_type: String,
}

async fn seed_fixtures(pool: &PgPool) {
    // Load JSON fixtures from repo.
    let datasets: Vec<DatasetFixture> = {
        let p = fixtures_dir().join("datasets.json");
        let s = std::fs::read_to_string(&p).expect("read datasets.json");
        serde_json::from_str(&s).expect("parse datasets.json")
    };

    let variants: Vec<VariantFixture> = {
        let p = fixtures_dir().join("genomic_variations.json");
        let s = std::fs::read_to_string(&p).expect("read genomic_variations.json");
        serde_json::from_str(&s).expect("parse genomic_variations.json")
    };

    let dataset_ids: Vec<String> = datasets.iter().map(|d| d.id.clone()).collect();

    // Cleanup any pre-existing fixture data to keep the test idempotent.
    // (Using array binding keeps this safe and avoids building ad-hoc SQL strings.)
    sqlx::query("DELETE FROM beacon_variants WHERE dataset_id = ANY($1)")
        .bind(&dataset_ids)
        .execute(pool)
        .await
        .expect("delete fixture variants");

    sqlx::query("DELETE FROM beacon_datasets WHERE id = ANY($1)")
        .bind(&dataset_ids)
        .execute(pool)
        .await
        .expect("delete fixture datasets");

    for d in datasets {
        sqlx::query(
            "INSERT INTO beacon_datasets (id, name, description, assembly_id) VALUES ($1, $2, $3, $4)",
        )
        .bind(d.id)
        .bind(d.name)
        .bind(d.description)
        .bind(d.assembly_id)
        .execute(pool)
        .await
        .expect("insert fixture dataset");
    }

    for v in variants {
        sqlx::query(
            "INSERT INTO beacon_variants (dataset_id, chromosome, start, \"end\", reference, alternate, variant_type)
             VALUES ($1, $2, $3, $4, $5, $6, $7)",
        )
        .bind(v.dataset_id)
        .bind(v.chromosome)
        .bind(v.start)
        .bind(v.end)
        .bind(v.reference)
        .bind(v.alternate)
        .bind(v.variant_type)
        .execute(pool)
        .await
        .expect("insert fixture variant");
    }
}

fn beacon_variant_query_envelope(request_parameters: serde_json::Value) -> serde_json::Value {
    // Learned from HelixTest: Beacon v2 `/query` payload is wrapped.
    // HelixTest sends:
    // { "meta": { "apiVersion": "v2.0.0" }, "query": { "requestParameters": {...} } }
    serde_json::json!({
        "meta": { "apiVersion": "v2.0.0" },
        "query": { "requestParameters": request_parameters }
    })
}

async fn post_json(app: &axum::Router, uri: &str, body: serde_json::Value) -> (StatusCode, serde_json::Value) {
    let req = Request::builder()
        .method(Method::POST)
        .uri(uri)
        .header("content-type", "application/json")
        .body(axum::body::Body::from(body.to_string()))
        .expect("build request");

    let resp = app.clone().oneshot(req).await.expect("oneshot");
    let status = resp.status();
    let bytes = axum::body::to_bytes(resp.into_body(), 10 * 1024 * 1024)
        .await
        .expect("read response body");
    let json: serde_json::Value = serde_json::from_slice(&bytes).unwrap_or(serde_json::json!({}));
    (status, json)
}

async fn get_json(app: &axum::Router, uri: &str) -> (StatusCode, serde_json::Value) {
    let req = Request::builder()
        .method(Method::GET)
        .uri(uri)
        .body(axum::body::Body::empty())
        .expect("build request");

    let resp = app.clone().oneshot(req).await.expect("oneshot");
    let status = resp.status();
    let bytes = axum::body::to_bytes(resp.into_body(), 10 * 1024 * 1024)
        .await
        .expect("read response body");
    let json: serde_json::Value = serde_json::from_slice(&bytes).unwrap_or(serde_json::json!({}));
    (status, json)
}

#[tokio::test]
async fn beacon_variants_exists_positive_and_negative() {
    let database_url = match std::env::var("DATABASE_URL") {
        Ok(v) => v,
        Err(_) => return,
    };
    let pool = PgPool::connect(&database_url).await.expect("connect pool");

    seed_fixtures(&pool).await;

    // Build the real router (no mocking).
    let app = router(pool.clone());

    // Positive: referenceName=1, start=1000, referenceBases=A, alternateBases=T
    let positive_params = serde_json::json!({
        "assemblyId": "GRCh38",
        "referenceName": "1",
        "start": 1000,
        "referenceBases": "A",
        "alternateBases": "T"
    });
    let (status, json) = post_json(&app, "/query", beacon_variant_query_envelope(positive_params)).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(json.pointer("/response/exists").and_then(|x| x.as_bool()), Some(true));

    // Negative: referenceName=1, start=999999999, referenceBases=C, alternateBases=G
    let negative_params = serde_json::json!({
        "assemblyId": "GRCh38",
        "referenceName": "1",
        "start": 999999999,
        "referenceBases": "C",
        "alternateBases": "G"
    });
    let (status, json) = post_json(&app, "/query", beacon_variant_query_envelope(negative_params)).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(json.pointer("/response/exists").and_then(|x| x.as_bool()), Some(false));
}

#[tokio::test]
async fn beacon_rejects_invalid_inputs() {
    let database_url = match std::env::var("DATABASE_URL") {
        Ok(v) => v,
        Err(_) => return,
    };
    let pool = PgPool::connect(&database_url).await.expect("connect pool");

    seed_fixtures(&pool).await;

    let app = router(pool.clone());

    // invalid assemblyId -> 400
    let params = serde_json::json!({
        "assemblyId": "BAD_ASSEMBLY",
        "referenceName": "1",
        "start": 1000,
        "referenceBases": "A",
        "alternateBases": "T"
    });
    let (status, _) = post_json(&app, "/query", beacon_variant_query_envelope(params)).await;
    assert_eq!(status, StatusCode::BAD_REQUEST);

    // start > end -> 400
    let params = serde_json::json!({
        "assemblyId": "GRCh38",
        "referenceName": "1",
        "start": 2000,
        "end": 1000,
        "referenceBases": "A",
        "alternateBases": "T"
    });
    let (status, _) = post_json(&app, "/query", beacon_variant_query_envelope(params)).await;
    assert_eq!(status, StatusCode::BAD_REQUEST);

    // requestedGranularity=record -> 400 (Ferrum rejects records)
    let params = serde_json::json!({
        "assemblyId": "GRCh38",
        "referenceName": "1",
        "start": 1000,
        "referenceBases": "A",
        "alternateBases": "T",
        "requestedGranularity": "record"
    });
    let (status, _) = post_json(&app, "/query", beacon_variant_query_envelope(params)).await;
    assert_eq!(status, StatusCode::BAD_REQUEST);
}

#[tokio::test]
async fn beacon_variants_count_and_end_defaulting() {
    let database_url = match std::env::var("DATABASE_URL") {
        Ok(v) => v,
        Err(_) => return,
    };
    let pool = PgPool::connect(&database_url).await.expect("connect pool");
    seed_fixtures(&pool).await;
    let app = router(pool.clone());

    let params = serde_json::json!({
        "assemblyId": "GRCh38",
        "referenceName": "chr1",
        "start": 1000,
        "end": 1000,
        "referenceBases": "A",
        "alternateBases": "T",
        "requestedGranularity": "count"
    });
    let (status, json) = post_json(&app, "/query", beacon_variant_query_envelope(params)).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(json.pointer("/response/exists").and_then(|x| x.as_bool()), None);
    assert_eq!(json.pointer("/response/count").and_then(|x| x.as_i64()), Some(1));

    let params = serde_json::json!({
        "assemblyId": "GRCh38",
        "referenceName": "1",
        "start": 2000,
        "end": 2000,
        "referenceBases": "A",
        "alternateBases": "T",
        "requestedGranularity": "count"
    });
    let (status, json) = post_json(&app, "/query", beacon_variant_query_envelope(params)).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(json.pointer("/response/count").and_then(|x| x.as_i64()), Some(0));

    // end default: omit "end" -> treat end=start and still hit
    let params = serde_json::json!({
        "assemblyId": "GRCh38",
        "referenceName": "1",
        "start": 1000,
        "referenceBases": "A",
        "alternateBases": "T",
        "requestedGranularity": "boolean"
    });
    let (status, json) = post_json(&app, "/query", beacon_variant_query_envelope(params)).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(json.pointer("/response/exists").and_then(|x| x.as_bool()), Some(true));
}

#[tokio::test]
async fn beacon_sanitizes_and_rejects_record_granularity() {
    let database_url = match std::env::var("DATABASE_URL") {
        Ok(v) => v,
        Err(_) => return,
    };
    let pool = PgPool::connect(&database_url).await.expect("connect pool");
    seed_fixtures(&pool).await;
    let app = router(pool.clone());

    let params = serde_json::json!({
        "assemblyId": "GRCh38",
        "referenceName": "1$",
        "start": 1000,
        "referenceBases": "A",
        "alternateBases": "T"
    });
    let (status, _) = post_json(&app, "/query", beacon_variant_query_envelope(params)).await;
    assert_eq!(status, StatusCode::BAD_REQUEST);

    let params = serde_json::json!({
        "assemblyId": "GRCh38",
        "referenceName": "1",
        "start": 1000,
        "referenceBases": "A",
        "alternateBases": "T",
        "requestedGranularity": "record"
    });
    let (status, _) = post_json(&app, "/query", beacon_variant_query_envelope(params)).await;
    assert_eq!(status, StatusCode::BAD_REQUEST);
}

#[tokio::test]
async fn beacon_routes_and_shapes() {
    let database_url = match std::env::var("DATABASE_URL") {
        Ok(v) => v,
        Err(_) => return,
    };
    let pool = PgPool::connect(&database_url).await.expect("connect pool");
    seed_fixtures(&pool).await;
    let app = router(pool.clone());

    let params = serde_json::json!({
        "assemblyId": "GRCh38",
        "referenceName": "1",
        "start": 1000,
        "referenceBases": "A",
        "alternateBases": "T"
    });
    let (status, json) = post_json(&app, "/g_variants/query", beacon_variant_query_envelope(params)).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(json.pointer("/response/exists").and_then(|x| x.as_bool()), Some(true));

    let (status, json) = get_json(&app, "/info").await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(json.get("id").and_then(|v| v.as_str()), Some("ferrum-beacon"));

    let (status, _) = get_json(&app, "/service-info").await;
    assert_eq!(status, StatusCode::OK);

    let (status, json) = get_json(&app, "/map").await;
    assert_eq!(status, StatusCode::OK);
    assert!(json.get("entryTypes").is_some());

    let (status, json) = post_json(&app, "/individuals/query", serde_json::json!({})).await;
    assert_eq!(status, StatusCode::OK);
    assert!(json.get("response").and_then(|r| r.get("individuals")).is_some());

    let (status, json) = post_json(&app, "/biosamples/query", serde_json::json!({})).await;
    assert_eq!(status, StatusCode::OK);
    assert!(json.get("response").and_then(|r| r.get("biosamples")).is_some());
}

#[tokio::test]
async fn beacon_fixture_bulk_positive_and_negative_queries() {
    let database_url = match std::env::var("DATABASE_URL") {
        Ok(v) => v,
        Err(_) => return,
    };
    let pool = PgPool::connect(&database_url).await.expect("connect pool");
    seed_fixtures(&pool).await;
    let app = router(pool.clone());

    // Best-effort: validate that seeded fixtures cover multiple positive coords
    // and that missing coords report exists=false.
    //
    // Learned from HelixTest: we must speak the wrapped Beacon v2 `/query` payload.
    let reference_bases = "A";
    let alternate_bases = "T";
    let assembly_id = "GRCh38";

    for start in 1000i64..1020i64 {
        let params = serde_json::json!({
            "assemblyId": assembly_id,
            "referenceName": "1",
            "start": start,
            "referenceBases": reference_bases,
            "alternateBases": alternate_bases
        });
        let (status, json) =
            post_json(&app, "/query", beacon_variant_query_envelope(params)).await;
        assert_eq!(status, StatusCode::OK);
        assert_eq!(
            json.pointer("/response/exists").and_then(|x| x.as_bool()),
            Some(true),
            "expected exists=true for start={start}"
        );
    }

    for start in [2000i64, 2001, 2002, 999999998, 999999999] {
        let params = serde_json::json!({
            "assemblyId": assembly_id,
            "referenceName": "1",
            "start": start,
            "referenceBases": reference_bases,
            "alternateBases": alternate_bases
        });
        let (status, json) =
            post_json(&app, "/query", beacon_variant_query_envelope(params)).await;
        assert_eq!(status, StatusCode::OK);
        assert_eq!(
            json.pointer("/response/exists").and_then(|x| x.as_bool()),
            Some(false),
            "expected exists=false for start={start}"
        );
    }
}

#[tokio::test]
async fn beacon_integration_matrix_min_20_checks() {
    let database_url = match std::env::var("DATABASE_URL") {
        Ok(v) => v,
        Err(_) => return,
    };
    let pool = PgPool::connect(&database_url).await.expect("connect pool");
    seed_fixtures(&pool).await;
    let app = router(pool.clone());

    // 1) Known variant exists (chr1 + allele match).
    let (status, json) = post_json(
        &app,
        "/query",
        beacon_variant_query_envelope(serde_json::json!({
            "assemblyId": "GRCh38",
            "referenceName": "chr1",
            "start": 1000,
            "end": 1000,
            "referenceBases": "A",
            "alternateBases": "T"
        })),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(
        json.pointer("/response/exists").and_then(|x| x.as_bool()),
        Some(true)
    );

    // 2) Same coordinate but wrong allele -> exists=false.
    let (status, json) = post_json(
        &app,
        "/query",
        beacon_variant_query_envelope(serde_json::json!({
            "assemblyId": "GRCh38",
            "referenceName": "chr1",
            "start": 1000,
            "end": 1000,
            "referenceBases": "C",
            "alternateBases": "G"
        })),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(
        json.pointer("/response/exists").and_then(|x| x.as_bool()),
        Some(false)
    );

    // 3) end omitted -> end defaults to start.
    let (status, json) = post_json(
        &app,
        "/query",
        beacon_variant_query_envelope(serde_json::json!({
            "assemblyId": "GRCh38",
            "referenceName": "1",
            "start": 1000,
            "referenceBases": "A",
            "alternateBases": "T"
        })),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(
        json.pointer("/response/exists").and_then(|x| x.as_bool()),
        Some(true)
    );

    // 4) Coordinate-only match: omit referenceBases/alternateBases -> exists=true.
    let (status, json) = post_json(
        &app,
        "/query",
        beacon_variant_query_envelope(serde_json::json!({
            "assemblyId": "GRCh38",
            "referenceName": "1",
            "start": 1000
        })),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(
        json.pointer("/response/exists").and_then(|x| x.as_bool()),
        Some(true)
    );

    // 5) Coordinate-only miss: missing coordinate -> exists=false.
    let (status, json) = post_json(
        &app,
        "/query",
        beacon_variant_query_envelope(serde_json::json!({
            "assemblyId": "GRCh38",
            "referenceName": "1",
            "start": 999999999
        })),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(
        json.pointer("/response/exists").and_then(|x| x.as_bool()),
        Some(false)
    );

    // 6) Count granularity: missing coordinate -> count=0.
    let (status, json) = post_json(
        &app,
        "/query",
        beacon_variant_query_envelope(serde_json::json!({
            "assemblyId": "GRCh38",
            "referenceName": "1",
            "start": 999999999,
            "end": 999999999,
            "requestedGranularity": "count"
        })),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(json.pointer("/response/count").and_then(|x| x.as_i64()), Some(0));

    // 7) referenceBases injection -> 400.
    let (status, _) = post_json(
        &app,
        "/query",
        beacon_variant_query_envelope(serde_json::json!({
            "assemblyId": "GRCh38",
            "referenceName": "1",
            "start": 1000,
            "referenceBases": "A$",
            "alternateBases": "T"
        })),
    )
    .await;
    assert_eq!(status, StatusCode::BAD_REQUEST);

    // 8) alternateBases injection -> 400.
    let (status, _) = post_json(
        &app,
        "/query",
        beacon_variant_query_envelope(serde_json::json!({
            "assemblyId": "GRCh38",
            "referenceName": "1",
            "start": 1000,
            "referenceBases": "A",
            "alternateBases": "T;"
        })),
    )
    .await;
    assert_eq!(status, StatusCode::BAD_REQUEST);

    // 9) assemblyId injection -> 400.
    let (status, _) = post_json(
        &app,
        "/query",
        beacon_variant_query_envelope(serde_json::json!({
            "assemblyId": "GRCh38;DROP",
            "referenceName": "1",
            "start": 1000,
            "referenceBases": "A",
            "alternateBases": "T"
        })),
    )
    .await;
    assert_eq!(status, StatusCode::BAD_REQUEST);

    // 10) negative start -> 400.
    let (status, _) = post_json(
        &app,
        "/query",
        beacon_variant_query_envelope(serde_json::json!({
            "assemblyId": "GRCh38",
            "referenceName": "1",
            "start": -1,
            "end": 0,
            "referenceBases": "A",
            "alternateBases": "T"
        })),
    )
    .await;
    assert_eq!(status, StatusCode::BAD_REQUEST);

    // 11) end out of bounds -> 400.
    let (status, _) = post_json(
        &app,
        "/query",
        beacon_variant_query_envelope(serde_json::json!({
            "assemblyId": "GRCh38",
            "referenceName": "1",
            "start": 1000,
            "end": 4_000_000_000i64,
            "referenceBases": "A",
            "alternateBases": "T"
        })),
    )
    .await;
    assert_eq!(status, StatusCode::BAD_REQUEST);

    // 12) start > end -> 400.
    let (status, _) = post_json(
        &app,
        "/query",
        beacon_variant_query_envelope(serde_json::json!({
            "assemblyId": "GRCh38",
            "referenceName": "1",
            "start": 2000,
            "end": 1000,
            "referenceBases": "A",
            "alternateBases": "T"
        })),
    )
    .await;
    assert_eq!(status, StatusCode::BAD_REQUEST);

    // 13) invalid referenceName (injection chars) -> 400.
    let (status, _) = post_json(
        &app,
        "/query",
        beacon_variant_query_envelope(serde_json::json!({
            "assemblyId": "GRCh38",
            "referenceName": "1$",
            "start": 1000,
            "referenceBases": "A",
            "alternateBases": "T"
        })),
    )
    .await;
    assert_eq!(status, StatusCode::BAD_REQUEST);

    // 14) requestedGranularity=record -> 400.
    let (status, _) = post_json(
        &app,
        "/query",
        beacon_variant_query_envelope(serde_json::json!({
            "assemblyId": "GRCh38",
            "referenceName": "1",
            "start": 1000,
            "referenceBases": "A",
            "alternateBases": "T",
            "requestedGranularity": "record"
        })),
    )
    .await;
    assert_eq!(status, StatusCode::BAD_REQUEST);

    // 15) unknown assemblyId -> 400.
    let (status, _) = post_json(
        &app,
        "/query",
        beacon_variant_query_envelope(serde_json::json!({
            "assemblyId": "BAD_ASSEMBLY",
            "referenceName": "1",
            "start": 1000,
            "referenceBases": "A",
            "alternateBases": "T"
        })),
    )
    .await;
    assert_eq!(status, StatusCode::BAD_REQUEST);

    // 16) alias route works.
    let (status, json) = post_json(
        &app,
        "/g_variants/query",
        beacon_variant_query_envelope(serde_json::json!({
            "assemblyId": "GRCh38",
            "referenceName": "1",
            "start": 1000,
            "referenceBases": "A",
            "alternateBases": "T"
        })),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(
        json.pointer("/response/exists").and_then(|x| x.as_bool()),
        Some(true)
    );

    // 17) /info shape.
    let (status, json) = get_json(&app, "/info").await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(json.get("id").and_then(|v| v.as_str()), Some("ferrum-beacon"));

    // 18) /map includes entryTypes.
    let (status, json) = get_json(&app, "/map").await;
    assert_eq!(status, StatusCode::OK);
    assert!(json.get("entryTypes").is_some());

    // 19) /individuals/query returns individuals array.
    let (status, json) = post_json(&app, "/individuals/query", serde_json::json!({})).await;
    assert_eq!(status, StatusCode::OK);
    assert!(json.get("response").and_then(|r| r.get("individuals")).is_some());

    // 20) /biosamples/query returns biosamples array.
    let (status, json) = post_json(&app, "/biosamples/query", serde_json::json!({})).await;
    assert_eq!(status, StatusCode::OK);
    assert!(json.get("response").and_then(|r| r.get("biosamples")).is_some());
}


