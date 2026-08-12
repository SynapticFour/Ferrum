# Operator runbook — Solum subject bridge (`solum_subject`)

**Audience:** operators wiring Ferrum genomic objects to Solum clinical subjects
**Status:** 2026-08-12 · org plan **F2**
**Contract:** Solum [ADR 0003](https://github.com/SynapticFour/Solum/blob/main/docs/adr/0003-subject-bridge.md) · Showcase [co-custody.md](https://github.com/SynapticFour/SynapticFour-Showcase/blob/main/docs/for-customers/co-custody.md)

---

## Goal

Use **one opaque string** as the join key:

| Surface | Field |
|---------|--------|
| Solum subject-link / consent | `solum_subject_id` |
| Ferrum DRS metadata / WES tags | `solum_subject` (constant `SOLUM_SUBJECT_METADATA_KEY`) |
| BRA Phenopacket (optional) | `phenopacket_id` on the same subject-link row |

If these diverge, consent teeth (H2.1) and Path E+ evidence will not correlate.

---

## Recommended sequence

### 1. Create clinical subject (Solum)

```bash
# Example: FHIR Patient — Solum auto-upserts subject-link with
# solum_subject_id = Patient.id
curl -sS -X POST "$SOLUM/v1/fhir/Patient" \
  -H "Authorization: Bearer $SOLUM_TOKEN" \
  -H "Content-Type: application/json" \
  -d '{"resourceType":"Patient","id":"subj-pilot-001"}'
```

Or upsert explicitly:

```bash
curl -sS -X POST "$SOLUM/v1/cdr/subject-link" \
  -H "Authorization: Bearer $SOLUM_TOKEN" \
  -H "Content-Type: application/json" \
  -d '{
    "solum_subject_id": "subj-pilot-001",
    "ferrum_drs_id": "drs.example/object-1",
    "phenopacket_id": "ppkt-pilot-001"
  }'
```

Capability: `solum:cdr:write` (read: `solum:cdr:read`).

### 2. Grant consent for a purpose

```bash
curl -sS -X POST "$SOLUM/v1/consent/grant" \
  -H "Authorization: Bearer $SOLUM_TOKEN" \
  -H "Content-Type: application/json" \
  -d '{"subject":"subj-pilot-001","purpose":"research","actor":"practitioner/ops"}'
```

### 3. Stamp Ferrum DRS / runs with the same string

Set object metadata / run tags:

- `solum_subject` = `subj-pilot-001`
- `solum_purpose` = `research` (or rely on `FERRUM_SOLUM__DEFAULT_*`)

Enable Ferrum `[solum]` / `FERRUM_SOLUM__BASE_URL` + sidecar token so DRS byte access and WES `POST /runs` poll Solum and **fail closed** unless `status=granted`.

### 4. Verify

```bash
curl -sS "$SOLUM/v1/cdr/subject-link/subj-pilot-001" -H "Authorization: Bearer $SOLUM_TOKEN"
curl -sS "$SOLUM/v1/consent/status?subject=subj-pilot-001&purpose=research" \
  -H "Authorization: Bearer $SOLUM_TOKEN"
```

Showcase: `make path-eplus-smoke` (live) · Evidence Pack role `solum_subject_link` (fixtures/CI).

### 5. BRA (optional research path)

When a Phenopacket exists, POST Solum subject-link with `phenopacket_id` equal to the Phenopacket resource id and `solum_subject_id` equal to the Ferrum metadata string (often the BRA `pseudonym_id`). See BioResearch Assistant `docs/SOLUM-SUBJECT-BRIDGE.md`.

---

## Anti-patterns

- Different strings for Patient.id vs DRS `solum_subject`
- Enabling Ferrum Solum consent without any subject-link / grant
- Treating Evidence Pack subject-link fixtures as production identity proof

---

## Related

- Ferrum [customer-runbook.md](customer-runbook.md) (auth / consent honesty)
- Solum [PARTNER-EHR-API.md](https://github.com/SynapticFour/Solum/blob/main/docs/customer/PARTNER-EHR-API.md)
- Solum [ferrum.md](https://github.com/SynapticFour/Solum/blob/main/docs/ferrum.md)
