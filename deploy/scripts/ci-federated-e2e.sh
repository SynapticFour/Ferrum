#!/usr/bin/env bash
# Federated access smoke: gateway ADS introspect gate + DRS proxy (in-process mocks).
set -euo pipefail
cd "$(dirname "$0")/../.."
cargo test -p ferrum-gateway --test federated_access --features full,discovery
