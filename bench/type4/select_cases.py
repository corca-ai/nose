#!/usr/bin/env python3
"""Select a coverage-preserving compact Type-4 manifest."""

from __future__ import annotations

import argparse
import json
import shutil
from collections import Counter
from pathlib import Path


def status_key(item: dict) -> str:
    return "positive" if item["expected_exact_detect"] else "negative"


def surface_family(surface: str) -> str:
    if surface in {"javascript", "typescript", "vue", "svelte", "html"}:
        return "js-family"
    return surface


def representation_pair(item: dict) -> str:
    return f"{item['left']['representation']}->{item['right']['representation']}"


def item_features(item: dict) -> set[str]:
    status = status_key(item)
    matrix = item["matrix"]
    left = item["left"]
    right = item["right"]
    negative_tag = matrix.get("negative_tag")
    relation = matrix["language_relation"]
    features = {
        f"status:{status}",
        f"split:{item['split']}:status:{status}",
        f"proposal:{item['proposal_id']}:status:{status}",
        f"computation:{matrix['computation']}:status:{status}",
        f"relation:{relation}:status:{status}",
        f"data_shape:{matrix['data_shape']}:status:{status}",
        f"representation:{representation_pair(item)}:status:{status}",
        f"left_surface:{left['surface']}:status:{status}",
        f"right_surface:{right['surface']}:status:{status}",
        f"left_family:{surface_family(left['surface'])}:status:{status}",
        f"right_family:{surface_family(right['surface'])}:status:{status}",
    }
    proposal = item["proposal_id"]
    features.add(f"proposal:{proposal}:representation:{representation_pair(item)}:status:{status}")
    features.add(f"proposal:{proposal}:relation:{relation}:status:{status}")
    for tag in item.get("transform_tags", []):
        features.add(f"transform:{tag}:status:{status}")
    for axis in matrix.get("semantic_axes", []):
        features.add(f"semantic_axis:{axis}:status:{status}")
        features.add(f"proposal:{proposal}:semantic_axis:{axis}:status:{status}")
        features.add(f"surface:{left['surface']}:semantic_axis:{axis}:status:{status}")
        features.add(f"surface:{right['surface']}:semantic_axis:{axis}:status:{status}")
    for capability, state in matrix.get("capabilities", {}).items():
        features.add(f"capability:{capability}:{state}:status:{status}")
        features.add(f"surface:{left['surface']}:capability:{capability}:{state}:status:{status}")
        features.add(f"surface:{right['surface']}:capability:{capability}:{state}:status:{status}")
    if negative_tag:
        features.add(f"negative_tag:{negative_tag}")
        features.add(f"negative_tag:{negative_tag}:proposal:{proposal}")
    if relation == "cross-surface":
        for side in (left, right):
            features.add(f"cross_surface:{side['surface']}:status:{status}")
            features.add(f"cross_family:{surface_family(side['surface'])}:status:{status}")
            features.add(f"proposal:{proposal}:cross_surface:{side['surface']}:status:{status}")
            features.add(
                f"proposal:{proposal}:cross_family:{surface_family(side['surface'])}:status:{status}"
            )
    return features


def selectable_items(manifest: dict, suite: str) -> list[dict]:
    items = manifest["items"]
    if suite == "core":
        return items
    if suite == "positive-core":
        return [item for item in items if item["expected_exact_detect"]]
    if suite == "negative-core":
        return [item for item in items if not item["expected_exact_detect"]]
    raise ValueError(f"unknown suite: {suite}")


def greedy_cover(items: list[dict]) -> tuple[list[dict], set[str]]:
    feature_by_case = {item["case_id"]: item_features(item) for item in items}
    uncovered = set().union(*feature_by_case.values()) if feature_by_case else set()
    selected: list[dict] = []
    remaining = items.copy()
    while uncovered:
        best = max(
            remaining,
            key=lambda item: (
                len(feature_by_case[item["case_id"]] & uncovered),
                -len(feature_by_case[item["case_id"]]),
                item["case_id"],
            ),
        )
        gain = feature_by_case[best["case_id"]] & uncovered
        if not gain:
            break
        selected.append(best)
        uncovered -= gain
        remaining.remove(best)
    return selected, uncovered


def copy_selected_sources(selected: list[dict], manifest_dir: Path, out_dir: Path) -> None:
    for item in selected:
        for side in ("left", "right"):
            rel = Path(item[side]["path"])
            src = manifest_dir / rel
            dst = out_dir / rel
            dst.parent.mkdir(parents=True, exist_ok=True)
            shutil.copy2(src, dst)


def write_manifest(
    original: dict,
    selected: list[dict],
    manifest_path: Path,
    out_dir: Path,
    suite: str,
) -> None:
    manifest = {
        "schema_version": original["schema_version"],
        "source": {
            **original["source"],
            "suite": suite,
            "selected_from": str(manifest_path),
        },
        "items": selected,
    }
    (out_dir / "manifest.json").write_text(json.dumps(manifest, indent=2, sort_keys=True) + "\n")


def print_summary(full_items: list[dict], selected: list[dict], suite: str) -> None:
    def counts(items: list[dict]) -> Counter[str]:
        c = Counter()
        for item in items:
            c[status_key(item)] += 1
            c[f"split:{item['split']}"] += 1
            tag = item["matrix"].get("negative_tag")
            if tag:
                c[f"negative_tag:{tag}"] += 1
            for axis in item["matrix"].get("semantic_axes", []):
                c[f"axis:{axis}"] += 1
            for capability, state in item["matrix"].get("capabilities", {}).items():
                c[f"capability:{capability}:{state}"] += 1
        return c

    full_counts = counts(full_items)
    selected_counts = counts(selected)
    print(f"suite: {suite}")
    print(f"selected items: {len(selected)}/{len(full_items)}")
    for key in sorted(selected_counts):
        print(f"  {key}: {selected_counts[key]}/{full_counts[key]}")


def main() -> None:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("manifest", type=Path)
    parser.add_argument("--suite", choices=["core", "positive-core", "negative-core"], default="core")
    parser.add_argument("--out-dir", required=True, type=Path)
    args = parser.parse_args()

    manifest_path = args.manifest.resolve()
    manifest_dir = manifest_path.parent
    out_dir = args.out_dir.resolve()
    if out_dir == manifest_dir:
        raise ValueError("--out-dir must differ from the input manifest directory")
    if out_dir.exists():
        shutil.rmtree(out_dir)
    out_dir.mkdir(parents=True)

    manifest = json.loads(manifest_path.read_text())
    candidates = selectable_items(manifest, args.suite)
    selected, uncovered = greedy_cover(candidates)
    if uncovered:
        raise RuntimeError(f"selector failed to cover {len(uncovered)} features")
    copy_selected_sources(selected, manifest_dir, out_dir)
    write_manifest(manifest, selected, manifest_path, out_dir, args.suite)
    print_summary(candidates, selected, args.suite)


if __name__ == "__main__":
    main()
