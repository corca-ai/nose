#!/usr/bin/env python3
"""Validate the checked #842 generated-provenance closeout and its evidence chain."""

from __future__ import annotations

import argparse
import hashlib
import json
from pathlib import Path
from typing import Any


ROOT = Path(__file__).resolve().parents[2]
DEFAULT = ROOT / "bench/labels/generated_provenance_closeout_2026_07_13.dev.v1.json"
TOP_KEYS = {
    "schema",
    "issue",
    "split",
    "heldout_policy",
    "implementation",
    "frozen_841_evidence",
    "expanded_default_behavior",
    "established_semantic_behavior",
    "performance",
    "result",
}
OFFICIAL_SHA = "0f73ea544da06cc175e01c31c383cc4cb86daf3d37a49d74de61dea3724fe0f3"


def fail(message: str) -> None:
    raise SystemExit(message)


def require(condition: bool, message: str) -> None:
    if not condition:
        fail(message)


def sha256(path: Path) -> str:
    return hashlib.sha256(path.read_bytes()).hexdigest()


def load(path: Path) -> dict[str, Any]:
    try:
        value = json.loads(path.read_text(encoding="utf-8"))
    except (OSError, json.JSONDecodeError) as error:
        fail(f"{path}: {error}")
    require(isinstance(value, dict), f"{path}: expected a JSON object")
    return value


def validate(path: Path) -> None:
    artifact = load(path)
    require(set(artifact) == TOP_KEYS, f"{path}: unexpected top-level schema")
    require(artifact["schema"] == "nose.generated_provenance_closeout.v1", "wrong schema")
    require(artifact["issue"] == 842 and artifact["split"] == "dev", "wrong issue or split")
    require(artifact["result"] == "pass", "closeout result is not pass")
    require("held-out" in artifact["heldout_policy"], "held-out policy is missing")

    implementation = artifact["implementation"]
    predicate = implementation["predicate"]
    require(predicate["bounded_prefix_bytes"] == 65536, "wrong byte bound")
    require(predicate["op"] == "all_unique_member_files", "wrong family quantifier")
    require(predicate["suffix"] == ".html", "wrong source suffix")
    require(
        predicate["requires_any"]
        == [["jazzy.css", "jazzy.js"], ['class="dashanchor"', "//apple_ref/"]],
        "wrong Jazzy provenance classes",
    )
    require(not implementation["repository_or_path_allowlist"], "allowlist must remain false")
    require(implementation["json_reason_code"] == {"field": "surface", "value": "generated"}, "wrong JSON reason")
    require(implementation["human_reason_code"] == "generated-code", "wrong human reason")

    frozen = artifact["frozen_841_evidence"]
    taxonomy = load(ROOT / frozen["taxonomy_artifact"])
    require(taxonomy.get("artifact_sha256") == frozen["taxonomy_artifact_sha256"], "#841 taxonomy binding changed")
    require(frozen["head_positives"] == {"expected": 10, "generated": 10, "missing": 0}, "head positives changed")
    require(frozen["deep_audit_positives"] == {"expected": 20, "generated": 20, "missing": 0}, "deep positives changed")
    require(frozen["html_hard_negatives"] == {"expected": 3, "default": 3, "false_demotions": 0}, "hard negatives changed")

    expanded = artifact["expanded_default_behavior"]
    require(expanded["repositories"] == 66 and expanded["families"] == 54754, "expanded corpus totals changed")
    require(expanded["byte_identical_repositories"] == 65, "expanded drift breadth changed")
    require(expanded["changed_repositories"] == ["alamofire"], "unexpected expanded drift")
    require(expanded["family_id_order_equal"] and expanded["non_surface_fields_equal"], "expanded identity invariant failed")
    require(sum(expanded["alamofire"]["surface_transitions"].values()) == 507, "wrong expanded transition count")

    semantic = artifact["established_semantic_behavior"]
    require(semantic["repositories"] == 66, "wrong semantic repository count")
    require(semantic["families_before"] == semantic["families_after"] == 9850, "semantic family total changed")
    require(semantic["byte_identical_repositories"] == 65, "semantic drift breadth changed")
    require(semantic["changed_repositories"] == ["alamofire"], "unexpected semantic drift")
    require(semantic["family_id_order_equal"] and semantic["non_surface_fields_equal"], "semantic identity invariant failed")

    performance = artifact["performance"]
    require(performance["published_baseline"]["binary_sha256"] == OFFICIAL_SHA, "official baseline changed")
    expected_repo_counts = [66, 66, 13, 13, 1, 1]
    for evidence, expected_repos in zip(performance["artifacts"], expected_repo_counts, strict=True):
        evidence_path = ROOT / evidence["path"]
        require(sha256(evidence_path) == evidence["sha256"], f"{evidence_path}: SHA-256 mismatch")
        report = load(evidence_path)
        require(report.get("schema") == "nose.query_regression_harness.v2", f"{evidence_path}: wrong schema")
        require(len(report.get("repos", [])) == expected_repos, f"{evidence_path}: wrong repository count")
        for repo in report["summary"]["by_repo"].values():
            require(len(repo["baseline"]["hashes"]) == 1, f"{evidence_path}: unstable baseline output")
            require(len(repo["current"]["hashes"]) == 1, f"{evidence_path}: unstable current output")
    for key in (
        "all_dev_three_iteration",
        "thirteen_repo_nine_iteration_recheck",
        "nginx_twenty_one_iteration_recheck",
    ):
        require(not performance[key]["material_regression"], f"{key}: material regression")

    print(f"generated provenance closeout OK: {path.relative_to(ROOT)}")


def self_test() -> None:
    require(
        hashlib.sha256(b"nose").hexdigest()
        == "d77e22123e64d3d87f1f95d9cff7a0b6af6c32b9a81552cb90e991eb55cf63d4",
        "SHA self-test failed",
    )
    print("generated provenance closeout self-test passed")


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
