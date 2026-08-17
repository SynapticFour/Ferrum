# Ferrum — Incident Response Runbook

**Status:** Living · 2026-08-12
**Audience:** Operators + Synaptic Four support
**Company plan:** not published in this repository. Operator incident steps for Ferrum are in this file.
**Threat model:** [THREAT_MODEL.md](THREAT_MODEL.md)

This runbook is product-specific. It does **not** replace legal counsel or the company IR plan.

---

## 1. What counts as an incident

- Suspected unauthorized access to DRS objects, WES/TES runs, or admin APIs
- Crypt4GH key material exposure or loss
- Security event spike (auth failures, SSRF blocks, path traversal)
- Ransomware / host compromise on gateway, DB, or object store
- Accidental public exposure of a pilot deployment
- Supply-chain alert on a dependency used in production

---

## 2. Severity (map to company plan)

| Level | Ferrum examples |
|-------|-----------------|
| Critical | Confirmed exfiltration of genomic objects; signing/node key theft |
| High | Auth bypass; public anonymous DRS with real data; backup theft |
| Medium | Contained vuln; failed intrusion; misconfig caught before data loss |
| Low | Dependency advisory with no exploit path in config |

---

## 3. Immediate actions (0–1 h)

1. **Preserve evidence** — do not wipe logs/volumes yet.
2. **Contain** — revoke tokens; disable public ingress; rotate exposed secrets; take gateway offline if needed.
3. **Notify** — operator security contact + `contact@synapticfour.com` if Synaptic Four support is contracted.
4. **Scope** — which environments (edge vs hub), which object IDs / workspaces, auth mode (builtin vs ga4gh-infra).

---

## 4. Investigation (1–24 h)

- Collect Ferrum security events / access logs
- Check object-store access logs and DB audit
- If Crypt4GH: assume ciphertext may be copied; determine if node private key was exposed
- If Passports/external auth: involve ga4gh-infra visa/broker logs
- Record timeline in a single incident doc (who/what/when)

---

## 5. Notification (regulatory / contractual)

- Follow **company IR plan** and customer DPA (often **72 hours** to controller after awareness of a personal-data breach — confirm with counsel).
- Synaptic Four does not unilaterally notify regulators for customer-operated deployments; the **operator** is typically controller. Support can help draft facts.

---

## 6. Eradication & recovery

- Patch Ferrum to fixed release; rotate all secrets and Crypt4GH keys as required
- Restore from known-good backups if integrity uncertain ([FIELD-OPS.md](FIELD-OPS.md), customer DR pack)
- Re-enable auth-required production profile only after verification
- Optional: re-run HelixTest smoke against the recovered stack

---

## 7. Post-incident

- Write postmortem (timeline, root cause, actions)
- Update this runbook / threat model if a new residual risk appeared
- Schedule dependency/CI follow-ups if supply-chain related

---

## 8. Contacts

| Role | Contact |
|------|---------|
| Synaptic Four security / support | contact@synapticfour.com |
| Operator on-call | *(fill per site)* |
| Counsel | *(fill per site)* |
