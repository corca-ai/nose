#!/usr/bin/env python3
"""Validate the checked #846 / #838 default-head no-go closeout."""

from __future__ import annotations

import argparse
import copy
import hashlib
import json
import math
import subprocess
import tempfile
from collections import defaultdict
from pathlib import Path
from typing import Any

import default_head_fresh_repository_audit as fresh
import eval_by_language as evaluator
import residual_ranking_closeout as residual


ROOT = Path(__file__).resolve().parents[2]
DEFAULT = ROOT / "bench/labels/default_head_closeout_2026_07_14.v1.json"
SIDECAR = ROOT / "bench/labels/default_head_closeout_2026_07_14.v1.json.sha256"
OFFICIAL_SHA = "0f73ea544da06cc175e01c31c383cc4cb86daf3d37a49d74de61dea3724fe0f3"
CURRENT_SHA = "f7fcda30aa63662f95000af7029eaf028c71ef074a18ba5e1e2048fe27c47fd0"
CURRENT_SOURCE = "cdab416706c32ea94bf808ec7ebb36781e483e65"
DEFAULT_DRIFT = ROOT / "bench/labels/default_head_closeout_v0_19_0.expected-drift.v1.json"
SEMANTIC_DRIFT = ROOT / ".github/semantic-regression-expected-drift.json"

DEFAULT_REPORTS = {
    "all120": (
        "bench/recall_loss/issue-846-official-v0.19.0-default-all120-r3-2026-07-14.v1.json",
        "bench/recall_loss/issue-846-official-self-control-default-all120-r3-2026-07-14.v1.json",
    ),
    "r6": (
        "bench/recall_loss/issue-846-official-v0.19.0-default-focused-r6-2026-07-14.v1.json",
        "bench/recall_loss/issue-846-official-self-control-default-focused-r6-2026-07-14.v1.json",
    ),
    "r21": (
        "bench/recall_loss/issue-846-official-v0.19.0-default-focused-r21-2026-07-14.v1.json",
        "bench/recall_loss/issue-846-official-self-control-default-focused-r21-2026-07-14.v1.json",
    ),
    "r40": (
        "bench/recall_loss/issue-846-official-v0.19.0-default-focused-r40-2026-07-14.v1.json",
        "bench/recall_loss/issue-846-official-self-control-default-focused-r40-2026-07-14.v1.json",
    ),
}
SEMANTIC_REPORTS = {
    "primary": (
        "bench/recall_loss/issue-846-official-v0.19.0-semantic-smoke-primary-2026-07-14.v1.json",
        "bench/recall_loss/issue-846-official-self-control-semantic-smoke-primary-2026-07-14.v1.json",
    ),
    "focused": (
        "bench/recall_loss/issue-846-official-v0.19.0-semantic-smoke-focused-2026-07-14.v1.json",
        "bench/recall_loss/issue-846-official-self-control-semantic-smoke-focused-2026-07-14.v1.json",
    ),
}
FINAL_DEFAULT_SIGNALS = [
    "alamofire:lower",
    "alamofire:parse+lower",
    "alamofire:query_gate",
    "alamofire:query_opp",
    "guava:query_opp",
    "netty:query_opp",
    "rxjava:query_opp",
    "sqlalchemy:query_opp",
    "sympy:query_opp",
]
SEMANTIC_SIGNALS = ["prettier:discover", "prettier:parse+lower"]


def require(condition: bool, message: str) -> None:
    if not condition:
        raise ValueError(message)


def close(actual: float, expected: float, label: str) -> None:
    require(
        math.isclose(actual, expected, rel_tol=1e-12, abs_tol=1e-9),
        f"{label}: {actual} != {expected}",
    )


def sha256(path: Path) -> str:
    return hashlib.sha256(path.read_bytes()).hexdigest()


def load(path: Path) -> dict[str, Any]:
    value = json.loads(path.read_text(encoding="utf-8"))
    require(isinstance(value, dict), f"{path}: expected object")
    return value


def checked_evidence(value: dict[str, Any]) -> None:
    evidence = value.get("evidence")
    require(isinstance(evidence, dict), "missing evidence")
    records = [record for record in evidence.values() if isinstance(record, dict)]
    performance = evidence.get("performance")
    require(isinstance(performance, list) and len(performance) == 15, "wrong performance evidence set")
    records.extend(performance)
    seen: set[str] = set()
    for record in records:
        require(set(record) == {"path", "sha256"}, "malformed evidence record")
        path = record["path"]
        require(path not in seen, f"duplicate evidence path: {path}")
        seen.add(path)
        absolute = ROOT / path
        require(absolute.is_file(), f"missing evidence: {path}")
        require(sha256(absolute) == record["sha256"], f"evidence hash changed: {path}")


def exact_dev_metrics() -> tuple[dict[str, Any], dict[str, dict[str, Any]]]:
    calibration = residual.read_json(residual.CALIBRATION)
    component = residual.read_json(residual.COMPONENT)
    dataset, _ = residual.apply_exact_overlay(calibration["dataset"], component)
    flags: dict[str, list[int]] = defaultdict(list)
    for repository in dataset["repositories"].values():
        top = sorted(repository["families"], key=lambda row: row["current_rank"])[:10]
        require(all(row["truth"] is not None for row in top), "incomplete exact dev head")
        flags[repository["language"]].extend(int(row["truth"]) for row in top)
    languages = {
        language: evaluator.binary_metric(
            rows,
            bootstrap=2000,
            rng=evaluator.metric_rng(
                split="dev", scope=language, metric="precision_at_10"
            ),
        )
        for language, rows in sorted(flags.items())
    }
    overall_flags = [flag for language in sorted(flags) for flag in flags[language]]
    overall = evaluator.binary_metric(
        overall_flags,
        bootstrap=2000,
        rng=evaluator.metric_rng(
            split="dev", scope="OVERALL", metric="precision_at_10"
        ),
    )
    return overall, languages


def validate_quality(value: dict[str, Any]) -> None:
    report_path = ROOT / value["evidence"]["quality_evaluation"]["path"]
    report = load(report_path)
    provenance = report["provenance"]
    require(provenance["working_tree_status_before_measurement"] == "", "quality run was dirty")
    require(provenance["nose_binary_sha256"] == CURRENT_SHA, "wrong quality binary")
    require(report["comparison"]["provenance"]["nose_binary_sha256"] == OFFICIAL_SHA, "wrong quality baseline")
    require(report["configuration"]["bootstrap_resamples"] == 2000, "wrong bootstrap count")
    require(report["configuration"]["bootstrap_seed"] == 1, "wrong bootstrap seed")

    exact_overall, exact_languages = exact_dev_metrics()
    dev_report = report["metrics"]["dev"]["OVERALL"]
    expected_dev = {
        "authority": (
            "The exact-key #845 overlay fully judges all reported positions; the standard "
            "evaluator row is retained only as an independently reproducible "
            "coverage-limited cross-check."
        ),
        "precision_at_10": exact_overall,
        "label_match_coverage": {"hits": 658, "n": 658, "pct": 100.0},
        "standard_evaluator_cross_check": {
            **dev_report["precision_at_10"],
            "unmatched_positions": 11,
        },
        "worthy_recall": dev_report["worthy_recall"],
        "languages": exact_languages,
    }
    require(value["quality"]["dev"] == expected_dev, "dev quality summary changed")

    heldout = report["metrics"]["heldout"]
    expected_heldout = {
        "precision_at_10": heldout["OVERALL"]["precision_at_10"],
        "label_match_coverage": heldout["OVERALL"]["label_match_coverage"],
        "worthy_recall": heldout["OVERALL"]["worthy_recall"],
        "languages": {
            language: row["precision_at_10"]
            for language, row in heldout.items()
            if language != "OVERALL"
        },
    }
    require(value["quality"]["heldout"] == expected_heldout, "heldout quality summary changed")
    comparison = report["comparison"]["worthy_recall"]
    expected_comparison = {
        "current_hits": comparison["current_hits"],
        "comparison_hits": comparison["comparison_hits"],
        "delta": comparison["delta"],
        "regressed": comparison["regressed_count"],
        "recovered": comparison["recovered_count"],
    }
    require(
        value["quality"]["worthy_recall_comparison_to_v0_19_0"]
        == expected_comparison,
        "worthy-recall comparison changed",
    )
    require(comparison["regressed"] == [], "worthy regressions present")


def validate_soundness(value: dict[str, Any]) -> None:
    report = load(ROOT / value["evidence"]["soundness"]["path"])
    expected = {
        "total_units": report["summary"]["total_units"],
        "interpretable_units": report["summary"]["interpretable_units"],
        "false_merges": report["soundness_gate"]["false_merges"],
        "canon_checked": report["summary"]["canon_checked"],
        "canon_preservation_violations": report["soundness_gate"][
            "canon_preservation_violations"
        ],
        "completeness": (
            f"{report['completeness']['fingerprint_equal_pairs']}/"
            f"{report['completeness']['behavior_equal_pairs']}"
        ),
        "advisory_disagreements": report["soundness_gate"]["advisory_disagreements"],
        "gate_passed": report["soundness_gate"]["gate_passed"],
    }
    require(value["soundness"] == expected, "soundness summary changed")


def validate_determinism(value: dict[str, Any]) -> None:
    primary = load(ROOT / DEFAULT_REPORTS["all120"][0])
    require(len(primary["repos"]) == 120, "all-120 determinism corpus changed")
    require(
        all(len(row["current"]["hashes"]) == 1 for row in primary["summary"]["by_repo"].values()),
        "repeated all-120 output changed",
    )
    dev = load(ROOT / value["evidence"]["dev_thread_determinism"]["path"])
    require(len(dev["rows"]) == 66, "dev determinism repository count changed")
    require(
        all(len(set(row["determinism"].values())) == 1 for row in dev["rows"]),
        "dev thread determinism failed",
    )
    heldout_path = ROOT / value["evidence"]["heldout_thread_determinism"]["path"]
    heldout_rows = [line.split("\t") for line in heldout_path.read_text().splitlines()]
    require(len(heldout_rows) == 54, "heldout determinism repository count changed")
    require(
        all(len(row) == 4 and row[1] == row[2] and row[3] == "pass" for row in heldout_rows),
        "heldout thread determinism failed",
    )
    expected = {
        "all_120_repeated_runs": {
            "repositories": 120,
            "iterations": 3,
            "all_single_hash": True,
        },
        "dev_thread_counts": {
            "repositories": 66,
            "thread_counts": [1, 4],
            "all_byte_identical": True,
        },
        "heldout_thread_counts": {
            "repositories": 54,
            "thread_counts": [1, 4],
            "all_byte_identical": True,
        },
    }
    require(value["determinism"] == expected, "determinism summary changed")


def run_checker(
    primary: tuple[str, str],
    focused: tuple[str, str],
    drift: Path,
) -> dict[str, Any]:
    with tempfile.TemporaryDirectory(prefix="nose-846-closeout-") as directory:
        status = Path(directory) / "status.json"
        command = [
            "python3",
            "scripts/check-query-regression.py",
            primary[0],
            "--same-binary-control",
            primary[1],
            "--focused-report",
            focused[0],
            "--focused-same-binary-control",
            focused[1],
            "--expected-drift-manifest",
            str(drift.relative_to(ROOT)),
            "--require-same-binary-control",
            "--max-runtime-delta-pct",
            "5",
            "--min-runtime-delta-ms",
            "5",
            "--status-output",
            str(status),
        ]
        result = subprocess.run(command, cwd=ROOT, capture_output=True, text=True)
        require(result.returncode == 1, f"checker did not reproduce failure: {result.stdout}{result.stderr}")
        return load(status)


def signal_names(status: dict[str, Any]) -> list[str]:
    return [
        f"{row['repo']}:{row['stage']}"
        for row in status["focused"]["runtime"]["triggered"]
    ]


def aggregate(status: dict[str, Any]) -> dict[str, Any]:
    return next(
        row
        for row in status["focused"]["runtime"]["signals"]
        if row["scope"] == "aggregate"
    )


def validate_performance(value: dict[str, Any]) -> None:
    primary = load(ROOT / DEFAULT_REPORTS["all120"][0])
    control = load(ROOT / DEFAULT_REPORTS["all120"][1])
    for report, expected_baseline, expected_current in (
        (primary, OFFICIAL_SHA, CURRENT_SHA),
        (control, OFFICIAL_SHA, OFFICIAL_SHA),
    ):
        require(report["command"] == "nose query <repo> all top=0 --format json", "wrong default performance command")
        require(report["provenance"]["baseline_binary_sha256"] == expected_baseline, "wrong performance baseline")
        require(report["provenance"]["current_binary_sha256"] == expected_current, "wrong performance current")
    baseline = primary["summary"]["aggregate_baseline_median_ms"]
    current = primary["summary"]["aggregate_current_median_ms"]
    control_delta = (
        control["summary"]["aggregate_current_median_ms"]
        - control["summary"]["aggregate_baseline_median_ms"]
    )
    raw = current - baseline
    adjusted = raw - control_delta
    recorded = value["performance"]["all_120_primary"]
    for key, expected in {
        "baseline_median_ms": baseline,
        "current_median_ms": current,
        "raw_delta_ms": raw,
        "raw_delta_pct": raw / baseline * 100,
        "control_delta_ms": control_delta,
        "control_adjusted_delta_ms": adjusted,
        "control_adjusted_delta_pct": adjusted / baseline * 100,
    }.items():
        close(recorded[key], expected, f"all120:{key}")

    all_status = run_checker(DEFAULT_REPORTS["all120"], DEFAULT_REPORTS["r6"], DEFAULT_DRIFT)
    require(len(all_status["primary"]["output"]["authorized_drifts"]) == 26, "authorized drift count changed")
    require(all_status["primary"]["output"]["unexpected_drifts"] == [], "unexpected output drift")
    require(recorded["authorized_output_drifts"] == 26, "recorded drift count changed")
    require(recorded["unexpected_output_drifts"] == 0, "recorded unexpected drift changed")

    final_status = run_checker(DEFAULT_REPORTS["r21"], DEFAULT_REPORTS["r40"], DEFAULT_DRIFT)
    require(signal_names(final_status) == FINAL_DEFAULT_SIGNALS, "final default signals changed")
    final_aggregate = aggregate(final_status)
    final = value["performance"]["final_focused"]
    for key, source_key in {
        "aggregate_baseline_ms": "baseline_ms",
        "aggregate_current_ms": "current_ms",
        "aggregate_control_adjusted_delta_ms": "adjusted_delta_ms",
        "aggregate_control_adjusted_delta_pct": "adjusted_delta_pct",
    }.items():
        close(final[key], final_aggregate[source_key], f"final:{key}")
    require(final["material_stage_signals"] == FINAL_DEFAULT_SIGNALS, "recorded default signals changed")
    require(final["gate_passed"] is False, "default performance failure hidden")

    semantic_status = run_checker(
        SEMANTIC_REPORTS["primary"], SEMANTIC_REPORTS["focused"], SEMANTIC_DRIFT
    )
    require(signal_names(semantic_status) == SEMANTIC_SIGNALS, "semantic signals changed")
    semantic_aggregate = aggregate(semantic_status)
    semantic = value["performance"]["semantic_smoke"]
    for key, source_key in {
        "aggregate_baseline_ms": "baseline_ms",
        "aggregate_current_ms": "current_ms",
        "aggregate_control_adjusted_delta_ms": "adjusted_delta_ms",
        "aggregate_control_adjusted_delta_pct": "adjusted_delta_pct",
    }.items():
        close(semantic[key], semantic_aggregate[source_key], f"semantic:{key}")
    require(semantic["material_stage_signals"] == SEMANTIC_SIGNALS, "recorded semantic signals changed")
    scaling = load(ROOT / "bench/recall_loss/issue-846-ruby-scaling-2026-07-14.v1.json")
    close(
        semantic["ruby_scaling_growth_exponent"],
        scaling["evaluation"]["growth_exponent"],
        "Ruby scaling",
    )
    require(semantic["gate_passed"] is False, "semantic performance failure hidden")


def validate_gates(value: dict[str, Any]) -> None:
    gates = value.get("gates")
    require(isinstance(gates, list) and len(gates) == 10, "wrong gate set")
    by_id = {gate["id"]: gate for gate in gates}
    require(len(by_id) == 10, "duplicate gate")
    expected_failures = {
        "dev-precision",
        "heldout-precision",
        "language-floor",
        "performance",
    }
    actual_failures = {gate_id for gate_id, gate in by_id.items() if not gate["passed"]}
    require(actual_failures == expected_failures, "gate verdicts changed")
    require(by_id["dev-precision"]["shortfall"] == "74 hits", "dev shortfall changed")
    require(by_id["heldout-precision"]["shortfall"] == "35 hits", "heldout shortfall changed")
    outcome = value.get("outcome", {})
    require(outcome.get("status") == "no-go", "closeout must be no-go")
    require(outcome.get("all_tracker_gates_passed") is False, "tracker failure hidden")
    require(outcome.get("product_frozen_unchanged") is True, "product mutation hidden")
    require(outcome.get("follow_up_issues") == [891, 892], "follow-up issues changed")


def validate_value(value: dict[str, Any]) -> None:
    require(value.get("schema") == "nose.default_head_closeout.v1", "wrong schema")
    require(value.get("issue") == 846 and value.get("tracker") == 838, "wrong issue binding")
    product = value.get("product", {})
    require(product.get("source_commit") == CURRENT_SOURCE, "wrong product source")
    require(product.get("binary_sha256") == CURRENT_SHA, "wrong product binary")
    require(product.get("product_changed_after_heldout_reveal") is False, "post-reveal mutation")
    require(value.get("published_baseline", {}).get("binary_sha256") == OFFICIAL_SHA, "wrong published baseline")
    checked_evidence(value)
    residual.validate_payload(residual.read_json(residual.DEFAULT_ARTIFACT))
    fresh.validate(ROOT / value["evidence"]["fresh_repository_audit"]["path"])
    validate_quality(value)
    validate_soundness(value)
    validate_determinism(value)
    validate_performance(value)
    validate_gates(value)


def validate(path: Path) -> None:
    if path.resolve() == DEFAULT.resolve():
        expected, name = SIDECAR.read_text(encoding="utf-8").strip().split()
        require(name == DEFAULT.name, "closeout sidecar filename changed")
        require(sha256(DEFAULT) == expected, "closeout sidecar hash changed")
    validate_value(load(path))


def self_test() -> None:
    original = load(DEFAULT)
    validate_value(original)
    mutations: list[tuple[dict[str, Any], Any]] = []
    changed = copy.deepcopy(original)
    changed["outcome"]["status"] = "go"
    mutations.append((changed, validate_gates))
    changed = copy.deepcopy(original)
    changed["gates"][-2]["passed"] = True
    mutations.append((changed, validate_gates))
    changed = copy.deepcopy(original)
    changed["quality"]["heldout"]["precision_at_10"]["hits"] += 1
    mutations.append((changed, validate_quality))
    changed = copy.deepcopy(original)
    changed["performance"]["final_focused"]["material_stage_signals"].pop()
    mutations.append((changed, validate_performance))
    for index, (mutation, validator) in enumerate(mutations, 1):
        try:
            validator(mutation)
        except ValueError:
            continue
        raise AssertionError(f"self-test mutation {index} was accepted")


def main() -> None:
    parser = argparse.ArgumentParser()
    parser.add_argument("path", nargs="?", type=Path, default=DEFAULT)
    parser.add_argument("--self-test", action="store_true")
    args = parser.parse_args()
    if args.self_test:
        self_test()
        print("default-head closeout self-test passed")
    else:
        validate(args.path)
        print(f"validated {args.path}")


if __name__ == "__main__":
    main()
