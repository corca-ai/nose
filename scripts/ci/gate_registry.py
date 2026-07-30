#!/usr/bin/env python3
"""Validate and render the authoritative repository CI gate inventory."""

from __future__ import annotations

import argparse
import copy
import json
import re
import sys
import tempfile
from pathlib import Path
from typing import Any


ROOT = Path(__file__).resolve().parents[2]
DEFAULT_MANIFEST = ROOT / "scripts/ci/gates.json"
DEFAULT_DISPATCHER = ROOT / "scripts/check-ci-local.sh"
DEFAULT_CI_WORKFLOW = ROOT / ".github/workflows/ci.yml"
DEFAULT_NIGHTLY_WORKFLOW = ROOT / ".github/workflows/corpus-verify.yml"
DEFAULT_RELEASE_WORKFLOW = ROOT / ".github/workflows/release.yml"

SCHEMA = "nose.ci-gates.v2"
WORKTREE_EFFECTS = {"read-only", "verify-checked-output"}
LOCAL_PLAN_LANES = {"fast": "local-fast", "full": "local-full"}
LIFECYCLE_GATE = "evidence-artifacts"
HOSTED_GATE_PREFIX = "gate · "
REQUIRED_GATE_FIELDS = {
    "name",
    "owner",
    "implementation",
    "tools",
    "inputs",
    "worktree_effect",
    "outputs",
    "cache",
    "parallel_safe",
    "resource_group",
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


def hosted_gate_names_from_workflow(path: Path) -> set[str]:
    text = path.read_text()
    names: set[str] = set()
    current_name: str | None = None
    current_body: list[str] = []

    def validate_step() -> None:
        if not current_body:
            return
        called = re.findall(r"--gate\s+([a-z0-9-]+)", "\n".join(current_body))
        if not called:
            return
        if len(called) != 1:
            raise RegistryError(
                f"hosted workflow step {current_name!r} must call exactly one named gate"
            )
        expected_name = f"{HOSTED_GATE_PREFIX}{called[0]}"
        if current_name != expected_name:
            raise RegistryError(
                f"hosted workflow gate {called[0]} must use step name {expected_name!r}"
            )
        if called[0] in names:
            raise RegistryError(f"hosted workflow calls gate {called[0]} more than once")
        names.add(called[0])

    for line in text.splitlines():
        if re.match(r"^ {6}- ", line):
            validate_step()
            name_match = re.match(r"^ {6}- name:\s*(.+?)\s*$", line)
            current_name = name_match.group(1) if name_match else None
            current_body = [line]
        elif current_body:
            current_body.append(line)
    validate_step()
    return names


def workflow_job_blocks(path: Path) -> dict[str, str]:
    blocks: dict[str, list[str]] = {}
    current: str | None = None
    in_jobs = False
    for line in path.read_text().splitlines():
        if line == "jobs:":
            in_jobs = True
            continue
        if not in_jobs:
            continue
        job_match = re.match(r"^ {2}([a-z0-9-]+):\s*$", line)
        if job_match:
            current = job_match.group(1)
            blocks[current] = [line]
        elif current is not None:
            blocks[current].append(line)
    return {name: "\n".join(lines) for name, lines in blocks.items()}


def validate_hosted_timing_policy(path: Path) -> None:
    text = path.read_text()
    blocks = workflow_job_blocks(path)
    timing = blocks.get("hosted-timing")
    if timing is None:
        raise RegistryError("hosted workflow must define the hosted-timing job")
    if "    if: ${{ always() }}" not in timing:
        raise RegistryError("hosted-timing must run with always()")

    needs_match = re.search(
        r"^ {4}needs:\n(?P<rows>(?:^ {6}- [a-z0-9-]+\n?)+)",
        timing,
        re.MULTILINE,
    )
    if needs_match is None:
        raise RegistryError("hosted-timing must list every quality job in needs")
    needs = set(re.findall(r"^ {6}- ([a-z0-9-]+)$", needs_match.group("rows"), re.MULTILINE))
    expected = set(blocks) - {"hosted-timing"}
    if needs != expected:
        raise RegistryError(
            "hosted-timing needs mismatch: "
            f"missing={sorted(expected - needs)}, extra={sorted(needs - expected)}"
        )

    header = text.split("\njobs:", maxsplit=1)[0]
    if re.search(r"^ {2}actions:\s*", header, re.MULTILINE):
        raise RegistryError("Actions permission must not be granted workflow-wide")
    if (
        "    permissions:\n"
        "      actions: read\n"
        "      contents: read\n" not in timing
    ):
        raise RegistryError("hosted-timing requires job-local read-only permissions")
    if "          persist-credentials: false" not in timing:
        raise RegistryError("hosted-timing checkout must not persist credentials")
    required_concurrency_tokens = {
        "github.event_name == 'pull_request'",
        "github.event.pull_request.number",
        "github.run_id",
        "cancel-in-progress: ${{ github.event_name == 'pull_request' }}",
    }
    missing_tokens = sorted(token for token in required_concurrency_tokens if token not in text)
    if missing_tokens:
        raise RegistryError(
            f"hosted PR-only concurrency policy drifted: missing {missing_tokens}"
        )


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
    plan_order_by_name: dict[str, dict[str, int]] = {
        mode: {} for mode in LOCAL_PLAN_LANES
    }
    plan_dependencies: dict[str, dict[str, list[str]]] = {
        mode: {} for mode in LOCAL_PLAN_LANES
    }
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
        if not isinstance(gate["parallel_safe"], bool):
            raise RegistryError(f"gate {name}: parallel_safe must be boolean")
        resource_group = gate["resource_group"]
        if resource_group is not None and (
            not isinstance(resource_group, str)
            or re.fullmatch(r"[a-z0-9-]+", resource_group) is None
        ):
            raise RegistryError(
                f"gate {name}: resource_group must be null or a kebab-case name"
            )
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
            if not isinstance(plan, dict) or set(plan) != {
                "order",
                "label",
                "args",
                "depends_on",
            }:
                raise RegistryError(
                    f"gate {name}: {mode} plan needs order, label, args, and depends_on"
                )
            order = plan["order"]
            if not isinstance(order, int) or order <= 0:
                raise RegistryError(f"gate {name}: {mode} order must be positive")
            if order in plan_orders[mode]:
                raise RegistryError(f"{mode} plan reuses order {order}")
            plan_orders[mode].add(order)
            plan_order_by_name[mode][name] = order
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
            dependencies = plan["depends_on"]
            if (
                not isinstance(dependencies, list)
                or any(
                    not isinstance(dependency, str)
                    or re.fullmatch(r"[a-z0-9-]+", dependency) is None
                    for dependency in dependencies
                )
                or len(dependencies) != len(set(dependencies))
            ):
                raise RegistryError(
                    f"gate {name}: {mode} depends_on must contain unique gate names"
                )
            if name in dependencies:
                raise RegistryError(f"gate {name}: {mode} cannot depend on itself")
            plan_dependencies[mode][name] = dependencies

    for mode in LOCAL_PLAN_LANES:
        planned_names = set(plan_order_by_name[mode])
        for name, dependencies in plan_dependencies[mode].items():
            unknown = set(dependencies) - planned_names
            if unknown:
                raise RegistryError(
                    f"gate {name}: {mode} depends on unplanned gates {sorted(unknown)}"
                )
            late = [
                dependency
                for dependency in dependencies
                if plan_order_by_name[mode][dependency] >= plan_order_by_name[mode][name]
            ]
            if late:
                raise RegistryError(
                    f"gate {name}: {mode} dependencies must have lower order: {sorted(late)}"
                )

    gates_by_name = {gate["name"]: gate for gate in gates}
    lifecycle = gates_by_name.get(LIFECYCLE_GATE)
    if lifecycle is not None:
        if lifecycle["parallel_safe"]:
            raise RegistryError(
                f"gate {LIFECYCLE_GATE}: lifecycle join must remain an ordering barrier"
            )
        for mode in LOCAL_PLAN_LANES:
            checked_output_gates = {
                gate["name"]
                for gate in gates
                if gate["worktree_effect"] == "verify-checked-output"
                and mode in gate["plans"]
            }
            if mode not in lifecycle["plans"]:
                if checked_output_gates:
                    raise RegistryError(
                        f"gate {LIFECYCLE_GATE}: missing {mode} lifecycle join"
                    )
                continue
            dependencies = set(plan_dependencies[mode][LIFECYCLE_GATE])
            missing = checked_output_gates - dependencies
            if missing:
                raise RegistryError(
                    f"gate {LIFECYCLE_GATE}: {mode} must depend on every "
                    f"checked-output gate: missing {sorted(missing)}"
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
    hosted_workflow_names = hosted_gate_names_from_workflow(ci_workflow)
    if hosted_workflow_names != pull_request_names:
        raise RegistryError(
            "hosted timing step names/pull-request lane mismatch: "
            f"registry-only={sorted(pull_request_names - hosted_workflow_names)}, "
            f"workflow-only={sorted(hosted_workflow_names - pull_request_names)}"
        )
    validate_hosted_timing_policy(ci_workflow)

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
    if '"actions": "read"' not in release_text:
        raise RegistryError(
            "release workflow must grant read-only Actions access to hosted timing"
        )
    return gates


def plan_rows(gates: list[dict[str, Any]], mode: str) -> list[dict[str, Any]]:
    return sorted(
        (
            {
                "name": gate["name"],
                "parallel_safe": gate["parallel_safe"],
                "resource_group": gate["resource_group"],
                **gate["plans"][mode],
            }
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
    print(
        "gate\tlanes\tworktree\tparallel\tresource group\tcache\towner\tfocused command"
    )
    for gate in gates:
        print(
            "\t".join(
                [
                    gate["name"],
                    ",".join(gate["lanes"]),
                    gate["worktree_effect"],
                    str(gate["parallel_safe"]).lower(),
                    gate["resource_group"] or "-",
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
                "parallel_safe": True,
                "resource_group": None,
                "lanes": ["local-fast"],
                "lane_reason": "sample rationale",
                "focused_command": "./scripts/check-ci-local.sh --gate sample",
                "plans": {
                    "fast": {
                        "order": 10,
                        "label": "sample",
                        "args": [],
                        "depends_on": [],
                    }
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

    bad_dependency = copy.deepcopy(sample)
    bad_dependency["gates"][0]["plans"]["fast"]["depends_on"] = ["missing"]
    try:
        validate_model(bad_dependency)
    except RegistryError as exc:
        assert "unplanned gates" in str(exc)
    else:
        raise AssertionError("unknown plan dependency passed")

    bad_group = copy.deepcopy(sample)
    bad_group["gates"][0]["resource_group"] = "not_a_group"
    try:
        validate_model(bad_group)
    except RegistryError as exc:
        assert "resource_group" in str(exc)
    else:
        raise AssertionError("invalid resource group passed")

    with tempfile.TemporaryDirectory() as directory:
        workflow = Path(directory) / "ci.yml"
        workflow.write_text(
            "jobs:\n"
            "  test:\n"
            "    steps:\n"
            f"      - name: {HOSTED_GATE_PREFIX}sample\n"
            "        run: ./scripts/check-ci-local.sh --gate sample\n"
        )
        assert hosted_gate_names_from_workflow(workflow) == {"sample"}
        workflow.write_text(
            "jobs:\n"
            "  test:\n"
            "    steps:\n"
            "      - name: wrong\n"
            "        run: ./scripts/check-ci-local.sh --gate sample\n"
        )
        try:
            hosted_gate_names_from_workflow(workflow)
        except RegistryError as exc:
            assert "must use step name" in str(exc)
        else:
            raise AssertionError("misnamed hosted timing step passed")

        valid_workflow = (
            "name: ci\n"
            "permissions:\n"
            "  contents: read\n"
            "concurrency:\n"
            "  group: ci-${{ github.event_name == 'pull_request' "
            "&& github.event.pull_request.number || github.run_id }}\n"
            "  cancel-in-progress: ${{ github.event_name == 'pull_request' }}\n"
            "jobs:\n"
            "  test:\n"
            "    steps:\n"
            f"      - name: {HOSTED_GATE_PREFIX}sample\n"
            "        run: ./scripts/check-ci-local.sh --gate sample\n"
            "  hosted-timing:\n"
            "    if: ${{ always() }}\n"
            "    needs:\n"
            "      - test\n"
            "    permissions:\n"
            "      actions: read\n"
            "      contents: read\n"
            "    steps:\n"
            "      - uses: actions/checkout@v7\n"
            "        with:\n"
            "          persist-credentials: false\n"
        )
        workflow.write_text(valid_workflow)
        validate_hosted_timing_policy(workflow)
        policy_mutations = [
            (
                "missing complete needs",
                valid_workflow.replace("      - test\n", ""),
                "must list every quality job",
            ),
            (
                "missing Actions permission",
                valid_workflow.replace("      actions: read\n", ""),
                "job-local read-only permissions",
            ),
            (
                "workflow-wide Actions permission",
                valid_workflow.replace(
                    "permissions:\n",
                    "permissions:\n  actions: read\n",
                    1,
                ),
                "must not be granted workflow-wide",
            ),
            (
                "persisted checkout credential",
                valid_workflow.replace("          persist-credentials: false\n", ""),
                "must not persist credentials",
            ),
            (
                "broad cancellation",
                valid_workflow.replace(
                    "cancel-in-progress: ${{ github.event_name == 'pull_request' }}",
                    "cancel-in-progress: true",
                ),
                "PR-only concurrency policy drifted",
            ),
            (
                "shared non-PR key",
                valid_workflow.replace("github.run_id", "'shared'"),
                "PR-only concurrency policy drifted",
            ),
        ]
        for name, mutated, expected in policy_mutations:
            workflow.write_text(mutated)
            try:
                validate_hosted_timing_policy(workflow)
            except RegistryError as exc:
                assert expected in str(exc), (name, exc)
            else:
                raise AssertionError(f"{name} passed")

    lifecycle_join = copy.deepcopy(sample)
    producer = lifecycle_join["gates"][0]
    producer["worktree_effect"] = "verify-checked-output"
    producer["outputs"] = ["checked.json"]
    lifecycle = copy.deepcopy(producer)
    lifecycle.update(
        {
            "name": LIFECYCLE_GATE,
            "owner": "artifact lifecycle",
            "worktree_effect": "read-only",
            "outputs": [],
            "parallel_safe": False,
            "focused_command": (
                f"./scripts/check-ci-local.sh --gate {LIFECYCLE_GATE}"
            ),
        }
    )
    lifecycle["plans"]["fast"] = {
        "order": 20,
        "label": "artifact lifecycle",
        "args": [],
        "depends_on": ["sample"],
    }
    lifecycle_join["gates"].append(lifecycle)
    validate_model(lifecycle_join)

    missing_join = copy.deepcopy(lifecycle_join)
    missing_join["gates"][1]["plans"]["fast"]["depends_on"] = []
    try:
        validate_model(missing_join)
    except RegistryError as exc:
        assert "must depend on every checked-output gate" in str(exc)
    else:
        raise AssertionError("lifecycle join without checked-output dependency passed")

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
