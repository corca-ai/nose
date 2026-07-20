#!/usr/bin/env python3
"""Measure active query-watch latency and clean-query equivalence."""

from __future__ import annotations

import argparse
import hashlib
import json
import math
import os
import platform
import selectors
import signal
import statistics
import subprocess
import tempfile
import time
from pathlib import Path
from typing import Any


ROOT = Path(__file__).resolve().parents[1]
SCHEMA = "nose.query_watch_benchmark/v1"
WATCH_SCHEMA = "nose.query-watch/v1"
BASELINE = ROOT / "bench/cache/official-v0.19.0-binaries.v1.json"
ONE_SHOT = ROOT / "bench/cache/issue-877-policy-leaf-sympy-paired-2026-07-21.v1.json"
TARGETS = {10_000: 250.0, 100_000: 1_000.0}


def sha256_file(path: Path) -> str:
    digest = hashlib.sha256()
    with path.open("rb") as handle:
        for block in iter(lambda: handle.read(1024 * 1024), b""):
            digest.update(block)
    return digest.hexdigest()


def target_triple() -> str:
    machine = platform.machine().lower()
    if platform.system() == "Darwin":
        arch = "aarch64" if machine in {"arm64", "aarch64"} else "x86_64"
        return f"{arch}-apple-darwin"
    if platform.system() == "Linux":
        arch = "aarch64" if machine in {"arm64", "aarch64"} else "x86_64"
        return f"{arch}-unknown-linux-gnu"
    raise SystemExit(f"unsupported benchmark platform: {platform.system()} {machine}")


def load_object(path: Path) -> dict[str, Any]:
    try:
        value = json.loads(path.read_text(encoding="utf-8"))
    except (OSError, json.JSONDecodeError) as error:
        raise SystemExit(f"cannot read {path}: {error}") from error
    if not isinstance(value, dict):
        raise SystemExit(f"{path}: expected a JSON object")
    return value


def official_identity(archive: Path | None, binary: Path | None) -> dict[str, Any]:
    manifest = load_object(BASELINE)
    triple = target_triple()
    row = next(
        (item for item in manifest.get("artifacts", []) if item.get("target") == triple),
        None,
    )
    if row is None:
        raise SystemExit(f"official baseline lacks {triple}")
    default_root = ROOT / "target/issue-875-baseline"
    archive = archive or default_root / row["archive"]
    binary = binary or default_root / row["archive"].removesuffix(".tar.xz") / "nose"
    archive = archive.resolve()
    binary = binary.resolve()
    for path, field in ((archive, "archive_sha256"), (binary, "binary_sha256")):
        if not path.is_file():
            raise SystemExit(f"missing official v0.19.0 {field}: {path}")
        actual = sha256_file(path)
        if actual != row[field]:
            raise SystemExit(f"official v0.19.0 {field} mismatch: {actual} != {row[field]}")
    return {
        "target": triple,
        "archive": str(archive.relative_to(ROOT)),
        "archive_sha256": row["archive_sha256"],
        "binary": str(binary.relative_to(ROOT)),
        "binary_sha256": row["binary_sha256"],
        "manifest": str(BASELINE.relative_to(ROOT)),
        "manifest_sha256": sha256_file(BASELINE),
        "verified": True,
    }


def generate(root: Path, files: int) -> None:
    for index in range(files):
        shard = root / f"s{index // 1_000:03d}"
        shard.mkdir(parents=True, exist_ok=True)
        (shard / f"f{index:06d}.py").write_text(
            f"def value_{index}(x):\n    return x + {index}\n", encoding="utf-8"
        )


def query_command(binary: Path, *, watch: bool, cache: Path | None = None) -> list[str]:
    command = [
        str(binary),
        "query",
        "repo",
        "--format",
        "jsonl" if watch else "json",
        "--mode",
        "semantic",
        "--min-size",
        "1",
        "--min-lines",
        "1",
    ]
    if watch:
        command.append("--watch")
    if cache is not None:
        command.extend(("--cache-dir", str(cache)))
    return command


def clean_snapshot(binary: Path, workspace: Path) -> dict[str, Any]:
    result = subprocess.run(
        query_command(binary, watch=False),
        cwd=workspace,
        stdout=subprocess.PIPE,
        stderr=subprocess.PIPE,
        check=False,
    )
    if result.returncode != 0:
        raise SystemExit(result.stderr.decode(errors="replace"))
    value = json.loads(result.stdout)
    if not isinstance(value, dict):
        raise SystemExit("clean query did not return an object")
    return value


def seed_cache(binary: Path, workspace: Path, cache: Path) -> None:
    result = subprocess.run(
        query_command(binary, watch=False, cache=cache),
        cwd=workspace,
        stdout=subprocess.DEVNULL,
        stderr=subprocess.PIPE,
        check=False,
    )
    if result.returncode != 0:
        raise SystemExit(f"cache seed failed: {result.stderr.decode(errors='replace')}")


def start_watch(binary: Path, workspace: Path, cache: Path) -> subprocess.Popen[str]:
    return subprocess.Popen(
        query_command(binary, watch=True, cache=cache),
        cwd=workspace,
        stdout=subprocess.PIPE,
        stderr=subprocess.PIPE,
        text=True,
        start_new_session=True,
    )


def read_record(process: subprocess.Popen[str], timeout: float = 300.0) -> dict[str, Any]:
    assert process.stdout is not None
    selector = selectors.DefaultSelector()
    selector.register(process.stdout, selectors.EVENT_READ)
    try:
        ready = selector.select(timeout)
    finally:
        selector.close()
    if not ready:
        status = process.poll()
        error = ""
        if status is not None and process.stderr is not None:
            error = process.stderr.read()
        raise SystemExit(f"watch record timeout; status={status}; stderr={error}")
    line = process.stdout.readline()
    if not line:
        error = process.stderr.read() if process.stderr is not None else ""
        raise SystemExit(f"watch exited before a record: {process.poll()}; {error}")
    value = json.loads(line)
    if not isinstance(value, dict) or value.get("schema") != WATCH_SCHEMA:
        raise SystemExit("watch emitted an unsupported record")
    return value


def stop_watch(process: subprocess.Popen[str], *, crash: bool = False) -> None:
    if process.poll() is not None:
        return
    os.killpg(process.pid, signal.SIGKILL if crash else signal.SIGINT)
    try:
        process.wait(timeout=10)
    except subprocess.TimeoutExpired:
        os.killpg(process.pid, signal.SIGKILL)
        process.wait()


def rss_bytes(pid: int) -> int:
    result = subprocess.run(
        ["ps", "-o", "rss=", "-p", str(pid)],
        stdout=subprocess.PIPE,
        stderr=subprocess.DEVNULL,
        text=True,
        check=False,
    )
    value = result.stdout.strip()
    return int(value) * 1024 if result.returncode == 0 and value else 0


def tree_usage(root: Path) -> dict[str, int]:
    paths = [path for path in root.rglob("*") if path.is_file()]
    return {"files": len(paths), "bytes": sum(path.stat().st_size for path in paths)}


def canonical_digest(value: dict[str, Any]) -> str:
    payload = json.dumps(value, sort_keys=True, separators=(",", ":")).encode()
    return hashlib.sha256(payload).hexdigest()


def p95(values: list[float]) -> float:
    ordered = sorted(values)
    return ordered[math.ceil(0.95 * len(ordered)) - 1]


def mutate(path: Path, index: int, negative: bool) -> None:
    operator = "-" if negative else "+"
    path.write_text(
        f"def value_{index}(x):\n    return x {operator} {index}\n", encoding="utf-8"
    )


def run_tier(binary: Path, files: int, replays: int, workspace: Path) -> dict[str, Any]:
    print(f"[{files}] generate", flush=True)
    repo = workspace / "repo"
    cache = workspace / "cache"
    repo.mkdir(parents=True)
    generate(repo, files)
    print(f"[{files}] seed cache", flush=True)
    seed_cache(binary, workspace, cache)
    print(f"[{files}] start session", flush=True)
    process = start_watch(binary, workspace, cache)
    initial = read_record(process)
    if initial.get("sequence") != 0 or initial.get("snapshot") != clean_snapshot(binary, workspace):
        stop_watch(process, crash=True)
        raise SystemExit(f"{files}: initial watch snapshot differs from a clean query")
    rows: list[dict[str, Any]] = []
    peak_rss = rss_bytes(process.pid)
    crash_recovery = False
    try:
        for replay in range(1, replays + 1):
            index = files - 1
            started = time.perf_counter()
            mutate(repo / f"s{index // 1_000:03d}" / f"f{index:06d}.py", index, replay % 2 == 1)
            record = read_record(process)
            end_to_end_ms = (time.perf_counter() - started) * 1_000.0
            clean = clean_snapshot(binary, workspace)
            if record.get("snapshot") != clean:
                raise SystemExit(f"{files} replay {replay}: watch snapshot differs from clean")
            current_rss = rss_bytes(process.pid)
            peak_rss = max(peak_rss, current_rss)
            rows.append(
                {
                    "replay": replay,
                    "sequence": record.get("sequence"),
                    "latency_ms": record.get("latency_ms"),
                    "end_to_end_ms": end_to_end_ms,
                    "reconciliation": record.get("reconciliation"),
                    "source_set_digest": record.get("source_set_digest"),
                    "snapshot_sha256": canonical_digest(clean),
                    "rss_bytes": current_rss,
                    "equivalent_to_clean": True,
                }
            )
            if replay == 1 or replay % 10 == 0:
                print(
                    f"[{files}] replay {replay}/{replays}: {record.get('latency_ms')}ms",
                    flush=True,
                )
            if replay == replays // 2:
                print(f"[{files}] crash/restart", flush=True)
                stop_watch(process, crash=True)
                process = start_watch(binary, workspace, cache)
                restarted = read_record(process)
                crash_recovery = restarted.get("snapshot") == clean_snapshot(binary, workspace)
                if not crash_recovery:
                    raise SystemExit(f"{files}: crash restart differs from clean")
                peak_rss = max(peak_rss, rss_bytes(process.pid))
    finally:
        stop_watch(process)
    latencies = [float(row["latency_ms"]) for row in rows]
    end_to_end = [float(row["end_to_end_ms"]) for row in rows]
    source_bytes = sum(path.stat().st_size for path in repo.rglob("*.py"))
    return {
        "files": files,
        "replays": replays,
        "target_p95_ms": TARGETS[files],
        "latency_ms": {"p50": statistics.median(latencies), "p95": p95(latencies)},
        "end_to_end_ms": {"p50": statistics.median(end_to_end), "p95": p95(end_to_end)},
        "peak_rss_bytes": peak_rss,
        "source_bytes": source_bytes,
        "store": tree_usage(cache),
        "crash_recovery": crash_recovery,
        "equivalent_to_clean": all(row["equivalent_to_clean"] for row in rows),
        "passed": p95(latencies) <= TARGETS[files] and crash_recovery,
        "rows": rows,
    }


def validate_report(path: Path) -> None:
    report = load_object(path)
    if report.get("schema") != SCHEMA or report.get("status") != "pass":
        raise SystemExit(f"{path}: watch benchmark is not passing")
    if not report.get("provenance", {}).get("official_v0_19", {}).get("verified"):
        raise SystemExit(f"{path}: official v0.19.0 binary was not verified")
    tiers = report.get("tiers")
    if not isinstance(tiers, list) or [tier.get("files") for tier in tiers] != list(TARGETS):
        raise SystemExit(f"{path}: expected 10k and 100k tiers")
    for tier in tiers:
        if tier.get("replays", 0) < 30 or not tier.get("equivalent_to_clean"):
            raise SystemExit(f"{path}: incomplete clean-equivalence evidence")
        if not tier.get("crash_recovery") or tier["latency_ms"]["p95"] > TARGETS[tier["files"]]:
            raise SystemExit(f"{path}: latency or crash-recovery gate failed")


def self_test() -> None:
    assert p95([float(value) for value in range(1, 21)]) == 19.0
    with tempfile.TemporaryDirectory() as temp:
        root = Path(temp)
        generate(root, 3)
        assert len(list(root.rglob("*.py"))) == 3
        mutate(root / "s000/f000002.py", 2, True)
        assert "x - 2" in (root / "s000/f000002.py").read_text(encoding="utf-8")
    print("watch session benchmark self-test passed")


def main() -> None:
    parser = argparse.ArgumentParser()
    parser.add_argument("--binary", type=Path, default=ROOT / "target/release/nose")
    parser.add_argument("--output", type=Path)
    parser.add_argument("--replays", type=int, default=30)
    parser.add_argument("--official-archive", type=Path)
    parser.add_argument("--official-binary", type=Path)
    parser.add_argument("--validate-report", type=Path)
    parser.add_argument("--self-test", action="store_true")
    args = parser.parse_args()
    if args.self_test:
        self_test()
        return
    if args.validate_report:
        validate_report(args.validate_report)
        print(f"watch session benchmark validated: {args.validate_report}")
        return
    if args.output is None or args.replays < 30 or not args.binary.is_file():
        raise SystemExit("measurement requires --output, --replays >=30, and an existing --binary")
    one_shot = load_object(ONE_SHOT)
    with tempfile.TemporaryDirectory(prefix="nose-watch-benchmark-") as temp:
        tiers = [
            run_tier(args.binary.resolve(), files, args.replays, Path(temp) / str(files))
            for files in TARGETS
        ]
    report = {
        "schema": SCHEMA,
        "status": "pass" if all(tier["passed"] for tier in tiers) else "fail",
        "environment": {
            "os": platform.system(),
            "os_release": platform.release(),
            "architecture": platform.machine(),
            "logical_cpu_count": os.cpu_count(),
        },
        "measurement": {
            "minimum_replays": 30,
            "replays": args.replays,
            "p95": "nearest-rank",
            "snapshot_equivalence": "parsed-full-dashboard-equality",
        },
        "provenance": {
            "candidate_binary": str(args.binary),
            "candidate_binary_sha256": sha256_file(args.binary),
            "candidate_revision": subprocess.run(
                ["git", "rev-parse", "HEAD"], cwd=ROOT, text=True,
                stdout=subprocess.PIPE, check=True,
            ).stdout.strip(),
            "harness": str(Path(__file__).relative_to(ROOT)),
            "harness_sha256": sha256_file(Path(__file__)),
            "official_v0_19": official_identity(args.official_archive, args.official_binary),
            "one_shot_evidence": {
                "path": str(ONE_SHOT.relative_to(ROOT)),
                "sha256": sha256_file(ONE_SHOT),
                "same_binary_equivalence": one_shot.get("equivalence"),
                "status": one_shot.get("status"),
            },
        },
        "tiers": tiers,
    }
    args.output.parent.mkdir(parents=True, exist_ok=True)
    args.output.write_text(json.dumps(report, indent=2, sort_keys=True) + "\n", encoding="utf-8")
    validate_report(args.output)
    print(json.dumps({"status": report["status"], "output": str(args.output)}, sort_keys=True))


if __name__ == "__main__":
    main()
