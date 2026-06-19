#!/usr/bin/env bash
# Minimal chr22 reference + gzipped truth VCF for TinyGermlineHC pilot seed.
# Requires: python3, samtools, bgzip, tabix (htslib).
set -euo pipefail
cd "$(dirname "$0")"

for cmd in python3 samtools bgzip tabix; do
  command -v "$cmd" >/dev/null || { echo "build-pilot-ref-bundle: need $cmd" >&2; exit 1; }
done

python3 - <<'PY'
from pathlib import Path

# 3.7 kb chr22 slice — covers TinyGermlineHC interval 22:1700-2300 and truth at 2000.
seq = ("ACGT" * 400) + ("N" * 500) + ("TGCA" * 400)
Path("pilot-ref.fa").write_text(f">chr22 pilot reference slice\n{seq}\n")

snp_pos = 2000
ref = seq[snp_pos - 1]
alt = "G" if ref != "G" else "C"
Path("pilot-truth.vcf").write_text(
    "##fileformat=VCFv4.2\n"
    "#CHROM\tPOS\tID\tREF\tALT\tQUAL\tFILTER\tINFO\n"
    f"chr22\t{snp_pos}\t.\t{ref}\t{alt}\t60\tPASS\t.\n"
)
print(f"pilot-ref.fa ({len(seq)} bp), truth chr22:{snp_pos} {ref}>{alt}")
PY

samtools faidx pilot-ref.fa
bgzip -f pilot-truth.vcf
tabix -p vcf pilot-truth.vcf.gz

samtools quickcheck pilot-ref.fa 2>/dev/null || true
echo "Wrote pilot-ref.fa, pilot-ref.fa.fai, pilot-truth.vcf.gz, pilot-truth.vcf.gz.tbi"
