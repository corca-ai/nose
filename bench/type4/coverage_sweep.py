#!/usr/bin/env python3
"""Verify sweep — run each generatable axis through nose per language and record
(axis, language) convergence as coverage evidence.

This fills the cheap "landed but unrecorded" cells with REAL evidence (does nose actually
converge this axis's positive pairs in this language?) and surfaces the true gaps (a positive
pair that does NOT converge → an implement target; a hard-negative that DOES merge → a
soundness bug). It reuses the existing generator (generate.py) and detector logic
(eval_manifest.py), so it is not a parallel oracle — it is the same gate, aggregated per cell.

Output: coverage_evidence.v1.json, consumed by coverage_matrix.py.

  python3 coverage_sweep.py                 # sweep all mapped axes
  python3 coverage_sweep.py --axis numeric_clamp --axis collection_empty_check
  python3 coverage_sweep.py --nose target/debug/nose
"""

from __future__ import annotations

import argparse
import json
import subprocess
import sys
import tempfile
from collections import defaultdict
from pathlib import Path

from eval_manifest import build_family_index, item_detected, run_scan

HERE = Path(__file__).resolve().parent
REPO_ROOT = HERE.parents[1]
EVIDENCE = HERE / "coverage_evidence.v1.json"

# generator axis (generate.py) -> taxonomy axis_id (coverage_taxonomy.py).
# Unmapped generator axes are recorded under their own name.
GEN_TO_AXIS = {
    "collection_empty_check": "collection_empty_check",
    "string_prefix_suffix": "string_prefix_suffix",
    "literal_collection_membership": "membership_contains",
    "map_key_membership": "membership_contains",
    "null_presence_predicate": "null_option_presence",
    "nullish_default": "null_option_presence",
    "map_default_lookup": "map_default_lookup",
    "literal_map_default_lookup": "map_default_lookup",
    "table_access": "map_default_lookup",
    "numeric_minmax_abs": "numeric_minmax_abs",
    "numeric_clamp": "numeric_clamp",
    "total_order_compare": "total_order_compare",
    "own_property_guard": "own_property_guard",
    "record_shape_guard": "property_type_guard",
    "projection_identity": "property_type_guard",
    "import_identity": "import_identity",
    "immutable_binding": "immutable_binding",
    "proven_callee_identity": "proven_callee_identity",
    "hof_filter_map": "filter_fusion",
    "java_integer_low_bit_toggle": "java_integer_low_bit_toggle",
    "java_statically_false_loop": "java_statically_false_loop",
    "c_u16_be_byte_pack": "c_u16_be_byte_pack",
    "c_u32_be_byte_pack": "c_u32_be_byte_pack",
    "python_docstring_noop": "python_docstring_noop",
    # unsafe_boundary generates pure hard-negatives (soundness probes), no positive cell.
    "unsafe_boundary": "identity_value_soundness",
}


def generatable_axes() -> list[str]:
    import generate
    return sorted({v["axis"] for v in generate.AXIS_PROPOSALS.values()})


def gen_manifest(gen_axis: str, out_dir: Path, cross: str = "none") -> Path:
    subprocess.run(
        [sys.executable, str(HERE / "generate.py"), "--axis", gen_axis,
         "--cross", cross, "--out-dir", str(out_dir)],
        check=True, capture_output=True, text=True,
    )
    return out_dir / "manifest.json"


def sweep_axis(gen_axis: str, nose: Path) -> dict:
    """Per-language same-language convergence for one generator axis."""
    with tempfile.TemporaryDirectory() as td:
        out = Path(td)
        manifest_path = gen_manifest(gen_axis, out)
        manifest = json.loads(manifest_path.read_text())
        families = run_scan(nose, out / "sources")
        index = build_family_index(families)
        cells: dict[str, dict[str, int]] = defaultdict(
            lambda: {"pos": 0, "pos_hit": 0, "neg": 0, "neg_hit": 0})
        for item in manifest["items"]:
            if item["left"]["language"] != item["right"]["language"]:
                continue  # per-cell coverage is same-language; cross-lang swept separately
            lang = item["left"]["language"]
            hit = item_detected(item, index, manifest_path.parent)
            row = cells[lang]
            if item["expected_exact_detect"]:
                row["pos"] += 1
                row["pos_hit"] += int(hit)
            else:
                row["neg"] += 1
                row["neg_hit"] += int(hit)
        return cells


def cell_status(row: dict[str, int]) -> str:
    if row["neg_hit"] > 0:
        return "false-merge"          # soundness bug — overrides everything
    if row["pos"] == 0:
        return "no-positive"          # generator emits only negatives here
    if row["pos_hit"] == row["pos"]:
        return "covered"
    if row["pos_hit"] == 0:
        return "gap"                  # nothing converges — a real miss
    return "partial"                  # some converge — a real partial gap


def main() -> int:
    ap = argparse.ArgumentParser(description=__doc__)
    ap.add_argument("--nose", default=str(REPO_ROOT / "target" / "debug" / "nose"))
    ap.add_argument("--axis", action="append", help="generator axis (repeatable); default all")
    ap.add_argument("--quiet", action="store_true")
    args = ap.parse_args()

    nose = Path(args.nose)
    if not nose.exists():
        print(f"error: nose binary not found at {nose} (cargo build first)", file=sys.stderr)
        return 2

    gen_axes = args.axis or generatable_axes()
    evidence = []
    print(f"{'taxonomy_axis':26s} {'lang':11s} {'status':12s} pos      fm")
    print("-" * 64)
    for gen_axis in gen_axes:
        tax_axis = GEN_TO_AXIS.get(gen_axis, gen_axis)
        try:
            cells = sweep_axis(gen_axis, nose)
        except subprocess.CalledProcessError as exc:
            print(f"  ! {gen_axis}: generate/scan failed: {exc.stderr[:120] if exc.stderr else exc}",
                  file=sys.stderr)
            continue
        for lang in sorted(cells):
            row = cells[lang]
            status = cell_status(row)
            evidence.append({
                "axis": tax_axis, "gen_axis": gen_axis, "language": lang,
                "status": status, "pos_hit": row["pos_hit"], "pos": row["pos"],
                "false_merges": row["neg_hit"], "neg": row["neg"], "source": "sweep",
            })
            flag = "  <-- SOUNDNESS" if status == "false-merge" else (
                "  <-- GAP" if status in ("gap", "partial") else "")
            if not args.quiet:
                print(f"{tax_axis:26s} {lang:11s} {status:12s} "
                      f"{row['pos_hit']}/{row['pos']:<4d} {row['neg_hit']}/{row['neg']:<4d}{flag}")

    EVIDENCE.write_text(
        json.dumps({"schema_version": 1, "evidence": sorted(
            evidence, key=lambda e: (e["axis"], e["language"]))}, indent=2) + "\n")
    covered = sum(1 for e in evidence if e["status"] == "covered")
    gaps = [e for e in evidence if e["status"] in ("gap", "partial")]
    fm = [e for e in evidence if e["status"] == "false-merge"]
    print(f"\nswept {len(evidence)} cells: {covered} covered, {len(gaps)} gaps, {len(fm)} false-merges")
    if gaps:
        print("REAL GAPS (implement targets):")
        for e in gaps:
            print(f"  {e['axis']} / {e['language']}  {e['pos_hit']}/{e['pos']}")
    if fm:
        print("SOUNDNESS BUGS (must fix):")
        for e in fm:
            print(f"  {e['axis']} / {e['language']}  false-merges {e['false_merges']}/{e['neg']}")
    print(f"wrote {EVIDENCE.name}")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
