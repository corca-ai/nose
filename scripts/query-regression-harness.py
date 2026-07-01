#!/usr/bin/env python3
"""Run alternating product-query regressions for two nose binaries."""

from __future__ import annotations

import argparse
import hashlib
import json
import statistics
import subprocess
import sys
import time
from pathlib import Path
from typing import Any


DEFAULT_QUERY_ARGS = ("query", "{repo}", "all", "top=0", "--mode", "semantic", "--format", "json")


def sha256_file(path: Path) -> str:
    digest = hashlib.sha256()
    with path.open("rb") as handle:
        for chunk in iter(lambda: handle.read(1024 * 1024), b""):
            digest.update(chunk)
    return digest.hexdigest()


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


def parse_query_args(raw: str) -> tuple[str, ...]:
    if not raw:
        return DEFAULT_QUERY_ARGS
    args = tuple(part for part in raw.split(" ") if part)
    if "{repo}" not in args:
        raise SystemExit("--query-args must contain {repo}")
    return args


def command_for(binary: Path, repo: Path, query_args: tuple[str, ...]) -> list[str]:
    return [str(binary), *[repo.as_posix() if arg == "{repo}" else arg for arg in query_args]]


def family_count(stdout: bytes) -> int:
    try:
        payload = json.loads(stdout)
    except json.JSONDecodeError:
        return 0
    if isinstance(payload, dict) and isinstance(payload.get("families"), list):
        return len(payload["families"])
    if isinstance(payload, list):
        return len(payload)
    return 0


def run_once(
    *,
    binary: Path,
    label: str,
    repo_name: str,
    repo_path: Path,
    iteration: int,
    query_args: tuple[str, ...],
) -> dict[str, Any]:
    command = command_for(binary, repo_path, query_args)
    start = time.perf_counter()
    result = subprocess.run(
        command,
        check=False,
        stdout=subprocess.PIPE,
        stderr=subprocess.PIPE,
    )
    elapsed_ms = (time.perf_counter() - start) * 1000.0
    if result.returncode != 0:
        raise SystemExit(
            f"{label} {repo_name} iteration {iteration} failed: "
            f"{result.stderr.decode(errors='replace')}"
        )
    return {
        "bytes": len(result.stdout),
        "elapsed_ms": elapsed_ms,
        "families": family_count(result.stdout),
        "iteration": iteration,
        "label": label,
        "repo": repo_name,
        "sha256": hashlib.sha256(result.stdout).hexdigest(),
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
            )


def summarize(runs: list[dict[str, Any]], repos: list[str]) -> dict[str, Any]:
    by_repo: dict[str, dict[str, dict[str, Any]]] = {}
    for repo in repos:
        by_repo[repo] = {}
        for label in ("baseline", "current"):
            rows = [row for row in runs if row["repo"] == repo and row["label"] == label]
            by_repo[repo][label] = {
                "bytes": sorted({row["bytes"] for row in rows}),
                "families": sorted({row["families"] for row in rows}),
                "hashes": sorted({row["sha256"] for row in rows}),
                "median_ms": statistics.median(row["elapsed_ms"] for row in rows),
            }

    aggregate_baseline = sum(by_repo[repo]["baseline"]["median_ms"] for repo in repos)
    aggregate_current = sum(by_repo[repo]["current"]["median_ms"] for repo in repos)
    delta_pct = (
        ((aggregate_current - aggregate_baseline) / aggregate_baseline) * 100.0
        if aggregate_baseline
        else 0.0
    )
    return {
        "aggregate_baseline_median_ms": aggregate_baseline,
        "aggregate_current_median_ms": aggregate_current,
        "aggregate_delta_pct": delta_pct,
        "by_repo": by_repo,
        "hashes_identical_by_repo": {
            repo: by_repo[repo]["baseline"]["hashes"] == by_repo[repo]["current"]["hashes"]
            for repo in repos
        },
    }


def run_self_test() -> None:
    rows = [
        {"repo": "a", "label": "baseline", "elapsed_ms": 10.0, "bytes": 1, "families": 1, "sha256": "x"},
        {"repo": "a", "label": "current", "elapsed_ms": 12.0, "bytes": 1, "families": 1, "sha256": "x"},
        {"repo": "b", "label": "baseline", "elapsed_ms": 20.0, "bytes": 2, "families": 2, "sha256": "y"},
        {"repo": "b", "label": "current", "elapsed_ms": 18.0, "bytes": 2, "families": 2, "sha256": "y"},
    ]
    summary = summarize(rows, ["a", "b"])
    assert summary["aggregate_baseline_median_ms"] == 30.0
    assert summary["aggregate_current_median_ms"] == 30.0
    assert summary["hashes_identical_by_repo"] == {"a": True, "b": True}
    print("query regression harness self-test passed")


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
    parser.add_argument("--iterations", type=int, default=9)
    parser.add_argument("--warmups", type=int, default=1)
    parser.add_argument("--query-args", default=" ".join(DEFAULT_QUERY_ARGS))
    parser.add_argument("--output", type=Path)
    parser.add_argument("--self-test", action="store_true")
    return parser.parse_args()


def main() -> int:
    args = parse_args()
    if args.self_test:
        run_self_test()
        return 0
    if not args.baseline_binary or not args.current_binary or not args.output or not args.repos:
        raise SystemExit("--baseline-binary, --current-binary, --repo, and --output are required")
    if args.iterations <= 0 or args.warmups < 0:
        raise SystemExit("--iterations must be positive and --warmups must be non-negative")

    baseline_binary = args.baseline_binary.resolve()
    current_binary = args.current_binary.resolve()
    repos = [(repo, (args.repos_root / repo).resolve()) for repo in args.repos]
    missing = [path for _, path in repos if not path.exists()]
    if missing:
        raise SystemExit(f"missing repo paths: {', '.join(path.as_posix() for path in missing)}")
    query_args = parse_query_args(args.query_args)
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
                    )
                )

    repo_names = [repo for repo, _ in repos]
    output = {
        "command": "nose " + " ".join(query_args).replace("{repo}", "<repo>"),
        "provenance": {
            "baseline_binary": baseline_binary.as_posix(),
            "baseline_binary_sha256": sha256_file(baseline_binary),
            "baseline_source_ref": args.baseline_source_ref,
            "baseline_source_sha": args.baseline_source_sha or git_output(["rev-parse", args.baseline_source_ref]),
            "current_binary": current_binary.as_posix(),
            "current_binary_sha256": sha256_file(current_binary),
            "current_source_ref": args.current_source_ref,
            "current_source_sha": args.current_source_sha or git_output(["rev-parse", args.current_source_ref]),
            "harness": "scripts/query-regression-harness.py",
            "harness_command": " ".join(sys.argv),
            "working_tree_status_before_measurement": working_tree_status_before_measurement,
        },
        "repos": repo_names,
        "runs": runs,
        "summary": summarize(runs, repo_names),
    }
    args.output.parent.mkdir(parents=True, exist_ok=True)
    args.output.write_text(json.dumps(output, indent=2, sort_keys=True) + "\n", encoding="utf-8")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
