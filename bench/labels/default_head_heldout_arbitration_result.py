#!/usr/bin/env python3
"""Compile and validate the procedurally blind #846 arbitration result."""

from __future__ import annotations

import argparse
import copy
import json
from pathlib import Path
from typing import Any

import default_head_heldout as heldout
import default_head_heldout_arbitration as arbitration
import default_head_heldout_arbitration_receipt as arbitration_receipt
import default_head_heldout_panel as panel
from labelset import validate_vote


ROOT = Path(__file__).resolve().parents[2]
RESULT = (
    ROOT
    / "bench/labels/default_head_heldout_arbitration_result_2026_07_14.heldout.v3.json"
)
RESULT_SCHEMA = "nose.default_head_heldout_arbitration_result.v3"


def require_equal(actual: object, expected: object, label: str) -> None:
    panel.require_equal(actual, expected, label)


def read_commitment() -> dict[str, Any]:
    payload = heldout.read_json(arbitration.COMMITMENT)
    arbitration_receipt.validate_git_receipt()
    arbitration_receipt.validate_payload(payload)
    return payload


def private_packet(
    path: Path, commitment: dict[str, Any]
) -> tuple[Path, dict[str, Any]]:
    return arbitration.private_packet_receipt(path, commitment)


def template_attestation() -> dict[str, bool]:
    return {key: False for key in arbitration.ARBITER_ATTESTATION}


def read_attestation(path: Path) -> dict[str, bool]:
    payload = heldout.read_json(path)
    require_equal(payload, arbitration.ARBITER_ATTESTATION, "arbiter attestation")
    return payload


def build_result(
    packet: dict[str, Any],
    commitment: dict[str, Any],
    votes: list[dict[str, Any]],
    attestation: dict[str, bool],
) -> dict[str, Any]:
    return {
        "schema": RESULT_SCHEMA,
        "issue": 846,
        "split": "heldout",
        "persona": "arbiter",
        "state": "blind-arbitration-judged",
        "source_packet": commitment["arbitration_packet"],
        "rubric": panel.read_commitment()["rubric"],
        "attestation": attestation,
        "votes": votes,
    }


def validate_public_result_payload(
    payload: dict[str, Any], commitment: dict[str, Any]
) -> None:
    require_equal(
        set(payload),
        {
            "schema",
            "issue",
            "split",
            "persona",
            "state",
            "source_packet",
            "rubric",
            "attestation",
            "votes",
        },
        "arbitration result fields",
    )
    require_equal(payload["schema"], RESULT_SCHEMA, "arbitration result schema")
    require_equal(payload["issue"], 846, "arbitration result issue")
    require_equal(payload["split"], "heldout", "arbitration result split")
    require_equal(payload["persona"], "arbiter", "arbitration result persona")
    require_equal(
        payload["state"], "blind-arbitration-judged", "arbitration result state"
    )
    require_equal(
        payload["source_packet"],
        commitment["arbitration_packet"],
        "arbitration source packet",
    )
    require_equal(
        payload["rubric"], panel.read_commitment()["rubric"], "arbitration rubric"
    )
    require_equal(
        payload["attestation"],
        arbitration.ARBITER_ATTESTATION,
        "arbiter attestation",
    )
    votes = payload["votes"]
    expected_count = commitment["arbitration_packet"]["candidate_count"]
    if not isinstance(votes, list) or len(votes) != expected_count:
        raise ValueError("arbitration result vote count mismatch")
    blind_ids: set[str] = set()
    for index, vote in enumerate(votes):
        if not isinstance(vote, dict) or set(vote) != set(panel.TSV_FIELDS):
            raise ValueError(f"arbitration votes[{index}]: fields mismatch")
        blind_id = vote["blind_id"]
        if (
            not isinstance(blind_id, str)
            or not blind_id.startswith("case-")
            or len(blind_id) != 29
        ):
            raise ValueError(f"arbitration votes[{index}].blind_id: invalid")
        heldout.require_hex(
            blind_id.removeprefix("case-"),
            24,
            f"arbitration votes[{index}].blind_id",
        )
        blind_ids.add(blind_id)
        validate_vote(vote, f"arbitration votes[{index}]")
        if not isinstance(vote["rationale"], str) or not vote["rationale"].strip():
            raise ValueError(f"arbitration votes[{index}].rationale: required")
    if len(blind_ids) != expected_count:
        raise ValueError("arbitration result blind IDs must be unique")


def validate_result_payload(
    payload: dict[str, Any],
    packet: dict[str, Any],
    commitment: dict[str, Any],
) -> None:
    validate_public_result_payload(payload, commitment)
    expected_ids = [candidate["blind_id"] for candidate in packet["candidates"]]
    for index, (vote, expected_id) in enumerate(
        zip(payload["votes"], expected_ids, strict=True)
    ):
        require_equal(
            vote["blind_id"], expected_id, f"arbitration votes[{index}].blind_id"
        )


def write_template(args: argparse.Namespace) -> None:
    commitment = read_commitment()
    _, packet = private_packet(args.private_packet, commitment)
    output_dir = args.output_dir.resolve()
    if not output_dir.is_dir() or any(output_dir.iterdir()):
        raise ValueError("arbiter output directory must exist and be empty")
    panel.write_text_exclusive(output_dir / "votes.tsv", panel.template_tsv(packet))
    panel.write_text_exclusive(
        output_dir / "attestation.json",
        json.dumps(template_attestation(), indent=2, sort_keys=True) + "\n",
    )
    print(f"wrote {len(packet['candidates'])} arbiter vote rows to {output_dir}")


def compile_result(args: argparse.Namespace) -> None:
    commitment = read_commitment()
    _, packet = private_packet(args.private_packet, commitment)
    expected_ids = [candidate["blind_id"] for candidate in packet["candidates"]]
    try:
        tsv = args.tsv.read_text(encoding="utf-8")
    except OSError as error:
        raise ValueError(f"cannot read arbiter vote TSV {args.tsv}: {error}") from error
    votes = panel.parse_tsv_text(tsv, expected_ids)
    attestation = read_attestation(args.attestation)
    payload = build_result(packet, commitment, votes, attestation)
    validate_result_payload(payload, packet, commitment)
    panel.write_text_exclusive(
        args.output,
        json.dumps(payload, sort_keys=True, separators=(",", ":")) + "\n",
    )
    print(f"compiled {len(votes)} arbitration decisions to {args.output}")


def validate_result_file(args: argparse.Namespace) -> None:
    commitment = read_commitment()
    _, packet = private_packet(args.private_packet, commitment)
    payload = heldout.read_json(args.result)
    validate_result_payload(payload, packet, commitment)
    print(f"validated {len(payload['votes'])} arbitration decisions: {args.result}")


def validate_public_result_file(args: argparse.Namespace) -> None:
    commitment = read_commitment()
    payload = heldout.read_json(args.result)
    validate_public_result_payload(payload, commitment)
    print(
        f"publicly validated {len(payload['votes'])} arbitration decisions: "
        f"{args.result}"
    )


def self_test(_: argparse.Namespace) -> None:
    frozen_commitment = read_commitment()
    changed_commitment = copy.deepcopy(frozen_commitment)
    changed_commitment["arbitration_packet"]["candidate_count"] = 1
    try:
        arbitration_receipt.validate_payload(changed_commitment)
    except ValueError:
        pass
    else:
        raise AssertionError("altered arbitration commitment receipt was accepted")
    expected_ids = ["case-" + "1" * 24, "case-" + "2" * 24]
    packet = {"candidates": [{"blind_id": value} for value in expected_ids]}
    source_packet = {
        "schema": arbitration.ARBITER_PACKET_SCHEMA,
        "sha256": "2" * 64,
        "byte_length": 100,
        "candidate_count": 2,
    }
    commitment = {"arbitration_packet": source_packet}
    votes = [
        {
            "blind_id": expected_ids[0],
            "worthy": True,
            "reason": "extract-helper",
            "rationale": "Shared operation.",
        },
        {
            "blind_id": expected_ids[1],
            "worthy": False,
            "reason": "trivial",
            "rationale": "Too small.",
        },
    ]
    rubric = panel.read_commitment()["rubric"]
    payload = {
        "schema": RESULT_SCHEMA,
        "issue": 846,
        "split": "heldout",
        "persona": "arbiter",
        "state": "blind-arbitration-judged",
        "source_packet": source_packet,
        "rubric": rubric,
        "attestation": arbitration.ARBITER_ATTESTATION,
        "votes": votes,
    }
    validate_result_payload(payload, packet, commitment)
    public_mutations: list[dict[str, Any]] = []
    changed = copy.deepcopy(payload)
    changed["issue"] = 846.0
    public_mutations.append(changed)
    changed = copy.deepcopy(payload)
    changed["attestation"] = {
        key: 1 for key in arbitration.ARBITER_ATTESTATION
    }
    public_mutations.append(changed)
    changed = copy.deepcopy(payload)
    changed["source_packet"]["sha256"] = "0" * 64
    public_mutations.append(changed)
    changed = copy.deepcopy(payload)
    changed["votes"][1]["blind_id"] = changed["votes"][0]["blind_id"]
    public_mutations.append(changed)
    changed = copy.deepcopy(payload)
    changed["votes"][0]["reason"] = "trivial"
    public_mutations.append(changed)
    changed = copy.deepcopy(payload)
    changed["votes"][0]["rationale"] = " "
    public_mutations.append(changed)
    changed = copy.deepcopy(payload)
    changed["votes"].pop()
    public_mutations.append(changed)
    changed = copy.deepcopy(payload)
    changed["unexpected"] = True
    public_mutations.append(changed)
    for mutation in public_mutations:
        try:
            validate_public_result_payload(mutation, commitment)
        except ValueError:
            continue
        raise AssertionError("invalid public arbitration result mutation was accepted")
    changed = copy.deepcopy(payload)
    changed["votes"].reverse()
    validate_public_result_payload(changed, commitment)
    try:
        validate_result_payload(changed, packet, commitment)
    except ValueError:
        pass
    else:
        raise AssertionError("private arbitration result order mutation was accepted")
    print("default-head held-out arbitration result self-test passed")


def parser() -> argparse.ArgumentParser:
    root = argparse.ArgumentParser()
    commands = root.add_subparsers(dest="command", required=True)
    template = commands.add_parser("write-template", allow_abbrev=False)
    template.add_argument("--private-packet", type=Path, required=True)
    template.add_argument("--output-dir", type=Path, required=True)
    template.set_defaults(run=write_template)
    compile_parser = commands.add_parser("compile-result", allow_abbrev=False)
    compile_parser.add_argument("--private-packet", type=Path, required=True)
    compile_parser.add_argument("--tsv", type=Path, required=True)
    compile_parser.add_argument("--attestation", type=Path, required=True)
    compile_parser.add_argument("--output", type=Path, required=True)
    compile_parser.set_defaults(run=compile_result)
    validate_parser = commands.add_parser("validate-result", allow_abbrev=False)
    validate_parser.add_argument("--private-packet", type=Path, required=True)
    validate_parser.add_argument("result", type=Path)
    validate_parser.set_defaults(run=validate_result_file)
    public_parser = commands.add_parser("validate-public", allow_abbrev=False)
    public_parser.add_argument("result", type=Path)
    public_parser.set_defaults(run=validate_public_result_file)
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
