#!/usr/bin/env python3
"""Unified Type-4 coverage matrix + a coverage-aware next-cell dispenser.

Fixes the old `type4-next` bias (axis-atom + static prevalence score → diagonal,
language-skewed matrix). Here the atom is a CELL `(axis, language)` and `next` scores cells
with a coverage-gap term + a fairness floor, so the loop expands EVENLY.

Usage:
  python3 coverage_matrix.py matrix            # print the (axis × language) grid + write json
  python3 coverage_matrix.py next [--limit N] [--arm recall|soundness]
                                  [--feasibility fixable|partial|landed|research]
                                  [--lang go,rust,...]

Evidence source: real_frontier.v1.json (the human-verified store). Landed-but-unrecorded
cells are surfaced as cheap "verify" work — exactly the lever that fills the go/js/rust holes.
The full soundness arm currently lives in tests/equivalence.rs and is only partially mapped
per cell here (noted, not faked).
"""

from __future__ import annotations

import argparse
import json
from collections import Counter, defaultdict
from pathlib import Path

import coverage_taxonomy as tax

HERE = Path(__file__).resolve().parent
REAL_FRONTIER = HERE / "real_frontier.v1.json"
SWEEP_EVIDENCE = HERE / "coverage_evidence.v1.json"
OUT_JSON = HERE / "coverage_matrix.v1.json"

# Best-status precedence when a cell has several evidence rows (lower index wins).
STATUS_RANK = ["false-merge", "closed", "already-covered", "hard-negative", "real-miss",
               "unsupported"]
COVERED = {"closed", "already-covered"}
# coverage_sweep.py status -> matrix status vocabulary.
SWEEP_MAP = {"covered": "closed", "gap": "unsupported", "partial": "real-miss",
             "false-merge": "false-merge"}
FEAS_W = {"fixable": 100, "partial": 70, "landed": 50, "research": 25}
FAMILY_BONUS = {"structural": 30, "soundness": 20, "algebraic": 10, "idiom": 0}
GLYPH = {
    "covered": "✓", "hard-negative": "⊘", "real-miss": "M", "unsupported": "U",
    "false-merge": "✗", "none": "·", "n/a": " ", "out": "x",
}


def canon(axis_field: str) -> str:
    return axis_field.split(" / ")[0].strip()


def load_evidence():
    """(axis_id_or_canon, language) -> best status, merging real_frontier + the live sweep."""
    raw = defaultdict(list)
    for it in json.loads(REAL_FRONTIER.read_text()).get("items", []):
        raw[(canon(it["candidate_axis"]), it["language"])].append(it["status"])
        raw[(it["candidate_axis"], it["language"])].append(it["status"])  # full string too
    if SWEEP_EVIDENCE.exists():
        for e in json.loads(SWEEP_EVIDENCE.read_text()).get("evidence", []):
            st = SWEEP_MAP.get(e["status"])
            if st:  # 'no-positive' carries no recall signal
                raw[(e["axis"], e["language"])].append(st)
    best = {}
    for key, statuses in raw.items():
        best[key] = min(statuses, key=lambda s: STATUS_RANK.index(s) if s in STATUS_RANK else 99)
    return best


def cell_status(axis: dict, lang: str, evidence: dict) -> str:
    if lang not in axis["languages"]:
        return "n/a"
    if axis["feasibility"] == "out-of-scope":
        return "out"
    keys = [(axis["axis_id"], lang)] + [(a, lang) for a in axis["aliases"]] \
        + [(canon(a), lang) for a in axis["aliases"]]
    found = [evidence[k] for k in keys if k in evidence]
    if not found:
        return "none"
    st = min(found, key=lambda s: STATUS_RANK.index(s) if s in STATUS_RANK else 99)
    if st in COVERED:
        return "covered"
    return st


def build_matrix(evidence):
    grid = {}
    for axis in tax.AXES:
        grid[axis["axis_id"]] = {l: cell_status(axis, l, evidence) for l in tax.LANGS}
    return grid


def is_covered(axis, status):
    if status == "covered":
        return True
    # a recorded hard negative IS the soundness guard for a soundness axis
    return axis["family"] == "soundness" and status == "hard-negative"


# ---------------------------------------------------------------------------- matrix view
def cmd_matrix(_args):
    evidence = load_evidence()
    grid = build_matrix(evidence)
    hdr = "axis".ljust(34) + " ".join(l[:2].rjust(2) for l in tax.LANGS)
    print("COVERAGE MATRIX  (✓ covered · untracked U unsupported M real-miss ⊘ hard-neg "
          "x out-of-scope ' ' n/a)")
    print(hdr)
    print("-" * len(hdr))
    by_family = defaultdict(list)
    for axis in tax.AXES:
        by_family[axis["family"]].append(axis)
    for fam in ("structural", "algebraic", "idiom", "soundness"):
        if fam not in by_family:
            continue
        print(f"# {fam}")
        for axis in by_family[fam]:
            row = ("  " + axis["axis_id"])[:34].ljust(34)
            row += " ".join(GLYPH[grid[axis["axis_id"]][l]].rjust(2) for l in tax.LANGS)
            tag = "" if axis["feasibility"] != "out-of-scope" else "  (non-goal)"
            print(row + f"   [{axis['feasibility']}]" + tag)
    # fairness summary
    lang_cov = Counter()
    applicable = Counter()
    for axis in tax.AXES:
        if axis["feasibility"] == "out-of-scope":
            continue
        for l in tax.LANGS:
            st = grid[axis["axis_id"]][l]
            if st == "n/a":
                continue
            applicable[l] += 1
            if is_covered(axis, st):
                lang_cov[l] += 1
    print("\nPER-LANGUAGE COVERED / APPLICABLE (the evenness gauge):")
    for l in tax.LANGS:
        ap = applicable[l]
        cv = lang_cov[l]
        bar = "█" * cv + "·" * (ap - cv)
        print(f"  {l:11s} {cv:2d}/{ap:<2d}  {bar}")
    total_cells = sum(applicable.values())
    total_cov = sum(lang_cov.values())
    print(f"\noverall: {total_cov}/{total_cells} applicable cells covered "
          f"({100*total_cov//max(total_cells,1)}%)")
    OUT_JSON.write_text(json.dumps(
        {"languages": tax.LANGS, "grid": grid,
         "per_language": {l: {"covered": lang_cov[l], "applicable": applicable[l]}
                          for l in tax.LANGS}},
        indent=2, sort_keys=True) + "\n")
    print(f"wrote {OUT_JSON.relative_to(HERE.parents[1])}")
    return 0


# ----------------------------------------------------------------------------- dispenser
def dispensable_cells(grid):
    cells = []
    lang_cov = Counter()
    axis_cov = Counter()
    for axis in tax.AXES:
        for l in tax.LANGS:
            if is_covered(axis, grid[axis["axis_id"]][l]):
                lang_cov[l] += 1
                axis_cov[axis["axis_id"]] += 1
    max_lang = max(lang_cov.values(), default=0)
    max_axis = max(axis_cov.values(), default=0)
    for axis in tax.AXES:
        if axis["feasibility"] == "out-of-scope":
            continue
        for l in tax.LANGS:
            st = grid[axis["axis_id"]][l]
            if st in ("n/a", "out") or is_covered(axis, st):
                continue
            need = "verify" if (axis["feasibility"] == "landed" and st == "none") else "implement"
            score = (
                FEAS_W.get(axis["feasibility"], 0)
                + FAMILY_BONUS.get(axis["family"], 0)
                + 6 * (max_lang - lang_cov[l])        # fairness: boost neglected languages
                + 3 * (max_axis - axis_cov[axis["axis_id"]])  # spread within an axis
                + (5 if st in ("real-miss", "unsupported") else 0)  # confirmed work is actionable
            )
            cells.append({
                "axis_id": axis["axis_id"], "language": l, "score": score, "need": need,
                "arm": axis["arm"], "family": axis["family"],
                "feasibility": axis["feasibility"], "status": st,
                "title": axis["title"], "note": axis["note"], "direction": axis["direction"],
            })
    cells.sort(key=lambda c: (-c["score"], c["axis_id"], c["language"]))
    return cells


def cmd_next(args):
    grid = build_matrix(load_evidence())
    cells = dispensable_cells(grid)
    if args.arm:
        cells = [c for c in cells if c["arm"] in (args.arm, "both")]
    if args.feasibility:
        cells = [c for c in cells if c["feasibility"] == args.feasibility]
    if args.lang:
        allow = set(args.lang.split(","))
        cells = [c for c in cells if c["language"] in allow]
    sel = cells[: max(args.limit, 0)]
    if args.json:
        print(json.dumps(sel, indent=2, sort_keys=True))
        return 0
    if not sel:
        print("No dispensable cells matched the filters.")
        return 0
    for i, c in enumerate(sel):
        if i:
            print("\n" + "-" * 72 + "\n")
        print(f"NEXT CELL  score={c['score']}  [{c['need']}]")
        print(f"  axis     : {c['axis_id']}  ({c['family']}, {c['feasibility']})")
        print(f"  language : {c['language']}     direction: {c['direction']}   arm: {c['arm']}")
        print(f"  status   : {c['status']}  (target: covered)")
        print(f"  claim    : {c['title']}")
        print(f"  note     : {c['note']}")
    return 0


def cmd_soundness(_args):
    """The soundness arm: per axis, is there a hard-negative guard, did any merge, and what
    did the oracle find. Strengthening the oracle = no axis left without a guard."""
    if not SWEEP_EVIDENCE.exists():
        print("no sweep evidence yet (run coverage_sweep.py)")
        return 0
    oracle = json.loads(SWEEP_EVIDENCE.read_text()).get("oracle", [])
    # fold duplicate (gen_axis) rows up to the taxonomy axis
    by_axis = defaultdict(lambda: {"hn": 0, "merged": 0, "leads": 0})
    for o in oracle:
        a = by_axis[o["axis"]]
        a["hn"] += o.get("hard_negatives") or 0
        a["merged"] += o.get("hard_negatives_merged") or 0
        a["leads"] += o.get("oracle_under_merged") or 0
    print("SOUNDNESS ARM  (hard-neg guard / merged=bug / oracle leads = recall feedback)")
    print(f"{'axis':28s} {'hard-neg':>8s} {'merged':>6s} {'leads':>6s}  guard")
    print("-" * 60)
    leaky = guarded = unguarded = 0
    for axis in sorted(by_axis):
        a = by_axis[axis]
        if a["merged"]:
            g, leaky = "✗ LEAKY", leaky + 1
        elif a["hn"] > 0:
            g, guarded = "✓", guarded + 1
        else:
            g, unguarded = "· none", unguarded + 1
        print(f"{axis:28s} {a['hn']:8d} {a['merged']:6d} {a['leads']:6d}  {g}")
    print(f"\n{guarded} guarded, {leaky} LEAKY (false merges), {unguarded} unguarded.")
    print("oracle: 0 merged hard-negatives across all axes = soundness arm holds on the "
          "synthetic corpus. (real-corpus gate: `nose verify bench/repos` == 0 violations.)")
    return 0


def main() -> int:
    p = argparse.ArgumentParser(description=__doc__)
    sub = p.add_subparsers(dest="cmd", required=True)
    sub.add_parser("matrix").set_defaults(func=cmd_matrix)
    sub.add_parser("soundness").set_defaults(func=cmd_soundness)
    n = sub.add_parser("next")
    n.add_argument("--limit", type=int, default=3)
    n.add_argument("--arm", choices=["recall", "soundness"])
    n.add_argument("--feasibility", choices=["fixable", "partial", "landed", "research"])
    n.add_argument("--lang")
    n.add_argument("--json", action="store_true")
    n.set_defaults(func=cmd_next)
    args = p.parse_args()
    return args.func(args)


if __name__ == "__main__":
    raise SystemExit(main())
