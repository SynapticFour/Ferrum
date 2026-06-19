#!/usr/bin/env bash
# Build tiny.bam + tiny.bam.bai for pilot seed (requires samtools).
set -euo pipefail
cd "$(dirname "$0")"
command -v samtools >/dev/null || { echo "samtools required" >&2; exit 1; }

cat > tiny-ref.fa <<'EOF'
>chr1
GATTACGATCGATCGATCGATCGATCGATCGATCGATCGATCGATCGATCGATCGATCGATCGATCGATCG
EOF
samtools faidx tiny-ref.fa

cat > tiny.sam <<'EOF'
@HD	VN:1.6	SO:coordinate
@SQ	SN:chr1	LN:71
@RG	ID:rg1	SM:pilot-demo-01
read1	0	chr1	1	60	39M	*	0	0	GATTACGATCGATCGATCGATCGATCGATCGATCGATCG	IIIIIIIIIIIIIIIIIIIIIIIIIIIIIIIIIIIIIII	RG:Z:rg1
EOF

samtools view -bS tiny.sam -o tiny.bam
samtools index tiny.bam
samtools quickcheck tiny.bam
rm -f tiny-ref.fa tiny-ref.fa.fai tiny.sam
echo "Wrote tiny.bam and tiny.bam.bai"
