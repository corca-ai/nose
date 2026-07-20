#!/usr/bin/env python3
"""Run alternating product-query regressions for two nose binaries."""

from __future__ import annotations

import argparse
from collections import Counter
import hashlib
import json
import os
import platform
import re
import shlex
import subprocess
import sys
import tempfile
import time
from pathlib import Path
from typing import Any

from binary_identity import binary_identity, run_self_test as run_binary_identity_self_test
from binary_identity import sha256_file
from query_regression_summary import summarize_runs


DEFAULT_QUERY_ARGS = ("query", "{repo}", "all", "top=0", "--mode", "semantic", "--format", "json")
SCHEMA = "nose.query_regression_harness.v3"
PAIR_ORDERS = ("baseline-current", "current-baseline")
TIME_RE = re.compile(r"\[time\]\s+([a-zA-Z0-9_+\-]+)\s+([0-9.]+)ms")


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
    return result.stdout.strip() or None


def physical_memory_bytes() -> int | None:
    if sys.platform == "darwin":
        raw = optional_command_output(["sysctl", "-n", "hw.memsize"])
        return int(raw) if raw and raw.isdigit() else None
    try:
        return os.sysconf("SC_PAGE_SIZE") * os.sysconf("SC_PHYS_PAGES")
    except (AttributeError, KeyError, OSError, TypeError, ValueError):
        return None


def measurement_environment() -> dict[str, Any]:
    return {
        "architecture": platform.machine(),
        "logical_cpu_count": os.cpu_count(),
        "machine_model": (
            optional_command_output(["sysctl", "-n", "hw.model"])
            if sys.platform == "darwin"
            else None
        ),
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


def command_for(binary: Path, repo_argument: str, query_args: tuple[str, ...]) -> list[str]:
    return [str(binary), *[repo_argument if arg == "{repo}" else arg for arg in query_args]]


def query_observations(stdout: bytes, *, source: str) -> dict[str, Any]:
    try:
        payload = json.loads(stdout)
    except json.JSONDecodeError as error:
        raise SystemExit(f"{source}: invalid query JSON: {error}") from error
    if not isinstance(payload, dict) or not isinstance(payload.get("families"), list):
        raise SystemExit(f"{source}: query JSON must be an object with a families array")
    surfaces: Counter[str] = Counter()
    for index, family in enumerate(payload["families"]):
        if not isinstance(family, dict) or not isinstance(family.get("surface"), str):
            raise SystemExit(f"{source}: families[{index}].surface must be a string")
        surfaces[family["surface"]] += 1
    schema_version = payload.get("schema_version")
    if isinstance(schema_version, bool) or not isinstance(schema_version, int):
        raise SystemExit(f"{source}: schema_version must be an integer")
    return {
        "families": len(payload["families"]),
        "schema_version": schema_version,
        "surface_counts": dict(sorted(surfaces.items())),
    }


def parse_stage_timings(stderr: bytes) -> dict[str, float]:
    text = stderr.decode(errors="replace")
    return {match.group(1): float(match.group(2)) for match in TIME_RE.finditer(text)}


def run_once(
    *,
    binary: Path,
    label: str,
    repo_name: str,
    repos_root: Path,
    iteration: int,
    query_args: tuple[str, ...],
) -> dict[str, Any]:
    # Invoke every checkout as its repo id from the repos-root directory. Family
    # and member ids include path identity, so this stable relative argument is
    # what makes exact output hashes portable across local and CI workspaces.
    command = command_for(binary, repo_name, query_args)
    env = dict(os.environ, NOSE_TIME="1")
    start = time.perf_counter()
    result = subprocess.run(
        command,
        check=False,
        stdout=subprocess.PIPE,
        stderr=subprocess.PIPE,
        env=env,
        cwd=repos_root,
    )
    elapsed_ms = (time.perf_counter() - start) * 1000.0
    if result.returncode != 0:
        raise SystemExit(
            f"{label} {repo_name} iteration {iteration} failed: "
            f"{result.stderr.decode(errors='replace')}"
        )
    observations = query_observations(
        result.stdout, source=f"{label} {repo_name} iteration {iteration}"
    )
    pair_order = PAIR_ORDERS[0] if iteration % 2 else PAIR_ORDERS[1]
    first_label = "baseline" if pair_order == PAIR_ORDERS[0] else "current"
    return {
        "bytes": len(result.stdout),
        "elapsed_ms": elapsed_ms,
        **observations,
        "iteration": iteration,
        "label": label,
        "pair_order": pair_order,
        "pair_position": 0 if label == first_label else 1,
        "repo": repo_name,
        "sha256": hashlib.sha256(result.stdout).hexdigest(),
        "stages_ms": parse_stage_timings(result.stderr),
    }


def measurement_order(repo_names: list[str], iteration: int) -> list[tuple[str, str]]:
    labels = ("baseline", "current") if iteration % 2 else ("current", "baseline")
    return [(label, repo_name) for repo_name in repo_names for label in labels]


def warmup(
    *,
    binaries: dict[str, Path],
    repos: list[tuple[str, Path]],
    repos_root: Path,
    warmups: int,
    query_args: tuple[str, ...],
) -> None:
    for iteration in range(1, warmups + 1):
        for label, repo_name in measurement_order([name for name, _ in repos], iteration):
            run_once(
                binary=binaries[label],
                label=label,
                repo_name=repo_name,
                repos_root=repos_root,
                iteration=-iteration,
                query_args=query_args,
            )


def repo_git_sha(repo_path: Path) -> str:
    result = subprocess.run(
        ["git", "-C", str(repo_path), "rev-parse", "HEAD"],
        check=False,
        stdout=subprocess.PIPE,
        stderr=subprocess.PIPE,
        text=True,
    )
    if result.returncode != 0:
        raise SystemExit(f"{repo_path}: cannot resolve pinned corpus revision: {result.stderr.strip()}")
    return result.stdout.strip()


def corpus_provenance(
    repos: list[tuple[str, Path]],
    corpus_manifest: Path,
    prune_manifest: Path,
    corpus_state: Path | None,
    expected_corpus_state: Path | None,
) -> dict[str, Any]:
    try:
        corpus = json.loads(corpus_manifest.read_text(encoding="utf-8"))
    except (OSError, json.JSONDecodeError) as error:
        raise SystemExit(f"cannot read corpus manifest {corpus_manifest}: {error}") from error
    pinned = {
        row["id"]: row["commit"]
        for row in corpus.get("repositories", [])
        if isinstance(row, dict) and isinstance(row.get("id"), str)
    }
    revisions = []
    for repo_name, repo_path in repos:
        expected = pinned.get(repo_name)
        if not isinstance(expected, str):
            raise SystemExit(f"{repo_name}: missing from {corpus_manifest}")
        actual = repo_git_sha(repo_path)
        if actual != expected:
            raise SystemExit(f"{repo_name}: corpus revision {actual} != pinned {expected}")
        revisions.append({"repo": repo_name, "commit": actual})
    provenance = {
        "corpus_manifest": corpus_manifest.as_posix(),
        "corpus_manifest_sha256": sha256_file(corpus_manifest),
        "prune_manifest": prune_manifest.as_posix(),
        "prune_manifest_sha256": sha256_file(prune_manifest),
        "repositories": revisions,
        "selection_sha256": hashlib.sha256(
            json.dumps(revisions, sort_keys=True, separators=(",", ":")).encode()
        ).hexdigest(),
    }
    if corpus_state is not None:
        try:
            state = json.loads(corpus_state.read_text(encoding="utf-8"))
        except (OSError, json.JSONDecodeError) as error:
            raise SystemExit(f"cannot read corpus state {corpus_state}: {error}") from error
        if state.get("schema") != "nose.pinned_corpus_subset.v1":
            raise SystemExit(f"{corpus_state}: unsupported pinned-corpus state schema")
        if state.get("manifest_sha256") != provenance["prune_manifest_sha256"]:
            raise SystemExit(f"{corpus_state}: prune manifest hash does not match")
        state_repos = state.get("repositories")
        selected_repos = sorted(repo for repo, _ in repos)
        if (
            not isinstance(state_repos, list)
            or state_repos != sorted(set(state_repos))
            or not set(selected_repos) <= set(state_repos)
        ):
            raise SystemExit(f"{corpus_state}: repository selection is not in checked subset")
        digest = state.get("subset_digest_after_prune")
        if not isinstance(digest, dict) or not isinstance(digest.get("hex"), str):
            raise SystemExit(f"{corpus_state}: missing post-prune subset digest")
        provenance.update(
            {
                "corpus_state": corpus_state.as_posix(),
                "corpus_state_sha256": sha256_file(corpus_state),
                "subset_digest_after_prune": digest,
            }
        )
    if expected_corpus_state is not None:
        if corpus_state is None:
            raise SystemExit("--expected-corpus-state requires --corpus-state")
        try:
            expected_state = json.loads(expected_corpus_state.read_text(encoding="utf-8"))
        except (OSError, json.JSONDecodeError) as error:
            raise SystemExit(
                f"cannot read expected corpus state {expected_corpus_state}: {error}"
            ) from error
        if expected_state.get("schema") != "nose.semantic_regression_corpus.v1":
            raise SystemExit(f"{expected_corpus_state}: unsupported expected state schema")
        checks = {
            "corpus_manifest_sha256": provenance["corpus_manifest_sha256"],
            "prune_manifest_sha256": provenance["prune_manifest_sha256"],
            "repositories": state["repositories"],
            "subset_digest_after_prune": state["subset_digest_after_prune"],
        }
        for key, actual in checks.items():
            if expected_state.get(key) != actual:
                raise SystemExit(f"{expected_corpus_state}: checked `{key}` does not match")
        provenance.update(
            {
                "expected_corpus_state": expected_corpus_state.as_posix(),
                "expected_corpus_state_sha256": sha256_file(expected_corpus_state),
            }
        )
    return provenance


def run_self_test() -> None:
    run_binary_identity_self_test()
    assert measurement_order(["a", "b"], 1) == [
        ("baseline", "a"), ("current", "a"), ("baseline", "b"), ("current", "b")
    ]
    assert measurement_order(["a"], 2) == [("current", "a"), ("baseline", "a")]

    def row(repo: str, label: str, elapsed_ms: float, size: int, stage_ms: float) -> dict[str, Any]:
        return {
            "repo": repo,
            "label": label,
            "elapsed_ms": elapsed_ms,
            "bytes": size,
            "families": size,
            "sha256": repo,
            "schema_version": 7,
            "surface_counts": {"default": size},
            "stages_ms": {"lower": stage_ms},
        }

    rows = [
        row("a", "baseline", 10.0, 1, 2.0),
        row("a", "current", 12.0, 1, 3.0),
        row("b", "baseline", 20.0, 2, 4.0),
        row("b", "current", 18.0, 2, 4.0),
    ]
    summary = summarize_runs(rows, ["a", "b"])
    assert summary["aggregate_baseline_median_ms"] == 30.0
    assert summary["aggregate_current_median_ms"] == 30.0
    assert summary["hashes_identical_by_repo"] == {"a": True, "b": True}
    assert summary["by_repo"]["a"]["current"]["stages_median_ms"]["lower"] == 3.0
    parsed = query_observations(
        b'{"schema_version":7,"families":[{"surface":"default"}]}', source="self-test"
    )
    assert parsed == {"families": 1, "schema_version": 7, "surface_counts": {"default": 1}}
    assert parse_query_args("query '{repo}' all top=0 --mode semantic --format json")[1] == "{repo}"
    with tempfile.TemporaryDirectory() as temporary:
        repos_root = Path(temporary)
        nested_repo = repos_root / "crates/nose-cli/src"
        nested_repo.mkdir(parents=True)
        probe = (
            "import json,pathlib,sys; "
            "assert pathlib.Path(sys.argv[1]).is_dir(); "
            "print(json.dumps({'schema_version': 7, 'families': []}))"
        )
        observation = run_once(
            binary=Path(sys.executable),
            label="baseline",
            repo_name="crates/nose-cli/src",
            repos_root=repos_root,
            iteration=1,
            query_args=("-c", probe, "{repo}"),
        )
        assert observation["families"] == 0
        assert observation["pair_order"] == "baseline-current"
        assert observation["pair_position"] == 0
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
    parser.add_argument("--all-repos", action="store_true")
    parser.add_argument("--iterations", type=int, default=9)
    parser.add_argument("--warmups", type=int, default=1)
    parser.add_argument("--query-args", default=" ".join(DEFAULT_QUERY_ARGS))
    parser.add_argument("--corpus-manifest", type=Path)
    parser.add_argument("--prune-manifest", type=Path)
    parser.add_argument("--corpus-state", type=Path)
    parser.add_argument("--expected-corpus-state", type=Path)
    parser.add_argument("--output", type=Path)
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
    repos_root = args.repos_root.resolve()
    repos = selected_repos(args)
    query_args = parse_query_args(args.query_args)
    if (args.corpus_manifest is None) != (args.prune_manifest is None):
        raise SystemExit("--corpus-manifest and --prune-manifest must be provided together")
    if args.corpus_state is not None and args.corpus_manifest is None:
        raise SystemExit("--corpus-state requires --corpus-manifest and --prune-manifest")
    if args.expected_corpus_state is not None and args.corpus_state is None:
        raise SystemExit("--expected-corpus-state requires --corpus-state")
    corpus = (
        corpus_provenance(
            repos,
            args.corpus_manifest.resolve(),
            args.prune_manifest.resolve(),
            args.corpus_state.resolve() if args.corpus_state else None,
            args.expected_corpus_state.resolve() if args.expected_corpus_state else None,
        )
        if args.corpus_manifest is not None and args.prune_manifest is not None
        else None
    )
    working_tree_status_before_measurement = git_output(["status", "--short"])

    binaries = {"baseline": baseline_binary, "current": current_binary}
    repo_names = [repo for repo, _ in repos]
    warmup(
        binaries=binaries,
        repos=repos,
        repos_root=repos_root,
        warmups=args.warmups,
        query_args=query_args,
    )

    runs: list[dict[str, Any]] = []
    for iteration in range(1, args.iterations + 1):
        for label, repo_name in measurement_order(repo_names, iteration):
            runs.append(
                run_once(
                    binary=binaries[label],
                    label=label,
                    repo_name=repo_name,
                    repos_root=repos_root,
                    iteration=iteration,
                    query_args=query_args,
                )
            )
    baseline_identity = binary_identity(baseline_binary)
    current_identity = binary_identity(current_binary)
    output = {
        "schema": SCHEMA,
        "command": "nose " + " ".join(query_args).replace("{repo}", "<repo>"),
        "corpus": corpus,
        "environment": measurement_environment(),
        "measurement": {
            "iterations": args.iterations,
            "warmups": args.warmups,
            "design": {
                "kind": "paired-alternating-blocks/v1",
                "block": "iteration",
                "orders": list(PAIR_ORDERS),
            },
        },
        "execution": {
            "repo_argument": "<repo-id>",
            "working_directory": repos_root.as_posix(),
        },
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
            "harness": "scripts/query-regression-harness.py",
            "harness_command": shlex.join(["python3", *sys.argv]),
            "working_tree_status_before_measurement": working_tree_status_before_measurement,
        },
        "repos": repo_names,
        "runs": runs,
        "summary": summarize_runs(runs, repo_names),
    }
    args.output.parent.mkdir(parents=True, exist_ok=True)
    args.output.write_text(json.dumps(output, indent=2, sort_keys=True) + "\n", encoding="utf-8")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
