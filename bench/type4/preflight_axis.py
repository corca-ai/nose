#!/usr/bin/env python3
"""Preflight a Type-4 frontier loop before spending detector work.

The loop is worth entering only when the candidate improves strict recall or
removes baseline false merges without introducing any candidate false merge.
This catches benchmark-only expansions that do not actually strengthen the
detector.
"""

from __future__ import annotations

import argparse
import json
import subprocess
from pathlib import Path

import eval_manifest


ROOT = Path(__file__).resolve().parents[2]


def run_generate(out_dir: Path, axis: str, cross: str, proposal_prefix: str | None) -> None:
    cmd = [
        "python3",
        str(ROOT / "bench" / "type4" / "generate.py"),
        "--out-dir",
        str(out_dir),
        "--axis",
        axis,
        "--cross",
        cross,
    ]
    if proposal_prefix:
        cmd.extend(["--proposal-prefix", proposal_prefix])
    subprocess.run(cmd, check=True)


def evaluate(manifest_path: Path, nose: Path) -> dict[str, int]:
    manifest_path = manifest_path.resolve()
    manifest_dir = manifest_path.parent
    manifest = json.loads(manifest_path.read_text())
    families = eval_manifest.run_scan(nose, manifest_dir / "sources")
    family_index = eval_manifest.build_family_index(families)
    positives = [item for item in manifest["items"] if item["expected_exact_detect"]]
    negatives = [item for item in manifest["items"] if not item["expected_exact_detect"]]
    detected = {
        item["case_id"]: eval_manifest.item_detected(item, family_index, manifest_dir)
        for item in manifest["items"]
    }
    pos_hits = sum(1 for item in positives if detected[item["case_id"]])
    false_merges = sum(1 for item in negatives if detected[item["case_id"]])
    return {
        "items": len(manifest["items"]),
        "positives": len(positives),
        "positive_hits": pos_hits,
        "positive_misses": len(positives) - pos_hits,
        "negatives": len(negatives),
        "false_merges": false_merges,
    }


def print_row(label: str, row: dict[str, int]) -> None:
    print(
        f"{label}: items={row['items']} "
        f"positive={row['positive_hits']}/{row['positives']} "
        f"misses={row['positive_misses']} "
        f"false_merges={row['false_merges']}/{row['negatives']}"
    )


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--axis", required=True)
    parser.add_argument("--proposal-prefix")
    parser.add_argument("--cross", default="ring")
    parser.add_argument("--out-dir", required=True, type=Path)
    parser.add_argument("--baseline", default=Path("target/release/nose"), type=Path)
    parser.add_argument("--candidate", default=Path("target/debug/nose"), type=Path)
    parser.add_argument("--allow-no-baseline-miss", action="store_true")
    args = parser.parse_args()

    run_generate(args.out_dir, args.axis, args.cross, args.proposal_prefix)
    manifest = args.out_dir / "manifest.json"
    baseline = evaluate(manifest, args.baseline)
    candidate = evaluate(manifest, args.candidate)
    print_row("baseline", baseline)
    print_row("candidate", candidate)

    if candidate["false_merges"] > 0:
        print("preflight failed: candidate introduces false merges")
        return 2
    recall_improved = candidate["positive_misses"] < baseline["positive_misses"]
    false_merges_removed = candidate["false_merges"] < baseline["false_merges"]
    if (
        baseline["positive_misses"] == 0
        and baseline["false_merges"] == 0
        and not args.allow_no_baseline_miss
    ):
        print("preflight failed: baseline already covers all strict positives")
        return 3
    if not (recall_improved or false_merges_removed):
        print("preflight failed: candidate does not improve strict recall or false merges")
        return 4
    print("preflight passed: candidate improves the frontier with zero false merges")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
