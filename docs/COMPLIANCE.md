# Compliance & Regulatory Framework

Ferrum is designed from the ground up for deployment in regulated research
environments. This document describes how Ferrum can technically support
institutions that need to comply with legal and regulatory frameworks in
Germany, the EU, and worldwide.

> **Important:** This document is for technical orientation only and is not
> legal advice. Ferrum is infrastructure software. Regulatory obligations
> (e.g. GDPR controller/processor roles, NIS2 categorisation, EHDS duties,
> ethics approvals) rest with the institution operating Ferrum and its
> partners, not with Synaptic Four as the software vendor. Operators must
> obtain their own legal assessment for any concrete deployment.

---

## Table of Contents

1. [GDPR / DSGVO](#gdpr--dsgvo)
2. [BDSG (Germany)](#bdsg-germany)
3. [Gaia-X](#gaia-x)
4. [NIS2 Directive](#nis2-directive)
5. [EU AI Act](#eu-ai-act)
6. [EHDS (European Health Data Space)](#ehds)
7. [GA4GH Policy Framework](#ga4gh-policy-framework)
8. [International Frameworks](#international-frameworks)
9. [Operator Checklist](#operator-checklist)

---

## GDPR / DSGVO

**Regulation (EU) 2016/679 — General Data Protection Regulation**

Genomic and health data are **special category data** under Article 9 GDPR
and require the highest level of protection. Ferrum provides technical
features that can support GDPR‑aligned deployments by operating institutions,
but it does not, by itself, make any deployment GDPR‑compliant.

### How Ferrum supports GDPR compliance

| GDPR Requirement | Ferrum Feature |
|---|---|
| Art. 5 — Data minimisation | DRS stores only what is explicitly ingested; no implicit data collection |
| Art. 5 — Integrity & confidentiality | Crypt4GH‑based encryption for stored genomic data when configured |
| Art. 5 — Accountability | Full provenance graph: every processing step is recorded |
| Art. 9 — Special category data | GA4GH Passport ControlledAccessGrants Visa for genomic data access |
| Art. 13/14 — Transparency | Beacon v2 provides structured, queryable data descriptions |
| Art. 17 — Right to erasure | DRS supports object deletion; operators must implement retention policies |
| Art. 20 — Data portability | DRS export via standard GA4GH APIs; RO-Crate export |
| Art. 25 — Privacy by design | Encryption at rest and in transit when configured; no plaintext storage is intended in normal operation |
| Art. 30 — Records of processing | Security event log and audit trail built into all services |
| Art. 32 — Security of processing | OWASP Top 10 hardened; AES-256-GCM encryption; TLS enforced |
| Art. 33 — Breach notification | Security event log with 72h-aligned alerting infrastructure |
| Art. 35 — DPIA | See [DPIA Template](#dpia-template) below |

### Genomic data and GDPR Article 9

Genomic data is explicitly listed as special category data under Art. 9(1)
GDPR. Processing requires one of the Art. 9(2) exceptions, most commonly:

- **Art. 9(2)(a)** — Explicit consent of the data subject
- **Art. 9(2)(j)** — Scientific research purposes with appropriate safeguards

Ferrum's GA4GH Passport system is designed to encode and enforce these
consent decisions as machine-readable Visas, enabling automated, auditable
access control that aligns with GDPR requirements.

### International data transfers (Art. 46/47 GDPR)

When genomic data is transferred across borders:
- **Within EU/EEA**: Subject to GDPR, but typically simpler when Ferrum is deployed on‑premises under the institution’s control.
- **To third countries**: Requires adequacy decision, SCCs, or BCRs as determined by the controller.
- Ferrum's on‑premises deployment model can help institutions keep data under their own control and reduce the need for cross‑border transfers, but architectural choices remain the operator’s responsibility.

### DPIA Template

Operators processing genomic data **must** conduct a Data Protection
Impact Assessment (Art. 35 GDPR). Key points for a Ferrum deployment:

**Processing description:**
- Genomic sequence data, phenotypic metadata, workflow results
- Purposes: scientific research, clinical genomics
- Legal basis: Art. 9(2)(j) GDPR + §27 BDSG (research)

**Risks identified:**
- Re-identification of pseudonymised genomic data
- Unauthorised access to controlled-access datasets

**Technical mitigations (provided by Ferrum):**
- Crypt4GH encryption with per-user key re-wrapping
- GA4GH Passport/Visa access control
- Audit logging of all data access events
- Network isolation via on-premises deployment

**Residual organisational measures (operator responsibility):**
- Ethics committee approval for research projects
- Data Processing Agreements with data sources
- Staff training on genomic data handling
- Appointment of a Data Protection Officer (required if >250 staff)

---

## BDSG (Germany)

**Bundesdatenschutzgesetz — German Federal Data Protection Act**

The BDSG supplements GDPR with German-specific provisions.
Key provisions relevant to Ferrum deployments:

### §27 BDSG — Research privilege

Processing of special category data (including genomic data) for
scientific research is permitted under §27 BDSG with appropriate
safeguards. Ferrum's access control and encryption directly serve
as these technical safeguards.

### §64 BDSG — Security requirements

Requires appropriate technical and organisational measures. Ferrum
addresses these through its built-in security architecture (see SECURITY.md).

### Data breach notification

German operators must notify the relevant *Landesdatenschutzbehörde*
within 72 hours of discovering a high-risk data breach (Art. 33 GDPR
+ §42 BDSG). Ferrum's security event log with webhook alerting supports
this obligation.

### Relevant German supervisory authorities

| State | Authority |
|---|---|
| Bayern | Bayerisches Landesamt für Datenschutzaufsicht (BayLDA) |
| Berlin | Berliner Beauftragte für Datenschutz und Informationsfreiheit |
| Hamburg | Der Hamburgische Beauftragte für Datenschutz |
| NRW | Landesbeauftragte für Datenschutz und Informationsfreiheit NRW |
| Federal | Bundesbeauftragter für den Datenschutz und die Informationsfreiheit (BfDI) |

---

## Gaia-X

**European Federated Data Infrastructure**

Ferrum is designed to operate in a way that is compatible with core Gaia‑X
principles (data sovereignty, interoperability, portability), but formal
Gaia‑X registration or labelling requires a separate assessment and
self‑description by the operator.

### Gaia-X alignment

| Gaia-X Principle | Ferrum Implementation |
|---|---|
| **Data Sovereignty** | On-premises deployment; institution retains full control |
| **Transparency** | Open source (BUSL-1.1); all APIs documented via OpenAPI |
| **Portability** | GA4GH standard APIs; no vendor lock-in |
| **Interoperability** | DRS, WES, TES, TRS, Beacon v2, Passports |
| **Security** | OWASP hardened; Crypt4GH encryption; TLS enforced |
| **European Values** | Developed in Germany 🇩🇪; designed with GDPR principles in mind. Actual legal compliance depends on the operator and its processes. |

### Gaia-X Label Levels

- **Standard Compliance / Label Level 1**: Achievable by self‑declaration
  of the operator. Ferrum's architecture is intended to support typical
  criteria, but any Gaia‑X label depends on the operator’s self‑description
  and, where applicable, third‑party assessment.
- **Label Level 2/3**: Requires third-party audit by a Conformity Assessment
  Body (CAB). Ferrum deployments may pursue this for regulated clinical use.

### Gaia-X Self-Description

Operators wishing to register a Ferrum‑based deployment in the Gaia‑X
Federated Catalogue should create a self‑description using the Gaia‑X
ontology and follow the Gaia‑X onboarding process (e.g. via Gaia‑X
Hub Germany). Ferrum does not, by itself, confer Gaia‑X membership
or labelling.

---

## NIS2 Directive

**Directive (EU) 2022/2555 — Network and Information Security**

### Does NIS2 apply to Synaptic Four?

As a small software vendor, Synaptic Four is not directly subject to NIS2
obligations (threshold: 50+ employees or €10M+ turnover).

### Does NIS2 apply to Ferrum operators?

**Yes, potentially.** Institutions operating Ferrum that qualify as:
- **Essential entities**: University hospitals, major research centres
- **Important entities**: Mid-sized research institutions, biobanks

...may be subject to NIS2 cybersecurity obligations.

### How Ferrum helps operators meet NIS2

| NIS2 Requirement | Ferrum Feature |
|---|---|
| Risk management policies | Security event log; OWASP-hardened codebase |
| Incident response | Webhook alerting; structured security events |
| Business continuity | Stateless microservices; Docker/Kubernetes deployment |
| Supply chain security | SBOM generated on every release; cargo-audit in CI |
| Cryptography | AES-256-GCM at rest; TLS in transit; Crypt4GH for genomics |
| Access control | GA4GH Passports; role-based workspace access |
| Vulnerability management | Automated dependency auditing via cargo-deny |

NIS2 fines: up to **€10 million or 2% of global turnover** for essential
entities. Ferrum's security features can help reduce certain technical risks
when correctly configured, but overall NIS2 risk depends on the operator’s
organisational and technical controls.

---

## EU AI Act

**Regulation (EU) 2024/1689 — Artificial Intelligence Act**
*(In force August 2024; fully applicable August 2026)*

### Is Ferrum an AI system under the AI Act?

**No.** Ferrum is workflow orchestration and data management infrastructure.
It does not implement machine learning, inference, or autonomous
decision-making. It is explicitly not an AI system as defined in Art. 3(1).

### Research exemption

Ferrum's primary use case — supporting scientific research — benefits from
the AI Act's research exemption (Art. 2(6)). AI systems used solely for
scientific research and development are outside the Act's scope.

### What this means for operators

- Ferrum itself requires no AI Act compliance measures
- AI tools or models *run on top of* Ferrum via WES may require assessment
  depending on their risk classification
- Bioinformatics pipelines for clinical diagnostics may qualify as
  high-risk AI systems (Annex III) and require separate conformity assessment

---

## EHDS

**Regulation (EU) 2025/327 — European Health Data Space**
*(Adopted January 2025; secondary use provisions applicable ~2029)*

The EHDS will transform how health data is shared across Europe.
Ferrum provides building blocks (GA4GH APIs, access control, provenance)
that can support EHDS‑related requirements. Whether a deployment is EHDS
compliant depends on future delegated acts and on the operator’s legal and
organisational measures.

### EHDS Primary Use (patient access to own data)

Ferrum's DRS API enables structured access to genomic data, supporting
the EHDS requirement that individuals can access their own health data.

### EHDS Secondary Use (research data sharing)

EHDS secondary use obligations require health data holders to share
electronic health data (EHD) — including genomic and "-omic" data —
with approved researchers via Health Data Access Bodies (HDABs).

Ferrum supports this through:
- GA4GH DRS as the data access layer
- Beacon v2 for federated data discovery without data transfer
- GA4GH Passports/Visas as the access authorisation mechanism
- Audit logging for HDAB accountability requirements

### EHDS readiness timeline

| Year | EHDS Milestone |
|---|---|
| 2025 | EHDS enters into force |
| 2027 | Primary use provisions apply |
| 2029 | Secondary use provisions apply (general) |
| 2031 | Secondary use — clinical trials and genomic data |

Institutions deploying Ferrum today are building EHDS-ready infrastructure.

---

## GA4GH Policy Framework

**Global Alliance for Genomics and Health**

Ferrum implements a broad GA4GH technical stack and is designed to align
with the GA4GH Framework for Responsible Sharing of Genomic and Health‑
Related Data at a technical level. This is not a legal endorsement; policy
implementation remains the operator’s responsibility.

### GA4GH standards implemented

| Standard | Version | Purpose |
|---|---|---|
| Data Repository Service (DRS) | 1.4 | Data access and location |
| Workflow Execution Service (WES) | 1.1 | Workflow submission |
| Task Execution Service (TES) | 1.1 | Compute task management |
| Tool Registry Service (TRS) | 2.0 | Workflow/tool discovery |
| Beacon | v2 | Federated data discovery |
| Passports & Visas | 1.0 | Access authorisation |
| Crypt4GH | 1.0 | Genomic data encryption |

### GA4GH Genomic Data Access Framework

GA4GH Passports implement a tiered data access model that maps directly
to GDPR consent requirements:
- **Open access**: Public data, no consent required
- **Registered access**: Bona fide researcher verification
- **Controlled access**: Dataset-specific consent via ControlledAccessGrants Visa

This framework is recognised by major funders (NIH, Wellcome Trust,
German Research Foundation/DFG) as a valid access control mechanism.

---

## International Frameworks

### United States

| Framework | Relevance | Ferrum |
|---|---|---|
| **HIPAA** | Health data protection | On‑premises deployment; any Business Associate Agreement (BAA) must be agreed separately with Synaptic Four. |
| **NIH Data Sharing Policy** | NIH-funded research | GA4GH APIs align with NIH requirements |
| **dbGaP** | Genomic data repository | DRS compatible with dbGaP access patterns |

### United Kingdom (post-Brexit)

UK GDPR (retained EU law) applies. The GDPR considerations described in this
document apply in the same way, but operators must assess their own legal
position under UK law.
UK Biobank, Genomics England, and HDR UK use GA4GH standards that Ferrum implements.

### Canada

PIPEDA / provincial privacy laws apply. GA4GH Passports support Canadian
access federation (CanDIG project uses GA4GH standards).

### Australia

Privacy Act 1988 + Australian Privacy Principles. GA4GH framework aligned.

### Global Research Networks

Ferrum is compatible with major international genomic data initiatives:
- **ELIXIR** (European life sciences infrastructure)
- **CanDIG** (Canadian Distributed Infrastructure for Genomics)
- **GA4GH Driver Projects** (Genomics England, TCGA, gnomAD, etc.)
- **EMBL-EBI** data services

---

## Operator Checklist

Before going into production with sensitive data, operators should complete:

### Legal & Organisational

- [ ] Appoint or confirm Data Protection Officer (DPO)
- [ ] Conduct Data Protection Impact Assessment (DPIA) — template above
- [ ] Establish legal basis for processing (Art. 9(2) GDPR)
- [ ] Obtain ethics committee approval for research projects
- [ ] Sign Data Processing Agreements (DPA/AVV) with data sources
- [ ] Review national law requirements (BDSG §27 for Germany)
- [ ] Register with relevant supervisory authority if required

### Technical (Ferrum configuration)

- [ ] Enable TLS in ferrum-gateway configuration
- [ ] Configure Crypt4GH node key with permissions 600
- [ ] Set up external Identity Provider (OIDC) for Passport issuance
- [ ] Configure dataset-level access control in DRS
- [ ] Enable security event webhook for breach alerting
- [ ] Set `config.environment = "production"` (disables debug output)
- [ ] Configure automated backups for PostgreSQL
- [ ] Review and apply network segmentation

### Audit & Monitoring

- [ ] Verify security event log is operational
- [ ] Test breach notification webhook
- [ ] Document data retention and deletion procedures
- [ ] Establish incident response procedure (72h notification obligation)
- [ ] Schedule quarterly dependency audits (cargo-audit)

---

## Contact & Commercial Licensing

For questions about compliance, commercial licensing, or Data Processing
Agreements:

🌐 https://github.com/SynapticFour/Ferrum

*Note: Legal and compliance questions should always be reviewed by a
qualified legal professional familiar with your jurisdiction.*

---

*Proudly developed by individuals on the autism spectrum in Germany 🇩🇪*
*© 2025 Synaptic Four — Precise tools for precise science*
