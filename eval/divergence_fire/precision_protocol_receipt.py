#!/usr/bin/env python3
"""History-bound receipt for the corrected #848 precision seal."""

from __future__ import annotations

import argparse
import copy
import hashlib
import json
from pathlib import Path
from typing import Any, Callable

import precision_protocol


ROOT = Path(__file__).resolve().parents[2]
ARTIFACT_COMMIT = "a0095807af06098eaf7aa1cdf3f5ae6c06b3065a"
ARTIFACT_PARENT = "7b2d56e809bbb749448de821332edb367d236e5a"
ARTIFACT_TREE = "e68d0c3b59f9557a2ff2af5ef5dd06324935f52e"
ARTIFACTS = (
    (
        "eval/divergence_fire/precision_protocol_2026_07_14.v2.json",
        "100644",
        "2d4d09609351c8553a0f82d22a9fc1fa8ff7e57d",
        400_227,
        "5783af5087f16646d4f9fa1269b65eca0ccec508ecd79a78a77d293fb88b5d39",
    ),
    (
        "eval/divergence_fire/precision_protocol_2026_07_14.v2.json.sha256",
        "100644",
        "c9605ca6e2a43683e941e25588f1b6dfb016a0d1",
        104,
        "bceaeae7d6a30874b77b5e9522c4a54e37bb0dca2ed7a70f6ad9d5b609d2327e",
    ),
)
ROOT_SEED_COMMITMENT = (
    "112396a1918ec744ffd3067c2c230863d5eabf7d5324706d5ca0ed3ef6871480"
)
PRIVATE_PACKET_SHA256 = (
    "feeba9b78cc68c6a254a0bda2f4c6ac4e87eefe519f60c293e845eee1478225c"
)
PRIVATE_PACKET_BYTES = 11_372_380
BLIND_REPOSITORIES = 28
BLIND_CHANGES = 1_120
BLIND_FINDING_SUPPORT = 100
BLIND_TARGET_SUPPORT = 100
BLIND_REPOSITORY_SUPPORT = 20
TEMPORAL_REPOSITORIES = 28
TEMPORAL_CHANGES = 1_000
TEMPORAL_CHECKPOINT_DAYS = (30, 60, 90, 120, 150, 180)


def require(condition: bool, label: str) -> None:
    if not condition:
        raise AssertionError(label)


def require_equal(actual: object, expected: object, label: str) -> None:
    require(actual == expected, f"{label}: expected {expected!r}, got {actual!r}")


def git_bytes(args: list[str], check: bool = True) -> bytes:
    completed = precision_protocol.git_bytes(*args, cwd=ROOT, check=False)
    if check and completed.returncode != 0:
        raise AssertionError(
            f"git {' '.join(args)} failed: "
            f"{completed.stderr.decode(errors='backslashreplace').strip()}"
        )
    return completed.stdout


def git_text(args: list[str]) -> str:
    return git_bytes(args).decode("ascii", "strict").strip()


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
        "blind_finding_support": BLIND_FINDING_SUPPORT,
        "blind_target_support": BLIND_TARGET_SUPPORT,
        "blind_repository_support": BLIND_REPOSITORY_SUPPORT,
        "temporal_repositories": TEMPORAL_REPOSITORIES,
        "temporal_changes": TEMPORAL_CHANGES,
        "temporal_checkpoint_days": list(TEMPORAL_CHECKPOINT_DAYS),
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
    completed = precision_protocol.git_bytes(
        "merge-base", "--is-ancestor", ancestor, descendant,
        cwd=ROOT, check=False,
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
    stop = document["sampling_stop_rule"]
    require_equal(
        stop["minimum_strict_findings"],
        record["blind_finding_support"],
        "blind finding support",
    )
    require_equal(
        stop["minimum_strict_targets"],
        record["blind_target_support"],
        "blind target support",
    )
    require_equal(
        stop["minimum_complete_repositories"],
        record["blind_repository_support"],
        "blind repository support",
    )
    temporal = population["temporal_canary_reserve"]
    require_equal(
        temporal["repository_count"],
        record["temporal_repositories"],
        "temporal repository count",
    )
    require_equal(
        temporal["sampling"]["target_change_count"],
        record["temporal_changes"],
        "temporal change count",
    )
    require_equal(
        temporal["sampling"]["checkpoint_days_after_seal"],
        record["temporal_checkpoint_days"],
        "temporal checkpoints",
    )


def validate_receipt() -> None:
    record = receipt_record()
    payloads = validate_git_receipt(record)
    validate_document_receipt(record, payloads)
    precision_protocol.validate_public()


def expect_failure(function: Callable[[], None], label: str) -> None:
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
        ("blind_finding_support", BLIND_FINDING_SUPPORT - 1),
        ("blind_target_support", BLIND_TARGET_SUPPORT - 1),
        ("blind_repository_support", BLIND_REPOSITORY_SUPPORT - 1),
        ("temporal_repositories", TEMPORAL_REPOSITORIES - 1),
        ("temporal_changes", TEMPORAL_CHANGES - 1),
        ("temporal_checkpoint_days", [30, 60]),
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
        f"{BLIND_FINDING_SUPPORT} finding support, "
        f"{TEMPORAL_CHANGES} temporal changes"
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
