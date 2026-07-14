#!/usr/bin/env python3
"""External Git receipt for the atomic #846 held-out panel vote freeze."""

from __future__ import annotations

import argparse
import copy
import hashlib
import json
import subprocess
from collections import Counter
from pathlib import Path
from typing import Any

import default_head_heldout as heldout
import default_head_heldout_panel as panel


ROOT = Path(__file__).resolve().parents[2]
VOTE_COMMIT = "1d3add508f6d34fcc636cd00e1221d5f4ddd3fed"
VOTE_TREE = "f675ed94a1635014c9a1629364d7472a3596d694"
VOTE_PARENT = "52a05ee0c8f5bffb33476bfa11ad40009cb67e3c"
VOTE_RECEIPTS = [
    {
        "persona": "dedupe",
        "path": "bench/labels/default_head_heldout_votes_2026_07_14.heldout.dedupe.v3.json",
        "sha256": "c3159dd5ff61f2074080bb88b2a433874d9bf57912fd98c0271a4ff6a00597fc",
        "byte_length": 39_057,
    },
    {
        "persona": "pragmatic",
        "path": "bench/labels/default_head_heldout_votes_2026_07_14.heldout.pragmatic.v3.json",
        "sha256": "9abe8c3f8d7ad094cee920acf4812fdd803ece2a580d7add3b0d16202a75bb62",
        "byte_length": 43_596,
    },
    {
        "persona": "skeptic",
        "path": "bench/labels/default_head_heldout_votes_2026_07_14.heldout.skeptic.v3.json",
        "sha256": "cfdd7da6091ecec7001637a780829328e607710fe0a103b9b9af637413d81e2d",
        "byte_length": 45_825,
    },
]
EXPECTED_SUMMARIES = {
    "dedupe": {
        "worthy": 159,
        "not_worthy": 55,
        "reasons": {
            "coincidental-shape": 4,
            "extract-base": 24,
            "extract-data-table": 4,
            "extract-helper": 83,
            "generated": 26,
            "parameterize": 48,
            "parallel-by-design": 10,
            "trivial": 14,
            "type-def": 1,
        },
    },
    "pragmatic": {
        "worthy": 138,
        "not_worthy": 76,
        "reasons": {
            "coincidental-shape": 1,
            "extract-base": 11,
            "extract-data-table": 6,
            "extract-helper": 71,
            "generated": 15,
            "parameterize": 50,
            "parallel-by-design": 30,
            "trivial": 27,
            "type-def": 3,
        },
    },
    "skeptic": {
        "worthy": 143,
        "not_worthy": 71,
        "reasons": {
            "coincidental-shape": 3,
            "extract-base": 15,
            "extract-data-table": 6,
            "extract-helper": 80,
            "generated": 17,
            "parameterize": 42,
            "parallel-by-design": 29,
            "trivial": 21,
            "type-def": 1,
        },
    },
}


def git_bytes(args: list[str]) -> bytes:
    result = subprocess.run(
        ["git", *args], cwd=ROOT, check=False, capture_output=True
    )
    if result.returncode != 0:
        raise ValueError(
            f"git {' '.join(args)} failed: "
            f"{result.stderr.decode(errors='replace').strip()}"
        )
    return result.stdout


def git_text(args: list[str]) -> str:
    return git_bytes(args).decode().strip()


def require_equal(actual: object, expected: object, label: str) -> None:
    if actual != expected:
        raise ValueError(f"{label}: mismatch")


def vote_summary(payload: dict[str, Any]) -> dict[str, Any]:
    votes = payload["votes"]
    worthy = sum(vote["worthy"] for vote in votes)
    return {
        "worthy": worthy,
        "not_worthy": len(votes) - worthy,
        "reasons": dict(sorted(Counter(vote["reason"] for vote in votes).items())),
    }


def read_payload(value: bytes, label: str) -> dict[str, Any]:
    try:
        payload = json.loads(value)
    except json.JSONDecodeError as error:
        raise ValueError(f"{label}: invalid JSON: {error}") from error
    if not isinstance(payload, dict):
        raise ValueError(f"{label}: expected an object")
    return payload


def validate_git_receipt() -> dict[str, dict[str, Any]]:
    require_equal(
        git_text(["rev-parse", f"{VOTE_COMMIT}^{{tree}}"]),
        VOTE_TREE,
        "vote tree",
    )
    require_equal(
        git_text(["show", "-s", "--format=%P", VOTE_COMMIT]),
        VOTE_PARENT,
        "vote parent",
    )
    expected_paths = [record["path"] for record in VOTE_RECEIPTS]
    require_equal(
        git_text(
            ["diff-tree", "--no-commit-id", "--name-only", "-r", VOTE_COMMIT]
        ).splitlines(),
        expected_paths,
        "atomic vote commit paths",
    )
    require_equal(
        git_text(
            [
                "diff-tree",
                "--no-commit-id",
                "--name-only",
                "--diff-filter=A",
                "-r",
                VOTE_COMMIT,
            ]
        ).splitlines(),
        expected_paths,
        "atomic vote additions",
    )
    payloads: dict[str, dict[str, Any]] = {}
    for record in VOTE_RECEIPTS:
        persona = record["persona"]
        path = record["path"]
        frozen = git_bytes(["show", f"{VOTE_COMMIT}:{path}"])
        require_equal(len(frozen), record["byte_length"], f"{persona} frozen bytes")
        require_equal(
            hashlib.sha256(frozen).hexdigest(),
            record["sha256"],
            f"{persona} frozen SHA",
        )
        current = (ROOT / path).read_bytes()
        require_equal(current, frozen, f"{persona} current bytes")
        payloads[persona] = read_payload(frozen, f"{persona} frozen vote")
    subprocess.run(
        ["git", "merge-base", "--is-ancestor", VOTE_COMMIT, "HEAD"],
        cwd=ROOT,
        check=True,
    )
    return payloads


def validate_vote_set(payloads: dict[str, dict[str, Any]]) -> None:
    commitment = panel.read_commitment()
    require_equal(set(payloads), set(heldout.PERSONAS), "vote personas")
    id_sets: dict[str, set[str]] = {}
    for persona in heldout.PERSONAS:
        payload = payloads[persona]
        panel.validate_public_vote_payload(payload, persona, commitment)
        require_equal(
            vote_summary(payload), EXPECTED_SUMMARIES[persona], f"{persona} summary"
        )
        id_sets[persona] = {vote["blind_id"] for vote in payload["votes"]}
    for index, first in enumerate(heldout.PERSONAS):
        for second in heldout.PERSONAS[index + 1 :]:
            require_equal(
                id_sets[first] & id_sets[second],
                set(),
                f"{first}/{second} blind-ID overlap",
            )


def validate(_: argparse.Namespace) -> None:
    payloads = validate_git_receipt()
    validate_vote_set(payloads)
    print("held-out vote receipt OK: 3 personas, 642 votes, one atomic commit")


def self_test(_: argparse.Namespace) -> None:
    payloads = validate_git_receipt()
    validate_vote_set(payloads)
    mutations: list[dict[str, dict[str, Any]]] = []
    changed = copy.deepcopy(payloads)
    changed["dedupe"]["votes"][1]["blind_id"] = changed["dedupe"]["votes"][0][
        "blind_id"
    ]
    mutations.append(changed)
    changed = copy.deepcopy(payloads)
    changed["pragmatic"]["attestation"]["assigned_material_only"] = False
    mutations.append(changed)
    changed = copy.deepcopy(payloads)
    changed["skeptic"]["votes"][0]["worthy"] = not changed["skeptic"]["votes"][0][
        "worthy"
    ]
    mutations.append(changed)
    changed = copy.deepcopy(payloads)
    changed["dedupe"]["source_packet"]["sha256"] = "0" * 64
    mutations.append(changed)
    changed = copy.deepcopy(payloads)
    changed["skeptic"] = changed.pop("pragmatic")
    mutations.append(changed)
    for mutation in mutations:
        try:
            validate_vote_set(mutation)
        except ValueError:
            continue
        raise AssertionError("invalid coordinated held-out vote mutation was accepted")
    print("default-head held-out vote receipt self-test passed")


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
