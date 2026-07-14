#!/usr/bin/env python3
"""External Git receipt for the private #846 arbitration packet commitment."""

from __future__ import annotations

import argparse
import copy
import hashlib
import json
import subprocess
from pathlib import Path
from typing import Any

import default_head_heldout as heldout
import default_head_heldout_arbitration as arbitration
import default_head_heldout_vote_receipt as vote_receipt


ROOT = Path(__file__).resolve().parents[2]
COMMITMENT = arbitration.COMMITMENT
COMMITMENT_PATH = COMMITMENT.relative_to(ROOT).as_posix()
COMMITMENT_COMMIT = "bf4b54f5884b6c614ee09836626656605292bfec"
COMMITMENT_TREE = "f7811e8d4af4981a97f612bbe5b2accdfaf2455f"
COMMITMENT_PARENT = "30aa2d86c355c99520b54f4fb03834e2df9811c5"
COMMITMENT_SHA256 = "cc891ab8801a556aab472b977bcdd72681f56a70775883bc98e12c26420c3b8c"
COMMITMENT_BYTES = 2_175
COLLECTOR_TREE = "413e358c709d4009583b0d197de4fb5fd9168220"
COLLECTOR_SHA256 = "ab67edb67d2ceade6f857fb5c1d811254a253026c72eea2aead7be45e93c0754"
COLLECTOR_BYTES = 32_169
ARBITRATION_PACKET = {
    "schema": arbitration.ARBITER_PACKET_SCHEMA,
    "sha256": "b0426488847d400a232e4feaed352422919ea825148dc6c0179dce9f9e764005",
    "byte_length": 599_244,
    "candidate_count": 90,
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
    arbitration.require_equal(actual, expected, label)


def payload_sha256(payload: dict[str, Any]) -> str:
    encoded = json.dumps(payload, sort_keys=True, separators=(",", ":")).encode() + b"\n"
    return hashlib.sha256(encoded).hexdigest()


def validate_git_receipt() -> None:
    require_equal(
        git_text(["rev-parse", f"{COMMITMENT_COMMIT}^{{tree}}"]),
        COMMITMENT_TREE,
        "arbitration commitment tree",
    )
    require_equal(
        git_text(["show", "-s", "--format=%P", COMMITMENT_COMMIT]),
        COMMITMENT_PARENT,
        "arbitration commitment parent",
    )
    require_equal(
        git_text(
            ["diff-tree", "--no-commit-id", "--name-only", "-r", COMMITMENT_COMMIT]
        ).splitlines(),
        [COMMITMENT_PATH],
        "arbitration commitment paths",
    )
    require_equal(
        git_text(
            [
                "diff-tree",
                "--no-commit-id",
                "--name-only",
                "--diff-filter=A",
                "-r",
                COMMITMENT_COMMIT,
            ]
        ).splitlines(),
        [COMMITMENT_PATH],
        "arbitration commitment addition",
    )
    frozen = git_bytes(["show", f"{COMMITMENT_COMMIT}:{COMMITMENT_PATH}"])
    require_equal(len(frozen), COMMITMENT_BYTES, "arbitration commitment bytes")
    require_equal(
        hashlib.sha256(frozen).hexdigest(),
        COMMITMENT_SHA256,
        "arbitration commitment SHA",
    )
    require_equal(COMMITMENT.read_bytes(), frozen, "current arbitration commitment")
    require_equal(
        git_text(["rev-parse", f"{COMMITMENT_PARENT}^{{tree}}"]),
        COLLECTOR_TREE,
        "arbitration collector tree",
    )
    collector_path = "bench/labels/default_head_heldout_arbitration.py"
    collector = git_bytes(["show", f"{COMMITMENT_PARENT}:{collector_path}"])
    require_equal(len(collector), COLLECTOR_BYTES, "arbitration collector bytes")
    require_equal(
        hashlib.sha256(collector).hexdigest(),
        COLLECTOR_SHA256,
        "arbitration collector SHA",
    )
    for revision in (
        vote_receipt.VOTE_COMMIT,
        COMMITMENT_PARENT,
        COMMITMENT_COMMIT,
    ):
        result = subprocess.run(
            ["git", "merge-base", "--is-ancestor", revision, "HEAD"],
            cwd=ROOT,
            check=False,
        )
        if result.returncode != 0:
            raise ValueError(f"arbitration receipt ancestor {revision}: mismatch")


def validate_payload(payload: dict[str, Any]) -> None:
    require_equal(
        payload_sha256(payload), COMMITMENT_SHA256, "arbitration payload receipt"
    )
    arbitration.validate_commitment(payload)
    require_equal(
        payload["arbitration_packet"],
        ARBITRATION_PACKET,
        "private arbitration packet receipt",
    )
    require_equal(
        payload["provenance"]["collector_commit"],
        COMMITMENT_PARENT,
        "arbitration collector commit",
    )
    require_equal(
        payload["provenance"]["collector_tree"],
        COLLECTOR_TREE,
        "arbitration collector tree",
    )


def validate(_: argparse.Namespace) -> None:
    validate_git_receipt()
    validate_payload(heldout.read_json(COMMITMENT))
    print(
        "held-out arbitration receipt OK: 90 disagreements, "
        "private packet committed before judgment"
    )


def self_test(_: argparse.Namespace) -> None:
    validate_git_receipt()
    payload = heldout.read_json(COMMITMENT)
    validate_payload(payload)
    mutations: list[dict[str, Any]] = []
    changed = copy.deepcopy(payload)
    changed["arbitration_packet"]["sha256"] = "0" * 64
    mutations.append(changed)
    changed = copy.deepcopy(payload)
    changed["arbitration_packet"]["candidate_count"] = 89
    mutations.append(changed)
    changed = copy.deepcopy(payload)
    changed["raw_votes"]["files"] = []
    mutations.append(changed)
    changed = copy.deepcopy(payload)
    changed["provenance"]["collector_commit"] = vote_receipt.VOTE_PARENT
    mutations.append(changed)
    changed = copy.deepcopy(payload)
    changed["protocol"]["raw_votes_frozen_before_packet"] = 1
    mutations.append(changed)
    for mutation in mutations:
        try:
            validate_payload(mutation)
        except ValueError:
            continue
        raise AssertionError("invalid arbitration commitment mutation was accepted")
    print("default-head held-out arbitration receipt self-test passed")


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
