#!/usr/bin/env python3
"""Build the private, pre-reveal #846 blind arbitration packet."""

from __future__ import annotations

import argparse
import copy
import hashlib
import json
from pathlib import Path
from typing import Any

import default_head_heldout as heldout
import default_head_heldout_commitment_receipt as receipt
import default_head_heldout_panel as panel
from labelset import validate_vote


ROOT = Path(__file__).resolve().parents[2]
COMMITMENT = (
    ROOT
    / "bench/labels/default_head_heldout_arbitration_commitment_2026_07_14.v3.json"
)
ARBITER_PACKET_SCHEMA = "nose.default_head_heldout_private_arbitration.v3"
ARBITRATION_COMMITMENT_SCHEMA = (
    "nose.default_head_heldout_arbitration_commitment.v3"
)
VOTE_PATHS = {
    persona: ROOT
    / f"bench/labels/default_head_heldout_votes_2026_07_14.heldout.{persona}.v3.json"
    for persona in heldout.PERSONAS
}
FREEZE_COMMAND = (
    "python3 bench/labels/default_head_heldout_arbitration.py freeze "
    "--private-panel-dir <outside-repository> "
    "--private-output <outside-repository>"
)
ARBITER_ATTESTATION = {
    "assigned_material_only": True,
    "no_git_or_corpus_lookup": True,
    "no_network_or_source_identity_lookup": True,
    "no_raw_vote_files_or_reviewer_contact": True,
}


def require_equal(actual: object, expected: object, label: str) -> None:
    if actual != expected:
        raise ValueError(f"{label}: mismatch")


def relative(path: Path) -> str:
    return path.resolve().relative_to(ROOT).as_posix()


def file_record(path: Path) -> dict[str, Any]:
    return {
        "path": relative(path),
        "sha256": heldout.sha256_file(path),
        "byte_length": path.stat().st_size,
    }


def frozen_file_record(path: Path, revision: str) -> dict[str, Any]:
    path_text = relative(path)
    frozen = heldout.git_bytes(["show", f"{revision}:{path_text}"])
    require_equal(frozen, path.read_bytes(), f"frozen file {path_text}")
    return {
        "path": path_text,
        "sha256": hashlib.sha256(frozen).hexdigest(),
        "byte_length": len(frozen),
    }


def load_panel_votes(
    private_dir: Path, commitment: dict[str, Any]
) -> tuple[dict[str, dict[str, Any]], list[dict[str, Any]]]:
    vote_payloads: dict[str, dict[str, Any]] = {}
    receipts: list[dict[str, Any]] = []
    for persona in heldout.PERSONAS:
        _, packet = panel.private_packet(private_dir, persona, commitment)
        path = VOTE_PATHS[persona]
        payload = heldout.read_json(path)
        panel.validate_vote_payload(payload, persona, packet, commitment)
        vote_payloads[persona] = payload
        receipts.append({"persona": persona, **file_record(path)})
    return vote_payloads, receipts


def persona_order(
    selected: list[dict[str, Any]], root_seed: bytes, persona: str
) -> list[dict[str, Any]]:
    seed = heldout.persona_seed(root_seed, persona)
    return sorted(
        selected,
        key=lambda candidate: heldout.hmac_hex(
            seed, "order", candidate["candidate_key"]
        ),
    )


def align_votes(
    selected: list[dict[str, Any]],
    root_seed: bytes,
    private_dir: Path,
    commitment: dict[str, Any],
    vote_payloads: dict[str, dict[str, Any]],
) -> dict[str, dict[str, dict[str, Any]]]:
    aligned: dict[str, dict[str, dict[str, Any]]] = {
        candidate["candidate_key"]: {} for candidate in selected
    }
    for persona in heldout.PERSONAS:
        _, actual_packet = panel.private_packet(private_dir, persona, commitment)
        expected_packet = heldout.private_packet(
            persona, selected, root_seed, heldout.RUBRIC
        )
        require_equal(actual_packet, expected_packet, f"{persona} packet seed replay")
        ordered = persona_order(selected, root_seed, persona)
        votes = vote_payloads[persona]["votes"]
        for candidate, visible, vote in zip(
            ordered, actual_packet["candidates"], votes, strict=True
        ):
            require_equal(
                vote["blind_id"], visible["blind_id"], f"{persona} aligned blind ID"
            )
            aligned[candidate["candidate_key"]][persona] = {
                "worthy": vote["worthy"],
                "reason": vote["reason"],
                "rationale": vote["rationale"].strip(),
            }
    return aligned


def disagreement_keys(
    selected: list[dict[str, Any]],
    aligned: dict[str, dict[str, dict[str, Any]]],
) -> list[str]:
    return [
        candidate["candidate_key"]
        for candidate in selected
        if len(
            {
                (vote["worthy"], vote["reason"])
                for vote in aligned[candidate["candidate_key"]].values()
            }
        )
        > 1
    ]


def anonymous_panel_votes(
    candidate_key: str,
    votes: dict[str, dict[str, Any]],
    arbiter_seed: bytes,
) -> list[dict[str, Any]]:
    ordered = sorted(
        heldout.PERSONAS,
        key=lambda persona: heldout.hmac_hex(
            arbiter_seed, "panel-order", f"{candidate_key}\0{persona}"
        ),
    )
    return [
        {"reviewer": f"reviewer-{index}", **votes[persona]}
        for index, persona in enumerate(ordered, start=1)
    ]


def arbiter_packet(
    selected: list[dict[str, Any]],
    aligned: dict[str, dict[str, dict[str, Any]]],
    root_seed: bytes,
) -> dict[str, Any]:
    by_key = {candidate["candidate_key"]: candidate for candidate in selected}
    keys = disagreement_keys(selected, aligned)
    seed = heldout.persona_seed(root_seed, "arbiter")
    keys.sort(key=lambda key: heldout.hmac_hex(seed, "order", key))
    cases = []
    for key in keys:
        visible = heldout.blind_candidate(by_key[key], seed)
        visible["panel_votes"] = anonymous_panel_votes(key, aligned[key], seed)
        cases.append(visible)
    return {
        "schema": ARBITER_PACKET_SCHEMA,
        "issue": 846,
        "split": "heldout",
        "persona": "arbiter",
        "judgment_status": "procedurally-blind-unjudged-disagreements",
        "packet_nonce": heldout.hmac_hex(seed, "packet-nonce", "arbiter"),
        "rubric_sha256": heldout.sha256_file(heldout.RUBRIC),
        "reviewer_protocol": {
            "guarantee": "procedural-product-metadata-blindness",
            "not_guaranteed": (
                "identity hiding from a reviewer who searches remembered or public source"
            ),
            "allowed_material": ["assigned arbitration packet", "bound rubric"],
            "prohibited_actions": [
                "inspect Git, the corpus, repositories, or unassigned files",
                "use network access or search for source identity",
                "read raw persona vote files or contact a reviewer",
            ],
            "required_vote_attestation": ARBITER_ATTESTATION,
        },
        "candidates": cases,
    }


def validate_arbiter_packet(payload: dict[str, Any]) -> None:
    require_equal(
        set(payload),
        {
            "schema",
            "issue",
            "split",
            "persona",
            "judgment_status",
            "packet_nonce",
            "rubric_sha256",
            "reviewer_protocol",
            "candidates",
        },
        "arbiter packet fields",
    )
    require_equal(payload["schema"], ARBITER_PACKET_SCHEMA, "arbiter schema")
    require_equal(payload["issue"], 846, "arbiter issue")
    require_equal(payload["split"], "heldout", "arbiter split")
    require_equal(payload["persona"], "arbiter", "arbiter persona")
    require_equal(
        payload["judgment_status"],
        "procedurally-blind-unjudged-disagreements",
        "arbiter judgment status",
    )
    heldout.require_hex(payload["packet_nonce"], 64, "arbiter packet nonce")
    require_equal(
        payload["rubric_sha256"], heldout.sha256_file(heldout.RUBRIC), "arbiter rubric"
    )
    protocol = payload["reviewer_protocol"]
    require_equal(
        protocol,
        {
            "guarantee": "procedural-product-metadata-blindness",
            "not_guaranteed": (
                "identity hiding from a reviewer who searches remembered or public source"
            ),
            "allowed_material": ["assigned arbitration packet", "bound rubric"],
            "prohibited_actions": [
                "inspect Git, the corpus, repositories, or unassigned files",
                "use network access or search for source identity",
                "read raw persona vote files or contact a reviewer",
            ],
            "required_vote_attestation": ARBITER_ATTESTATION,
        },
        "arbiter reviewer protocol",
    )
    candidates = payload["candidates"]
    if not isinstance(candidates, list) or not candidates:
        raise ValueError("arbiter packet must contain disagreements")
    ids: set[str] = set()
    for index, candidate in enumerate(candidates):
        if not isinstance(candidate, dict) or set(candidate) != {
            "blind_id",
            "language",
            "family",
            "panel_votes",
        }:
            raise ValueError(f"arbiter candidates[{index}]: fields mismatch")
        blind_id = candidate["blind_id"]
        if (
            not isinstance(blind_id, str)
            or not blind_id.startswith("case-")
            or len(blind_id) != 29
        ):
            raise ValueError(f"arbiter candidates[{index}].blind_id: invalid")
        heldout.require_hex(
            blind_id.removeprefix("case-"),
            24,
            f"arbiter candidates[{index}].blind_id",
        )
        ids.add(blind_id)
        family = candidate["family"]
        if not isinstance(family, dict) or set(family) != heldout.VISIBLE_FAMILY_KEYS:
            raise ValueError(f"arbiter candidates[{index}].family: fields mismatch")
        members = family["members"]
        if (
            not isinstance(members, list)
            or not members
            or family["member_count"] != len(members)
        ):
            raise ValueError(f"arbiter candidates[{index}].members: invalid")
        for member_index, member in enumerate(members):
            if not isinstance(member, dict) or set(member) != heldout.VISIBLE_MEMBER_KEYS:
                raise ValueError(
                    f"arbiter candidates[{index}].members[{member_index}]: fields mismatch"
                )
            source_id = member["source_id"]
            if not isinstance(source_id, str) or not source_id.startswith("source-"):
                raise ValueError(
                    f"arbiter candidates[{index}].members[{member_index}].source_id: invalid"
                )
            heldout.require_hex(
                source_id.removeprefix("source-"),
                24,
                f"arbiter candidates[{index}].members[{member_index}].source_id",
            )
            for field in ("context_before", "source", "context_after"):
                if not isinstance(member[field], str):
                    raise ValueError(
                        f"arbiter candidates[{index}].members[{member_index}].{field}: invalid"
                    )
        votes = candidate["panel_votes"]
        if not isinstance(votes, list) or len(votes) != 3:
            raise ValueError(f"arbiter candidates[{index}]: needs three panel votes")
        for vote_index, vote in enumerate(votes, start=1):
            if not isinstance(vote, dict) or set(vote) != {
                "reviewer",
                "worthy",
                "reason",
                "rationale",
            }:
                raise ValueError(
                    f"arbiter candidates[{index}].panel_votes: fields mismatch"
                )
            require_equal(
                vote["reviewer"],
                f"reviewer-{vote_index}",
                f"arbiter candidates[{index}] reviewer order",
            )
            validate_vote(vote, f"arbiter candidates[{index}].panel_votes[{vote_index}]")
    if len(ids) != len(candidates):
        raise ValueError("arbiter blind IDs must be unique")


def collect_commitment(
    args: argparse.Namespace, root_seed: bytes
) -> tuple[dict[str, Any], dict[str, Any]]:
    status = heldout.git_text(["status", "--short"])
    if status:
        raise ValueError("arbitration freeze requires a clean working tree")
    if len(root_seed) != 32:
        raise ValueError("blind root seed must contain exactly 32 bytes")
    panel_commitment = panel.read_commitment()
    require_equal(
        hashlib.sha256(root_seed).hexdigest(),
        panel_commitment["protocol"]["root_seed_commitment_sha256"],
        "root seed commitment",
    )
    vote_commit = heldout.git_text(["rev-parse", "HEAD"])
    vote_tree = heldout.git_text(["rev-parse", "HEAD^{tree}"])
    vote_payloads, _ = load_panel_votes(args.private_panel_dir, panel_commitment)
    vote_receipts = [
        {"persona": persona, **frozen_file_record(VOTE_PATHS[persona], vote_commit)}
        for persona in heldout.PERSONAS
    ]
    _, selected = heldout.replay(args)
    aligned = align_votes(
        selected,
        root_seed,
        args.private_panel_dir,
        panel_commitment,
        vote_payloads,
    )
    private = arbiter_packet(selected, aligned, root_seed)
    validate_arbiter_packet(private)
    content = heldout.packet_bytes(private)
    collector = Path(__file__).resolve()
    commitment = {
        "schema": ARBITRATION_COMMITMENT_SCHEMA,
        "issue": 846,
        "split": "heldout",
        "state": "blind-arbitration-packet-committed",
        "panel_commitment": {
            "path": relative(panel.COMMITMENT),
            "sha256": heldout.sha256_file(panel.COMMITMENT),
            "commit": receipt.COMMITMENT_COMMIT,
        },
        "raw_votes": {
            "commit": vote_commit,
            "tree": vote_tree,
            "count": len(vote_receipts),
            "files": vote_receipts,
        },
        "arbitration_packet": {
            "schema": ARBITER_PACKET_SCHEMA,
            "sha256": hashlib.sha256(content).hexdigest(),
            "byte_length": len(content),
            "candidate_count": len(private["candidates"]),
        },
        "protocol": {
            "root_seed_commitment_sha256": panel_commitment["protocol"][
                "root_seed_commitment_sha256"
            ],
            "raw_votes_frozen_before_packet": True,
            "mapping_release": "after blind-ID arbitration is frozen",
        },
        "provenance": {
            "command": FREEZE_COMMAND,
            "collector": file_record(collector),
            "vote_commit": vote_commit,
            "vote_tree": vote_tree,
            "working_tree_status_before_freeze": status,
        },
    }
    return commitment, private


def validate_commitment(payload: dict[str, Any]) -> None:
    require_equal(
        set(payload),
        {
            "schema",
            "issue",
            "split",
            "state",
            "panel_commitment",
            "raw_votes",
            "arbitration_packet",
            "protocol",
            "provenance",
        },
        "arbitration commitment fields",
    )
    require_equal(payload["schema"], ARBITRATION_COMMITMENT_SCHEMA, "schema")
    require_equal(payload["state"], "blind-arbitration-packet-committed", "state")
    require_equal(payload["issue"], 846, "issue")
    require_equal(payload["split"], "heldout", "split")
    require_equal(payload["raw_votes"]["count"], 3, "raw vote count")
    packet = payload["arbitration_packet"]
    require_equal(packet["schema"], ARBITER_PACKET_SCHEMA, "arbiter packet schema")
    heldout.require_hex(packet["sha256"], 64, "arbiter packet SHA")
    if not isinstance(packet["byte_length"], int) or packet["byte_length"] < 1:
        raise ValueError("arbiter packet byte length is invalid")
    if not isinstance(packet["candidate_count"], int) or packet["candidate_count"] < 1:
        raise ValueError("arbiter candidate count is invalid")
    require_equal(
        payload["protocol"]["mapping_release"],
        "after blind-ID arbitration is frozen",
        "mapping release",
    )
    require_equal(
        payload["protocol"]["raw_votes_frozen_before_packet"],
        True,
        "raw vote chronology",
    )
    provenance = payload["provenance"]
    require_equal(provenance["command"], FREEZE_COMMAND, "freeze command")
    require_equal(provenance["working_tree_status_before_freeze"], "", "clean freeze")
    require_equal(provenance["vote_commit"], payload["raw_votes"]["commit"], "vote commit")
    require_equal(provenance["vote_tree"], payload["raw_votes"]["tree"], "vote tree")


def freeze(args: argparse.Namespace) -> None:
    if args.output.resolve() != COMMITMENT.resolve():
        raise ValueError(f"commitment output must be {relative(COMMITMENT)}")
    heldout.require_private_directory(args.private_panel_dir, empty=False)
    private_parent = heldout.require_private_directory(
        args.private_output.parent, empty=False
    )
    private_output = private_parent / args.private_output.name
    if private_output.exists():
        raise ValueError(f"refusing to replace private arbitration packet: {private_output}")
    root_seed = heldout.read_root_seed()
    commitment, private = collect_commitment(args, root_seed)
    heldout.write_exclusive(private_output, heldout.packet_bytes(private), 0o600)
    heldout.write_exclusive(args.output, heldout.packet_bytes(commitment), 0o644)
    print(
        f"committed arbitration packet with {len(private['candidates'])} disagreements"
    )


def validate(args: argparse.Namespace) -> None:
    payload = heldout.read_json(args.commitment)
    validate_commitment(payload)
    print(f"validated {args.commitment}")


def self_test(_: argparse.Namespace) -> None:
    votes = {
        "dedupe": {"worthy": True, "reason": "extract-helper", "rationale": "a"},
        "pragmatic": {"worthy": False, "reason": "trivial", "rationale": "b"},
        "skeptic": {"worthy": False, "reason": "trivial", "rationale": "c"},
    }
    seed = b"x" * 32
    first = anonymous_panel_votes("candidate", votes, seed)
    second = anonymous_panel_votes("candidate", votes, seed)
    require_equal(first, second, "deterministic anonymous panel order")
    if {row["reviewer"] for row in first} != {"reviewer-1", "reviewer-2", "reviewer-3"}:
        raise AssertionError("anonymous reviewer labels are incomplete")
    changed = copy.deepcopy(votes)
    changed["skeptic"]["reason"] = "coincidental-shape"
    if first == anonymous_panel_votes("candidate", changed, seed):
        raise AssertionError("vote content mutation was ignored")
    print("default-head held-out arbitration self-test passed")


def add_live_arguments(parser: argparse.ArgumentParser) -> None:
    heldout.add_live_arguments(parser)


def parser() -> argparse.ArgumentParser:
    root = argparse.ArgumentParser()
    commands = root.add_subparsers(dest="command", required=True)
    freeze_parser = commands.add_parser("freeze", allow_abbrev=False)
    add_live_arguments(freeze_parser)
    freeze_parser.add_argument("--private-panel-dir", type=Path, required=True)
    freeze_parser.add_argument("--private-output", type=Path, required=True)
    freeze_parser.add_argument("--output", type=Path, default=COMMITMENT)
    freeze_parser.set_defaults(run=freeze)
    validate_parser = commands.add_parser("validate")
    validate_parser.add_argument(
        "commitment", type=Path, nargs="?", default=COMMITMENT
    )
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
