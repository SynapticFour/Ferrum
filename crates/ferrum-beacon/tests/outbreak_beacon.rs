// SPDX-License-Identifier: BUSL-1.1
use ferrum_beacon::{repo::BeaconRepo, router_with_outbreak};
use ferrum_core::FerrumPool;
use ferrum_core::{
    auth::AuthClaims, auth::PassportClaims, ActivateRequest, OutbreakConfig, OutbreakPolicy,
    OutbreakService,
};
use http::{Method, Request, StatusCode};
use std::sync::Arc;
use tower::ServiceExt;

async fn outbreak_beacon_app() -> axum::Router {
    let pool = sqlx::sqlite::SqlitePoolOptions::new()
        .connect("sqlite::memory:")
        .await
        .expect("pool");
    sqlx::migrate!("../ferrum-embed/migrations")
        .run(&pool)
        .await
        .expect("migrate");
    let fp = FerrumPool::Sqlite(pool.clone());
    let repo = BeaconRepo::new(fp.clone());
    repo.insert_pathogen_annotation("mpox1", "Monkeypox_virus", &[], None, None, None, None)
        .await
        .expect("insert");

    let outbreak = Arc::new(OutbreakService::new(
        fp.clone(),
        OutbreakConfig {
            enabled: true,
            policies: vec![OutbreakPolicy {
                name: "mpox_who_emergency".into(),
                trigger_pathogen: "Monkeypox_virus".into(),
                emergency_recipients: vec!["who.int".into()],
                access_level: "beacon_only".into(),
                gisaid_auto_package: true,
            }],
        },
    ));
    outbreak
        .activate(&ActivateRequest {
            policy: "mpox_who_emergency".into(),
            activated_by: "ops@lab.org".into(),
        })
        .await
        .expect("activate");

    router_with_outbreak(fp, Some(outbreak))
}

fn who_passport_claims() -> AuthClaims {
    AuthClaims::Passport {
        raw_token: None,
        claims: PassportClaims {
            sub: Some("who-user".into()),
            iss: Some("who.int".into()),
            exp: None,
            iat: None,
            jti: None,
            ga4gh_passport_v1: None,
            scope: None,
            aud: None,
        },
        visas: vec![],
    }
}

#[tokio::test]
async fn test_outbreak_emergency_recipient_beacon_http() {
    let app = outbreak_beacon_app().await;
    let body = serde_json::json!({
        "meta": { "apiVersion": "v2.0.0" },
        "query": {
            "requestParameters": {
                "organism": "Monkeypox_virus",
                "requestedGranularity": "boolean"
            }
        }
    });
    let req = Request::builder()
        .method(Method::POST)
        .uri("/g_variants/query")
        .header("content-type", "application/json")
        .extension(who_passport_claims())
        .body(axum::body::Body::from(serde_json::to_vec(&body).unwrap()))
        .unwrap();
    let resp = app.oneshot(req).await.unwrap();
    assert_eq!(resp.status(), StatusCode::OK);
    let v: serde_json::Value = serde_json::from_slice(
        &axum::body::to_bytes(resp.into_body(), usize::MAX)
            .await
            .unwrap(),
    )
    .unwrap();
    assert_eq!(v["response"]["exists"], true);
}
