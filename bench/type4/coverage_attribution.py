"""Coverage-loss attribution — where nose loses analysis coverage, by language and construct.

§BS measured the behavior-keyed recall frontier NO-GO and concluded worthy-recall is bounded
by **unit extraction / coverage**, not by missing matching machinery. This instrument makes
that boundary actionable: it runs `nose stats --json` over the pinned corpus and ranks the
**IL-lowering loss** — the `Raw` nodes a construct lowers to, which are invisible to
value-matching — by (language, surface-kind) prevalence. That ranked list is the worklist for
lowering fidelity (the cheapest recall lever, zero soundness risk).

It is the decision instrument for the analysis-quality set: it says which language/construct
carries the mass, so levers are picked by prevalence, not guessed.

Usage:
  python3 coverage_attribution.py                 # run over the pinned corpus, print + write json
  python3 coverage_attribution.py --limit 10      # first 10 repos (smoke)
  python3 coverage_attribution.py --nose PATH     # binary (default target/release/nose)

Output: coverage_attribution.<date>.json — per-language Raw ratio + the prevalence-ranked
top unhandled surface kinds, aggregated across the corpus. Deterministic and corpus-pinned.

NOTE (scope): this measures IL-lowering loss (`Raw` nodes). Value-graph modeling loss
(`Opaque` nodes — the collections/mutation/#391 gap) is a separate dimension, now measured by
the sibling `value_graph_attribution.py`, which consumes the `nose value-census` opaque census
(the "light instrumentation" this note used to defer).
"""

from __future__ import annotations

import argparse
import json
import subprocess
import sys
from collections import defaultdict
from pathlib import Path

ROOT = Path(__file__).resolve().parents[2]


def corpus_repos(repos_root: Path, limit: int | None):
    corpus = json.loads((ROOT / "bench/goldens/corpus.json").read_text())["repositories"]
    repos = sorted(corpus, key=lambda r: r["id"])
    if limit:
        repos = repos[:limit]
    return [(r["id"], r["primary_language"], repos_root / r["id"]) for r in repos]


def stats_for(nose: str, path: Path) -> dict | None:
    try:
        # A high --top captures every unhandled surface kind per repo (for stats, --top 0
        # means "show none", not "all"); we re-rank by aggregated Raw mass below.
        out = subprocess.run(
            [nose, "stats", str(path), "--json", "--top", "1000"],
            capture_output=True,
            text=True,
            timeout=300,
        )
    except subprocess.TimeoutExpired:
        return None
    if out.returncode != 0 or not out.stdout.strip():
        return None
    try:
        return json.loads(out.stdout)
    except json.JSONDecodeError:
        return None


def main() -> int:
    ap = argparse.ArgumentParser()
    ap.add_argument("--nose", default=str(ROOT / "target" / "release" / "nose"))
    ap.add_argument("--repos-root", default=str(ROOT / "bench" / "repos"))
    ap.add_argument("--limit", type=int, default=None)
    ap.add_argument("--date", default="undated", help="stamp for the output filename")
    args = ap.parse_args()

    repos = corpus_repos(Path(args.repos_root), args.limit)
    # Per-language node/raw totals, and Raw counts summed by (lang, surface_kind).
    lang_nodes: dict[str, int] = defaultdict(int)
    lang_raw: dict[str, int] = defaultdict(int)
    lang_boundary: dict[str, int] = defaultdict(int)
    kind_raw: dict[tuple[str, str], int] = defaultdict(int)
    kind_boundary: dict[tuple[str, str], bool] = {}
    missing = []
    for rid, _primary, path in repos:
        if not path.exists():
            missing.append(rid)
            continue
        st = stats_for(args.nose, path)
        if st is None:
            missing.append(rid)
            continue
        for pl in st.get("per_lang", []):
            lang_nodes[pl["lang"]] += pl["nodes"]
            lang_raw[pl["lang"]] += pl["raw_nodes"]
            lang_boundary[pl["lang"]] += pl.get("boundary_raw", 0)
        for u in st.get("top_unhandled", []):
            kind_raw[(u["lang"], u["surface_kind"])] += u["count"]
            kind_boundary[(u["lang"], u["surface_kind"])] = u.get("boundary", False)
        print(f"  {rid:<24} raw {st['raw_nodes']:>6} / {st['total_nodes']:>8}", file=sys.stderr)

    langs = sorted(
        lang_nodes,
        key=lambda l: lang_raw[l] / lang_nodes[l] if lang_nodes[l] else 0.0,
        reverse=True,
    )
    per_lang = [
        {
            "lang": l,
            "nodes": lang_nodes[l],
            "raw_nodes": lang_raw[l],
            "raw_ratio": round(lang_raw[l] / lang_nodes[l], 5) if lang_nodes[l] else 0.0,
            "boundary_raw": lang_boundary[l],
            "gap_raw": lang_raw[l] - lang_boundary[l],
            "gap_ratio": round((lang_raw[l] - lang_boundary[l]) / lang_nodes[l], 5)
            if lang_nodes[l]
            else 0.0,
        }
        for l in langs
    ]
    # Rank by Raw mass; tag each kind boundary (by-design) vs gap (fixable lowering target).
    top_overall = sorted(kind_raw.items(), key=lambda kv: kv[1], reverse=True)
    top_kinds = [
        {
            "lang": lang,
            "surface_kind": kind,
            "raw": n,
            "boundary": kind_boundary.get((lang, kind), False),
        }
        for (lang, kind), n in top_overall
    ]
    # The genuine fixable lowering worklist: gaps only (boundaries are by design).
    gap_kinds = [k for k in top_kinds if not k["boundary"]]

    result = {
        "instrument": "coverage_attribution",
        "repos": len(repos) - len(missing),
        "missing": missing,
        "per_lang": per_lang,
        "top_unhandled": top_kinds,
    }
    out_path = ROOT / "bench" / "type4" / f"coverage_attribution.{args.date}.json"
    out_path.write_text(json.dumps(result, indent=2) + "\n")

    print(f"\ncoverage attribution — {result['repos']} repos (Raw = lowering-gap + protocol-boundary)\n")
    print(f"{'language':<14}{'nodes':>12}{'raw':>10}{'raw%':>9}{'gap':>10}{'gap%':>9}")
    for pl in per_lang:
        print(
            f"{pl['lang']:<14}{pl['nodes']:>12}{pl['raw_nodes']:>10}{pl['raw_ratio'] * 100:>8.3f}%"
            f"{pl['gap_raw']:>10}{pl['gap_ratio'] * 100:>8.3f}%"
        )
    print("\ngenuine lowering gaps (boundaries excluded — the fixable worklist):")
    print(f"  {'lang':<12}{'surface_kind':<34}{'raw':>9}")
    for k in gap_kinds[:25]:
        print(f"  {k['lang']:<12}{k['surface_kind']:<34}{k['raw']:>9}")
    print("\nlargest protocol boundaries (by design — NOT lowering targets):")
    for k in [k for k in top_kinds if k["boundary"]][:8]:
        print(f"  {k['lang']:<12}{k['surface_kind']:<34}{k['raw']:>9}")
    if missing:
        print(f"\n(skipped {len(missing)} repos not checked out / failed: {', '.join(missing[:8])}…)")
    print(f"\nwrote {out_path.relative_to(ROOT)}")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
