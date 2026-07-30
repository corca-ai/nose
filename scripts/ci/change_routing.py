#!/usr/bin/env python3
"""Compute and verify fail-closed, report-only CI change routing."""

from __future__ import annotations

import argparse
import fnmatch
import json
import re
import subprocess
import sys
from dataclasses import dataclass
from pathlib import Path
from typing import Any

import gate_registry


ROOT = Path(__file__).resolve().parents[2]
DEFAULT_MANIFEST = ROOT / "scripts/ci/gates.json"
DEFAULT_WORKFLOW = ROOT / ".github/workflows/ci.yml"
ROUTING_SCHEMA = "nose.ci-change-routing.v1"
RECEIPT_SCHEMA = "nose.ci-change-route-receipt.v1"
CONTROL_JOBS = {"route", "hosted-timing", "qualification"}
ALWAYS_REQUIRED_JOBS = {"route", "hosted-timing"}
SAFE_CHANGE_STATUSES = {"A", "M"}


class RoutingError(ValueError):
    """Raised when routing policy or runtime evidence is invalid."""


@dataclass(frozen=True)
class Change:
    status: str
    path: str
    previous_path: str | None = None


def _text_list(value: Any, context: str, *, allow_empty: bool = False) -> list[str]:
    if (
        not isinstance(value, list)
        or any(not isinstance(item, str) or not item for item in value)
        or len(value) != len(set(value))
        or (not allow_empty and not value)
    ):
        qualifier = "unique non-empty strings"
        if allow_empty:
            qualifier = "unique strings"
        raise RoutingError(f"{context} must contain {qualifier}")
    return value


def quality_jobs(workflow: Path = DEFAULT_WORKFLOW) -> dict[str, str]:
    blocks = gate_registry.workflow_job_blocks(workflow)
    missing_controls = CONTROL_JOBS - set(blocks)
    if missing_controls:
        raise RoutingError(
            f"CI workflow misses routing control jobs: {sorted(missing_controls)}"
        )
    return {name: block for name, block in blocks.items() if name not in CONTROL_JOBS}


def eligible_quality_jobs(
    event: str, workflow: Path = DEFAULT_WORKFLOW
) -> set[str]:
    event_conditions = {
        "workspace-tests-pr": "${{ github.event_name == 'pull_request' }}",
        "workspace-tests-protected": "${{ github.event_name != 'pull_request' }}",
    }
    eligible: set[str] = set()
    for name, block in quality_jobs(workflow).items():
        condition_rows = re.findall(r"^ {4}if:\s*(.+)$", block, re.MULTILINE)
        if len(condition_rows) > 1:
            raise RoutingError(f"job {name} has multiple job-level conditions")
        condition = condition_rows[0] if condition_rows else None
        expected_condition = event_conditions.get(name)
        if condition != expected_condition:
            raise RoutingError(
                f"job {name} condition must be {expected_condition!r}, got {condition!r}"
            )
        if condition is None or (
            name == "workspace-tests-pr" and event == "pull_request"
        ) or (
            name == "workspace-tests-protected" and event != "pull_request"
        ):
            eligible.add(name)
    return eligible


def gate_jobs(workflow: Path = DEFAULT_WORKFLOW) -> dict[str, set[str]]:
    jobs: dict[str, set[str]] = {}
    for job, block in quality_jobs(workflow).items():
        for name in re.findall(r"--gate\s+([a-z0-9-]+)", block):
            jobs.setdefault(name, set()).add(job)
    return jobs


def load_policy(
    manifest: Path = DEFAULT_MANIFEST,
    workflow: Path = DEFAULT_WORKFLOW,
) -> tuple[dict[str, Any], dict[str, Any]]:
    registry = gate_registry.load_registry(manifest)
    gates = gate_registry.validate_model(registry)
    policy = registry.get("change_routing")
    if not isinstance(policy, dict):
        raise RoutingError("gate registry must define change_routing policy")
    required_fields = {
        "schema",
        "mode",
        "full_events",
        "always_required_jobs",
        "global_patterns",
        "rules",
        "historical_cases",
    }
    if set(policy) != required_fields:
        raise RoutingError(
            "change_routing fields drifted: "
            f"missing={sorted(required_fields - set(policy))}, "
            f"extra={sorted(set(policy) - required_fields)}"
        )
    if policy["schema"] != ROUTING_SCHEMA:
        raise RoutingError(f"change_routing schema must be {ROUTING_SCHEMA!r}")
    if policy["mode"] != "report-only":
        raise RoutingError("change_routing must remain report-only until rollout")
    full_events = set(_text_list(policy["full_events"], "full_events"))
    if not {"push", "workflow_call"} <= full_events:
        raise RoutingError("push and workflow_call must always qualify the full gate set")
    always = set(
        _text_list(policy["always_required_jobs"], "always_required_jobs")
    )
    if always != ALWAYS_REQUIRED_JOBS:
        raise RoutingError(
            f"always_required_jobs must be {sorted(ALWAYS_REQUIRED_JOBS)}"
        )
    global_patterns = _text_list(policy["global_patterns"], "global_patterns")
    if any(pattern.startswith("/") or "\x00" in pattern for pattern in global_patterns):
        raise RoutingError("global_patterns must be relative, NUL-free globs")

    gates_by_name = {gate["name"]: gate for gate in gates}
    mapped_jobs = gate_jobs(workflow)
    available_jobs = set(quality_jobs(workflow))
    pull_request_jobs = eligible_quality_jobs("pull_request", workflow)
    release_jobs = eligible_quality_jobs("push", workflow)
    for gate in gates:
        jobs = mapped_jobs.get(gate["name"], set())
        if "pull-request" in gate["lanes"] and not jobs & pull_request_jobs:
            raise RoutingError(f"gate {gate['name']} has no pull-request eligible job")
        if "release" in gate["lanes"] and not jobs & release_jobs:
            raise RoutingError(f"gate {gate['name']} has no release eligible job")
    seen_classes: set[str] = set()
    for index, rule in enumerate(policy["rules"]):
        if not isinstance(rule, dict) or set(rule) != {
            "class",
            "patterns",
            "gates",
            "jobs",
        }:
            raise RoutingError(
                f"routing rule {index} must define class, patterns, gates, and jobs"
            )
        class_name = rule["class"]
        if (
            not isinstance(class_name, str)
            or re.fullmatch(r"[a-z0-9-]+", class_name) is None
            or class_name in seen_classes
        ):
            raise RoutingError(f"routing rule {index} has invalid/duplicate class")
        seen_classes.add(class_name)
        patterns = _text_list(rule["patterns"], f"rule {class_name}.patterns")
        if any(pattern.startswith("/") or "\x00" in pattern for pattern in patterns):
            raise RoutingError(f"rule {class_name} patterns must be relative globs")
        rule_gates = _text_list(
            rule["gates"], f"rule {class_name}.gates", allow_empty=True
        )
        rule_jobs = _text_list(
            rule["jobs"], f"rule {class_name}.jobs", allow_empty=True
        )
        if not rule_gates and not rule_jobs:
            raise RoutingError(f"rule {class_name} must select a gate or job")
        unknown_gates = set(rule_gates) - set(gates_by_name)
        if unknown_gates:
            raise RoutingError(
                f"rule {class_name} names unknown gates: {sorted(unknown_gates)}"
            )
        non_pr_gates = {
            name
            for name in rule_gates
            if "pull-request" not in gates_by_name[name]["lanes"]
        }
        if non_pr_gates:
            raise RoutingError(
                f"rule {class_name} names non-PR gates: {sorted(non_pr_gates)}"
            )
        unmapped_gates = set(rule_gates) - set(mapped_jobs)
        if unmapped_gates:
            raise RoutingError(
                f"rule {class_name} gates have no CI job: {sorted(unmapped_gates)}"
            )
        unknown_jobs = set(rule_jobs) - pull_request_jobs
        if unknown_jobs:
            raise RoutingError(
                f"rule {class_name} names non-PR/unknown jobs: {sorted(unknown_jobs)}"
            )

    cases = policy["historical_cases"]
    if not isinstance(cases, list) or not cases:
        raise RoutingError("historical_cases must be a non-empty array")
    seen_case_names: set[str] = set()
    for index, case in enumerate(cases):
        required = {
            "name",
            "commit",
            "expected_full",
            "required_classes",
            "required_gates",
            "required_jobs",
        }
        if not isinstance(case, dict) or set(case) != required:
            raise RoutingError(f"historical case {index} fields drifted")
        name = case["name"]
        commit = case["commit"]
        if (
            not isinstance(name, str)
            or not name
            or name in seen_case_names
            or not isinstance(commit, str)
            or re.fullmatch(r"[0-9a-f]{40}", commit) is None
            or not isinstance(case["expected_full"], bool)
        ):
            raise RoutingError(f"historical case {index} metadata is invalid")
        seen_case_names.add(name)
        _text_list(
            case["required_classes"],
            f"historical case {name}.required_classes",
            allow_empty=True,
        )
        required_gates = set(
            _text_list(
                case["required_gates"],
                f"historical case {name}.required_gates",
                allow_empty=True,
            )
        )
        required_jobs = set(
            _text_list(
                case["required_jobs"],
                f"historical case {name}.required_jobs",
                allow_empty=True,
            )
        )
        if not required_gates <= set(gates_by_name):
            raise RoutingError(f"historical case {name} names unknown gates")
        if not required_jobs <= available_jobs:
            raise RoutingError(f"historical case {name} names unknown jobs")
    return registry, policy


def parse_name_status_z(data: bytes) -> list[Change]:
    if not data or not data.endswith(b"\0"):
        raise RoutingError("name-status stream must be non-empty and NUL-terminated")
    raw_tokens = data[:-1].split(b"\0")
    try:
        tokens = [token.decode("utf-8") for token in raw_tokens]
    except UnicodeDecodeError as exc:
        raise RoutingError("name-status stream is not UTF-8") from exc
    changes: list[Change] = []
    index = 0
    while index < len(tokens):
        status = tokens[index]
        index += 1
        if re.fullmatch(r"(?:[AMDTUXB]|[RC](?:100|0[0-9]{2}))", status) is None:
            raise RoutingError(f"malformed change status: {status!r}")
        if status[0] in {"R", "C"}:
            if index + 1 >= len(tokens):
                raise RoutingError(f"change status {status} misses two paths")
            previous_path, path = tokens[index : index + 2]
            index += 2
            changes.append(Change(status=status, path=path, previous_path=previous_path))
        else:
            if index >= len(tokens):
                raise RoutingError(f"change status {status} misses its path")
            changes.append(Change(status=status, path=tokens[index]))
            index += 1
    return changes


def _valid_path(path: str) -> bool:
    parts = Path(path).parts
    return (
        bool(path)
        and not path.startswith("/")
        and "\\" not in path
        and all(part not in {"", ".", ".."} for part in parts)
        and not any(ord(character) < 32 for character in path)
    )


def _matches(path: str, patterns: list[str]) -> bool:
    return any(fnmatch.fnmatchcase(path, pattern) for pattern in patterns)


def compute_route(
    registry: dict[str, Any],
    policy: dict[str, Any],
    *,
    event: str,
    changes: list[Change],
    parse_error: str | None = None,
    workflow: Path = DEFAULT_WORKFLOW,
) -> dict[str, Any]:
    eligible = eligible_quality_jobs(event, workflow)
    mappings = gate_jobs(workflow)
    gates = registry["gates"]
    lane = "pull-request" if event == "pull_request" else "release"
    full_gate_set = {gate["name"] for gate in gates if lane in gate["lanes"]}
    selected_gates: set[str] = set()
    selected_jobs: set[str] = set()
    matched_classes: set[str] = set()
    reasons: set[str] = set()

    if event in set(policy["full_events"]):
        reasons.add(f"full-qualification-event:{event}")
    if parse_error is not None:
        reasons.add(f"change-data-error:{parse_error}")
    if not changes and not reasons:
        reasons.add("empty-change-set")

    for change in changes:
        if change.status not in SAFE_CHANGE_STATUSES:
            reasons.add(f"unsafe-change-status:{change.status}")
        paths = [change.path]
        if change.previous_path is not None:
            paths.append(change.previous_path)
        if any(not _valid_path(path) for path in paths):
            reasons.add("malformed-path")
            continue
        if any(_matches(path, policy["global_patterns"]) for path in paths):
            reasons.add("global-policy-path")
            continue
        path_matched = False
        for rule in policy["rules"]:
            if any(_matches(path, rule["patterns"]) for path in paths):
                path_matched = True
                matched_classes.add(rule["class"])
                selected_gates.update(rule["gates"])
                selected_jobs.update(rule["jobs"])
        if not path_matched:
            reasons.add("unclassified-path")

    full = bool(reasons)
    if full:
        selected_gates = full_gate_set
        proposed_jobs = eligible
    else:
        proposed_jobs = selected_jobs | {
            job
            for name in selected_gates
            for job in mappings.get(name, set())
        }
        proposed_jobs &= eligible

    enforced_quality = eligible if policy["mode"] == "report-only" else proposed_jobs
    enforced_jobs = enforced_quality | set(policy["always_required_jobs"])
    return {
        "schema": RECEIPT_SCHEMA,
        "mode": policy["mode"],
        "event": event,
        "decision": "full" if full else "selective",
        "full_reasons": sorted(reasons),
        "changes": [
            {
                "status": change.status,
                "path": change.path,
                **(
                    {"previous_path": change.previous_path}
                    if change.previous_path is not None
                    else {}
                ),
            }
            for change in changes
        ],
        "matched_classes": sorted(matched_classes),
        "selected_gates": sorted(selected_gates),
        "proposed_jobs": sorted(proposed_jobs),
        "enforced_jobs": sorted(enforced_jobs),
    }


def route_summary(receipt: dict[str, Any]) -> str:
    proposed = ", ".join(f"`{name}`" for name in receipt["proposed_jobs"]) or "_none_"
    classes = ", ".join(f"`{name}`" for name in receipt["matched_classes"]) or "_none_"
    reasons = ", ".join(f"`{name}`" for name in receipt["full_reasons"]) or "_none_"
    return (
        "## Change-aware CI route\n\n"
        f"- Mode: `{receipt['mode']}` (all eligible jobs remain enforced)\n"
        f"- Decision: `{receipt['decision']}`\n"
        f"- Matched classes: {classes}\n"
        f"- Full-route reasons: {reasons}\n"
        f"- Proposed quality jobs: {proposed}\n"
        f"- Changed entries: `{len(receipt['changes'])}`\n"
    )


def validate_workflow(workflow: Path = DEFAULT_WORKFLOW) -> None:
    text = workflow.read_text()
    header = text.split("\njobs:", maxsplit=1)[0]
    if re.search(r"^\s+paths(?:-ignore)?:", header, re.MULTILINE):
        raise RoutingError("CI workflow must not use trigger-level path filtering")
    blocks = gate_registry.workflow_job_blocks(workflow)
    route = blocks.get("route", "")
    qualification = blocks.get("qualification", "")
    for job, expected_name in gate_registry.UNTIMED_HOSTED_JOB_NAMES.items():
        if f"    name: {expected_name}\n" not in blocks.get(job, ""):
            raise RoutingError(f"control job {job} display name drifted")
    if "    outputs:\n" not in route:
        raise RoutingError("route job must publish routing outputs")
    if "    if: ${{ always() }}" not in qualification:
        raise RoutingError("qualification job must run with always()")
    match = re.search(
        r"^ {4}needs:\n(?P<rows>(?:^ {6}- [a-z0-9-]+\n?)+)",
        qualification,
        re.MULTILINE,
    )
    if match is None:
        raise RoutingError("qualification job must list every dependency")
    actual = set(re.findall(r"^ {6}- ([a-z0-9-]+)$", match.group("rows"), re.MULTILINE))
    expected = set(blocks) - {"qualification"}
    if actual != expected:
        raise RoutingError(
            "qualification needs mismatch: "
            f"missing={sorted(expected - actual)}, extra={sorted(actual - expected)}"
        )
    if "toJSON(needs)" not in qualification:
        raise RoutingError("qualification must consume the complete needs result map")


def _write_json(path: Path, value: dict[str, Any]) -> None:
    path.parent.mkdir(parents=True, exist_ok=True)
    path.write_text(json.dumps(value, indent=2, sort_keys=True) + "\n")


def _write_summary(path: Path, text: str) -> None:
    path.parent.mkdir(parents=True, exist_ok=True)
    path.write_text(text)


def parse_args() -> argparse.Namespace:
    parser = argparse.ArgumentParser()
    parser.add_argument("--manifest", type=Path, default=DEFAULT_MANIFEST)
    parser.add_argument("--workflow", type=Path, default=DEFAULT_WORKFLOW)
    subparsers = parser.add_subparsers(dest="command", required=True)
    validate = subparsers.add_parser("validate")
    validate.add_argument("--self-test", action="store_true")
    route = subparsers.add_parser("route")
    route.add_argument("--event", required=True)
    route.add_argument("--changes-z", type=Path)
    route.add_argument("--output", type=Path, required=True)
    route.add_argument("--summary", type=Path, required=True)
    route.add_argument("--github-output", type=Path)
    return parser.parse_args()


def main() -> int:
    args = parse_args()
    try:
        registry, policy = load_policy(args.manifest, args.workflow)
        validate_workflow(args.workflow)
        if args.command == "validate":
            if args.self_test:
                import change_routing_selftest

                change_routing_selftest.self_test(registry, policy)
            print("change-aware CI routing policy OK")
            return 0
        changes: list[Change] = []
        parse_error: str | None = None
        if args.changes_z is None:
            if args.event == "pull_request":
                parse_error = "missing-change-data"
        else:
            try:
                changes = parse_name_status_z(args.changes_z.read_bytes())
            except (OSError, RoutingError) as exc:
                parse_error = str(exc)
        receipt = compute_route(
            registry,
            policy,
            event=args.event,
            changes=changes,
            parse_error=parse_error,
            workflow=args.workflow,
        )
        _write_json(args.output, receipt)
        _write_summary(args.summary, route_summary(receipt))
        if args.github_output is not None:
            with args.github_output.open("a") as output:
                for key in ("mode", "decision"):
                    output.write(f"{key}={receipt[key]}\n")
                for key in ("proposed_jobs", "enforced_jobs"):
                    value = json.dumps(receipt[key], separators=(",", ":"))
                    output.write(f"{key}={value}\n")
        return 0
    except (OSError, RoutingError, subprocess.CalledProcessError) as exc:
        print(f"change-aware CI routing error: {exc}", file=sys.stderr)
        return 1


if __name__ == "__main__":
    sys.modules.setdefault("change_routing", sys.modules[__name__])
    raise SystemExit(main())
