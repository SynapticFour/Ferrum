#!/usr/bin/env bash
# Deprecated wrapper — use ./scripts/build-edge-native.sh (ADR-018).
echo "[ferrum] Warning: build-laptop-native.sh is deprecated; use build-edge-native.sh" >&2
exec "$(dirname "$0")/build-edge-native.sh" "$@"
