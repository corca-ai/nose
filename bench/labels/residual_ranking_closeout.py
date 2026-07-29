#!/usr/bin/env python3
"""Close issue #845 with the fully judged, dev-only residual-ranking result."""

from __future__ import annotations

import argparse
import copy
import json
import math
import subprocess
from pathlib import Path
from typing import Any

import residual_ranking as ranking
import residual_ranking_panel as panel


ROOT = Path(__file__).resolve().parents[2]
CALIBRATION = ranking.DEFAULT_ARTIFACT
COMPONENT = panel.COMPONENT
DEFAULT_ARTIFACT = (
    ROOT / "bench/labels/residual_ranking_closeout_2026_07_14.dev.v1.json"
)
EXPECTED_OVERLAY_DATASET_SHA256 = (
    "ef5bdc4b970201d721deb0ef90681b52891b7486cb15158ddbaae2f106ea5fbe"
)
EXPECTED_TOOL_COMMIT = "45018a24d9f6587678fd6c353456019775101477"
EXPECTED_PANEL_SHA256 = "72e805f23adf7fc7807930e6bafbebdb7d2f67b3a6a683012fab43303ae5e7ff"
EXPECTED_EVALUATOR_SHA256 = "fabf1414d84d936f540e5e19045f8ff24eb8cf0c4b11bcffa59d2b36edb92559"
EXPECTED_BASELINE = {
    "best_case_slot_precision_pct": 58.8146,
    "coverage_pct": 100.0,
    "hits": 387,
    "matched": 658,
    "precision_pct": 58.8146,
    "reported": 658,
    "slot_yield_pct": 58.8146,
}
EXPECTED_BEST_GUARDED = {
    "proposal": "grid-s-1.00-same0.65-conn1.00",
    "overall": {
        "best_case_slot_precision_pct": 68.2371,
        "coverage_pct": 100.0,
        "hits": 449,
        "matched": 658,
        "precision_pct": 68.2371,
        "reported": 658,
        "slot_yield_pct": 68.2371,
    },
}
EXPECTED_OOF_OVERALL = {
    "best_case_slot_precision_pct": 63.2219,
    "coverage_pct": 100.0,
    "hits": 416,
    "matched": 658,
    "precision_pct": 63.2219,
    "reported": 658,
    "slot_yield_pct": 63.2219,
}


def read_json(path: Path) -> dict[str, Any]:
    value = json.loads(path.read_text(encoding="utf-8"))
    if not isinstance(value, dict):
        raise ValueError(f"{path}: expected an object")
    return value


def path_record(path: Path) -> dict[str, str]:
    return {"path": path.relative_to(ROOT).as_posix(), "sha256": ranking.sha256_file(path)}


def frozen_tool_record(path: Path, expected_sha256: str) -> dict[str, str]:
    relative = path.relative_to(ROOT).as_posix()
    frozen = subprocess.run(
        ["git", "show", f"{EXPECTED_TOOL_COMMIT}:{relative}"],
        cwd=ROOT,
        check=True,
        capture_output=True,
    ).stdout
    require_equal(
        ranking.sha256_bytes(frozen), expected_sha256, f"{relative}: frozen tool"
    )
    return {"path": relative, "sha256": expected_sha256}


def require_equal(actual: object, expected: object, label: str) -> None:
    if actual != expected:
        raise ValueError(f"{label}: mismatch")


def require_exact_keys(value: object, expected: set[str], label: str) -> dict[str, Any]:
    if not isinstance(value, dict) or set(value) != expected:
        raise ValueError(f"{label}: expected exact keys {sorted(expected)}")
    return value


def apply_exact_overlay(
    dataset: dict[str, Any], component: dict[str, Any]
) -> tuple[dict[str, Any], dict[str, Any]]:
    labels = component["labels"]
    by_key = {row["candidate_key"]: row for row in labels}
    if len(by_key) != len(labels):
        raise ValueError("duplicate exact top-up candidate key")
    result = copy.deepcopy(dataset)
    used: set[str] = set()
    prior_statuses: dict[str, int] = {}
    for repo, repository in result["repositories"].items():
        for family in repository["families"]:
            key = f"{repo}:{family['key']}"
            label = by_key.get(key)
            if label is None:
                continue
            if family["truth"] is not None:
                raise ValueError(f"{key}: exact top-up attempted to replace known truth")
            status = family["truth_status"]
            if status not in {"conflicting-best-overlap", "unmatched"}:
                raise ValueError(f"{key}: unexpected unresolved status {status}")
            prior_statuses[status] = prior_statuses.get(status, 0) + 1
            family["truth"] = label["worthy"]
            family["truth_status"] = "exact-panel-topup"
            used.add(key)
    require_equal(used, set(by_key), "exact overlay coverage")
    return result, {
        "selected": len(labels),
        "applied": len(used),
        "candidate_key_sha256": ranking.canonical_sha256(sorted(used)),
        "prior_truth_statuses": dict(sorted(prior_statuses.items())),
        "truth_status": "exact-panel-topup",
        "mapping": "exact-candidate-key-only",
        "fuzzy_overlap_propagation": False,
    }


def assert_complete_formula_heads(
    dataset: dict[str, Any], evaluation: dict[str, Any]
) -> None:
    for proposal, result in evaluation["proposal_results"].items():
        overall = result["overall"]
        if overall["matched"] != overall["reported"]:
            raise ValueError(f"{proposal}: incomplete top-10 truth coverage")
        for language, record in result["languages"].items():
            if record["matched"] != record["reported"]:
                raise ValueError(f"{proposal}/{language}: incomplete truth coverage")
    for proposal in ranking.PROPOSALS:
        measured = ranking.metrics_for(dataset, proposal)
        for repo, record in measured["repositories"].items():
            if record["counts"]["matched"] != record["counts"]["reported"]:
                raise ValueError(f"{proposal.id}/{repo}: incomplete top-10 truth coverage")


def decision_record(evaluation: dict[str, Any]) -> dict[str, Any]:
    best = evaluation["best_coverage_guarded"]
    if best is None:
        raise ValueError("expected at least one coverage-guarded proposal")
    overall = best["result"]["overall"]
    required_hits = math.ceil(
        ranking.CONTRACT["precision_at_10_min_pct"] * overall["reported"] / 100
    )
    language_failures = []
    for language, result in sorted(best["result"]["languages"].items()):
        if result["reported"] < ranking.CONTRACT["language_floor_min_positions"]:
            continue
        required = math.ceil(
            ranking.CONTRACT["language_precision_floor_pct"]
            * result["matched"]
            / 100
        )
        if result["hits"] < required:
            language_failures.append(
                {
                    "language": language,
                    "hits": result["hits"],
                    "matched": result["matched"],
                    "precision_pct": result["precision_pct"],
                    "required_hits": required,
                    "shortfall_hits": required - result["hits"],
                }
            )
    return {
        "decision": "no-go",
        "frozen_proposal": None,
        "retained_signals": [],
        "tested_formula_count": evaluation["proposal_formula_count"],
        "best_coverage_guarded_proposal": best["proposal"],
        "best_coverage_guarded_precision": overall,
        "target_hits": required_hits,
        "target_shortfall_hits": required_hits - overall["hits"],
        "language_floor_failures": language_failures,
        "cross_validation_oof": evaluation["cross_validation"]["oof"],
        "reason": (
            "No pre-registered formula reached 70% dev P@10 under the coverage, "
            "language-floor, and regression gates; retain the current product order."
        ),
        "next_step": (
            "Issue #846 may measure and close the unchanged product, but must not open "
            "held-out evidence as a new tuning round."
        ),
    }


def build_artifact(
    context: panel.ValidationContext | None = None,
) -> dict[str, Any]:
    if context is None:
        context = panel.build_validation_context()
    calibration = context.topup.calibration
    component = context.component
    if component is None:
        raise ValueError("residual closeout requires the complete panel artifact chain")
    dataset, overlay = apply_exact_overlay(calibration["dataset"], component)
    dataset_sha256 = ranking.canonical_sha256(dataset)
    require_equal(dataset_sha256, EXPECTED_OVERLAY_DATASET_SHA256, "overlay dataset digest")
    evaluation = ranking.evaluate_dataset(dataset)
    assert_complete_formula_heads(dataset, evaluation)
    require_equal(evaluation["decision"], "no-go", "evaluation decision")
    require_equal(evaluation["successful_proposals"], [], "successful proposals")
    require_equal(evaluation["best_eligible"], None, "eligible proposal")
    require_equal(
        evaluation["optimistically_possible_proposals"], [], "optimistic proposals"
    )
    require_equal(evaluation["baseline"]["overall"], EXPECTED_BASELINE, "baseline")
    best = evaluation["best_coverage_guarded"]
    require_equal(best["proposal"], EXPECTED_BEST_GUARDED["proposal"], "best proposal")
    require_equal(
        best["result"]["overall"], EXPECTED_BEST_GUARDED["overall"], "best result"
    )
    require_equal(
        evaluation["cross_validation"]["oof"]["overall"],
        EXPECTED_OOF_OVERALL,
        "out-of-fold result",
    )
    return {
        "schema": "nose.residual_ranking_closeout.v1",
        "issue": 845,
        "split": "dev",
        "decision": "no-go",
        "heldout_policy": ranking.HELDOUT_POLICY,
        "contract": ranking.CONTRACT,
        "provenance": {
            "calibration": path_record(CALIBRATION),
            "label_component": path_record(COMPONENT),
            "panel_tool": frozen_tool_record(
                Path(panel.__file__), EXPECTED_PANEL_SHA256
            ),
            "evaluator": frozen_tool_record(
                Path(__file__), EXPECTED_EVALUATOR_SHA256
            ),
            "binary_sha256": ranking.EXPECTED_BINARY_SHA256,
            "binary_version": ranking.EXPECTED_BINARY_VERSION,
        },
        "input_dataset_sha256": calibration["dataset_sha256"],
        "dataset_sha256": dataset_sha256,
        "overlay": overlay,
        "evaluation": evaluation,
        "decision_record": decision_record(evaluation),
        "preservation": {
            "product_code_changed": False,
            "ranking_changed": False,
            "surface_changed": False,
            "heldout_opened": False,
            "full_universe_worthy_recall": "2716/2849",
            "worthy_recall_delta": 0,
        },
    }


def freeze(args: argparse.Namespace) -> None:
    artifact = build_artifact()
    args.output.write_text(
        json.dumps(artifact, sort_keys=True, separators=(",", ":")) + "\n",
        encoding="utf-8",
    )
    print(json.dumps(artifact["decision_record"], indent=2, sort_keys=True))


def validate_payload(
    artifact: dict[str, Any], *, expected: dict[str, Any] | None = None
) -> None:
    require_exact_keys(
        artifact,
        {
            "schema",
            "issue",
            "split",
            "decision",
            "heldout_policy",
            "contract",
            "provenance",
            "input_dataset_sha256",
            "dataset_sha256",
            "overlay",
            "evaluation",
            "decision_record",
            "preservation",
        },
        "closeout artifact",
    )
    if expected is None:
        expected = build_artifact()
    require_equal(artifact, expected, "closeout artifact")


def validate(args: argparse.Namespace) -> None:
    validate_payload(read_json(args.artifact))
    print(f"validated {args.artifact}")


def self_test(args: argparse.Namespace) -> None:
    artifact = read_json(args.artifact)
    expected = build_artifact()
    validate_payload(artifact, expected=expected)
    mutations = []
    changed = copy.deepcopy(artifact)
    changed["decision"] = "go"
    mutations.append(changed)
    changed = copy.deepcopy(artifact)
    changed["overlay"]["applied"] -= 1
    mutations.append(changed)
    changed = copy.deepcopy(artifact)
    changed["heldout_policy"]["labels_opened"] = True
    mutations.append(changed)
    changed = copy.deepcopy(artifact)
    changed["decision_record"]["frozen_proposal"] = "current"
    mutations.append(changed)
    for mutation in mutations:
        try:
            validate_payload(mutation, expected=expected)
        except ValueError:
            continue
        raise AssertionError("invalid residual-ranking closeout mutation was accepted")
    print("residual-ranking closeout self-test passed")


def inspect(args: argparse.Namespace) -> None:
    artifact = read_json(args.artifact)
    print(json.dumps(artifact["decision_record"], indent=2, sort_keys=True))


def parser() -> argparse.ArgumentParser:
    root = argparse.ArgumentParser()
    commands = root.add_subparsers(dest="command", required=True)
    freeze_parser = commands.add_parser("freeze")
    freeze_parser.add_argument("--output", type=Path, default=DEFAULT_ARTIFACT)
    freeze_parser.set_defaults(run=freeze)
    validate_parser = commands.add_parser("validate")
    validate_parser.add_argument("artifact", type=Path, nargs="?", default=DEFAULT_ARTIFACT)
    validate_parser.set_defaults(run=validate)
    self_test_parser = commands.add_parser("self-test")
    self_test_parser.add_argument("--artifact", type=Path, default=DEFAULT_ARTIFACT)
    self_test_parser.set_defaults(run=self_test)
    inspect_parser = commands.add_parser("inspect")
    inspect_parser.add_argument("--artifact", type=Path, default=DEFAULT_ARTIFACT)
    inspect_parser.set_defaults(run=inspect)
    return root


def main() -> None:
    args = parser().parse_args()
    args.run(args)


if __name__ == "__main__":
    main()
