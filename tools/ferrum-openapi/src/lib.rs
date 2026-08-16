// SPDX-License-Identifier: BUSL-1.1

//! Build the gateway-prefixed OpenAPI document from the same utoipa types as swagger-ui.

use utoipa::openapi::info::{ContactBuilder, InfoBuilder, LicenseBuilder};
use utoipa::openapi::{OpenApi, ServerBuilder};
use utoipa::OpenApi as OpenApiTrait;

const VERSION: &str = env!("CARGO_PKG_VERSION");

const DESCRIPTION: &str = "Implementation map of the Ferrum gateway (utoipa dump). Source of truth for GA4GH DRS/WES/TES/TRS/Beacon/htsget is each standard’s published OpenAPI — see docs/GA4GH.md. This file records what this process exposes, including Ferrum-only paths. Not a replacement spec. Not official GA4GH certification. Auth is a deployment choice. Behavioral proof against the published specs is HelixTest.";

fn prefix_paths(mut spec: OpenApi, prefix: &str) -> OpenApi {
    let old = std::mem::take(&mut spec.paths.paths);
    for (path, item) in old {
        let prefixed = if path.starts_with(prefix) {
            path
        } else {
            format!("{prefix}{path}")
        };
        spec.paths.paths.insert(prefixed, item);
    }
    spec
}

fn merge_service(spec: &mut OpenApi, other: OpenApi, prefix: &str) {
    spec.merge(prefix_paths(other, prefix));
}

/// Gateway-absolute OpenAPI 3 document.
pub fn ferrum_openapi() -> OpenApi {
    let mut spec = prefix_paths(ferrum_drs::DrsApiDoc::openapi(), "/ga4gh/drs/v1");
    spec.info = InfoBuilder::new()
        .title("Ferrum")
        .version(VERSION)
        .description(Some(DESCRIPTION))
        .contact(Some(
            ContactBuilder::new()
                .name(Some("Synaptic Four"))
                .email(Some("contact@synapticfour.com"))
                .build(),
        ))
        .license(Some(
            LicenseBuilder::new()
                .name("BUSL-1.1")
                .url(Some(
                    "https://github.com/SynapticFour/Ferrum/blob/main/LICENSE",
                ))
                .build(),
        ))
        .build();
    spec.servers = Some(vec![ServerBuilder::new()
        .url("/")
        .description(Some(
            "Paths are absolute from the Ferrum gateway origin (default http://127.0.0.1:8080).",
        ))
        .build()]);

    merge_service(&mut spec, ferrum_wes::WesApiDoc::openapi(), "/ga4gh/wes/v1");
    merge_service(&mut spec, ferrum_tes::TesApiDoc::openapi(), "/ga4gh/tes/v1");
    merge_service(&mut spec, ferrum_trs::TrsApiDoc::openapi(), "/ga4gh/trs/v2");
    merge_service(
        &mut spec,
        ferrum_beacon::BeaconApiDoc::openapi(),
        "/ga4gh/beacon/v2",
    );
    merge_service(
        &mut spec,
        ferrum_htsget::HtsgetApiDoc::openapi(),
        "/ga4gh/htsget/v1",
    );
    merge_service(
        &mut spec,
        ferrum_crypt4gh::Crypt4GhApiDoc::openapi(),
        "/ga4gh/crypt4gh/v1",
    );
    merge_service(
        &mut spec,
        ferrum_passports::PassportsApiDoc::openapi(),
        "/passports/v1",
    );
    merge_service(
        &mut spec,
        ferrum_cohorts::CohortApiDoc::openapi(),
        "/cohorts/v1",
    );
    spec
}

/// Pretty JSON with a trailing newline (the committed file shape).
pub fn ferrum_openapi_json() -> String {
    let mut json = ferrum_openapi()
        .to_pretty_json()
        .expect("serialize Ferrum OpenAPI");
    if !json.ends_with('\n') {
        json.push('\n');
    }
    json
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn dumps_gateway_prefixed_drs_service_info() {
        let json = ferrum_openapi_json();
        assert!(
            json.contains("/ga4gh/drs/v1/service-info"),
            "DRS service-info missing"
        );
        assert!(
            json.contains("/ga4gh/wes/v1/service-info"),
            "WES service-info missing"
        );
        assert!(
            json.contains("/ga4gh/htsget/v1/reads/service-info"),
            "htsget reads service-info missing"
        );
    }

    #[test]
    fn committed_spec_matches_generator() {
        let generated = ferrum_openapi_json();
        let committed = include_str!("../../../docs/openapi/ferrum.openapi.json");
        assert_eq!(
            generated, committed,
            "docs/openapi/ferrum.openapi.json is stale — run: make openapi"
        );
    }
}
