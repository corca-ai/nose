#!/usr/bin/env python3
"""Freeze and validate the integrated v0.20.0 release evidence."""

from __future__ import annotations

import argparse
import hashlib
import json
import math
import subprocess
from pathlib import Path
from typing import Any


ROOT = Path(__file__).resolve().parents[1]
RELEASE = ROOT / "bench/release/0.20.0"
QUERY_RECEIPT = RELEASE / "query-performance-943.v1.json"
CACHE_RECEIPT = ROOT / "bench/cache/release-0.20.0-cache-943.v1.json"
WATCH_REPORT = ROOT / "bench/cache/release-0.20.0-watch-943.v1.json"
SOUNDNESS_BINDING = ROOT / "bench/soundness/0.20.0/release-binding-943.v1.json"
SOUNDNESS_NIGHTLY = ROOT / "bench/soundness/0.20.0/release-nightly-943.v1.json"
SOUNDNESS_DEEP = ROOT / "bench/soundness/0.20.0/release-deep-943.v1.json"
SOUNDNESS_GATE = ROOT / "bench/soundness/0.20.0/release-gate-943.v1.json"

CANDIDATE_SOURCE = "a544d03b6801871dbcd90bcb370825942d6851c8"
CANDIDATE_CRATES = "6d38b79884a44d1fe38a47cec19ca4d9a2ef7570"
CANDIDATE_BINARY = "ad1f3fa3695168083be85ed81b199e059c91c45dcff769e1c9d99e6597081328"
CANDIDATE_CODE = "4dc1b4e18bf11777f1319d4ca89df35c582d1619ea56cd3f713f8f987f9613ac"
CANDIDATE_ARCHIVE = "7187ac4d634ab64519827f7bd92604fb7d9c0a5f9f161ae0337904b39d6011c4"
LOCKFILE = "02e8398ae87566d48ad4d04a168a521bb2cba38485d3339705941a022952bfa0"
BASELINE_SOURCE = "0985e6963c58d5a97e523bc532b88aa5e34f2ef9"
BASELINE_BINARY = "0f73ea544da06cc175e01c31c383cc4cb86daf3d37a49d74de61dea3724fe0f3"
BASELINE_CODE = "e55d0e989993ff1d1d6b4e933dbd3f5ade38203368b8321d3a7842799a95aca6"
BASELINE_ARCHIVE = "097c7e766e9ab756a32cec715897067d1360e145074715168a653962be409981"
CORPUS_MANIFEST = "87b3defc02c87e53f5ce20d10b68afdbc7190a6db5d5bfdb6b655b305bbc7ba8"
PRUNE_MANIFEST = "c22f34d3ab4da9b89b5938140bbfdf7664178b3b7b57e5ea3937ba0bb47c2980"
CORPUS_STATE = "b28d7245aa34d7e0320d0c80ef988803ba6acb65b63eaf1bae6d7ac840b168e0"
BASE_WORKLOAD = "ed76a6a2b5b2551dfd61f627998c6db50e0be70fb479067ccabf7b42f97b2ad6"

QUERY_INPUTS = {
    "base": {
        "repos": 17,
        "primary": "target/issue-943-final5-query-base-primary-five-s5.json",
        "control": "target/issue-943-final5-query-base-control-five-s5.json",
        "focused": None,
        "focused_control": None,
        "status": "target/issue-943-final5-query-base-status.json",
        "drift": "bench/recall_loss/release-0.20.0-base-expected-drift.v1.json",
    },
    "default": {
        "repos": 120,
        "primary": "target/issue-943-final5-query-default-primary-five-s5.json",
        "control": "target/issue-943-final5-query-default-control-five-s5.json",
        "focused": "target/issue-943-final5-query-default-focused-six-s5.json",
        "focused_control": "target/issue-943-final5-query-default-focused-control-six-s5.json",
        "status": "target/issue-943-final5-query-default-status.json",
        "drift": "bench/recall_loss/release-0.20.0-default-expected-drift.v1.json",
    },
    "semantic": {
        "repos": 120,
        "primary": "target/issue-943-final5-query-semantic-primary-five-s5.json",
        "control": "target/issue-943-final5-query-semantic-control-five-s5.json",
        "focused": "target/issue-943-final5-query-semantic-focused-six-s5.json",
        "focused_control": "target/issue-943-final5-query-semantic-focused-control-six-s5.json",
        "status": "target/issue-943-final5-query-semantic-status.json",
        "drift": "bench/recall_loss/release-0.20.0-semantic-expected-drift.v1.json",
    },
    "near-no-pack": {
        "repos": 120,
        "primary": "target/issue-943-final5-query-near-no-pack-primary-five-s5.json",
        "control": "target/issue-943-final5-query-near-no-pack-control-five-s5.json",
        "focused": "target/issue-943-final5-query-near-no-pack-focused-six-s5.json",
        "focused_control": "target/issue-943-final5-query-near-no-pack-focused-control-six-s5.json",
        "status": "target/issue-943-final5-query-near-no-pack-status.json",
        "drift": "bench/recall_loss/release-0.20.0-near-no-pack-expected-drift.v1.json",
    },
}

CACHE_INPUTS = {
    "sympy-leaf": "target/issue-943-final5-cache-sympy-leaf-30.json",
    "sympy-noop": "target/issue-943-final5-cache-sympy-noop-30.json",
    "prettier-noop": "target/issue-943-final5-cache-prettier-noop-30.json",
    "netty-noop": "target/issue-943-final5-cache-netty-noop-30.json",
    "fastlane-noop": "target/issue-943-final5-cache-fastlane-noop-30.json",
    "mutation-matrix": "target/issue-943-final5-cache-mutation-matrix-30-receipt.json",
}
WATCH_INPUT = "target/issue-943-final5-watch-session-30.json"


def load(path: Path) -> dict[str, Any]:
    value = json.loads(path.read_text())
    if not isinstance(value, dict):
        raise ValueError(f"{path}: expected a JSON object")
    return value


def sha256(path: Path) -> str:
    return hashlib.sha256(path.read_bytes()).hexdigest()


def require_sha256(value: Any, label: str) -> str:
    if not isinstance(value, str) or len(value) != 64:
        raise ValueError(f"{label}: expected SHA-256")
    int(value, 16)
    return value


def artifact(path: str) -> dict[str, Any]:
    full = ROOT / path
    return {
        "path": path,
        "sha256": sha256(full),
        "bytes": full.stat().st_size,
        "retention": "local-target" if path.startswith("target/") else "checked",
    }


def candidate_identity() -> dict[str, Any]:
    return {
        "source_sha": CANDIDATE_SOURCE,
        "crates_tree": CANDIDATE_CRATES,
        "lockfile_sha256": LOCKFILE,
        "binary_sha256": CANDIDATE_BINARY,
        "binary_code_sha256": CANDIDATE_CODE,
        "archive_sha256": CANDIDATE_ARCHIVE,
        "target": "aarch64-apple-darwin",
        "rustc": "rustc 1.96.0 (ac68faa20 2026-05-25)",
        "xcode": "26.3 (17C528)",
        "sdk": "macOS 26.2",
    }


def baseline_identity() -> dict[str, Any]:
    return {
        "source_sha": BASELINE_SOURCE,
        "binary_sha256": BASELINE_BINARY,
        "binary_code_sha256": BASELINE_CODE,
        "archive_sha256": BASELINE_ARCHIVE,
        "target": "aarch64-apple-darwin",
        "published_and_checksum_verified": True,
    }


def corpus_identity() -> dict[str, Any]:
    return {
        "manifest": "bench/goldens/corpus.json",
        "manifest_sha256": CORPUS_MANIFEST,
        "prune_manifest": "bench/labels/prune_manifest.json",
        "prune_manifest_sha256": PRUNE_MANIFEST,
        "expected_state": "bench/default_head_closeout_corpus.v1.json",
        "expected_state_sha256": CORPUS_STATE,
        "repositories": 120,
        "base_workload": "bench/base_view_release_workload.v1.json",
        "base_workload_sha256": BASE_WORKLOAD,
        "base_repositories": 17,
    }


def unresolved(decision: dict[str, Any] | None) -> dict[str, int]:
    runtime = (decision or {}).get("runtime", {})
    output = (decision or {}).get("output", {})
    return {
        "triggered": len(runtime.get("triggered", [])),
        "inconclusive": len(runtime.get("inconclusive", [])),
        "unexpected_output_drifts": len(output.get("unexpected_drifts", [])),
        "unused_output_declarations": len(output.get("unused_declarations", [])),
    }


def freeze_query() -> None:
    workloads = []
    for workload_id, config in QUERY_INPUTS.items():
        primary = load(ROOT / config["primary"])
        status = load(ROOT / config["status"])
        provenance = primary["provenance"]
        if (
            primary.get("schema") != "nose.query_regression_harness.v3"
            or status.get("status") != "pass"
            or len(primary.get("repos", [])) != config["repos"]
            or provenance.get("current_source_sha") != CANDIDATE_SOURCE
            or provenance.get("current_binary_sha256") != CANDIDATE_BINARY
            or provenance.get("baseline_source_sha") != BASELINE_SOURCE
            or provenance.get("baseline_binary_sha256") != BASELINE_BINARY
        ):
            raise ValueError(f"{workload_id}: query evidence identity or status failed")
        final_decision = status.get("focused") if config["focused"] else status.get("primary")
        final_unresolved = unresolved(final_decision)
        if any(final_unresolved.values()):
            raise ValueError(f"{workload_id}: final query decision is unresolved")
        artifacts = {
            key: artifact(config[key])
            for key in ("primary", "control", "focused", "focused_control", "status")
            if config[key]
        }
        artifacts["expected_drift"] = artifact(config["drift"])
        workloads.append(
            {
                "id": workload_id,
                "repositories": config["repos"],
                "measurement": primary["measurement"],
                "aggregate": {
                    key: primary["summary"][key]
                    for key in (
                        "aggregate_baseline_median_ms",
                        "aggregate_current_median_ms",
                        "aggregate_delta_ms",
                        "aggregate_delta_pct",
                    )
                },
                "primary_signals": unresolved(status.get("primary")),
                "focused_repositories": status.get("focused_repos", []),
                "final_unresolved": final_unresolved,
                "status": "pass",
                "artifacts": artifacts,
            }
        )
    receipt = {
        "schema": "nose.release_query_performance/v1",
        "issue": 943,
        "release": "0.20.0",
        "candidate": candidate_identity(),
        "baseline": baseline_identity(),
        "corpus": corpus_identity(),
        "policy": {
            "runtime": "order-aware-v3",
            "material_only_if_delta_pct_gt": 5.0,
            "material_only_if_delta_ms_gt": 5.0,
            "primary_blocks": 5,
            "focused_blocks": 6,
            "single_focused_rerun": True,
        },
        "workloads": workloads,
        "status": "pass",
    }
    QUERY_RECEIPT.parent.mkdir(parents=True, exist_ok=True)
    QUERY_RECEIPT.write_text(json.dumps(receipt, indent=2, sort_keys=True) + "\n")


def phase_delta(report: dict[str, Any], phase: str, quantile: str) -> dict[str, Any]:
    workload = report["workload"]["id"]
    baseline = report["summary"]["official"][workload][phase]["elapsed_ms"][quantile]
    current = report["summary"]["candidate"][workload][phase]["elapsed_ms"][quantile]
    delta_ms = current - baseline
    delta_pct = delta_ms / baseline * 100.0
    return {
        "baseline_ms": baseline,
        "candidate_ms": current,
        "delta_ms": delta_ms,
        "delta_pct": delta_pct,
        "material_regression": delta_ms > 5.0 and delta_pct > 5.0,
    }


def freeze_cache() -> None:
    paired = {}
    for label in ("sympy-leaf", "sympy-noop"):
        report = load(ROOT / CACHE_INPUTS[label])
        if (
            report.get("status") != "passed"
            or report.get("measurement", {}).get("replays") != 30
            or report.get("equivalence") != {"candidate": True, "official": True}
            or report.get("provenance", {}).get("candidate", {}).get("binary_revision")
            != CANDIDATE_SOURCE
            or report["provenance"]["candidate"].get("binary_sha256") != CANDIDATE_BINARY
            or report["provenance"]["official"].get("binary_sha256") != BASELINE_BINARY
        ):
            raise ValueError(f"{label}: paired cache evidence failed")
        phases = {
            phase: {
                quantile: phase_delta(report, phase, quantile)
                for quantile in ("p50", "p95")
            }
            for phase in ("clean-after", "empty-store-after", "history-after")
        }
        if any(
            row["material_regression"]
            for phase in phases.values()
            for row in phase.values()
        ):
            raise ValueError(f"{label}: material cache regression")
        paired[label] = {"phases": phases, "artifact": artifact(CACHE_INPUTS[label])}

    leaf = load(ROOT / CACHE_INPUTS["sympy-leaf"])
    history_candidate = leaf["summary"]["candidate"]["sympy"]["history-after"]
    history_official = leaf["summary"]["official"]["sympy"]["history-after"]
    source_bytes = sum(
        path.stat().st_size for path in (ROOT / "bench/repos/sympy").rglob("*.py")
    )
    resources = {
        "source_bytes": source_bytes,
        "store_bytes_p95": history_candidate["store_bytes"]["p95"],
        "store_to_source_ratio": history_candidate["store_bytes"]["p95"] / source_bytes,
        "store_ratio_vs_official": (
            history_candidate["store_bytes"]["p95"] / history_official["store_bytes"]["p95"]
        ),
        "warm_leaf_rss_ratio_p50": (
            history_candidate["peak_rss_bytes"]["p50"]
            / history_official["peak_rss_bytes"]["p50"]
        ),
        "warm_leaf_rss_ratio_p95": (
            history_candidate["peak_rss_bytes"]["p95"]
            / history_official["peak_rss_bytes"]["p95"]
        ),
    }
    if (
        resources["store_to_source_ratio"] > 6.0
        or resources["store_ratio_vs_official"] > 0.5
        or resources["warm_leaf_rss_ratio_p50"] > 0.6
        or resources["warm_leaf_rss_ratio_p95"] > 0.6
    ):
        raise ValueError("cache resource gate failed")

    real_workloads = []
    for label in ("prettier-noop", "netty-noop", "fastlane-noop"):
        report = load(ROOT / CACHE_INPUTS[label])
        equivalence = report.get("equivalence", {})
        if (
            report.get("status") != "passed"
            or report.get("measurement", {}).get("replays") != 30
            or equivalence.get("after_clean_equals_empty_store") is not True
            or equivalence.get("after_clean_equals_history_store") is not True
            or report.get("provenance", {}).get("binary_revision") != CANDIDATE_SOURCE
            or report.get("provenance", {}).get("binary_sha256") != CANDIDATE_BINARY
        ):
            raise ValueError(f"{label}: real cache equivalence failed")
        real_workloads.append({"id": label, "status": "pass", "artifact": artifact(CACHE_INPUTS[label])})

    matrix = load(ROOT / CACHE_INPUTS["mutation-matrix"])
    if (
        matrix.get("status") != "passed"
        or matrix.get("measurement", {}).get("replays") != 30
        or matrix.get("raw_report", {}).get("rows") != 2100
        or len(matrix.get("workload", {}).get("ids", [])) != 14
        or any(value is not True for value in matrix.get("equivalence", {}).values())
        or matrix.get("provenance", {}).get("binary_revision") != CANDIDATE_SOURCE
        or matrix.get("provenance", {}).get("binary_sha256") != CANDIDATE_BINARY
    ):
        raise ValueError("cache mutation matrix failed")
    receipt = {
        "schema": "nose.release_cache_evidence/v1",
        "issue": 943,
        "release": "0.20.0",
        "candidate": candidate_identity(),
        "baseline": baseline_identity(),
        "policy": {"max_delta_pct": 5.0, "min_delta_ms": 5.0, "p95": "nearest-rank"},
        "paired": paired,
        "resources": resources,
        "real_workloads": real_workloads,
        "mutation_matrix": {
            "status": "pass",
            "workloads": matrix["workload"]["ids"],
            "raw_report": matrix["raw_report"],
            "artifact": artifact(CACHE_INPUTS["mutation-matrix"]),
        },
        "status": "pass",
    }
    CACHE_RECEIPT.parent.mkdir(parents=True, exist_ok=True)
    CACHE_RECEIPT.write_text(json.dumps(receipt, indent=2, sort_keys=True) + "\n")


def freeze_watch() -> None:
    source = ROOT / WATCH_INPUT
    report = load(source)
    provenance = report.get("provenance", {})
    if (
        report.get("status") != "pass"
        or provenance.get("candidate_revision") != CANDIDATE_SOURCE
        or provenance.get("candidate_binary_sha256") != CANDIDATE_BINARY
        or provenance.get("one_shot_evidence", {}).get("exact_candidate_binding") is not True
    ):
        raise ValueError("watch report is not bound to the candidate")
    WATCH_REPORT.write_bytes(source.read_bytes())


def validate_query() -> None:
    receipt = load(QUERY_RECEIPT)
    if (
        receipt.get("schema") != "nose.release_query_performance/v1"
        or receipt.get("issue") != 943
        or receipt.get("status") != "pass"
        or receipt.get("candidate") != candidate_identity()
        or receipt.get("baseline") != baseline_identity()
        or receipt.get("corpus") != corpus_identity()
        or [row.get("id") for row in receipt.get("workloads", [])] != list(QUERY_INPUTS)
    ):
        raise ValueError("query release receipt identity failed")
    for row, expected in zip(receipt["workloads"], QUERY_INPUTS.values(), strict=True):
        if (
            row.get("repositories") != expected["repos"]
            or row.get("status") != "pass"
            or any(row.get("final_unresolved", {}).values())
            or row.get("measurement", {}).get("iterations") != 5
            or row.get("measurement", {}).get("samples_per_observation") != 5
        ):
            raise ValueError(f"query release workload failed: {row.get('id')}")
        for evidence in row.get("artifacts", {}).values():
            require_sha256(evidence.get("sha256"), "query artifact")


def validate_cache() -> None:
    receipt = load(CACHE_RECEIPT)
    if (
        receipt.get("schema") != "nose.release_cache_evidence/v1"
        or receipt.get("issue") != 943
        or receipt.get("status") != "pass"
        or receipt.get("candidate") != candidate_identity()
        or receipt.get("baseline") != baseline_identity()
    ):
        raise ValueError("cache release receipt identity failed")
    for comparison in receipt.get("paired", {}).values():
        for phase in comparison.get("phases", {}).values():
            if any(row.get("material_regression") for row in phase.values()):
                raise ValueError("cache receipt retains a material regression")
    resources = receipt.get("resources", {})
    if (
        resources.get("store_to_source_ratio", math.inf) > 6.0
        or resources.get("store_ratio_vs_official", math.inf) > 0.5
        or resources.get("warm_leaf_rss_ratio_p50", math.inf) > 0.6
        or resources.get("warm_leaf_rss_ratio_p95", math.inf) > 0.6
        or len(receipt.get("real_workloads", [])) != 3
        or len(receipt.get("mutation_matrix", {}).get("workloads", [])) != 14
    ):
        raise ValueError("cache release resource or workload gate failed")


def validate_soundness_release() -> None:
    nightly = load(SOUNDNESS_NIGHTLY)
    deep = load(SOUNDNESS_DEEP)
    gate = load(SOUNDNESS_GATE)
    totals = nightly.get("totals", {})
    if (
        nightly.get("schema") != "nose-corpus-verify-merged/v1"
        or nightly.get("complete") is not True
        or nightly.get("source_commit") != CANDIDATE_SOURCE
        or nightly.get("nose", {}).get("sha256") != CANDIDATE_BINARY
        or totals.get("repositories") != 120
        or totals.get("failed_repositories") != 0
        or totals.get("false_merges") != 0
        or totals.get("canon_changes") != 0
        or nightly.get("advisory", {}).get("blocking") is not False
    ):
        raise ValueError("Soundness Lab nightly release evidence failed")
    checks = deep.get("checks", {})
    if (
        deep.get("schema") != "nose-soundness-deep-evidence/v1"
        or deep.get("source_commit") != CANDIDATE_SOURCE
        or set(checks)
        != {
            "source_runtime_calibration",
            "metamorphic_equivalence",
            "multi_seed_falsification",
        }
        or any(value is not True for value in checks.values())
    ):
        raise ValueError("Soundness Lab deep release evidence failed")
    hard = gate.get("hard_gates", {})
    coverage = gate.get("risk_weighted_coverage", {})
    if (
        gate.get("schema") != "nose-soundness-release-gate/v1"
        or gate.get("source_commit") != CANDIDATE_SOURCE
        or gate.get("gate_passed") is not True
        or hard.get("pinned_corpus") != totals
        or hard.get("deep_campaign") != checks
        or hard.get("registered_claims") is not True
        or hard.get("guarded_tier_a_cells") is not True
        or hard.get("attributed_exclusions") is not True
        or hard.get("blind_attack", {}).get("gate_passed") is not True
        or hard.get("blind_attack", {}).get("false_merges") != 0
        or hard.get("focused_falsification", {}).get("false_merges") != 0
        or coverage.get("macro_ppm", 0) < coverage.get("release_target_ppm", math.inf)
        or gate.get("performance") != load(QUERY_RECEIPT)
        or gate.get("official_baseline", {}).get("source_commit") != BASELINE_SOURCE
        or gate.get("official_baseline", {}).get("published_binary_sha256")
        != BASELINE_BINARY
    ):
        raise ValueError("Soundness Lab integrated release gate failed")


def run_checked(command: list[str]) -> None:
    completed = subprocess.run(command, cwd=ROOT, text=True, capture_output=True)
    if completed.returncode:
        raise ValueError(completed.stderr.strip() or completed.stdout.strip())


def validate_all(*, include_soundness: bool = True) -> None:
    validate_query()
    validate_cache()
    run_checked(
        [
            "python3",
            "scripts/watch-session-benchmark.py",
            "--validate-report",
            str(WATCH_REPORT.relative_to(ROOT)),
        ]
    )
    if include_soundness:
        if not SOUNDNESS_BINDING.is_file():
            raise ValueError("final soundness binding is missing")
        run_checked(
            [
                "python3",
                "scripts/check-soundness-scorecard.py",
                "--release-commit",
                CANDIDATE_SOURCE,
            ]
        )
        validate_soundness_release()


def self_test() -> None:
    assert 5.1 > 5.0 and 5.1 > 5.0
    assert not (5.1 > 5.0 and 4.9 > 5.0)
    assert not (4.9 > 5.0 and 5.1 > 5.0)
    require_sha256("0" * 64, "self-test")
    print("release 0.20.0 evidence checker self-test passed")


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--freeze", action="store_true")
    parser.add_argument("--self-test", action="store_true")
    args = parser.parse_args()
    if args.self_test:
        self_test()
        return 0
    if args.freeze:
        freeze_query()
        freeze_cache()
        freeze_watch()
        validate_all(include_soundness=False)
        print("froze query, cache, and watch release evidence")
        return 0
    validate_all()
    print("release 0.20.0 integrated evidence validated")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
