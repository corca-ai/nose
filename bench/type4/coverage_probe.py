#!/usr/bin/env python3
"""Focused-probe coverage for axes generate.py has no generator for.

Each axis/language carries a checked-in POSITIVE pair (must converge — recall) and one or
more adjacent HARD-NEGATIVE pairs (must NOT converge — the soundness guard). The runner
queries each pair and records the cell to coverage_evidence.v1.json (source="probe"),
advancing BOTH arms at once. A hard-negative that converges is a soundness bug.

Layout:
  coverage_probes/<axis>/<lang>/pos/{a,b}.<ext>
  coverage_probes/<axis>/<lang>/neg-<tag>/{a,b}.<ext>

Soundness-family axes may omit `pos/` and provide only `neg-*` siblings. Those cells are
recorded as `no-positive`, which the matrix counts only as a soundness hard-negative.

  python3 coverage_probe.py [--nose target/debug/nose] [--axis reduce_minmax_anyall]
"""

from __future__ import annotations

import argparse
import hashlib
import json
import subprocess
import sys
import tempfile
from pathlib import Path

import coverage_taxonomy as tax

HERE = Path(__file__).resolve().parent
REPO_ROOT = HERE.parents[1]
PROBES = HERE / "coverage_probes"
EVIDENCE = HERE / "coverage_evidence.v1.json"
NOSE_DEFAULT = str(REPO_ROOT / "target" / "debug" / "nose")


def corpus_identity(root: Path) -> tuple[str, int]:
    digest = hashlib.sha256()
    files = sorted(path for path in root.rglob("*") if path.is_file())
    for path in files:
        relative = path.relative_to(root).as_posix().encode()
        digest.update(len(relative).to_bytes(8, "big"))
        digest.update(relative)
        payload = path.read_bytes()
        digest.update(len(payload).to_bytes(8, "big"))
        digest.update(payload)
    return digest.hexdigest(), len(files)


def run_blind_attacker(nose: str, output: Path) -> bool:
    """Run the oracle over the whole probe corpus without consulting case labels."""
    with tempfile.TemporaryDirectory() as tmp:
        full_report = Path(tmp) / "report.json"
        proc = subprocess.run(
            [
                nose,
                "verify",
                str(PROBES),
                "--max-violations",
                "0",
                "--recall-loss-report",
                str(full_report),
            ],
            capture_output=True,
            text=True,
        )
        if not full_report.is_file():
            detail = (proc.stdout + proc.stderr)[-2000:]
            raise RuntimeError(f"blind attacker did not emit its report:\n{detail}")
        report = json.loads(full_report.read_text())

    summary = report["summary"]
    gate = report["soundness_gate"]
    exclusions = {
        row["reason"]: row["count"] for row in report["oracle_exclusions"]["counts"]
    }
    corpus_sha256, corpus_files = corpus_identity(PROBES)
    crates_tree = subprocess.run(
        ["git", "rev-parse", "HEAD:crates"],
        cwd=REPO_ROOT,
        check=True,
        capture_output=True,
        text=True,
    ).stdout.strip()
    compact = {
        "schema_version": 1,
        "attacker": "blind-oracle",
        "corpus": "bench/type4/coverage_probes",
        "corpus_sha256": corpus_sha256,
        "corpus_files": corpus_files,
        "product_crates_tree": crates_tree,
        "summary": {
            "total_units": summary["total_units"],
            "interpretable_units": summary["interpretable_units"],
            "excluded_units": summary["excluded_units"],
            "canon_checked": summary["canon_checked"],
            "admission_rejections": summary["admission_rejections"],
        },
        "hard_gate": {
            "fingerprint_groups": gate["fingerprint_groups"],
            "false_merges": gate["false_merges"],
            "canon_preservation_violations": gate["canon_preservation_violations"],
            "gate_passed": gate["gate_passed"],
        },
        "advisory_disagreements": gate["advisory_disagreements"],
        "oracle_exclusions": exclusions,
    }
    output.write_text(json.dumps(compact, indent=2, sort_keys=True) + "\n")
    print(
        "blind attacker: "
        f"{compact['hard_gate']['fingerprint_groups']} exact groups, "
        f"{compact['hard_gate']['false_merges']} false merges, "
        f"{compact['hard_gate']['canon_preservation_violations']} canon violations"
    )
    print(f"wrote {output.resolve().relative_to(REPO_ROOT)}")
    return proc.returncode == 0 and gate["gate_passed"]


def converges(nose: str, pair_dir: Path) -> bool:
    """True iff nose reports an exact semantic family spanning the two files in pair_dir."""
    cmd = [
        nose, "query", str(pair_dir), "all", "witness=exact",
        "--mode", "semantic", "--format", "json", "--min-size", "1",
    ]
    proc = subprocess.run(cmd, capture_output=True, text=True)
    if proc.returncode != 0:
        raise RuntimeError(
            f"nose query failed for {pair_dir} (exit {proc.returncode}):\n"
            f"{proc.stderr[-2000:]}"
        )
    families = json.loads(proc.stdout or "{}").get("families", [])
    files = {f.name for f in pair_dir.iterdir() if f.is_file()}
    for fam in families:
        if fam.get("witness") != "exact":
            continue
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
    neg_dirs = sorted(d for d in lang_dir.iterdir() if d.is_dir() and d.name.startswith("neg"))
    has_pos = pos.is_dir()
    if not has_pos:
        axis = tax.axis_index().get(axis_dir.name)
        if not neg_dirs or not axis or axis.get("family") != "soundness":
            return None
        pos_ok = False
    else:
        pos_ok = converges(nose, pos)
    merged = [d.name for d in neg_dirs if converges(nose, d)]
    if merged:
        status = "false-merge"
    elif pos_ok:
        status = "covered"
    elif not has_pos:
        status = "no-positive"
    else:
        status = "gap"
    return {
        "axis": axis_dir.name, "gen_axis": f"probe:{axis_dir.name}", "language": lang,
        "status": status, "pos_hit": int(pos_ok), "pos": int(has_pos),
        "false_merges": len(merged), "neg": len(neg_dirs), "source": "probe",
    }


def main() -> int:
    ap = argparse.ArgumentParser(description=__doc__)
    ap.add_argument("--nose", default=NOSE_DEFAULT)
    ap.add_argument("--axis", action="append", help="axis dir name (repeatable); default all")
    ap.add_argument(
        "--blind-report",
        type=Path,
        help="also run the label-blind oracle over every probe and write a compact receipt",
    )
    args = ap.parse_args()
    if not Path(args.nose).exists():
        print(f"error: nose not found at {args.nose}", file=sys.stderr)
        return 2
    if not PROBES.is_dir():
        print(f"error: focused probe corpus is missing at {PROBES}", file=sys.stderr)
        return 2

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
                  f"{cell['pos_hit']}/{cell['pos']}  "
                  f"{cell['neg'] - cell['false_merges']}/{cell['neg']}{flag}")

    # Merge into evidence (probe rows are keyed separately from sweep). A full run replaces
    # every probe row, so deleting or renaming a fixture cannot leave stale exact credit behind.
    # A filtered development run preserves probe rows belonging to axes it did not exercise.
    prev = json.loads(EVIDENCE.read_text()) if EVIDENCE.exists() else {}
    selected = {axis_dir.name for axis_dir in axis_dirs}
    merged: dict[tuple, dict] = {
        (e["gen_axis"], e["language"]): e
        for e in prev.get("evidence", [])
        if e.get("source") != "probe"
        or (args.axis and e.get("axis") not in selected)
    }
    for e in rows:
        merged[(e["gen_axis"], e["language"])] = e
    out = sorted(merged.values(), key=lambda e: (e["axis"], e["gen_axis"], e["language"]))
    EVIDENCE.write_text(json.dumps(
        {"schema_version": 1, "evidence": out, "oracle": prev.get("oracle", [])}, indent=2) + "\n")
    covered = sum(1 for r in rows if r["status"] == "covered")
    hard_negatives = sum(1 for r in rows if r["status"] == "no-positive")
    bugs = [r for r in rows if r["status"] == "false-merge"]
    gaps = [r for r in rows if r["status"] == "gap"]
    print(f"\nprobed {len(rows)} cells: {covered} covered, "
          f"{hard_negatives} soundness hard-negatives, {len(gaps)} gaps, "
          f"{len(bugs)} soundness bugs")
    blind_ok = True
    if args.blind_report:
        blind_ok = run_blind_attacker(args.nose, args.blind_report)
    if bugs or not blind_ok:
        print("SOUNDNESS BUGS (hard negative converged — must fix):")
        for r in bugs:
            print(f"  {r['axis']} / {r['language']}")
        return 1
    print(f"wrote {EVIDENCE.name}")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
