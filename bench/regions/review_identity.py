#!/usr/bin/env python3
"""Audit review-key coverage and location-only edits on Nose's Rust corpus."""
import argparse
import collections
import json
import os
from pathlib import Path
import shutil
import subprocess
import tempfile

MODES = ("semantic", "syntax", "near", "abstraction")


def query(binary, corpus, mode, workers=2):
    result = subprocess.run(
        [str(binary), "query", str(corpus), "all", "top=0", "--mode", mode,
         "--format", "json"],
        check=True, capture_output=True, timeout=120,
        env={**os.environ, "RAYON_NUM_THREADS": str(workers)},
    )
    return json.loads(result.stdout)["families"]


def coverage(families):
    counts = {}
    for family in families:
        row = counts.setdefault(family["witness"], {"total": 0, "available": 0, "missing": []})
        row["total"] += 1
        if family["review_key"] is not None:
            row["available"] += 1
        else:
            row["missing"].append({
                "id": family["id"],
                "missing_source": any(l["region"] is None for l in family["locations"]),
                "external_pack": any(k in family for k in ("semantic_pack_near", "semantic_pack_external_exact")),
                "locations": family["locations"],
            })
    return counts


def keys(families):
    return collections.Counter(f["review_key"] for f in families)


def audit(binary, corpus, baseline=None):
    report = {"corpus": str(corpus), "modes": {}, "passed": True}
    with tempfile.TemporaryDirectory(prefix="nose-review-audit-") as temp:
        current = Path(temp) / "original"
        shutil.copytree(corpus, current)
        before = {mode: query(binary, current, mode) for mode in MODES}
        for mode, families in before.items():
            entry = {"coverage": coverage(families)}
            if baseline:
                old = query(baseline, current, mode)
                entry["baseline_coverage"] = coverage(old)
                def legacy(rows):
                    rows = json.loads(json.dumps(rows))
                    for row in rows:
                        row.pop("review_key", None)
                        for loc in row["locations"]:
                            loc.pop("region", None)
                            loc.pop("region_key", None)
                    return rows
                entry["legacy_output_unchanged"] = legacy(old) == legacy(families)
                report["passed"] &= entry["legacy_output_unchanged"]
                previous = {f["id"]: f["review_key"] for f in old if f["review_key"] is not None}
                entry["previous_keys_preserved"] = all(
                    previous[f["id"]] == f["review_key"] for f in families if f["id"] in previous
                )
                entry["corrected_previous_keys"] = sum(
                    previous[f["id"]] != f["review_key"] for f in families if f["id"] in previous
                )
            entry["thread_deterministic"] = families == query(binary, current, mode, 4)
            report["passed"] &= entry["thread_deterministic"] and None not in keys(families)
            report["modes"][mode] = entry
        for path in current.rglob("*.rs"):
            path.write_bytes(b"// unrelated review-identity audit header\r\n" + path.read_bytes())
        for mode in MODES:
            shifted = query(binary, current, mode)
            equal = keys(before[mode]) == keys(shifted)
            report["modes"][mode]["line_shift_stable"] = equal
            if not equal:
                report["modes"][mode]["shift_removed"] = dict(keys(before[mode]) - keys(shifted))
                report["modes"][mode]["shift_added"] = dict(keys(shifted) - keys(before[mode]))
            report["passed"] &= equal
        moved = Path(temp) / "relocated"
        current.rename(moved)
        for mode in MODES:
            equal = keys(before[mode]) == keys(query(binary, moved, mode))
            report["modes"][mode]["file_move_stable"] = equal
            report["passed"] &= equal
    return report


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--nose", type=Path, default=Path("target/release/nose"))
    parser.add_argument("--baseline-nose", type=Path)
    parser.add_argument("--corpus", type=Path, default=Path("crates"))
    parser.add_argument("--output", type=Path, default=Path("target/review-identity-completion/coverage.json"))
    args = parser.parse_args()
    report = audit(args.nose.resolve(), args.corpus.resolve(),
                   args.baseline_nose.resolve() if args.baseline_nose else None)
    args.output.parent.mkdir(parents=True, exist_ok=True)
    args.output.write_text(json.dumps(report, indent=2, sort_keys=True) + "\n")
    print(json.dumps(report, indent=2, sort_keys=True))
    raise SystemExit(0 if report["passed"] else 1)


if __name__ == "__main__":
    main()
