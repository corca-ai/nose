#!/usr/bin/env python3
"""Fail no-behavior-change query-regression artifacts on product drift."""

from __future__ import annotations

import argparse
import json
import tempfile
from pathlib import Path
from typing import Any


class CheckFailed(Exception):
    pass


def load_json(path: Path) -> dict[str, Any]:
    try:
        value = json.loads(path.read_text(encoding="utf-8"))
    except OSError as err:
        raise CheckFailed(f"{path}: read failed: {err}") from err
    except json.JSONDecodeError as err:
        raise CheckFailed(f"{path}: invalid JSON: {err}") from err
    if not isinstance(value, dict):
        raise CheckFailed(f"{path}: top-level JSON value must be an object")
    return value


def require_summary(report: dict[str, Any], label: str) -> dict[str, Any]:
    summary = report.get("summary")
    if not isinstance(summary, dict):
        raise CheckFailed(f"{label}: missing object `summary`")
    by_repo = summary.get("by_repo")
    if not isinstance(by_repo, dict):
        raise CheckFailed(f"{label}: missing object `summary.by_repo`")
    return summary


def require_provenance(report: dict[str, Any], label: str) -> dict[str, Any]:
    provenance = report.get("provenance")
    if not isinstance(provenance, dict):
        raise CheckFailed(f"{label}: missing object `provenance`")
    return provenance


def validate_same_binary_control(
    report: dict[str, Any], same_binary_control: dict[str, Any]
) -> None:
    if report.get("command") != same_binary_control.get("command"):
        raise CheckFailed("same-binary control command does not match report command")
    if report.get("repos") != same_binary_control.get("repos"):
        raise CheckFailed("same-binary control repo set does not match report repo set")

    control_provenance = require_provenance(same_binary_control, "same-binary control")
    baseline_sha = control_provenance.get("baseline_binary_sha256")
    current_sha = control_provenance.get("current_binary_sha256")
    if not isinstance(baseline_sha, str) or not isinstance(current_sha, str):
        raise CheckFailed(
            "same-binary control missing baseline/current binary sha256 provenance"
        )
    if baseline_sha != current_sha:
        raise CheckFailed(
            "same-binary control must compare one binary with itself; "
            f"got {baseline_sha} vs {current_sha}"
        )
    report_provenance = require_provenance(report, "report")
    report_baseline_sha = report_provenance.get("baseline_binary_sha256")
    report_current_sha = report_provenance.get("current_binary_sha256")
    if not isinstance(report_baseline_sha, str) or not isinstance(report_current_sha, str):
        raise CheckFailed("report missing baseline/current binary sha256 provenance")
    if baseline_sha not in {report_baseline_sha, report_current_sha}:
        raise CheckFailed(
            "same-binary control binary sha256 must match the report baseline or current binary"
        )


def value_list(row: dict[str, Any], key: str) -> list[Any]:
    value = row.get(key)
    if not isinstance(value, list):
        raise CheckFailed(f"summary row missing list `{key}`")
    return value


def numeric(summary: dict[str, Any], key: str, label: str) -> float:
    value = summary.get(key)
    if isinstance(value, bool) or not isinstance(value, (int, float)):
        raise CheckFailed(f"{label}: missing numeric `summary.{key}`")
    return float(value)


def output_drift_repos(summary: dict[str, Any]) -> list[dict[str, Any]]:
    drifts: list[dict[str, Any]] = []
    for repo, rows in sorted(summary["by_repo"].items()):
        if not isinstance(rows, dict):
            raise CheckFailed(f"{repo}: summary row must be an object")
        baseline = rows.get("baseline")
        current = rows.get("current")
        if not isinstance(baseline, dict) or not isinstance(current, dict):
            raise CheckFailed(f"{repo}: missing baseline/current summary rows")
        changed = {
            key: {
                "baseline": value_list(baseline, key),
                "current": value_list(current, key),
            }
            for key in ["hashes", "bytes", "families"]
            if value_list(baseline, key) != value_list(current, key)
        }
        if changed:
            drifts.append({"repo": repo, "changed": changed})
    return drifts


def check_report(
    report: dict[str, Any],
    *,
    same_binary_control: dict[str, Any] | None = None,
    max_runtime_delta_pct: float,
) -> dict[str, Any]:
    summary = require_summary(report, "report")
    drifts = output_drift_repos(summary)

    aggregate_delta = numeric(summary, "aggregate_delta_pct", "report")
    control_delta = 0.0
    if same_binary_control is not None:
        validate_same_binary_control(report, same_binary_control)
        control_summary = require_summary(same_binary_control, "same-binary control")
        control_drifts = output_drift_repos(control_summary)
        if control_drifts:
            raise CheckFailed(
                "same-binary control has product output drift: "
                + ", ".join(row["repo"] for row in control_drifts)
            )
        control_delta = numeric(control_summary, "aggregate_delta_pct", "same-binary control")

    adjusted_delta = aggregate_delta - control_delta
    runtime_regressed = adjusted_delta > max_runtime_delta_pct
    status = {
        "output_drift_repos": drifts,
        "output_drift_repo_count": len(drifts),
        "aggregate_delta_pct": aggregate_delta,
        "same_binary_control_delta_pct": control_delta,
        "adjusted_delta_pct": adjusted_delta,
        "max_runtime_delta_pct": max_runtime_delta_pct,
        "runtime_status": "regression" if runtime_regressed else "within-threshold",
    }

    failures: list[str] = []
    if drifts:
        failures.append(
            "product output drift in " + ", ".join(row["repo"] for row in drifts)
        )
    if runtime_regressed:
        failures.append(
            f"runtime delta {adjusted_delta:.2f}% exceeds {max_runtime_delta_pct:.2f}%"
        )
    if failures:
        raise CheckFailed("; ".join(failures))
    return status


def sample_report(*, hash_current: str = "h", delta: float = 2.0) -> dict[str, Any]:
    return {
        "command": "nose query <repo> all top=0 --mode semantic --format json",
        "repos": ["repo-a"],
        "provenance": {
            "baseline_binary_sha256": "baseline",
            "current_binary_sha256": "current",
        },
        "summary": {
            "aggregate_baseline_median_ms": 100.0,
            "aggregate_current_median_ms": 100.0 + delta,
            "aggregate_delta_pct": delta,
            "by_repo": {
                "repo-a": {
                    "baseline": {
                        "bytes": [123],
                        "families": [2],
                        "hashes": ["h"],
                        "median_ms": 100.0,
                    },
                    "current": {
                        "bytes": [123],
                        "families": [2],
                        "hashes": [hash_current],
                        "median_ms": 100.0 + delta,
                    },
                }
            },
        }
    }


def sample_control(
    *, delta: float = 2.0, repos: list[str] | None = None, sha: str = "current"
) -> dict[str, Any]:
    report = sample_report(delta=delta)
    report["repos"] = repos or ["repo-a"]
    report["provenance"] = {
        "baseline_binary_sha256": sha,
        "current_binary_sha256": sha,
    }
    return report


def run_self_test() -> None:
    check_report(sample_report(), max_runtime_delta_pct=5.0)
    check_report(
        sample_report(delta=7.0),
        same_binary_control=sample_control(delta=3.0),
        max_runtime_delta_pct=5.0,
    )
    for report, expected in [
        (sample_report(hash_current="changed"), "product output drift"),
        (sample_report(delta=8.0), "runtime delta"),
    ]:
        try:
            check_report(report, max_runtime_delta_pct=5.0)
        except CheckFailed as err:
            assert expected in str(err), str(err)
        else:
            raise AssertionError(f"expected failure containing {expected!r}")
    for control, expected in [
        (sample_report(delta=3.0), "must compare one binary with itself"),
        (sample_control(delta=3.0, repos=["repo-b"]), "repo set does not match"),
        (
            sample_control(delta=3.0, sha="unrelated"),
            "must match the report baseline or current binary",
        ),
    ]:
        try:
            check_report(
                sample_report(delta=7.0),
                same_binary_control=control,
                max_runtime_delta_pct=5.0,
            )
        except CheckFailed as err:
            assert expected in str(err), str(err)
        else:
            raise AssertionError(f"expected control failure containing {expected!r}")

    bool_delta = sample_report()
    bool_delta["summary"]["aggregate_delta_pct"] = True
    try:
        check_report(bool_delta, max_runtime_delta_pct=5.0)
    except CheckFailed as err:
        assert "missing numeric `summary.aggregate_delta_pct`" in str(err), str(err)
    else:
        raise AssertionError("expected boolean aggregate delta to fail")

    with tempfile.TemporaryDirectory(prefix="nose-query-check-") as tmp:
        path = Path(tmp) / "report.json"
        path.write_text(json.dumps(sample_report()), encoding="utf-8")
        loaded = load_json(path)
        assert check_report(loaded, max_runtime_delta_pct=5.0)["output_drift_repo_count"] == 0
    print("query regression checker self-test passed")


def parse_args() -> argparse.Namespace:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("report", nargs="?", type=Path)
    parser.add_argument("--same-binary-control", type=Path)
    parser.add_argument("--max-runtime-delta-pct", type=float, default=5.0)
    parser.add_argument("--self-test", action="store_true")
    return parser.parse_args()


def main() -> int:
    args = parse_args()
    if args.self_test:
        run_self_test()
        return 0
    if args.report is None:
        raise SystemExit("report path is required unless --self-test is used")

    try:
        status = check_report(
            load_json(args.report),
            same_binary_control=(
                load_json(args.same_binary_control) if args.same_binary_control else None
            ),
            max_runtime_delta_pct=args.max_runtime_delta_pct,
        )
    except CheckFailed as err:
        raise SystemExit(f"query regression check failed: {err}") from err
    print(json.dumps(status, indent=2, sort_keys=True))
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
