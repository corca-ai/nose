#!/usr/bin/env python3
"""Summarize Type-4 benchmark misses into detector-improvement frontiers."""

from __future__ import annotations

import argparse
from collections import Counter, defaultdict
import json
from pathlib import Path

from eval_manifest import item_detected, run_scan


def surface_key(item: dict) -> str:
    left = item["left"]
    right = item["right"]
    return (
        f"{left['surface']}:{left['representation']}"
        f" -> {right['surface']}:{right['representation']}"
    )


def proposal_table(items: list[dict], detected: dict[str, bool]) -> list[tuple[str, int, int]]:
    rows: dict[str, list[int]] = defaultdict(lambda: [0, 0])
    for item in items:
        row = rows[item["proposal_id"]]
        row[0] += 1
        row[1] += int(detected[item["case_id"]])
    return sorted((proposal, total, hit) for proposal, (total, hit) in rows.items())


def print_counter(title: str, counter: Counter[str], limit: int) -> None:
    if not counter:
        return
    print(f"\n{title}:")
    for key, count in counter.most_common(limit):
        print(f"  {count:3d}  {key}")


def counter_dict(counter: Counter[str]) -> dict[str, int]:
    return {key: counter[key] for key in sorted(counter)}


def build_summary(manifest: dict, families: list[dict], manifest_dir: Path) -> dict:
    positives = [i for i in manifest["items"] if i["expected_exact_detect"]]
    negatives = [i for i in manifest["items"] if not i["expected_exact_detect"]]
    detected = {i["case_id"]: item_detected(i, families, manifest_dir) for i in manifest["items"]}
    misses = [i for i in positives if not detected[i["case_id"]]]
    false_merges = [i for i in negatives if detected[i["case_id"]]]

    by_proposal = {
        proposal: {"total": total, "hit": hit, "miss": total - hit}
        for proposal, total, hit in proposal_table(positives, detected)
    }
    by_pair = Counter(surface_key(item) for item in misses)
    by_left = Counter(item["left"]["surface"] for item in misses)
    by_right = Counter(item["right"]["surface"] for item in misses)
    by_relation = Counter(item["matrix"]["language_relation"] for item in misses)
    by_computation = Counter(item["matrix"]["computation"] for item in misses)
    by_split = Counter(item["split"] for item in misses)
    false_merges_by_split = Counter(item["split"] for item in false_merges)
    false_merges_by_negative_tag = Counter(
        (item.get("matrix", {}).get("negative_tag") or "unspecified") for item in false_merges
    )
    by_representation = Counter(
        f"{item['left']['representation']} -> {item['right']['representation']}" for item in misses
    )

    return {
        "items": len(manifest["items"]),
        "positive_total": len(positives),
        "positive_hits": len(positives) - len(misses),
        "positive_misses": len(misses),
        "negative_total": len(negatives),
        "false_merges": len(false_merges),
        "by_proposal": by_proposal,
        "misses_by_surface_pair": counter_dict(by_pair),
        "misses_by_left_surface": counter_dict(by_left),
        "misses_by_right_surface": counter_dict(by_right),
        "misses_by_relation": counter_dict(by_relation),
        "misses_by_computation": counter_dict(by_computation),
        "misses_by_split": counter_dict(by_split),
        "false_merges_by_split": counter_dict(false_merges_by_split),
        "false_merges_by_negative_tag": counter_dict(false_merges_by_negative_tag),
        "misses_by_representation": counter_dict(by_representation),
        "misses": [
            {
                "case_id": item["case_id"],
                "proposal_id": item["proposal_id"],
                "surface_pair": surface_key(item),
                "computation": item["matrix"]["computation"],
                "relation": item["matrix"]["language_relation"],
                "split": item["split"],
            }
            for item in misses
        ],
        "false_merge_items": [
            {
                "case_id": item["case_id"],
                "proposal_id": item["proposal_id"],
                "surface_pair": surface_key(item),
                "split": item["split"],
                "counterexample": item["evidence"].get("counterexample"),
            }
            for item in false_merges
        ],
    }


def print_summary(summary: dict, limit: int) -> None:
    print(f"items: {summary['items']}")
    print(f"positive misses: {summary['positive_misses']}/{summary['positive_total']}")
    print(f"hard-negative false merges: {summary['false_merges']}/{summary['negative_total']}")

    print("\nby proposal:")
    for proposal, row in sorted(summary["by_proposal"].items()):
        print(f"  {proposal}: hit {row['hit']}/{row['total']}, miss {row['miss']}")

    print_counter("misses by surface pair", Counter(summary["misses_by_surface_pair"]), limit)
    print_counter("misses by left surface", Counter(summary["misses_by_left_surface"]), limit)
    print_counter("misses by right surface", Counter(summary["misses_by_right_surface"]), limit)
    print_counter("misses by relation", Counter(summary["misses_by_relation"]), limit)
    print_counter("misses by split", Counter(summary["misses_by_split"]), limit)
    print_counter("misses by computation", Counter(summary["misses_by_computation"]), limit)
    print_counter("misses by representation", Counter(summary["misses_by_representation"]), limit)

    if summary["misses"]:
        print("\nfrontier examples:")
        for item in summary["misses"][:limit]:
            print(f"  {item['case_id']} {item['proposal_id']} {item['surface_pair']}")

    if summary["false_merge_items"]:
        print("\nfalse-merge examples:")
        for item in summary["false_merge_items"][:limit]:
            print(
                f"  {item['case_id']} {item['proposal_id']} "
                f"counterexample={item['counterexample']}"
            )


def compare_summaries(before: dict, after: dict) -> dict:
    def delta_dict(key: str) -> dict[str, int]:
        keys = set(before.get(key, {})) | set(after.get(key, {}))
        return {k: after.get(key, {}).get(k, 0) - before.get(key, {}).get(k, 0) for k in sorted(keys)}

    return {
        "positive_hits_delta": after["positive_hits"] - before["positive_hits"],
        "positive_misses_delta": after["positive_misses"] - before["positive_misses"],
        "false_merges_delta": after["false_merges"] - before["false_merges"],
        "misses_by_surface_pair_delta": delta_dict("misses_by_surface_pair"),
        "misses_by_computation_delta": delta_dict("misses_by_computation"),
        "misses_by_relation_delta": delta_dict("misses_by_relation"),
        "misses_by_split_delta": delta_dict("misses_by_split"),
        "false_merges_by_split_delta": delta_dict("false_merges_by_split"),
    }


def print_comparison(comparison: dict, limit: int) -> None:
    print("\ncomparison:")
    print(f"  positive hits delta: {comparison['positive_hits_delta']:+d}")
    print(f"  positive misses delta: {comparison['positive_misses_delta']:+d}")
    print(f"  false merges delta: {comparison['false_merges_delta']:+d}")
    improved = Counter(
        {
            key: -delta
            for key, delta in comparison["misses_by_surface_pair_delta"].items()
            if delta < 0
        }
    )
    regressed = Counter(
        {
            key: delta
            for key, delta in comparison["misses_by_surface_pair_delta"].items()
            if delta > 0
        }
    )
    print_counter("surface pairs improved", improved, limit)
    print_counter("surface pairs regressed", regressed, limit)


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("manifest", type=Path)
    parser.add_argument("--scan-json", type=Path)
    parser.add_argument("--nose", default=Path("target/release/nose"), type=Path)
    parser.add_argument("--limit", default=20, type=int)
    parser.add_argument("--json-out", type=Path, help="write the frontier summary as JSON")
    parser.add_argument("--compare-to", type=Path, help="compare with a previous frontier JSON")
    parser.add_argument("--compare-out", type=Path, help="write the comparison as JSON")
    parser.add_argument(
        "--fail-on-regression",
        action="store_true",
        help="fail if comparison increases misses or false merges",
    )
    parser.add_argument(
        "--min-positive-hits-delta",
        default=None,
        type=int,
        help="with --compare-to, fail unless positive hit delta is at least this value",
    )
    args = parser.parse_args()

    manifest_path = args.manifest.resolve()
    manifest_dir = manifest_path.parent
    manifest = json.loads(manifest_path.read_text())
    if args.scan_json:
        families = json.loads(args.scan_json.read_text())
    else:
        families = run_scan(args.nose, manifest_dir / "sources")

    summary = build_summary(manifest, families, manifest_dir)
    print_summary(summary, args.limit)
    if args.json_out:
        args.json_out.write_text(json.dumps(summary, indent=2, sort_keys=True) + "\n")
    failed = bool(summary["false_merges"])
    if args.compare_to:
        before = json.loads(args.compare_to.read_text())
        comparison = compare_summaries(before, summary)
        print_comparison(comparison, args.limit)
        if args.compare_out:
            args.compare_out.write_text(json.dumps(comparison, indent=2, sort_keys=True) + "\n")
        if args.fail_on_regression and (
            comparison["positive_hits_delta"] < 0 or comparison["false_merges_delta"] > 0
        ):
            failed = True
        if (
            args.min_positive_hits_delta is not None
            and comparison["positive_hits_delta"] < args.min_positive_hits_delta
        ):
            failed = True

    return 1 if failed else 0


if __name__ == "__main__":
    raise SystemExit(main())
