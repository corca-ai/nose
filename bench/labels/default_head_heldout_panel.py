#!/usr/bin/env python3
"""Compile and validate procedurally blind #846 panel votes."""

from __future__ import annotations

import argparse
import copy
import csv
import io
import json
import tempfile
from pathlib import Path
from typing import Any

import default_head_heldout as heldout
import default_head_heldout_commitment_receipt as receipt
from labelset import validate_vote


ROOT = Path(__file__).resolve().parents[2]
COMMITMENT = heldout.COMMITMENT
VOTE_SCHEMA = "nose.default_head_heldout_panel_vote.v3"
TSV_FIELDS = ("blind_id", "worthy", "reason", "rationale")


def require_equal(actual: object, expected: object, label: str) -> None:
    if actual != expected:
        raise ValueError(f"{label}: mismatch")


def read_commitment() -> dict[str, Any]:
    payload = heldout.read_json(COMMITMENT)
    receipt.validate_git_receipt()
    receipt.validate_payload(payload)
    return payload


def packet_record(commitment: dict[str, Any], persona: str) -> dict[str, Any]:
    for record in commitment["packets"]:
        if record["persona"] == persona:
            return record
    raise ValueError(f"missing {persona} packet receipt")


def private_packet(
    private_dir: Path, persona: str, commitment: dict[str, Any]
) -> tuple[Path, dict[str, Any]]:
    directory = heldout.require_private_directory(private_dir, empty=False)
    path = directory / f"{persona}.json"
    payload = heldout.read_json(path)
    heldout.validate_private_packet(payload, persona)
    record = packet_record(commitment, persona)
    require_equal(heldout.sha256_file(path), record["sha256"], f"{persona} packet SHA")
    require_equal(path.stat().st_size, record["byte_length"], f"{persona} packet bytes")
    return path, payload


def write_text_exclusive(path: Path, value: str) -> None:
    try:
        with path.open("x", encoding="utf-8", newline="") as output:
            output.write(value)
    except FileExistsError as error:
        raise ValueError(f"refusing to replace existing file: {path}") from error


def template_tsv(packet: dict[str, Any]) -> str:
    output = io.StringIO(newline="")
    writer = csv.writer(output, delimiter="\t", lineterminator="\n")
    writer.writerow(TSV_FIELDS)
    for candidate in packet["candidates"]:
        writer.writerow([candidate["blind_id"], "", "", ""])
    return output.getvalue()


def template_attestation() -> dict[str, bool]:
    return {key: False for key in heldout.REVIEWER_ATTESTATION}


def write_template(args: argparse.Namespace) -> None:
    commitment = read_commitment()
    _, packet = private_packet(args.private_dir, args.persona, commitment)
    output_dir = args.output_dir.resolve()
    if not output_dir.is_dir() or any(output_dir.iterdir()):
        raise ValueError("vote output directory must exist and be empty")
    write_text_exclusive(output_dir / "votes.tsv", template_tsv(packet))
    write_text_exclusive(
        output_dir / "attestation.json",
        json.dumps(template_attestation(), indent=2, sort_keys=True) + "\n",
    )
    print(f"wrote {args.persona} vote template to {output_dir}")


def parse_tsv_text(value: str, expected_ids: list[str]) -> list[dict[str, Any]]:
    rows = list(csv.reader(io.StringIO(value), delimiter="\t"))
    if not rows or tuple(rows[0]) != TSV_FIELDS:
        raise ValueError("vote TSV header mismatch")
    if len(rows) != len(expected_ids) + 1:
        raise ValueError("vote TSV row count mismatch")
    votes: list[dict[str, Any]] = []
    for index, (row, expected_id) in enumerate(
        zip(rows[1:], expected_ids, strict=True), start=1
    ):
        if len(row) != len(TSV_FIELDS):
            raise ValueError(f"vote TSV row {index}: expected four columns")
        blind_id, worthy_text, reason, rationale = row
        require_equal(blind_id, expected_id, f"vote TSV row {index} blind ID")
        if worthy_text not in {"true", "false"}:
            raise ValueError(f"vote TSV row {index}: worthy must be true or false")
        vote = {
            "blind_id": blind_id,
            "worthy": worthy_text == "true",
            "reason": reason,
            "rationale": rationale.strip(),
        }
        validate_vote(vote, f"vote TSV row {index}")
        if not vote["rationale"]:
            raise ValueError(f"vote TSV row {index}: rationale is required")
        votes.append(vote)
    return votes


def read_attestation(path: Path) -> dict[str, bool]:
    payload = heldout.read_json(path)
    require_equal(payload, heldout.REVIEWER_ATTESTATION, "reviewer attestation")
    return payload


def build_vote(
    persona: str,
    packet: dict[str, Any],
    commitment: dict[str, Any],
    votes: list[dict[str, Any]],
    attestation: dict[str, bool],
) -> dict[str, Any]:
    return {
        "schema": VOTE_SCHEMA,
        "issue": 846,
        "split": "heldout",
        "persona": persona,
        "source_packet": packet_record(commitment, persona),
        "rubric": commitment["rubric"],
        "attestation": attestation,
        "votes": votes,
    }


def validate_vote_payload(
    payload: dict[str, Any],
    persona: str,
    packet: dict[str, Any],
    commitment: dict[str, Any],
) -> None:
    require_equal(
        set(payload),
        {
            "schema",
            "issue",
            "split",
            "persona",
            "source_packet",
            "rubric",
            "attestation",
            "votes",
        },
        f"{persona} vote fields",
    )
    require_equal(payload["schema"], VOTE_SCHEMA, f"{persona} vote schema")
    require_equal(payload["issue"], 846, f"{persona} vote issue")
    require_equal(payload["split"], "heldout", f"{persona} vote split")
    require_equal(payload["persona"], persona, f"{persona} vote persona")
    require_equal(
        payload["source_packet"], packet_record(commitment, persona), "source packet"
    )
    require_equal(payload["rubric"], commitment["rubric"], "rubric")
    require_equal(
        payload["attestation"], heldout.REVIEWER_ATTESTATION, "reviewer attestation"
    )
    votes = payload["votes"]
    expected_ids = [candidate["blind_id"] for candidate in packet["candidates"]]
    if not isinstance(votes, list) or len(votes) != len(expected_ids):
        raise ValueError(f"{persona}: vote count mismatch")
    for index, (vote, expected_id) in enumerate(
        zip(votes, expected_ids, strict=True)
    ):
        if not isinstance(vote, dict) or set(vote) != set(TSV_FIELDS):
            raise ValueError(f"{persona}.votes[{index}]: fields mismatch")
        require_equal(
            vote["blind_id"], expected_id, f"{persona}.votes[{index}].blind_id"
        )
        validate_vote(vote, f"{persona}.votes[{index}]")
        rationale = vote["rationale"]
        if not isinstance(rationale, str) or not rationale.strip():
            raise ValueError(f"{persona}.votes[{index}].rationale: required")


def compile_vote(args: argparse.Namespace) -> None:
    commitment = read_commitment()
    _, packet = private_packet(args.private_dir, args.persona, commitment)
    expected_ids = [candidate["blind_id"] for candidate in packet["candidates"]]
    try:
        tsv = args.tsv.read_text(encoding="utf-8")
    except OSError as error:
        raise ValueError(f"cannot read vote TSV {args.tsv}: {error}") from error
    votes = parse_tsv_text(tsv, expected_ids)
    attestation = read_attestation(args.attestation)
    payload = build_vote(args.persona, packet, commitment, votes, attestation)
    validate_vote_payload(payload, args.persona, packet, commitment)
    write_text_exclusive(
        args.output,
        json.dumps(payload, sort_keys=True, separators=(",", ":")) + "\n",
    )
    print(f"compiled {len(votes)} {args.persona} votes to {args.output}")


def validate_vote_file(args: argparse.Namespace) -> None:
    commitment = read_commitment()
    _, packet = private_packet(args.private_dir, args.persona, commitment)
    payload = heldout.read_json(args.vote)
    validate_vote_payload(payload, args.persona, packet, commitment)
    print(f"validated {len(payload['votes'])} {args.persona} votes: {args.vote}")


def self_test(_: argparse.Namespace) -> None:
    expected_ids = ["case-" + "1" * 24, "case-" + "2" * 24]
    valid = (
        "blind_id\tworthy\treason\trationale\n"
        f"{expected_ids[0]}\ttrue\textract-helper\tOne shared operation.\n"
        f"{expected_ids[1]}\tfalse\ttrivial\tToo small to extract.\n"
    )
    votes = parse_tsv_text(valid, expected_ids)
    require_equal(len(votes), 2, "self-test vote count")
    mutations = [
        valid.replace("true", "yes", 1),
        valid.replace("extract-helper", "trivial", 1),
        valid.replace(expected_ids[0], expected_ids[1], 1),
        valid.replace("One shared operation.", "", 1),
        valid + f"{expected_ids[1]}\tfalse\ttrivial\textra\n",
    ]
    for mutation in mutations:
        try:
            parse_tsv_text(mutation, expected_ids)
        except ValueError:
            continue
        raise AssertionError("invalid held-out vote TSV mutation was accepted")
    attestation = template_attestation()
    with tempfile.TemporaryDirectory() as directory:
        path = Path(directory) / "attestation.json"
        path.write_text(json.dumps(attestation), encoding="utf-8")
        try:
            read_attestation(path)
        except ValueError:
            pass
        else:
            raise AssertionError("false reviewer attestation was accepted")
        approved = copy.deepcopy(attestation)
        approved.update(heldout.REVIEWER_ATTESTATION)
        path.write_text(json.dumps(approved), encoding="utf-8")
        read_attestation(path)
    print("default-head held-out panel self-test passed")


def parser() -> argparse.ArgumentParser:
    root = argparse.ArgumentParser()
    commands = root.add_subparsers(dest="command", required=True)
    template = commands.add_parser("write-template", allow_abbrev=False)
    template.add_argument("--private-dir", type=Path, required=True)
    template.add_argument("--persona", choices=heldout.PERSONAS, required=True)
    template.add_argument("--output-dir", type=Path, required=True)
    template.set_defaults(run=write_template)
    compile_parser = commands.add_parser("compile-vote", allow_abbrev=False)
    compile_parser.add_argument("--private-dir", type=Path, required=True)
    compile_parser.add_argument("--persona", choices=heldout.PERSONAS, required=True)
    compile_parser.add_argument("--tsv", type=Path, required=True)
    compile_parser.add_argument("--attestation", type=Path, required=True)
    compile_parser.add_argument("--output", type=Path, required=True)
    compile_parser.set_defaults(run=compile_vote)
    validate_parser = commands.add_parser("validate-vote", allow_abbrev=False)
    validate_parser.add_argument("--private-dir", type=Path, required=True)
    validate_parser.add_argument("--persona", choices=heldout.PERSONAS, required=True)
    validate_parser.add_argument("vote", type=Path)
    validate_parser.set_defaults(run=validate_vote_file)
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
