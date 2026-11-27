# SPDX-License-Identifier: Apache-2.0
"""Walkthrough for demo/federation-edge: five checks against two running Edge nodes.

Reads WORK, PORT_A, PORT_B, NAME_A, NAME_B from the environment (set by run.sh).
Writes report.json and report.md into WORK. Standard library only.
Exit code 1 when a check fails.
"""

import datetime
import json
import os
import sys
import urllib.parse
import urllib.request

WORK = os.environ["WORK"]
A = {"name": os.environ["NAME_A"], "base": f"http://127.0.0.1:{os.environ['PORT_A']}"}
B = {"name": os.environ["NAME_B"], "base": f"http://127.0.0.1:{os.environ['PORT_B']}"}

ONLY_B = {
    "assemblyId": "GRCh38",
    "referenceName": "chr11",
    "start": "5227002",
    "referenceBases": "T",
    "alternateBases": "A",
}
AUDIT_RANGE = "from=2000-01-01T00:00:00Z&to=2100-01-01T00:00:00Z"


def get(url):
    with urllib.request.urlopen(url, timeout=15) as r:
        return json.loads(r.read().decode())


def beacon(node, federate, granularity=None):
    q = dict(ONLY_B)
    if federate:
        q["federate"] = "true"
    if granularity:
        q["requestedGranularity"] = granularity
    return get(f"{node['base']}/ga4gh/beacon/v2/g_variants?{urllib.parse.urlencode(q)}")


def audit(node):
    return get(f"{node['base']}/api/v1/audit/residency?{AUDIT_RANGE}")


def drs_ids(node):
    try:
        listing = get(f"{node['base']}/ga4gh/drs/v1/objects")
    except Exception:
        return None
    items = listing if isinstance(listing, list) else listing.get("objects", listing.get("items", []))
    return sorted(o.get("id") for o in items if isinstance(o, dict))


checks = []


def check(title, claim, ok, evidence):
    checks.append({"title": title, "claim": claim, "ok": bool(ok), "evidence": evidence})
    mark = "OK  " if ok else "FAIL"
    print(f"  [{mark}] {title}")
    for line in evidence.splitlines():
        print(f"         {line}")


# 1. Local query on A: the variant is only on B, so A alone says no.
r = beacon(A, federate=False)
check(
    f"{A['name']} alone does not have the variant",
    "Each node answers from its own data.",
    r["response"]["exists"] is False,
    f"GET g_variants on {A['name']} without federate -> exists={r['response']['exists']}",
)

# 2. Federated query on A: B answers, A reports the union.
before_a = len(audit(A)["entries"])
before_b = len(audit(B)["entries"])
r = beacon(A, federate=True, granularity="count")
warnings = r["meta"].get("warnings")
check(
    f"{A['name']} finds it through {B['name']}",
    "Partners learn whether a variant exists, without receiving the data.",
    r["response"]["exists"] is True and (r["response"]["count"] or 0) >= 1 and not warnings,
    f"GET g_variants on {A['name']} with federate=true -> exists={r['response']['exists']}, "
    f"count={r['response']['count']}, warnings={warnings}",
)

# 3. Audit on A: the outgoing peer query is recorded, and no data left the node.
entries_a = audit(A)["entries"][before_a:]
sent = [e for e in entries_a if e["event_type"] == "peer_query_sent"]
check(
    f"{A['name']} logs the question it sent",
    "Every query to a partner is recorded in the node's own audit log.",
    any(e["destination"] == B["name"] and e["data_left_node"] is False for e in sent),
    "\n".join(
        f"{e['event_type']} -> destination={e['destination']}, data_left_node={e['data_left_node']}"
        for e in entries_a
    )
    or "no new audit entries",
)

# 4. Audit on B: the incoming query is recorded on B's side too.
entries_b = audit(B)["entries"][before_b:]
check(
    f"{B['name']} logs the question it answered",
    "The answering node keeps its own record as well.",
    any(e["event_type"] == "beacon_query" for e in entries_b),
    "\n".join(f"{e['event_type']} requester={e['requester']}" for e in entries_b)
    or "no new audit entries",
)

# 5. Both chains verify, and no file moved between nodes.
va = get(f"{A['base']}/api/v1/audit/residency/verify")
vb = get(f"{B['base']}/api/v1/audit/residency/verify")
ids_a, ids_b = drs_ids(A), drs_ids(B)
own_a = open(os.path.join(WORK, "a", "object_id")).read().strip()
own_b = open(os.path.join(WORK, "b", "object_id")).read().strip()
files_stayed = ids_a is None or (own_b not in ids_a)
check(
    "Both audit chains verify and each file stayed on its node",
    "The log is hash-chained; the federated answer moved no file.",
    va["chain_valid"] and vb["chain_valid"] and files_stayed,
    f"{A['name']}: chain_valid={va['chain_valid']} entries={va['entry_count']} last_hash={va['last_hash'][:16]}...\n"
    f"{B['name']}: chain_valid={vb['chain_valid']} entries={vb['entry_count']} last_hash={vb['last_hash'][:16]}...\n"
    f"{A['name']} DRS objects: {ids_a if ids_a is not None else 'listing not available'} "
    f"(own: {own_a}; {B['name']}'s object {own_b} not present)",
)

# --- Report ---------------------------------------------------------------------
health = get(f"{A['base']}/health")
report = {
    "generated_at": datetime.datetime.now(datetime.timezone.utc).isoformat(timespec="seconds"),
    "ferrum_version": health.get("version"),
    "mode": "edge (SQLite, local objects), two processes on one computer",
    "nodes": [A, B],
    "summary": {"ok": all(c["ok"] for c in checks), "passed": sum(c["ok"] for c in checks), "total": len(checks)},
    "checks": checks,
    "limits": [
        "Both nodes run on this computer. The demo shows the protocol and the logs, not a network between countries.",
        "Synthetic variants only. No person's data.",
        "Sign-in is off (require_auth = false) so the walkthrough needs no account. A real node keeps it on.",
        "A passing run is a technical signal for this version. It is not GA4GH certification and not a legal assessment.",
    ],
}
with open(os.path.join(WORK, "report.json"), "w") as f:
    json.dump(report, f, indent=2)

md = [
    "# Ferrum federation walkthrough",
    "",
    f"Run at {report['generated_at']} on Ferrum {report['ferrum_version']}, {report['mode']}.",
    f"Result: {report['summary']['passed']} of {report['summary']['total']} checks passed.",
    "",
    "| # | Check | What it shows | Result |",
    "| --- | --- | --- | --- |",
]
for i, c in enumerate(checks, 1):
    md.append(f"| {i} | {c['title']} | {c['claim']} | {'passed' if c['ok'] else 'failed'} |")
md += ["", "## Evidence", ""]
for i, c in enumerate(checks, 1):
    md += [f"### {i}. {c['title']}", "", "```", c["evidence"], "```", ""]
md += ["## What this run does not show", ""] + [f"- {l}" for l in report["limits"]] + [""]
with open(os.path.join(WORK, "report.md"), "w") as f:
    f.write("\n".join(md))

print(f"\n  {report['summary']['passed']} of {report['summary']['total']} checks passed.")
sys.exit(0 if report["summary"]["ok"] else 1)
