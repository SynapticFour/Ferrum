# Business model (open core)

Ferrum is licensed under **BUSL-1.1** (Business Source License). The full legal text is in the repository **[LICENSE](../LICENSE)**. **Change Date:** four years from each **version’s** release; **Change License:** **Apache-2.0** (same delayed-open pattern as documented in the license file).

This page is **high-level guidance**, not legal advice. **Only the `LICENSE` file and your written agreements with Synaptic Four** determine your rights and obligations. Laws differ by **country and region**; universities, companies, and public bodies should **consult qualified counsel** where use touches export controls, tax, employment, or sector-specific rules. For commercial use, redistribution, or SaaS, contact **Synaptic Four** via [github.com/SynapticFour](https://github.com/SynapticFour) or the channels listed in `LICENSE`.

**Illustrative examples** below (e.g. “typically covers universities”) describe common situations only; they **do not** extend or narrow the license text.

---

## Relationship to [Ferrum Lab Kit](https://github.com/SynapticFour/Ferrum-Lab-Kit)

| Layer | Role |
|-------|------|
| **Ferrum** (this repo) | GA4GH implementations, gateway, storage, Crypt4GH, conformance-relevant behaviour. **No runtime license key** on the core services. |
| **[Ferrum Lab Kit](https://github.com/SynapticFour/Ferrum-Lab-Kit)** | Deployment and integration: Compose/Helm fragments, profiles, `lab-kit` CLI, LS Login wiring. Follows the same BUSL philosophy; see **[Lab Kit `docs/BUSINESS-MODEL.md`](https://github.com/SynapticFour/Ferrum-Lab-Kit/blob/main/docs/BUSINESS-MODEL.md)** for product-specific commercial lines (e.g. optional **PDF** conformance reports behind `FERRUM_LAB_KIT_LICENSE_KEY`). |

Lab Kit **does not** replace Ferrum’s license: you run Ferrum **under** Ferrum’s BUSL terms; Lab Kit adds packaging and optional gated **artifacts**, not a second license on the GA4GH binaries themselves.

---

## Free under the Additional Use Grant (typical cases)

The **Additional Use Grant** in `LICENSE` allows broad **non-commercial research, academic, and educational** use, and **internal research** at institutions **without** offering Ferrum as a **commercial service to third parties**.

In practice, this usually covers:

- Universities, institutes, and public research organisations running Ferrum for their own projects.
- **Internal** platforms where the primary purpose is research infrastructure, not paid multi-tenant DaaS/SaaS to external customers.
- Development, HelixTest-style conformance runs, teaching, and proof-of-concept deployments.
- Use of the **full** codebase: DRS, WES, TES, TRS, Beacon, Passports, htsget, Crypt4GH, gateway, etc.

**Product principle (aligned with Lab Kit):** Core GA4GH-facing APIs in **this repository** are **not** feature-gated behind Synaptic Four **runtime license keys**. Whether a deployment passes external **conformance** or **accreditation** suites depends on configuration, data, and process—**not** guaranteed by the license or this document.

---

## When to talk to us (commercial or unclear)

BUSL restricts use for **commercial advantage or private monetary gain** and certain **hosted/managed service** models unless covered by the grant or a **separate agreement**. The bullets below are **practical signposts**, not tax, export, or regulatory advice.

**Typically commercial** (needs a license or contract):

- Offering Ferrum (or a derivative) as a **paid** cloud or managed **service** to external customers.
- Embedding Ferrum in a **proprietary product** sold to third parties without a commercial license.
- Scenarios where the **primary business** is reselling access to GA4GH endpoints powered by Ferrum.

**Grey zones** (often resolvable with a short clarification or lightweight agreement):

- Hospital or biobank **IT** providing infrastructure **primarily** for internal clinicians/researchers vs. **external** paying clients.
- **ELIXIR / GDI / national node** deployments with mixed public and industry stakeholders.
- **Consortia** where one member operates the stack for partners — depends on funding flow and who is the “customer”.

If in doubt, **ask early**; we prefer a clear paper trail for both sides.

---

## More differentiated paths (beyond “research free / else pay”)

The **simple** model — **free for non-commercial research, commercial license for for-profit use** — is intentionally the default and matches the **Additional Use Grant** text.

Optional **finer** structures (all subject to written agreement):

| Model | Idea |
|-------|------|
| **Delayed open source** | Already in BUSL: each version **automatically** moves to **Apache-2.0** after the Change Date, reducing long-term lock-in for adopters. |
| **Institutional / consortium licenses** | Single agreement for a university hospital, ELIXIR node, or GDI pilot covering multiple environments and sub-sites. |
| **Support & SLA** | Paid **priority support**, security advisory channel, or onboarding — **without** changing what the open-core code does. |
| **Indemnification / enhanced terms** | **May** be negotiated under **separate commercial agreements** (subject to mutual terms and scope)—not implied by BUSL or this page. The Licensed Work remains **“as is”** unless a signed contract says otherwise. |
| **Dual-use research** | Mixed public–private funding: sometimes handled via **narrow addenda** (field-of-use, attribution) rather than a full commercial license for the entire institution. |

Nothing here replaces the **LICENSE** file; it describes **how** Synaptic Four can engage beyond the default grant.

---

## What we do **not** do (non-negotiables)

- **No paywall** on core GA4GH APIs in this repo to “unlock” DRS/WES/TES/etc.
- **No** requirement to use Lab Kit or any other product to **run** Ferrum under permitted research use.
- **No** retroactive relicensing of past releases without updating `LICENSE` / release notes.

---

## Summary

| | |
|--|--|
| **License** | BUSL-1.1 → Apache-2.0 after Change Date per version |
| **Research / education** | Broadly permitted under Additional Use Grant |
| **Commercial / SaaS / embedding** | Contact Synaptic Four |
| **Lab Kit** | Same philosophy; optional **gated extras** (e.g. PDF reports) documented [there](https://github.com/SynapticFour/Ferrum-Lab-Kit/blob/main/docs/BUSINESS-MODEL.md) |
| **Ferrum core** | Full stack usable without vendor keys under permitted use |
| **Standards conformance** | Interoperability targets GA4GH APIs; **formal conformance** or **accreditation** is deployment-specific and **not** promised by the license or this page. |

For product boundaries between Ferrum and packaging tools, see **[GA4GH-LAB-KIT-SCOPE.md](GA4GH-LAB-KIT-SCOPE.md)** (German).

---

*[← Documentation index](README.md)*
