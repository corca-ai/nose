#!/usr/bin/env python3
"""Independent miss-mining arm: same-computation candidate pairs nose does NOT report.

The v5 gold-set pool is nose ∪ jscpd, and a token detector cannot contribute
semantic misses — so in-the-wild Type-4 recall has no measurement
(experiments §BJ closed the *mechanism* question; this tool addresses the
*measurement* one, #194). It mines, per pinned-corpus repo:

- modality B (structure): unit pairs whose exact value-multiset Jaccard is high
  (>= --vj, default 0.80) yet no reported family on the MAXIMAL current surface
  (`--mode syntax,semantic,near --min-value 0`) contains both — i.e. the
  detector's own value evidence says "same computation" while every product
  channel stays silent. Candidates come from LSH banding over the detection
  minhash (32 bands x 4 rows), so the pass is near-linear, then exact vj
  confirms. Each candidate is annotated with a source text-similarity ratio so
  analysis can slice off the textually-obvious ones (the jscpd-shaped blind
  spot is the LOW-text-similarity tail).

(Modality A — behavior-equal fingerprint-split pairs from the interpreter
oracle — is `nose verify --leads`; run it alongside.)

Output is a QUEUE SIGNAL only (issue #36 discipline): every record carries
`evidence_tier: detector-suggested` and nothing here writes frontier status.
A confirmed miss graduates by hand into `real_frontier.v1.json` via the audit
template in docs/frontier-platform.md.

Usage:
  python3 bench/type4/miss_mining.py [--repos-root bench/repos] [--vj 0.8]
    [--limit-repos N] [--per-repo 30] [--json-out bench/type4/miss_mining_<date>.json]

Deterministic given pinned inputs: no timestamps; candidates sorted on stable keys.
"""

import argparse
import difflib
import hashlib
import json
import subprocess
from collections import Counter, defaultdict
from pathlib import Path

ROOT = Path(__file__).resolve().parents[2]
NOSE = ROOT / "target" / "release" / "nose"

BANDS, ROWS = 32, 4  # over the 128-slot detection minhash
MIN_LINES, MIN_TOKENS = 5, 40  # meaningful-size floor for mined units
SCAN_TIMEOUT = 900


def rel(p: str) -> str:
    p = p.replace(str(ROOT) + "/", "")
    i = p.find("bench/repos/")
    return p[i:] if i >= 0 else p


def run_json(cmd):
    r = subprocess.run(cmd, cwd=ROOT, capture_output=True, text=True, timeout=SCAN_TIMEOUT)
    if r.returncode != 0:
        return None
    return json.loads(r.stdout)


def reported_groups(repo_dir):
    """Family id per (file,line) span on the maximal current surface."""
    payload = run_json([
        str(NOSE), "scan", str(repo_dir), "--format", "json", "--top", "0",
        "--mode", "syntax,semantic,near", "--min-value", "0", "--min-members", "2",
    ])
    if payload is None:
        return None
    fams = payload.get("families", []) if isinstance(payload, dict) else payload
    spans = defaultdict(list)  # file -> [(start, end, fam_idx)]
    for i, f in enumerate(fams):
        for loc in f["locations"]:
            spans[rel(loc["file"])].append((loc["start_line"], loc["end_line"], i))
    return spans


def fams_covering(spans, file, start, end):
    return {
        fi for (s, e, fi) in spans.get(file, [])
        if not (e < start or end < s)
    }


def vj(a, b) -> float:
    ca, cb = Counter(a), Counter(b)
    inter = sum((ca & cb).values())
    union = sum((ca | cb).values())
    return inter / union if union else 0.0


def text_ratio(ua, ub) -> float:
    def slice_of(u):
        try:
            lines = (ROOT / rel(u["path"])).read_text(errors="replace").splitlines()
        except OSError:
            return ""
        return "\n".join(lines[u["start_line"] - 1 : u["end_line"]])[:4000]

    return round(
        difflib.SequenceMatcher(None, slice_of(ua), slice_of(ub), autojunk=False).ratio(), 3
    )


def overlapping(ua, ub) -> bool:
    return rel(ua["path"]) == rel(ub["path"]) and not (
        ua["end_line"] < ub["start_line"] or ub["end_line"] < ua["start_line"]
    )


def mine_repo(repo_dir, vj_floor, per_repo):
    spans = reported_groups(repo_dir)
    if spans is None:
        return None, "scan-failed"
    feats = run_json([
        str(NOSE), "features", str(repo_dir),
        "--min-lines", str(MIN_LINES), "--min-tokens", str(MIN_TOKENS),
    ])
    if feats is None:
        return None, "features-failed"
    units = [u for u in feats["units"] if len(u["value"]) >= 8]

    buckets = defaultdict(list)
    for i, u in enumerate(units):
        mh = u["minhash"]
        for b in range(BANDS):
            key = (b, tuple(mh[b * ROWS : (b + 1) * ROWS]))
            buckets[key].append(i)

    seen = set()
    found = []
    for members in buckets.values():
        if len(members) < 2 or len(members) > 50:  # giant buckets are boilerplate
            continue
        for x in range(len(members)):
            for y in range(x + 1, len(members)):
                i, j = members[x], members[y]
                if (i, j) in seen:
                    continue
                seen.add((i, j))
                ua, ub = units[i], units[j]
                if overlapping(ua, ub):
                    continue
                score = vj(ua["value"], ub["value"])
                if score < vj_floor:
                    continue
                fa = fams_covering(spans, rel(ua["path"]), ua["start_line"], ua["end_line"])
                fb = fams_covering(spans, rel(ub["path"]), ub["start_line"], ub["end_line"])
                if fa & fb:
                    continue  # already reported together somewhere
                found.append({
                    "vj": round(score, 3),
                    "fp_equal": ua["value"] == ub["value"],
                    "exact_safe": [ua["exact_safe"], ub["exact_safe"]],
                    "kinds": [ua["kind"], ub["kind"]],
                    "text_similarity": text_ratio(ua, ub),
                    "members": [
                        {"file": rel(u["path"]), "start_line": u["start_line"],
                         "end_line": u["end_line"], "name": u.get("name")}
                        for u in (ua, ub)
                    ],
                    "evidence_tier": "detector-suggested",
                })
    found.sort(key=lambda r: (-r["vj"], r["members"][0]["file"],
                              r["members"][0]["start_line"],
                              r["members"][1]["file"], r["members"][1]["start_line"]))
    return found[:per_repo], None


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("--repos-root", default=str(ROOT / "bench" / "repos"))
    ap.add_argument("--vj", type=float, default=0.80)
    ap.add_argument("--per-repo", type=int, default=30)
    ap.add_argument("--limit-repos", type=int, default=None)
    ap.add_argument("--json-out", default=None)
    args = ap.parse_args()

    corpus = json.loads((ROOT / "bench/goldens/corpus.json").read_text())["repositories"]
    digest = hashlib.sha256()
    for r in sorted(corpus, key=lambda r: r["id"]):
        digest.update(
            f"{r['id']}\t{r['split']}\t{r['primary_language']}\t{r['commit']}\n".encode()
        )
    repos = sorted(corpus, key=lambda r: r["id"])
    if args.limit_repos:
        repos = repos[: args.limit_repos]

    out_repos, failures = {}, []
    totals = Counter()
    for r in repos:
        repo_dir = Path(args.repos_root) / r["id"]
        if not repo_dir.is_dir():
            continue
        cands, err = mine_repo(repo_dir, args.vj, args.per_repo)
        if err:
            failures.append({"repo": r["id"], "error": err})
            continue
        if cands:
            out_repos[r["id"]] = {
                "split": r["split"], "language": r["primary_language"], "candidates": cands,
            }
        totals[(r["primary_language"], r["split"])] += len(cands or [])
        low_text = sum(1 for c in cands or [] if c["text_similarity"] < 0.5)
        print(f"{r['id']}: {len(cands or [])} candidates ({low_text} low-text)", flush=True)

    nose_ver = subprocess.run([str(NOSE), "--version"], capture_output=True, text=True).stdout.strip()
    print("\nper (language, split):")
    for (lang, split), n in sorted(totals.items()):
        print(f"  {lang}/{split}: {n}")
    if failures:
        print("failures:", ", ".join(f"{f['repo']}({f['error']})" for f in failures))

    if args.json_out:
        Path(args.json_out).write_text(json.dumps({
            "schema_version": "0.1.0",
            "discipline": "queue-signal-only; see docs/frontier-platform.md (#36)",
            "corpus_commit_digest": digest.hexdigest(),
            "nose_version": nose_ver,
            "vj_floor": args.vj,
            "min_unit": {"lines": MIN_LINES, "tokens": MIN_TOKENS, "value_nodes": 8},
            "scan_failures": sorted(f["repo"] for f in failures),
            "repos": dict(sorted(out_repos.items())),
        }, indent=1, sort_keys=True) + "\n")
        print(f"wrote {args.json_out}")


if __name__ == "__main__":
    main()
