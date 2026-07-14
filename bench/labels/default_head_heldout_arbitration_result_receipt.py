#!/usr/bin/env python3
"""External Git receipt for the frozen #846 blind arbitration result."""

from __future__ import annotations

import argparse
import copy
import hashlib
import subprocess
from collections import Counter
from pathlib import Path
from typing import Any

import default_head_heldout as heldout
import default_head_heldout_arbitration_receipt as arbitration_receipt
import default_head_heldout_arbitration_result as result


ROOT = Path(__file__).resolve().parents[2]
RESULT = result.RESULT
RESULT_PATH = RESULT.relative_to(ROOT).as_posix()
RESULT_COMMIT = "e419e48be0e3dc780a553b8b9b0be51a922538ec"
RESULT_TREE = "bdb531d691af3e1f0adfbfd0213e4fb68e9107dc"
RESULT_PARENT = "b0f60803afa65794480c9ffdb364ecd19999d4a6"
RESULT_SHA256 = "1458a26f08005fd9b2d3d6877a5d1d43092590a4414bde239b8c656088077865"
RESULT_BYTES = 17_700
EXPECTED_SUMMARY = {
    "worthy": 55,
    "not_worthy": 35,
    "reasons": {
        "coincidental-shape": 1,
        "extract-base": 9,
        "extract-data-table": 3,
        "extract-helper": 23,
        "generated": 2,
        "parameterize": 20,
        "parallel-by-design": 18,
        "trivial": 12,
        "type-def": 2,
    },
}


def git_bytes(args: list[str]) -> bytes:
    completed = subprocess.run(
        ["git", *args], cwd=ROOT, check=False, capture_output=True
    )
    if completed.returncode != 0:
        raise ValueError(
            f"git {' '.join(args)} failed: "
            f"{completed.stderr.decode(errors='replace').strip()}"
        )
    return completed.stdout


def git_text(args: list[str]) -> str:
    return git_bytes(args).decode().strip()


def require_equal(actual: object, expected: object, label: str) -> None:
    result.require_equal(actual, expected, label)


def result_summary(payload: dict[str, Any]) -> dict[str, Any]:
    votes = payload["votes"]
    worthy = sum(vote["worthy"] for vote in votes)
    return {
        "worthy": worthy,
        "not_worthy": len(votes) - worthy,
        "reasons": dict(sorted(Counter(vote["reason"] for vote in votes).items())),
    }


def validate_git_receipt() -> None:
    require_equal(
        git_text(["rev-parse", f"{RESULT_COMMIT}^{{tree}}"]),
        RESULT_TREE,
        "arbitration result tree",
    )
    require_equal(
        git_text(["show", "-s", "--format=%P", RESULT_COMMIT]),
        RESULT_PARENT,
        "arbitration result parent",
    )
    require_equal(
        git_text(
            ["diff-tree", "--no-commit-id", "--name-only", "-r", RESULT_COMMIT]
        ).splitlines(),
        [RESULT_PATH],
        "arbitration result paths",
    )
    require_equal(
        git_text(
            [
                "diff-tree",
                "--no-commit-id",
                "--name-only",
                "--diff-filter=A",
                "-r",
                RESULT_COMMIT,
            ]
        ).splitlines(),
        [RESULT_PATH],
        "arbitration result addition",
    )
    frozen = git_bytes(["show", f"{RESULT_COMMIT}:{RESULT_PATH}"])
    require_equal(len(frozen), RESULT_BYTES, "arbitration result bytes")
    require_equal(
        hashlib.sha256(frozen).hexdigest(),
        RESULT_SHA256,
        "arbitration result SHA",
    )
    require_equal(RESULT.read_bytes(), frozen, "current arbitration result")
    for ancestor, descendant, label in (
        (
            arbitration_receipt.COMMITMENT_COMMIT,
            RESULT_PARENT,
            "commitment-before-result parent",
        ),
        (RESULT_COMMIT, "HEAD", "arbitration result ancestry"),
    ):
        completed = subprocess.run(
            ["git", "merge-base", "--is-ancestor", ancestor, descendant],
            cwd=ROOT,
            check=False,
            capture_output=True,
        )
        if completed.returncode != 0:
            raise ValueError(f"{label}: mismatch")


def validate_payload(payload: dict[str, Any]) -> None:
    commitment = result.read_commitment()
    result.validate_public_result_payload(payload, commitment)
    require_equal(result_summary(payload), EXPECTED_SUMMARY, "arbitration summary")


def validate(_: argparse.Namespace) -> None:
    validate_git_receipt()
    payload = heldout.read_json(RESULT)
    validate_payload(payload)
    print("held-out arbitration result receipt OK: 90 decisions, 55 worthy")


def self_test(_: argparse.Namespace) -> None:
    validate_git_receipt()
    payload = heldout.read_json(RESULT)
    validate_payload(payload)
    mutations: list[dict[str, Any]] = []
    changed = copy.deepcopy(payload)
    changed["source_packet"]["sha256"] = "0" * 64
    mutations.append(changed)
    changed = copy.deepcopy(payload)
    changed["attestation"]["assigned_material_only"] = 1
    mutations.append(changed)
    changed = copy.deepcopy(payload)
    changed["votes"][0]["worthy"] = not changed["votes"][0]["worthy"]
    mutations.append(changed)
    changed = copy.deepcopy(payload)
    changed["votes"][1]["blind_id"] = changed["votes"][0]["blind_id"]
    mutations.append(changed)
    changed = copy.deepcopy(payload)
    changed["votes"].pop()
    mutations.append(changed)
    for mutation in mutations:
        try:
            validate_payload(mutation)
        except ValueError:
            continue
        raise AssertionError("invalid arbitration result mutation was accepted")
    print("default-head held-out arbitration result receipt self-test passed")


def parser() -> argparse.ArgumentParser:
    root = argparse.ArgumentParser()
    commands = root.add_subparsers(dest="command", required=True)
    validate_parser = commands.add_parser("validate")
    validate_parser.set_defaults(run=validate)
    self_parser = commands.add_parser("self-test")
    self_parser.set_defaults(run=self_test)
    return root


def main() -> None:
    args = parser().parse_args()
    try:
        args.run(args)
    except ValueError as error:
        raise SystemExit(str(error)) from error


if __name__ == "__main__":
    main()
