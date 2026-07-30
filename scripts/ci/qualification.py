#!/usr/bin/env python3
"""Verify the stable aggregate result for routed repository qualification."""

from __future__ import annotations

import argparse
import json
import os
import sys
from pathlib import Path
from typing import Any

import change_routing


ROOT = Path(__file__).resolve().parents[2]
DEFAULT_WORKFLOW = ROOT / ".github/workflows/ci.yml"
SCHEMA = "nose.ci-qualification-receipt.v1"
KNOWN_RESULTS = {"success", "failure", "cancelled", "skipped"}


def aggregate_results(
    *,
    mode: str,
    event: str,
    enforced_jobs: list[Any],
    needs: dict[str, Any],
    workflow: Path = DEFAULT_WORKFLOW,
) -> dict[str, Any]:
    errors: list[str] = []
    if not isinstance(needs, dict):
        raise change_routing.RoutingError("needs evidence must be an object")
    malformed_jobs = any(
        not isinstance(job, str) or not job for job in enforced_jobs
    )
    valid_jobs = [
        job for job in enforced_jobs if isinstance(job, str) and job
    ]
    if malformed_jobs or len(valid_jobs) != len(set(valid_jobs)):
        errors.append("enforced job list is malformed or contains duplicates")
    enforced = set(valid_jobs)
    missing_controls = change_routing.ALWAYS_REQUIRED_JOBS - enforced
    if missing_controls:
        errors.append(
            f"always-required jobs are absent: {sorted(missing_controls)}"
        )
    if mode == "report-only":
        expected = (
            change_routing.eligible_quality_jobs(event, workflow)
            | change_routing.ALWAYS_REQUIRED_JOBS
        )
        if enforced != expected:
            errors.append(
                "report-only enforced job set drifted: "
                f"missing={sorted(expected - enforced)}, "
                f"extra={sorted(enforced - expected)}"
            )
    elif mode != "enforce":
        errors.append(f"unknown routing mode: {mode!r}")

    observed: dict[str, str | None] = {}
    for job, evidence in needs.items():
        result = evidence.get("result") if isinstance(evidence, dict) else None
        observed[job] = result
        if result not in KNOWN_RESULTS:
            errors.append(f"job {job} has invalid/absent result: {result!r}")
    missing = enforced - set(needs)
    if missing:
        errors.append(f"required selected jobs are absent: {sorted(missing)}")
    for job in sorted(enforced & set(needs)):
        if observed[job] != "success":
            errors.append(f"required selected job {job} concluded {observed[job]!r}")
    return {
        "schema": SCHEMA,
        "mode": mode,
        "event": event,
        "conclusion": "success" if not errors else "failure",
        "enforced_jobs": sorted(enforced),
        "observed_results": dict(sorted(observed.items())),
        "errors": errors,
    }


def summary(receipt: dict[str, Any]) -> str:
    errors = "\n".join(f"- {error}" for error in receipt["errors"]) or "- none"
    return (
        "## Repository qualification\n\n"
        f"- Conclusion: `{receipt['conclusion']}`\n"
        f"- Routing mode: `{receipt['mode']}`\n"
        f"- Enforced jobs: `{len(receipt['enforced_jobs'])}`\n"
        "- Errors:\n"
        f"{errors}\n"
    )


def self_test() -> None:
    eligible = change_routing.eligible_quality_jobs("pull_request")
    enforced = sorted(eligible | change_routing.ALWAYS_REQUIRED_JOBS)
    passing_needs = {job: {"result": "success"} for job in enforced}
    passing_needs["workspace-tests-protected"] = {"result": "skipped"}
    passed = aggregate_results(
        mode="report-only",
        event="pull_request",
        enforced_jobs=enforced,
        needs=passing_needs,
    )
    assert passed["conclusion"] == "success"
    selected = next(iter(eligible))
    for label, mutation in (
        ("failure", {**passing_needs, selected: {"result": "failure"}}),
        (
            "missing",
            {job: value for job, value in passing_needs.items() if job != "route"},
        ),
        ("selected skip", {**passing_needs, selected: {"result": "skipped"}}),
    ):
        failed = aggregate_results(
            mode="report-only",
            event="pull_request",
            enforced_jobs=enforced,
            needs=mutation,
        )
        assert failed["conclusion"] == "failure", label
    malformed = aggregate_results(
        mode="report-only",
        event="pull_request",
        enforced_jobs=[*enforced, ["nested-list"]],
        needs=passing_needs,
    )
    assert malformed["conclusion"] == "failure"
    assert "enforced job list is malformed or contains duplicates" in malformed["errors"]
    print("repository qualification aggregate self-test passed")


def parse_args() -> argparse.Namespace:
    parser = argparse.ArgumentParser()
    parser.add_argument("--workflow", type=Path, default=DEFAULT_WORKFLOW)
    parser.add_argument("--self-test", action="store_true")
    parser.add_argument("--event")
    parser.add_argument("--mode")
    parser.add_argument("--enforced-jobs")
    parser.add_argument("--needs-env", default="NEEDS_JSON")
    parser.add_argument("--output", type=Path)
    parser.add_argument("--summary", type=Path)
    return parser.parse_args()


def main() -> int:
    args = parse_args()
    if args.self_test:
        self_test()
        if args.event is None:
            return 0
    required = {
        "--event": args.event,
        "--mode": args.mode,
        "--enforced-jobs": args.enforced_jobs,
        "--output": args.output,
        "--summary": args.summary,
    }
    missing = [name for name, value in required.items() if value is None]
    if missing:
        print(f"qualification error: missing {', '.join(missing)}", file=sys.stderr)
        return 2

    parse_errors: list[str] = []
    try:
        enforced_jobs = json.loads(args.enforced_jobs)
    except json.JSONDecodeError as exc:
        enforced_jobs = []
        parse_errors.append(f"cannot parse enforced jobs: {exc}")
    if not isinstance(enforced_jobs, list):
        enforced_jobs = []
        parse_errors.append("enforced jobs output is not an array")
    try:
        needs = json.loads(os.environ.get(args.needs_env, ""))
    except json.JSONDecodeError as exc:
        needs = {}
        parse_errors.append(f"cannot parse needs evidence: {exc}")
    try:
        receipt = aggregate_results(
            mode=args.mode,
            event=args.event,
            enforced_jobs=enforced_jobs,
            needs=needs,
            workflow=args.workflow,
        )
    except (OSError, change_routing.RoutingError) as exc:
        receipt = {
            "schema": SCHEMA,
            "mode": args.mode,
            "event": args.event,
            "conclusion": "failure",
            "enforced_jobs": enforced_jobs,
            "observed_results": {},
            "errors": [str(exc)],
        }
    receipt["errors"] = parse_errors + receipt["errors"]
    if parse_errors:
        receipt["conclusion"] = "failure"
    args.output.parent.mkdir(parents=True, exist_ok=True)
    args.output.write_text(json.dumps(receipt, indent=2, sort_keys=True) + "\n")
    args.summary.parent.mkdir(parents=True, exist_ok=True)
    args.summary.write_text(summary(receipt))
    return 0 if receipt["conclusion"] == "success" else 1


if __name__ == "__main__":
    raise SystemExit(main())
