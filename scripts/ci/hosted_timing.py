#!/usr/bin/env python3
"""Build and validate GitHub-hosted CI timing receipts from Actions metadata."""

from __future__ import annotations

import argparse
import json
import math
import platform
import re
import sys
from datetime import datetime
from pathlib import Path
from typing import Any

import gate_registry


ROOT = Path(__file__).resolve().parents[2]
SCHEMA = "nose.hosted-ci-timings.v1"
MINIMUM_PYTHON = (3, 10)
TIMING_JOB_NAME = "hosted CI timing"


class TimingError(ValueError):
    """Raised when hosted timing input or output violates its contract."""


def parse_timestamp(value: Any, context: str) -> datetime:
    if not isinstance(value, str) or not value:
        raise TimingError(f"{context} must be an ISO-8601 timestamp")
    try:
        return datetime.fromisoformat(value.replace("Z", "+00:00"))
    except ValueError as exc:
        raise TimingError(f"{context} must be an ISO-8601 timestamp") from exc


def elapsed_seconds(started: Any, completed: Any, context: str) -> float | None:
    if started is None and completed is None:
        return None
    start = parse_timestamp(started, f"{context}.started_at")
    finish = parse_timestamp(completed, f"{context}.completed_at")
    seconds = (finish - start).total_seconds()
    if seconds < 0:
        raise TimingError(f"{context} completed before it started")
    return round(seconds, 3)


def load_object(path: Path, context: str) -> dict[str, Any]:
    try:
        value = json.loads(path.read_text())
    except (OSError, json.JSONDecodeError) as exc:
        raise TimingError(f"cannot load {context} {path}: {exc}") from exc
    if not isinstance(value, dict):
        raise TimingError(f"{context} must be a JSON object")
    return value


def percentile(values: list[float], fraction: float) -> float | None:
    if not values:
        return None
    ordered = sorted(values)
    index = max(0, math.ceil(fraction * len(ordered)) - 1)
    return round(ordered[index], 3)


def is_timing_job_name(name: Any) -> bool:
    return isinstance(name, str) and (
        name == TIMING_JOB_NAME or name.endswith(f" / {TIMING_JOB_NAME}")
    )


def scoped_quality_jobs(
    jobs_payload: dict[str, Any],
    context: str,
    *,
    require_timing_job: bool,
) -> list[dict[str, Any]]:
    raw_jobs = jobs_payload.get("jobs")
    if not isinstance(raw_jobs, list):
        raise TimingError(f"{context} must contain jobs")
    if any(not isinstance(job, dict) for job in raw_jobs):
        raise TimingError(f"{context}.jobs must contain only objects")

    timing_jobs = [job for job in raw_jobs if is_timing_job_name(job.get("name"))]
    if len(timing_jobs) > 1:
        raise TimingError(f"{context} contains multiple hosted timing jobs")
    if not timing_jobs:
        if require_timing_job:
            raise TimingError(f"{context} does not contain the hosted timing job")
        return raw_jobs

    timing_name = timing_jobs[0]["name"]
    prefix = timing_name[: -len(TIMING_JOB_NAME)]
    return [
        job
        for job in raw_jobs
        if job is not timing_jobs[0]
        and isinstance(job.get("name"), str)
        and job["name"].startswith(prefix)
    ]


def quality_job_seconds(
    jobs_payload: dict[str, Any],
    context: str,
    *,
    require_timing_job: bool,
) -> float:
    raw_jobs = scoped_quality_jobs(
        jobs_payload,
        context,
        require_timing_job=require_timing_job,
    )
    windows = [
        (
            parse_timestamp(job["started_at"], f"{context}.{index}.started_at"),
            parse_timestamp(job["completed_at"], f"{context}.{index}.completed_at"),
        )
        for index, job in enumerate(raw_jobs)
        if job.get("conclusion") != "skipped"
        if job.get("started_at") is not None
        and job.get("completed_at") is not None
    ]
    if not windows:
        raise TimingError(f"{context} has no completed quality jobs")
    first_start = min(start for start, _ in windows)
    last_finish = max(end for _, end in windows)
    return round((last_finish - first_start).total_seconds(), 3)


def history_summary(
    runs_payload: dict[str, Any],
    history_jobs: dict[int, dict[str, Any]],
    expected_event: str,
) -> dict[str, Any]:
    rows = runs_payload.get("workflow_runs")
    if not isinstance(rows, list):
        raise TimingError("workflow runs payload must contain workflow_runs")
    samples: list[dict[str, Any]] = []
    for index, row in enumerate(rows):
        if not isinstance(row, dict):
            raise TimingError(f"workflow_runs[{index}] must be an object")
        if row.get("status") != "completed" or row.get("conclusion") != "success":
            continue
        if row.get("event") != expected_event:
            continue
        run_id = row.get("id")
        if not isinstance(run_id, int) or run_id not in history_jobs:
            continue
        try:
            seconds = quality_job_seconds(
                history_jobs[run_id],
                f"workflow_runs[{index}].jobs",
                require_timing_job=expected_event == "workflow_call",
            )
        except TimingError:
            # Old reusable-workflow runs have no timing job from which to derive
            # the called-job prefix, so they are not comparable samples.
            continue
        samples.append(
            {
                "run_id": run_id,
                "head_sha": row.get("head_sha"),
                "event": row.get("event"),
                "seconds": seconds,
            }
        )
    durations = [sample["seconds"] for sample in samples]
    return {
        "sample_count": len(samples),
        "p50_seconds": percentile(durations, 0.50),
        "p95_seconds": percentile(durations, 0.95),
        "samples": samples,
    }


def normalized_step(
    raw: dict[str, Any],
    *,
    job_name: str,
    index: int,
) -> dict[str, Any]:
    name = raw.get("name")
    if not isinstance(name, str) or not name:
        raise TimingError(f"job {job_name} step {index} must have a name")
    seconds = elapsed_seconds(
        raw.get("started_at"),
        raw.get("completed_at"),
        f"job {job_name} step {name}",
    )
    gate = (
        name.removeprefix(gate_registry.HOSTED_GATE_PREFIX)
        if name.startswith(gate_registry.HOSTED_GATE_PREFIX)
        else None
    )
    return {
        "number": raw.get("number"),
        "name": name,
        "gate": gate,
        "conclusion": raw.get("conclusion"),
        "started_at": raw.get("started_at"),
        "completed_at": raw.get("completed_at"),
        "seconds": seconds,
        "timing_available": seconds is not None,
    }


def normalized_job(raw: dict[str, Any], index: int) -> dict[str, Any]:
    name = raw.get("name")
    if not isinstance(name, str) or not name:
        raise TimingError(f"jobs[{index}] must have a name")
    conclusion = raw.get("conclusion")
    # GitHub may give skipped jobs synthetic timestamps with completion before
    # start. Preserve those API values for auditability, but do not present
    # them as elapsed timing data.
    seconds = (
        None
        if conclusion == "skipped"
        else elapsed_seconds(
            raw.get("started_at"),
            raw.get("completed_at"),
            f"job {name}",
        )
    )
    raw_steps = raw.get("steps")
    if not isinstance(raw_steps, list):
        raise TimingError(f"job {name} must contain steps")
    steps = [
        normalized_step(step, job_name=name, index=step_index)
        for step_index, step in enumerate(raw_steps)
        if isinstance(step, dict)
    ]
    return {
        "id": raw.get("id"),
        "name": name,
        "conclusion": conclusion,
        "started_at": raw.get("started_at"),
        "completed_at": raw.get("completed_at"),
        "seconds": seconds,
        "timing_available": seconds is not None,
        "runner_name": raw.get("runner_name"),
        "runner_group_name": raw.get("runner_group_name"),
        "labels": raw.get("labels", []),
        "steps": steps,
    }


def observed_conclusion(jobs: list[dict[str, Any]]) -> str:
    conclusions = {job["conclusion"] for job in jobs}
    for conclusion in ("failure", "cancelled", "timed_out", "action_required"):
        if conclusion in conclusions:
            return conclusion
    if conclusions <= {"success", "skipped", "neutral"}:
        return "success"
    return "unknown"


def read_checked_toolchains(root: Path) -> dict[str, str]:
    rust_text = (root / "rust-toolchain.toml").read_text()
    rust_match = re.search(r'^channel\s*=\s*"([^"]+)"', rust_text, re.MULTILINE)
    cargo_text = (root / "Cargo.toml").read_text()
    msrv_match = re.search(r'^rust-version\s*=\s*"([^"]+)"', cargo_text, re.MULTILINE)
    if rust_match is None or msrv_match is None:
        raise TimingError("cannot read checked Rust toolchain identities")
    return {
        "rust": rust_match.group(1),
        "msrv": msrv_match.group(1),
        "lean": (root / "lean-toolchain").read_text().strip(),
        "collector_python": platform.python_version(),
    }


def collect_receipt(
    jobs_payload: dict[str, Any],
    runs_payload: dict[str, Any],
    history_jobs: dict[int, dict[str, Any]],
    *,
    metadata: dict[str, Any],
    expected_gates: set[str],
    toolchains: dict[str, str],
) -> dict[str, Any]:
    raw_jobs = scoped_quality_jobs(
        jobs_payload,
        "jobs payload",
        require_timing_job=True,
    )
    jobs = [
        normalized_job(raw, index)
        for index, raw in enumerate(raw_jobs)
    ]
    if not jobs:
        raise TimingError("jobs payload has no completed quality jobs")

    timed_jobs = [job for job in jobs if job["timing_available"]]
    if not timed_jobs:
        raise TimingError("jobs payload has no timing data")
    workflow_start = min(
        parse_timestamp(job["started_at"], f"job {job['name']}.started_at")
        for job in timed_jobs
    )
    workflow_end = max(
        parse_timestamp(job["completed_at"], f"job {job['name']}.completed_at")
        for job in timed_jobs
    )
    critical_job = max(
        timed_jobs,
        key=lambda job: parse_timestamp(
            job["completed_at"], f"job {job['name']}.completed_at"
        ),
    )
    for job in jobs:
        if job["timing_available"]:
            started = parse_timestamp(
                job["started_at"], f"job {job['name']}.started_at"
            )
            completed = parse_timestamp(
                job["completed_at"], f"job {job['name']}.completed_at"
            )
            job["start_offset_seconds"] = round(
                (started - workflow_start).total_seconds(), 3
            )
            job["completion_offset_seconds"] = round(
                (completed - workflow_start).total_seconds(), 3
            )
            job["completion_slack_seconds"] = round(
                (workflow_end - completed).total_seconds(), 3
            )
        else:
            job["start_offset_seconds"] = None
            job["completion_offset_seconds"] = None
            job["completion_slack_seconds"] = None
        job["wall_time_limiter"] = job["name"] == critical_job["name"]

    gates = [
        {
            "name": step["gate"],
            "job": job["name"],
            "step": step["name"],
            "conclusion": step["conclusion"],
            "seconds": step["seconds"],
            "timing_available": step["timing_available"],
            "on_wall_time_limiter": job["wall_time_limiter"],
            "limiter_job_percent": (
                round(step["seconds"] * 100.0 / job["seconds"], 1)
                if job["wall_time_limiter"]
                and step["timing_available"]
                and job["seconds"] not in (None, 0)
                else None
            ),
        }
        for job in jobs
        for step in job["steps"]
        if step["gate"] is not None
    ]
    gate_names = [gate["name"] for gate in gates]
    actual_gates = set(gate_names)
    if len(gate_names) != len(actual_gates):
        duplicates = sorted(
            name for name in actual_gates if gate_names.count(name) > 1
        )
        raise TimingError(f"duplicate hosted gates: {duplicates}")
    if actual_gates != expected_gates:
        raise TimingError(
            "hosted gate coverage mismatch: "
            f"missing={sorted(expected_gates - actual_gates)}, "
            f"unknown={sorted(actual_gates - expected_gates)}"
        )
    history = history_summary(runs_payload, history_jobs, metadata["event"])
    workflow_seconds = round((workflow_end - workflow_start).total_seconds(), 3)
    p50 = history["p50_seconds"]
    comparison = (
        None
        if p50 in (None, 0)
        else round((workflow_seconds - p50) * 100.0 / p50, 1)
    )
    history["current_vs_p50_percent"] = comparison
    return {
        "schema": SCHEMA,
        "generated_at": metadata["generated_at"],
        "gate_contract": {
            "source": "scripts/ci/gates.json",
            "lane": hosted_lane(metadata["event"]),
            "expected_gates": sorted(expected_gates),
        },
        "run": {
            "repository": metadata["repository"],
            "workflow": metadata["workflow"],
            "run_id": metadata["run_id"],
            "run_attempt": metadata["run_attempt"],
            "head_sha": metadata["head_sha"],
            "event": metadata["event"],
            "observed_conclusion": observed_conclusion(jobs),
            "started_at": workflow_start.isoformat(),
            "completed_at": workflow_end.isoformat(),
            "seconds": workflow_seconds,
        },
        "runner": {
            "os": metadata["runner_os"],
            "arch": metadata["runner_arch"],
            "image_os": metadata["image_os"],
            "image_version": metadata["image_version"],
        },
        "toolchains": toolchains,
        "critical_path": {
            "method": "latest-quality-job-fan-in",
            "wall_time_limiter_job": critical_job["name"],
            "workflow_seconds": workflow_seconds,
            "job_seconds": critical_job["seconds"],
            "job_start_offset_seconds": critical_job["start_offset_seconds"],
            "job_completion_offset_seconds": critical_job[
                "completion_offset_seconds"
            ],
        },
        "history": history,
        "jobs": jobs,
        "gates": gates,
    }


def require_numeric(value: Any, context: str) -> None:
    if (
        not isinstance(value, (int, float))
        or isinstance(value, bool)
        or not math.isfinite(value)
    ):
        raise TimingError(f"{context} must be a finite number")


def require_number(value: Any, context: str) -> None:
    require_numeric(value, context)
    if value < 0:
        raise TimingError(f"{context} must be a non-negative number")


def require_equal_number(value: Any, expected: float, context: str) -> None:
    require_numeric(value, context)
    if not math.isclose(value, expected, abs_tol=0.001):
        raise TimingError(f"{context} does not match its source timing data")


def require_text(value: Any, context: str, *, allow_empty: bool = False) -> None:
    if not isinstance(value, str) or (not allow_empty and not value):
        qualifier = "text" if allow_empty else "non-empty text"
        raise TimingError(f"{context} must be {qualifier}")


def validate_receipt(
    receipt: dict[str, Any],
    expected_gates: set[str] | None = None,
) -> None:
    if receipt.get("schema") != SCHEMA:
        raise TimingError(f"timing receipt schema must be {SCHEMA}")
    parse_timestamp(receipt.get("generated_at"), "generated_at")
    run = receipt.get("run")
    if not isinstance(run, dict):
        raise TimingError("timing receipt must contain run metadata")
    for field in ("repository", "workflow", "event", "observed_conclusion"):
        require_text(run.get(field), f"run.{field}")
    for field in ("run_id", "run_attempt"):
        require_number(run.get(field), f"run.{field}")
    if re.fullmatch(r"[0-9a-f]{40}", str(run.get("head_sha"))) is None:
        raise TimingError("run.head_sha must be a full Git commit")
    started = parse_timestamp(run.get("started_at"), "run.started_at")
    completed = parse_timestamp(run.get("completed_at"), "run.completed_at")
    if completed < started:
        raise TimingError("run completed before it started")
    run_seconds = round((completed - started).total_seconds(), 3)
    require_equal_number(run.get("seconds"), run_seconds, "run.seconds")

    jobs = receipt.get("jobs")
    gates = receipt.get("gates")
    if not isinstance(jobs, list) or not jobs:
        raise TimingError("timing receipt jobs must be a non-empty array")
    if not isinstance(gates, list):
        raise TimingError("timing receipt gates must be an array")
    job_names: set[str] = set()
    jobs_by_name: dict[str, dict[str, Any]] = {}
    wall_time_limiters: list[str] = []
    for index, job in enumerate(jobs):
        if not isinstance(job, dict) or not isinstance(job.get("name"), str):
            raise TimingError(f"jobs[{index}] must be a named object")
        if job["name"] in job_names:
            raise TimingError(f"duplicate hosted job: {job['name']}")
        job_names.add(job["name"])
        jobs_by_name[job["name"]] = job
        require_text(job.get("conclusion"), f"job {job['name']}.conclusion")
        if not isinstance(job.get("wall_time_limiter"), bool):
            raise TimingError(
                f"job {job['name']}.wall_time_limiter must be boolean"
            )
        if job["wall_time_limiter"]:
            wall_time_limiters.append(job["name"])
        if job.get("timing_available"):
            job_started = parse_timestamp(
                job.get("started_at"), f"job {job['name']}.started_at"
            )
            job_completed = parse_timestamp(
                job.get("completed_at"), f"job {job['name']}.completed_at"
            )
            if job_started < started or job_completed > completed:
                raise TimingError(f"job {job['name']} falls outside the run window")
            job_seconds = round((job_completed - job_started).total_seconds(), 3)
            require_equal_number(
                job.get("seconds"), job_seconds, f"job {job['name']}.seconds"
            )
            require_equal_number(
                job.get("start_offset_seconds"),
                round((job_started - started).total_seconds(), 3),
                f"job {job['name']}.start_offset_seconds",
            )
            require_equal_number(
                job.get("completion_offset_seconds"),
                round((job_completed - started).total_seconds(), 3),
                f"job {job['name']}.completion_offset_seconds",
            )
            require_equal_number(
                job.get("completion_slack_seconds"),
                round((completed - job_completed).total_seconds(), 3),
                f"job {job['name']}.completion_slack_seconds",
            )
        else:
            for field in (
                "seconds",
                "start_offset_seconds",
                "completion_offset_seconds",
                "completion_slack_seconds",
            ):
                if job.get(field) is not None:
                    raise TimingError(
                        f"job {job['name']}.{field} must be null without timing"
                    )
    if run.get("observed_conclusion") != observed_conclusion(jobs):
        raise TimingError("run.observed_conclusion does not match recorded jobs")

    gate_names: list[str] = []
    for index, gate in enumerate(gates):
        if not isinstance(gate, dict):
            raise TimingError(f"gates[{index}] must be an object")
        require_text(gate.get("name"), f"gates[{index}].name")
        require_text(gate.get("job"), f"gates[{index}].job")
        if gate["job"] not in job_names:
            raise TimingError(f"gates[{index}].job must name a recorded job")
        limiter = jobs_by_name[gate["job"]]["wall_time_limiter"]
        if gate.get("on_wall_time_limiter") is not limiter:
            raise TimingError(
                f"gate {gate['name']}.on_wall_time_limiter does not match its job"
            )
        if gate.get("timing_available"):
            require_number(gate.get("seconds"), f"gate {gate['name']}.seconds")
        expected_percent = None
        job_seconds = jobs_by_name[gate["job"]].get("seconds")
        if limiter and gate.get("timing_available") and job_seconds not in (None, 0):
            expected_percent = round(gate["seconds"] * 100.0 / job_seconds, 1)
        if expected_percent is None:
            if gate.get("limiter_job_percent") is not None:
                raise TimingError(
                    f"gate {gate['name']}.limiter_job_percent must be null"
                )
        else:
            require_equal_number(
                gate.get("limiter_job_percent"),
                expected_percent,
                f"gate {gate['name']}.limiter_job_percent",
            )
        gate_names.append(gate["name"])
    actual_gates = set(gate_names)
    if len(gate_names) != len(actual_gates):
        raise TimingError("timing receipt contains duplicate hosted gates")
    gate_contract = receipt.get("gate_contract")
    contract_gates: set[str] | None = None
    if gate_contract is not None:
        if not isinstance(gate_contract, dict):
            raise TimingError("timing receipt gate_contract must be an object")
        if gate_contract.get("source") != "scripts/ci/gates.json":
            raise TimingError("gate_contract.source is unsupported")
        if gate_contract.get("lane") != hosted_lane(run["event"]):
            raise TimingError("gate_contract.lane does not match run.event")
        sealed_gates = gate_contract.get("expected_gates")
        if (
            not isinstance(sealed_gates, list)
            or any(not isinstance(name, str) or not name for name in sealed_gates)
            or sealed_gates != sorted(set(sealed_gates))
        ):
            raise TimingError(
                "gate_contract.expected_gates must be sorted unique gate names"
            )
        contract_gates = set(sealed_gates)
    if expected_gates is None:
        # Receipts created before gate_contract was added remain independently
        # verifiable from their recorded, creation-time gate inventory.
        expected_gates = contract_gates or actual_gates
    elif contract_gates is not None and contract_gates != expected_gates:
        raise TimingError("gate_contract does not match the expected hosted gates")
    if actual_gates != expected_gates:
        raise TimingError(
            "timing receipt gate coverage mismatch: "
            f"missing={sorted(expected_gates - actual_gates)}, "
            f"unknown={sorted(actual_gates - expected_gates)}"
        )
    critical = receipt.get("critical_path")
    if not isinstance(critical, dict):
        raise TimingError("timing receipt must contain critical-path metadata")
    if critical.get("method") != "latest-quality-job-fan-in":
        raise TimingError("critical_path.method is unsupported")
    limiter_name = critical.get("wall_time_limiter_job")
    if limiter_name not in job_names:
        raise TimingError("critical_path.wall_time_limiter_job must name a job")
    if wall_time_limiters != [limiter_name]:
        raise TimingError("exactly one wall-time limiter must match critical_path")
    limiter_job = jobs_by_name[limiter_name]
    require_equal_number(
        critical.get("workflow_seconds"),
        run_seconds,
        "critical_path.workflow_seconds",
    )
    for field, job_field in (
        ("job_seconds", "seconds"),
        ("job_start_offset_seconds", "start_offset_seconds"),
        ("job_completion_offset_seconds", "completion_offset_seconds"),
    ):
        require_equal_number(
            critical.get(field),
            limiter_job[job_field],
            f"critical_path.{field}",
        )

    history = receipt.get("history")
    if not isinstance(history, dict):
        raise TimingError("timing receipt must contain bounded history")
    sample_count = history.get("sample_count")
    samples = history.get("samples")
    if (
        not isinstance(sample_count, int)
        or isinstance(sample_count, bool)
        or not 0 <= sample_count <= 20
        or not isinstance(samples, list)
        or len(samples) != sample_count
    ):
        raise TimingError("timing receipt history must contain at most 20 samples")
    sample_seconds: list[float] = []
    for index, sample in enumerate(samples):
        if not isinstance(sample, dict):
            raise TimingError(f"history.samples[{index}] must be an object")
        require_number(sample.get("run_id"), f"history.samples[{index}].run_id")
        if re.fullmatch(r"[0-9a-f]{40}", str(sample.get("head_sha"))) is None:
            raise TimingError(f"history.samples[{index}].head_sha must be a commit")
        if sample.get("event") != run["event"]:
            raise TimingError(f"history.samples[{index}].event is not comparable")
        require_number(sample.get("seconds"), f"history.samples[{index}].seconds")
        sample_seconds.append(sample["seconds"])
    expected_p50 = percentile(sample_seconds, 0.50)
    expected_p95 = percentile(sample_seconds, 0.95)
    for field, expected in (
        ("p50_seconds", expected_p50),
        ("p95_seconds", expected_p95),
    ):
        if expected is None:
            if history.get(field) is not None:
                raise TimingError(f"history.{field} must be null without samples")
        else:
            require_equal_number(history.get(field), expected, f"history.{field}")
    expected_comparison = (
        None
        if expected_p50 in (None, 0)
        else round((run_seconds - expected_p50) * 100.0 / expected_p50, 1)
    )
    if expected_comparison is None:
        if history.get("current_vs_p50_percent") is not None:
            raise TimingError("history.current_vs_p50_percent must be null")
    else:
        require_equal_number(
            history.get("current_vs_p50_percent"),
            expected_comparison,
            "history.current_vs_p50_percent",
        )
    runner = receipt.get("runner")
    if not isinstance(runner, dict):
        raise TimingError("timing receipt must contain runner identity")
    for field in ("os", "arch"):
        require_text(runner.get(field), f"runner.{field}")
    for field in ("image_os", "image_version"):
        require_text(runner.get(field), f"runner.{field}", allow_empty=True)
    toolchains = receipt.get("toolchains")
    if not isinstance(toolchains, dict):
        raise TimingError("timing receipt must contain toolchain identity")
    for field in ("rust", "msrv", "lean", "collector_python"):
        require_text(toolchains.get(field), f"toolchains.{field}")


def render_summary(receipt: dict[str, Any]) -> str:
    run = receipt["run"]
    critical = receipt["critical_path"]
    history = receipt["history"]
    lines = [
        "## Hosted CI timing",
        "",
        f"- Observed result: **{run['observed_conclusion']}**",
        f"- Quality-job wall time: **{run['seconds']:.1f}s**",
        (
            "- Critical-path wall-time limiter: "
            f"**{critical['wall_time_limiter_job']}** "
            f"({critical['job_seconds']:.1f}s job duration, "
            f"started +{critical['job_start_offset_seconds']:.1f}s)"
        ),
    ]
    if history["sample_count"]:
        comparison = history["current_vs_p50_percent"]
        comparison_text = (
            "n/a" if comparison is None else f"{comparison:+.1f}%"
        )
        lines.extend(
            [
                (
                    f"- Recent successful runs: n={history['sample_count']}, "
                    f"p50={history['p50_seconds']:.1f}s, "
                    f"p95={history['p95_seconds']:.1f}s"
                ),
                f"- Current versus recent p50: **{comparison_text}**",
            ]
        )
    else:
        lines.append("- Recent successful runs: no samples available")

    lines.extend(["", "### Slowest jobs", "", "| Job | Result | Seconds |", "|---|---:|---:|"])
    timed_jobs = sorted(
        (job for job in receipt["jobs"] if job["timing_available"]),
        key=lambda job: job["seconds"],
        reverse=True,
    )
    for job in timed_jobs[:10]:
        lines.append(
            f"| {job['name']} | {job['conclusion']} | {job['seconds']:.1f} |"
        )
    lines.extend(
        [
            "",
            "### Slowest named gates",
            "",
            "| Gate | Job | Result | Seconds | Limiter job |",
            "|---|---|---:|---:|---:|",
        ]
    )
    timed_gates = sorted(
        (gate for gate in receipt["gates"] if gate["timing_available"]),
        key=lambda gate: gate["seconds"],
        reverse=True,
    )
    for gate in timed_gates[:10]:
        limiter_share = (
            f"{gate['limiter_job_percent']:.1f}%"
            if gate["limiter_job_percent"] is not None
            else "—"
        )
        lines.append(
            f"| {gate['name']} | {gate['job']} | "
            f"{gate['conclusion']} | {gate['seconds']:.1f} | {limiter_share} |"
        )
    lines.extend(
        [
            "",
            "> Timing is diagnostic. A single noisy sample never changes a gate result.",
            "",
        ]
    )
    return "\n".join(lines)


def sample_inputs() -> tuple[dict[str, Any], dict[str, Any]]:
    jobs = {
        "jobs": [
            {
                "id": 1,
                "name": "build and test",
                "conclusion": "success",
                "started_at": "2026-01-01T00:00:00Z",
                "completed_at": "2026-01-01T00:02:00Z",
                "runner_name": "runner",
                "runner_group_name": "GitHub Actions",
                "labels": ["ubuntu-latest"],
                "steps": [
                    {
                        "number": 1,
                        "name": f"{gate_registry.HOSTED_GATE_PREFIX}sample",
                        "conclusion": "success",
                        "started_at": "2026-01-01T00:00:10Z",
                        "completed_at": "2026-01-01T00:01:40Z",
                    }
                ],
            },
            {
                "id": 2,
                "name": "docs",
                "conclusion": "failure",
                "started_at": "2026-01-01T00:00:05Z",
                "completed_at": "2026-01-01T00:00:45Z",
                "runner_name": "runner",
                "runner_group_name": "GitHub Actions",
                "labels": ["ubuntu-latest"],
                "steps": [],
            },
            {
                "id": 4,
                "name": "event-inapplicable tests",
                "conclusion": "skipped",
                "started_at": "2026-01-01T00:02:00Z",
                "completed_at": "2026-01-01T00:01:59Z",
                "runner_name": None,
                "runner_group_name": None,
                "labels": ["ubuntu-latest"],
                "steps": [],
            },
            {
                "id": 3,
                "name": TIMING_JOB_NAME,
                "conclusion": None,
                "started_at": "2026-01-01T00:02:05Z",
                "completed_at": None,
                "runner_name": "runner",
                "runner_group_name": "GitHub Actions",
                "labels": ["ubuntu-latest"],
                "steps": [],
            },
        ]
    }
    runs = {
        "workflow_runs": [
            {
                "id": 10,
                "head_sha": "b" * 40,
                "event": "pull_request",
                "status": "completed",
                "conclusion": "success",
                "run_started_at": "2025-12-31T00:00:00Z",
                "updated_at": "2025-12-31T00:01:40Z",
            },
            {
                "id": 11,
                "head_sha": "c" * 40,
                "event": "pull_request",
                "status": "completed",
                "conclusion": "failure",
                "run_started_at": "2025-12-31T01:00:00Z",
                "updated_at": "2025-12-31T01:05:00Z",
            },
            {
                "id": 12,
                "head_sha": "d" * 40,
                "event": "push",
                "status": "completed",
                "conclusion": "success",
                "run_started_at": "2025-12-31T02:00:00Z",
                "updated_at": "2025-12-31T02:05:00Z",
            },
        ]
    }
    return jobs, runs


def self_test() -> None:
    jobs, runs = sample_inputs()
    metadata = {
        "generated_at": "2026-01-01T00:03:00+00:00",
        "repository": "example/nose",
        "workflow": "ci",
        "run_id": 1,
        "run_attempt": 1,
        "head_sha": "a" * 40,
        "event": "pull_request",
        "runner_os": "Linux",
        "runner_arch": "X64",
        "image_os": "ubuntu24",
        "image_version": "test",
    }
    receipt = collect_receipt(
        jobs,
        runs,
        {10: jobs, 12: jobs},
        metadata=metadata,
        expected_gates={"sample"},
        toolchains={
            "rust": "test",
            "msrv": "test",
            "lean": "test",
            "collector_python": "test",
        },
    )
    validate_receipt(receipt, {"sample"})
    assert receipt["run"]["observed_conclusion"] == "failure"
    assert receipt["run"]["seconds"] == 120.0
    assert (
        receipt["critical_path"]["wall_time_limiter_job"] == "build and test"
    )
    assert receipt["history"]["sample_count"] == 1
    assert receipt["history"]["p50_seconds"] == 120.0
    skipped = next(
        job for job in receipt["jobs"] if job["conclusion"] == "skipped"
    )
    assert skipped["timing_available"] is False
    assert skipped["seconds"] is None
    assert "sample" in render_summary(receipt)
    historical_receipt = json.loads(json.dumps(receipt))
    historical_receipt.pop("gate_contract")
    validate_receipt(historical_receipt)

    reusable_jobs = json.loads(json.dumps(jobs))
    reusable_jobs["jobs"].insert(
        0,
        {
            "id": 99,
            "name": "plan",
            "conclusion": "success",
            "started_at": "2026-01-01T00:00:00Z",
            "completed_at": "2026-01-01T00:00:05Z",
            "steps": [],
        },
    )
    for job in reusable_jobs["jobs"][1:]:
        job["name"] = f"repository quality gates / {job['name']}"
    reusable_runs = {
        "workflow_runs": [
            {
                "id": 20,
                "head_sha": "e" * 40,
                "event": "workflow_call",
                "status": "completed",
                "conclusion": "success",
            }
        ]
    }
    reusable_metadata = dict(metadata, event="workflow_call")
    reusable_receipt = collect_receipt(
        reusable_jobs,
        reusable_runs,
        {20: reusable_jobs},
        metadata=reusable_metadata,
        expected_gates={"sample"},
        toolchains=receipt["toolchains"],
    )
    validate_receipt(reusable_receipt, {"sample"})
    assert "plan" not in {job["name"] for job in reusable_receipt["jobs"]}

    mutations = [
        ("schema", lambda value: value.update(schema="wrong"), "schema"),
        (
            "gate coverage",
            lambda value: value["gates"].clear(),
            "gate coverage mismatch",
        ),
        (
            "critical path",
            lambda value: value["critical_path"].update(
                wall_time_limiter_job="missing"
            ),
            "critical_path.wall_time_limiter_job",
        ),
        (
            "derived run time",
            lambda value: value["run"].update(seconds=1),
            "source timing data",
        ),
        (
            "duplicate gate",
            lambda value: value["gates"].append(value["gates"][0]),
            "duplicate hosted gates",
        ),
        (
            "missing toolchain",
            lambda value: value["toolchains"].pop("lean"),
            "toolchains.lean",
        ),
        (
            "wall-time limiter",
            lambda value: value["jobs"][0].update(
                wall_time_limiter=False
            ),
            "on_wall_time_limiter",
        ),
        (
            "derived critical time",
            lambda value: value["critical_path"].update(job_seconds=999),
            "source timing data",
        ),
        (
            "negative history sample",
            lambda value: value["history"]["samples"][0].update(seconds=-1),
            "non-negative",
        ),
        (
            "derived p50",
            lambda value: value["history"].update(p50_seconds=999),
            "source timing data",
        ),
    ]
    for name, mutate, expected in mutations:
        changed = json.loads(json.dumps(receipt))
        mutate(changed)
        try:
            validate_receipt(changed, {"sample"})
        except TimingError as exc:
            assert expected in str(exc), (name, exc)
        else:
            raise AssertionError(f"{name} mutation passed")
    print("hosted CI timing self-test passed")


def hosted_lane(event: str) -> str:
    return "pull-request" if event == "pull_request" else "release"


def expected_hosted_gates(event: str) -> set[str]:
    gates = gate_registry.validate_live_registry(gate_registry.load_registry())
    lane = hosted_lane(event)
    return {gate["name"] for gate in gates if lane in gate["lanes"]}


def load_history_jobs(path: Path) -> dict[int, dict[str, Any]]:
    if not path.is_dir():
        raise TimingError(f"history jobs path is not a directory: {path}")
    payloads: dict[int, dict[str, Any]] = {}
    for item in sorted(path.glob("*.json")):
        try:
            run_id = int(item.stem)
        except ValueError as exc:
            raise TimingError(f"history jobs filename must be a run id: {item}") from exc
        payloads[run_id] = load_object(item, "history jobs payload")
    return payloads


def parse_args() -> argparse.Namespace:
    parser = argparse.ArgumentParser()
    parser.add_argument("--self-test", action="store_true")
    parser.add_argument("--validate", type=Path)
    parser.add_argument("--jobs", type=Path)
    parser.add_argument("--runs", type=Path)
    parser.add_argument("--history-jobs", type=Path)
    parser.add_argument("--output", type=Path)
    parser.add_argument("--summary", type=Path)
    parser.add_argument("--repository")
    parser.add_argument("--workflow")
    parser.add_argument("--run-id", type=int)
    parser.add_argument("--run-attempt", type=int)
    parser.add_argument("--head-sha")
    parser.add_argument("--event")
    parser.add_argument("--runner-os")
    parser.add_argument("--runner-arch")
    parser.add_argument("--image-os", default="")
    parser.add_argument("--image-version", default="")
    return parser.parse_args()


def main() -> int:
    if sys.version_info < MINIMUM_PYTHON:
        print("hosted CI timing requires Python 3.10 or newer", file=sys.stderr)
        return 127
    args = parse_args()
    try:
        if args.self_test:
            self_test()
            return 0
        if args.validate is not None:
            receipt = load_object(args.validate, "timing receipt")
            validate_receipt(receipt)
            print(f"hosted CI timing receipt OK: {args.validate}")
            return 0
        required = {
            "--jobs": args.jobs,
            "--runs": args.runs,
            "--history-jobs": args.history_jobs,
            "--output": args.output,
            "--summary": args.summary,
            "--repository": args.repository,
            "--workflow": args.workflow,
            "--run-id": args.run_id,
            "--run-attempt": args.run_attempt,
            "--head-sha": args.head_sha,
            "--event": args.event,
            "--runner-os": args.runner_os,
            "--runner-arch": args.runner_arch,
        }
        missing = [name for name, value in required.items() if value is None]
        if missing:
            raise TimingError(f"missing required arguments: {', '.join(missing)}")
        expected_gates = expected_hosted_gates(args.event)
        metadata = {
            "generated_at": datetime.now().astimezone().isoformat(),
            "repository": args.repository,
            "workflow": args.workflow,
            "run_id": args.run_id,
            "run_attempt": args.run_attempt,
            "head_sha": args.head_sha,
            "event": args.event,
            "runner_os": args.runner_os,
            "runner_arch": args.runner_arch,
            "image_os": args.image_os,
            "image_version": args.image_version,
        }
        receipt = collect_receipt(
            load_object(args.jobs, "jobs payload"),
            load_object(args.runs, "workflow runs payload"),
            load_history_jobs(args.history_jobs),
            metadata=metadata,
            expected_gates=expected_gates,
            toolchains=read_checked_toolchains(ROOT),
        )
        validate_receipt(receipt)
        args.output.parent.mkdir(parents=True, exist_ok=True)
        args.output.write_text(json.dumps(receipt, indent=2) + "\n")
        args.summary.parent.mkdir(parents=True, exist_ok=True)
        args.summary.write_text(render_summary(receipt))
        print(
            f"hosted CI timing: {receipt['run']['seconds']:.1f}s, "
            "wall-time limiter "
            f"{receipt['critical_path']['wall_time_limiter_job']}"
        )
        return 0
    except (OSError, TimingError, gate_registry.RegistryError) as exc:
        print(f"hosted CI timing error: {exc}", file=sys.stderr)
        return 1


if __name__ == "__main__":
    raise SystemExit(main())
