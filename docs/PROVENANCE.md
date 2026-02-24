# Data provenance and lineage

Ferrum tracks **provenance** as a directed acyclic graph (DAG) linking DRS objects and WES runs: which runs consumed which objects as inputs, which runs produced which objects as outputs, and which objects were derived from others (e.g. manual ingest with `derived_from`).

This enables **lineage queries** (upstream/downstream), **visualization** in the UI, and **RO-Crate export** for citation and submission to Zenodo/Figshare.

---

## Model

- **Nodes:** DRS objects (`drs_object`) and WES runs (`wes_run`).
- **Edges:** `input` (object → run), `output` (run → object), `derived_from` (object → object).

Edges are stored in `provenance_edges`; a recursive view `provenance_lineage` supports traversal with a depth limit and cycle detection.

---

## When provenance is recorded

| Event | Action |
|-------|--------|
| **WES run submitted** | `workflow_params` is scanned for `drs://` URIs; for each resolved object ID, `record_wes_input(run_id, object_id)` is called. |
| **WES run completes** | When outputs are registered in DRS (e.g. auto-ingest), call `record_wes_output(run_id, object_id)` for each output. |
| **DRS ingest with `derived_from`** | POST `/ingest/url` or `/ingest/batch` can include `"derived_from": ["drs://host/id", ...]`; for each source object, `record_derived_from(from_id, to_id)` is called. |

Provenance is only written when a **provenance store** is configured (PostgreSQL; optional).

---

## API endpoints

| Where | Method | Path | Description |
|-------|--------|------|-------------|
| DRS | GET | `/ga4gh/drs/v1/objects/{object_id}/provenance` | Lineage for a DRS object. Query: `direction=upstream\|downstream\|both`, `depth=1..20`. Returns `{ object_id, direction, graph: { nodes, edges, mermaid } }`. |
| WES | GET | `/ga4gh/wes/v1/runs/{run_id}/provenance` | Lineage subgraph for a run (all inputs and outputs). Returns `{ run_id, graph: { nodes, edges, mermaid, cytoscape } }`. |
| WES | GET | `/ga4gh/wes/v1/provenance/graph` | Generic subgraph. Query: `root_id`, `root_type=drs_object\|wes_run`, `direction`, `depth`. Powers the UI graph view. |
| WES | GET | `/ga4gh/wes/v1/runs/{run_id}/export/ro-crate` | Download run as RO-Crate (ZIP with `ro-crate-metadata.json`) for citation and submission. |

---

## UI

- **DRS object detail** (`/data/objects/:id`): **Lineage** tab shows upstream/downstream graph (cytoscape.js), with Export PNG and Export Mermaid.
- **WES run detail** (`/workflows/runs/:id`): **Lineage** tab shows inputs and outputs and their connections.
- **Dashboard:** **Recent provenance** lists the last 10 runs with links to their lineage page.

---

## RO-Crate export

`GET /runs/{run_id}/export/ro-crate` returns a ZIP containing:

- **ro-crate-metadata.json** — [RO-Crate 1.1](https://w3id.org/ro/crate/1.1): `@graph` with CreativeWork (metadata), Dataset (the run with `hasPart` = inputs/outputs), SoftwareApplication (workflow), CreateAction (the run with `result` = outputs).

Inputs and outputs are taken from provenance edges when available, otherwise from the run’s `outputs` field. The crate can be submitted to Zenodo, Figshare, or similar for DOI and long-term preservation.

---

## Configuration

Provenance is **optional**. To enable it:

1. Use **PostgreSQL** (the recursive view is PostgreSQL-specific).
2. Run migrations (includes `20250101000008_provenance`).
3. When building the gateway (or DRS/WES state), construct `ProvenanceStore::new(pool)` and pass it as:
   - **DRS:** `provenance_store: Some(Arc::new(store))` on `ferrum_drs::AppState`.
   - **WES:** fifth argument to `ferrum_wes::router(..., provenance_store: Some(Arc::new(store)))`.
4. When a run completes and outputs are registered in DRS, call `provenance_store.record_wes_output(run_id, object_id)` for each output (e.g. in your executor or completion handler).

---

*[← Documentation index](README.md)*
