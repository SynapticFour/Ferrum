# Ferrum — Threat Model

**Status:** Living · customer-shareable
**Version:** 1.0 · 2026-08-12
**Audience:** Security reviewers, operators, procurement
**Related:** [SECURITY.md](../SECURITY.md) · [CRYPT4GH.md](CRYPT4GH.md) · [COMPLIANCE.md](COMPLIANCE.md) · [INCIDENT_RESPONSE.md](INCIDENT_RESPONSE.md) (when present)

This document states **what Ferrum protects, from whom, and what is explicitly out of scope**. It is not a penetration-test report and not a certification.

---

## 1. Product in one line

Ferrum is a **customer-operated GA4GH gateway** (DRS, WES, TES, TRS, Beacon, htsget, Passports, Crypt4GH) for genomic data and compute. Operators deploy it on their hardware (lab, hub, or edge). Synaptic Four does not hold customer production keys or data in the default model.

---

## 2. Assets

| Asset | Sensitivity | Notes |
|-------|-------------|-------|
| Genomic object bytes (BAM/CRAM/VCF/…) | High | Often special-category / research data under operator’s legal basis |
| Crypt4GH node / recipient private keys | Critical | Compromise → ciphertext recoverable |
| Auth tokens / Passport visas / JWKS trust config | High | Forged access to DRS/WES |
| WES/TES workflow definitions & run params | Medium–High | May embed identifiers or paths |
| Service registry / metadata (DRS IDs, Beacon indices) | Medium | Discovery and linkage risk |
| Audit / security event logs | Medium | Integrity matters for investigations |
| Admin credentials / deployment secrets | Critical | Compose env, DB, object-store keys |

---

## 3. Trust boundaries

```text
┌─────────────────────────────────────────────────────────────┐
│  Operator organisation (trusted to configure & hold keys)   │
│  ┌──────────────┐   ┌──────────────┐   ┌─────────────────┐ │
│  │ Ferrum       │───│ Object store │   │ Compute (TES)   │ │
│  │ gateway      │   │ / DB         │   │ / WES engine    │ │
│  └──────┬───────┘   └──────────────┘   └─────────────────┘ │
│         │ optional                                              │
│         ▼                                                       │
│  ┌──────────────┐                                               │
│  │ ga4gh-infra  │  (external Passport / ADS / DUO)              │
│  └──────────────┘                                               │
└─────────────────────────────────────────────────────────────┘
         ▲ HTTPS
         │
   Researchers / systems with valid credentials
```

| Boundary | Inside | Outside |
|----------|--------|---------|
| Process | Ferrum services | Clients, browsers, partner EHRs |
| Crypto | Node key material on disk/HSM path operator chooses | Requester Crypt4GH keys |
| Identity | Built-in passports **or** external ga4gh-infra | IdP / institutional OIDC |
| Edge | SQLite / local store on Pi-class node | Hub sync path when online |

---

## 4. Adversaries (in scope)

| Adversary | Goal | Ferrum posture |
|-----------|------|----------------|
| External network attacker | Unauth access, SSRF, injection | Auth middleware, SSRF URL policy, input validation (see SECURITY.md) |
| Stolen / leaked bearer token | Impersonate researcher | Short-lived tokens, optional revocation, TLS |
| Insider with OS access to gateway host | Read keys / plaintext during decrypt | Operator key custody + host hardening (out of Ferrum code) |
| Malicious workflow submitter (auth’d) | Abuse compute / exfil via WES | Authz on WES/TES; operator policy on engines |
| Compromised dependency | RCE / supply chain | SBOM on release; cargo-deny in CI; dependency-review |
| Lost / stolen edge device | Offline data exposure | Crypt4GH at rest when enabled; field wipe/runbook |

### Out of scope (explicit non-goals)

| Non-goal | Meaning |
|----------|---------|
| Nation-state targeted compromise of a specific operator | Beyond product scope; organisational / national controls |
| Guaranteeing legal “compliance” (GDPR/EHDS/HIPAA) | Technical controls only; operator’s legal basis |
| Protecting data if operator disables auth / Crypt4GH | Misconfiguration is operator risk |
| TEE / confidential computing for plaintext-in-process | Documented future; today decrypt touches process memory |
| Synaptic Four as custodian of production keys | Default model is customer-held |

---

## 5. STRIDE summary

| STRIDE | Examples | Mitigations (current) | Residual |
|--------|----------|----------------------|----------|
| Spoofing | Fake JWT / Passport | Algorithm pinning; JWKS; optional issuer checks | Weak IdP config |
| Tampering | Modify objects in store | Object-store IAM; optional integrity; Crypt4GH AEAD | Store admin compromise |
| Repudiation | Deny access | Security event log; optional persistence | Log retention operator-owned |
| Info disclosure | Plaintext stream endpoint; logs | Crypt4GH; authz before re-wrap; SSRF controls | `/stream` plaintext over TLS by design when used |
| DoS | Flood WES/TES | Operator rate limits / capacity | No global anti-DDoS in product |
| Elevation | Path traversal; workspace escape | `safe_join`; workspace checks | Bugs → IR + patch |

Detail for Crypt4GH invariants: [CRYPT4GH.md](CRYPT4GH.md).

---

## 6. Key management (threat-relevant)

- **Node master key** encrypts objects at rest when Crypt4GH enabled; download **re-wraps header** for requester pubkey after authz.
- Loss of node private key → data unrecoverable (availability risk) or, if copied by attacker, confidentiality risk.
- Staff turnover: operator must rotate/revoke keys and Passport issuers; Ferrum does not automate HR offboarding.
- See customer key-custody one-pager in Showcase (org level-up B10) when published.

---

## 7. Dependencies on other products

| Component | Trust implication |
|-----------|-------------------|
| **ga4gh-infra** (optional) | Identity plane; compromise → unauthorized visas |
| **Solum** (optional companion) | Consent revoke can fail-closed DRS/WES when configured; subject IDs must match |
| **HELIOS** | Consumes evidence; does not protect Ferrum runtime |
| **Object store / DB** | Integrity and backup are operator responsibilities |

---

## 8. Acceptance for pilots

A pilot security review should at least verify:

1. Auth required on data/compute APIs (no open anonymous DRS/WES in production profile).
2. Crypt4GH key files permissions and backup.
3. TLS termination and network exposure.
4. Backup/restore of metadata + object store.
5. Incident contact path ([INCIDENT_RESPONSE.md](INCIDENT_RESPONSE.md)).

---

## 9. Maintenance

Update this file when auth modes, Crypt4GH behaviour, or edge trust boundaries change materially. Link new residual risks; do not claim residual risk is zero.
