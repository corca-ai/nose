#!/usr/bin/env python3
"""Measure clean, cold-cache, and history-bearing query equivalence."""

from __future__ import annotations

import argparse
from dataclasses import dataclass
from query_cache_output import NORMALIZER, comparable_output, self_test as output_self_test

import hashlib
import json
import math
import os
import platform
import re
import shutil
import statistics
import subprocess
import sys
import tempfile
import time
from pathlib import Path
from typing import Any

from binary_identity import binary_identity, sha256_file


ROOT = Path(__file__).resolve().parents[1]
DEFAULT_MANIFEST = ROOT / "bench/cache/mutation-manifest.v1.json"
DEFAULT_BASELINE = ROOT / "bench/cache/official-v0.19.0-binaries.v1.json"
SCHEMA = "nose.cache_query_regression/v1"
RECEIPT_SCHEMA = "nose.cache_query_regression_receipt/v1"
COMPARISON_SCHEMA = "nose.cache_query_comparison/v1"
MANIFEST_SCHEMA = "nose.incremental_mutation_manifest/v1"
BASELINE_SCHEMA = "nose.official_binary_baseline/v1"
TIME_RE = re.compile(r"\[time\]\s+([a-zA-Z0-9_+\-]+)\s+([0-9.]+)ms")
CACHE_RE = re.compile(
    r"\[cache\]\s+files=(\d+)\s+hits=(\d+)\s+misses=(\d+)\s+"
    r"read_bytes=(\d+)\s+written_bytes=(\d+)"
)
INVALIDATION_PREFIX = "[invalidation] "
DARWIN_RSS_RE = re.compile(r"^\s*(\d+)\s+maximum resident set size\s*$", re.MULTILINE)
LINUX_RSS_RE = re.compile(
    r"^\s*Maximum resident set size \(kbytes\):\s*(\d+)\s*$", re.MULTILINE
)
DEFAULT_TERMS = ("all", "top=0")
DEFAULT_FLAGS = ("--min-lines", "1", "--min-size", "1")
DEFAULT_MODES = ("semantic",)
EXECUTABLE_CASES = {
    "no-op",
    "leaf-edit",
    "provider-export-edit",
    "provider-non-export-edit",
    "high-fanout-provider-edit",
    "add-delete-rename",
    "embedded-region-edit",
    "ignore-exclude-root-change",
    "analysis-config-change",
    "view-config-change",
    "baseline-ignore-change",
    "semantic-pack-change",
    "swift-global-barrier-change",
    "same-size-restored-mtime-edit",
}


@dataclass(frozen=True)
class Scenario:
    before: dict[str, str]
    after: dict[str, str]
    before_terms: tuple[str, ...] = DEFAULT_TERMS
    after_terms: tuple[str, ...] = DEFAULT_TERMS
    before_flags: tuple[str, ...] = DEFAULT_FLAGS
    after_flags: tuple[str, ...] = DEFAULT_FLAGS
    restore_mtime: tuple[str, ...] = ()


@dataclass(frozen=True)
class RealLeafMutation:
    path: Path
    find: str
    replace: str


def clone_source(name: str, operator: str = "+") -> str:
    return (
        f"def {name}(items):\n"
        "    total = 0\n"
        "    for item in items:\n"
        "        if item > 0:\n"
        f"            total = total {operator} item * item\n"
        "    return total\n"
    )


def ordinary_clone_files() -> dict[str, str]:
    return {
        "a/one.py": clone_source("one"),
        "b/two.py": clone_source("two"),
        "c/three.py": clone_source("three"),
    }


def imported_map_files(value: int = 1, fanout: int = 1) -> dict[str, str]:
    files = {
        "local.py": (
            "def lookup(key, other):\n"
            "    return {\"red\": 1, \"blue\": 2}.get(key, 0)\n"
        ),
        "tables.py": f"LOOKUP = {{\"red\": {value}, \"blue\": 2}}\n",
    }
    for index in range(fanout):
        files[f"consumers/imported_{index:02d}.py"] = (
            "from tables import LOOKUP\n\n"
            f"def lookup_{index}(key, other):\n"
            "    return LOOKUP.get(key, 0)\n"
        )
    return files


def embedded_files(changed: bool = False) -> dict[str, str]:
    files = {}
    for index in range(3):
        operator = "-" if changed and index == 2 else "+"
        files[f"site/{index}.html"] = (
            "<html><body><script>\n"
            f"function f{index}(items) {{\n"
            "  let total = 0;\n"
            "  for (const item of items) {\n"
            "    if (item > 0) {\n"
            f"      total = total {operator} item * item;\n"
            "    }\n"
            "  }\n"
            "  return total;\n"
            "}\n</script></body></html>\n"
        )
    return files


def scenario(case: str) -> Scenario:
    clones = ordinary_clone_files()
    if case == "no-op":
        return Scenario(clones, clones)
    if case == "leaf-edit":
        after = dict(clones)
        after["c/three.py"] = clone_source("three", "-")
        return Scenario(clones, after)
    if case == "provider-export-edit":
        return Scenario(imported_map_files(), imported_map_files(9))
    if case == "provider-non-export-edit":
        before = imported_map_files()
        after = dict(before)
        before["tables.py"] += "\ndef private_helper():\n    return 1\n"
        after["tables.py"] += "\ndef private_helper():\n    return 9\n"
        return Scenario(before, after)
    if case == "high-fanout-provider-edit":
        return Scenario(imported_map_files(fanout=32), imported_map_files(9, fanout=32))
    if case == "add-delete-rename":
        before = dict(clones)
        before["deleted/four.py"] = clone_source("four")
        after = {
            "a/one.py": clones["a/one.py"],
            "renamed/two.py": clones["b/two.py"],
            "c/three.py": clones["c/three.py"],
            "added/five.py": clone_source("five"),
        }
        return Scenario(before, after)
    if case == "embedded-region-edit":
        return Scenario(embedded_files(), embedded_files(changed=True))
    if case == "ignore-exclude-root-change":
        return Scenario(clones, clones, after_flags=(*DEFAULT_FLAGS, "--exclude", "c/**"))
    if case == "analysis-config-change":
        return Scenario(clones, clones, after_flags=("--min-lines", "20", "--min-size", "1"))
    if case == "view-config-change":
        return Scenario(clones, clones, after_terms=("surface=hidden", "top=0"))
    if case == "baseline-ignore-change":
        before = {**clones, "nose.ignore.json": '{"ignores":[]}\n'}
        after = {
            **clones,
            "nose.ignore.json": (
                '{"ignores":[{"paths":["a/**","b/**","c/**"],'
                '"reason":"template-copy"}]}\n'
            ),
        }
        flags = (*DEFAULT_FLAGS, "--ignore-file", "repo/nose.ignore.json")
        return Scenario(before, after, before_flags=flags, after_flags=flags)
    if case == "semantic-pack-change":
        pack = (ROOT / "docs/examples/semantic-packs/v0/library-pack.json").read_text(
            encoding="utf-8"
        )
        after = {**clones, "pack.json": pack}
        return Scenario(clones, after, after_flags=(*DEFAULT_FLAGS, "--semantic-pack", "repo/pack.json"))
    if case == "swift-global-barrier-change":
        user = (
            "func positive(_ values: [Int]) -> Bool {\n"
            "  return values.allSatisfy { value in value >= 0 }\n"
            "}\n"
        )
        twin = user.replace("positive", "alsoPositive")
        before = {"User.swift": user, "Twin.swift": twin}
        after = {
            **before,
            "Overload.swift": (
                "extension Array where Element == Int {\n"
                "  func allSatisfy(_ predicate: (Int) -> Bool) -> Bool { false }\n"
                "}\n"
            ),
        }
        return Scenario(before, after)
    if case == "same-size-restored-mtime-edit":
        before = {**clones, "leaf.py": "def value(x):\n    return x + 1\n"}
        after = {**clones, "leaf.py": "def value(x):\n    return x - 1\n"}
        assert len(before["leaf.py"]) == len(after["leaf.py"])
        return Scenario(before, after, restore_mtime=("leaf.py",))
    raise ValueError(f"unsupported executable mutation: {case}")


def load_json(path: Path) -> dict[str, Any]:
    try:
        value = json.loads(path.read_text(encoding="utf-8"))
    except (OSError, json.JSONDecodeError) as error:
        raise SystemExit(f"cannot read {path}: {error}") from error
    if not isinstance(value, dict):
        raise SystemExit(f"{path}: expected a JSON object")
    return value


def portable_path(path: Path) -> str:
    resolved = path.resolve()
    try:
        return resolved.relative_to(ROOT).as_posix()
    except ValueError:
        return resolved.as_posix()


def validate_manifests(mutation_path: Path, baseline_path: Path) -> tuple[dict[str, Any], dict[str, Any]]:
    manifest = load_json(mutation_path)
    if manifest.get("schema") != MANIFEST_SCHEMA:
        raise SystemExit(f"{mutation_path}: unsupported schema")
    if manifest.get("minimum_replays") != 30:
        raise SystemExit(f"{mutation_path}: minimum_replays must be 30")
    mutations = manifest.get("mutations")
    if not isinstance(mutations, list):
        raise SystemExit(f"{mutation_path}: mutations must be an array")
    ids = [row.get("id") for row in mutations if isinstance(row, dict)]
    if len(ids) != len(mutations) or any(not isinstance(case, str) for case in ids):
        raise SystemExit(f"{mutation_path}: every mutation needs a string id")
    if len(set(ids)) != len(ids) or not EXECUTABLE_CASES < set(ids):
        raise SystemExit(f"{mutation_path}: mutation ids are missing, duplicate, or incomplete")
    for row in mutations:
        if not isinstance(row.get("changed_inputs"), list) or not isinstance(
            row.get("expected_invalidation_closure"), list
        ):
            raise SystemExit(f"{mutation_path}: {row.get('id')} lacks an invalidation contract")
    tiers = manifest.get("workloads", {}).get("synthetic", [])
    if [row.get("files") for row in tiers] != [1000, 10000, 100000]:
        raise SystemExit(f"{mutation_path}: synthetic tiers must be exactly 1k/10k/100k")

    baseline = load_json(baseline_path)
    if baseline.get("schema") != BASELINE_SCHEMA or re.fullmatch(
        r"[0-9]+\.[0-9]+\.[0-9]+", str(baseline.get("version"))
    ) is None:
        raise SystemExit(f"{baseline_path}: expected a versioned official release baseline")
    artifacts = baseline.get("artifacts")
    if not isinstance(artifacts, list) or len(artifacts) != 4:
        raise SystemExit(f"{baseline_path}: expected four published targets")
    for artifact in artifacts:
        for key in ("archive_sha256", "binary_sha256", "binary_code_sha256"):
            value = artifact.get(key)
            if not isinstance(value, str) or re.fullmatch(r"[0-9a-f]{64}", value) is None:
                raise SystemExit(f"{baseline_path}: invalid {key}")
    return manifest, baseline


def source_identity(root: Path) -> str:
    digest = hashlib.sha256()
    files = sorted(
        path
        for path in root.rglob("*")
        if path.is_file() and ".git" not in path.relative_to(root).parts
    )
    for path in files:
        relative = path.relative_to(root).as_posix().encode()
        digest.update(relative)
        digest.update(b"\0")
        digest.update(path.read_bytes())
        digest.update(b"\0")
    return digest.hexdigest()


def repository_head(root: Path) -> str:
    result = subprocess.run(
        ["git", "-C", str(root), "rev-parse", "HEAD"],
        stdout=subprocess.PIPE,
        stderr=subprocess.PIPE,
        text=True,
    )
    head = result.stdout.strip()
    if result.returncode != 0 or re.fullmatch(r"[0-9a-f]{40}", head) is None:
        raise SystemExit(f"cannot resolve repository HEAD for {root}: {result.stderr.strip()}")
    return head


def write_snapshot(root: Path, files: dict[str, str]) -> None:
    shutil.rmtree(root, ignore_errors=True)
    root.mkdir(parents=True)
    for relative, content in sorted(files.items()):
        path = root / relative
        path.parent.mkdir(parents=True, exist_ok=True)
        path.write_text(content, encoding="utf-8")


def apply_after(root: Path, value: Scenario) -> None:
    mtimes = {
        relative: (root / relative).stat().st_mtime_ns for relative in value.restore_mtime
    }
    write_snapshot(root, value.after)
    for relative, mtime_ns in mtimes.items():
        os.utime(root / relative, ns=(mtime_ns, mtime_ns))


def parse_stages(stderr: str) -> dict[str, float]:
    return {match.group(1): float(match.group(2)) for match in TIME_RE.finditer(stderr)}


def parse_cache_stats(stderr: str) -> dict[str, int] | None:
    match = CACHE_RE.search(stderr)
    if match is None:
        return None
    return dict(zip(("files", "hits", "misses", "read_bytes", "written_bytes"), map(int, match.groups())))


def parse_invalidation(stderr: str) -> dict[str, Any] | None:
    for line in stderr.splitlines():
        stripped = line.strip()
        if not stripped.startswith(INVALIDATION_PREFIX):
            continue
        try:
            value = json.loads(stripped.removeprefix(INVALIDATION_PREFIX))
        except json.JSONDecodeError as error:
            raise SystemExit(f"invalid cache invalidation JSON: {error}") from error
        if not isinstance(value, dict) or value.get("schema") != "nose.invalidation/v1":
            raise SystemExit("cache invalidation evidence has an unsupported schema")
        return value
    return None


def parse_rss(stderr: str) -> int:
    if sys.platform == "darwin":
        match = DARWIN_RSS_RE.search(stderr)
        multiplier = 1
    elif sys.platform.startswith("linux"):
        match = LINUX_RSS_RE.search(stderr)
        multiplier = 1024
    else:
        raise SystemExit(f"peak RSS measurement is unsupported on {sys.platform}")
    if match is None:
        raise SystemExit("/usr/bin/time did not report peak RSS")
    return int(match.group(1)) * multiplier


def store_usage(root: Path) -> dict[str, int]:
    files = [path for path in root.rglob("*") if path.is_file()] if root.exists() else []
    return {"files": len(files), "bytes": sum(path.stat().st_size for path in files)}


def query_command(
    binary: Path,
    repo_argument: str,
    terms: tuple[str, ...],
    flags: tuple[str, ...],
    cache: Path | None,
) -> list[str]:
    command = [
        str(binary),
        "query",
        repo_argument,
        *terms,
        "--format",
        "json",
        *flags,
    ]
    for mode in DEFAULT_MODES:
        command.extend(("--mode", mode))
    if cache is not None:
        command.extend(("--cache-dir", str(cache)))
    return command


def run_query(
    *,
    binary: Path,
    cwd: Path,
    repo_argument: str,
    phase: str,
    replay: int,
    terms: tuple[str, ...],
    flags: tuple[str, ...],
    cache: Path | None,
    require_cache_stats: bool,
) -> tuple[dict[str, Any], bytes]:
    command = query_command(binary, repo_argument, terms, flags, cache)
    if not Path("/usr/bin/time").is_file():
        raise SystemExit("/usr/bin/time is required for peak RSS measurement")
    timed = ["/usr/bin/time", "-l" if sys.platform == "darwin" else "-v", *command]
    env = dict(os.environ, NOSE_TIME="1")
    if cache is not None:
        env["NOSE_CACHE_STATS"] = "1"
    started = time.perf_counter()
    result = subprocess.run(timed, cwd=cwd, env=env, stdout=subprocess.PIPE, stderr=subprocess.PIPE)
    elapsed_ms = (time.perf_counter() - started) * 1000.0
    stderr = result.stderr.decode(errors="replace")
    if result.returncode != 0:
        raise SystemExit(f"{phase} replay {replay} failed ({result.returncode}):\n{stderr}")
    try:
        payload = json.loads(result.stdout)
    except json.JSONDecodeError as error:
        raise SystemExit(f"{phase} replay {replay}: invalid query JSON: {error}") from error
    if not isinstance(payload, dict) or not isinstance(payload.get("families"), list):
        raise SystemExit(f"{phase} replay {replay}: expected query object with families")
    cache_stats = parse_cache_stats(stderr)
    if cache is not None and require_cache_stats and cache_stats is None:
        raise SystemExit(f"{phase} replay {replay}: binary omitted NOSE_CACHE_STATS evidence")
    invalidation = parse_invalidation(stderr)
    if cache is not None and require_cache_stats and invalidation is None:
        raise SystemExit(f"{phase} replay {replay}: binary omitted invalidation evidence")
    comparable = comparable_output(payload, cache)
    row = {
        "phase": phase,
        "replay": replay,
        "elapsed_ms": elapsed_ms,
        "peak_rss_bytes": parse_rss(stderr),
        "output_bytes": len(result.stdout),
        "output_sha256": hashlib.sha256(comparable).hexdigest(),
        "raw_output_sha256": hashlib.sha256(result.stdout).hexdigest(),
        "output_normalizer": NORMALIZER,
        "families": len(payload["families"]),
        "schema_version": payload.get("schema_version"),
        "stages_ms": parse_stages(stderr),
        "cache": cache_stats,
        "invalidation": invalidation,
        "store": store_usage(cache) if cache is not None else None,
    }
    return row, comparable


def assert_equal(left: bytes, right: bytes, label: str) -> None:
    if left != right:
        raise SystemExit(
            f"{label}: output mismatch "
            f"{hashlib.sha256(left).hexdigest()} != {hashlib.sha256(right).hexdigest()}"
        )


def run_fixture_replay(
    binary: Path,
    workspace: Path,
    value: Scenario,
    replay: int,
    require_cache_stats: bool,
) -> tuple[list[dict[str, Any]], str, str]:
    repo = workspace / "repo"
    history = workspace / "history-cache"
    cold = workspace / "cold-cache"
    shutil.rmtree(history, ignore_errors=True)
    shutil.rmtree(cold, ignore_errors=True)
    write_snapshot(repo, value.before)
    before_identity = source_identity(repo)
    clean_seed, clean_seed_bytes = run_query(
        binary=binary, cwd=workspace, repo_argument="repo", phase="clean-seed",
        replay=replay, terms=value.before_terms, flags=value.before_flags, cache=None,
        require_cache_stats=require_cache_stats,
    )
    seed, seed_bytes = run_query(
        binary=binary, cwd=workspace, repo_argument="repo", phase="empty-store-seed",
        replay=replay, terms=value.before_terms, flags=value.before_flags, cache=history,
        require_cache_stats=require_cache_stats,
    )
    assert_equal(clean_seed_bytes, seed_bytes, f"seed replay {replay}")
    apply_after(repo, value)
    after_identity = source_identity(repo)
    clean_after, clean_after_bytes = run_query(
        binary=binary, cwd=workspace, repo_argument="repo", phase="clean-after",
        replay=replay, terms=value.after_terms, flags=value.after_flags, cache=None,
        require_cache_stats=require_cache_stats,
    )
    cold_after, cold_after_bytes = run_query(
        binary=binary, cwd=workspace, repo_argument="repo", phase="empty-store-after",
        replay=replay, terms=value.after_terms, flags=value.after_flags, cache=cold,
        require_cache_stats=require_cache_stats,
    )
    warm_after, warm_after_bytes = run_query(
        binary=binary, cwd=workspace, repo_argument="repo", phase="history-after",
        replay=replay, terms=value.after_terms, flags=value.after_flags, cache=history,
        require_cache_stats=require_cache_stats,
    )
    assert_equal(clean_after_bytes, cold_after_bytes, f"cold replay {replay}")
    assert_equal(clean_after_bytes, warm_after_bytes, f"history replay {replay}")
    return [clean_seed, seed, clean_after, cold_after, warm_after], before_identity, after_identity


def generate_synthetic(root: Path, files: int) -> None:
    shutil.rmtree(root, ignore_errors=True)
    root.mkdir(parents=True)
    for index in range(files):
        shard = root / f"s{index // 1000:03d}"
        shard.mkdir(exist_ok=True)
        (shard / f"f{index:06d}.py").write_text(
            f"def value_{index}(x):\n    return x + {index}\n", encoding="utf-8"
        )


def run_noop_replay(
    binary: Path,
    workspace: Path,
    repo: Path,
    replay: int,
    require_cache_stats: bool,
) -> tuple[list[dict[str, Any]], bool, bool]:
    cold = workspace / "cold-cache"
    shutil.rmtree(cold, ignore_errors=True)
    cwd = repo.parent
    argument = repo.name
    clean, clean_bytes = run_query(
        binary=binary, cwd=cwd, repo_argument=argument, phase="clean-after", replay=replay,
        terms=DEFAULT_TERMS, flags=DEFAULT_FLAGS, cache=None,
        require_cache_stats=require_cache_stats,
    )
    cold_row, cold_bytes = run_query(
        binary=binary, cwd=cwd, repo_argument=argument, phase="empty-store-after", replay=replay,
        terms=DEFAULT_TERMS, flags=DEFAULT_FLAGS, cache=cold,
        require_cache_stats=require_cache_stats,
    )
    warm_row, warm_bytes = run_query(
        binary=binary, cwd=cwd, repo_argument=argument, phase="history-after", replay=replay,
        terms=DEFAULT_TERMS, flags=DEFAULT_FLAGS, cache=cold,
        require_cache_stats=require_cache_stats,
    )
    return [clean, cold_row, warm_row], clean_bytes == cold_bytes, clean_bytes == warm_bytes


def run_real_leaf_replay(
    binary: Path,
    workspace: Path,
    source_repo: Path,
    mutation: RealLeafMutation,
    replay: int,
    require_cache_stats: bool,
    *, include_seed: bool = False,
) -> tuple[list[dict[str, Any]], bool, bool, str, str]:
    shutil.rmtree(workspace, ignore_errors=True)
    workspace.mkdir(parents=True)
    repo = workspace / "repo"
    shutil.copytree(
        source_repo,
        repo,
        symlinks=True,
        ignore=shutil.ignore_patterns(".git"),
    )
    leaf = repo / mutation.path
    try:
        content = leaf.read_text(encoding="utf-8")
    except OSError as error:
        raise SystemExit(f"cannot read real leaf {leaf}: {error}") from error
    if not mutation.find or content.count(mutation.find) != 1:
        raise SystemExit(
            f"real leaf {mutation.path}: --leaf-find must occur exactly once"
        )
    before_identity = source_identity(repo)
    history = workspace / "history-cache"
    cold = workspace / "cold-cache"
    clean_seed, clean_seed_bytes = run_query(
        binary=binary, cwd=workspace, repo_argument="repo", phase="clean-seed",
        replay=replay, terms=DEFAULT_TERMS, flags=DEFAULT_FLAGS, cache=None,
        require_cache_stats=require_cache_stats,
    )
    seed, seed_bytes = run_query(
        binary=binary, cwd=workspace, repo_argument="repo", phase="empty-store-seed",
        replay=replay, terms=DEFAULT_TERMS, flags=DEFAULT_FLAGS, cache=history,
        require_cache_stats=require_cache_stats,
    )
    assert_equal(clean_seed_bytes, seed_bytes, f"real leaf seed replay {replay}")
    leaf.write_text(content.replace(mutation.find, mutation.replace), encoding="utf-8")
    after_identity = source_identity(repo)
    clean, clean_bytes = run_query(
        binary=binary, cwd=workspace, repo_argument="repo", phase="clean-after",
        replay=replay, terms=DEFAULT_TERMS, flags=DEFAULT_FLAGS, cache=None,
        require_cache_stats=require_cache_stats,
    )
    cold_row, cold_bytes = run_query(
        binary=binary, cwd=workspace, repo_argument="repo", phase="empty-store-after",
        replay=replay, terms=DEFAULT_TERMS, flags=DEFAULT_FLAGS, cache=cold,
        require_cache_stats=require_cache_stats,
    )
    warm_row, warm_bytes = run_query(
        binary=binary, cwd=workspace, repo_argument="repo", phase="history-after",
        replay=replay, terms=DEFAULT_TERMS, flags=DEFAULT_FLAGS, cache=history,
        require_cache_stats=require_cache_stats,
    )
    return (
        ([clean_seed, seed] if include_seed else []) + [clean, cold_row, warm_row],
        clean_bytes == cold_bytes,
        clean_bytes == warm_bytes,
        before_identity,
        after_identity,
    )


def nearest_rank_p95(values: list[float]) -> float:
    ordered = sorted(values)
    return ordered[math.ceil(0.95 * len(ordered)) - 1]


def distribution(values: list[float]) -> dict[str, float]:
    return {"p50": statistics.median(values), "p95": nearest_rank_p95(values)}


def summarize_phases(rows: list[dict[str, Any]]) -> dict[str, Any]:
    summary = {}
    for phase in sorted({row["phase"] for row in rows}):
        selected = [row for row in rows if row["phase"] == phase]
        stage_names = sorted({name for row in selected for name in row["stages_ms"]})
        cache_names = ("files", "hits", "misses", "read_bytes", "written_bytes")
        summary[phase] = {
            "elapsed_ms": distribution([row["elapsed_ms"] for row in selected]),
            "peak_rss_bytes": distribution([row["peak_rss_bytes"] for row in selected]),
            "stages_ms": {
                name: distribution([row["stages_ms"].get(name, 0.0) for row in selected])
                for name in stage_names
            },
            "cache": {
                name: distribution([row["cache"][name] for row in selected if row["cache"] is not None])
                for name in cache_names
            } if any(row["cache"] is not None for row in selected) else None,
            "store_bytes": distribution([row["store"]["bytes"] for row in selected if row["store"] is not None])
            if any(row["store"] is not None for row in selected) else None,
            "output_sha256": sorted({row["output_sha256"] for row in selected}),
        }
    return summary


def summarize(rows: list[dict[str, Any]]) -> dict[str, Any]:
    return {
        workload_id: summarize_phases(
            [row for row in rows if row["workload_id"] == workload_id]
        )
        for workload_id in sorted({row["workload_id"] for row in rows})
    }


def verify_official_binary(
    binary: Path, archive: Path, target: str, baseline: dict[str, Any]
) -> dict[str, str]:
    artifact = next(
        (row for row in baseline["artifacts"] if row.get("target") == target), None
    )
    if artifact is None:
        raise SystemExit(f"official baseline has no target {target}")
    if not archive.is_file():
        raise SystemExit(f"missing official archive: {archive}")
    archive_sha256 = sha256_file(archive)
    if archive_sha256 != artifact["archive_sha256"]:
        raise SystemExit(
            f"{archive}: official {target} archive hash mismatch "
            f"({archive_sha256} != {artifact['archive_sha256']})"
        )
    identity = binary_identity(binary)
    if identity.file_sha256 != artifact["binary_sha256"]:
        raise SystemExit(
            f"{binary}: official {target} binary hash mismatch "
            f"({identity.file_sha256} != {artifact['binary_sha256']})"
        )
    if identity.code_sha256 != artifact["binary_code_sha256"]:
        raise SystemExit(
            f"{binary}: official {target} code hash mismatch "
            f"({identity.code_sha256} != {artifact['binary_code_sha256']})"
        )
    return {
        "target": target,
        "archive": portable_path(archive),
        "archive_sha256": archive_sha256,
    }


def require_fields(value: dict[str, Any], fields: tuple[str, ...], label: str) -> None:
    missing = [field for field in fields if field not in value]
    if missing:
        raise SystemExit(f"{label}: missing {', '.join(missing)}")


def validate_report_payload(report: dict[str, Any]) -> None:
    status = report.get("status")
    if report.get("schema") != SCHEMA or status not in {"passed", "failed-equivalence"}:
        raise SystemExit("cache report must be a passed or characterized v1 report")
    measurement = report.get("measurement")
    workload = report.get("workload")
    provenance = report.get("provenance")
    runs = report.get("runs")
    if not isinstance(measurement, dict) or not isinstance(workload, dict):
        raise SystemExit("cache report lacks measurement or workload metadata")
    if not isinstance(provenance, dict) or not isinstance(runs, list) or not runs:
        raise SystemExit("cache report lacks provenance or raw runs")
    require_fields(
        provenance,
        (
            "binary_sha256",
            "binary_code_sha256",
            "binary_revision",
            "harness_sha256",
            "mutation_manifest_sha256",
            "official_baseline_sha256",
        ),
        "cache report provenance",
    )
    for field in (
        "binary_sha256",
        "binary_code_sha256",
        "harness_sha256",
        "mutation_manifest_sha256",
        "official_baseline_sha256",
    ):
        if re.fullmatch(r"[0-9a-f]{64}", str(provenance[field])) is None:
            raise SystemExit(f"cache report provenance has invalid {field}")
    if not isinstance(provenance["binary_revision"], str) or not provenance["binary_revision"]:
        raise SystemExit("cache report provenance requires a binary revision")

    replays = measurement.get("replays")
    minimum = measurement.get("minimum_replays")
    if not isinstance(replays, int) or not isinstance(minimum, int) or replays < minimum:
        raise SystemExit("cache report does not meet its minimum replay count")
    if measurement.get("p95") != "nearest-rank":
        raise SystemExit("cache report does not use nearest-rank p95")
    characterization = measurement.get("characterization_only") is True
    if status == "failed-equivalence" and not characterization:
        raise SystemExit("an equivalence failure must be an explicit characterization")
    if characterization and not isinstance(provenance.get("official_release_verification"), dict):
        raise SystemExit("equivalence characterization requires a verified official release")

    kind = workload.get("kind")
    if kind == "fixture-matrix":
        workload_ids = workload.get("ids")
        if workload_ids != sorted(EXECUTABLE_CASES):
            raise SystemExit("fixture matrix is not the complete executable mutation set")
        expected_phases = {
            "clean-seed",
            "empty-store-seed",
            "clean-after",
            "empty-store-after",
            "history-after",
        }
    elif kind == "fixture":
        workload_ids = [workload.get("id")]
        if workload_ids[0] not in EXECUTABLE_CASES:
            raise SystemExit("cache report has an unknown fixture")
        expected_phases = {
            "clean-seed",
            "empty-store-seed",
            "clean-after",
            "empty-store-after",
            "history-after",
        }
    elif kind in {"real", "synthetic"}:
        workload_ids = [workload.get("id")]
        expected_phases = {"clean-after", "empty-store-after", "history-after"}
    else:
        raise SystemExit(f"cache report has unsupported workload kind {kind}")

    require_cache_stats = measurement.get("cache_stats_required") is True
    mismatches = 0
    seed_mismatches = 0
    cold_mismatches = 0
    history_mismatches = 0
    for workload_id in workload_ids:
        selected = [row for row in runs if row.get("workload_id") == workload_id]
        if len(selected) != replays * len(expected_phases):
            raise SystemExit(f"{workload_id}: incomplete raw replay rows")
        for replay in range(1, replays + 1):
            replay_rows = [row for row in selected if row.get("replay") == replay]
            by_phase = {row.get("phase"): row for row in replay_rows}
            if set(by_phase) != expected_phases:
                raise SystemExit(f"{workload_id} replay {replay}: incomplete phase set")
            for phase, row in by_phase.items():
                require_fields(
                    row,
                    (
                        "elapsed_ms",
                        "peak_rss_bytes",
                        "output_sha256",
                        "stages_ms",
                        "store",
                    ),
                    f"{workload_id} replay {replay} {phase}",
                )
                if phase != "clean-after" and phase != "clean-seed":
                    if require_cache_stats and not isinstance(row.get("cache"), dict):
                        raise SystemExit(
                            f"{workload_id} replay {replay} {phase}: missing cache stats"
                        )
            if "clean-seed" in by_phase:
                if (
                    by_phase["clean-seed"]["output_sha256"]
                    != by_phase["empty-store-seed"]["output_sha256"]
                ):
                    mismatches += 1
                    seed_mismatches += 1
            clean_hash = by_phase["clean-after"]["output_sha256"]
            cold_mismatch = by_phase["empty-store-after"]["output_sha256"] != clean_hash
            history_mismatch = by_phase["history-after"]["output_sha256"] != clean_hash
            cold_mismatches += cold_mismatch
            history_mismatches += history_mismatch
            mismatches += cold_mismatch + history_mismatch

    equivalence = report.get("equivalence")
    if not isinstance(equivalence, dict):
        raise SystemExit("cache report lacks equivalence summary")
    expected_seed = None if kind in {"real", "synthetic"} else seed_mismatches == 0
    if equivalence != {
        "seed_clean_equals_empty_store": expected_seed,
        "after_clean_equals_empty_store": cold_mismatches == 0,
        "after_clean_equals_history_store": history_mismatches == 0,
    }:
        raise SystemExit("cache report equivalence summary does not match its raw rows")

    if status == "passed" and mismatches:
        raise SystemExit("passed cache report contains output mismatches")
    if status == "failed-equivalence" and not mismatches:
        raise SystemExit("failed-equivalence report contains no output mismatch")

    expected_summary = summarize(runs)
    if report.get("summary") != expected_summary:
        raise SystemExit("cache report summary does not match its raw rows")


def validate_comparison_payload(report: dict[str, Any]) -> None:
    if report.get("schema") != COMPARISON_SCHEMA or report.get("status") != "passed":
        raise SystemExit("cache comparison must be a passed v1 report")
    measurement = report.get("measurement")
    workload = report.get("workload")
    provenance = report.get("provenance")
    runs = report.get("runs")
    if not all(isinstance(value, dict) for value in (measurement, workload, provenance)):
        raise SystemExit("cache comparison lacks measurement, workload, or provenance metadata")
    if not isinstance(runs, list) or not runs:
        raise SystemExit("cache comparison lacks raw runs")
    if workload.get("kind") != "real" or not isinstance(workload.get("id"), str):
        raise SystemExit("cache comparison requires one pinned real workload")
    replays = measurement.get("replays")
    minimum = measurement.get("minimum_replays")
    if not isinstance(replays, int) or not isinstance(minimum, int) or replays < minimum:
        raise SystemExit("cache comparison does not meet its minimum replay count")
    if measurement.get("p95") != "nearest-rank":
        raise SystemExit("cache comparison does not use nearest-rank p95")
    if measurement.get("order") != "alternating-ab-ba":
        raise SystemExit("cache comparison does not alternate candidate and official order")
    require_fields(
        provenance,
        (
            "candidate",
            "official",
            "harness_sha256",
            "mutation_manifest_sha256",
            "official_baseline_sha256",
        ),
        "cache comparison provenance",
    )
    for role in ("candidate", "official"):
        identity = provenance.get(role)
        if not isinstance(identity, dict):
            raise SystemExit(f"cache comparison lacks {role} binary provenance")
        require_fields(
            identity,
            ("binary_sha256", "binary_code_sha256", "binary_revision"),
            f"cache comparison {role} provenance",
        )
        for field in ("binary_sha256", "binary_code_sha256"):
            if re.fullmatch(r"[0-9a-f]{64}", str(identity[field])) is None:
                raise SystemExit(f"cache comparison {role} has invalid {field}")
        if not isinstance(identity["binary_revision"], str) or not identity["binary_revision"]:
            raise SystemExit(f"cache comparison {role} requires a binary revision")
    if not isinstance(provenance["official"].get("release_verification"), dict):
        raise SystemExit("cache comparison requires verified official release provenance")
    for field in (
        "harness_sha256",
        "mutation_manifest_sha256",
        "official_baseline_sha256",
    ):
        if re.fullmatch(r"[0-9a-f]{64}", str(provenance[field])) is None:
            raise SystemExit(f"cache comparison provenance has invalid {field}")

    expected_phases = {"clean-after", "empty-store-after", "history-after"}
    workload_id = workload["id"]
    if len(runs) != replays * len(expected_phases) * 2:
        raise SystemExit("cache comparison has an incomplete raw replay matrix")
    equivalence: dict[str, bool] = {}
    for role in ("candidate", "official"):
        selected = [row for row in runs if row.get("binary_role") == role]
        if len(selected) != replays * len(expected_phases):
            raise SystemExit(f"cache comparison has incomplete {role} rows")
        role_equivalent = True
        for replay in range(1, replays + 1):
            replay_rows = [row for row in selected if row.get("replay") == replay]
            by_phase = {row.get("phase"): row for row in replay_rows}
            if set(by_phase) != expected_phases:
                raise SystemExit(f"cache comparison {role} replay {replay}: incomplete phases")
            expected_first = "candidate" if replay % 2 else "official"
            for phase, row in by_phase.items():
                if row.get("workload_id") != workload_id:
                    raise SystemExit(f"cache comparison {role} replay {replay}: wrong workload")
                if row.get("first_role") != expected_first:
                    raise SystemExit(f"cache comparison replay {replay}: wrong execution order")
                require_fields(
                    row,
                    ("elapsed_ms", "peak_rss_bytes", "output_sha256", "stages_ms", "store"),
                    f"cache comparison {role} replay {replay} {phase}",
                )
                if role == "candidate" and phase != "clean-after" and not isinstance(
                    row.get("cache"), dict
                ):
                    raise SystemExit(
                        f"cache comparison candidate replay {replay} {phase}: missing cache stats"
                    )
            clean_hash = by_phase["clean-after"]["output_sha256"]
            role_equivalent &= by_phase["empty-store-after"]["output_sha256"] == clean_hash
            role_equivalent &= by_phase["history-after"]["output_sha256"] == clean_hash
        equivalence[role] = role_equivalent
    if report.get("equivalence") != equivalence or not all(equivalence.values()):
        raise SystemExit("cache comparison equivalence summary does not match its raw rows")
    expected_summary = {
        role: summarize([row for row in runs if row.get("binary_role") == role])
        for role in ("candidate", "official")
    }
    if report.get("summary") != expected_summary:
        raise SystemExit("cache comparison summary does not match its raw rows")


def validate_report(path: Path, baseline_path: Path = DEFAULT_BASELINE) -> None:
    report = load_json(path)
    if report.get("schema") == COMPARISON_SCHEMA:
        validate_comparison_payload(report)
    else:
        validate_report_payload(report)
    provenance = report["provenance"]
    current = {
        "mutation_manifest_sha256": sha256_file(DEFAULT_MANIFEST),
        "official_baseline_sha256": sha256_file(baseline_path),
    }
    for field, expected in current.items():
        if provenance[field] != expected:
            raise SystemExit(
                f"{path}: {field} does not match the checked benchmark contract"
            )
    print(f"cache query regression report validated ({report['status']}): {path}")


def write_receipt(report_path: Path, output_path: Path) -> None:
    report = load_json(report_path)
    validate_report_payload(report)
    if report.get("status") != "passed" or report.get("workload", {}).get("kind") != "fixture-matrix":
        raise SystemExit("a checked receipt requires a passed fixture-matrix report")
    raw_bytes = report_path.read_bytes()
    receipt = {key: value for key, value in report.items() if key not in {"schema", "runs"}}
    receipt.update(
        {
            "schema": RECEIPT_SCHEMA,
            "report_schema": SCHEMA,
            "raw_report": {
                "sha256": hashlib.sha256(raw_bytes).hexdigest(),
                "bytes": len(raw_bytes),
                "rows": len(report["runs"]),
                "retention": "local-target; regenerate with the checked harness",
            },
        }
    )
    output_path.parent.mkdir(parents=True, exist_ok=True)
    output_path.write_text(
        json.dumps(receipt, indent=2, sort_keys=True) + "\n", encoding="utf-8"
    )


def validate_receipt(path: Path, baseline_path: Path = DEFAULT_BASELINE) -> None:
    receipt = load_json(path)
    if (
        receipt.get("schema") != RECEIPT_SCHEMA
        or receipt.get("report_schema") != SCHEMA
        or receipt.get("status") != "passed"
    ):
        raise SystemExit(f"{path}: expected a passed cache-regression receipt")
    workload = receipt.get("workload")
    measurement = receipt.get("measurement")
    raw = receipt.get("raw_report")
    provenance = receipt.get("provenance")
    if not all(isinstance(value, dict) for value in (workload, measurement, raw, provenance)):
        raise SystemExit(f"{path}: incomplete receipt metadata")
    if workload != {"kind": "fixture-matrix", "ids": sorted(EXECUTABLE_CASES)}:
        raise SystemExit(f"{path}: incomplete fixture matrix")
    replays = measurement.get("replays")
    minimum = measurement.get("minimum_replays")
    if not isinstance(replays, int) or not isinstance(minimum, int) or replays < minimum:
        raise SystemExit(f"{path}: insufficient receipt replays")
    if raw.get("rows") != len(EXECUTABLE_CASES) * replays * 5:
        raise SystemExit(f"{path}: raw row count does not cover the matrix")
    if not isinstance(raw.get("bytes"), int) or raw["bytes"] <= 0:
        raise SystemExit(f"{path}: invalid raw report size")
    if re.fullmatch(r"[0-9a-f]{64}", str(raw.get("sha256"))) is None:
        raise SystemExit(f"{path}: invalid raw report seal")
    current = {
        "mutation_manifest_sha256": sha256_file(DEFAULT_MANIFEST),
        "official_baseline_sha256": sha256_file(baseline_path),
    }
    for field, expected in current.items():
        if provenance.get(field) != expected:
            raise SystemExit(f"{path}: {field} does not match the checked contract")
    if receipt.get("equivalence") != {
        "seed_clean_equals_empty_store": True,
        "after_clean_equals_empty_store": True,
        "after_clean_equals_history_store": True,
    }:
        raise SystemExit(f"{path}: receipt does not prove cache equivalence")
    identities = receipt.get("source_identity")
    summary = receipt.get("summary")
    if not isinstance(identities, dict) or set(identities) != EXECUTABLE_CASES:
        raise SystemExit(f"{path}: incomplete source identities")
    if not isinstance(summary, dict) or set(summary) != EXECUTABLE_CASES:
        raise SystemExit(f"{path}: incomplete fixture summaries")
    phases = {
        "clean-seed",
        "empty-store-seed",
        "clean-after",
        "empty-store-after",
        "history-after",
    }
    for case, case_summary in summary.items():
        if not isinstance(case_summary, dict) or set(case_summary) != phases:
            raise SystemExit(f"{path}: {case} has incomplete phase summaries")
        seed_hashes = {
            tuple(case_summary[phase].get("output_sha256", []))
            for phase in ("clean-seed", "empty-store-seed")
        }
        after_hashes = {
            tuple(case_summary[phase].get("output_sha256", []))
            for phase in ("clean-after", "empty-store-after", "history-after")
        }
        if len(seed_hashes) != 1 or len(after_hashes) != 1:
            raise SystemExit(f"{path}: {case} output identities are inconsistent")
        for phase, phase_summary in case_summary.items():
            require_fields(
                phase_summary,
                ("elapsed_ms", "peak_rss_bytes", "stages_ms", "output_sha256"),
                f"{path}: {case} {phase}",
            )
            if not phase_summary["stages_ms"]:
                raise SystemExit(f"{path}: {case} {phase} lacks stage timing")
    print(f"cache query regression receipt validated: {path}")


def run_paired_comparison(
    args: argparse.Namespace,
    manifest: dict[str, Any],
    baseline: dict[str, Any],
    candidate: Path,
    workspace: Path,
) -> None:
    if args.root is None:
        raise SystemExit("--compare-official-binary requires --root")
    if not args.compare_official_revision:
        raise SystemExit("--compare-official-revision is required for paired evidence")
    if args.official_target is None or args.official_archive is None:
        raise SystemExit(
            "--compare-official-binary requires --official-target and --official-archive"
        )
    official = args.compare_official_binary.resolve()
    if not official.is_file():
        raise SystemExit(f"missing official binary: {official}")
    official_verification = verify_official_binary(
        official,
        args.official_archive.resolve(),
        args.official_target,
        baseline,
    )
    repo = args.root.resolve()
    if not repo.is_dir():
        raise SystemExit(f"missing repository: {repo}")
    workload_id = args.label or repo.name
    expected = next(
        (
            row["commit"]
            for row in manifest["workloads"]["real"]
            if row["id"] == workload_id
        ),
        None,
    )
    if expected is None:
        raise SystemExit(f"{workload_id}: repository is not pinned by the mutation manifest")
    observed = repository_head(repo)
    if observed != expected:
        raise SystemExit(
            f"{workload_id}: repository HEAD {observed} does not match {expected}"
        )

    mutation = real_leaf_mutation(args)
    binaries = {"candidate": candidate, "official": official}
    rows: list[dict[str, Any]] = []
    equivalence = {"candidate": True, "official": True}
    source_identities = set()
    for replay in range(1, args.replays + 1):
        first_role = "candidate" if replay % 2 else "official"
        second_role = "official" if first_role == "candidate" else "candidate"
        for role in (first_role, second_role):
            if mutation is None:
                replay_rows, cold_equal, warm_equal = run_noop_replay(
                    binaries[role], workspace / role, repo, replay,
                    require_cache_stats=role == "candidate",
                )
            else:
                replay_rows, cold_equal, warm_equal, before_id, after_id = run_real_leaf_replay(
                    binaries[role], workspace / role, repo, mutation, replay,
                    require_cache_stats=role == "candidate",
                )
                source_identities.add((before_id, after_id))
            if not cold_equal:
                raise SystemExit(f"{role} cold no-op replay {replay}: output mismatch")
            if not warm_equal:
                raise SystemExit(f"{role} warm no-op replay {replay}: output mismatch")
            equivalence[role] &= cold_equal and warm_equal
            for row in replay_rows:
                row.update(
                    {
                        "binary_role": role,
                        "first_role": first_role,
                        "workload_id": workload_id,
                    }
                )
            rows.extend(replay_rows)
    if mutation is not None and len(source_identities) != 1:
        raise SystemExit("real leaf source identities varied across roles or replays")

    candidate_identity = binary_identity(candidate)
    official_identity = binary_identity(official)
    output = {
        "schema": COMPARISON_SCHEMA,
        "status": "passed",
        "workload": {
            "kind": "real",
            "id": workload_id,
            "path": portable_path(repo),
            "commit": observed,
            "mutation": (
                {
                    "kind": "single-leaf-replace",
                    "path": mutation.path.as_posix(),
                    "find_sha256": hashlib.sha256(mutation.find.encode()).hexdigest(),
                    "replace_sha256": hashlib.sha256(mutation.replace.encode()).hexdigest(),
                }
                if mutation is not None
                else None
            ),
        },
        "measurement": {
            "replays": args.replays,
            "minimum_replays": (
                args.replays if args.allow_short_run else manifest["minimum_replays"]
            ),
            "p95": "nearest-rank",
            "order": "alternating-ab-ba",
            "cache_stats_required": {"candidate": True, "official": False},
        },
        "provenance": {
            "candidate": {
                "binary": portable_path(candidate),
                "binary_sha256": candidate_identity.file_sha256,
                "binary_code_sha256": candidate_identity.code_sha256,
                "binary_code_sha256_algorithm": candidate_identity.code_sha256_algorithm,
                "binary_revision": args.binary_revision,
            },
            "official": {
                "binary": portable_path(official),
                "binary_sha256": official_identity.file_sha256,
                "binary_code_sha256": official_identity.code_sha256,
                "binary_code_sha256_algorithm": official_identity.code_sha256_algorithm,
                "binary_revision": args.compare_official_revision,
                "release_verification": official_verification,
            },
            "harness": "scripts/cache-query-regression.py",
            "harness_sha256": sha256_file(Path(__file__)),
            "mutation_manifest": portable_path(args.manifest),
            "mutation_manifest_sha256": sha256_file(args.manifest),
            "official_baseline": portable_path(args.official_baseline),
            "official_baseline_sha256": sha256_file(args.official_baseline),
        },
        "source_identity": (
            {
                "before": next(iter(source_identities))[0],
                "after": next(iter(source_identities))[1],
            }
            if mutation is not None
            else source_identity(repo)
        ),
        "equivalence": equivalence,
        "environment": measurement_environment(),
        "runs": rows,
        "summary": {
            role: summarize([row for row in rows if row["binary_role"] == role])
            for role in ("candidate", "official")
        },
    }
    validate_comparison_payload(output)
    args.output.parent.mkdir(parents=True, exist_ok=True)
    args.output.write_text(
        json.dumps(output, indent=2, sort_keys=True) + "\n", encoding="utf-8"
    )


def real_leaf_mutation(args: argparse.Namespace) -> RealLeafMutation | None:
    values = (args.leaf_path, args.leaf_find, args.leaf_replace)
    if not any(value is not None for value in values):
        return None
    if not all(value is not None for value in values):
        raise SystemExit("--leaf-path, --leaf-find, and --leaf-replace must be used together")
    path = Path(args.leaf_path)
    if path.is_absolute() or ".." in path.parts or path == Path("."):
        raise SystemExit("--leaf-path must be a safe repository-relative path")
    if args.leaf_find == args.leaf_replace:
        raise SystemExit("--leaf-find and --leaf-replace must differ")
    return RealLeafMutation(path=path, find=args.leaf_find, replace=args.leaf_replace)


def measurement_environment() -> dict[str, Any]:
    return {
        "architecture": platform.machine(),
        "logical_cpu_count": os.cpu_count(),
        "os": platform.system(),
        "os_release": platform.release(),
        "python_version": platform.python_version(),
    }


def self_test() -> None:
    output_self_test()
    validate_manifests(DEFAULT_MANIFEST, DEFAULT_BASELINE)
    assert nearest_rank_p95(list(range(1, 21))) == 19
    assert parse_cache_stats("  [cache] files=3 hits=1 misses=2 read_bytes=4 written_bytes=5") == {
        "files": 3, "hits": 1, "misses": 2, "read_bytes": 4, "written_bytes": 5
    }
    with tempfile.TemporaryDirectory() as temporary:
        root = Path(temporary)
        path = root / "a.py"
        path.write_text("x = 1\n", encoding="utf-8")
        first = source_identity(root)
        os.utime(path, ns=(1_000_000_000, 1_000_000_000))
        assert source_identity(root) == first
        path.write_text("x = 2\n", encoding="utf-8")
        assert source_identity(root) != first
    for case in EXECUTABLE_CASES:
        value = scenario(case)
        if case == "same-size-restored-mtime-edit":
            assert len(value.before["leaf.py"]) == len(value.after["leaf.py"])
    print("cache query regression self-test passed")


def parse_args() -> argparse.Namespace:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--binary", type=Path)
    source = parser.add_mutually_exclusive_group()
    source.add_argument("--fixture", choices=[*sorted(EXECUTABLE_CASES), "all"])
    source.add_argument("--root", type=Path)
    source.add_argument("--synthetic-files", type=int, choices=(1000, 10000, 100000))
    parser.add_argument("--label")
    parser.add_argument("--replays", type=int, default=30)
    parser.add_argument("--allow-short-run", action="store_true")
    parser.add_argument("--no-require-cache-stats", action="store_false", dest="require_cache_stats")
    parser.add_argument("--characterize-equivalence-failures", action="store_true")
    parser.add_argument("--manifest", type=Path, default=DEFAULT_MANIFEST)
    parser.add_argument("--official-baseline", type=Path, default=DEFAULT_BASELINE)
    parser.add_argument("--official-target")
    parser.add_argument("--official-archive", type=Path)
    parser.add_argument("--compare-official-binary", type=Path)
    parser.add_argument("--compare-official-revision")
    parser.add_argument("--leaf-path")
    parser.add_argument("--leaf-find")
    parser.add_argument("--leaf-replace")
    parser.add_argument("--binary-revision")
    parser.add_argument("--work-dir", type=Path, default=ROOT / "target/cache-query-regression")
    parser.add_argument("--output", type=Path)
    parser.add_argument("--validate-report", type=Path)
    parser.add_argument("--write-receipt", type=Path)
    parser.add_argument("--validate-receipt", type=Path)
    parser.add_argument("--self-test", action="store_true")
    return parser.parse_args()


def main() -> int:
    args = parse_args()
    if args.self_test:
        self_test()
        return 0
    if args.validate_report is not None:
        validate_report(args.validate_report.resolve(), args.official_baseline.resolve())
        return 0
    if args.write_receipt is not None:
        if args.output is None:
            raise SystemExit("--write-receipt requires --output")
        write_receipt(args.write_receipt.resolve(), args.output.resolve())
        return 0
    if args.validate_receipt is not None:
        validate_receipt(args.validate_receipt.resolve(), args.official_baseline.resolve())
        return 0
    if args.binary is None or args.output is None:
        raise SystemExit("--binary and --output are required")
    if args.fixture is None and args.root is None and args.synthetic_files is None:
        raise SystemExit("one of --fixture, --root, or --synthetic-files is required")
    if any(value is not None for value in (args.leaf_path, args.leaf_find, args.leaf_replace)):
        if args.compare_official_binary is None or args.root is None:
            raise SystemExit("real leaf measurement requires paired comparison with --root")
    manifest, baseline = validate_manifests(
        args.manifest.resolve(), args.official_baseline.resolve()
    )
    minimum = manifest["minimum_replays"]
    if args.replays <= 0 or (args.replays < minimum and not args.allow_short_run):
        raise SystemExit(f"--replays must be at least {minimum} (or use --allow-short-run for development)")
    binary = args.binary.resolve()
    if not binary.is_file():
        raise SystemExit(f"missing binary: {binary}")
    if not args.binary_revision:
        raise SystemExit("--binary-revision is required for reproducible evidence")
    if (args.official_target is None) != (args.official_archive is None):
        raise SystemExit("--official-target and --official-archive must be provided together")
    if args.output.exists():
        args.output.unlink()
    workspace = args.work_dir.resolve()
    workspace.mkdir(parents=True, exist_ok=True)
    if args.compare_official_binary is not None:
        run_paired_comparison(args, manifest, baseline, binary, workspace)
        return 0

    official_release_verification = None
    if args.official_target is not None:
        official_release_verification = verify_official_binary(
            binary,
            args.official_archive.resolve(),
            args.official_target,
            baseline,
        )
    if args.characterize_equivalence_failures and (
        official_release_verification is None or args.root is None
    ):
        raise SystemExit(
            "--characterize-equivalence-failures requires a verified official binary and --root"
        )
    rows: list[dict[str, Any]] = []
    cold_equivalent = True
    history_equivalent = True
    before_id: str
    after_id: str
    if args.fixture is not None:
        cases = sorted(EXECUTABLE_CASES) if args.fixture == "all" else [args.fixture]
        fixture_identities = {}
        for case in cases:
            value = scenario(case)
            identities = set()
            for replay in range(1, args.replays + 1):
                replay_rows, before_id, after_id = run_fixture_replay(
                    binary, workspace / case, value, replay, args.require_cache_stats
                )
                for row in replay_rows:
                    row["workload_id"] = case
                rows.extend(replay_rows)
                identities.add((before_id, after_id))
            if len(identities) != 1:
                raise SystemExit(f"{case}: fixture source identities varied across replays")
            before_id, after_id = next(iter(identities))
            fixture_identities[case] = {"before": before_id, "after": after_id}
        if args.fixture == "all":
            workload = {"kind": "fixture-matrix", "ids": cases}
            source_identities: dict[str, Any] = fixture_identities
        else:
            workload = {"kind": "fixture", "id": args.fixture}
            source_identities = fixture_identities[args.fixture]
    else:
        if args.root is not None:
            repo = args.root.resolve()
            if not repo.is_dir():
                raise SystemExit(f"missing repository: {repo}")
            workload_id = args.label or repo.name
            expected = next(
                (
                    row["commit"]
                    for row in manifest["workloads"]["real"]
                    if row["id"] == workload_id
                ),
                None,
            )
            if expected is None:
                raise SystemExit(f"{workload_id}: repository is not pinned by the mutation manifest")
            observed = repository_head(repo)
            if observed != expected:
                raise SystemExit(
                    f"{workload_id}: repository HEAD {observed} does not match {expected}"
                )
            workload = {
                "kind": "real",
                "id": workload_id,
                "path": portable_path(repo),
                "commit": observed,
            }
        else:
            repo = workspace / f"synthetic-{args.synthetic_files}"
            generate_synthetic(repo, args.synthetic_files)
            workload = {"kind": "synthetic", "id": f"synthetic-{args.synthetic_files}", "files": args.synthetic_files}
        before_id = after_id = source_identity(repo)
        for replay in range(1, args.replays + 1):
            replay_rows, replay_cold_equivalent, replay_history_equivalent = run_noop_replay(
                binary, workspace, repo, replay, args.require_cache_stats
            )
            cold_equivalent &= replay_cold_equivalent
            history_equivalent &= replay_history_equivalent
            if not args.characterize_equivalence_failures:
                if not replay_cold_equivalent:
                    raise SystemExit(f"cold no-op replay {replay}: output mismatch")
                if not replay_history_equivalent:
                    raise SystemExit(f"warm no-op replay {replay}: output mismatch")
            for row in replay_rows:
                row["workload_id"] = workload["id"]
            rows.extend(replay_rows)
        source_identities = {"before": before_id, "after": after_id}
    identity = binary_identity(binary)
    output = {
        "schema": SCHEMA,
        "status": (
            "passed"
            if cold_equivalent and history_equivalent
            else "failed-equivalence"
        ),
        "workload": workload,
        "measurement": {
            "replays": args.replays,
            "minimum_replays": args.replays if args.allow_short_run else minimum,
            "p95": "nearest-rank",
            "cache_stats_required": args.require_cache_stats,
            "characterization_only": args.characterize_equivalence_failures,
        },
        "provenance": {
            "binary": portable_path(binary),
            "binary_sha256": identity.file_sha256,
            "binary_code_sha256": identity.code_sha256,
            "binary_code_sha256_algorithm": identity.code_sha256_algorithm,
            "binary_revision": args.binary_revision,
            "harness": "scripts/cache-query-regression.py",
            "harness_sha256": sha256_file(Path(__file__)),
            "mutation_manifest": portable_path(args.manifest),
            "mutation_manifest_sha256": sha256_file(args.manifest),
            "official_baseline": portable_path(args.official_baseline),
            "official_baseline_sha256": sha256_file(args.official_baseline),
            "official_release_verification": official_release_verification,
        },
        "source_identity": source_identities,
        "equivalence": {
            "seed_clean_equals_empty_store": True if args.fixture is not None else None,
            "after_clean_equals_empty_store": cold_equivalent,
            "after_clean_equals_history_store": history_equivalent,
        },
        "environment": measurement_environment(),
        "runs": rows,
        "summary": summarize(rows),
    }
    args.output.parent.mkdir(parents=True, exist_ok=True)
    args.output.write_text(json.dumps(output, indent=2, sort_keys=True) + "\n", encoding="utf-8")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
