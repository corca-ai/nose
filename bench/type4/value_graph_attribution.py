"""Value-graph coverage-loss attribution — where the FINGERPRINT loses coverage, by construct.

The sibling instrument `coverage_attribution.py` measures IL-**lowering** loss (`Raw` nodes). Its
closing note flagged a separate dimension it could not see: value-graph modeling loss (`Opaque`
nodes — the collections/mutation/#391 question), because `Opaque` carried no construct provenance.
This script consumes the instrumentation that closed that gap: `nose value-census` walks every
function unit, builds its value graph with the opaque census on, and reports per IL construct how
many `ValOp::Opaque` fallbacks were minted — the value-graph analog of the `Raw` ratio.

`total_fallback` opaques are full coverage gaps (the construct could not be modeled at all); the
rest are semantic opaques carrying structure (e.g. `instanceof`).

It is the #391 decision instrument: it says whether map/collection reads carry real value-graph
opaque mass (model `Value::Map`) or whether they are modeled-but-divergent (a convergence problem,
not an opaque-coverage one). The map-read constructs are `Index` (`m[k]`), `Field` (`m.x`), and
`Call` (`m.get(k)` / `m.items()`).

Usage:
  python3 value_graph_attribution.py                 # run over the pinned corpus, print + write json
  python3 value_graph_attribution.py --limit 10      # first 10 repos (smoke)
  python3 value_graph_attribution.py --nose PATH     # binary (default target/release/nose)

Output: value_graph_attribution.<date>.json — the prevalence-ranked opaque worklist by construct,
plus the map-read constructs' share. Deterministic and corpus-pinned.
"""

from __future__ import annotations

import argparse
import json
import subprocess
import sys
from collections import defaultdict
from pathlib import Path

ROOT = Path(__file__).resolve().parents[2]

# The IL constructs that carry map / collection reads — the #391 question. `Index` is `m[k]`
# (and `xs[i]`); `Field` is `m.x`; `Call` is `m.get(k)` / `m.items()` / membership helpers.
MAP_READ_CONSTRUCTS = {"Index", "Field", "Call"}


def corpus_repos(repos_root: Path, limit: int | None):
    corpus = json.loads((ROOT / "bench/goldens/corpus.json").read_text())["repositories"]
    repos = sorted(corpus, key=lambda r: r["id"])
    if limit:
        repos = repos[:limit]
    return [(r["id"], r.get("primary_language", "?"), repos_root / r["id"]) for r in repos]


def census_for(nose: str, path: Path) -> dict | None:
    try:
        out = subprocess.run(
            [nose, "value-census", str(path)],
            capture_output=True,
            text=True,
            timeout=600,
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
    # Opaque nodes + affected units summed by (construct, total_fallback), and per-language unit
    # opaque rates for an at-a-glance view.
    opaque_nodes: dict[tuple[str, bool], int] = defaultdict(int)
    opaque_units: dict[tuple[str, bool], int] = defaultdict(int)
    lang_units: dict[str, int] = defaultdict(int)
    lang_opaque_units: dict[str, int] = defaultdict(int)
    total_units = 0
    total_opaque_units = 0
    missing = []
    for rid, primary, path in repos:
        if not path.exists():
            missing.append(rid)
            continue
        c = census_for(args.nose, path)
        if c is None:
            missing.append(rid)
            continue
        total_units += c["function_units"]
        total_opaque_units += c["units_with_opaque"]
        lang_units[primary] += c["function_units"]
        lang_opaque_units[primary] += c["units_with_opaque"]
        for row in c["by_construct"]:
            key = (row["construct"], row["total_fallback"])
            opaque_nodes[key] += row["opaque_nodes"]
            opaque_units[key] += row["units"]
        print(
            f"  {rid:<24} units {c['function_units']:>6}  opaque-units {c['units_with_opaque']:>6}",
            file=sys.stderr,
        )

    by_construct = sorted(
        (
            {
                "construct": k,
                "total_fallback": total,
                "opaque_nodes": n,
                "units": opaque_units[(k, total)],
            }
            for (k, total), n in opaque_nodes.items()
        ),
        key=lambda r: (-r["opaque_nodes"], r["construct"], r["total_fallback"]),
    )
    all_opaque = sum(opaque_nodes.values())
    map_read_opaque = sum(n for (k, _t), n in opaque_nodes.items() if k in MAP_READ_CONSTRUCTS)
    per_lang = sorted(
        (
            {
                "lang": l,
                "function_units": lang_units[l],
                "opaque_units": lang_opaque_units[l],
                "opaque_unit_ratio": round(lang_opaque_units[l] / lang_units[l], 4)
                if lang_units[l]
                else 0.0,
            }
            for l in lang_units
        ),
        key=lambda r: -r["opaque_unit_ratio"],
    )

    result = {
        "instrument": "value_graph_attribution",
        "repos": len(repos) - len(missing),
        "missing": missing,
        "function_units": total_units,
        "units_with_opaque": total_opaque_units,
        "opaque_unit_ratio": round(total_opaque_units / total_units, 4) if total_units else 0.0,
        "opaque_nodes_total": all_opaque,
        "map_read_opaque_nodes": map_read_opaque,
        "map_read_opaque_share": round(map_read_opaque / all_opaque, 4) if all_opaque else 0.0,
        "per_lang": per_lang,
        "by_construct": by_construct,
    }
    out_path = ROOT / "bench" / "type4" / f"value_graph_attribution.{args.date}.json"
    out_path.write_text(json.dumps(result, indent=2))

    print(f"\nvalue-graph opaque census: {len(repos) - len(missing)} repos, {total_units} units")
    print(
        f"units with ≥1 opaque: {total_opaque_units} ({result['opaque_unit_ratio']:.1%}) · "
        f"opaque nodes: {all_opaque}"
    )
    print(
        f"map-read constructs (Index/Field/Call) opaque share: "
        f"{map_read_opaque}/{all_opaque} = {result['map_read_opaque_share']:.1%}"
    )
    print("\ntop opaque-minting constructs (the value-graph coverage worklist):")
    for r in by_construct[:15]:
        flag = " [map-read]" if r["construct"] in MAP_READ_CONSTRUCTS else ""
        kind = "total-fallback" if r["total_fallback"] else "semantic"
        print(
            f"  {r['construct']:<8} {kind:<14} opaque_nodes {r['opaque_nodes']:>7}  "
            f"units {r['units']:>6}{flag}"
        )
    print(f"\nwrote {out_path}")
    if missing:
        print(f"missing/failed repos ({len(missing)}): {', '.join(missing)}", file=sys.stderr)
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
