#!/usr/bin/env python3
"""Focused mutation and historical-diff tests for change routing."""

from __future__ import annotations

import subprocess
import tempfile
from pathlib import Path
from typing import Any

import change_routing as routing


def _git_changes(commit: str) -> list[routing.Change]:
    process = subprocess.run(
        [
            "git",
            "show",
            "--format=",
            "--name-status",
            "-z",
            "--find-renames",
            commit,
        ],
        cwd=routing.ROOT,
        check=True,
        stdout=subprocess.PIPE,
    )
    return routing.parse_name_status_z(process.stdout)


def _malformed_stream_fails_closed(
    registry: dict[str, Any],
    policy: dict[str, Any],
    data: bytes,
) -> None:
    try:
        routing.parse_name_status_z(data)
    except routing.RoutingError as exc:
        receipt = routing.compute_route(
            registry,
            policy,
            event="pull_request",
            changes=[],
            parse_error=str(exc),
        )
        assert receipt["decision"] == "full"
        assert receipt["full_reasons"][0].startswith("change-data-error:")
    else:
        raise AssertionError(f"malformed change stream passed: {data!r}")


def self_test(registry: dict[str, Any], policy: dict[str, Any]) -> None:
    docs = routing.compute_route(
        registry,
        policy,
        event="pull_request",
        changes=[routing.Change("M", "docs/repository-gates.md")],
    )
    assert docs["decision"] == "selective"
    assert docs["proposed_jobs"] == ["docs"]

    proof_sensitive = routing.compute_route(
        registry,
        policy,
        event="pull_request",
        changes=[routing.Change("M", "crates/nose-detect/src/witness.rs")],
    )
    assert "formal-obligations" in proof_sensitive["selected_gates"]
    assert "formal" in proof_sensitive["proposed_jobs"]

    for change, reason in (
        (routing.Change("M", "unowned/new-area.txt"), "unclassified-path"),
        (routing.Change("D", "docs/retired.md"), "unsafe-change-status:D"),
        (
            routing.Change("R100", "docs/new.md", "docs/old.md"),
            "unsafe-change-status:R100",
        ),
        (routing.Change("M", "Cargo.lock"), "global-policy-path"),
    ):
        receipt = routing.compute_route(
            registry,
            policy,
            event="pull_request",
            changes=[change],
        )
        assert receipt["decision"] == "full"
        assert reason in receipt["full_reasons"]

    for malformed in (
        b"M\0unterminated",
        b"M42\0docs/novel.md\0",
        b"A100\0docs/novel.md\0",
        b"R101\0docs/old.md\0docs/new.md\0",
    ):
        _malformed_stream_fails_closed(registry, policy, malformed)

    workflow_text = routing.DEFAULT_WORKFLOW.read_text()
    marker = "  docs:\n    name: docs wiki connectivity (awiki)\n"
    assert marker in workflow_text
    with tempfile.TemporaryDirectory() as temporary_directory:
        mutated_workflow = Path(temporary_directory) / "ci.yml"
        mutated_workflow.write_text(
            workflow_text.replace(
                marker,
                "  docs:\n"
                "    name: docs wiki connectivity (awiki)\n"
                "    if: ${{ false }}\n",
                1,
            )
        )
        try:
            routing.load_policy(workflow=mutated_workflow)
        except routing.RoutingError as exc:
            assert "job docs condition must be None" in str(exc)
        else:
            raise AssertionError("unexpected quality-job condition passed validation")

    for case in policy["historical_cases"]:
        receipt = routing.compute_route(
            registry,
            policy,
            event="pull_request",
            changes=_git_changes(case["commit"]),
        )
        assert (receipt["decision"] == "full") == case["expected_full"], case["name"]
        assert set(case["required_classes"]) <= set(receipt["matched_classes"]), case[
            "name"
        ]
        assert set(case["required_gates"]) <= set(receipt["selected_gates"]), case[
            "name"
        ]
        assert set(case["required_jobs"]) <= set(receipt["proposed_jobs"]), case["name"]

    print("change-aware CI routing self-test passed")
