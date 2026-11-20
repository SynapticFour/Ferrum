#!/usr/bin/env python3
# SPDX-License-Identifier: Apache-2.0
"""Regenerate the CRAM and BCF fixtures for tests/regions.rs with htslib (pysam).

Usage: python3 make_fixtures.py   (needs pysam; writes next to this script)

The data is synthetic. The reference sequence is a formula (see `ref_base`) that
tests/regions.rs computes the same way, so no FASTA is stored in the repo.
CRAM is written with small slices (one slice per container) so that a region
ticket covers only a few containers; BCF is block-gzipped by htslib and indexed
with CSI, the layout bcftools produces.
"""
import os
import pysam

HERE = os.path.dirname(os.path.abspath(__file__))
REFS = [("chr1", 120_000), ("chr2", 60_000)]
READ_LEN = 50
STEP = 20


def ref_base(ref_id, i):
    return "ACGT"[(i * 7 + i // 13 + ref_id * 3) % 4]


def ref_seq(ref_id, length):
    return "".join(ref_base(ref_id, i) for i in range(length))


def write_fasta(path):
    with open(path, "w") as f:
        for ref_id, (name, length) in enumerate(REFS):
            f.write(f">{name}\n")
            s = ref_seq(ref_id, length)
            for i in range(0, length, 60):
                f.write(s[i : i + 60] + "\n")
    pysam.faidx(path)


def make_cram(tmp_fasta):
    path = os.path.join(HERE, "sample.cram")
    header = {
        "HD": {"VN": "1.6", "SO": "coordinate"},
        "SQ": [{"SN": n, "LN": l} for n, l in REFS],
    }
    with pysam.AlignmentFile(
        path,
        "wc",
        header=header,
        reference_filename=tmp_fasta,
        format_options=[b"seqs_per_slice=200", b"slices_per_container=1", b"version=3.0"],
    ) as out:
        for ref_id, (name, length) in enumerate(REFS):
            seq = ref_seq(ref_id, length)
            for n, pos in enumerate(range(0, length - READ_LEN, STEP)):
                a = pysam.AlignedSegment(out.header)
                a.query_name = f"c{ref_id}_{n}"
                a.reference_id = ref_id
                a.reference_start = pos
                a.mapping_quality = 60
                a.cigartuples = [(0, READ_LEN)]
                bases = list(seq[pos : pos + READ_LEN])
                if n % 3 == 0:  # one mismatch in every third read
                    k = n % READ_LEN
                    bases[k] = "A" if bases[k] != "A" else "C"
                a.query_sequence = "".join(bases)
                a.query_qualities = pysam.qualitystring_to_array("I" * READ_LEN)
                out.write(a)
    pysam.index(path)  # writes sample.cram.crai


def make_bcf():
    path = os.path.join(HERE, "calls.bcf")
    header = pysam.VariantHeader()
    for name, length in REFS:
        header.contigs.add(name, length=length)
    header.add_meta("INFO", items=[("ID", "DP"), ("Number", 1), ("Type", "Integer"), ("Description", "Depth")])
    with pysam.VariantFile(path, "wb", header=header) as out:
        for ref_id, (name, length) in enumerate(REFS):
            for n, pos in enumerate(range(10, length, 40)):
                rec = out.new_record(
                    contig=name,
                    start=pos,
                    alleles=(ref_base(ref_id, pos), "T" if ref_base(ref_id, pos) != "T" else "G"),
                    id=f"v{ref_id}_{n}",
                )
                rec.info["DP"] = 10 + n % 50
                out.write(rec)
    pysam.tabix_index(path, preset="bcf", csi=True, force=True)  # writes calls.bcf.csi


if __name__ == "__main__":
    import tempfile

    with tempfile.TemporaryDirectory() as d:
        fa = os.path.join(d, "ref.fa")
        write_fasta(fa)
        make_cram(fa)
    make_bcf()
    for f in sorted(os.listdir(HERE)):
        print(f, os.path.getsize(os.path.join(HERE, f)))
