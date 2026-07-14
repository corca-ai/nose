#!/usr/bin/env python3
"""Freeze and validate the dev-only judgment frontier for issue #845.

The selector is deliberately non-adaptive: it includes every unresolved family that
appears in the top ten of any pre-registered residual-ranking formula.  This makes all
46 formulas and every repository CV fold fully judged after one panel pass.  Selection
contains no judgment fields and keeps held-out evidence closed.
"""

from __future__ import annotations

import argparse
import copy
import hashlib
import json
from pathlib import Path
from typing import Any

import residual_ranking as ranking


ROOT = Path(__file__).resolve().parents[2]
CALIBRATION = (
    ROOT / "bench/labels/residual_ranking_calibration_2026_07_14.dev.v1.json"
)
RUBRIC = ROOT / "bench/labels/RUBRIC.md"
DEFAULT_SELECTION = (
    ROOT / "bench/labels/residual_ranking_topup_selection_2026_07_14.dev.v1.json"
)
EXPECTED_COLLECTION_SHA256 = "e1f297e5ae1321e989aaf3e591d810e1a303c9621909d71ec79d83d36df7f769"
EXPECTED_COLLECTION_PROJECTION_SHA256 = "5ee7ea38b02d3c3e593bc900a6d3bc5aa8cf4c9a43fabd0f07737d44ede18205"
EXPECTED_KEY_DIGEST = "3eea1b2cc7d59808c618f461fd638c20006aec70c787b4a536b09d524adb5bab"
EXPECTED_RAW_FAMILY_DIGEST = "d5d611f63be2f6d11cecc761afba95da8e40919b2f3b782d30a6390dc816f787"
EXPECTED_SOURCE_DIGEST = "60ac805b44cf51102afd2998f89654bfdfb0b0e1804feb4a3ddfda3ec1e43940"
EXPECTED_SELECTED = 219
FORBIDDEN_JUDGMENT_KEYS = {
    "arbiter",
    "confidence",
    "judgment",
    "labeler",
    "note",
    "rationale",
    "reason",
    "votes",
    "worthy",
}

CONTRACT = {
    "formula_count": 46,
    "metric_eligibility": ["precision_at_10"],
    "sampling": "none",
    "second_round_selection": "forbidden",
    "selection": "union of truth-null top-10 families over every frozen formula",
    "top_k": 10,
    "unresolved_statuses": ["conflicting-best-overlap", "unmatched"],
}


def read_json(path: Path) -> dict[str, Any]:
    value = json.loads(path.read_text(encoding="utf-8"))
    if not isinstance(value, dict):
        raise ValueError(f"{path}: expected an object")
    return value


def canonical_bytes(value: object) -> bytes:
    return json.dumps(value, sort_keys=True, separators=(",", ":")).encode()


def canonical_sha256(value: object) -> str:
    return hashlib.sha256(canonical_bytes(value)).hexdigest()


def sha256_file(path: Path) -> str:
    return hashlib.sha256(path.read_bytes()).hexdigest()


def path_record(path: Path) -> dict[str, str]:
    return {"path": path.relative_to(ROOT).as_posix(), "sha256": sha256_file(path)}


def require_equal(actual: object, expected: object, label: str) -> None:
    if actual != expected:
        raise ValueError(f"{label}: mismatch")


def require_exact_keys(value: object, expected: set[str], label: str) -> dict[str, Any]:
    if not isinstance(value, dict) or set(value) != expected:
        raise ValueError(f"{label}: expected exact keys {sorted(expected)}")
    return value


def family_key(family: dict[str, Any], rank: int) -> str:
    anchor = family["locations"][0]
    return f"{family['id']}@{anchor['file']}:{anchor['start']}#rank-{rank}"


def unresolved_membership(
    dataset: dict[str, Any],
) -> dict[str, list[dict[str, object]]]:
    membership: dict[str, list[dict[str, object]]] = {}
    for proposal in ranking.PROPOSALS:
        for repo, row in sorted(dataset["repositories"].items()):
            for rank, family in enumerate(
                ranking.order_families(row["families"], proposal)[: CONTRACT["top_k"]],
                start=1,
            ):
                if family["truth"] is not None:
                    continue
                candidate_key = f"{repo}:{family['key']}"
                membership.setdefault(candidate_key, []).append(
                    {"proposal": proposal.id, "rank": rank}
                )
    return {key: membership[key] for key in sorted(membership)}


def raw_family_index(collection: dict[str, Any]) -> dict[str, dict[str, Any]]:
    index: dict[str, dict[str, Any]] = {}
    for repo, row in collection["repositories"].items():
        for rank, family in enumerate(row["families"], start=1):
            key = f"{repo}:{family_key(family, rank)}"
            if key in index:
                raise ValueError(f"duplicate raw family key: {key}")
            index[key] = family
    return index


def assert_no_judgments(value: object, label: str = "selection") -> None:
    if isinstance(value, dict):
        forbidden = set(value) & FORBIDDEN_JUDGMENT_KEYS
        if forbidden:
            raise ValueError(f"{label}: judgment fields forbidden: {sorted(forbidden)}")
        for key, nested in value.items():
            assert_no_judgments(nested, f"{label}.{key}")
    elif isinstance(value, list):
        for index, nested in enumerate(value):
            assert_no_judgments(nested, f"{label}[{index}]")


def candidate_core(
    *,
    order: int,
    key: str,
    memberships: list[dict[str, object]],
    compact: dict[str, Any],
    repository: dict[str, Any],
    raw: dict[str, Any],
) -> dict[str, Any]:
    repo, frozen_key = key.split(":", 1)
    return {
        "candidate_key": key,
        "selection_order": order,
        "repo": repo,
        "split": "dev",
        "language": repository["language"],
        "family_key": frozen_key,
        "current_rank": compact["current_rank"],
        "truth_status": compact["truth_status"],
        "proposal_membership": memberships,
        "raw_family_sha256": canonical_sha256(raw),
        "raw_family": raw,
    }


def build_selection(collection: dict[str, Any], calibration: dict[str, Any]) -> dict[str, Any]:
    ranking.validate_payload(calibration)
    if (
        collection.get("schema") != "nose.residual_ranking_collection.v1"
        or collection.get("split") != "dev"
        or collection.get("heldout_policy") != ranking.HELDOUT_POLICY
    ):
        raise ValueError("collection is not the frozen dev-only query universe")
    require_equal(collection["nose"]["sha256"], ranking.EXPECTED_BINARY_SHA256, "binary")
    require_equal(collection["nose"]["version"], ranking.EXPECTED_BINARY_VERSION, "version")
    dataset = calibration["dataset"]
    memberships = unresolved_membership(dataset)
    require_equal(len(memberships), EXPECTED_SELECTED, "selected candidate count")
    raw_by_key = raw_family_index(collection)
    source_paths: set[str] = set()
    candidates = []
    for order, (key, proposal_membership) in enumerate(memberships.items(), start=1):
        repo, frozen_key = key.split(":", 1)
        compact_by_key = {
            family["key"]: family for family in dataset["repositories"][repo]["families"]
        }
        compact = compact_by_key[frozen_key]
        raw = raw_by_key[key]
        for location in raw["locations"]:
            source_paths.add(location["file"])
        core = candidate_core(
            order=order,
            key=key,
            memberships=proposal_membership,
            compact=compact,
            repository=dataset["repositories"][repo],
            raw=raw,
        )
        candidates.append({**core, "candidate_sha256": canonical_sha256(core)})
    source_files = [path_record(ROOT / path) for path in sorted(source_paths)]
    keys = [candidate["candidate_key"] for candidate in candidates]
    raw_records = [
        [candidate["candidate_key"], candidate["raw_family_sha256"]]
        for candidate in candidates
    ]
    collection_projection = [
        [repo, row["commit"], row["stdout_sha256"]]
        for repo, row in sorted(collection["repositories"].items())
    ]
    result = {
        "schema": "nose.residual_ranking_topup_selection.v1",
        "issue": 845,
        "split": "dev",
        "judgment_status": "frozen-unjudged",
        "heldout_policy": ranking.HELDOUT_POLICY,
        "provenance": {
            "calibration": path_record(CALIBRATION),
            "collection_sha256": canonical_sha256(collection),
            "collection_projection_sha256": canonical_sha256(collection_projection),
            "binary_sha256": collection["nose"]["sha256"],
            "binary_version": collection["nose"]["version"],
            "corpus": path_record(ranking.CORPUS),
            "dev_labels": [path_record(path) for path in ranking.DEV_LABELS],
            "rubric": path_record(RUBRIC),
        },
        "selection_contract": CONTRACT,
        "proposal_ids": [proposal.id for proposal in ranking.PROPOSALS],
        "selection": {
            "selected_count": len(candidates),
            "candidate_key_sha256": canonical_sha256(keys),
            "raw_family_sha256": canonical_sha256(raw_records),
            "source_files_sha256": canonical_sha256(source_files),
        },
        "source_files": source_files,
        "candidates": candidates,
    }
    assert_no_judgments(result)
    return result


def freeze(args: argparse.Namespace) -> None:
    result = build_selection(read_json(args.collection), read_json(CALIBRATION))
    args.output.write_text(
        json.dumps(result, sort_keys=True, separators=(",", ":")) + "\n",
        encoding="utf-8",
    )
    print(f"selected={len(result['candidates'])}")
    print(f"candidate_key_sha256={result['selection']['candidate_key_sha256']}")


def validate_payload(payload: dict[str, Any]) -> None:
    require_exact_keys(
        payload,
        {
            "schema",
            "issue",
            "split",
            "judgment_status",
            "heldout_policy",
            "provenance",
            "selection_contract",
            "proposal_ids",
            "selection",
            "source_files",
            "candidates",
        },
        "selection artifact",
    )
    require_equal(payload["schema"], "nose.residual_ranking_topup_selection.v1", "schema")
    require_equal(payload["issue"], 845, "issue")
    require_equal(payload["split"], "dev", "split")
    require_equal(payload["judgment_status"], "frozen-unjudged", "judgment status")
    require_equal(payload["heldout_policy"], ranking.HELDOUT_POLICY, "held-out policy")
    require_equal(payload["selection_contract"], CONTRACT, "selection contract")
    require_equal(
        payload["proposal_ids"], [proposal.id for proposal in ranking.PROPOSALS], "proposals"
    )
    provenance = require_exact_keys(
        payload["provenance"],
        {
            "calibration",
            "collection_sha256",
            "collection_projection_sha256",
            "binary_sha256",
            "binary_version",
            "corpus",
            "dev_labels",
            "rubric",
        },
        "provenance",
    )
    require_equal(provenance["calibration"], path_record(CALIBRATION), "calibration")
    require_equal(provenance["binary_sha256"], ranking.EXPECTED_BINARY_SHA256, "binary")
    require_equal(provenance["binary_version"], ranking.EXPECTED_BINARY_VERSION, "version")
    require_equal(provenance["corpus"], path_record(ranking.CORPUS), "corpus")
    require_equal(
        provenance["dev_labels"], [path_record(path) for path in ranking.DEV_LABELS], "labels"
    )
    require_equal(provenance["rubric"], path_record(RUBRIC), "rubric")
    if EXPECTED_COLLECTION_SHA256:
        require_equal(
            provenance["collection_sha256"], EXPECTED_COLLECTION_SHA256, "collection digest"
        )
        require_equal(
            provenance["collection_projection_sha256"],
            EXPECTED_COLLECTION_PROJECTION_SHA256,
            "collection projection digest",
        )
    calibration = read_json(CALIBRATION)
    ranking.validate_payload(calibration)
    dataset = calibration["dataset"]
    expected_membership = unresolved_membership(dataset)
    require_equal(len(expected_membership), EXPECTED_SELECTED, "derived selection count")
    require_equal(
        [candidate["candidate_key"] for candidate in payload["candidates"]],
        list(expected_membership),
        "candidate keys/order",
    )
    compact = {
        f"{repo}:{family['key']}": (repo, family)
        for repo, row in dataset["repositories"].items()
        for family in row["families"]
    }
    labels = ranking.load_dev_labels()
    for order, candidate in enumerate(payload["candidates"], start=1):
        require_exact_keys(
            candidate,
            {
                "candidate_key",
                "selection_order",
                "repo",
                "split",
                "language",
                "family_key",
                "current_rank",
                "truth_status",
                "proposal_membership",
                "raw_family_sha256",
                "raw_family",
                "candidate_sha256",
            },
            f"candidate {order}",
        )
        key = candidate["candidate_key"]
        repo, family = compact[key]
        require_equal(candidate["selection_order"], order, f"{key}: selection order")
        require_equal(candidate["repo"], repo, f"{key}: repo")
        require_equal(candidate["split"], "dev", f"{key}: split")
        require_equal(candidate["language"], dataset["repositories"][repo]["language"], f"{key}: language")
        require_equal(candidate["family_key"], family["key"], f"{key}: family key")
        require_equal(candidate["current_rank"], family["current_rank"], f"{key}: rank")
        require_equal(candidate["truth_status"], family["truth_status"], f"{key}: truth status")
        require_equal(candidate["proposal_membership"], expected_membership[key], f"{key}: membership")
        for location in candidate["raw_family"]["locations"]:
            source = Path(location["file"])
            expected_prefix = Path("bench/repos") / repo
            if source.is_absolute() or ".." in source.parts or not source.is_relative_to(expected_prefix):
                raise ValueError(f"{key}: source path escapes its pinned dev checkout")
        require_equal(candidate["raw_family_sha256"], canonical_sha256(candidate["raw_family"]), f"{key}: raw hash")
        rebuilt = ranking.compact_family(
            candidate["raw_family"], labels[repo], family["current_rank"]
        )
        require_equal(rebuilt, family, f"{key}: compact projection")
        core = {name: value for name, value in candidate.items() if name != "candidate_sha256"}
        require_equal(candidate["candidate_sha256"], canonical_sha256(core), f"{key}: candidate hash")
    source_paths = sorted(
        {
            location["file"]
            for candidate in payload["candidates"]
            for location in candidate["raw_family"]["locations"]
        }
    )
    require_equal(
        [record["path"] for record in payload["source_files"]], source_paths, "source paths"
    )
    for index, record in enumerate(payload["source_files"]):
        require_exact_keys(record, {"path", "sha256"}, f"source file {index}")
        if len(record["sha256"]) != 64:
            raise ValueError(f"source file {index}: invalid digest")
    keys = [candidate["candidate_key"] for candidate in payload["candidates"]]
    raw_records = [
        [candidate["candidate_key"], candidate["raw_family_sha256"]]
        for candidate in payload["candidates"]
    ]
    expected_selection = {
        "selected_count": EXPECTED_SELECTED,
        "candidate_key_sha256": canonical_sha256(keys),
        "raw_family_sha256": canonical_sha256(raw_records),
        "source_files_sha256": canonical_sha256(payload["source_files"]),
    }
    require_equal(payload["selection"], expected_selection, "selection summary")
    if EXPECTED_KEY_DIGEST:
        require_equal(expected_selection["candidate_key_sha256"], EXPECTED_KEY_DIGEST, "key digest")
        require_equal(expected_selection["raw_family_sha256"], EXPECTED_RAW_FAMILY_DIGEST, "raw digest")
        require_equal(expected_selection["source_files_sha256"], EXPECTED_SOURCE_DIGEST, "source digest")
    assert_no_judgments(payload)


def validate(args: argparse.Namespace) -> None:
    validate_payload(read_json(args.selection))
    print(f"validated {args.selection}")


def self_test(args: argparse.Namespace) -> None:
    payload = read_json(args.selection)
    validate_payload(payload)
    mutations = []
    changed = copy.deepcopy(payload)
    changed["heldout_result"] = {"precision_at_10": 100}
    mutations.append(changed)
    changed = copy.deepcopy(payload)
    changed["candidates"][0]["worthy"] = True
    mutations.append(changed)
    changed = copy.deepcopy(payload)
    changed["candidates"] = changed["candidates"][1:]
    mutations.append(changed)
    changed = copy.deepcopy(payload)
    changed["candidates"][0], changed["candidates"][1] = (
        changed["candidates"][1],
        changed["candidates"][0],
    )
    mutations.append(changed)
    changed = copy.deepcopy(payload)
    changed["provenance"]["dev_labels"] = []
    mutations.append(changed)
    changed = copy.deepcopy(payload)
    changed["selection_contract"]["second_round_selection"] = "allowed"
    mutations.append(changed)
    for mutation in mutations:
        try:
            validate_payload(mutation)
        except ValueError:
            continue
        raise AssertionError("invalid selection mutation was accepted")
    print("residual-ranking top-up self-test passed")


def parser() -> argparse.ArgumentParser:
    root = argparse.ArgumentParser()
    commands = root.add_subparsers(dest="command", required=True)
    freeze_parser = commands.add_parser("freeze")
    freeze_parser.add_argument("--collection", type=Path, required=True)
    freeze_parser.add_argument("--output", type=Path, default=DEFAULT_SELECTION)
    freeze_parser.set_defaults(run=freeze)
    validate_parser = commands.add_parser("validate")
    validate_parser.add_argument("selection", type=Path, nargs="?", default=DEFAULT_SELECTION)
    validate_parser.set_defaults(run=validate)
    self_test_parser = commands.add_parser("self-test")
    self_test_parser.add_argument("--selection", type=Path, default=DEFAULT_SELECTION)
    self_test_parser.set_defaults(run=self_test)
    return root


def main() -> None:
    args = parser().parse_args()
    args.run(args)


if __name__ == "__main__":
    main()
