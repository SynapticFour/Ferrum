#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "$0")/.." && pwd)"
FAILED=0

pass() { echo "PASS: $1"; }
fail() { echo "FAIL: $1"; FAILED=1; }

check_present() {
  local pattern="$1"
  local path="$2"
  local label="$3"
  if python3 - "$pattern" "$path" <<'PY'
import pathlib, re, sys
pat = re.compile(sys.argv[1])
scope = pathlib.Path(sys.argv[2])
files = [scope] if scope.is_file() else [p for p in scope.rglob("*") if p.is_file()]
for f in files:
    try:
        if pat.search(f.read_text(encoding="utf-8", errors="ignore")):
            sys.exit(0)
    except Exception:
        pass
sys.exit(1)
PY
  then
    pass "${label}"
  else
    fail "${label}"
  fi
}

check_absent() {
  local pattern="$1"
  local path="$2"
  local label="$3"
  if python3 - "$pattern" "$path" <<'PY'
import pathlib, re, sys
pat = re.compile(sys.argv[1])
scope = pathlib.Path(sys.argv[2])
files = [scope] if scope.is_file() else [p for p in scope.rglob("*") if p.is_file()]
for f in files:
    try:
        if pat.search(f.read_text(encoding="utf-8", errors="ignore")):
            sys.exit(0)
    except Exception:
        pass
sys.exit(1)
PY
  then
    fail "${label}"
  else
    pass "${label}"
  fi
}

echo "Running docs consistency checks in ${ROOT_DIR}"

check_present "docs/deployment/README\\.md" "${ROOT_DIR}/README.md" "README links deployment matrix"
check_present "INSTALLATION\\.md" "${ROOT_DIR}/docs/README.md" "Docs index points to INSTALLATION"
check_present "docs/deployment/UPDATE-SOP\\.md" "${ROOT_DIR}/docs/deployment/README.md" "Deployment doc links SOP"
check_present "docs/deployment/RELEASE-CHECKLIST\\.md" "${ROOT_DIR}/docs/deployment/README.md" "Deployment doc links release checklist"
check_absent "in Vorbereitung" "${ROOT_DIR}/docs/deployment" "No stale 'in preparation' text in deployment docs"
check_present "deployment_preflight\\.sh" "${ROOT_DIR}/docs/deployment" "Preflight script referenced in deployment docs"

if [[ "${FAILED}" -eq 1 ]]; then
  echo "Docs consistency checks failed."
  exit 1
fi

echo "Docs consistency checks passed."

