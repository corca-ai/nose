#!/usr/bin/env python3
"""External Git receipt for the private-packet #846 commitment."""

from __future__ import annotations

import argparse
import copy
import hashlib
import json
import subprocess
from pathlib import Path
from typing import Any

import default_head_heldout as heldout


ROOT = Path(__file__).resolve().parents[2]
COMMITMENT = heldout.COMMITMENT
COMMITMENT_PATH = COMMITMENT.relative_to(ROOT).as_posix()
COMMITMENT_COMMIT = "6ec0e95f57360dcf2b9e2d99af1171a6ec6452a0"
COMMITMENT_TREE = "2a7887ae5c835c8f1b994719df5c7537917fac1f"
COMMITMENT_SHA256 = "747b1049245b9d439b7faa2639712465216904e5f0e9546cfde0f82271a657d6"
UNSEAL_COMMIT = "37319a18c71d76259fc632e2a685c8006f939fb0"
UNSEAL_TREE = "03ee6e9c4ab40727c58b2716725b914d3fafc643"
COLLECTOR_SHA256 = "41afb223d153bdb86a62e7ecd4427a5ea0ecf2268a6ac1595c451803cba28c7d"
ROOT_SEED_COMMITMENT = "5fb54160ab4870a0b1bed00ae64e182bf163fd2724a38a3fc5730569f2675323"
SEALED_SELECTION_SHA256 = "ea94c8764a23c517497fb7e399832c11a69d3c2dd306197c1fbec75339d25d9b"
PACKET_RECEIPTS = [
    {
        "byte_length": 6_083_360,
        "candidate_count": 214,
        "persona": "dedupe",
        "schema": heldout.PRIVATE_PACKET_SCHEMA,
        "sha256": "57d4ddf561a4df142cb85f9b0970033cf639399145f2ab5eac5f0186685d3de3",
    },
    {
        "byte_length": 6_083_363,
        "candidate_count": 214,
        "persona": "pragmatic",
        "schema": heldout.PRIVATE_PACKET_SCHEMA,
        "sha256": "5375c63a612f2e36f60f1e2b91c51a87af8b8078b3bd0baf7a2757e709d6e92d",
    },
    {
        "byte_length": 6_083_361,
        "candidate_count": 214,
        "persona": "skeptic",
        "schema": heldout.PRIVATE_PACKET_SCHEMA,
        "sha256": "32db34ec0e441ea1a79de3fd776fa2b83a78082a0e03fded7afef061f99ad3b1",
    },
]


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


def payload_sha256(payload: dict[str, Any]) -> str:
    encoded = json.dumps(payload, sort_keys=True, separators=(",", ":")).encode() + b"\n"
    return hashlib.sha256(encoded).hexdigest()


def validate_git_receipt() -> None:
    require_equal(
        git_text(["rev-parse", f"{COMMITMENT_COMMIT}^{{tree}}"]),
        COMMITMENT_TREE,
        "commitment tree",
    )
    require_equal(
        git_text(["show", "-s", "--format=%P", COMMITMENT_COMMIT]),
        UNSEAL_COMMIT,
        "commitment parent",
    )
    frozen = git_bytes(["show", f"{COMMITMENT_COMMIT}:{COMMITMENT_PATH}"])
    require_equal(
        hashlib.sha256(frozen).hexdigest(), COMMITMENT_SHA256, "commitment blob"
    )
    require_equal(
        heldout.sha256_file(COMMITMENT), COMMITMENT_SHA256, "current commitment bytes"
    )
    require_equal(
        git_text(["rev-parse", f"{UNSEAL_COMMIT}^{{tree}}"]),
        UNSEAL_TREE,
        "unseal tree",
    )
    collector = git_bytes(
        ["show", f"{UNSEAL_COMMIT}:bench/labels/default_head_heldout.py"]
    )
    require_equal(
        hashlib.sha256(collector).hexdigest(), COLLECTOR_SHA256, "collector blob"
    )
    for revision in (UNSEAL_COMMIT, COMMITMENT_COMMIT):
        subprocess.run(
            ["git", "merge-base", "--is-ancestor", revision, "HEAD"],
            cwd=ROOT,
            check=True,
        )


def validate_payload(payload: dict[str, Any]) -> None:
    require_equal(
        payload_sha256(payload), COMMITMENT_SHA256, "commitment payload receipt"
    )
    heldout.validate_commitment(payload)
    provenance = payload["provenance"]
    require_equal(provenance["unseal_commit"], UNSEAL_COMMIT, "unseal commit")
    require_equal(provenance["unseal_tree"], UNSEAL_TREE, "unseal tree")
    require_equal(
        provenance["collector"],
        {
            "path": "bench/labels/default_head_heldout.py",
            "sha256": COLLECTOR_SHA256,
        },
        "collector receipt",
    )
    require_equal(
        payload["protocol"]["root_seed_commitment_sha256"],
        ROOT_SEED_COMMITMENT,
        "root seed commitment",
    )
    require_equal(payload["packets"], PACKET_RECEIPTS, "private packet receipts")
    require_equal(
        payload["selection"]["sealed_candidate_keys_sha256"],
        SEALED_SELECTION_SHA256,
        "sealed selection digest",
    )


def validate(_: argparse.Namespace) -> None:
    validate_git_receipt()
    validate_payload(heldout.read_json(COMMITMENT))
    print(f"held-out commitment receipt OK: {COMMITMENT}")


def self_test(_: argparse.Namespace) -> None:
    validate_git_receipt()
    payload = heldout.read_json(COMMITMENT)
    validate_payload(payload)
    mutations: list[dict[str, Any]] = []
    changed = copy.deepcopy(payload)
    changed["packets"][0]["sha256"] = "0" * 64
    mutations.append(changed)
    changed = copy.deepcopy(payload)
    changed["packets"].reverse()
    mutations.append(changed)
    changed = copy.deepcopy(payload)
    changed["protocol"]["root_seed_commitment_sha256"] = "0" * 64
    mutations.append(changed)
    changed = copy.deepcopy(payload)
    changed["provenance"]["unseal_commit"] = COMMITMENT_COMMIT
    changed["provenance"]["unseal_tree"] = COMMITMENT_TREE
    mutations.append(changed)
    for mutation in mutations:
        try:
            validate_payload(mutation)
        except ValueError:
            continue
        raise AssertionError("coordinated commitment mutation was accepted")
    print("default-head held-out commitment receipt self-test passed")


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
