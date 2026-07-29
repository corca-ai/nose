#!/usr/bin/env python3
"""Validate and render the authoritative repository CI gate inventory."""

from __future__ import annotations

import argparse
import copy
import json
import re
import sys
from pathlib import Path
from typing import Any


ROOT = Path(__file__).resolve().parents[2]
DEFAULT_MANIFEST = ROOT / "scripts/ci/gates.json"
DEFAULT_DISPATCHER = ROOT / "scripts/check-ci-local.sh"
DEFAULT_CI_WORKFLOW = ROOT / ".github/workflows/ci.yml"
DEFAULT_NIGHTLY_WORKFLOW = ROOT / ".github/workflows/corpus-verify.yml"
DEFAULT_RELEASE_WORKFLOW = ROOT / ".github/workflows/release.yml"

SCHEMA = "nose.ci-gates.v1"
WORKTREE_EFFECTS = {"read-only", "verify-checked-output"}
LOCAL_PLAN_LANES = {"fast": "local-fast", "full": "local-full"}
REQUIRED_GATE_FIELDS = {
    "name",
    "owner",
    "implementation",
    "tools",
    "inputs",
    "worktree_effect",
    "outputs",
    "cache",
    "lanes",
    "lane_reason",
    "focused_command",
    "plans",
}


class RegistryError(ValueError):
    """Raised when gate metadata and executable policy disagree."""


def load_registry(path: Path = DEFAULT_MANIFEST) -> dict[str, Any]:
    try:
        value = json.loads(path.read_text())
    except (OSError, json.JSONDecodeError) as exc:
        raise RegistryError(f"cannot load gate registry {path}: {exc}") from exc
    if not isinstance(value, dict):
        raise RegistryError("gate registry root must be an object")
    return value


def gate_names_from_dispatcher(path: Path = DEFAULT_DISPATCHER) -> set[str]:
    text = path.read_text()
    match = re.search(
        r"^run_named_gate\(\) \{(?P<body>.*?)^\}\n\nif \[\[ \"\$mode\"",
        text,
        re.MULTILINE | re.DOTALL,
    )
    if match is None:
        raise RegistryError(f"cannot locate run_named_gate dispatcher in {path}")
    return set(re.findall(r"^ {8}([a-z0-9-]+)\)$", match.group("body"), re.MULTILINE))


def gate_names_from_workflow(path: Path) -> set[str]:
    return set(re.findall(r"--gate\s+([a-z0-9-]+)", path.read_text()))


def _require_non_empty_text(gate: dict[str, Any], field: str, name: str) -> None:
    if not isinstance(gate[field], str) or not gate[field].strip():
        raise RegistryError(f"gate {name}: {field} must be non-empty text")


def validate_model(registry: dict[str, Any]) -> list[dict[str, Any]]:
    if registry.get("schema") != SCHEMA:
        raise RegistryError(f"gate registry schema must be {SCHEMA!r}")
    lane_descriptions = registry.get("lanes")
    if not isinstance(lane_descriptions, dict) or not lane_descriptions:
        raise RegistryError("gate registry lanes must be a non-empty object")
    known_lanes = set(lane_descriptions)
    gates = registry.get("gates")
    if not isinstance(gates, list) or not gates:
        raise RegistryError("gate registry gates must be a non-empty array")

    seen_names: set[str] = set()
    plan_orders: dict[str, set[int]] = {mode: set() for mode in LOCAL_PLAN_LANES}
    for index, gate in enumerate(gates):
        if not isinstance(gate, dict):
            raise RegistryError(f"gate at index {index} must be an object")
        missing = REQUIRED_GATE_FIELDS - set(gate)
        extra = set(gate) - REQUIRED_GATE_FIELDS
        if missing:
            raise RegistryError(f"gate at index {index} misses fields: {sorted(missing)}")
        if extra:
            raise RegistryError(f"gate at index {index} has unknown fields: {sorted(extra)}")

        name = gate["name"]
        if not isinstance(name, str) or re.fullmatch(r"[a-z0-9-]+", name) is None:
            raise RegistryError(f"gate at index {index}: invalid name {name!r}")
        if name in seen_names:
            raise RegistryError(f"duplicate gate name: {name}")
        seen_names.add(name)

        for field in ("owner", "implementation", "cache", "lane_reason", "focused_command"):
            _require_non_empty_text(gate, field, name)
        for field in ("tools", "inputs", "outputs", "lanes"):
            value = gate[field]
            if not isinstance(value, list) or any(
                not isinstance(item, str) or not item for item in value
            ):
                raise RegistryError(f"gate {name}: {field} must be an array of text")
        if not gate["tools"]:
            raise RegistryError(f"gate {name}: tools must name at least one dependency")
        if len(gate["lanes"]) != len(set(gate["lanes"])):
            raise RegistryError(f"gate {name}: lanes contain duplicates")
        unknown_lanes = set(gate["lanes"]) - known_lanes
        if unknown_lanes:
            raise RegistryError(f"gate {name}: unknown lanes {sorted(unknown_lanes)}")
        if gate["worktree_effect"] not in WORKTREE_EFFECTS:
            raise RegistryError(
                f"gate {name}: unknown worktree effect {gate['worktree_effect']!r}"
            )
        if gate["worktree_effect"] == "verify-checked-output" and not gate["outputs"]:
            raise RegistryError(f"gate {name}: checked-output gate must name its outputs")
        focused_prefix = f"./scripts/check-ci-local.sh --gate {name}"
        if not gate["focused_command"].startswith(focused_prefix):
            raise RegistryError(
                f"gate {name}: focused_command must start with {focused_prefix!r}"
            )

        plans = gate["plans"]
        if not isinstance(plans, dict) or not set(plans) <= set(LOCAL_PLAN_LANES):
            raise RegistryError(f"gate {name}: plans may contain only fast/full")
        for mode, lane in LOCAL_PLAN_LANES.items():
            planned = mode in plans
            if planned != (lane in gate["lanes"]):
                raise RegistryError(
                    f"gate {name}: {mode} plan and {lane} lane must agree"
                )
            if not planned:
                continue
            plan = plans[mode]
            if not isinstance(plan, dict) or set(plan) != {"order", "label", "args"}:
                raise RegistryError(
                    f"gate {name}: {mode} plan needs order, label, and args"
                )
            order = plan["order"]
            if not isinstance(order, int) or order <= 0:
                raise RegistryError(f"gate {name}: {mode} order must be positive")
            if order in plan_orders[mode]:
                raise RegistryError(f"{mode} plan reuses order {order}")
            plan_orders[mode].add(order)
            if not isinstance(plan["label"], str) or not plan["label"]:
                raise RegistryError(f"gate {name}: {mode} label must be non-empty")
            args = plan["args"]
            if (
                not isinstance(args, list)
                or len(args) > 2
                or any(not isinstance(arg, str) or "\t" in arg for arg in args)
            ):
                raise RegistryError(
                    f"gate {name}: {mode} args must contain at most two strings"
                )

    return gates


def validate_live_registry(
    registry: dict[str, Any],
    *,
    dispatcher: Path = DEFAULT_DISPATCHER,
    ci_workflow: Path = DEFAULT_CI_WORKFLOW,
    nightly_workflow: Path = DEFAULT_NIGHTLY_WORKFLOW,
    release_workflow: Path = DEFAULT_RELEASE_WORKFLOW,
) -> list[dict[str, Any]]:
    gates = validate_model(registry)
    registry_names = {gate["name"] for gate in gates}
    dispatcher_names = gate_names_from_dispatcher(dispatcher)
    if registry_names != dispatcher_names:
        raise RegistryError(
            "gate registry/dispatcher mismatch: "
            f"registry-only={sorted(registry_names - dispatcher_names)}, "
            f"dispatcher-only={sorted(dispatcher_names - registry_names)}"
        )

    pull_request_names = {
        gate["name"] for gate in gates if "pull-request" in gate["lanes"]
    }
    workflow_names = gate_names_from_workflow(ci_workflow)
    if pull_request_names != workflow_names:
        raise RegistryError(
            "pull-request lane/.github workflow mismatch: "
            f"registry-only={sorted(pull_request_names - workflow_names)}, "
            f"workflow-only={sorted(workflow_names - pull_request_names)}"
        )

    nightly_names = {gate["name"] for gate in gates if "nightly" in gate["lanes"]}
    nightly_workflow_names = gate_names_from_workflow(nightly_workflow)
    if nightly_names != nightly_workflow_names:
        raise RegistryError(
            "nightly lane/workflow mismatch: "
            f"registry-only={sorted(nightly_names - nightly_workflow_names)}, "
            f"workflow-only={sorted(nightly_workflow_names - nightly_names)}"
        )

    release_names = {gate["name"] for gate in gates if "release" in gate["lanes"]}
    if release_names != pull_request_names:
        raise RegistryError(
            "release lane must equal pull-request lane while release reuses ci.yml"
        )
    release_text = release_workflow.read_text()
    if "uses: ./.github/workflows/ci.yml" not in release_text:
        raise RegistryError("release workflow no longer reuses .github/workflows/ci.yml")
    return gates


def plan_rows(gates: list[dict[str, Any]], mode: str) -> list[dict[str, Any]]:
    return sorted(
        (
            {"name": gate["name"], **gate["plans"][mode]}
            for gate in gates
            if mode in gate["plans"]
        ),
        key=lambda row: row["order"],
    )


def print_plan(rows: list[dict[str, Any]], output_format: str) -> None:
    if output_format == "json":
        json.dump(rows, sys.stdout, indent=2)
        print()
        return
    for row in rows:
        fields = [row["name"], row["label"], *row["args"]]
        while len(fields) < 4:
            fields.append("")
        print("|".join(fields))


def print_list(gates: list[dict[str, Any]], output_format: str) -> None:
    if output_format == "json":
        json.dump(gates, sys.stdout, indent=2)
        print()
        return
    print("gate\tlanes\tworktree\tcache\towner\tfocused command")
    for gate in gates:
        print(
            "\t".join(
                [
                    gate["name"],
                    ",".join(gate["lanes"]),
                    gate["worktree_effect"],
                    gate["cache"],
                    gate["owner"],
                    gate["focused_command"],
                ]
            )
        )


def self_test() -> None:
    sample = {
        "schema": SCHEMA,
        "lanes": {
            "local-fast": "fast",
            "local-full": "full",
            "pull-request": "pr",
            "release": "release",
            "nightly": "nightly",
        },
        "gates": [
            {
                "name": "sample",
                "owner": "sample owner",
                "implementation": "sample implementation",
                "tools": ["python3"],
                "inputs": ["input"],
                "worktree_effect": "read-only",
                "outputs": [],
                "cache": "none",
                "lanes": ["local-fast"],
                "lane_reason": "sample rationale",
                "focused_command": "./scripts/check-ci-local.sh --gate sample",
                "plans": {
                    "fast": {"order": 10, "label": "sample", "args": []}
                },
            }
        ],
    }
    gates = validate_model(sample)
    assert plan_rows(gates, "fast")[0]["name"] == "sample"
    assert not plan_rows(gates, "full")

    duplicate = copy.deepcopy(sample)
    duplicate["gates"].append(copy.deepcopy(duplicate["gates"][0]))
    try:
        validate_model(duplicate)
    except RegistryError as exc:
        assert "duplicate gate name" in str(exc)
    else:
        raise AssertionError("duplicate gate name passed")

    missing_output = copy.deepcopy(sample)
    missing_output["gates"][0]["worktree_effect"] = "verify-checked-output"
    try:
        validate_model(missing_output)
    except RegistryError as exc:
        assert "must name its outputs" in str(exc)
    else:
        raise AssertionError("checked-output gate without outputs passed")

    drifted_lane = copy.deepcopy(sample)
    drifted_lane["gates"][0]["lanes"].remove("local-fast")
    try:
        validate_model(drifted_lane)
    except RegistryError as exc:
        assert "must agree" in str(exc)
    else:
        raise AssertionError("plan/lane drift passed")

    print("CI gate registry self-test passed")


def parse_args() -> argparse.Namespace:
    parser = argparse.ArgumentParser()
    parser.add_argument("--manifest", type=Path, default=DEFAULT_MANIFEST)
    subparsers = parser.add_subparsers(dest="command", required=True)
    validate_parser = subparsers.add_parser("validate")
    validate_parser.add_argument("--self-test", action="store_true")
    list_parser = subparsers.add_parser("list")
    list_parser.add_argument("--format", choices=("text", "json"), default="text")
    plan_parser = subparsers.add_parser("plan")
    plan_parser.add_argument("--mode", choices=("fast", "full"), required=True)
    plan_parser.add_argument("--format", choices=("lines", "json"), default="lines")
    return parser.parse_args()


def main() -> int:
    args = parse_args()
    if args.command == "validate" and args.self_test:
        self_test()
    registry = load_registry(args.manifest)
    try:
        gates = validate_live_registry(registry)
    except (OSError, RegistryError) as exc:
        print(f"CI gate registry error: {exc}", file=sys.stderr)
        return 1
    if args.command == "validate":
        print(f"CI gate registry OK: {len(gates)} named gates")
    elif args.command == "list":
        print_list(gates, args.format)
    else:
        print_plan(plan_rows(gates, args.mode), args.format)
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
