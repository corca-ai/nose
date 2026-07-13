#!/usr/bin/env python3
"""Validate #844's checked proof/actionability no-go against the #841 dev taxonomy."""

from __future__ import annotations

import argparse
import copy
import hashlib
import json
from collections import Counter
from pathlib import Path
from typing import Any


ROOT = Path(__file__).resolve().parents[2]
DEFAULT = ROOT / "bench/labels/proof_actionability_no_go_2026_07_14.dev.v1.json"
CORE = ROOT / "bench/labels/default_head_taxonomy_2026_07_13.dev.core.v1.json"
OVERLAY = ROOT / "bench/labels/default_head_taxonomy_2026_07_13.dev.v1.json"
EXPECTED_ARTIFACT_SHA = "8645afd44f67bbcef19e295abec4b443e001deffaac7eaaa445e62c47560275e"
EXPECTED_CORE_BYTES = "98422f418b63745e51ee2dc0970b3d06ef308a0eb27e8829df9356aae5d2608e"
EXPECTED_OVERLAY_BYTES = "68eff466212f0322a45a16648c1fcfd51a301bd5351c93f0795147f2baa33969"
EXPECTED_CORE_SEMANTIC = "e3e4d63aba1065bf37a2e5460decfd417c1f76d315405fce941801809d1fb0a2"
EXPECTED_OVERLAY_SEMANTIC = "206f7e6c2eb9e5bb3750dd6e12f6f920228d719868b2e4922395433a2315c71a"


def fail(message: str) -> None:
    raise SystemExit(f"proof-actionability no-go validation failed: {message}")


def require(condition: bool, message: str) -> None:
    if not condition:
        fail(message)


def load(path: Path) -> dict[str, Any]:
    return json.loads(path.read_text())


def sha256(path: Path) -> str:
    return hashlib.sha256(path.read_bytes()).hexdigest()


def canonical_sha(value: Any) -> str:
    payload = json.dumps(value, sort_keys=True, separators=(",", ":")).encode()
    return hashlib.sha256(payload).hexdigest()


def summary(rows: list[dict[str, Any]]) -> dict[str, int | float]:
    non_action = sum(not row["truth"]["worthy"] for row in rows)
    worthy = len(rows) - non_action
    return {
        "non_action": non_action,
        "non_action_precision": non_action / len(rows) if rows else 0.0,
        "reviewed": len(rows),
        "worthy": worthy,
    }


def boundary_row(row: dict[str, Any]) -> dict[str, Any]:
    predicate = (
        "trivial" if row["predicate_results"]["existing_trivial"] else "shallow-extraction"
    )
    return {
        "position_key": row["position_key"],
        "predicate": predicate,
        "source_bounds_sha256": canonical_sha(row["source_bounds"]),
        "truth_reason": row["truth"]["reason"],
        "witness": row["facets"]["witness"],
        "worthy": row["truth"]["worthy"],
    }


def derived(core: dict[str, Any]) -> dict[str, Any]:
    head = [row for row in core["head_rows"] if row["predicate_results"]["proof_backed"]]
    deep = [
        row for row in core["deep_labeled_rows"] if row["predicate_results"]["proof_backed"]
    ]
    proof = head + deep
    boundaries = [
        row
        for row in proof
        if row["predicate_results"]["existing_trivial"]
        or row["predicate_results"]["existing_shallow"]
    ]
    trivial = [row for row in boundaries if row["predicate_results"]["existing_trivial"]]
    shallow = [row for row in boundaries if row["predicate_results"]["existing_shallow"]]
    worthy = [row for row in proof if row["truth"]["worthy"]]
    return {
        "blanket_proof_cohort": {
            "combined": summary(proof),
            "deep": summary(deep),
            "head": summary(head),
            "position_key_set_sha256": canonical_sha([row["position_key"] for row in proof]),
            "worthy_hard_negative_reasons": dict(
                sorted(Counter(row["truth"]["reason"] for row in worthy).items())
            ),
            "worthy_position_key_set_sha256": canonical_sha(
                [row["position_key"] for row in worthy]
            ),
        },
        "current_exemption_cohort": {
            "combined": summary(boundaries),
            "predicate_summaries": {
                "shallow-extraction": summary(shallow),
                "trivial": summary(trivial),
            },
            "rows": [boundary_row(row) for row in boundaries],
        },
    }


def validate(path: Path = DEFAULT, *, check_reviewed_bytes: bool = True) -> None:
    artifact = load(path)
    require(
        set(artifact)
        == {
            "binary_identity",
            "blanket_proof_cohort",
            "current_exemption_cohort",
            "decision",
            "hard_negative_boundaries",
            "heldout_policy",
            "independent_review",
            "inputs",
            "issue",
            "preservation",
            "schema",
            "split",
        },
        "unexpected top-level fields",
    )
    if path.resolve() == DEFAULT.resolve() and check_reviewed_bytes:
        require(sha256(path) == EXPECTED_ARTIFACT_SHA, "closeout artifact bytes changed")
    require(artifact["schema"] == "nose.proof_actionability_no_go.v1", "wrong schema")
    require(artifact["issue"] == 844 and artifact["split"] == "dev", "wrong scope")
    require("closed" in artifact["heldout_policy"], "held-out was not kept closed")
    require(
        artifact["binary_identity"]
        == {
            "closeout_sha256": "f7fcda30aa63662f95000af7029eaf028c71ef074a18ba5e1e2048fe27c47fd0",
            "immediate_parent_commit": "1384e601957d60628cfce72cba4346ca0b6a4e43",
            "immediate_parent_sha256": "f7fcda30aa63662f95000af7029eaf028c71ef074a18ba5e1e2048fe27c47fd0",
            "published_v0_19_0_sha256": "0f73ea544da06cc175e01c31c383cc4cb86daf3d37a49d74de61dea3724fe0f3",
        },
        "binary identity changed",
    )

    core = load(CORE)
    overlay = load(OVERLAY)
    inputs = artifact["inputs"]
    require(
        set(inputs) == {"taxonomy_core", "taxonomy_overlay"},
        "unexpected taxonomy inputs",
    )
    require(
        inputs["taxonomy_core"]["path"]
        == "bench/labels/default_head_taxonomy_2026_07_13.dev.core.v1.json",
        "core path changed",
    )
    require(
        inputs["taxonomy_overlay"]["path"]
        == "bench/labels/default_head_taxonomy_2026_07_13.dev.v1.json",
        "overlay path changed",
    )
    require(sha256(CORE) == inputs["taxonomy_core"]["byte_sha256"], "core bytes changed")
    require(sha256(OVERLAY) == inputs["taxonomy_overlay"]["byte_sha256"], "overlay bytes changed")
    require(inputs["taxonomy_core"]["byte_sha256"] == EXPECTED_CORE_BYTES, "unreviewed core")
    require(
        inputs["taxonomy_overlay"]["byte_sha256"] == EXPECTED_OVERLAY_BYTES,
        "unreviewed overlay",
    )
    require(core["core_sha256"] == EXPECTED_CORE_SEMANTIC, "core semantic digest changed")
    require(
        overlay["artifact_sha256"] == EXPECTED_OVERLAY_SEMANTIC,
        "overlay semantic digest changed",
    )
    require(
        inputs["taxonomy_core"]["semantic_sha256"] == core["core_sha256"],
        "core semantic binding changed",
    )
    require(
        inputs["taxonomy_overlay"]["semantic_sha256"] == overlay["artifact_sha256"],
        "overlay semantic binding changed",
    )

    expected = derived(core)
    for key, value in expected.items():
        require(artifact[key] == value, f"{key} is not derived from the taxonomy")
    require(
        artifact["hard_negative_boundaries"]
        == {
            "data_tables": [
                "prometheus:524550adcecb8123:rank-2",
                "prometheus:c1601349e39f9af1:rank-3",
                "zap:b85ddcf733f2377d:rank-8",
            ],
            "small_helper": ["clap:b526f00d436e1689:rank-10"],
        },
        "hard-negative boundaries changed",
    )
    rows_by_key = {
        row["position_key"]: row for row in core["head_rows"] + core["deep_labeled_rows"]
    }
    for kind, keys in artifact["hard_negative_boundaries"].items():
        expected_reason = "extract-helper" if kind == "small_helper" else "extract-data-table"
        for key in keys:
            require(key in rows_by_key, "hard-negative key is outside the frozen rows")
            row = rows_by_key[key]
            require(row["predicate_results"]["proof_backed"], "hard negative lost proof")
            require(row["truth"]["worthy"], "hard negative is no longer worthy")
            require(row["truth"]["reason"] == expected_reason, "hard-negative reason changed")
    require(
        artifact["independent_review"]
        == {
            "all_five_source_labels_unanimous": True,
            "no_defensible_90_percent_subcohort": True,
            "reviewers": ["evidence", "product", "soundness"],
        },
        "independent-review result changed",
    )
    require(
        artifact["decision"]
        == {
            "minimum_non_action_precision": 0.9,
            "product_behavior_change": False,
            "reason": "no audited proof/actionability predicate reaches the precision gate",
            "result": "rejected-no-go",
        },
        "decision changed",
    )
    require(
        artifact["preservation"]
        == {
            "accepted_pair_coverage_unchanged": True,
            "canon_preservation_violations_zero": True,
            "family_membership_and_fingerprint_unchanged": True,
            "family_universe_recall_unchanged": True,
            "false_merges_zero": True,
            "fold_forest_unchanged": True,
            "surface_and_reason_transitions_zero": True,
            "witness_and_provenance_unchanged": True,
        },
        "required semantic or output-preservation gates changed",
    )
    require(
        artifact["current_exemption_cohort"]["combined"]["non_action_precision"] < 0.9,
        "current exemption removal unexpectedly clears the gate",
    )
    require(
        artifact["blanket_proof_cohort"]["combined"]["non_action_precision"] < 0.9,
        "blanket proof classifier unexpectedly clears the gate",
    )
    print(f"proof/actionability no-go OK: {path.relative_to(ROOT)}")


def self_test() -> None:
    artifact = load(DEFAULT)
    mutated = copy.deepcopy(artifact)
    mutated["current_exemption_cohort"]["rows"][0]["worthy"] = False
    temporary = DEFAULT.with_suffix(".self-test.json")
    temporary.write_text(json.dumps(mutated))
    try:
        try:
            validate(temporary, check_reviewed_bytes=False)
        except SystemExit:
            pass
        else:
            fail("truth mutation was accepted")
    finally:
        temporary.unlink(missing_ok=True)
    validate(DEFAULT)
    print("proof/actionability no-go self-test passed")


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("artifact", nargs="?", type=Path, default=DEFAULT)
    parser.add_argument("--self-test", action="store_true")
    args = parser.parse_args()
    if args.self_test:
        self_test()
    else:
        validate(args.artifact.resolve())
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
