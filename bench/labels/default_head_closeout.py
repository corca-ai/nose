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
OFFICIAL_CODE_SHA = "e55d0e989993ff1d1d6b4e933dbd3f5ade38203368b8321d3a7842799a95aca6"
CURRENT_SHA = "f7fcda30aa63662f95000af7029eaf028c71ef074a18ba5e1e2048fe27c47fd0"
CURRENT_CODE_SHA = "03cc5827cdadc225478a34266de78805c6e495810f90e8642f2ae2807b3a4f5a"
CURRENT_SOURCE = "cdab416706c32ea94bf808ec7ebb36781e483e65"
CURRENT_SOURCE_TREE = "0f42757629a79ce7be0cd0cd5cd90c2d5b78c3da"
BASELINE_SOURCE = "0985e6963c58d5a97e523bc532b88aa5e34f2ef9"
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
    "r9": (
        "bench/recall_loss/issue-846-official-v0.19.0-default-focused-r9-2026-07-14.v1.json",
        "bench/recall_loss/issue-846-official-self-control-default-focused-r9-2026-07-14.v1.json",
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


def git_output(*args: str) -> str:
    result = subprocess.run(
        ["git", *args], cwd=ROOT, capture_output=True, text=True
    )
    require(
        result.returncode == 0,
        f"git {' '.join(args)} failed: {result.stderr.strip()}",
    )
    return result.stdout.strip()


def require_commit(commit: str) -> None:
    require(git_output("cat-file", "-t", commit) == "commit", f"not a commit: {commit}")


def require_source_commit(commit: str) -> None:
    require_commit(commit)
    result = subprocess.run(
        ["git", "merge-base", "--is-ancestor", commit, "HEAD"], cwd=ROOT
    )
    require(result.returncode == 0, f"source commit is not an ancestor of HEAD: {commit}")


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


def validate_measurement_provenance(value: dict[str, Any]) -> None:
    record = value["evidence"].get("measurement_provenance")
    require(isinstance(record, dict), "missing measurement provenance")
    manifest = load(ROOT / record["path"])
    validate_measurement_manifest(manifest)


def validate_measurement_manifest(manifest: dict[str, Any]) -> None:
    require(
        manifest.get("schema") == "nose.default_head_measurement_provenance.v1",
        "wrong measurement-provenance schema",
    )
    require(
        manifest.get("issue") == 846 and manifest.get("tracker") == 838,
        "wrong measurement-provenance issue binding",
    )

    require_source_commit(CURRENT_SOURCE)
    require_source_commit(BASELINE_SOURCE)
    require(
        git_output("rev-parse", f"{CURRENT_SOURCE}:crates") == CURRENT_SOURCE_TREE,
        "frozen product source tree changed",
    )
    product = manifest.get("product", {})
    require(product.get("source_commit") == CURRENT_SOURCE, "wrong measurement product source")
    require(product.get("binary_sha256") == CURRENT_SHA, "wrong measurement product binary")
    require(product.get("binary_code_sha256") == CURRENT_CODE_SHA, "wrong measurement product code")
    require(
        product.get("binary_code_sha256_algorithm")
        == "sha256/mach-o-zero-uuid-signature-v1",
        "wrong measurement product code-hash algorithm",
    )
    require(
        product.get("working_tree_status_before_measurement") == "",
        "measurement product tree was dirty",
    )

    inputs = manifest.get("inputs", {})
    source_tree = inputs.get("source_tree", {})
    require(
        source_tree == {"path": "crates", "git_tree_sha1": CURRENT_SOURCE_TREE},
        "wrong soundness input tree",
    )
    corpus = inputs.get("corpus", {})
    expected_corpus = {
        "manifest": "bench/goldens/corpus.json",
        "manifest_sha256": sha256(ROOT / "bench/goldens/corpus.json"),
        "prune_manifest": "bench/labels/prune_manifest.json",
        "prune_manifest_sha256": sha256(ROOT / "bench/labels/prune_manifest.json"),
        "state_contract": "bench/default_head_closeout_corpus.v1.json",
        "state_contract_sha256": sha256(ROOT / "bench/default_head_closeout_corpus.v1.json"),
        "subset_digest_after_prune": load(
            ROOT / "bench/default_head_closeout_corpus.v1.json"
        )["subset_digest_after_prune"]["hex"],
    }
    require(corpus == expected_corpus, "measurement corpus contract changed")

    measurements = manifest.get("measurements", {})
    require(
        set(measurements)
        == {"soundness", "heldout_thread_determinism", "ruby_redefinition_scaling"},
        "wrong bound measurement set",
    )
    for name, measurement in measurements.items():
        artifact = ROOT / measurement["artifact"]
        require(artifact.is_file(), f"missing bound measurement: {name}")
        require(
            sha256(artifact) == measurement["artifact_sha256"],
            f"bound measurement changed: {name}",
        )
        require(measurement.get("source_commit") == CURRENT_SOURCE, f"wrong source: {name}")
        require(measurement.get("binary_sha256") == CURRENT_SHA, f"wrong binary: {name}")
        require(
            measurement.get("working_tree_status_before_measurement") == "",
            f"dirty measurement: {name}",
        )

    soundness = measurements["soundness"]
    require(soundness["source_tree_sha1"] == CURRENT_SOURCE_TREE, "wrong soundness tree")
    require(
        soundness["command"]
        == (
            "target/release/nose verify crates --max-violations 0 "
            "--recall-loss-report target/issue-846-closeout/recall-loss.crates.v1.json"
        ),
        "wrong soundness command",
    )

    heldout = measurements["heldout_thread_determinism"]
    require(heldout["split"] == "heldout", "wrong determinism split")
    require(heldout["repositories"] == 54, "wrong heldout repository count")
    require(heldout["thread_counts"] == [1, 4], "wrong determinism thread counts")
    require(
        heldout["corpus_state_contract_sha256"]
        == expected_corpus["state_contract_sha256"],
        "wrong determinism corpus state",
    )
    require(
        heldout["subset_digest_after_prune"]
        == expected_corpus["subset_digest_after_prune"],
        "wrong determinism corpus digest",
    )
    require(
        heldout["command"]
        == (
            "for each heldout corpus repository: RAYON_NUM_THREADS={1,4} "
            "target/release/nose query bench/repos/<repo> all top=0 --format json"
        ),
        "wrong heldout determinism command",
    )

    scaling = measurements["ruby_redefinition_scaling"]
    require(
        scaling["command"]
        == (
            "python3 scripts/ruby-redefinition-scaling.py --binary target/release/nose "
            "--output target/issue-846-semantic-smoke-artifacts/ruby-scaling.json"
        ),
        "wrong Ruby scaling command",
    )


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
    require(dev["current"]["binary_sha256"] == CURRENT_SHA, "wrong dev determinism binary")
    dev_source = dev["current"]["commit"]
    require_commit(dev_source)
    require(
        dev["corpus"]["manifest_sha256"] == sha256(ROOT / "bench/goldens/corpus.json"),
        "wrong dev determinism corpus",
    )
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
    corpus = load(ROOT / "bench/goldens/corpus.json")
    expected_heldout = sorted(
        row["id"] for row in corpus["repositories"] if row["split"] == "heldout"
    )
    observed_heldout = sorted(row[0] for row in heldout_rows)
    require(observed_heldout == expected_heldout, "heldout determinism corpus changed")
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


def validate_query_report_provenance(
    report_path: str, expected_current_binary: str, expected_current_source: str
) -> None:
    report = load(ROOT / report_path)
    provenance = report.get("provenance", {})
    require(
        provenance.get("baseline_binary_sha256") == OFFICIAL_SHA,
        f"wrong performance baseline binary: {report_path}",
    )
    require(
        provenance.get("baseline_binary_code_sha256") == OFFICIAL_CODE_SHA,
        f"wrong performance baseline code: {report_path}",
    )
    require(
        provenance.get("baseline_source_sha") == BASELINE_SOURCE,
        f"wrong performance baseline source: {report_path}",
    )
    require(
        provenance.get("current_binary_sha256") == expected_current_binary,
        f"wrong performance current binary: {report_path}",
    )
    expected_current_code = (
        CURRENT_CODE_SHA if expected_current_binary == CURRENT_SHA else OFFICIAL_CODE_SHA
    )
    require(
        provenance.get("current_binary_code_sha256") == expected_current_code,
        f"wrong performance current code: {report_path}",
    )
    require(
        provenance.get("baseline_binary_code_sha256_algorithm")
        == "sha256/mach-o-zero-uuid-signature-v1"
        and provenance.get("current_binary_code_sha256_algorithm")
        == "sha256/mach-o-zero-uuid-signature-v1",
        f"wrong performance code-hash algorithm: {report_path}",
    )
    require(
        provenance.get("current_source_sha") == expected_current_source,
        f"wrong performance current source: {report_path}",
    )
    require(
        provenance.get("working_tree_status_before_measurement") == "",
        f"performance run was dirty: {report_path}",
    )
    command = provenance.get("harness_command", "")
    require(
        f"--baseline-source-sha {BASELINE_SOURCE}" in command,
        f"baseline source missing from command: {report_path}",
    )
    require(
        f"--current-source-sha {expected_current_source}" in command,
        f"current source missing from command: {report_path}",
    )
    require_source_commit(provenance["baseline_source_sha"])
    require_source_commit(provenance["current_source_sha"])


def validate_performance(value: dict[str, Any]) -> None:
    for primary_path, control_path in [
        *DEFAULT_REPORTS.values(),
        *SEMANTIC_REPORTS.values(),
    ]:
        validate_query_report_provenance(primary_path, CURRENT_SHA, CURRENT_SOURCE)
        validate_query_report_provenance(control_path, OFFICIAL_SHA, BASELINE_SOURCE)

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
    validate_measurement_provenance(value)
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
    provenance = load(
        ROOT / original["evidence"]["measurement_provenance"]["path"]
    )
    changed = copy.deepcopy(provenance)
    changed["product"]["source_commit"] = BASELINE_SOURCE
    mutations.append((changed, validate_measurement_manifest))
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
