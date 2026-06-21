# Federated Access — Remaining Gaps

Phase 5 closed the **medium-priority** federated-access items (in-app compute-pool submit, local WES auto-forward, grant dedup). This document lists what remains for future work.

See also: [TEST-COVERAGE-GAPS.md](TEST-COVERAGE-GAPS.md), [../HELIXTEST-INTEGRATION.md](../HELIXTEST-INTEGRATION.md).

---

## Low priority (documented, not blocking)

| Gap | Notes |
|-----|--------|
| **Multi-container federated E2E** | `federated_access` tests use in-process mocks; no Docker stack with two Ferrum gateways + two ADS nodes yet. |
| **VCF → Beacon indexing limits** | Publish indexing is local, capped (~10k variants); large VCFs need chunked/async pipeline hardening. |
| **Publish UI toggles** | `draft`, `index_beacon_federate`, and `index_variants` are API-only; no UI controls on the publish dialog. |
| **Registry health weighting** | Service selection uses static prefs; no live health/latency weighting for DRS/WES/ADS resolution. |
| **Beacon federation depth** | Publish probes federation peers; no multi-hop or conflict resolution across Beacon networks. |
| **Operational runbook** | No single ops doc for rotating ADS keys, re-seeding federated catalogs, or peer onboarding checklist. |

---

## Completed in Phase 5

- **Federated compute-pool submit UI** — Grants tab → “Run workflow on compute pool” dialog (`FederatedComputeRunDialog`).
- **Auto remote WES routing** — `POST /ga4gh/wes/v1/runs` forwards when tags include `ads_compute_pool_id` + (`federation_origin` or `remote_wes_base_url`).
- **Grant dedup** — `GET /access/v1/me/grants` deduplicates by `external_id` / `dataset_id` with origin preference (same pattern as federated catalog).
- **CI** — `make test-federated` + Ferrum CI step; ga4gh-infra Docker E2E runs `prepare-docker-vendor` before image build.

---

## Suggested follow-ups (when prioritised)

1. **Two-node pilot E2E** — extend `ci-pilot-aai-e2e.sh` or add `ci-federated-pilot-e2e.sh` with compose profile for peer gateway.
2. **Publish UI** — expose visibility and Beacon indexing flags on workspace publish flow.
3. **Runbook** — short `docs/FEDERATED-ACCESS-OPS.md` once a second production peer is live.
