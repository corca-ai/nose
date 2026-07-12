#!/usr/bin/env python3
"""Run timed query triage for two nose binaries and classify repo-level regressions."""

from __future__ import annotations

import argparse
from dataclasses import dataclass
import hashlib
import json
import os
import platform
import re
import shlex
import statistics
import subprocess
import sys
import time
from pathlib import Path
from typing import Any

from binary_identity import binary_identity, run_self_test as run_binary_identity_self_test


DEFAULT_QUERY_ARGS = ("query", "{repo}", "all", "top=0", "--mode", "semantic", "--format", "json")
SCHEMA = "nose.runtime_triage_harness.v1"
TIME_RE = re.compile(r"\[time\]\s+([a-zA-Z0-9_+\-]+)\s+([0-9.]+)ms")
UNIT_SUMMARY_RE = re.compile(
    r"\[unit-summary\]\s+(\S+)\s+(.*?)\s+"
    r"seen=(\d+)\s+kept=(\d+)\s+skipped=(\d+)\s+tokens=(\d+)\s+"
    r"value_atoms=(\d+)\s+total=([0-9.]+)ms\s+pre=([0-9.]+)ms\s+"
    r"safe=([0-9.]+)ms\s+value=([0-9.]+)ms\s+features=([0-9.]+)ms"
)


@dataclass(frozen=True)
class ClassificationPolicy:
    regression_pct: float
    small_absolute_ms: float
    hot_unit_ms: float

    def as_json(self) -> dict[str, float]:
        return {
            "regression_pct": self.regression_pct,
            "small_absolute_ms": self.small_absolute_ms,
            "hot_unit_ms": self.hot_unit_ms,
        }


def git_output(args: list[str]) -> str:
    result = subprocess.run(
        ["git", *args],
        check=False,
        stdout=subprocess.PIPE,
        stderr=subprocess.PIPE,
        text=True,
    )
    if result.returncode != 0:
        return f"<git {' '.join(args)} failed: {result.stderr.strip()}>"
    return result.stdout.strip()


def optional_command_output(args: list[str]) -> str | None:
    try:
        result = subprocess.run(
            args,
            check=False,
            stdout=subprocess.PIPE,
            stderr=subprocess.PIPE,
            text=True,
        )
    except OSError:
        return None
    if result.returncode != 0:
        return None
    output = result.stdout.strip()
    return output or None


def physical_memory_bytes() -> int | None:
    if sys.platform == "darwin":
        raw = optional_command_output(["sysctl", "-n", "hw.memsize"])
        return int(raw) if raw and raw.isdigit() else None
    try:
        return os.sysconf("SC_PAGE_SIZE") * os.sysconf("SC_PHYS_PAGES")
    except (AttributeError, KeyError, OSError, TypeError, ValueError):
        return None


def measurement_environment() -> dict[str, Any]:
    machine_model = None
    if sys.platform == "darwin":
        machine_model = optional_command_output(["sysctl", "-n", "hw.model"])
    return {
        "architecture": platform.machine(),
        "logical_cpu_count": os.cpu_count(),
        "machine_model": machine_model,
        "memory_bytes": physical_memory_bytes(),
        "os": platform.system(),
        "os_release": platform.release(),
        "python_version": platform.python_version(),
        "rustc_version": optional_command_output(["rustc", "--version"]),
    }


def parse_query_args(raw: str) -> tuple[str, ...]:
    if not raw:
        return DEFAULT_QUERY_ARGS
    args = tuple(shlex.split(raw))
    if "{repo}" not in args:
        raise SystemExit("--query-args must contain {repo}")
    return args


def all_repo_names(repos_root: Path) -> list[str]:
    if not repos_root.exists():
        raise SystemExit(f"missing repos root: {repos_root}")
    return sorted(path.name for path in repos_root.iterdir() if path.is_dir())


def selected_repos(args: argparse.Namespace) -> list[tuple[str, Path]]:
    repo_names = list(args.repos)
    if args.all_repos:
        repo_names.extend(all_repo_names(args.repos_root))
    repo_names = sorted(dict.fromkeys(repo_names))
    if not repo_names:
        raise SystemExit("--repo or --all-repos is required")
    repos = [(repo, (args.repos_root / repo).resolve()) for repo in repo_names]
    missing = [path for _, path in repos if not path.exists()]
    if missing:
        raise SystemExit(f"missing repo paths: {', '.join(path.as_posix() for path in missing)}")
    return repos


def command_for(binary: Path, repo: Path, query_args: tuple[str, ...]) -> list[str]:
    return [str(binary), *[repo.as_posix() if arg == "{repo}" else arg for arg in query_args]]


def query_payload(stdout: bytes) -> dict[str, Any]:
    try:
        payload = json.loads(stdout)
    except json.JSONDecodeError:
        return {"families": []}
    if isinstance(payload, dict):
        return payload
    if isinstance(payload, list):
        return {"families": payload}
    return {"families": []}


def parse_timing(stderr: bytes) -> dict[str, Any]:
    text = stderr.decode(errors="replace")
    stages = {match.group(1): float(match.group(2)) for match in TIME_RE.finditer(text)}
    units = []
    for match in UNIT_SUMMARY_RE.finditer(text):
        units.append(
            {
                "kind": match.group(1),
                "file": match.group(2),
                "seen": int(match.group(3)),
                "kept": int(match.group(4)),
                "skipped": int(match.group(5)),
                "tokens": int(match.group(6)),
                "value_atoms": int(match.group(7)),
                "total_ms": float(match.group(8)),
                "pre_ms": float(match.group(9)),
                "safe_ms": float(match.group(10)),
                "value_ms": float(match.group(11)),
                "features_ms": float(match.group(12)),
            }
        )
    units.sort(key=lambda unit: unit["total_ms"], reverse=True)
    return {"stages_ms": stages, "top_units": units[:10]}


def run_once(
    *,
    binary: Path,
    label: str,
    repo_name: str,
    repo_path: Path,
    iteration: int,
    query_args: tuple[str, ...],
    value_graph_timing: bool,
) -> dict[str, Any]:
    command = command_for(binary, repo_path, query_args)
    env = dict(os.environ, NOSE_TIME="1", NOSE_TIME_UNIT_SUMMARY="1")
    if value_graph_timing:
        env["NOSE_TIME_VALUE_GRAPH"] = "1"
    start = time.perf_counter()
    result = subprocess.run(
        command,
        check=False,
        stdout=subprocess.PIPE,
        stderr=subprocess.PIPE,
        env=env,
    )
    elapsed_ms = (time.perf_counter() - start) * 1000.0
    if result.returncode != 0:
        raise SystemExit(
            f"{label} {repo_name} iteration {iteration} failed: "
            f"{result.stderr.decode(errors='replace')}"
        )
    payload = query_payload(result.stdout)
    timing = parse_timing(result.stderr)
    return {
        "bytes": len(result.stdout),
        "elapsed_ms": elapsed_ms,
        "families": len(payload.get("families", [])),
        "iteration": iteration,
        "label": label,
        "repo": repo_name,
        "sha256": hashlib.sha256(result.stdout).hexdigest(),
        "timing": timing,
    }


def warmup(
    *,
    binary: Path,
    label: str,
    repos: list[tuple[str, Path]],
    warmups: int,
    query_args: tuple[str, ...],
) -> None:
    for iteration in range(1, warmups + 1):
        for repo_name, repo_path in repos:
            run_once(
                binary=binary,
                label=label,
                repo_name=repo_name,
                repo_path=repo_path,
                iteration=-iteration,
                query_args=query_args,
                value_graph_timing=False,
            )


def median(values: list[float]) -> float:
    return statistics.median(values) if values else 0.0


def summarize_label(rows: list[dict[str, Any]]) -> dict[str, Any]:
    stage_names = sorted({name for row in rows for name in row["timing"]["stages_ms"]})
    stages = {
        name: median([row["timing"]["stages_ms"].get(name, 0.0) for row in rows])
        for name in stage_names
    }
    target = stages.get("normalize+extract", median([row["elapsed_ms"] for row in rows]))
    representative = min(
        rows,
        key=lambda row: abs(row["timing"]["stages_ms"].get("normalize+extract", row["elapsed_ms"]) - target),
    )
    return {
        "bytes": sorted({row["bytes"] for row in rows}),
        "families": sorted({row["families"] for row in rows}),
        "hashes": sorted({row["sha256"] for row in rows}),
        "median_ms": median([row["elapsed_ms"] for row in rows]),
        "stages_median_ms": stages,
        "representative_top_units": representative["timing"]["top_units"],
    }


def classify_repo(
    baseline: dict[str, Any],
    current: dict[str, Any],
    *,
    policy: ClassificationPolicy,
) -> dict[str, Any]:
    baseline_ms = baseline["median_ms"]
    current_ms = current["median_ms"]
    delta_ms = current_ms - baseline_ms
    delta_pct = (delta_ms / baseline_ms) * 100.0 if baseline_ms else 0.0
    baseline_families = baseline["families"][0] if len(baseline["families"]) == 1 else None
    current_families = current["families"][0] if len(current["families"]) == 1 else None
    family_delta = (
        current_families - baseline_families
        if baseline_families is not None and current_families is not None
        else None
    )
    current_top = (current.get("representative_top_units") or [{}])[0]
    stage_delta = {
        name: current["stages_median_ms"].get(name, 0.0) - baseline["stages_median_ms"].get(name, 0.0)
        for name in sorted(set(baseline["stages_median_ms"]) | set(current["stages_median_ms"]))
    }

    if delta_ms <= 0:
        kind = "not-reproduced"
        reason = "current median is not slower than baseline"
    elif abs(delta_ms) < policy.small_absolute_ms or delta_pct < policy.regression_pct:
        kind = "small-or-noisy"
        reason = "runtime delta is below configured regression thresholds"
    elif family_delta is not None and family_delta > 0:
        kind = "capability-growth"
        reason = "family count increased; measure cost per newly surfaced family before optimizing"
    elif (
        current_top.get("value_ms", 0.0) >= policy.hot_unit_ms
        and stage_delta.get("normalize+extract", 0.0) >= stage_delta.get("lower", 0.0)
    ):
        kind = "no-family-growth-value-hot-path"
        reason = "family count did not grow and representative unit value time is high"
    elif stage_delta.get("lower", 0.0) > stage_delta.get("normalize+extract", 0.0):
        kind = "no-family-growth-lower-or-frontend"
        reason = "family count did not grow and lower/front-end stage delta dominates"
    else:
        kind = "no-family-growth-mixed-hot-path"
        reason = "family count did not grow; timing is split across stages"

    return {
        "kind": kind,
        "reason": reason,
        "delta_ms": delta_ms,
        "delta_pct": delta_pct,
        "family_delta": family_delta,
        "stage_delta_ms": stage_delta,
    }


def summarize(
    runs: list[dict[str, Any]],
    repos: list[str],
    *,
    policy: ClassificationPolicy,
) -> dict[str, Any]:
    by_repo: dict[str, dict[str, Any]] = {}
    for repo in repos:
        baseline = summarize_label([row for row in runs if row["repo"] == repo and row["label"] == "baseline"])
        current = summarize_label([row for row in runs if row["repo"] == repo and row["label"] == "current"])
        by_repo[repo] = {
            "baseline": baseline,
            "current": current,
            "classification": classify_repo(
                baseline,
                current,
                policy=policy,
            ),
            "hashes_identical": baseline["hashes"] == current["hashes"],
        }
    aggregate_baseline = sum(by_repo[repo]["baseline"]["median_ms"] for repo in repos)
    aggregate_current = sum(by_repo[repo]["current"]["median_ms"] for repo in repos)
    aggregate_delta_pct = (
        ((aggregate_current - aggregate_baseline) / aggregate_baseline) * 100.0
        if aggregate_baseline
        else 0.0
    )
    return {
        "aggregate_baseline_median_ms": aggregate_baseline,
        "aggregate_current_median_ms": aggregate_current,
        "aggregate_delta_pct": aggregate_delta_pct,
        "by_repo": by_repo,
    }


def run_self_test() -> None:
    run_binary_identity_self_test()

    stderr = (
        b"  [time] lower          10.5ms\n"
        b"  [time] normalize+extract    22.0ms   (total    22.0ms)\n"
        b"  [unit-summary] Function bench/repos/x/a.rs seen=2 kept=2 skipped=0 "
        b"tokens=100 value_atoms=20 total=15.0ms pre=1.0ms safe=2.0ms value=11.0ms features=1.0ms\n"
    )
    parsed = parse_timing(stderr)
    assert parsed["stages_ms"]["lower"] == 10.5
    assert parsed["top_units"][0]["value_ms"] == 11.0
    rows = [
        {
            "repo": "a",
            "label": "baseline",
            "elapsed_ms": 100.0,
            "bytes": 1,
            "families": 2,
            "sha256": "x",
            "timing": {"stages_ms": {"lower": 10.0, "normalize+extract": 20.0}, "top_units": []},
        },
        {
            "repo": "a",
            "label": "current",
            "elapsed_ms": 150.0,
            "bytes": 2,
            "families": 3,
            "sha256": "y",
            "timing": {"stages_ms": {"lower": 15.0, "normalize+extract": 30.0}, "top_units": []},
        },
        {
            "repo": "b",
            "label": "baseline",
            "elapsed_ms": 100.0,
            "bytes": 1,
            "families": 2,
            "sha256": "x",
            "timing": {
                "stages_ms": {"lower": 10.0, "normalize+extract": 20.0},
                "top_units": [{"value_ms": 5.0}],
            },
        },
        {
            "repo": "b",
            "label": "current",
            "elapsed_ms": 160.0,
            "bytes": 1,
            "families": 2,
            "sha256": "z",
            "timing": {
                "stages_ms": {"lower": 12.0, "normalize+extract": 80.0},
                "top_units": [{"value_ms": 40.0}],
            },
        },
    ]
    policy = ClassificationPolicy(
        regression_pct=20.0,
        small_absolute_ms=10.0,
        hot_unit_ms=20.0,
    )
    summary = summarize(
        rows,
        ["a", "b"],
        policy=policy,
    )
    assert summary["by_repo"]["a"]["classification"]["kind"] == "capability-growth"
    assert summary["by_repo"]["b"]["classification"]["kind"] == "no-family-growth-value-hot-path"
    assert parse_query_args("query '{repo}' all top=0 --mode semantic --format json")[1] == "{repo}"
    environment = measurement_environment()
    assert environment["architecture"]
    assert environment["logical_cpu_count"]
    assert environment["os"]
    assert environment["python_version"]
    print("runtime triage harness self-test passed")


def parse_args() -> argparse.Namespace:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--baseline-binary", type=Path)
    parser.add_argument("--current-binary", type=Path)
    parser.add_argument("--baseline-source-ref", default="origin/main")
    parser.add_argument("--current-source-ref", default="HEAD")
    parser.add_argument("--baseline-source-sha")
    parser.add_argument("--current-source-sha")
    parser.add_argument("--repos-root", type=Path, default=Path("bench/repos"))
    parser.add_argument("--repo", action="append", dest="repos", default=[])
    parser.add_argument("--all-repos", action="store_true")
    parser.add_argument("--iterations", type=int, default=5)
    parser.add_argument("--warmups", type=int, default=1)
    parser.add_argument("--query-args", default=" ".join(DEFAULT_QUERY_ARGS))
    parser.add_argument("--output", type=Path)
    parser.add_argument("--regression-pct", type=float, default=20.0)
    parser.add_argument("--small-absolute-ms", type=float, default=25.0)
    parser.add_argument("--hot-unit-ms", type=float, default=20.0)
    parser.add_argument("--value-graph-timing", action="store_true")
    parser.add_argument("--self-test", action="store_true")
    return parser.parse_args()


def main() -> int:
    args = parse_args()
    if args.self_test:
        run_self_test()
        return 0
    if not args.baseline_binary or not args.current_binary or not args.output:
        raise SystemExit("--baseline-binary, --current-binary, and --output are required")
    if args.iterations <= 0 or args.warmups < 0:
        raise SystemExit("--iterations must be positive and --warmups must be non-negative")

    baseline_binary = args.baseline_binary.resolve()
    current_binary = args.current_binary.resolve()
    repos = selected_repos(args)
    query_args = parse_query_args(args.query_args)
    policy = ClassificationPolicy(
        regression_pct=args.regression_pct,
        small_absolute_ms=args.small_absolute_ms,
        hot_unit_ms=args.hot_unit_ms,
    )
    working_tree_status_before_measurement = git_output(["status", "--short"])

    warmup(binary=baseline_binary, label="baseline", repos=repos, warmups=args.warmups, query_args=query_args)
    warmup(binary=current_binary, label="current", repos=repos, warmups=args.warmups, query_args=query_args)

    runs: list[dict[str, Any]] = []
    for iteration in range(1, args.iterations + 1):
        order = ("baseline", "current") if iteration % 2 else ("current", "baseline")
        binaries = {"baseline": baseline_binary, "current": current_binary}
        for label in order:
            for repo_name, repo_path in repos:
                runs.append(
                    run_once(
                        binary=binaries[label],
                        label=label,
                        repo_name=repo_name,
                        repo_path=repo_path,
                        iteration=iteration,
                        query_args=query_args,
                        value_graph_timing=args.value_graph_timing,
                    )
                )

    repo_names = [repo for repo, _ in repos]
    baseline_identity = binary_identity(baseline_binary)
    current_identity = binary_identity(current_binary)
    output = {
        "schema": SCHEMA,
        "command": "nose " + " ".join(query_args).replace("{repo}", "<repo>"),
        "classification_policy": policy.as_json(),
        "environment": measurement_environment(),
        "provenance": {
            "baseline_binary": baseline_binary.as_posix(),
            "baseline_binary_code_sha256": baseline_identity.code_sha256,
            "baseline_binary_code_sha256_algorithm": baseline_identity.code_sha256_algorithm,
            "baseline_binary_sha256": baseline_identity.file_sha256,
            "baseline_source_ref": args.baseline_source_ref,
            "baseline_source_sha": args.baseline_source_sha or git_output(["rev-parse", args.baseline_source_ref]),
            "current_binary": current_binary.as_posix(),
            "current_binary_code_sha256": current_identity.code_sha256,
            "current_binary_code_sha256_algorithm": current_identity.code_sha256_algorithm,
            "current_binary_sha256": current_identity.file_sha256,
            "current_source_ref": args.current_source_ref,
            "current_source_sha": args.current_source_sha or git_output(["rev-parse", args.current_source_ref]),
            "harness": "scripts/runtime-triage-harness.py",
            "harness_command": shlex.join(["python3", *sys.argv]),
            "working_tree_status_before_measurement": working_tree_status_before_measurement,
        },
        "repos": repo_names,
        "runs": runs,
        "summary": summarize(
            runs,
            repo_names,
            policy=policy,
        ),
    }
    args.output.parent.mkdir(parents=True, exist_ok=True)
    args.output.write_text(json.dumps(output, indent=2, sort_keys=True) + "\n", encoding="utf-8")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
