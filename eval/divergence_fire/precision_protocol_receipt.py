#!/usr/bin/env python3
"""History-bound receipt for the #848 sealed precision protocol."""

from __future__ import annotations

import argparse
import copy
import hashlib
import json
import subprocess
from pathlib import Path
from typing import Any

import precision_protocol


ROOT = Path(__file__).resolve().parents[2]
ARTIFACT_COMMIT = "adc66d08a37f254a411be226b9861008566d9dd6"
ARTIFACT_PARENT = "40b29412269403ca25fcb7868e824b33803095f8"
ARTIFACT_TREE = "e9855c21752ceea09583abdfd7681d0d075e9112"
ARTIFACTS = (
    (
        "eval/divergence_fire/precision_protocol_2026_07_14.v1.json",
        "100644",
        "45c1016c0fd151276832d6dc42f25f6f1b3283c0",
        394_680,
        "7270d8e3fee82c6a9305d9b0512c736276458404d5af8e984c4162a17a3cfcbd",
    ),
    (
        "eval/divergence_fire/precision_protocol_2026_07_14.v1.json.sha256",
        "100644",
        "68db2f655111c1921703c3ca34b694737285c9ea",
        104,
        "9a13df5aadb35eb2b1dd1a2e259c9cddfcaec1a1418c8e3c02b8219b5035154a",
    ),
)
ROOT_SEED_COMMITMENT = (
    "ddc931e1cf228e5d5c2531d1983f7800f90079fc75b39e9d8539befa70e5b7b7"
)
PRIVATE_PACKET_SHA256 = (
    "6b083a31beb204c0a992bd0e471b3ce6adc62224ca9c3989b4b6947cf1bc1be2"
)
PRIVATE_PACKET_BYTES = 7_563_153
BLIND_REPOSITORIES = 28
BLIND_CHANGES = 1_120
TEMPORAL_REPOSITORIES = 28


def require(condition: bool, label: str) -> None:
    if not condition:
        raise AssertionError(label)


def require_equal(actual: object, expected: object, label: str) -> None:
    require(actual == expected, f"{label}: expected {expected!r}, got {actual!r}")


def git_bytes(args: list[str]) -> bytes:
    completed = subprocess.run(
        ["git", *args], cwd=ROOT, check=False, capture_output=True
    )
    if completed.returncode != 0:
        raise AssertionError(
            f"git {' '.join(args)} failed: "
            f"{completed.stderr.decode(errors='replace').strip()}"
        )
    return completed.stdout


def git_text(args: list[str]) -> str:
    return git_bytes(args).decode().strip()


def receipt_record() -> dict[str, Any]:
    return {
        "artifact_commit": ARTIFACT_COMMIT,
        "artifact_parent": ARTIFACT_PARENT,
        "artifact_tree": ARTIFACT_TREE,
        "artifacts": [
            {
                "path": path,
                "mode": mode,
                "git_blob": blob,
                "bytes": byte_count,
                "sha256": digest,
            }
            for path, mode, blob, byte_count, digest in ARTIFACTS
        ],
        "root_seed_commitment": ROOT_SEED_COMMITMENT,
        "private_packet_sha256": PRIVATE_PACKET_SHA256,
        "private_packet_bytes": PRIVATE_PACKET_BYTES,
        "blind_repositories": BLIND_REPOSITORIES,
        "blind_changes": BLIND_CHANGES,
        "temporal_repositories": TEMPORAL_REPOSITORIES,
    }


def require_exact_record(record: dict[str, Any]) -> None:
    require_equal(record, receipt_record(), "receipt constants")


def frozen_payloads() -> dict[str, bytes]:
    return {
        path: git_bytes(["show", f"{ARTIFACT_COMMIT}:{path}"])
        for path, *_ in ARTIFACTS
    }


def validate_payloads(record: dict[str, Any], payloads: dict[str, bytes]) -> None:
    artifacts = record["artifacts"]
    expected_paths = [row["path"] for row in artifacts]
    require_equal(sorted(payloads), expected_paths, "receipt payload paths")
    for artifact in artifacts:
        path = artifact["path"]
        payload = payloads[path]
        require_equal(len(payload), artifact["bytes"], f"{path} bytes")
        require_equal(
            hashlib.sha256(payload).hexdigest(), artifact["sha256"], f"{path} SHA"
        )


def require_ancestor(ancestor: str, descendant: str, label: str) -> None:
    completed = subprocess.run(
        ["git", "merge-base", "--is-ancestor", ancestor, descendant],
        cwd=ROOT,
        check=False,
        capture_output=True,
    )
    require(completed.returncode == 0, label)


def validate_git_receipt(record: dict[str, Any]) -> dict[str, bytes]:
    require_exact_record(record)
    require_equal(
        git_text(["cat-file", "-t", ARTIFACT_COMMIT]), "commit", "artifact type"
    )
    require_equal(
        git_text(["rev-parse", f"{ARTIFACT_COMMIT}^{{tree}}"]),
        ARTIFACT_TREE,
        "artifact tree",
    )
    require_equal(
        git_text(["show", "-s", "--format=%P", ARTIFACT_COMMIT]),
        ARTIFACT_PARENT,
        "artifact parent",
    )
    paths = [path for path, *_ in ARTIFACTS]
    require_equal(paths, sorted(paths), "artifact path order")
    require_equal(
        git_text(
            ["diff-tree", "--no-commit-id", "--name-only", "-r", ARTIFACT_COMMIT]
        ).splitlines(),
        paths,
        "atomic artifact paths",
    )
    require_equal(
        git_text(
            [
                "diff-tree",
                "--no-commit-id",
                "--name-only",
                "--diff-filter=A",
                "-r",
                ARTIFACT_COMMIT,
            ]
        ).splitlines(),
        paths,
        "atomic artifact additions",
    )
    payloads = frozen_payloads()
    validate_payloads(record, payloads)
    for path, mode, blob, _, _ in ARTIFACTS:
        fields = git_text(["ls-tree", ARTIFACT_COMMIT, "--", path]).split()
        require_equal(fields[:3], [mode, "blob", blob], f"{path} Git identity")
        require_equal((ROOT / path).read_bytes(), payloads[path], f"{path} current bytes")
    require_ancestor(ARTIFACT_PARENT, ARTIFACT_COMMIT, "artifact parent ancestry")
    require_ancestor(ARTIFACT_COMMIT, "HEAD", "artifact commit ancestry")
    return payloads


def validate_document_receipt(record: dict[str, Any], payloads: dict[str, bytes]) -> None:
    public_path = str(precision_protocol.PUBLIC_PATH.relative_to(ROOT))
    document = json.loads(payloads[public_path])
    require_equal(
        document["provenance"]["freeze_parent"], ARTIFACT_PARENT, "freeze parent"
    )
    privacy = document["privacy"]
    require_equal(
        privacy["root_seed_commitment"],
        record["root_seed_commitment"],
        "root seed commitment",
    )
    require_equal(
        privacy["private_packet_sha256"],
        record["private_packet_sha256"],
        "private packet SHA",
    )
    require_equal(
        privacy["private_packet_bytes"],
        record["private_packet_bytes"],
        "private packet bytes",
    )
    population = document["population"]
    require_equal(
        population["blind"]["repository_count"],
        record["blind_repositories"],
        "blind repository count",
    )
    require_equal(
        population["blind"]["selected_change_count"],
        record["blind_changes"],
        "blind change count",
    )
    require_equal(
        population["temporal_canary_reserve"]["repository_count"],
        record["temporal_repositories"],
        "temporal repository count",
    )


def validate_receipt() -> None:
    record = receipt_record()
    payloads = validate_git_receipt(record)
    validate_document_receipt(record, payloads)
    precision_protocol.validate_public()


def expect_failure(function: Any, label: str) -> None:
    try:
        function()
    except (AssertionError, KeyError, TypeError, ValueError):
        return
    raise AssertionError(f"mutation accepted: {label}")


def self_test(_: argparse.Namespace) -> None:
    validate_receipt()
    record = receipt_record()
    value_mutations = (
        ("artifact_commit", "0" * 40),
        ("root_seed_commitment", "0" * 64),
        ("private_packet_sha256", "0" * 64),
        ("private_packet_bytes", PRIVATE_PACKET_BYTES + 1),
        ("blind_repositories", BLIND_REPOSITORIES - 1),
        ("blind_changes", BLIND_CHANGES - 1),
        ("temporal_repositories", TEMPORAL_REPOSITORIES - 1),
    )
    for field, value in value_mutations:
        mutated = copy.deepcopy(record)
        mutated[field] = value
        expect_failure(lambda row=mutated: require_exact_record(row), field)

    payloads = frozen_payloads()
    for path in sorted(payloads):
        mutated_payloads = dict(payloads)
        payload = bytearray(payloads[path])
        payload[0] ^= 1
        mutated_payloads[path] = bytes(payload)
        expect_failure(
            lambda rows=mutated_payloads: validate_payloads(record, rows), path
        )
    print("precision protocol receipt self-test OK")


def validate(_: argparse.Namespace) -> None:
    validate_receipt()
    print(
        "precision protocol receipt OK: "
        f"{BLIND_REPOSITORIES} blind repos, {BLIND_CHANGES} blind changes, "
        f"{TEMPORAL_REPOSITORIES} temporal repos"
    )


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
    except (AssertionError, KeyError, TypeError, ValueError) as error:
        raise SystemExit(str(error)) from error


if __name__ == "__main__":
    main()
