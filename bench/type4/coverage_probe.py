#!/usr/bin/env python3
"""Focused-probe coverage for axes generate.py has no generator for.

Each axis/language carries a checked-in POSITIVE pair (must converge — recall) and one or
more adjacent HARD-NEGATIVE pairs (must NOT converge — the soundness guard). The runner
scans each pair and records the cell to coverage_evidence.v1.json (source="probe"),
advancing BOTH arms at once. A hard-negative that converges is a soundness bug.

Layout:
  coverage_probes/<axis>/<lang>/pos/{a,b}.<ext>
  coverage_probes/<axis>/<lang>/neg-<tag>/{a,b}.<ext>

  python3 coverage_probe.py [--nose target/debug/nose] [--axis reduce_minmax_anyall]
"""

from __future__ import annotations

import argparse
import json
import subprocess
import sys
from pathlib import Path

from eval_manifest import scan_families

HERE = Path(__file__).resolve().parent
PROBES = HERE / "coverage_probes"
EVIDENCE = HERE / "coverage_evidence.v1.json"
NOSE_DEFAULT = str(HERE.parents[1] / "target" / "debug" / "nose")


def converges(nose: str, pair_dir: Path) -> bool:
    """True iff nose reports a semantic family spanning the two files in pair_dir."""
    cmd = [nose, "scan", str(pair_dir), "--mode", "semantic", "--format", "json",
           "--top", "1000000", "--min-size", "1", "--min-lines", "1"]
    proc = subprocess.run(cmd, capture_output=True, text=True)
    families = scan_families(json.loads(proc.stdout or "[]"))
    files = {f.name for f in pair_dir.iterdir() if f.is_file()}
    for fam in families:
        # Skip Block sub-units (eval_manifest convention): a bare loop block with no escaping
        # effect is observably a no-op, so two of them are vacuously equivalent — that is a
        # SOUND collision, not a clone of the intended whole-unit. Count only real units.
        locs = {Path(loc["file"]).name for loc in fam.get("locations", [])
                if loc.get("kind") != "Block"}
        if len(locs & files) >= 2:
            return True
    return False


def probe_cell(nose: str, axis_dir: Path, lang: str) -> dict | None:
    lang_dir = axis_dir / lang
    pos = lang_dir / "pos"
    if not pos.is_dir():
        return None
    pos_ok = converges(nose, pos)
    neg_dirs = sorted(d for d in lang_dir.iterdir() if d.is_dir() and d.name.startswith("neg"))
    merged = [d.name for d in neg_dirs if converges(nose, d)]
    if merged:
        status = "false-merge"
    elif pos_ok:
        status = "covered"
    else:
        status = "gap"
    return {
        "axis": axis_dir.name, "gen_axis": f"probe:{axis_dir.name}", "language": lang,
        "status": status, "pos_hit": int(pos_ok), "pos": 1,
        "false_merges": len(merged), "neg": len(neg_dirs), "source": "probe",
    }


def main() -> int:
    ap = argparse.ArgumentParser(description=__doc__)
    ap.add_argument("--nose", default=NOSE_DEFAULT)
    ap.add_argument("--axis", action="append", help="axis dir name (repeatable); default all")
    args = ap.parse_args()
    if not Path(args.nose).exists():
        print(f"error: nose not found at {args.nose}", file=sys.stderr)
        return 2
    if not PROBES.is_dir():
        print(f"no probes dir at {PROBES}")
        return 0

    axis_dirs = [PROBES / a for a in args.axis] if args.axis else sorted(
        d for d in PROBES.iterdir() if d.is_dir())
    rows = []
    print(f"{'axis':26s} {'lang':11s} {'status':12s} pos  hard-neg")
    print("-" * 60)
    for axis_dir in axis_dirs:
        for lang_dir in sorted(d for d in axis_dir.iterdir() if d.is_dir()):
            cell = probe_cell(args.nose, axis_dir, lang_dir.name)
            if cell is None:
                continue
            rows.append(cell)
            flag = "  <-- SOUNDNESS BUG" if cell["status"] == "false-merge" else (
                "  <-- GAP" if cell["status"] == "gap" else "")
            print(f"{cell['axis']:26s} {cell['language']:11s} {cell['status']:12s} "
                  f"{cell['pos_hit']}/1  {cell['neg'] - cell['false_merges']}/{cell['neg']}{flag}")

    # merge into evidence (probe rows keyed distinct from sweep via gen_axis="probe:<axis>")
    prev = json.loads(EVIDENCE.read_text()) if EVIDENCE.exists() else {}
    merged: dict[tuple, dict] = {(e["gen_axis"], e["language"]): e
                                 for e in prev.get("evidence", [])}
    for e in rows:
        merged[(e["gen_axis"], e["language"])] = e
    out = sorted(merged.values(), key=lambda e: (e["axis"], e["gen_axis"], e["language"]))
    EVIDENCE.write_text(json.dumps(
        {"schema_version": 1, "evidence": out, "oracle": prev.get("oracle", [])}, indent=2) + "\n")
    covered = sum(1 for r in rows if r["status"] == "covered")
    bugs = [r for r in rows if r["status"] == "false-merge"]
    gaps = [r for r in rows if r["status"] == "gap"]
    print(f"\nprobed {len(rows)} cells: {covered} covered, {len(gaps)} gaps, {len(bugs)} soundness bugs")
    if bugs:
        print("SOUNDNESS BUGS (hard negative converged — must fix):")
        for r in bugs:
            print(f"  {r['axis']} / {r['language']}")
        return 1
    print(f"wrote {EVIDENCE.name}")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
