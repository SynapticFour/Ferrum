# Federation walkthrough on two Edge nodes

This walkthrough starts two Ferrum Edge nodes on one computer and asks one of them about a variant that only the other holds, so a reader can see the answer cross between nodes while each file stays where it was loaded.

For someone evaluating federated Beacon on a laptop, without Docker and without a database server.

**Status: No public status is recorded.** The claims registry has no row for this walkthrough.

## What it does

- It starts two `ferrum-gateway` processes built with the `edge` feature (SQLite and local objects). Each one names the other as a federation peer.
- It loads one small synthetic VCF into each node through DRS ingest (`/ga4gh/drs/v1/ingest/file`) and indexes it into Beacon with `ferrum pipeline index-beacon`.
- It runs five checks and writes `report.md` and `report.json`:
  1. Node A alone does not have a variant that only node B holds.
  2. Node A with `federate=true` reports it, with B's count and no peer warning.
  3. Node A's residency audit log has a `peer_query_sent` row for B with `data_left_node: false`.
  4. Node B's audit log has the `beacon_query` it answered.
  5. Both audit chains verify, and B's DRS object is not in A's object list.

## Limits

- Both nodes run on one computer. The run shows the protocol and the two logs. It does not show a network between countries.
- The data is synthetic: three SNVs in two files. No person's data.
- Sign-in is off (`require_auth = false`) so the walkthrough needs no account. A real node keeps the default, `true`.
- Edge nodes serve DRS, Beacon, htsget, and Crypt4GH. WES, TES, and TRS are not part of this walkthrough.
- A passing run is a technical signal for the version it ran on. It is not GA4GH certification and not a legal assessment of data residency.

## Quick start

Prerequisites: the Rust toolchain from `rust-toolchain.toml`, `curl`, and `python3`. No Docker.

```bash
demo/federation-edge/run.sh
```

The first run builds the Edge gateway and the `ferrum` CLI, which takes a few minutes. You should see `5 of 5 checks passed.` and the paths of the two report files. The nodes keep running on ports 18101 and 18102, so you can repeat the queries yourself.

Stop the nodes with `demo/federation-edge/run.sh stop`. Remove the working folder with `demo/federation-edge/run.sh clean`.

To use binaries you already built, set `FERRUM_GATEWAY_BIN` and `FERRUM_CLI_BIN`. Ports, node names, and the working folder are set with `FED_PORT_A`, `FED_PORT_B`, `FED_NAME_A`, `FED_NAME_B`, and `FED_WORKDIR`.

## See also

- [Federated Beacon](../../docs/FEDERATION.md)
- [Data residency audit log](../../docs/DATA-RESIDENCY-AUDIT.md)
- [Installation, Edge mode](../../docs/INSTALLATION.md)
