# Architecture Overview

This document describes the technical architecture of Ferrum: the monorepo design, service composition, data flows, and deployment topologies.

---

## System diagram

```mermaid
graph TD
  subgraph Crates["Ferrum Crates"]
    GW[ferrum-gateway]
    CORE[ferrum-core]
    DRS[ferrum-drs]
    TRS[ferrum-trs]
    WES[ferrum-wes]
    TES[ferrum-tes]
    BEACON[ferrum-beacon]
    PASS[ferrum-passports]
    C4[ferrum-crypt4gh]
  end

  subgraph External["External"]
    PG[(PostgreSQL)]
    MINIO[MinIO / S3]
    KC[Keycloak]
  end

  GW --> DRS
  GW --> TRS
  GW --> WES
  GW --> TES
  GW --> BEACON
  GW --> PASS
  GW --> C4
  DRS --> CORE
  TRS --> CORE
  WES --> CORE
  TES --> CORE
  BEACON --> CORE
  PASS --> CORE
  C4 --> CORE
  GW --> CORE

  DRS --> PG
  WES --> PG
  TES --> PG
  TRS --> PG
  BEACON --> PG
  PASS --> PG
  DRS --> C4
  C4 --> MINIO
  PASS --> KC
```

**ferrum-core** is the foundation: all service crates depend on it for config, database, auth, storage, and error types. **ferrum-gateway** composes all service routers and mounts them under `/ga4gh/*` and `/passports/*`.

---

## Monorepo design philosophy

Ferrum uses a single Cargo workspace so that:

- **Atomic refactoring** — Changes to shared types (e.g. in `ferrum-core`) and all consumers can be committed and tested together.
- **Shared types** — DRS, WES, TES, and others use the same `FerrumConfig`, `DatabasePool`, and error types without version drift.
- **Single CI** — One pipeline runs tests and clippy for the entire codebase.
- **Independent deployment** — The gateway binary is one artifact; individual services are enabled via config flags and composed at runtime.

---

## ferrum-core (shared foundation)

| Module | Responsibility | Key crates |
|--------|----------------|------------|
| `config.rs` | Layered config (defaults → file → env), `FerrumConfig` | serde, config |
| `db.rs` | SQLite/PostgreSQL pool, migrations | sqlx |
| `auth.rs` | JWT validation, JWKS, optional auth | jsonwebtoken, reqwest |
| `storage.rs` | Local and S3 backends, streaming reads | tokio, aws-sdk-s3 / object_store |
| `error.rs` | `FerrumError`, result type | thiserror |
| `types.rs` | Shared GA4GH types (checksums, access methods) | serde |
| `health.rs` | Health check endpoint | axum |

---

## Service isolation model

Each service crate exposes a `router()` (or equivalent) that returns an `axum::Router`. The gateway composes them by nesting under standard GA4GH paths.

**Per-service pattern:**

```rust
// Each service crate
pub fn router(state: AppState) -> Router {
    Router::new()
        .route("/objects/:id", get(get_object))
        .route("/objects", post(create_object))
}

// Gateway composes all
let app = Router::new()
    .nest("/ga4gh/drs/v1", ferrum_drs::router(drs_state))
    .nest("/ga4gh/trs/v2", ferrum_trs::router(trs_state))
    .nest("/ga4gh/wes/v1", ferrum_wes::router(wes_state))
    .nest("/ga4gh/tes/v1", ferrum_tes::router(tes_state))
    .nest("/ga4gh/beacon/v2", ferrum_beacon::router(beacon_state))
    .nest("/passports/v1", ferrum_passports::router(passport_state))
    .nest("/ga4gh/crypt4gh/v1", ferrum_crypt4gh::router());
```

State (DB pool, storage, repo) is constructed in the gateway from config and passed into each service. Services that are disabled return 503 for their routes.

---

## Data flow diagrams

### a) Data ingest flow

```mermaid
sequenceDiagram
  participant Client
  participant DRS
  participant Crypt4GH as Crypt4GH Layer
  participant Storage

  Client->>DRS: POST /ingest/url or multipart upload
  DRS->>DRS: Validate, compute checksums
  DRS->>Crypt4GH: Encrypt (node key)
  Crypt4GH->>Storage: Write encrypted stream
  Storage-->>Crypt4GH: OK
  Crypt4GH-->>DRS: OK
  DRS->>DRS: Insert drs_objects, storage_references
  DRS-->>Client: 201 { id }
```

### b) Transparent Crypt4GH download flow

```mermaid
sequenceDiagram
  participant Client
  participant DRS
  participant Crypt4GH as Crypt4GH Layer
  participant Storage

  Client->>DRS: GET /objects/{id}/access (Auth + X-Crypt4GH-Public-Key)
  DRS->>DRS: Auth + resolve object
  DRS->>Crypt4GH: Open stream (object_id, client_pubkey)
  Crypt4GH->>Storage: get(object_id)
  Storage-->>Crypt4GH: Encrypted stream (node key)
  Crypt4GH->>Crypt4GH: Decrypt header (node key)
  Crypt4GH->>Crypt4GH: Re-encrypt header (client key)
  Crypt4GH-->>DRS: Stream (new header + same body)
  DRS-->>Client: 200 stream
```

### c) Workflow execution flow

```mermaid
sequenceDiagram
  participant Client
  participant WES
  participant DRS
  participant Engine as WES Engine
  participant SSE as Log stream

  Client->>WES: POST /runs (workflow + DRS inputs)
  WES->>DRS: Resolve drs:// URIs to access URLs
  WES->>Engine: Submit run (Nextflow/CWL/WDL/Snakemake)
  WES-->>Client: 200 { run_id }
  Client->>WES: GET /runs/{id}/logs/stream
  WES->>SSE: Attach to run logs
  Engine-->>SSE: stdout/stderr
  SSE-->>Client: SSE event stream
```

### d) Authorization flow

```mermaid
sequenceDiagram
  participant Client
  participant IdP as Identity Provider
  participant Broker as Passport Broker
  participant Ferrum as Ferrum (Passports + DRS)

  Client->>IdP: Login
  IdP-->>Client: Access token
  Client->>Broker: Request Passport (token)
  Broker->>Broker: Issue Visas (policies)
  Broker-->>Client: GA4GH Passport (JWT)
  Client->>Ferrum: GET /objects/{id} (Authorization: Bearer <passport>)
  Ferrum->>Ferrum: Validate Passport, check Visa
  Ferrum->>Ferrum: Resolve object, apply Crypt4GH re-wrap
  Ferrum-->>Client: 200 stream
```

---

## Database schema overview

### DRS

```mermaid
erDiagram
  drs_objects ||--o{ drs_checksums : has
  drs_objects ||--o{ drs_access_methods : has
  drs_objects ||--o| storage_references : has
  drs_objects {
    text id PK
    text name
    bigint size
    timestamptz created_time
    timestamptz updated_time
  }
  storage_references {
    text object_id PK,FK
    text storage_backend
    text storage_key
    boolean is_encrypted
  }
  drs_access_methods {
    text object_id PK,FK
    text type
    jsonb access_url
  }
```

### WES

```mermaid
erDiagram
  wes_runs ||--o{ wes_run_log : has
  wes_runs ||--o{ wes_task_logs : has
  wes_runs {
    text id PK
    text state
    timestamptz created_at
    timestamptz updated_at
  }
  wes_run_log {
    bigint id PK
    text run_id FK
    text stream
    text msg
  }
  wes_task_logs {
    text run_id FK
    text task_id
    text stream
    text msg
  }
```

### Beacon

```mermaid
erDiagram
  beacon_datasets ||--o{ beacon_variants : has
  beacon_datasets {
    text id PK
    text name
  }
  beacon_variants {
    bigint id PK
    text dataset_id FK
    text reference_name
    bigint start
    text ref
    text alt
  }
```

---

## Async streaming architecture

Crypt4GH download uses a **zero-copy async stream chain**:

1. **Storage::get()** — Returns an `AsyncRead` of the encrypted object (e.g. from S3 or local disk).
2. **Crypt4GHDecryptHeader** — Reads and decrypts the header with the node’s secret key; exposes the same body stream.
3. **Crypt4GHReEncryptHeader** — Re-encrypts only the header for the client’s public key; body is passed through.
4. **ByteRangeFilter** — Applies optional range requests.
5. **axum::Body** — Stream is sent to the client over TLS.

**Peak memory** is on the order of the header size (~64 KB), regardless of file size. The file body is never fully loaded into memory.

---

## Configuration system

Config is resolved in this order (later overrides earlier):

1. Built-in defaults  
2. `/etc/ferrum/config.toml`  
3. `./config.toml` or `~/.ferrum/config.toml`  
4. Path from `FERRUM_CONFIG` env var  
5. Environment variables `FERRUM_*` (nested keys use double underscore, e.g. `FERRUM_DATABASE__URL`)

Example `config.toml`:

```toml
bind = "0.0.0.0:8080"

[database]
url = "postgres://user:pass@host:5432/ferrum"
run_migrations = true

[storage]
backend = "s3"
s3_endpoint = "http://minio:9000"
s3_bucket = "ferrum"

[auth]
require_auth = true
jwks_url = "https://auth.example.com/realms/ferrum/protocol/openid-connect/certs"

[services]
enable_drs = true
enable_wes = true
# ...
```

---

## Deployment topologies

### a) Single-node (MacBook demo)

```mermaid
flowchart LR
  subgraph Mac["MacBook"]
    G[ferrum-gateway]
    PG[(PostgreSQL)]
    M[MinIO]
    K[Keycloak]
  end
  Browser --> G
  G --> PG
  G --> M
  G --> K
```

### b) Single-node production

```mermaid
flowchart LR
  subgraph Server["Single server"]
    N[nginx TLS]
    G[ferrum-gateway]
    PG[(PostgreSQL)]
    M[MinIO]
  end
  Internet --> N
  N --> G
  G --> PG
  G --> M
```

### c) Distributed HPC cluster

```mermaid
flowchart TB
  subgraph Head["Head node"]
    N[nginx]
    G[ferrum-gateway]
  end
  subgraph Compute["Compute nodes"]
    C1[SLURM worker 1]
    C2[SLURM worker 2]
  end
  subgraph Data["Data layer"]
    PG[(PostgreSQL)]
    M[MinIO cluster]
    NFS[NFS]
  end
  Users --> N
  N --> G
  G --> PG
  G --> M
  G --> C1
  G --> C2
  C1 --> NFS
  C2 --> NFS
```

---

*[← Documentation index](README.md)*
