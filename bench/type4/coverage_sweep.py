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
import re
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


def run_oracle(nose: Path, sources: Path) -> dict:
    """Strong soundness arm: run the interpreter oracle (`nose verify`) over the generated
    corpus. It checks EVERY fingerprint-equal pair on an input battery (catching coincidental
    false merges the labeled hard-negatives miss), and exports under-merged behavior-equal
    pairs as recall leads — so one run advances BOTH arms.

    NOTE: on the *synthetic* corpus the canon-preservation count is a characterized
    oracle-fidelity BUDGET (e.g. C 1/0 == bool, experiments §A2), NOT false merges; the
    real-corpus 0-violation gate is `nose verify bench/repos`. We therefore record the
    oracle signal (leads + completeness) rather than gate on the synthetic artifact count.
    """
    proc = subprocess.run([str(nose), "verify", str(sources)],
                          capture_output=True, text=True)
    out = proc.stdout + proc.stderr

    def grab(pat, default=None):
        m = re.search(pat, out)
        return int(m.group(1)) if m else default

    return {
        "under_merged": grab(r"under-merged behavior groups[^:]*:\s*(\d+)"),
        "completeness_pct": grab(r"completeness:.*?=\s*(\d+)%"),
        "canon_changed": grab(r"(\d+)\s+unit\(s\) whose behavior CHANGED"),  # synthetic budget
        "exit": proc.returncode,
    }


def sweep_axis(gen_axis: str, nose: Path) -> tuple[dict, dict]:
    """Per-language same-language convergence + the oracle soundness/leads signal."""
    with tempfile.TemporaryDirectory() as td:
        out = Path(td)
        manifest_path = gen_manifest(gen_axis, out)
        manifest = json.loads(manifest_path.read_text())
        families = run_scan(nose, out / "sources")
        oracle = run_oracle(nose, out / "sources")
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
        return cells, oracle


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
    oracle_rows = []
    print(f"{'taxonomy_axis':26s} {'lang':11s} {'status':12s} pos      fm")
    print("-" * 64)
    for gen_axis in gen_axes:
        tax_axis = GEN_TO_AXIS.get(gen_axis, gen_axis)
        try:
            cells, oracle = sweep_axis(gen_axis, nose)
        except subprocess.CalledProcessError as exc:
            print(f"  ! {gen_axis}: generate/scan failed: {exc.stderr[:120] if exc.stderr else exc}",
                  file=sys.stderr)
            continue
        # Soundness arm: did the generator's hard negatives stay un-merged, and what did the
        # oracle find (under-merged leads = recall feedback; canon_changed = synthetic budget).
        hard_neg = sum(c["neg"] for c in cells.values())
        hard_neg_merged = sum(c["neg_hit"] for c in cells.values())
        oracle_rows.append({
            "axis": tax_axis, "gen_axis": gen_axis,
            "hard_negatives": hard_neg, "hard_negatives_merged": hard_neg_merged,
            "oracle_under_merged": oracle.get("under_merged"),
            "oracle_completeness_pct": oracle.get("completeness_pct"),
            "oracle_canon_changed_budget": oracle.get("canon_changed"),
        })
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

    # Merge into existing evidence: a filtered run (--axis) updates only the cells/axes it
    # swept, never clobbering the rest of the matrix.
    prev = json.loads(EVIDENCE.read_text()) if EVIDENCE.exists() else {}
    merged: dict[tuple, dict] = {(e["gen_axis"], e["language"]): e
                                 for e in prev.get("evidence", [])}
    for e in evidence:
        merged[(e["gen_axis"], e["language"])] = e
    merged_oracle: dict[str, dict] = {o["gen_axis"]: o for o in prev.get("oracle", [])}
    for o in oracle_rows:
        merged_oracle[o["gen_axis"]] = o
    rows = sorted(merged.values(), key=lambda e: (e["axis"], e["gen_axis"], e["language"]))
    oracle_out = sorted(merged_oracle.values(), key=lambda o: (o["axis"], o["gen_axis"]))
    EVIDENCE.write_text(json.dumps(
        {"schema_version": 1, "evidence": rows, "oracle": oracle_out}, indent=2) + "\n")

    covered = sum(1 for e in evidence if e["status"] == "covered")
    gaps = [e for e in evidence if e["status"] in ("gap", "partial")]
    fm = [e for e in evidence if e["status"] == "false-merge"]
    unguarded = [o for o in oracle_rows if o["hard_negatives"] == 0]
    leaky = [o for o in oracle_rows if o["hard_negatives_merged"]]
    print(f"\nswept {len(evidence)} cells: {covered} covered, {len(gaps)} gaps, {len(fm)} false-merges")
    print(f"soundness arm: {len(oracle_rows)} axes oracle-checked; "
          f"{len(leaky)} with merged hard-negatives; {len(unguarded)} with NO hard-negative guard")
    if gaps:
        print("REAL RECALL GAPS (implement targets):")
        for e in gaps:
            print(f"  {e['axis']} / {e['language']}  {e['pos_hit']}/{e['pos']}")
    if fm or leaky:
        print("SOUNDNESS BUGS (must fix):")
        for e in fm:
            print(f"  false-merge: {e['axis']} / {e['language']}  {e['false_merges']}/{e['neg']}")
        for o in leaky:
            print(f"  oracle: {o['axis']} merged {o['hard_negatives_merged']}/{o['hard_negatives']} hard-negs")
    if unguarded:
        print("NO HARD-NEGATIVE GUARD (soundness arm not advanced for these axes):")
        for o in unguarded:
            print(f"  {o['axis']}")
    print(f"wrote {EVIDENCE.name}")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
