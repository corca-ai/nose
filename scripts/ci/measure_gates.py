#!/usr/bin/env python3
"""Measure named CI gates without duplicating their executable policy."""

from __future__ import annotations

import argparse
import hashlib
import json
import platform
import re
import subprocess
import sys
import tempfile
import time
from datetime import datetime, timezone
from pathlib import Path
from typing import Any

import gate_registry


ROOT = Path(__file__).resolve().parents[2]
CHECK_SCRIPT = ROOT / "scripts/check-ci-local.sh"
SCHEMA = "nose.ci-gate-timings.v1"


def run_text(command: list[str]) -> str:
    return subprocess.run(
        command,
        cwd=ROOT,
        check=True,
        stdout=subprocess.PIPE,
        stderr=subprocess.PIPE,
        text=True,
    ).stdout.strip()


def worktree_fingerprint() -> str:
    status = run_text(
        ["git", "status", "--porcelain=v1", "--untracked-files=all"]
    )
    return hashlib.sha256(status.encode()).hexdigest()


def environment() -> dict[str, str]:
    return {
        "platform": platform.platform(),
        "machine": platform.machine(),
        "python": platform.python_version(),
        "rustc": run_text(["rustc", "--version"]),
        "cargo": run_text(["cargo", "--version"]),
    }


def empty_receipt() -> dict[str, Any]:
    return {
        "schema": SCHEMA,
        "generated_at": datetime.now(timezone.utc).isoformat(),
        "source_commit": run_text(["git", "rev-parse", "HEAD"]),
        "environment": environment(),
        "runs": [],
    }


def load_receipt(path: Path, append: bool) -> dict[str, Any]:
    if not append or not path.exists():
        return empty_receipt()
    try:
        receipt = json.loads(path.read_text())
    except (OSError, json.JSONDecodeError) as exc:
        raise ValueError(f"cannot load timing receipt {path}: {exc}") from exc
    if receipt.get("schema") != SCHEMA or not isinstance(receipt.get("runs"), list):
        raise ValueError(f"{path} is not a {SCHEMA} receipt")
    receipt["generated_at"] = datetime.now(timezone.utc).isoformat()
    return receipt


def write_receipt(path: Path, receipt: dict[str, Any]) -> None:
    path.parent.mkdir(parents=True, exist_ok=True)
    path.write_text(json.dumps(receipt, indent=2) + "\n")


def validate_receipt(receipt: dict[str, Any], gates: list[dict[str, Any]]) -> None:
    if receipt.get("schema") != SCHEMA:
        raise ValueError(f"timing receipt schema must be {SCHEMA}")
    if not isinstance(receipt.get("environment"), dict):
        raise ValueError("timing receipt must record its environment")
    source_commit = receipt.get("source_commit")
    if not isinstance(source_commit, str) or re.fullmatch(r"[0-9a-f]{40}", source_commit) is None:
        raise ValueError("timing receipt source_commit must be a full Git commit")
    runs = receipt.get("runs")
    if not isinstance(runs, list):
        raise ValueError("timing receipt runs must be an array")

    required_profiles = {
        ("clean-tree", "fast"),
        ("clean-tree", "full"),
        ("no-change", "fast"),
    }
    observed_profiles: set[tuple[str, str]] = set()
    observed_gates: set[str] = set()
    for run in runs:
        if not isinstance(run, dict):
            raise ValueError("timing run must be an object")
        profile = run.get("profile")
        mode = run.get("mode")
        if not isinstance(profile, str) or mode not in {"fast", "full"}:
            raise ValueError("timing run needs a profile and fast/full mode")
        observed_profiles.add((profile, mode))
        expected_names = [
            row["name"] for row in gate_registry.plan_rows(gates, mode)
        ]
        results = run.get("gates")
        if not isinstance(results, list):
            raise ValueError(f"timing run {profile}/{mode} gates must be an array")
        actual_names = [result.get("name") for result in results]
        if actual_names != expected_names:
            raise ValueError(
                f"timing run {profile}/{mode} does not cover its complete plan"
            )
        if run.get("completed_gates") != len(expected_names):
            raise ValueError(f"timing run {profile}/{mode} is incomplete")
        if run.get("planned_gates") != len(expected_names):
            raise ValueError(f"timing run {profile}/{mode} planned count drifted")
        for result in results:
            if result.get("exit_code") != 0:
                raise ValueError(f"timed gate {result.get('name')} did not pass")
            if result.get("worktree_changed") is not False:
                raise ValueError(f"timed gate {result.get('name')} changed the worktree")
            seconds = result.get("seconds")
            if not isinstance(seconds, (int, float)) or seconds < 0:
                raise ValueError(f"timed gate {result.get('name')} has invalid time")
            observed_gates.add(result["name"])

    missing_profiles = required_profiles - observed_profiles
    if missing_profiles:
        raise ValueError(f"timing receipt misses profiles: {sorted(missing_profiles)}")
    expected_gates = {gate["name"] for gate in gates}
    if observed_gates != expected_gates:
        raise ValueError(
            "timing receipt gate coverage mismatch: "
            f"missing={sorted(expected_gates - observed_gates)}, "
            f"unknown={sorted(observed_gates - expected_gates)}"
        )


def measure_mode(
    gates: list[dict[str, Any]],
    *,
    mode: str,
    profile: str,
) -> tuple[dict[str, Any], int]:
    plan = gate_registry.plan_rows(gates, mode)
    started_at = datetime.now(timezone.utc).isoformat()
    mode_start = time.perf_counter()
    gate_results: list[dict[str, Any]] = []
    result_code = 0

    for row in plan:
        command = [
            str(CHECK_SCRIPT),
            "--gate",
            row["name"],
            *row["args"],
        ]
        before = worktree_fingerprint()
        start = time.perf_counter()
        completed = subprocess.run(command, cwd=ROOT, check=False)
        elapsed = time.perf_counter() - start
        after = worktree_fingerprint()
        changed = before != after
        status = completed.returncode
        if changed and status == 0:
            status = 1
            print(
                f"gate {row['name']} changed the worktree during measurement",
                file=sys.stderr,
            )
        gate_results.append(
            {
                "name": row["name"],
                "args": row["args"],
                "seconds": round(elapsed, 3),
                "exit_code": completed.returncode,
                "worktree_changed": changed,
            }
        )
        if status != 0:
            result_code = status
            break

    return (
        {
            "profile": profile,
            "mode": mode,
            "started_at": started_at,
            "seconds": round(time.perf_counter() - mode_start, 3),
            "completed_gates": len(gate_results),
            "planned_gates": len(plan),
            "gates": gate_results,
        },
        result_code,
    )


def self_test() -> None:
    gate = {
        "name": "sample",
        "plans": {
            "fast": {"order": 10, "label": "sample", "args": []},
            "full": {"order": 10, "label": "sample", "args": []},
        },
    }
    gate_result = {
        "name": "sample",
        "args": [],
        "seconds": 1.0,
        "exit_code": 0,
        "worktree_changed": False,
    }
    sample = {
        "schema": SCHEMA,
        "generated_at": "2026-01-01T00:00:00+00:00",
        "source_commit": "a" * 40,
        "environment": {"platform": "test"},
        "runs": [
            {
                "profile": "clean-tree",
                "mode": "fast",
                "seconds": 1.0,
                "completed_gates": 1,
                "planned_gates": 1,
                "gates": [gate_result],
            },
            {
                "profile": "clean-tree",
                "mode": "full",
                "seconds": 1.0,
                "completed_gates": 1,
                "planned_gates": 1,
                "gates": [gate_result],
            },
            {
                "profile": "no-change",
                "mode": "fast",
                "seconds": 1.0,
                "completed_gates": 1,
                "planned_gates": 1,
                "gates": [gate_result],
            },
        ],
    }
    validate_receipt(sample, [gate])
    with tempfile.TemporaryDirectory() as temp_dir:
        receipt_path = Path(temp_dir) / "receipt.json"
        write_receipt(receipt_path, sample)
        loaded = json.loads(receipt_path.read_text())
        assert loaded == sample
        assert loaded["runs"][0]["gates"][0]["worktree_changed"] is False
    drifted = json.loads(json.dumps(sample))
    drifted["runs"][0]["gates"][0]["worktree_changed"] = True
    try:
        validate_receipt(drifted, [gate])
    except ValueError as exc:
        assert "changed the worktree" in str(exc)
    else:
        raise AssertionError("worktree-changing timed gate passed")
    print("CI gate timing self-test passed")


def parse_args() -> argparse.Namespace:
    parser = argparse.ArgumentParser()
    parser.add_argument("--profile", help="measurement profile name")
    parser.add_argument(
        "--mode",
        action="append",
        choices=("fast", "full"),
        dest="modes",
        help="local plan to measure; repeat to measure both",
    )
    parser.add_argument("--output", type=Path)
    parser.add_argument("--append", action="store_true")
    parser.add_argument("--validate", type=Path)
    parser.add_argument("--self-test", action="store_true")
    return parser.parse_args()


def main() -> int:
    args = parse_args()
    if args.self_test:
        self_test()
        return 0
    if args.validate is not None:
        try:
            gates = gate_registry.validate_live_registry(gate_registry.load_registry())
            validate_receipt(json.loads(args.validate.read_text()), gates)
        except (
            OSError,
            json.JSONDecodeError,
            ValueError,
            gate_registry.RegistryError,
        ) as exc:
            print(f"CI gate timing receipt error: {exc}", file=sys.stderr)
            return 1
        print(f"CI gate timing receipt OK: {args.validate}")
        return 0
    if not args.profile or not args.modes or args.output is None:
        print("--profile, --mode, and --output are required", file=sys.stderr)
        return 2

    try:
        gates = gate_registry.validate_live_registry(gate_registry.load_registry())
        receipt = load_receipt(args.output, args.append)
    except (OSError, ValueError, gate_registry.RegistryError) as exc:
        print(f"CI gate measurement error: {exc}", file=sys.stderr)
        return 1

    for mode in args.modes:
        run, result_code = measure_mode(gates, mode=mode, profile=args.profile)
        receipt["runs"].append(run)
        write_receipt(args.output, receipt)
        if result_code != 0:
            return result_code
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
