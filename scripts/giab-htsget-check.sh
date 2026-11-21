#!/usr/bin/env bash
# SPDX-License-Identifier: Apache-2.0
# Abnahme der htsget-Regionen an echten GIAB-Daten (HG002, GRCh38), auf einem Mac.
#
# Was passiert:
#   1. samtools/bcftools holen Ausschnitte aus öffentlichen GIAB-Dateien (nur der Ausschnitt wird
#      übertragen, über den Index): Illumina 2x250 BAM, ONT-Ultralong-BAM (Reads länger als ein
#      BGZF-Block) und die v4.2.1-Benchmark-VCF; daraus BAM+BAI, VCF.gz+TBI, BCF+CSI, CRAM+CRAI.
#   2. Das veröffentlichte Release-Binary Ferrum v0.3.12 (Edge, für deinen Mac) wird von GitHub
#      geladen, gegen SHA256SUMS.txt des Releases geprüft und lokal auf 127.0.0.1:18080 gestartet,
#      mit eigener SQLite-Datei in einem Temp-Ordner.
#   3. Die Dateien werden über DRS hochgeladen; für viele Regionen werden htsget-Tickets geholt,
#      alle URLs geladen, zusammengesetzt und mit samtools/bcftools gegen die Originaldatei
#      verglichen: alle Records der Region vorhanden, keine doppelt, keine htslib-Warnung.
#   4. Ausgabe: Tabelle + Bytes im Ticket vs. Dateigröße. Bitte die Ausgabe zurückschicken.
#
# Nichts wird gepusht, nichts in deinen Checkouts geändert. Daten und Binary liegen unter
# ~/.cache/ferrum-giab-check (GIAB_WORK überschreibt das).
# Lädt herunter: das Release-Archiv ferrum-gateway-<arch>-apple-darwin.tar.gz (~12 MB) und
# SHA256SUMS.txt von github.com/SynapticFour/Ferrum (Release v0.3.12), dazu die GIAB-Ausschnitte
# (einige hundert MB) von ftp-trace.ncbi.nlm.nih.gov.
# Braucht: samtools, bcftools, htslib (brew install samtools bcftools), gh, python3, curl.
# Dauer: ca. 10-20 Min (vor allem der Download der Ausschnitte).
# Aufruf:  bash scripts/giab-htsget-check.sh
#          FERRUM_BIN=/pfad/ferrum-gateway   nimmt ein vorhandenes Binary (z. B. Linux)
#          FERRUM_VERSION=v0.3.12            Release, dessen macOS-Binary geladen wird
# Bericht des Laufs vom 21.11.2026: docs/evidence/2026-11-21-htsget-giab/README.md
set -Eeuo pipefail
trap 'echo "STOP: Fehler in Zeile $LINENO: $BASH_COMMAND" >&2' ERR
BASE="$HOME/devel/SynapticFour"
WORK="${GIAB_WORK:-$HOME/.cache/ferrum-giab-check}"
PORT=18080

stop() { echo; echo "STOP: $*"; exit 1; }
for t in samtools bcftools bgzip tabix python3 curl gh shasum tar; do
  command -v "$t" >/dev/null 2>&1 || stop "$t fehlt (brew install samtools bcftools gh)"
done
VERSION="${FERRUM_VERSION:-v0.3.12}"

GIAB=https://ftp-trace.ncbi.nlm.nih.gov/ReferenceSamples/giab
ILLUMINA="${GIAB_ILLUMINA:-$GIAB/data/AshkenazimTrio/HG002_NA24385_son/NIST_Illumina_2x250bps/novoalign_bams/HG002.GRCh38.2x250.bam}"
ONT="${GIAB_ONT:-$GIAB/data/AshkenazimTrio/HG002_NA24385_son/Ultralong_OxfordNanopore/guppy-V3.2.4_2020-01-22/HG002_GRCh38_ONT-UL_GIAB_20200122.phased.bam}"
VCF_CANDIDATES=(
  ${GIAB_VCF:+"$GIAB_VCF"}
  "$GIAB/release/AshkenazimTrio/HG002_NA24385_son/NISTv4.2.1/GRCh38/HG002_GRCh38_1_22_v4.2.1_benchmark.vcf.gz"
  "https://ftp-trace.ncbi.nlm.nih.gov/giab/ftp/release/AshkenazimTrio/HG002_NA24385_son/NISTv4.2.1/GRCh38/HG002_GRCh38_1_22_v4.2.1_benchmark.vcf.gz"
)

mkdir -p "$WORK/data"
cd "$WORK/data"

echo "== 1/4 GIAB-Ausschnitte (HG002, GRCh38)"
# Header erst ganz lesen: awk beendet sich beim ersten Treffer, und ein Pipe-Abbruch
# (SIGPIPE, Exit 141) hätte mit pipefail das Skript ohne Meldung beendet.
chrom_of() {
  local h
  h="$(samtools view -H "$1")" || { echo "  samtools konnte den Header nicht lesen: $1" >&2; return 1; }
  awk -F'\t' '$1=="@SQ"{for(i=2;i<=NF;i++) if($i=="SN:chr20"||$i=="SN:20"){sub("SN:","",$i); print $i; exit}}' <<< "$h"
}
if [ ! -s illumina.bam.bai ]; then
  C="$(chrom_of "$ILLUMINA" || true)"; [ -n "$C" ] || stop "chr20 nicht im Header der Illumina-BAM (URL erreichbar?)"
  echo "  Illumina 2x250, $C:30,000,000-31,000,000"
  samtools view -b -o illumina.bam "$ILLUMINA" "$C:30000000-31000000"
  samtools index illumina.bam
  rm -f HG002.GRCh38.2x250.bam.bai
fi
if [ ! -s ont.bam.bai ]; then
  C="$(chrom_of "$ONT" || true)"; [ -n "$C" ] || stop "chr20 nicht im Header der ONT-BAM"
  echo "  ONT ultralong, $C:30,000,000-32,000,000"
  samtools view -b -o ont.bam "$ONT" "$C:30000000-32000000"
  samtools index ont.bam
  rm -f HG002_GRCh38_ONT-UL_GIAB_20200122.phased.bam.bai
fi
if [ ! -s benchmark.vcf.gz.tbi ]; then
  ok=""
  for u in "${VCF_CANDIDATES[@]}"; do
    if bcftools view -h "$u" >/dev/null 2>&1; then ok="$u"; break; fi
  done
  [ -n "$ok" ] || stop "Benchmark-VCF nicht erreichbar"
  echo "  Benchmark-VCF v4.2.1, chr20 + chr21"
  bcftools view -r chr20,chr21 -Oz -o benchmark.vcf.gz "$ok"
  tabix -p vcf benchmark.vcf.gz
  bcftools view -Ob -o benchmark.bcf benchmark.vcf.gz
  bcftools index --csi benchmark.bcf
  rm -f HG002_GRCh38_1_22_v4.2.1_benchmark.vcf.gz.tbi
fi
ls -l "$WORK/data" | awk 'NR>1{printf "  %-26s %12d\n", $9, $5}'

echo; echo "== 2/4 Ferrum $VERSION (Release-Binary, Edge)"
if [ -n "${FERRUM_BIN:-}" ]; then BIN="$FERRUM_BIN"; echo "  vorhandenes Binary: $BIN"; else
case "$(uname -m)" in arm64|aarch64) ARCH=aarch64 ;; x86_64) ARCH=x86_64 ;; *) stop "unbekannte Architektur $(uname -m)" ;; esac
ASSET="ferrum-gateway-$ARCH-apple-darwin.tar.gz"
REL="$WORK/release-$VERSION"; mkdir -p "$REL"; cd "$REL"
if [ ! -s "$ASSET" ]; then
  gh release download "$VERSION" -R SynapticFour/Ferrum -p "$ASSET" -p SHA256SUMS.txt --clobber
fi
grep " $ASSET\$" SHA256SUMS.txt | shasum -a 256 -c - || stop "Prüfsumme von $ASSET passt nicht"
tar -xzf "$ASSET"
BIN="$REL/ferrum-gateway"; [ -x "$BIN" ] || stop "ferrum-gateway fehlt im Archiv"
echo "  $ASSET sha256 $(shasum -a 256 "$ASSET" | cut -c1-16)…  (Release $VERSION)"
fi

echo; echo "== 3/4 Ferrum starten und Dateien über DRS hochladen"
RUN="$(mktemp -d)"
mkdir -p "$RUN/objects"
cat > "$RUN/config.toml" <<EOF
bind = "127.0.0.1:$PORT"
[auth]
require_auth = false
[africa]
offline_first = true
sqlite_path = "$RUN/ferrum.db"
objects_path = "$RUN/objects"
EOF
FERRUM_CONFIG="$RUN/config.toml" FERRUM_PUBLIC_BASE_URL="http://127.0.0.1:$PORT" "$BIN" > "$RUN/gateway.log" 2>&1 &
GW=$!
trap 'kill $GW 2>/dev/null || true' EXIT
for _ in $(seq 1 120); do curl -sf "http://127.0.0.1:$PORT/health" >/dev/null && break; sleep 0.5; done
curl -sf "http://127.0.0.1:$PORT/health" >/dev/null || { tail -20 "$RUN/gateway.log"; stop "Ferrum startet nicht"; }
: > "$RUN/ids.txt"
for f in illumina.bam:application/vnd.ga4gh.bam illumina.bam.bai:application/octet-stream \
         ont.bam:application/vnd.ga4gh.bam ont.bam.bai:application/octet-stream \
         benchmark.vcf.gz:application/vnd.ga4gh.vcf benchmark.vcf.gz.tbi:application/octet-stream \
         benchmark.bcf:application/vnd.ga4gh.bcf benchmark.bcf.csi:application/octet-stream; do
  n="${f%%:*}"; t="${f#*:}"
  id="$(curl -sf -F "file=@$WORK/data/$n;type=$t;filename=$n" "http://127.0.0.1:$PORT/ga4gh/drs/v1/ingest/file" | python3 -c 'import json,sys; print(json.load(sys.stdin)["id"])')"
  echo "$n $id" >> "$RUN/ids.txt"
done
echo "  $(wc -l < "$RUN/ids.txt" | tr -d ' ') Objekte hochgeladen"

echo; echo "== 4/4 Tickets prüfen"
python3 - "$RUN" "$WORK/data" "$PORT" <<'PY'
import base64, json, os, random, subprocess, sys, urllib.request, urllib.error
from collections import Counter
run, data, port = sys.argv[1], sys.argv[2], sys.argv[3]
base = f"http://127.0.0.1:{port}/ga4gh/htsget/v1"
ids = dict(l.split() for l in open(os.path.join(run, "ids.txt")))

def http(method, url, body=None, headers=None):
    req = urllib.request.Request(url, method=method, headers=headers or {},
                                 data=None if body is None else json.dumps(body).encode())
    if body is not None:
        req.add_header("Content-Type", "application/json")
    try:
        with urllib.request.urlopen(req, timeout=300) as r:
            return r.status, r.read()
    except urllib.error.HTTPError as e:
        return e.code, e.read()

def fetch(ticket):
    out = b""
    for u in ticket["htsget"]["urls"]:
        if u["url"].startswith("data:"):
            out += base64.b64decode(u["url"].split(",", 1)[1])
        else:
            st, b = http("GET", u["url"], headers=u.get("headers", {}))
            assert st in (200, 206), (st, b[:200])
            out += b
    return out

def sh(*a):
    p = subprocess.run(a, capture_output=True, text=True)
    return p.returncode, p.stdout, p.stderr

def reads(path, region=None):
    # Whole SAM lines: name/flag/chrom/pos alone is not unique in real data
    # (the GIAB ONT file has records that share all four).
    args = ["samtools", "view", "--no-PG", path] + ([region] if region else [])
    rc, out, err = sh(*args)
    return rc, out.splitlines(), err

def variants(path, region=None):
    args = ["bcftools", "view", "-H"] + (["-r", region] if region else []) + [path]
    rc, out, err = sh(*args)
    return rc, out.splitlines(), err

def chrom(path):
    rc, out, _ = sh("samtools", "view", "-H", path)
    for l in out.splitlines():
        if l.startswith("@SQ") and ("SN:chr20" in l.split("\t") or "SN:20" in l.split("\t")):
            return [f for f in l.split("\t") if f.startswith("SN:")][0][3:]

whole = {}  # Counter of every record in each original file (for the duplicate check)
def whole_counts(ep, path):
    if path not in whole:
        whole[path] = Counter((reads if ep == "reads" else variants)(path)[1])
    return whole[path]

def clean(err):
    # htslib notes about index timestamps are not about the ticket
    return "\n".join(l for l in err.splitlines() if l.strip() and "index file is older" not in l)

random.seed(20)
cases = []
for name in ("illumina.bam", "ont.bam"):
    c = chrom(os.path.join(data, name))
    lo, hi = (30_000_000, 31_000_000) if name == "illumina.bam" else (30_000_000, 32_000_000)
    regs = [(c, lo, lo + 1_000), (c, hi - 5_000, hi)]
    for _ in range(6):
        s = random.randint(lo, hi - 50_000)
        regs.append((c, s, s + random.choice([100, 2_000, 20_000, 50_000])))
    for r in regs:
        cases.append(("reads", name, [r]))
    cases.append(("reads", name, regs[2:5]))  # POST with several regions
for name in ("benchmark.vcf.gz", "benchmark.bcf"):
    regs = [("chr20", 30_000_000, 31_000_000), ("chr21", 15_000_000, 15_200_000), ("chr20", 0, 70_000)]
    for _ in range(4):
        s = random.randint(1_000_000, 60_000_000)
        regs.append(("chr20", s, s + random.choice([1_000, 100_000])))
    for r in regs:
        cases.append(("variants", name, [r]))
    cases.append(("variants", name, regs[:2]))

ok_all = True
empty = 0
print(f"{'Datei':18} {'Region(en)':42} {'erwartet':>9} {'erhalten':>9} {'fehlt':>5} {'zu oft':>6} {'Ticket-Bytes':>13} {'Datei-Bytes':>12}  Ergebnis")
for ep, name, regs in cases:
    path = os.path.join(data, name)
    fmt = {"illumina.bam": "BAM", "ont.bam": "BAM", "benchmark.vcf.gz": "VCF", "benchmark.bcf": "BCF"}[name]
    body = {"format": fmt, "regions": [{"referenceName": c, "start": s, "end": e} for c, s, e in regs]}
    st, b = http("POST", f"{base}/{ep}/{ids[name]}", body)
    label = ", ".join(f"{c}:{s}-{e}" for c, s, e in regs)
    if st != 200:
        print(f"{name:18} {label[:42]:42} HTTP {st} {b[:160]!r}  FEHLER"); ok_all = False; continue
    t = json.loads(b)
    blob = fetch(t)
    ext = {"BAM": ".bam", "VCF": ".vcf.gz", "BCF": ".bcf"}[fmt]
    tp = os.path.join(run, "ticket" + ext)
    open(tp, "wb").write(blob)
    # Expected: every record overlapping a region, as often as the original has it.
    # Overlapping regions in one POST must not duplicate records (htsget spec), so the
    # expectation is the union of the per-region sets with original multiplicity.
    per_region = Counter()
    for c, s, e in regs:
        region = f"{c}:{s + 1}-{e}"  # htsget 0-based half-open -> samtools 1-based closed
        rc0, recs, _ = (reads if ep == "reads" else variants)(path, region)
        for r, n in Counter(recs).items():
            per_region[r] = max(per_region[r], n)
    rc, got, err = (reads if ep == "reads" else variants)(tp)
    got_c = Counter(got)
    warn = clean(err)
    missing = sum(max(0, n - got_c[r]) for r, n in per_region.items())
    orig = whole_counts(ep, path)
    too_many = sum(max(0, n - orig[r]) for r, n in got_c.items())
    expected = sum(per_region.values())
    good = rc == 0 and missing == 0 and too_many == 0 and not warn
    if good and expected == 0:
        empty += 1
        verdict = "ok (Region leer im Original)"
    else:
        verdict = "ok" if good else "FEHLER " + warn[:120]
    ok_all &= good
    print(f"{name:18} {label[:42]:42} {expected:9d} {len(got):9d} {missing:5d} {too_many:6d} {len(blob):13d} {os.path.getsize(path):12d}  {verdict}")

# Header-Tickets
for ep, name in (("reads", "illumina.bam"), ("variants", "benchmark.bcf")):
    st, b = http("GET", f"{base}/{ep}/{ids[name]}?class=header")
    blob = fetch(json.loads(b)) if st == 200 else b""
    tp = os.path.join(run, "header" + (".bam" if ep == "reads" else ".bcf"))
    open(tp, "wb").write(blob)
    rc, recs, err = (reads if ep == "reads" else variants)(tp)
    good = st == 200 and rc == 0 and not recs and not clean(err)
    ok_all &= good
    print(f"{name:18} {'class=header':42} {0:9d} {len(recs):9d} {'':5} {'':7} {len(blob):13d} {'':12}  {'ok' if good else 'FEHLER ' + err[:120]}")
print()
if empty:
    print(f"{empty} Region(en) ohne Records im Original: geprüft wurde nur, dass das Ticket gültig ist und nichts erfindet.")
print("ALLES OK" if ok_all else "MINDESTENS EIN FEHLER (Ausgabe bitte zurückschicken)")
sys.exit(0 if ok_all else 1)
PY
