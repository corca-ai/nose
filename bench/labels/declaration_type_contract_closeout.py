#!/usr/bin/env python3
"""Validate #843 from bound behavior, quality, and raw performance evidence."""

from __future__ import annotations

import argparse
import copy
from pathlib import Path
from typing import Any

import declaration_type_contract_behavior as behavior
import generated_provenance_closeout as generated


ROOT = generated.ROOT
DEFAULT = ROOT / "bench/labels/declaration_type_contract_closeout_2026_07_14.dev.v1.json"
EXPECTED_CLOSEOUT_SHA = "1f393994f95e15a676f60eeee7f7594be65c31c646e573362aba0d3721ff3732"
OFFICIAL_SHA = generated.OFFICIAL_SHA
OFFICIAL_TAG_OBJECT = generated.OFFICIAL_TAG_OBJECT
OFFICIAL_COMMIT = generated.OFFICIAL_COMMIT
PARENT_SHA = behavior.PARENT_BINARY_SHA
CURRENT_SHA = behavior.CURRENT_BINARY_SHA
CURRENT_SOURCE = behavior.CURRENT_COMMIT
QUERY_COMMAND = generated.QUERY_COMMAND


def quality_projection(report: dict[str, Any]) -> dict[str, Any]:
    overall = report["metrics"]["dev"]["OVERALL"]
    def summary(name: str) -> dict[str, int | float]:
        value = overall[name]
        return {key: value[key] for key in ("hits", "n", "pct")}

    return {
        "comparison_worthy_recall_delta": report["comparison"]["worthy_recall"]["delta"],
        "default_label_coverage": summary("label_match_coverage"),
        "default_precision_at_10": summary("precision_at_10"),
        "worthy_recall": summary("worthy_recall"),
    }


def validate_quality(reference: dict[str, Any]) -> None:
    path = ROOT / reference["path"]
    generated.require(generated.sha256(path) == reference["sha256"], "quality SHA-256 mismatch")
    report = generated.load(path)
    generated.require(report["schema"] == "nose.product_quality_evaluation.v3", "wrong quality schema")
    generated.require(report["repository_count"] == 66, "quality repository count changed")
    generated.require(report["configuration"]["splits"] == ["dev"], "quality split changed")
    generated.require(report["configuration"]["cache_policy"].startswith("disabled"), "quality used cache")
    generated.require(report["configuration"]["precision_surface"] == "default", "wrong precision surface")
    provenance = report["provenance"]
    generated.require(provenance["git_sha"] == CURRENT_SOURCE, "quality source changed")
    generated.require(provenance["nose_binary_sha256"] == CURRENT_SHA, "quality binary changed")
    generated.require(
        report["comparison"]["provenance"]["nose_binary_sha256"] == PARENT_SHA,
        "quality comparison is not the immediate parent",
    )
    comparison = report["comparison"]["worthy_recall"]
    generated.require(comparison["current_hits"] == comparison["comparison_hits"] == 2716, "worthy hits changed")
    generated.require(comparison["recovered"] == comparison["regressed"] == [], "worthy IDs changed")
    actual = {"path": reference["path"], "sha256": reference["sha256"], **quality_projection(report)}
    generated.require(actual == reference, "quality summary is not derived from its report")


def validate_report_role(
    report: dict[str, Any],
    control: dict[str, Any],
    *,
    repositories: int,
    iterations: int,
    warmups: int,
    corpus_rows: list[dict[str, str]],
    corpus_sha: str,
    label: str,
) -> None:
    for value, role in ((report, "report"), (control, "control")):
        generated.require(value["schema"] == "nose.query_regression_harness.v2", f"{label}:{role}: schema")
        generated.require(value["command"] == QUERY_COMMAND, f"{label}:{role}: command")
        generated.require(
            value["measurement"] == {"iterations": iterations, "warmups": warmups},
            f"{label}:{role}: measurement",
        )
        generated.require(len(value["repos"]) == repositories, f"{label}:{role}: repo count")
        generated.stable_outputs(value, f"{label}:{role}")
        generated.validate_bound_corpus(value, corpus_rows, corpus_sha, f"{label}:{role}")
    generated.require(report["repos"] == control["repos"], f"{label}: control repos differ")
    primary = report["provenance"]
    generated.require(primary["baseline_binary_sha256"] == OFFICIAL_SHA, f"{label}: baseline binary")
    generated.require(primary["baseline_source_ref"] == "v0.19.0", f"{label}: baseline ref")
    generated.require(primary["baseline_source_sha"] == OFFICIAL_TAG_OBJECT, f"{label}: baseline tag")
    generated.require(primary["current_binary_sha256"] == CURRENT_SHA, f"{label}: current binary")
    generated.require(primary["current_source_sha"] == CURRENT_SOURCE, f"{label}: current source")
    control_provenance = control["provenance"]
    for key in ("baseline_binary_sha256", "current_binary_sha256"):
        generated.require(control_provenance[key] == CURRENT_SHA, f"{label}: control binary")
    for key in ("baseline_source_sha", "current_source_sha"):
        generated.require(control_provenance[key] == CURRENT_SOURCE, f"{label}: control source")


def checker_command(report: str, control: str, drift: str) -> list[str]:
    return generated.checker_base(report, control, drift)


def validate_escalation(
    evidence: list[dict[str, str]], reports: list[dict[str, Any]], drift: str
) -> None:
    expected_codes = [1, 1, 0]
    for edge, expected_code in enumerate(expected_codes):
        primary = edge * 2
        focused = primary + 2
        requested, _ = generated.requested_focused_repos(
            evidence[primary]["path"],
            evidence[primary + 1]["path"],
            drift,
            f"#843 escalation request {edge}",
        )
        generated.require(requested == reports[focused]["repos"], f"edge {edge}: focused repos differ")
        command = checker_command(
            evidence[primary]["path"], evidence[primary + 1]["path"], drift
        ) + [
            "--focused-report",
            evidence[focused]["path"],
            "--focused-same-binary-control",
            evidence[focused + 1]["path"],
            "--min-focused-iterations",
            str(reports[focused]["measurement"]["iterations"]),
        ]
        output = generated.run_checker(command, f"#843 escalation result {edge}", expected_code)
        if edge == 2:
            generated.require("passed after focused rerun" in output, "final checker did not pass")


def validate_performance(performance: dict[str, Any], corpus: dict[str, Any]) -> None:
    generated.validate_published_baseline(performance["published_baseline"])
    generated.require(
        performance["current"] == {"commit": CURRENT_SOURCE, "binary_sha256": CURRENT_SHA},
        "current performance role changed",
    )
    drift = performance["expected_drift_manifest"]
    drift_path = ROOT / drift["path"]
    generated.require(generated.sha256(drift_path) == drift["sha256"], "drift manifest SHA mismatch")
    manifest = generated.load(drift_path)
    generated.require([entry["repo"] for entry in manifest["entries"]] == ["alamofire", "netty"], "drift repos changed")
    evidence = performance["artifacts"]
    generated.require(len(evidence) == 8, "expected four performance pairs")
    reports = []
    for item in evidence:
        path = ROOT / item["path"]
        generated.require(generated.sha256(path) == item["sha256"], f"{path}: SHA mismatch")
        reports.append(generated.load(path))
    specs = [
        (66, 3, 1, "all-dev"),
        (25, 9, 2, "focused-r9"),
        (6, 21, 2, "focused-r21"),
        (2, 40, 2, "focused-r40"),
    ]
    for index, (repositories, iterations, warmups, label) in enumerate(specs):
        validate_report_role(
            reports[index * 2],
            reports[index * 2 + 1],
            repositories=repositories,
            iterations=iterations,
            warmups=warmups,
            corpus_rows=corpus["repositories"],
            corpus_sha=corpus["manifest_sha256"],
            label=label,
        )
    generated.compare_metrics(
        performance["all_dev_three_iteration"],
        generated.aggregate(reports[0], reports[1]),
        "all-dev performance",
    )
    generated.compare_metrics(
        performance["final_forty_iteration"],
        generated.aggregate(reports[6], reports[7]),
        "final-r40 performance",
    )
    generated.require(
        performance["escalation"]
        == {
            "iterations": [3, 9, 21, 40],
            "repository_counts": [66, 25, 6, 2],
            "result": "pass",
        },
        "wrong escalation summary",
    )
    validate_escalation(evidence, reports, drift["path"])


def validate(path: Path) -> None:
    artifact = generated.load(path)
    generated.require(
        set(artifact)
        == {
            "behavior_evidence",
            "dogfood",
            "heldout_policy",
            "implementation",
            "issue",
            "performance",
            "product_quality",
            "result",
            "schema",
            "split",
        },
        "unexpected closeout fields",
    )
    if path == DEFAULT:
        generated.require(generated.sha256(path) == EXPECTED_CLOSEOUT_SHA, "closeout SHA is not reviewed")
    generated.require(artifact["schema"] == "nose.declaration_type_contract_closeout.v1", "wrong schema")
    generated.require(artifact["issue"] == 843 and artifact["split"] == "dev", "wrong scope")
    generated.require(artifact["result"] == "pass", "closeout result is not pass")
    generated.require("held-out" in artifact["heldout_policy"], "held-out policy missing")
    implementation = artifact["implementation"]
    generated.require(implementation["commit"] == CURRENT_SOURCE, "implementation source changed")
    generated.require(implementation["binary_sha256"] == CURRENT_SHA, "implementation binary changed")
    generated.require(not implementation["origin_vocabulary_expanded"], "origin vocabulary expanded")
    generated.require(not implementation["repository_or_language_allowlist"], "allowlist introduced")
    generated.require(
        implementation["predicate"]
        == {
            "forbids_runtime_data_implementation_or_behavior_evidence": True,
            "location_kind": "class",
            "op": "all_locations",
            "requires_body_kind": "declaration-only",
            "requires_domains_exactly": ["type-contract"],
            "requires_evidence": ["declaration-only", "type-only"],
            "requires_non_fragment": True,
            "requires_source_granularity": "whole-unit",
            "subkinds": ["interface-trait-protocol", "type-alias", "defined-type"],
        },
        "declaration type-contract predicate changed",
    )
    generated.require(
        implementation["json_reason_code"] == {"field": "surface", "value": "declaration"},
        "JSON reason contract changed",
    )
    behavior_ref = artifact["behavior_evidence"]
    behavior_path = ROOT / behavior_ref["path"]
    generated.require(generated.sha256(behavior_path) == behavior_ref["sha256"], "behavior SHA mismatch")
    behavior_artifact = generated.load(behavior_path)
    generated.require(
        behavior_artifact["evidence_digest"] == behavior_ref["evidence_digest"],
        "behavior digest changed",
    )
    generated.run_checked(
        ["python3", "bench/labels/declaration_type_contract_behavior.py", "validate"],
        "behavior validation",
    )
    validate_quality(artifact["product_quality"])
    validate_performance(artifact["performance"], behavior_artifact["corpus"])
    generated.require(
        artifact["dogfood"]
        == {
            "accepted_default_families": 28,
            "budget": 28,
            "new_accepted_families": 0,
            "stale_removed_family": "856ea94f585f0c67",
        },
        "historical dogfood evidence changed",
    )
    print(f"declaration type-contract closeout OK: {path.relative_to(ROOT)}")


def self_test() -> None:
    artifact = generated.load(DEFAULT)
    mutated = copy.deepcopy(artifact)
    mutated["product_quality"]["worthy_recall"]["hits"] -= 1
    try:
        validate_quality(mutated["product_quality"])
    except SystemExit:
        pass
    else:
        generated.fail("quality summary mutation was accepted")
    validate(DEFAULT)
    print("declaration type-contract closeout self-test passed")


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
