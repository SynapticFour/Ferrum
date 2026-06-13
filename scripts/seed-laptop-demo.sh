#!/usr/bin/env bash
# Seed minimal demo data into a running Ferrum Laptop Mode instance (DRS + Beacon).
# Usage: BASE_URL=http://127.0.0.1:8080 ./scripts/seed-laptop-demo.sh
set -euo pipefail

BASE_URL="${BASE_URL:-http://127.0.0.1:8080}"
BASE_URL="${BASE_URL%/}"
SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
DATA="${DATA_DIR:-/tmp/ferrum-seed-data}"
DB="${FERRUM_SQLITE_PATH:-${HOME}/.ferrum/ferrum.db}"

mkdir -p "$DATA"

echo "==> Generating synthetic chr22 subset in $DATA"
python3 - "$DATA" <<'PY'
import random
from pathlib import Path
import sys
data = Path(sys.argv[1])
seq = "A" * 500 + "C" * 500 + "G" * 500 + "T" * 500 + "N" * 1000 + "A" * 1000
snp = 2000
ref = seq[snp - 1]
alt = "G" if ref != "G" else "C"
(data / "ref_slice.fa").write_text(">22\n" + seq + "\n")
(data / "truth.vcf").write_text(
    f"##fileformat=VCFv4.2\n#CHROM\tPOS\tID\tREF\tALT\tQUAL\tFILTER\tINFO\n"
    f"22\t{snp}\t.\t{ref}\t{alt}\t60\t.\t.\n"
)
print(f"SNP chr22:{snp} {ref}>{alt}")
PY

if command -v samtools >/dev/null 2>&1; then
  samtools faidx "$DATA/ref_slice.fa"
fi

echo "==> Ingesting DRS objects"
REF_ID=$(curl -fsS -X POST "$BASE_URL/ga4gh/drs/v1/ingest/file" \
  -F "file=@$DATA/ref_slice.fa" -F "name=ref_slice.fa" | python3 -c "import sys,json; print(json.load(sys.stdin)['id'])")
echo "  ref_slice.fa → $REF_ID"

if [[ -f "$DB" ]] && command -v sqlite3 >/dev/null 2>&1; then
  echo "==> Seeding Beacon (SQLite $DB)"
  sqlite3 "$DB" <<'SQL'
INSERT OR IGNORE INTO beacon_datasets (id, name, description, assembly_id)
VALUES ('demo-public', 'Demo Public', 'ferrum demo seed', 'GRCh37');
DELETE FROM beacon_variants WHERE dataset_id = 'demo-public';
INSERT INTO beacon_variants (dataset_id, chromosome, start, "end", reference, alternate, variant_type)
VALUES ('demo-public', '22', 2000, 2000, 'T', 'G', 'SNV');
SQL
  echo "  Beacon chr22:2000 T>G ready"
else
  echo "warning: sqlite3 or $DB not found — skip Beacon seed (ingest DRS only)"
fi

echo "==> Done. Try Beacon query chr22:2000 T>G or open Data Browser for object $REF_ID"
