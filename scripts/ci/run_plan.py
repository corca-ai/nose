#!/usr/bin/env python3
"""Run a registry-defined local CI plan with bounded parallelism."""

from __future__ import annotations

import argparse
import hashlib
import os
import signal
import subprocess
import sys
import tempfile
import time
from dataclasses import dataclass
from pathlib import Path
from typing import Any, BinaryIO

import gate_registry


ROOT = Path(__file__).resolve().parents[2]
CHECK_SCRIPT = ROOT / "scripts/check-ci-local.sh"
MINIMUM_PYTHON = (3, 10)


@dataclass
class RunningGate:
    index: int
    row: dict[str, Any]
    process: subprocess.Popen[bytes]
    log_path: Path
    log_stream: BinaryIO
    started: float


@dataclass(frozen=True)
class GateResult:
    status: int
    seconds: float
    log_path: Path


def worktree_fingerprint() -> str:
    fingerprint = hashlib.sha256()
    for command in (
        ["git", "status", "--porcelain=v1", "-z", "--untracked-files=all"],
        ["git", "diff", "--no-ext-diff", "--binary", "--"],
        ["git", "diff", "--cached", "--no-ext-diff", "--binary", "--"],
    ):
        output = subprocess.run(
            command,
            cwd=ROOT,
            check=True,
            stdout=subprocess.PIPE,
        ).stdout
        fingerprint.update(len(output).to_bytes(8, "big"))
        fingerprint.update(output)
    untracked = subprocess.run(
        ["git", "ls-files", "--others", "--exclude-standard", "-z"],
        cwd=ROOT,
        check=True,
        stdout=subprocess.PIPE,
    ).stdout
    for raw_path in untracked.split(b"\0"):
        if not raw_path:
            continue
        fingerprint.update(raw_path)
        fingerprint.update((ROOT / os.fsdecode(raw_path)).read_bytes())
    return fingerprint.hexdigest()


def can_start(
    row: dict[str, Any],
    active_rows: list[dict[str, Any]],
    completed: dict[str, int],
) -> bool:
    if any(completed.get(dependency) != 0 for dependency in row["depends_on"]):
        return False
    if not row["parallel_safe"]:
        return not active_rows
    if any(not active["parallel_safe"] for active in active_rows):
        return False
    resource_group = row["resource_group"]
    return resource_group is None or all(
        active["resource_group"] != resource_group for active in active_rows
    )


def next_startable_position(
    pending: list[tuple[int, dict[str, Any]]],
    active_rows: list[dict[str, Any]],
    completed: dict[str, int],
) -> int | None:
    for position, (_, row) in enumerate(pending):
        if can_start(row, active_rows, completed):
            return position
        if not row["parallel_safe"]:
            return None
    return None


def emit_result(row: dict[str, Any], result: GateResult) -> None:
    verdict = "passed" if result.status == 0 else f"failed ({result.status})"
    print(
        f"\n\033[1m== {row['label']}: {verdict} in {result.seconds:.1f}s ==\033[0m",
        flush=True,
    )
    with result.log_path.open("rb") as stream:
        while chunk := stream.read(1024 * 1024):
            sys.stdout.buffer.write(chunk)
    sys.stdout.buffer.flush()


def run_plan(
    rows: list[dict[str, Any]],
    jobs: int,
    *,
    check_script: Path = CHECK_SCRIPT,
) -> int:
    before = worktree_fingerprint()
    completed: dict[str, int] = {}
    results: dict[int, GateResult] = {}
    pending = list(enumerate(rows))
    running: dict[int, RunningGate] = {}
    next_to_emit = 0
    first_failure = 0

    with tempfile.TemporaryDirectory(prefix="nose-ci-plan-") as directory:
        log_dir = Path(directory)
        try:
            while pending or running:
                if first_failure == 0:
                    while len(running) < jobs:
                        active_rows = [gate.row for gate in running.values()]
                        candidate = next_startable_position(
                            pending,
                            active_rows,
                            completed,
                        )
                        if candidate is None:
                            break
                        index, row = pending.pop(candidate)
                        log_path = log_dir / f"{index:03d}-{row['name']}.log"
                        log_stream = log_path.open("wb")
                        command = [
                            str(check_script),
                            "--gate",
                            row["name"],
                            *row["args"],
                        ]
                        process = subprocess.Popen(
                            command,
                            cwd=ROOT,
                            stdout=log_stream,
                            stderr=subprocess.STDOUT,
                            start_new_session=True,
                        )
                        running[index] = RunningGate(
                            index=index,
                            row=row,
                            process=process,
                            log_path=log_path,
                            log_stream=log_stream,
                            started=time.perf_counter(),
                        )
                        print(
                            f"\033[1m== started: {row['label']} ==\033[0m",
                            flush=True,
                        )

                if not running:
                    if pending:
                        if first_failure == 0:
                            blocked = ", ".join(row["name"] for _, row in pending)
                            print(
                                f"local CI plan could not schedule: {blocked}",
                                file=sys.stderr,
                            )
                            first_failure = 1
                        else:
                            skipped = ", ".join(row["name"] for _, row in pending)
                            print(
                                f"local CI gates not run after failure: {skipped}",
                                file=sys.stderr,
                            )
                    break

                while True:
                    finished: list[int] = []
                    for index, gate in running.items():
                        status = gate.process.poll()
                        if status is None:
                            continue
                        gate.log_stream.close()
                        results[index] = GateResult(
                            status=status,
                            seconds=time.perf_counter() - gate.started,
                            log_path=gate.log_path,
                        )
                        completed[gate.row["name"]] = status
                        if status != 0 and first_failure == 0:
                            first_failure = status
                        finished.append(index)
                    for index in finished:
                        del running[index]
                    if finished:
                        break
                    time.sleep(0.05)

                while next_to_emit in results:
                    emit_result(rows[next_to_emit], results[next_to_emit])
                    next_to_emit += 1
        except KeyboardInterrupt:
            for gate in running.values():
                try:
                    os.killpg(gate.process.pid, signal.SIGTERM)
                except ProcessLookupError:
                    pass
            for gate in running.values():
                gate.process.wait()
                gate.log_stream.close()
            print("local CI plan interrupted", file=sys.stderr)
            return 130

        for index in sorted(results):
            if index >= next_to_emit:
                emit_result(rows[index], results[index])

    after = worktree_fingerprint()
    if before != after:
        print("local CI plan changed the worktree", file=sys.stderr)
        if first_failure == 0:
            first_failure = 1
    return first_failure


def self_test() -> None:
    base = {
        "depends_on": [],
        "parallel_safe": True,
        "resource_group": None,
    }
    first = {**base, "name": "first", "resource_group": "shared"}
    same_group = {**base, "name": "same-group", "resource_group": "shared"}
    independent = {**base, "name": "independent"}
    dependent = {**base, "name": "dependent", "depends_on": ["first"]}
    exclusive = {**base, "name": "exclusive", "parallel_safe": False}

    assert can_start(first, [], {})
    assert not can_start(same_group, [first], {})
    assert can_start(independent, [first], {})
    assert not can_start(dependent, [], {})
    assert can_start(dependent, [], {"first": 0})
    assert not can_start(dependent, [], {"first": 1})
    assert not can_start(exclusive, [first], {})
    assert not can_start(first, [exclusive], {})
    pending = [(0, exclusive), (1, independent)]
    assert next_startable_position(pending, [first], {}) is None
    with tempfile.TemporaryDirectory(prefix="nose-ci-plan-selftest-") as directory:
        dispatcher = Path(directory) / "gate"
        dispatcher.write_text(
            "#!/usr/bin/env bash\n"
            "if [[ \"$2\" == \"fail\" ]]; then exit 7; fi\n"
            "printf 'fake gate passed: %s\\n' \"$2\"\n"
        )
        dispatcher.chmod(0o755)
        row = {
            **base,
            "name": "pass",
            "label": "passing fake gate",
            "args": [],
        }
        assert run_plan([row], 2, check_script=dispatcher) == 0
        failed = {
            **exclusive,
            "name": "fail",
            "label": "failing fake gate",
            "args": [],
        }
        assert run_plan([failed], 2, check_script=dispatcher) == 7
    print("local CI parallel planner self-test passed")


def parse_args() -> argparse.Namespace:
    parser = argparse.ArgumentParser()
    parser.add_argument("--mode", choices=("fast", "full"))
    parser.add_argument("--jobs", type=int, default=1)
    parser.add_argument("--self-test", action="store_true")
    return parser.parse_args()


def main() -> int:
    if sys.version_info < MINIMUM_PYTHON:
        observed = ".".join(str(part) for part in sys.version_info[:3])
        print(
            "local CI parallel planner requires Python 3.10 or newer "
            f"(observed {observed})",
            file=sys.stderr,
        )
        return 127
    args = parse_args()
    if args.self_test:
        self_test()
        return 0
    if args.mode is None:
        print("--mode is required", file=sys.stderr)
        return 2
    if args.jobs <= 0:
        print("--jobs must be a positive integer", file=sys.stderr)
        return 2
    try:
        gates = gate_registry.validate_live_registry(gate_registry.load_registry())
        rows = gate_registry.plan_rows(gates, args.mode)
    except (OSError, gate_registry.RegistryError) as exc:
        print(f"local CI plan error: {exc}", file=sys.stderr)
        return 1
    return run_plan(rows, args.jobs)


if __name__ == "__main__":
    raise SystemExit(main())
