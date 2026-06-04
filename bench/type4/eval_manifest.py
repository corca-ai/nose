#!/usr/bin/env python3
"""Evaluate a nose JSON scan against a generated Type-4 benchmark manifest."""

from __future__ import annotations

import argparse
from collections import defaultdict
import json
import subprocess
import sys
from pathlib import Path


def abs_path(base: Path, rel: str) -> str:
    return str((base / rel).resolve())


def overlaps(loc: dict, src: dict, manifest_dir: Path) -> bool:
    if loc.get("kind") == "Block":
        return False
    if str(Path(loc["file"]).resolve()) != abs_path(manifest_dir, src["path"]):
        return False
    return not (loc["end_line"] < src["start_line"] or src["end_line"] < loc["start_line"])


def item_detected(item: dict, families: list[dict], manifest_dir: Path) -> bool:
    for family in families:
        locations = family.get("locations", [])
        has_left = any(overlaps(loc, item["left"], manifest_dir) for loc in locations)
        has_right = any(overlaps(loc, item["right"], manifest_dir) for loc in locations)
        if has_left and has_right:
            return True
    return False


def run_scan(nose: Path, sources: Path) -> list[dict]:
    cmd = [
        str(nose),
        "scan",
        str(sources),
        "--mode",
        "semantic",
        "--format",
        "json",
        "--top",
        "1000000",
        "--min-tokens",
        "1",
        "--min-lines",
        "1",
    ]
    proc = subprocess.run(cmd, check=True, capture_output=True, text=True)
    return json.loads(proc.stdout or "[]")


def count_row() -> dict[str, int]:
    return {"pos": 0, "pos_hit": 0, "neg": 0, "neg_hit": 0}


def record_detection(row: dict[str, int], item: dict, hit: bool) -> None:
    if item["expected_exact_detect"]:
        row["pos"] += 1
        row["pos_hit"] += int(hit)
    else:
        row["neg"] += 1
        row["neg_hit"] += int(hit)


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("manifest", type=Path)
    parser.add_argument("--scan-json", type=Path)
    parser.add_argument("--nose", default=Path("target/release/nose"), type=Path)
    parser.add_argument("--fail-on-false-merge", action="store_true")
    args = parser.parse_args()

    manifest_path = args.manifest.resolve()
    manifest_dir = manifest_path.parent
    manifest = json.loads(manifest_path.read_text())
    if args.scan_json:
        families = json.loads(args.scan_json.read_text())
    else:
        families = run_scan(args.nose, manifest_dir / "sources")

    positives = [i for i in manifest["items"] if i["expected_exact_detect"]]
    negatives = [i for i in manifest["items"] if not i["expected_exact_detect"]]
    detected = {i["case_id"]: item_detected(i, families, manifest_dir) for i in manifest["items"]}

    pos_hits = sum(1 for i in positives if detected[i["case_id"]])
    false_merges = [i for i in negatives if detected[i["case_id"]]]
    print(f"items: {len(manifest['items'])}")
    print(f"positive recall: {pos_hits}/{len(positives)}")
    print(f"hard-negative false merges: {len(false_merges)}/{len(negatives)}")

    by_split: dict[str, dict[str, int]] = defaultdict(count_row)
    for item in manifest["items"]:
        row = by_split[item["split"]]
        record_detection(row, item, detected[item["case_id"]])

    print("\nby split:")
    for split in sorted(by_split):
        row = by_split[split]
        print(
            f"  {split}: "
            f"positive {row['pos_hit']}/{row['pos']}, "
            f"false merges {row['neg_hit']}/{row['neg']}"
        )

    by_proposal: dict[str, dict[str, int]] = defaultdict(count_row)
    for item in manifest["items"]:
        row = by_proposal[item["proposal_id"]]
        record_detection(row, item, detected[item["case_id"]])

    print("\nby proposal:")
    for proposal_id in sorted(by_proposal):
        row = by_proposal[proposal_id]
        print(
            f"  {proposal_id}: "
            f"positive {row['pos_hit']}/{row['pos']}, "
            f"false merges {row['neg_hit']}/{row['neg']}"
        )

    by_axis: dict[str, dict[str, int]] = defaultdict(count_row)
    for item in manifest["items"]:
        for axis in item.get("matrix", {}).get("semantic_axes", []):
            row = by_axis[axis]
            record_detection(row, item, detected[item["case_id"]])

    if by_axis:
        print("\nby semantic axis:")
        for axis in sorted(by_axis):
            row = by_axis[axis]
            print(
                f"  {axis}: "
                f"positive {row['pos_hit']}/{row['pos']}, "
                f"false merges {row['neg_hit']}/{row['neg']}"
            )

    by_negative_tag: dict[str, dict[str, int]] = defaultdict(lambda: {"neg": 0, "neg_hit": 0})
    for item in negatives:
        tag = item.get("matrix", {}).get("negative_tag") or "unspecified"
        row = by_negative_tag[tag]
        row["neg"] += 1
        row["neg_hit"] += int(detected[item["case_id"]])

    if by_negative_tag:
        print("\nby negative tag:")
        for tag in sorted(by_negative_tag):
            row = by_negative_tag[tag]
            print(f"  {tag}: false merges {row['neg_hit']}/{row['neg']}")

    misses = [i for i in positives if not detected[i["case_id"]]]
    if misses:
        print("\nmissed positives:")
        for item in misses[:20]:
            print(
                f"  {item['case_id']} {item['proposal_id']} "
                f"{item['left']['surface']}:{item['left']['representation']} -> "
                f"{item['right']['surface']}:{item['right']['representation']}"
            )

    if false_merges:
        print("\nfalse merges:")
        for item in false_merges[:20]:
            ce = item["evidence"].get("counterexample", {})
            print(
                f"  {item['case_id']} {item['proposal_id']} "
                f"{item['left']['surface']} -> {item['right']['surface']} "
                f"counterexample={ce}"
            )

    if false_merges and args.fail_on_false_merge:
        return 1
    return 0


if __name__ == "__main__":
    sys.exit(main())
