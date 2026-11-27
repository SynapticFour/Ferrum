#!/usr/bin/env bash
# SPDX-License-Identifier: Apache-2.0
#
# Two Ferrum Edge nodes on one computer, federated Beacon between them.
# No Docker, no database server. Synthetic data only.
#
#   ./run.sh          build if needed, start both nodes, run the walkthrough, write the report
#   ./run.sh stop     stop both nodes
#   ./run.sh clean    stop and delete the working folder
#
# Environment:
#   FERRUM_GATEWAY_BIN   edge gateway binary (default: build with cargo)
#   FERRUM_CLI_BIN       ferrum CLI binary   (default: build with cargo)
#   FED_WORKDIR          working folder      (default: ./.work next to this script)
#   FED_PORT_A / FED_PORT_B   ports          (default: 18101 / 18102)
#   FED_NAME_A / FED_NAME_B   node names     (default: node-de / node-za)
set -euo pipefail

HERE="$(cd "$(dirname "$0")" && pwd)"
ROOT="$(cd "$HERE/../.." && pwd)"
WORK="${FED_WORKDIR:-$HERE/.work}"
PORT_A="${FED_PORT_A:-18101}"
PORT_B="${FED_PORT_B:-18102}"
NAME_A="${FED_NAME_A:-node-de}"
NAME_B="${FED_NAME_B:-node-za}"

say()  { printf '\n\033[1m%s\033[0m\n' "$*"; }
info() { printf '  %s\n' "$*"; }
die()  { printf 'federation-edge: %s\n' "$*" >&2; exit 1; }

stop_nodes() {
  for n in a b; do
    pidfile="$WORK/$n/gateway.pid"
    if [ -f "$pidfile" ]; then
      kill "$(cat "$pidfile")" 2>/dev/null || true
      rm -f "$pidfile"
    fi
  done
}

case "${1:-run}" in
  stop)  stop_nodes; echo "Both nodes stopped."; exit 0 ;;
  clean) stop_nodes; rm -rf "$WORK"; echo "Stopped and removed $WORK."; exit 0 ;;
  run)   ;;
  *)     die "unknown command '$1' (use: run, stop, clean)" ;;
esac

command -v curl >/dev/null || die "curl is required"
command -v python3 >/dev/null || die "python3 is required"

# --- 1. Binaries -------------------------------------------------------------
say "1/6  Binaries"
target_dir() {
  (cd "$ROOT" && cargo metadata --format-version 1 --no-deps \
    | python3 -c 'import json,sys; print(json.load(sys.stdin)["target_directory"])')
}
GATEWAY="${FERRUM_GATEWAY_BIN:-}"
CLI="${FERRUM_CLI_BIN:-}"
if [ -z "$GATEWAY" ] || [ -z "$CLI" ]; then
  command -v cargo >/dev/null || die "cargo not found; set FERRUM_GATEWAY_BIN and FERRUM_CLI_BIN"
  TD="$(target_dir)"
  if [ -z "$GATEWAY" ]; then
    info "building the Edge gateway (first run takes a few minutes)"
    (cd "$ROOT" && cargo build -q -p ferrum-gateway --no-default-features --features edge)
    GATEWAY="$TD/debug/ferrum-gateway"
  fi
  if [ -z "$CLI" ]; then
    info "building the ferrum CLI"
    (cd "$ROOT" && cargo build -q -p ferrum-cli)
    CLI="$TD/debug/ferrum"
  fi
fi
[ -x "$GATEWAY" ] || die "gateway binary not found: $GATEWAY"
[ -x "$CLI" ] || die "CLI binary not found: $CLI"
info "gateway: $GATEWAY"
info "cli:     $CLI"

# --- 2. Two nodes --------------------------------------------------------------
say "2/6  Start two Edge nodes: $NAME_A (:$PORT_A) and $NAME_B (:$PORT_B)"
stop_nodes
rm -rf "$WORK"
mkdir -p "$WORK/a/objects" "$WORK/b/objects"

write_config() { # dir port peer_name peer_port
  cat >"$1/config.toml" <<EOF
# Demo only: anonymous access is on so the walkthrough needs no sign-in.
# A real node keeps require_auth = true (the default).
bind = "127.0.0.1:$2"

[auth]
require_auth = false

[africa]
offline_first = true
sqlite_path = "$1/ferrum.db"
objects_path = "$1/objects"

[federation]
enabled = true
fan_out_parallel = true
aggregate_strategy = "union"
peer_requests_per_minute = 60

[[federation.peers]]
name = "$3"
beacon_endpoint = "http://127.0.0.1:$4/ga4gh/beacon/v2"
timeout_ms = 5000
EOF
}
write_config "$WORK/a" "$PORT_A" "$NAME_B" "$PORT_B"
write_config "$WORK/b" "$PORT_B" "$NAME_A" "$PORT_A"

for n in a b; do
  FERRUM_CONFIG="$WORK/$n/config.toml" nohup "$GATEWAY" >"$WORK/$n/gateway.log" 2>&1 &
  echo $! >"$WORK/$n/gateway.pid"
done
trap 'echo; echo "Nodes are still running. Stop them with: $0 stop"' EXIT

wait_health() {
  for _ in $(seq 1 60); do
    curl -sf "http://127.0.0.1:$1/health" >/dev/null 2>&1 && return 0
    sleep 0.5
  done
  die "node on :$1 did not answer /health (see $WORK/*/gateway.log)"
}
wait_health "$PORT_A"; wait_health "$PORT_B"
info "both nodes answer /health"

# --- 3. Synthetic data ----------------------------------------------------------
say "3/6  Load synthetic variants (each node keeps its own file)"
write_vcf() { # file sample rows...
  local f="$1" s="$2"; shift 2
  {
    printf '##fileformat=VCFv4.2\n##source=ferrum-federation-edge-demo (synthetic)\n'
    printf '#CHROM\tPOS\tID\tREF\tALT\tQUAL\tFILTER\tINFO\tFORMAT\t%s\n' "$s"
    for row in "$@"; do printf '%s\t.\tPASS\t.\tGT\t0/1\n' "$row"; done
  } >"$f"
}
# Shared: chr1:1000 A>T. Only A: chr7:117559590 G>A. Only B: chr11:5227002 T>A.
write_vcf "$WORK/a/cohort-a.vcf" SYN-A-001 \
  $'chr1\t1000\t.\tA\tT\t50' $'chr7\t117559590\t.\tG\tA\t50'
write_vcf "$WORK/b/cohort-b.vcf" SYN-B-001 \
  $'chr1\t1000\t.\tA\tT\t50' $'chr11\t5227002\t.\tT\tA\t50'

ingest_and_index() { # node port dataset
  local json id
  json="$(curl -sf -F "file=@$WORK/$1/cohort-$1.vcf;type=text/vcf" \
    "http://127.0.0.1:$2/ga4gh/drs/v1/ingest/file")" || die "DRS ingest failed on :$2"
  id="$(printf '%s' "$json" | python3 -c 'import json,sys; print(json.load(sys.stdin)["id"])')"
  "$CLI" pipeline index-beacon --object-id "$id" --dataset "$3" \
    --config "$WORK/$1/config.toml" >"$WORK/$1/index.log" 2>&1 \
    || die "index-beacon failed (see $WORK/$1/index.log)"
  printf '%s' "$id" >"$WORK/$1/object_id"
  info "$3: DRS object $id, indexed into Beacon"
}
ingest_and_index a "$PORT_A" "cohort-$NAME_A"
ingest_and_index b "$PORT_B" "cohort-$NAME_B"

# --- 4. Walkthrough -------------------------------------------------------------
say "4/6  Walkthrough"
export WORK PORT_A PORT_B NAME_A NAME_B
python3 "$HERE/walkthrough.py"

# --- 5. Report ------------------------------------------------------------------
say "5/6  Report"
info "$WORK/report.md"
info "$WORK/report.json"

say "6/6  Explore yourself"
info "Beacon A: http://127.0.0.1:$PORT_A/ga4gh/beacon/v2/g_variants?assemblyId=GRCh38&referenceName=chr11&start=5227002&referenceBases=T&alternateBases=A&federate=true"
info "Audit A:  http://127.0.0.1:$PORT_A/api/v1/audit/residency/verify"
info "Stop:     $0 stop"
