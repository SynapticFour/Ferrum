# Ferrum documentation index

Every document under `docs/`, grouped in the Diátaxis order: a tutorial, how-to guides, reference, then explanation. One line each.

These pages are technical orientation. They are not legal advice. The licence file is [LICENSE](../LICENSE).

## Tutorial

| Document | What it is |
|----------|------------|
| [GETTING-STARTED.md](GETTING-STARTED.md) | Clone this repository, run `make prove`, then the auth-on path and the demo stack. |
| [customer-runbook.md](customer-runbook.md) | German operator handbook for a first install and the health check. |

## How-to guides

| Document | What it is |
|----------|------------|
| [OPERATIONS.md](OPERATIONS.md) | DNS check for outreach-mail authentication, as the script already runs it. |
| [INCIDENT_RESPONSE.md](INCIDENT_RESPONSE.md) | Operator steps for a Ferrum incident. |
| [METADATA-STORE.md](METADATA-STORE.md) | Optional HTTP API for metadata documents without a new file ingest. |
| [INSTALLATION.md](INSTALLATION.md) | Install, configure, upgrade, and uninstall. The single-node section is a design target. |
| [deployment/README.md](deployment/README.md) | Deployment shapes. The shapes are design targets, not a record of a cluster in use. |
| [deployment/OFFLINE-AIRGAP.md](deployment/OFFLINE-AIRGAP.md) | Move a pinned bundle into a network that does not fetch images. |
| [deployment/UPDATE-SOP.md](deployment/UPDATE-SOP.md) | Template for a controlled update and a rollback. |
| [deployment/RELEASE-CHECKLIST.md](deployment/RELEASE-CHECKLIST.md) | Checks named before a release or a hotfix rollout. |
| [AFRICA-DEPLOYMENT.md](AFRICA-DEPLOYMENT.md) | One computer, including intermittent networks. Cites ADR-026 and ADR-022. |
| [FIELD-OPS.md](FIELD-OPS.md) | Power, bandwidth, backup, and updates on the field edge. |
| [FIELD-AUTH-OFFLINE.md](FIELD-AUTH-OFFLINE.md) | Offline JWKS cache, field roles, and edge accounts. |
| [FIELD-SYNC-QUEUE.md](FIELD-SYNC-QUEUE.md) | Queue on the edge for a later upload. |
| [FIELD-SYNC-HUB.md](FIELD-SYNC-HUB.md) | What the hub does when that upload arrives. |
| [FIELD-ONT-BASECALLING.md](FIELD-ONT-BASECALLING.md) | External Nanopore basecalling (Dorado or Guppy). |
| [FIELD-BEACON-INDEX.md](FIELD-BEACON-INDEX.md) | Build a Beacon index from VCF on the edge, and the limits of that index. |
| [FIELD-GA4GH-DEMO-PI.md](FIELD-GA4GH-DEMO-PI.md) | Field edge on a Raspberry Pi beside the laptop demo. |
| [FIELD-REGULATORY.md](FIELD-REGULATORY.md) | Regulatory pointers. Not legal advice. |
| [INGEST-LAB-KIT.md](INGEST-LAB-KIT.md) | Ingest HTTP API that Lab Kit calls. |
| [WORKFLOWS.md](WORKFLOWS.md) | Submit a workflow through WES. |
| [TES-DOCKER-BACKEND.md](TES-DOCKER-BACKEND.md) | TES with Docker or Podman, including nested containers. |
| [WES-WORKFLOW-ENGINES.md](WES-WORKFLOW-ENGINES.md) | WDL, Nextflow, CWL, and Snakemake on the WES path. |
| [kubernetes-deployment.md](kubernetes-deployment.md) | Optional Helm chart. |
| [solum-subject-bridge-runbook.md](solum-subject-bridge-runbook.md) | Operator steps for the Solum subject bridge. |
| [hotfix-process.md](hotfix-process.md) | Internal hotfix process. The page is in German. |
| [first-release-checklist.md](first-release-checklist.md) | Checklist kept from the v0.3.0 release. |
| [MII-CONNECT.md](MII-CONNECT.md) | MII-oriented profile checks and the `ferrum mii` commands. |
| [MII-CI-INTEGRATION.md](MII-CI-INTEGRATION.md) | How those checks sit in CI. |
| [GA4GH-INFRA-INTEGRATION.md](GA4GH-INFRA-INTEGRATION.md) | External sign-in with ga4gh-infra. |
| [HELIXTEST-INTEGRATION.md](HELIXTEST-INTEGRATION.md) | What the HelixTest job in CI exercises. Not a certificate. |

## Reference

| Document | What it is |
|----------|------------|
| [GA4GH.md](GA4GH.md) | Endpoints, versions, and auth for the interfaces in this tree. |
| [HTSGET.md](HTSGET.md) | htsget tickets: whole objects, regions and headers via the object index, Crypt4GH tickets. |
| [CRYPT4GH.md](CRYPT4GH.md) | Crypt4GH encryption and key handling. |
| [CRYPT4GH-CHUNK-BOUNDARY-FLAKE.md](CRYPT4GH-CHUNK-BOUNDARY-FLAKE.md) | A known chunk-boundary flake. |
| [openapi/README.md](openapi/README.md) | How the OpenAPI dump is produced. |
| [openapi/ferrum.openapi.json](openapi/ferrum.openapi.json) | The committed OpenAPI dump. |
| [COMPATIBILITY.md](COMPATIBILITY.md) | What is pinned together, and what is not one repository. |
| [COMPOSE-IMAGE-PINS.md](COMPOSE-IMAGE-PINS.md) | Digest pins for Compose images. |
| [IMAGE-VARIANTS.md](IMAGE-VARIANTS.md) | Image tags `full`, `edge`, and `edge-infra`. |
| [TESTED-S3-BACKENDS.md](TESTED-S3-BACKENDS.md) | SeaweedFS is the reference object store. |
| [STORAGE-BACKENDS.md](STORAGE-BACKENDS.md) | Local and S3 storage in the code. |
| [IDENTITY.md](IDENTITY.md) | Who the software is for. |
| [CI.md](CI.md) | What starts the workflows. |
| [METADATA-STORE-ROADMAP.md](METADATA-STORE-ROADMAP.md) | Plan linked from the metadata store page. |
| [MII-MODULE-MAPPING.md](MII-MODULE-MAPPING.md) | Module mapping used by the MII checks. |
| [REFERENCE-GENOMES.md](REFERENCE-GENOMES.md) | Reference genome registry. |
| [DATA-RESIDENCY-AUDIT.md](DATA-RESIDENCY-AUDIT.md) | Fields of the residency audit chain. |
| [PROVENANCE.md](PROVENANCE.md) | Provenance records and export. |
| [FERRUM-UI-AUTH.md](FERRUM-UI-AUTH.md) | What ferrum-ui expects from authentication. |
| [PERFORMANCE-CRYPT4GH.md](PERFORMANCE-CRYPT4GH.md) | Plain and Crypt4GH stream timings recorded in this tree. |
| [GOVERNANCE.md](GOVERNANCE.md) | Who maintains the tree. |
| [PORTFOLIO.md](PORTFOLIO.md) | This repository among the public repositories. |
| [BUSINESS-MODEL.md](BUSINESS-MODEL.md) | Open-core boundary. Not a price list. |
| [COMMERCIAL.md](COMMERCIAL.md) | Commercial use needs a written agreement. |
| [COMPLIANCE.md](COMPLIANCE.md) | Regulatory orientation. Not legal advice. |
| [SUITE-OVERVIEW.md](SUITE-OVERVIEW.md) | Pointer to the organisation map. |
| [GA4GH-LAB-KIT-SCOPE.md](GA4GH-LAB-KIT-SCOPE.md) | Boundary between this repository and Lab Kit. |
| [ECOSYSTEM.md](ECOSYSTEM.md) | Sibling repositories and their roles. |
| [OPERATOR-TRUST.md](OPERATOR-TRUST.md) | Which demo switches are on, and what an operator still owns. |
| [OUTBREAK-MODE.md](OUTBREAK-MODE.md) | Outbreak policy switches. |
| [FEDERATION.md](FEDERATION.md) | Federated Beacon without a central coordinator. |
| [FIELD-ECOSYSTEM.md](FIELD-ECOSYSTEM.md) | Field edge beside the neighbouring repositories. |

## Explanation

| Document | What it is |
|----------|------------|
| [ARCHITECTURE.md](ARCHITECTURE.md) | How the gateway, storage, and builds fit together. |
| [ARCHITECTURE-DECISIONS.md](ARCHITECTURE-DECISIONS.md) | Index of the decision records in [DECISIONS.md](../DECISIONS.md). |
| [FOR-EVALUATORS.md](FOR-EVALUATORS.md) | What this tree tests and what it does not claim. |
| [THREAT_MODEL.md](THREAT_MODEL.md) | Threats considered for this repository. |
| [DEPLOYMENT-SHAPES.md](DEPLOYMENT-SHAPES.md) | Edge and full builds, and what is verified. |

Root files that are not under `docs/`: [README](../README.md), [CHANGELOG.md](../CHANGELOG.md), [SECURITY.md](../SECURITY.md), [CONTRIBUTING.md](../CONTRIBUTING.md), [DECISIONS.md](../DECISIONS.md), [CITATION.cff](../CITATION.cff).

## Internal notes (contributors)

## Other task (not merged)

| Branch | Why left | Notes |
|--------|----------|-------|
| `evidence/g-continuity` | Docs-only but **not accurate for current `main`** | Adds `CONTINUITY.md` (“continuity: no unwritten plan promise”). Audited tip still named as `8d72bc4` / tag `v0.3.3`; `main` is at `v0.3.5`+ SeaweedFS work. Belongs to another task — refresh against current tip before merge. Worktree: `_worktrees/Ferrum-g-continuity` (untouched). |

Local `audit-main` worktree (`_worktrees/Ferrum-audit`) is intentionally left alone. Keep branch `license/ferrum`.

---

*[← Back to Ferrum README](../README.md)*
