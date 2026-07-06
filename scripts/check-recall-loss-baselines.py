#!/usr/bin/env python3
"""Fail recall-loss reports on hard gate regressions or unsafe bucket growth."""

from __future__ import annotations

import argparse
import json
from pathlib import Path
from typing import Any

UNATTRIBUTED_STRICT_EXACT_UNSAFE = "unattributed-strict-exact-unsafe"


class CheckFailed(Exception):
    pass


def load_report(path: Path) -> dict[str, Any]:
    try:
        report = json.loads(path.read_text(encoding="utf-8"))
    except OSError as err:
        raise CheckFailed(f"{path}: read failed: {err}") from err
    except json.JSONDecodeError as err:
        raise CheckFailed(f"{path}: invalid JSON: {err}") from err
    if not isinstance(report, dict):
        raise CheckFailed(f"{path}: top-level JSON value must be an object")
    if report.get("schema_version") != 1:
        raise CheckFailed(f"{path}: expected schema_version=1")
    report_kind = report.get("report_kind")
    if report_kind == "recall-loss-diagnostics":
        return report
    if report_kind == "recall-loss-baseline-summary":
        return normalize_baseline_summary(path, report)
    raise CheckFailed(
        f"{path}: expected report_kind=recall-loss-diagnostics or recall-loss-baseline-summary"
    )


def normalize_baseline_summary(path: Path, report: dict[str, Any]) -> dict[str, Any]:
    current = report.get("current")
    if not isinstance(current, dict):
        raise CheckFailed(f"{path}: baseline summary missing object `current`")
    reasons = current.get("admission_rejections_by_reason")
    if not isinstance(reasons, dict):
        raise CheckFailed(
            f"{path}: baseline summary missing object `current.admission_rejections_by_reason`"
        )
    by_reason = []
    for reason, count in sorted(reasons.items()):
        if isinstance(count, bool) or not isinstance(count, int) or count < 0:
            raise CheckFailed(
                f"{path}: current.admission_rejections_by_reason.{reason} must be a non-negative integer"
            )
        by_reason.append({"reason": reason, "count": count})
    return {
        "schema_version": 1,
        "report_kind": "recall-loss-diagnostics",
        "soundness_gate": current.get("soundness_gate"),
        "by_reason": by_reason,
    }


def normalize_report(label: str, report: dict[str, Any]) -> dict[str, Any]:
    if report.get("report_kind") == "recall-loss-baseline-summary":
        return normalize_baseline_summary(Path(label), report)
    return report


def numeric(report: dict[str, Any], section: str, key: str) -> int:
    value = report.get(section, {}).get(key)
    if isinstance(value, bool) or not isinstance(value, int) or value < 0:
        raise CheckFailed(f"missing non-negative integer `{section}.{key}`")
    return value


def reason_count(report: dict[str, Any], reason: str) -> int:
    total = 0
    rows = report.get("by_reason")
    if not isinstance(rows, list):
        raise CheckFailed("missing array `by_reason`")
    for row in rows:
        if not isinstance(row, dict):
            raise CheckFailed("`by_reason` entries must be objects")
        if row.get("reason") == reason:
            count = row.get("count")
            if isinstance(count, bool) or not isinstance(count, int) or count < 0:
                raise CheckFailed(
                    f"`by_reason` count for {reason} must be a non-negative integer"
                )
            total += count
    return total


def check_single_report(report: dict[str, Any], label: str) -> dict[str, int | str]:
    report = normalize_report(label, report)
    false_merges = numeric(report, "soundness_gate", "false_merges")
    canon_violations = numeric(
        report, "soundness_gate", "canon_preservation_violations"
    )
    unsafe = reason_count(report, UNATTRIBUTED_STRICT_EXACT_UNSAFE)
    failures = []
    if false_merges > 0:
        failures.append(f"{label}: false_merges={false_merges}")
    if canon_violations > 0:
        failures.append(f"{label}: canon_preservation_violations={canon_violations}")
    if failures:
        raise CheckFailed("; ".join(failures))
    return {
        "label": label,
        "false_merges": false_merges,
        "canon_preservation_violations": canon_violations,
        "unattributed_strict_exact_unsafe": unsafe,
    }


def check_reports(
    *,
    reports: list[tuple[str, dict[str, Any]]],
    baseline: dict[str, Any] | None = None,
    current: dict[str, Any] | None = None,
) -> dict[str, Any]:
    checked = [check_single_report(report, label) for label, report in reports]
    if baseline is not None or current is not None:
        if baseline is None or current is None:
            raise CheckFailed("--baseline and --current must be provided together")
        before = check_single_report(baseline, "baseline")
        after = check_single_report(current, "current")
        before_unsafe = int(before["unattributed_strict_exact_unsafe"])
        after_unsafe = int(after["unattributed_strict_exact_unsafe"])
        if after_unsafe > before_unsafe:
            raise CheckFailed(
                f"{UNATTRIBUTED_STRICT_EXACT_UNSAFE} grew "
                f"{before_unsafe} -> {after_unsafe}"
            )
        checked.extend([before, after])
    return {"checked_reports": checked}


def sample_report(
    *,
    false_merges: int = 0,
    canon_violations: int = 0,
    unsafe: int = 0,
) -> dict[str, Any]:
    return {
        "schema_version": 1,
        "report_kind": "recall-loss-diagnostics",
        "soundness_gate": {
            "false_merges": false_merges,
            "canon_preservation_violations": canon_violations,
        },
        "by_reason": [
            {"reason": UNATTRIBUTED_STRICT_EXACT_UNSAFE, "count": unsafe},
            {"reason": "receiver-domain-proof-missing", "count": 2},
        ],
    }


def run_self_test() -> None:
    check_reports(reports=[("ok", sample_report())])
    check_reports(
        reports=[
            (
                "compact",
                {
                    "schema_version": 1,
                    "report_kind": "recall-loss-baseline-summary",
                    "current": {
                        "soundness_gate": {
                            "false_merges": 0,
                            "canon_preservation_violations": 0,
                        },
                        "admission_rejections_by_reason": {
                            UNATTRIBUTED_STRICT_EXACT_UNSAFE: 0
                        },
                    },
                },
            )
        ]
    )
    check_reports(
        reports=[],
        baseline=sample_report(unsafe=3),
        current=sample_report(unsafe=2),
    )
    for kwargs, expected in [
        ({"false_merges": 1}, "false_merges=1"),
        ({"canon_violations": 1}, "canon_preservation_violations=1"),
        ({"false_merges": -1}, "missing non-negative integer"),
        ({"unsafe": -1}, "must be a non-negative integer"),
    ]:
        try:
            check_reports(reports=[("bad", sample_report(**kwargs))])
        except CheckFailed as err:
            assert expected in str(err), str(err)
        else:
            raise AssertionError(f"expected failure containing {expected!r}")
    try:
        check_reports(
            reports=[],
            baseline=sample_report(unsafe=1),
            current=sample_report(unsafe=2),
        )
    except CheckFailed as err:
        assert "grew 1 -> 2" in str(err), str(err)
    else:
        raise AssertionError("expected unsafe bucket growth to fail")
    for bad_report, expected in [
        ({**sample_report(), "by_reason": None}, "missing array `by_reason`"),
        (
            {**sample_report(), "by_reason": [{"reason": UNATTRIBUTED_STRICT_EXACT_UNSAFE}]},
            "must be a non-negative integer",
        ),
        (
            {
                "schema_version": 1,
                "report_kind": "recall-loss-baseline-summary",
                "current": {
                    "soundness_gate": {
                        "false_merges": 0,
                        "canon_preservation_violations": 0,
                    },
                    "admission_rejections_by_reason": {
                        UNATTRIBUTED_STRICT_EXACT_UNSAFE: -1
                    },
                },
            },
            "must be a non-negative integer",
        ),
    ]:
        try:
            check_reports(reports=[("malformed", bad_report)])
        except CheckFailed as err:
            assert expected in str(err), str(err)
        else:
            raise AssertionError(f"expected malformed report failure containing {expected!r}")
    print("recall-loss baseline checker self-test passed")


def parse_args() -> argparse.Namespace:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--report", action="append", type=Path, default=[])
    parser.add_argument("--baseline", type=Path)
    parser.add_argument("--current", type=Path)
    parser.add_argument("--self-test", action="store_true")
    return parser.parse_args()


def main() -> int:
    args = parse_args()
    if args.self_test:
        run_self_test()
        return 0
    if not args.report and not (args.baseline and args.current):
        raise SystemExit("--report or --baseline/--current is required unless --self-test is used")
    try:
        status = check_reports(
            reports=[(path.as_posix(), load_report(path)) for path in args.report],
            baseline=load_report(args.baseline) if args.baseline else None,
            current=load_report(args.current) if args.current else None,
        )
    except CheckFailed as err:
        raise SystemExit(f"recall-loss baseline check failed: {err}") from err
    print(json.dumps(status, indent=2, sort_keys=True))
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
