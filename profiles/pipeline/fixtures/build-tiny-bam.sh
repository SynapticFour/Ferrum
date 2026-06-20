#!/usr/bin/env bash
# Build tiny.bam + tiny.bam.bai for pilot seed (requires samtools).
# Aligns to chr22:2000 so TinyGermlineHC interval chr22:1700-2300 matches reference + truth.
set -euo pipefail
cd "$(dirname "$0")"
command -v samtools >/dev/null || { echo "samtools required" >&2; exit 1; }

if [[ ! -f pilot-ref.fa || ! -f pilot-ref.fa.fai ]]; then
  bash build-pilot-ref-bundle.sh
fi

REF_LEN="$(awk 'NR==2 {print length($0)}' pilot-ref.fa)"
SNP_POS=2000

cat > tiny.sam <<EOF
@HD	VN:1.6	SO:coordinate
@SQ	SN:chr22	LN:${REF_LEN}
@RG	ID:rg1	SM:pilot-demo-01
read1	0	chr22	${SNP_POS}	60	39M	*	0	0	GATTACGATCGATCGATCGATCGATCGATCGATCGATCG	IIIIIIIIIIIIIIIIIIIIIIIIIIIIIIIIIIIIIII	RG:Z:rg1
EOF

samtools view -bS tiny.sam -o tiny.bam
samtools index tiny.bam
samtools quickcheck tiny.bam
rm -f tiny.sam
echo "Wrote tiny.bam and tiny.bam.bai (chr22:${SNP_POS})"
