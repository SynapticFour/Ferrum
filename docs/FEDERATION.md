# Federated Beacon (P2P)

Ferrum supports **peer-to-peer federated Beacon queries** without a central coordinator. Each node maintains its own peer list and fans out queries when explicitly requested.

## Enable federation

Federation is **disabled by default**.

```toml
[federation]
enabled = true
fan_out_parallel = true
aggregate_strategy = "union"   # union | intersection | local_first
peer_requests_per_minute = 10

[[federation.peers]]
name = "KEMRI-Wellcome"
beacon_endpoint = "https://ferrum.kemri-wellcome.org/ga4gh/beacon/v2"
timeout_ms = 5000
# service_token = "..."   # optional Bearer for peer auth

[[federation.peers]]
name = "IRESSEF-Dakar"
beacon_endpoint = "https://ferrum.iressef.org/ga4gh/beacon/v2"
timeout_ms = 8000
```

## Query API

Local-only (default — unchanged HelixTest behaviour):

```http
GET /ga4gh/beacon/v2/g_variants?referenceName=1&start=1000&referenceBases=A&alternateBases=T
```

Federated fan-out (opt-in):

```http
GET /ga4gh/beacon/v2/g_variants?federate=true&referenceName=1&start=1000&referenceBases=A&alternateBases=T
```

### Behaviour

- Peers are queried **in parallel** when `fan_out_parallel = true`.
- Unreachable peers are **non-fatal**; local results are returned and `meta.warnings` lists failed peers.
- **`union`** (default): `exists = local OR any peer`; counts summed.
- **`intersection`**: all responders must agree.
- **`local_first`**: response reflects local data; peer payloads appear in `meta.peerResults`.
- Per-peer rate limit defaults to **10 requests/minute** (`peer_requests_per_minute`).

## Auth

Configure `service_token` per peer for Bearer authentication. Open-access public Beacons can omit the token.

## Residency audit

Each successful peer fan-out appends a `peer_query_sent` entry to the [data residency audit log](DATA-RESIDENCY-AUDIT.md).
