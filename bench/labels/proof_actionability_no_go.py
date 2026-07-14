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
PARENT_CLOSEOUT = (
    ROOT / "bench/labels/declaration_type_contract_closeout_2026_07_14.dev.v1.json"
)
EXPECTED_ARTIFACT_SHA = "dc92cd6ca9741724c71ec69003eaaee737109ffe48845ad2866a6ae0baad4154"
EXPECTED_CORE_BYTES = "98422f418b63745e51ee2dc0970b3d06ef308a0eb27e8829df9356aae5d2608e"
EXPECTED_OVERLAY_BYTES = "68eff466212f0322a45a16648c1fcfd51a301bd5351c93f0795147f2baa33969"
EXPECTED_PARENT_CLOSEOUT_BYTES = (
    "1f393994f95e15a676f60eeee7f7594be65c31c646e573362aba0d3721ff3732"
)
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


def review_packet(rows: list[dict[str, Any]]) -> list[dict[str, Any]]:
    return [
        {
            key: row[key]
            for key in ("position_key", "predicate", "source_bounds_sha256", "witness")
        }
        for row in rows
    ]


def review_judgments(rows: list[dict[str, Any]]) -> list[dict[str, Any]]:
    return [
        {key: row[key] for key in ("position_key", "truth_reason", "worthy")} for row in rows
    ]


def wilson_lower(successes: int, reviewed: int, z: float = 1.6448536269514722) -> float:
    if reviewed == 0:
        return 0.0
    proportion = successes / reviewed
    denominator = 1 + z * z / reviewed
    center = proportion + z * z / (2 * reviewed)
    radius = z * ((proportion * (1 - proportion) / reviewed + z * z / (4 * reviewed**2)) ** 0.5)
    return (center - radius) / denominator


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


def validate_parent_evidence(
    parent: dict[str, Any], artifact: dict[str, Any]
) -> dict[str, Any]:
    require(
        parent["schema"] == "nose.declaration_type_contract_closeout.v1"
        and parent["issue"] == 843
        and parent["split"] == "dev"
        and parent["result"] == "pass",
        "wrong parent closeout",
    )
    require(
        parent["heldout_policy"] == "closed; no held-out checkout or judgment was opened",
        "parent held-out policy changed",
    )
    binary = artifact["binary_identity"]
    require(
        parent["implementation"]["binary_sha256"] == binary["parent"]["file_sha256"],
        "parent binary is not bound to #843",
    )
    require(
        parent["implementation"]["commit"]
        == binary["parent"]["checked_evidence_commit"],
        "parent implementation commit changed",
    )
    performance = parent["performance"]
    require(
        performance["current"]["binary_sha256"] == binary["parent"]["file_sha256"],
        "parent performance current binary changed",
    )
    require(
        performance["current"]["commit"] == binary["parent"]["checked_evidence_commit"],
        "parent performance commit changed",
    )
    require(
        performance["published_baseline"]["binary_sha256"]
        == binary["published_v0_19_0"]["file_sha256"],
        "published performance baseline changed",
    )
    require(len(performance["artifacts"]) == 8, "parent performance chain changed")
    for reference in performance["artifacts"]:
        evidence_path = ROOT / reference["path"]
        require(sha256(evidence_path) == reference["sha256"], "raw performance evidence changed")

    behavior = parent["behavior_evidence"]
    behavior_path = ROOT / behavior["path"]
    require(sha256(behavior_path) == behavior["sha256"], "parent behavior evidence changed")
    require(
        load(behavior_path)["evidence_digest"] == behavior["evidence_digest"],
        "parent behavior digest changed",
    )
    quality = parent["product_quality"]
    quality_path = ROOT / quality["path"]
    require(sha256(quality_path) == quality["sha256"], "parent quality evidence changed")
    require(
        quality["worthy_recall"] == {"hits": 2716, "n": 2849, "pct": 95.3317}
        and quality["comparison_worthy_recall_delta"] == 0,
        "parent worthy recall changed",
    )
    return {
        "basis": "normalized executable-code identity with the checked #843 parent",
        "incremental": {
            "accepted_pair_coverage_delta": 0,
            "canon_preservation_violations": 0,
            "family_membership_or_fingerprint_changes": 0,
            "false_merges": 0,
            "fold_forest_changes": 0,
            "surface_or_reason_transitions": 0,
            "witness_or_provenance_changes": 0,
            "worthy_recall_delta": 0,
        },
        "parent_behavior": {
            "evidence_digest": behavior["evidence_digest"],
            "sha256": behavior["sha256"],
        },
        "parent_product_quality": {
            "sha256": quality["sha256"],
            "worthy_recall": {key: quality["worthy_recall"][key] for key in ("hits", "n")},
        },
    }


def validate_independent_reviews(
    independent: dict[str, Any], boundary_rows: list[dict[str, Any]]
) -> None:
    packet_sha = canonical_sha(review_packet(boundary_rows))
    protocol = independent["protocol"]
    minimum_reviewed = next(n for n in range(1, 1000) if wilson_lower(n, n) >= 0.9)
    require(
        protocol
        == {
            "confidence_method": "one-sided Wilson 95% lower bound",
            "minimum_confirmatory_reviewed": minimum_reviewed,
            "minimum_non_action_point_precision": 0.9,
            "minimum_non_action_wilson_lower_bound": 0.9,
            "source_packet_sha256": packet_sha,
        },
        "review protocol changed",
    )
    require(minimum_reviewed == 25, "confirmatory minimum changed")
    expected_judgments = review_judgments(boundary_rows)
    reviews = independent["reviews"]
    require(len(reviews) == 3, "review count changed")
    require(
        {review["reviewer"] for review in reviews} == {"evidence", "product", "soundness"},
        "reviewer roles changed",
    )
    for review in reviews:
        require(
            set(review)
            == {
                "decision",
                "defensible_90_percent_subcohort",
                "judgments",
                "reviewer",
                "source_packet_sha256",
            },
            "review schema changed",
        )
        require(review["source_packet_sha256"] == packet_sha, "review packet changed")
        require(review["judgments"] == expected_judgments, "review judgments changed")
        require(review["decision"] == "rejected-no-go", "review decision changed")
        require(
            review["defensible_90_percent_subcohort"] is False,
            "review admitted an unqualified subcohort",
        )
    unanimous = all(review["judgments"] == reviews[0]["judgments"] for review in reviews)
    no_subcohort = all(not review["defensible_90_percent_subcohort"] for review in reviews)
    require(
        independent["summary"]
        == {
            "all_five_source_labels_unanimous": unanimous,
            "no_defensible_90_percent_subcohort": no_subcohort,
            "reviewer_count": len(reviews),
        },
        "review summary is not derived",
    )


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
    require(
        artifact["heldout_policy"]
        == {
            "opened": False,
            "split": "dev",
            "statement": "no held-out component, source path, or judgment was read",
        },
        "held-out policy changed",
    )
    require(
        artifact["binary_identity"]
        == {
            "algorithm": "sha256/mach-o-zero-uuid-signature-v1",
            "candidate": {
                "build_command": (
                    "CARGO_TARGET_DIR=/tmp/nose-844-review-target "
                    "cargo build --release -p nose-cli"
                ),
                "code_sha256": "03cc5827cdadc225478a34266de78805c6e495810f90e8642f2ae2807b3a4f5a",
                "file_sha256": "c0a70c0d31739da42260c94f910ddd89d46ae5e6979542f9d88d82a9808a42b3",
                "source_commit": "0b57bd183317b35ea082b1629391ac59f748cab4",
                "source_tree": "b81480bcf0d8b99137c9a4862ec3ea3b7a9d6e28",
            },
            "full_file_equal": False,
            "normalized_code_equal": True,
            "parent": {
                "checked_evidence_commit": "182881a8097ff14ecf513a4fb32f1ad22cc31394",
                "checked_evidence_tree": "7b4b4734571af8206311aa8862315b680619e6db",
                "code_sha256": "03cc5827cdadc225478a34266de78805c6e495810f90e8642f2ae2807b3a4f5a",
                "file_sha256": "f7fcda30aa63662f95000af7029eaf028c71ef074a18ba5e1e2048fe27c47fd0",
                "merge_commit": "1384e601957d60628cfce72cba4346ca0b6a4e43",
                "merge_tree": "6aae853031752a26ed2f80d40b0e548514723dff",
            },
            "published_v0_19_0": {
                "code_sha256": "e55d0e989993ff1d1d6b4e933dbd3f5ade38203368b8321d3a7842799a95aca6",
                "file_sha256": "0f73ea544da06cc175e01c31c383cc4cb86daf3d37a49d74de61dea3724fe0f3",
            },
        },
        "binary identity changed",
    )
    require(
        artifact["binary_identity"]["candidate"]["code_sha256"]
        == artifact["binary_identity"]["parent"]["code_sha256"],
        "candidate executable code differs from the parent",
    )
    require(
        artifact["binary_identity"]["candidate"]["file_sha256"]
        != artifact["binary_identity"]["parent"]["file_sha256"],
        "Mach-O full-file distinction was not recorded",
    )

    core = load(CORE)
    overlay = load(OVERLAY)
    inputs = artifact["inputs"]
    require(
        set(inputs) == {"parent_closeout", "taxonomy_core", "taxonomy_overlay"},
        "unexpected closeout inputs",
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
    require(
        inputs["parent_closeout"]
        == {
            "byte_sha256": EXPECTED_PARENT_CLOSEOUT_BYTES,
            "path": "bench/labels/declaration_type_contract_closeout_2026_07_14.dev.v1.json",
            "schema": "nose.declaration_type_contract_closeout.v1",
            "split": "dev",
        },
        "parent closeout binding changed",
    )
    require(sha256(CORE) == inputs["taxonomy_core"]["byte_sha256"], "core bytes changed")
    require(sha256(OVERLAY) == inputs["taxonomy_overlay"]["byte_sha256"], "overlay bytes changed")
    require(
        sha256(PARENT_CLOSEOUT) == inputs["parent_closeout"]["byte_sha256"],
        "parent closeout bytes changed",
    )
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
    for value, label in ((core, "core"), (overlay, "overlay")):
        require(value["split"] == "dev", f"{label} is not dev-only")
        require(
            value["heldout_policy"]
            == "closed; no held-out component, source path, or judgment was read",
            f"{label} held-out policy changed",
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
    validate_independent_reviews(
        artifact["independent_review"], artifact["current_exemption_cohort"]["rows"]
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
    parent = load(PARENT_CLOSEOUT)
    expected_preservation = validate_parent_evidence(parent, artifact)
    require(
        artifact["preservation"] == expected_preservation,
        "preservation is not derived from parent evidence and code identity",
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
    temporary = DEFAULT.with_suffix(".self-test.json")

    def rejects(mutated: dict[str, Any], label: str) -> None:
        temporary.write_text(json.dumps(mutated))
        try:
            validate(temporary, check_reviewed_bytes=False)
        except SystemExit:
            return
        else:
            fail(f"{label} mutation was accepted")

    try:
        mutated = copy.deepcopy(artifact)
        mutated["current_exemption_cohort"]["rows"][0]["worthy"] = False
        rejects(mutated, "truth")

        mutated = copy.deepcopy(artifact)
        mutated["independent_review"]["reviews"][0]["judgments"][0]["worthy"] = False
        rejects(mutated, "review")

        mutated = copy.deepcopy(artifact)
        mutated["heldout_policy"] = "not closed; held-out source was read"
        rejects(mutated, "held-out policy")

        mutated = copy.deepcopy(artifact)
        mutated["preservation"]["incremental"]["surface_or_reason_transitions"] = 1
        rejects(mutated, "preservation")
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
