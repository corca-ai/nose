#!/usr/bin/env python3
"""Collect and validate the checked, dev-only #843 behavior evidence."""

from __future__ import annotations

import argparse
import copy
import json
import os
import re
import subprocess
import sys
from pathlib import Path
from typing import Any

import generated_provenance_behavior as common


ROOT = common.ROOT
DEFAULT = ROOT / "bench/labels/declaration_type_contract_behavior_2026_07_14.dev.v1.json"
TAXONOMY = common.TAXONOMY
PARENT_COMMIT = "6d0575176020738978bd3d1fffe68509cd6b57b9"
PARENT_BINARY_SHA = "6d906e88270994a6ac2589977b2ce9b7616788c1bba67f9dc1b66791161de3dc"
CURRENT_COMMIT = "dfba43a5808b2d181f57106a4d769aa819b31343"
CURRENT_BINARY_SHA = "0a91b20a4160cac015e5d1cc5b5b865f33a7366710fda823a73565ed78800c09"
EXPECTED_EVIDENCE_DIGEST = "9cf10f01868d5f4c85a954a5591e5068ddfec170c6ee45b7763a5077dff869e1"
EXPECTED_ORIGIN_FAMILIES = {
    "commons-lang": {"103c02f1e1a97403", "f143803dc0ed74ed"},
    "graphhopper": {"22f74f2c5d4bce01"},
    "junit5": {"f2bd81e0ae2e0961"},
    "netty": {"db1f204915530e30"},
}
BEFORE_FLAGS = ["type-only", "interface-default-method"]
AFTER_FLAGS = ["type-only", "runtime-value", "interface-default-method"]


def declaration_lever() -> dict[str, Any]:
    common.require(
        common.sha256_file(TAXONOMY) == common.TAXONOMY_FILE_SHA,
        "#841 taxonomy file SHA-256 changed",
    )
    taxonomy = common.load(TAXONOMY)
    common.require(
        taxonomy.get("artifact_sha256") == common.TAXONOMY_SEMANTIC_SHA,
        "#841 taxonomy semantic binding changed",
    )
    return next(
        row
        for row in taxonomy["lever_decisions"]
        if row["lever_id"] == "declaration-only-type.v1"
    )


def query_with_threads(binary: Path, repo: str, threads: int | None) -> bytes:
    env = os.environ.copy()
    if threads is not None:
        env["RAYON_NUM_THREADS"] = str(threads)
    command = [
        str(binary),
        "query",
        f"bench/repos/{repo}",
        "all",
        "top=0",
        "--format",
        "json",
    ]
    return subprocess.run(
        command,
        cwd=ROOT,
        check=True,
        capture_output=True,
        env=env,
    ).stdout


def changed_leaves(before: Any, after: Any, path: str = "") -> list[dict[str, Any]]:
    if type(before) is not type(after):
        return [{"path": path, "before": before, "after": after}]
    if isinstance(before, dict):
        rows = []
        for key in sorted(set(before) | set(after)):
            if key == "surface":
                continue
            child_path = f"{path}.{key}"
            if key not in before or key not in after:
                rows.append(
                    {"path": child_path, "before": before.get(key), "after": after.get(key)}
                )
            else:
                rows.extend(changed_leaves(before[key], after[key], child_path))
        return rows
    if isinstance(before, list):
        if len(before) != len(after):
            return [{"path": path, "before": before, "after": after}]
        rows = []
        for index, (left, right) in enumerate(zip(before, after, strict=True)):
            rows.extend(changed_leaves(left, right, f"{path}[{index}]"))
        return rows
    return [] if before == after else [{"path": path, "before": before, "after": after}]


def non_surface_changes(
    before: list[dict[str, Any]], after: list[dict[str, Any]]
) -> list[dict[str, Any]]:
    common.require(
        [row["id"] for row in before] == [row["id"] for row in after],
        "cannot derive field drift when ordered ids differ",
    )
    return [
        {"family_id": left["id"], "changes": changes}
        for left, right in zip(before, after, strict=True)
        if (changes := changed_leaves(left, right))
    ]


def cohort_rows(
    keys: list[str], expanded: dict[str, list[dict[str, Any]]]
) -> list[dict[str, str]]:
    rows = []
    for key in keys:
        repo, family_id = common.parse_position_key(key)
        family = next((row for row in expanded[repo] if row["id"] == family_id), None)
        common.require(family is not None, f"{key}: family missing from expanded output")
        rows.append({"position_key": key, "surface": family["surface"]})
    return rows


def collect(parent_binary: Path, current_binary: Path, output: Path) -> None:
    common.require(common.sha256_file(parent_binary) == PARENT_BINARY_SHA, "wrong parent binary")
    common.require(common.sha256_file(current_binary) == CURRENT_BINARY_SHA, "wrong current binary")
    repos = common.dev_repositories()
    rows = []
    current_expanded: dict[str, list[dict[str, Any]]] = {}
    for index, repo_row in enumerate(repos, 1):
        repo = repo_row["repo"]
        print(f"[{index:02d}/66] {repo}", file=sys.stderr, flush=True)
        parent_all_raw, parent_all = common.query(parent_binary, repo, "expanded")
        current_all_raw, current_all = common.query(current_binary, repo, "expanded")
        parent_default_raw, parent_default = common.query(parent_binary, repo, "default")
        current_default_raw, current_default = common.query(current_binary, repo, "default")
        parent_semantic_raw, parent_semantic = common.query(parent_binary, repo, "semantic")
        current_semantic_raw, current_semantic = common.query(current_binary, repo, "semantic")
        repeat_raw = query_with_threads(current_binary, repo, None)
        single_raw = query_with_threads(current_binary, repo, 1)
        four_raw = query_with_threads(current_binary, repo, 4)
        current_expanded[repo] = current_all
        rows.append(
            {
                **repo_row,
                "expanded": {
                    **common.comparison(
                        parent_all_raw, parent_all, current_all_raw, current_all
                    ),
                    "non_surface_changes": non_surface_changes(parent_all, current_all),
                },
                "default_top30": common.default_comparison(
                    parent_default_raw, parent_default, current_default_raw, current_default
                ),
                "semantic": {
                    **common.comparison(
                        parent_semantic_raw,
                        parent_semantic,
                        current_semantic_raw,
                        current_semantic,
                    ),
                    "non_surface_changes": non_surface_changes(
                        parent_semantic, current_semantic
                    ),
                },
                "determinism": {
                    "initial_sha256": common.sha256_bytes(current_all_raw),
                    "repeat_sha256": common.sha256_bytes(repeat_raw),
                    "one_thread_sha256": common.sha256_bytes(single_raw),
                    "four_thread_sha256": common.sha256_bytes(four_raw),
                },
            }
        )
    lever = declaration_lever()
    artifact = {
        "schema": "nose.declaration_type_contract_behavior.v1",
        "issue": 843,
        "split": "dev",
        "heldout_policy": "closed; no held-out checkout or judgment was opened",
        "parent": {"commit": PARENT_COMMIT, "binary_sha256": PARENT_BINARY_SHA},
        "current": {"commit": CURRENT_COMMIT, "binary_sha256": CURRENT_BINARY_SHA},
        "corpus": {
            "manifest": "bench/goldens/corpus.json",
            "manifest_sha256": common.CORPUS_SHA,
            "repositories": repos,
        },
        "taxonomy": {
            "path": "bench/labels/default_head_taxonomy_2026_07_13.dev.v1.json",
            "file_sha256": common.TAXONOMY_FILE_SHA,
            "artifact_sha256": common.TAXONOMY_SEMANTIC_SHA,
        },
        "commands": {
            "expanded": "nose query bench/repos/<repo> all top=0 --format json",
            "default_top30": "nose query bench/repos/<repo> top=30 --format json",
            "semantic": "nose query bench/repos/<repo> all top=0 --mode semantic --format json",
        },
        "expanded_summary": common.summarize(rows, "expanded"),
        "semantic_summary": common.summarize(rows, "semantic"),
        "default_changed_repositories": [
            row["repo"]
            for row in rows
            if row["default_top30"]["parent_stdout_sha256"]
            != row["default_top30"]["current_stdout_sha256"]
        ],
        "cohorts": {
            "head_positives": cohort_rows(lever["positive_position_keys"], current_expanded),
            "deep_audit_positives": cohort_rows(lever["audit_packet_keys"], current_expanded),
            "hard_negatives": cohort_rows(lever["hard_negative_position_keys"], current_expanded),
        },
        "rows": rows,
    }
    artifact["evidence_digest"] = common.digest(artifact)
    output.write_text(json.dumps(artifact, indent=2, sort_keys=True) + "\n", encoding="utf-8")
    print(f"wrote {output} ({artifact['evidence_digest']})")


def validate_origin_changes(rows: list[dict[str, Any]], field: str) -> None:
    actual = {}
    for row in rows:
        family_rows = row[field]["non_surface_changes"]
        if family_rows:
            actual[row["repo"]] = {family["family_id"] for family in family_rows}
        for family in family_rows:
            for change in family["changes"]:
                if field == "expanded":
                    common.require(
                        re.fullmatch(
                            r"\.locations\[\d+\]\.origin\.evidence_flags",
                            change["path"],
                        )
                        is not None,
                        f"{row['repo']}:{family['family_id']}: unexpected field drift",
                    )
                    common.require(
                        change["before"] == BEFORE_FLAGS,
                        "unexpected prior origin flags",
                    )
                    common.require(
                        change["after"] == AFTER_FLAGS,
                        "unexpected current origin flags",
                    )
                else:
                    expected_change = {
                        "body_kind": ("declaration-only", "mixed"),
                        "evidence_flags[0]": ("declaration-only", "type-only"),
                        "evidence_flags[1]": ("type-only", "runtime-value"),
                    }
                    suffix = change["path"].rsplit(".origin.", 1)[-1]
                    common.require(suffix in expected_change, "unexpected semantic origin field")
                    common.require(
                        (change["before"], change["after"]) == expected_change[suffix],
                        "unexpected semantic origin transition",
                    )
    expected = (
        EXPECTED_ORIGIN_FAMILIES
        if field == "expanded"
        else {"netty": {"1f88ca5e0902da4a"}}
    )
    common.require(actual == expected, f"{field}: unexpected origin-truthfulness drift")


def validate(path: Path) -> None:
    artifact = common.load(path)
    bound = artifact.pop("evidence_digest", None)
    common.require(bound == EXPECTED_EVIDENCE_DIGEST, "behavior digest is not reviewed")
    common.require(common.digest(artifact) == bound, "behavior contents do not match digest")
    common.require(artifact["schema"] == "nose.declaration_type_contract_behavior.v1", "wrong schema")
    common.require(artifact["issue"] == 843 and artifact["split"] == "dev", "wrong scope")
    common.require("held-out" in artifact["heldout_policy"], "held-out policy missing")
    common.require(
        artifact["parent"] == {"commit": PARENT_COMMIT, "binary_sha256": PARENT_BINARY_SHA},
        "wrong parent role",
    )
    common.require(
        artifact["current"] == {"commit": CURRENT_COMMIT, "binary_sha256": CURRENT_BINARY_SHA},
        "wrong current role",
    )
    common.require(artifact["corpus"]["repositories"] == common.dev_repositories(), "wrong corpus")
    common.require(
        artifact["taxonomy"]
        == {
            "path": "bench/labels/default_head_taxonomy_2026_07_13.dev.v1.json",
            "file_sha256": common.TAXONOMY_FILE_SHA,
            "artifact_sha256": common.TAXONOMY_SEMANTIC_SHA,
        },
        "wrong taxonomy binding",
    )
    rows = artifact["rows"]
    expanded = artifact["expanded_summary"]
    semantic = artifact["semantic_summary"]
    common.require(expanded == common.summarize(rows, "expanded"), "expanded summary is not derived")
    common.require(semantic == common.summarize(rows, "semantic"), "semantic summary is not derived")
    common.require(expanded["families_before"] == expanded["families_after"] == 54754, "expanded family total changed")
    common.require(semantic["families_before"] == semantic["families_after"] == 9850, "semantic family total changed")
    common.require(expanded["family_id_order_equal"] and semantic["family_id_order_equal"], "ordered ids changed")
    common.require(
        expanded["changed_repositories"]
        == [
            "antlr4",
            "commons-lang",
            "drizzle-orm",
            "fastlane",
            "graphhopper",
            "guava",
            "jest",
            "junit5",
            "mockito",
            "netty",
            "prettier",
            "prometheus",
            "swr",
            "zod",
            "zustand",
        ],
        "expanded drift breadth changed",
    )
    common.require(
        expanded["surface_transitions"] == {"default->declaration": 91, "shallow->declaration": 44},
        "expanded surface transitions changed",
    )
    common.require(semantic["surface_transitions"] == {}, "semantic surfaces changed")
    common.require(semantic["changed_repositories"] == ["netty"], "semantic drift breadth changed")
    validate_origin_changes(rows, "expanded")
    validate_origin_changes(rows, "semantic")
    common.require(
        all(len(set(row["determinism"].values())) == 1 for row in rows),
        "repeat or thread-count output drift",
    )
    default_moves = {
        row["repo"]: {
            "removed": row["default_top30"]["removed"],
            "added": row["default_top30"]["added"],
        }
        for row in rows
        if row["default_top30"]["removed"] or row["default_top30"]["added"]
    }
    common.require(
        default_moves
        == {
            "antlr4": {
                "removed": ["0027a4580f69a91b", "ae4c8da2e1e24a84"],
                "added": ["60dedf519dc16a64", "f3367804bdf80ae9"],
            },
            "prettier": {
                "removed": ["28745f9b360ca305", "20080c8b31b01058"],
                "added": ["e568b2d77c3505df", "f77f104cbb0368cd"],
            },
            "zustand": {
                "removed": ["cdb6c513b5432417"],
                "added": ["c717cce89f1a5dbf"],
            },
        },
        "default top-30 replacements changed",
    )
    lever = declaration_lever()
    expected_keys = {
        "head_positives": lever["positive_position_keys"],
        "deep_audit_positives": lever["audit_packet_keys"],
        "hard_negatives": lever["hard_negative_position_keys"],
    }
    for cohort, keys in expected_keys.items():
        cohort_rows_value = artifact["cohorts"][cohort]
        common.require([row["position_key"] for row in cohort_rows_value] == keys, f"{cohort}: keys changed")
        expected_surface = "default" if cohort == "hard_negatives" else "declaration"
        common.require(all(row["surface"] == expected_surface for row in cohort_rows_value), f"{cohort}: wrong surface")
    print(f"declaration type-contract behavior OK: {path.relative_to(ROOT)}")


def self_test() -> None:
    artifact = common.load(DEFAULT)
    original = artifact["evidence_digest"]
    mutated = copy.deepcopy(artifact)
    mutated["rows"][0]["expanded"]["current_families"] += 1
    mutated.pop("evidence_digest")
    common.require(common.digest(mutated) != original, "mutation was not detected")
    validate(DEFAULT)
    print("declaration type-contract behavior self-test passed")


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--self-test", action="store_true")
    sub = parser.add_subparsers(dest="command")
    collect_parser = sub.add_parser("collect")
    collect_parser.add_argument("--parent-binary", type=Path, required=True)
    collect_parser.add_argument("--current-binary", type=Path, required=True)
    collect_parser.add_argument("--output", type=Path, default=DEFAULT)
    validate_parser = sub.add_parser("validate")
    validate_parser.add_argument("artifact", nargs="?", type=Path, default=DEFAULT)
    args = parser.parse_args()
    if args.self_test:
        self_test()
    elif args.command == "collect":
        collect(args.parent_binary.resolve(), args.current_binary.resolve(), args.output.resolve())
    elif args.command == "validate":
        validate(args.artifact.resolve())
    else:
        parser.error("choose collect or validate")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
