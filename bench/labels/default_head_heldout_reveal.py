#!/usr/bin/env python3
"""Reveal and validate the frozen #846 held-out judgment chain."""

from __future__ import annotations

import argparse
import copy
import hashlib
import json
import os
import subprocess
import tempfile
from collections.abc import Callable
from pathlib import Path
from typing import Any

import default_head_heldout as heldout
import default_head_heldout_arbitration as arbitration
import default_head_heldout_arbitration_receipt as arbitration_receipt
import default_head_heldout_arbitration_result as arbitration_result
import default_head_heldout_arbitration_result_receipt as result_receipt
import default_head_heldout_panel as panel
import default_head_heldout_vote_receipt as vote_receipt
from labelset import PRECISION_METRIC, validate_component_family, validate_vote


ROOT = Path(__file__).resolve().parents[2]
LABELS = ROOT / "bench/labels"
REVEAL = LABELS / "default_head_heldout_reveal_2026_07_14.heldout.v3.json"
DECISIONS = LABELS / "default_head_label_decisions_2026_07_14.heldout.v3.json"
COMPONENT = LABELS / "refactoring_families.v7.heldout.json"
PANEL_PACKET_PATHS = {
    persona: LABELS
    / f"default_head_heldout_packet_reveal_2026_07_14.heldout.{persona}.v3.json"
    for persona in heldout.PERSONAS
}
ARBITER_PACKET = (
    LABELS
    / "default_head_heldout_arbitration_packet_reveal_2026_07_14.heldout.v3.json"
)
REVEAL_SCHEMA = "nose.default_head_heldout_reveal.v3"
DECISIONS_SCHEMA = "nose.refactoring_label_decisions.v1"
COMPONENT_SCHEMA = "nose.refactoring_family_labels.v1"
PERSONA_ORDER = ("pragmatic", "dedupe", "skeptic")
FREEZE_COMMAND = (
    "python3 bench/labels/default_head_heldout_reveal.py freeze "
    "--private-panel-dir <outside-repository> "
    "--private-arbiter-packet <outside-repository>"
)
TRANSACTION = LABELS / ".default_head_heldout_reveal.transaction.json"
REVEALED_CANDIDATE_KEYS = {
    "candidate_key",
    "repo",
    "split",
    "language",
    "lane",
    "rank",
    "base_matched",
    "family",
    "raw_family_sha256",
    "candidate_sha256",
    "selected",
    "selection_reason",
    "selection_order",
}
REVEALED_FAMILY_KEYS = {
    "id",
    "members",
    "member_count",
    "scope",
    "surface",
    "witness",
    "extraction_shape",
    "value",
    "matched_v6_family_id",
    "matched_v6_member_overlap",
}
REVEALED_MEMBER_KEYS = {"file", "start_line", "end_line", "name"}


def require_equal(actual: object, expected: object, label: str) -> None:
    panel.require_equal(actual, expected, label)


def file_record(path: Path) -> dict[str, Any]:
    return {
        "path": path.relative_to(LABELS).as_posix(),
        "sha256": heldout.sha256_file(path),
        "byte_length": path.stat().st_size,
    }


def evidence_record(path: Path, content: bytes | None = None) -> dict[str, str]:
    digest = (
        heldout.sha256_file(path)
        if content is None
        else hashlib.sha256(content).hexdigest()
    )
    return {"path": path.name, "sha256": digest}


def require_ancestor(ancestor: str, descendant: str, label: str) -> None:
    completed = subprocess.run(
        ["git", "merge-base", "--is-ancestor", ancestor, descendant],
        cwd=ROOT,
        check=False,
        capture_output=True,
    )
    if completed.returncode != 0:
        raise ValueError(f"{label}: mismatch")


def validate_revealed_candidate(candidate: object, label: str) -> dict[str, Any]:
    row = heldout.require_exact_keys(candidate, REVEALED_CANDIDATE_KEYS, label)
    family = heldout.require_exact_keys(
        row["family"], REVEALED_FAMILY_KEYS, f"{label}.family"
    )
    members = family["members"]
    if not isinstance(members, list) or len(members) < 2:
        raise ValueError(f"{label}.family.members: expected at least two members")
    if (
        isinstance(family["member_count"], bool)
        or not isinstance(family["member_count"], int)
        or family["member_count"] != len(members)
    ):
        raise ValueError(f"{label}.family.member_count: mismatch")
    for index, member in enumerate(members):
        member_label = f"{label}.family.members[{index}]"
        member_row = heldout.require_exact_keys(
            member, REVEALED_MEMBER_KEYS, member_label
        )
        if not isinstance(member_row["file"], str) or not member_row["file"]:
            raise ValueError(f"{member_label}.file: invalid")
        for field in ("start_line", "end_line"):
            value = member_row[field]
            if isinstance(value, bool) or not isinstance(value, int):
                raise ValueError(f"{member_label}.{field}: invalid")
        if (
            member_row["start_line"] < 1
            or member_row["end_line"] < member_row["start_line"]
        ):
            raise ValueError(f"{member_label}: invalid line interval")
        if member_row["name"] is not None and not isinstance(member_row["name"], str):
            raise ValueError(f"{member_label}.name: invalid")
    return row


def ordered_keys(
    candidates: list[dict[str, Any]], seed: bytes, domain: str = "order"
) -> list[str]:
    return [
        row["candidate_key"]
        for row in sorted(
            candidates,
            key=lambda row: heldout.hmac_hex(seed, domain, row["candidate_key"]),
        )
    ]


def blind_id(seed: bytes, candidate_key: str) -> str:
    return f"case-{heldout.hmac_hex(seed, 'blind-id', candidate_key)[:24]}"


def read_frozen_votes() -> dict[str, dict[str, Any]]:
    payloads = vote_receipt.validate_git_receipt()
    vote_receipt.validate_vote_set(payloads)
    return payloads


def align_panel_votes(
    candidates: list[dict[str, Any]],
    root_seed: bytes,
    packets: dict[str, dict[str, Any]],
    vote_payloads: dict[str, dict[str, Any]],
) -> dict[str, dict[str, dict[str, Any]]]:
    aligned = {row["candidate_key"]: {} for row in candidates}
    by_key = {row["candidate_key"]: row for row in candidates}
    revealed_sources: dict[str, list[dict[str, str]]] = {}
    for persona in heldout.PERSONAS:
        seed = heldout.persona_seed(root_seed, persona)
        keys = ordered_keys(candidates, seed)
        visible_rows = packets[persona]["candidates"]
        votes = vote_payloads[persona]["votes"]
        require_equal(
            packets[persona]["packet_nonce"],
            heldout.hmac_hex(seed, "packet-nonce", persona),
            f"{persona} packet nonce",
        )
        if len(keys) != len(visible_rows) or len(keys) != len(votes):
            raise ValueError(f"{persona}: revealed packet/vote count mismatch")
        for index, (key, visible, vote) in enumerate(
            zip(keys, visible_rows, votes, strict=True)
        ):
            require_equal(visible["blind_id"], blind_id(seed, key), f"{persona}[{index}] ID")
            require_equal(vote["blind_id"], visible["blind_id"], f"{persona}[{index}] vote ID")
            candidate = by_key[key]
            require_equal(visible["language"], candidate["language"], f"{persona}[{index}] language")
            family = visible["family"]
            members = candidate["family"]["members"]
            require_equal(family["member_count"], len(members), f"{persona}[{index}] members")
            require_equal(len(family["members"]), len(members), f"{persona}[{index}] visible members")
            current_sources = []
            for member_index, (source, member) in enumerate(
                zip(family["members"], members, strict=True)
            ):
                expected_source_id = (
                    f"source-{heldout.hmac_hex(seed, 'source', member['file'])[:24]}"
                )
                require_equal(
                    source["source_id"],
                    expected_source_id,
                    f"{persona}[{index}].members[{member_index}] source ID",
                )
                current_sources.append(
                    {
                        field: source[field]
                        for field in ("context_before", "source", "context_after")
                    }
                )
            if key in revealed_sources:
                require_equal(
                    current_sources,
                    revealed_sources[key],
                    f"{persona}[{index}] source contents",
                )
            else:
                revealed_sources[key] = current_sources
            aligned[key][persona] = {
                "worthy": vote["worthy"],
                "reason": vote["reason"],
                "rationale": vote["rationale"].strip(),
            }
    return aligned


def align_arbiter_result(
    candidates: list[dict[str, Any]],
    aligned: dict[str, dict[str, dict[str, Any]]],
    root_seed: bytes,
    packet: dict[str, Any],
    result_payload: dict[str, Any],
    panel_packets: dict[str, dict[str, Any]],
) -> dict[str, dict[str, Any]]:
    keys = arbitration.disagreement_keys(candidates, aligned)
    seed = heldout.persona_seed(root_seed, "arbiter")
    require_equal(
        packet["packet_nonce"],
        heldout.hmac_hex(seed, "packet-nonce", "arbiter"),
        "arbiter packet nonce",
    )
    keys.sort(key=lambda key: heldout.hmac_hex(seed, "order", key))
    by_key = {row["candidate_key"]: row for row in candidates}
    dedupe_seed = heldout.persona_seed(root_seed, "dedupe")
    dedupe_keys = ordered_keys(candidates, dedupe_seed)
    dedupe_visible = {
        key: visible
        for key, visible in zip(
            dedupe_keys, panel_packets["dedupe"]["candidates"], strict=True
        )
    }
    packet_rows = packet["candidates"]
    result_rows = result_payload["votes"]
    if len(keys) != len(packet_rows) or len(keys) != len(result_rows):
        raise ValueError("arbiter packet/result count mismatch")
    resolutions: dict[str, dict[str, Any]] = {}
    for index, (key, visible, vote) in enumerate(
        zip(keys, packet_rows, result_rows, strict=True)
    ):
        expected_id = blind_id(seed, key)
        require_equal(visible["blind_id"], expected_id, f"arbiter[{index}] ID")
        require_equal(vote["blind_id"], expected_id, f"arbiter[{index}] result ID")
        candidate = by_key[key]
        require_equal(visible["language"], candidate["language"], f"arbiter[{index}] language")
        expected_votes = arbitration.anonymous_panel_votes(key, aligned[key], seed)
        require_equal(visible["panel_votes"], expected_votes, f"arbiter[{index}] panel votes")
        members = candidate["family"]["members"]
        require_equal(visible["family"]["member_count"], len(members), f"arbiter[{index}] members")
        base_members = dedupe_visible[key]["family"]["members"]
        for member_index, (source, base_source, member) in enumerate(
            zip(visible["family"]["members"], base_members, members, strict=True)
        ):
            require_equal(
                source["source_id"],
                f"source-{heldout.hmac_hex(seed, 'source', member['file'])[:24]}",
                f"arbiter[{index}].members[{member_index}] source ID",
            )
            for field in ("context_before", "source", "context_after"):
                require_equal(
                    source[field],
                    base_source[field],
                    f"arbiter[{index}].members[{member_index}].{field}",
                )
        resolutions[key] = {
            "worthy": vote["worthy"],
            "reason": vote["reason"],
            "rationale": vote["rationale"].strip(),
        }
    return resolutions


def final_decisions(
    candidates: list[dict[str, Any]],
    aligned: dict[str, dict[str, dict[str, Any]]],
    resolutions: dict[str, dict[str, Any]],
) -> list[dict[str, Any]]:
    decisions = []
    for candidate in candidates:
        key = candidate["candidate_key"]
        votes = aligned[key]
        pairs = {(vote["worthy"], vote["reason"]) for vote in votes.values()}
        if len(pairs) == 1:
            panel_vote = votes[PERSONA_ORDER[0]]
            arbiter = None
            confidence = "high"
            note = f"All three personas independently selected {panel_vote['reason']}."
        else:
            if key not in resolutions:
                raise ValueError(f"missing arbitration for {key}")
            arbiter = resolutions[key]
            confidence = "medium"
            note = arbiter["rationale"]
        decisions.append(
            {
                "candidate_key": key,
                "votes": {persona: votes[persona] for persona in PERSONA_ORDER},
                "arbiter": arbiter,
                "confidence": confidence,
                "note": note,
            }
        )
    disagreement_keys = {
        row["candidate_key"]
        for row in candidates
        if len(
            {
                (vote["worthy"], vote["reason"])
                for vote in aligned[row["candidate_key"]].values()
            }
        )
        > 1
    }
    require_equal(set(resolutions), disagreement_keys, "arbitration coverage")
    return decisions


def decisions_payload(
    reveal_path: Path,
    decisions: list[dict[str, Any]],
    reveal_content: bytes | None = None,
) -> dict[str, Any]:
    return {
        "schema": DECISIONS_SCHEMA,
        "split": "heldout",
        "source_artifact": evidence_record(reveal_path, reveal_content),
        "vote_inputs": {
            record["persona"]: {
                "path": Path(record["path"]).name,
                "sha256": record["sha256"],
            }
            for record in vote_receipt.VOTE_RECEIPTS
        },
        "arbitration_input": evidence_record(arbitration_result.RESULT),
        "decisions": decisions,
    }


def component_payload(
    reveal_path: Path,
    candidates: list[dict[str, Any]],
    decisions_path: Path,
    decisions: list[dict[str, Any]],
    reveal_content: bytes | None = None,
    decisions_content: bytes | None = None,
) -> dict[str, Any]:
    by_key = {row["candidate_key"]: row for row in decisions}
    families = []
    for candidate in candidates:
        key = candidate["candidate_key"]
        decision = by_key[key]
        votes = decision["votes"]
        pairs = [
            validate_vote(votes[persona], f"{key}.votes.{persona}")
            for persona in PERSONA_ORDER
        ]
        if len(set(pairs)) == 1:
            worthy, reason = pairs[0]
            labeler = "panel"
        else:
            worthy, reason = validate_vote(decision["arbiter"], f"{key}.arbiter")
            labeler = "llm-arbiter"
        family = candidate["family"]
        output = {
            "family_id": family["id"],
            "candidate_key": key,
            "candidate_sha256": candidate["candidate_sha256"],
            "repo": candidate["repo"],
            "split": "heldout",
            "language": candidate["language"],
            "channel": "current-default",
            "scope": family["scope"],
            "members": family["members"],
            "metric_eligibility": [PRECISION_METRIC],
            "worthy": worthy,
            "reason": reason,
            "confidence": decision["confidence"],
            "labeler": labeler,
            "votes": votes,
            "arbiter": decision["arbiter"],
            "note": decision["note"],
            "selection": {
                "lane": candidate["lane"],
                "product_rank": candidate["rank"],
                "selection_order": candidate["selection_order"],
                "runway": "v7-default-head",
                "selection_reason": candidate["selection_reason"],
            },
        }
        validate_component_family(output, "heldout", f"component.{key}")
        families.append(output)
    return {
        "schema": COMPONENT_SCHEMA,
        "split": "heldout",
        "source_artifact": evidence_record(reveal_path, reveal_content),
        "rubric": evidence_record(heldout.RUBRIC),
        "decision_input": evidence_record(decisions_path, decisions_content),
        "protocol": {
            "panel": list(PERSONA_ORDER),
            "split_votes_escalate_to": "llm-arbiter",
            "metric_eligibility": [PRECISION_METRIC],
            "policy_or_ranking_changes": "none",
        },
        "families": families,
    }


def reveal_payload(
    root_seed: bytes,
    candidates: list[dict[str, Any]],
    panel_packet_sources: dict[str, Path],
    arbiter_packet_source: Path,
    collector_commit: str,
    collector_tree: str,
) -> dict[str, Any]:
    result_payload = heldout.read_json(arbitration_result.RESULT)
    arbiter_ids = revealed_arbiter_ids(candidates, root_seed, result_payload)
    records = []
    for candidate in candidates:
        key = candidate["candidate_key"]
        records.append(
            {
                "candidate": candidate,
                "blind_ids": {
                    persona: blind_id(heldout.persona_seed(root_seed, persona), key)
                    for persona in heldout.PERSONAS
                }
                | {"arbiter": arbiter_ids[key]},
            }
        )
    return {
        "schema": REVEAL_SCHEMA,
        "issue": 846,
        "split": "heldout",
        "state": "blind-judgments-revealed",
        "root_seed_hex": root_seed.hex(),
        "root_seed_commitment_sha256": hashlib.sha256(root_seed).hexdigest(),
        "selection": {
            "count": len(candidates),
            "selected_candidate_keys_sha256": selection_digest(candidates),
        },
        "upstream": {
            "panel_commitment": evidence_record(heldout.COMMITMENT),
            "arbitration_commitment": evidence_record(arbitration.COMMITMENT),
            "arbitration_result": evidence_record(arbitration_result.RESULT),
        },
        "revealed_packets": {
            "panel": [
                {
                    "persona": persona,
                    "path": PANEL_PACKET_PATHS[persona].name,
                    "sha256": heldout.sha256_file(panel_packet_sources[persona]),
                    "byte_length": panel_packet_sources[persona].stat().st_size,
                }
                for persona in heldout.PERSONAS
            ],
            "arbiter": {
                "path": ARBITER_PACKET.name,
                "sha256": heldout.sha256_file(arbiter_packet_source),
                "byte_length": arbiter_packet_source.stat().st_size,
            },
        },
        "candidates": records,
        "provenance": {
            "command": FREEZE_COMMAND,
            "collector": file_record(Path(__file__).resolve()),
            "collector_commit": collector_commit,
            "collector_tree": collector_tree,
            "working_tree_status_before_reveal": "",
            "nose_binary_sha256": heldout.sha256_file(heldout.OFFICIAL_NOSE),
            "corpus_manifest_sha256": heldout.sha256_file(heldout.CORPUS),
            "base_labelset_sha256": heldout.sha256_file(heldout.BASE_LABELSET),
        },
    }


def selection_digest(candidates: list[dict[str, Any]]) -> str:
    return heldout.canonical_sha256([row["candidate_key"] for row in candidates])


def revealed_arbiter_ids(
    candidates: list[dict[str, Any]],
    root_seed: bytes,
    result_payload: dict[str, Any],
) -> dict[str, str | None]:
    result_ids = {vote["blind_id"] for vote in result_payload["votes"]}
    seed = heldout.persona_seed(root_seed, "arbiter")
    revealed = {
        candidate["candidate_key"]: (
            candidate_id
            if (candidate_id := blind_id(seed, candidate["candidate_key"])) in result_ids
            else None
        )
        for candidate in candidates
    }
    matched_ids = {value for value in revealed.values() if value is not None}
    require_equal(matched_ids, result_ids, "revealed arbitration IDs")
    return revealed


def validate_packet_receipts(
    reveal: dict[str, Any],
    panel_commitment: dict[str, Any],
    arbitration_commitment: dict[str, Any],
) -> None:
    require_equal(
        set(reveal["revealed_packets"]),
        {"panel", "arbiter"},
        "revealed packet fields",
    )
    expected_panel_records = []
    for persona in heldout.PERSONAS:
        committed = panel.packet_record(panel_commitment, persona)
        expected_panel_records.append(
            {
                "persona": persona,
                "path": PANEL_PACKET_PATHS[persona].name,
                "sha256": committed["sha256"],
                "byte_length": committed["byte_length"],
            }
        )
    require_equal(
        reveal["revealed_packets"]["panel"],
        expected_panel_records,
        "revealed panel packet commitments",
    )
    committed_arbiter = arbitration_commitment["arbitration_packet"]
    require_equal(
        reveal["revealed_packets"]["arbiter"],
        {
            "path": ARBITER_PACKET.name,
            "sha256": committed_arbiter["sha256"],
            "byte_length": committed_arbiter["byte_length"],
        },
        "revealed arbiter packet commitment",
    )


def validate_provenance(payload: dict[str, Any], seal: dict[str, Any]) -> None:
    provenance = payload["provenance"]
    require_equal(
        set(provenance),
        {
            "command",
            "collector",
            "collector_commit",
            "collector_tree",
            "working_tree_status_before_reveal",
            "nose_binary_sha256",
            "corpus_manifest_sha256",
            "base_labelset_sha256",
        },
        "reveal provenance fields",
    )
    require_equal(provenance["command"], FREEZE_COMMAND, "reveal command")
    require_equal(
        provenance["working_tree_status_before_reveal"], "", "clean reveal"
    )
    for field in (
        "nose_binary_sha256",
        "corpus_manifest_sha256",
        "base_labelset_sha256",
    ):
        require_equal(
            provenance[field], seal["provenance"][field], f"reveal {field}"
        )
    commit = provenance["collector_commit"]
    tree = provenance["collector_tree"]
    heldout.require_hex(commit, 40, "reveal collector commit")
    heldout.require_hex(tree, 40, "reveal collector tree")
    require_equal(
        heldout.git_text(["rev-parse", f"{commit}^{{tree}}"]),
        tree,
        "reveal collector tree",
    )
    collector = provenance["collector"]
    require_equal(
        set(collector), {"path", "sha256", "byte_length"}, "reveal collector record"
    )
    require_equal(
        collector["path"], Path(__file__).name, "reveal collector path"
    )
    collector_path = f"bench/labels/{collector['path']}"
    frozen = heldout.git_bytes(["show", f"{commit}:{collector_path}"])
    require_equal(len(frozen), collector["byte_length"], "reveal collector bytes")
    require_equal(
        hashlib.sha256(frozen).hexdigest(),
        collector["sha256"],
        "reveal collector SHA",
    )
    require_equal(Path(__file__).read_bytes(), frozen, "current reveal collector")
    require_ancestor(
        result_receipt.RESULT_COMMIT,
        commit,
        "result-before-reveal-collector chronology",
    )
    require_ancestor(commit, "HEAD", "reveal collector ancestry")


def load_revealed_packets() -> tuple[dict[str, dict[str, Any]], dict[str, Any]]:
    packets = {
        persona: heldout.read_json(PANEL_PACKET_PATHS[persona])
        for persona in heldout.PERSONAS
    }
    for persona, packet_payload in packets.items():
        heldout.validate_private_packet(packet_payload, persona)
    arbiter = heldout.read_json(ARBITER_PACKET)
    arbitration.validate_arbiter_packet(arbiter)
    return packets, arbiter


def transaction_payload(contents: dict[Path, bytes]) -> dict[str, Any]:
    return {
        "schema": "nose.default_head_heldout_reveal_transaction.v1",
        "outputs": [
            {
                "path": path.name,
                "sha256": hashlib.sha256(content).hexdigest(),
                "byte_length": len(content),
            }
            for path, content in contents.items()
        ],
    }


def recover_interrupted_publish(
    outputs: list[Path], transaction: Path = TRANSACTION
) -> None:
    if not transaction.exists():
        return
    payload = heldout.read_json(transaction)
    expected_names = [path.name for path in outputs]
    if set(payload) != {"schema", "outputs"} or payload["schema"] != (
        "nose.default_head_heldout_reveal_transaction.v1"
    ):
        raise ValueError("invalid held-out reveal transaction marker")
    records = payload["outputs"]
    if not isinstance(records, list) or len(records) != len(outputs):
        raise ValueError("held-out reveal transaction outputs mismatch")
    checked: list[tuple[Path, dict[str, Any]]] = []
    for index, (path, record) in enumerate(zip(outputs, records, strict=True)):
        if not isinstance(record, dict) or set(record) != {
            "path",
            "sha256",
            "byte_length",
        }:
            raise ValueError("invalid held-out reveal transaction output")
        require_equal(record["path"], expected_names[index], "transaction path")
        heldout.require_hex(record["sha256"], 64, "transaction SHA")
        if (
            isinstance(record["byte_length"], bool)
            or not isinstance(record["byte_length"], int)
            or record["byte_length"] < 1
        ):
            raise ValueError("invalid held-out reveal transaction byte length")
        checked.append((path, record))
    for path, record in checked:
        if not path.exists():
            continue
        if (
            heldout.sha256_file(path) != record["sha256"]
            or path.stat().st_size != record["byte_length"]
        ):
            raise ValueError(f"refusing to remove mismatched interrupted output: {path}")
    for path, _ in checked:
        if not path.exists():
            continue
        path.unlink()
    transaction.unlink()


def content_matches(path: Path, content: bytes) -> bool:
    return (
        path.is_file()
        and path.stat().st_size == len(content)
        and heldout.sha256_file(path) == hashlib.sha256(content).hexdigest()
    )


def rollback_publish(
    published: list[Path],
    contents: dict[Path, bytes],
    transaction: Path,
    marker: bytes,
    *,
    marker_owned: bool,
) -> None:
    if not marker_owned:
        return
    if transaction.exists() and not content_matches(transaction, marker):
        return
    if any(path.exists() and not content_matches(path, contents[path]) for path in published):
        return
    for path in reversed(published):
        path.unlink(missing_ok=True)
    if transaction.exists():
        transaction.unlink()


def publish_outputs(
    contents: dict[Path, bytes],
    *,
    transaction: Path = TRANSACTION,
    validator: Callable[[], None] | None = None,
    staging_parent: Path = ROOT / "target",
    marker_writer: Callable[[Path, bytes], None] | None = None,
    source_unlinker: Callable[[Path], None] | None = None,
) -> None:
    marker = heldout.packet_bytes(transaction_payload(contents))
    published: list[Path] = []
    marker_owned = False
    try:
        with tempfile.TemporaryDirectory(
            prefix=".heldout-reveal-staging-", dir=staging_parent
        ) as directory:
            staging = Path(directory)
            staged_marker = staging / transaction.name
            if marker_writer is None:
                heldout.write_exclusive(staged_marker, marker, 0o600)
            else:
                marker_writer(staged_marker, marker)
            os.link(staged_marker, transaction)
            marker_owned = True
            staged_marker.unlink()
            for target, content in contents.items():
                source = staging / target.name
                heldout.write_exclusive(source, content, 0o644)
                os.link(source, target)
                published.append(target)
                if source_unlinker is None:
                    source.unlink()
                else:
                    source_unlinker(source)
        if validator is None:
            validate_checked(allow_transaction=True)
        else:
            validator()
        if not content_matches(transaction, marker):
            raise ValueError("held-out reveal transaction marker changed")
        transaction.unlink()
    except BaseException:
        rollback_publish(
            published,
            contents,
            transaction,
            marker,
            marker_owned=marker_owned,
        )
        raise


def freeze(args: argparse.Namespace) -> None:
    outputs = [REVEAL, DECISIONS, COMPONENT, ARBITER_PACKET, *PANEL_PACKET_PATHS.values()]
    recover_interrupted_publish(outputs)
    existing = [path for path in outputs if path.exists()]
    if existing:
        raise ValueError(f"refusing to replace reveal output: {existing[0]}")
    status = heldout.git_text(["status", "--short"])
    if status:
        raise ValueError("held-out reveal requires a clean working tree")
    root_seed = heldout.read_root_seed()
    panel_commitment = panel.read_commitment()
    require_equal(
        hashlib.sha256(root_seed).hexdigest(),
        panel_commitment["protocol"]["root_seed_commitment_sha256"],
        "root seed commitment",
    )
    vote_payloads = read_frozen_votes()
    result_receipt.validate(None)
    _, candidates = heldout.replay(args)
    private_dir = heldout.require_private_directory(args.private_panel_dir, empty=False)
    panel_sources: dict[str, Path] = {}
    packets: dict[str, dict[str, Any]] = {}
    for persona in heldout.PERSONAS:
        source, actual = panel.private_packet(private_dir, persona, panel_commitment)
        expected = heldout.private_packet(persona, candidates, root_seed, heldout.RUBRIC)
        require_equal(actual, expected, f"{persona} packet reveal replay")
        panel_sources[persona] = source
        packets[persona] = actual
    arbitration_commitment = heldout.read_json(arbitration.COMMITMENT)
    arbitration_receipt.validate_payload(arbitration_commitment)
    arbiter_source, arbiter_packet = arbitration.private_packet_receipt(
        args.private_arbiter_packet, arbitration_commitment
    )
    aligned = align_panel_votes(candidates, root_seed, packets, vote_payloads)
    expected_arbiter_packet = arbitration.arbiter_packet(candidates, aligned, root_seed)
    require_equal(arbiter_packet, expected_arbiter_packet, "arbiter packet reveal replay")
    result_payload = heldout.read_json(arbitration_result.RESULT)
    arbitration_result.validate_result_payload(
        result_payload, arbiter_packet, arbitration_commitment
    )
    resolutions = align_arbiter_result(
        candidates, aligned, root_seed, arbiter_packet, result_payload, packets
    )
    decisions = final_decisions(candidates, aligned, resolutions)
    collector_commit = heldout.git_text(["rev-parse", "HEAD"])
    collector_tree = heldout.git_text(["rev-parse", "HEAD^{tree}"])
    reveal = reveal_payload(
        root_seed,
        candidates,
        panel_sources,
        arbiter_source,
        collector_commit,
        collector_tree,
    )
    validate_packet_receipts(reveal, panel_commitment, arbitration_commitment)
    checked_seed, checked_candidates = validate_reveal_payload(reveal)
    require_equal(checked_seed, root_seed, "pre-publish revealed root seed")
    require_equal(checked_candidates, candidates, "pre-publish revealed candidates")
    reveal_content = heldout.packet_bytes(reveal)
    decisions_value = decisions_payload(REVEAL, decisions, reveal_content)
    decisions_content = heldout.packet_bytes(decisions_value)
    component = component_payload(
        REVEAL,
        candidates,
        DECISIONS,
        decisions,
        reveal_content,
        decisions_content,
    )
    contents = {
        REVEAL: reveal_content,
        DECISIONS: decisions_content,
        COMPONENT: heldout.packet_bytes(component),
        ARBITER_PACKET: arbiter_source.read_bytes(),
        **{
            PANEL_PACKET_PATHS[persona]: panel_sources[persona].read_bytes()
            for persona in heldout.PERSONAS
        },
    }
    require_equal(list(contents), outputs, "reveal publication order")
    publish_outputs(contents)
    print(
        f"revealed {len(candidates)} candidates with {len(resolutions)} "
        f"arbitrations and {sum(row['worthy'] for row in component['families'])} worthy"
    )


def validate_reveal_payload(payload: dict[str, Any]) -> tuple[bytes, list[dict[str, Any]]]:
    require_equal(
        set(payload),
        {
            "schema",
            "issue",
            "split",
            "state",
            "root_seed_hex",
            "root_seed_commitment_sha256",
            "selection",
            "upstream",
            "revealed_packets",
            "candidates",
            "provenance",
        },
        "reveal fields",
    )
    require_equal(payload["schema"], REVEAL_SCHEMA, "reveal schema")
    require_equal(payload["issue"], 846, "reveal issue")
    require_equal(payload["split"], "heldout", "reveal split")
    require_equal(payload["state"], "blind-judgments-revealed", "reveal state")
    root_seed_hex = payload["root_seed_hex"]
    heldout.require_hex(root_seed_hex, 64, "revealed root seed")
    root_seed = bytes.fromhex(root_seed_hex)
    panel_commitment = panel.read_commitment()
    require_equal(
        hashlib.sha256(root_seed).hexdigest(),
        panel_commitment["protocol"]["root_seed_commitment_sha256"],
        "revealed root seed commitment",
    )
    require_equal(
        payload["root_seed_commitment_sha256"],
        panel_commitment["protocol"]["root_seed_commitment_sha256"],
        "recorded root seed commitment",
    )
    require_equal(
        payload["upstream"],
        {
            "panel_commitment": evidence_record(heldout.COMMITMENT),
            "arbitration_commitment": evidence_record(arbitration.COMMITMENT),
            "arbitration_result": evidence_record(arbitration_result.RESULT),
        },
        "reveal upstream",
    )
    records = payload["candidates"]
    if not isinstance(records, list) or len(records) != 214:
        raise ValueError("reveal candidate count mismatch")
    candidates = []
    seen: set[str] = set()
    seal = heldout.validate_seal_receipt()
    commitments = {
        row["candidate_key"]: row for row in seal["candidate_commitments"] if row["selected"]
    }
    for index, record in enumerate(records, start=1):
        if not isinstance(record, dict) or set(record) != {"candidate", "blind_ids"}:
            raise ValueError(f"reveal candidates[{index}]: fields mismatch")
        candidate = validate_revealed_candidate(
            record["candidate"], f"reveal candidates[{index}].candidate"
        )
        key = candidate["candidate_key"]
        if key in seen:
            raise ValueError(f"duplicate reveal candidate {key}")
        seen.add(key)
        require_equal(candidate["selection_order"], index, f"{key} selection order")
        require_equal(heldout.commitment(candidate), commitments[key], f"{key} commitment")
        require_equal(
            candidate["candidate_sha256"],
            heldout.canonical_sha256(heldout.runway.runway_candidate_content(candidate)),
            f"{key} candidate SHA",
        )
        expected_ids = {
            persona: blind_id(heldout.persona_seed(root_seed, persona), key)
            for persona in heldout.PERSONAS
        }
        require_equal(
            set(record["blind_ids"]),
            {*heldout.PERSONAS, "arbiter"},
            f"{key} blind ID fields",
        )
        for persona, expected_id in expected_ids.items():
            require_equal(
                record["blind_ids"][persona],
                expected_id,
                f"{key} {persona} blind ID",
            )
        candidates.append(candidate)
    require_equal(
        [row["candidate_key"] for row in candidates],
        seal["selection"]["selected_candidate_keys"],
        "revealed selection",
    )
    revealed_arbiter_ids_by_key = revealed_arbiter_ids(
        candidates, root_seed, heldout.read_json(arbitration_result.RESULT)
    )
    for record in records:
        key = record["candidate"]["candidate_key"]
        require_equal(
            record["blind_ids"]["arbiter"],
            revealed_arbiter_ids_by_key[key],
            f"{key} arbiter blind ID",
        )
    require_equal(
        payload["selection"],
        {"count": 214, "selected_candidate_keys_sha256": selection_digest(candidates)},
        "reveal selection receipt",
    )
    validate_provenance(payload, seal)
    return root_seed, candidates


def require_completed_publish(transaction: Path = TRANSACTION) -> None:
    if transaction.exists():
        raise ValueError("held-out reveal transaction is still in progress")


def validate_checked(*, allow_transaction: bool = False) -> None:
    if not allow_transaction:
        require_completed_publish()
    vote_payloads = read_frozen_votes()
    arbitration_receipt.validate(None)
    result_receipt.validate(None)
    reveal = heldout.read_json(REVEAL)
    panel_commitment = panel.read_commitment()
    arbitration_commitment = heldout.read_json(arbitration.COMMITMENT)
    arbitration_receipt.validate_payload(arbitration_commitment)
    validate_packet_receipts(reveal, panel_commitment, arbitration_commitment)
    root_seed, candidates = validate_reveal_payload(reveal)
    packets, arbiter_packet = load_revealed_packets()
    panel_records = reveal["revealed_packets"]["panel"]
    expected_panel_records = [
        {"persona": persona, **file_record(PANEL_PACKET_PATHS[persona])}
        for persona in heldout.PERSONAS
    ]
    require_equal(panel_records, expected_panel_records, "revealed panel packets")
    require_equal(
        reveal["revealed_packets"]["arbiter"],
        file_record(ARBITER_PACKET),
        "revealed arbiter packet",
    )
    aligned = align_panel_votes(candidates, root_seed, packets, vote_payloads)
    result_payload = heldout.read_json(arbitration_result.RESULT)
    revealed_arbiter_ids_by_key = revealed_arbiter_ids(
        candidates, root_seed, result_payload
    )
    for record in reveal["candidates"]:
        key = record["candidate"]["candidate_key"]
        require_equal(
            record["blind_ids"]["arbiter"],
            revealed_arbiter_ids_by_key[key],
            f"{key} revealed arbiter ID",
        )
    resolutions = align_arbiter_result(
        candidates, aligned, root_seed, arbiter_packet, result_payload, packets
    )
    expected_decisions = final_decisions(candidates, aligned, resolutions)
    require_equal(
        heldout.read_json(DECISIONS),
        decisions_payload(REVEAL, expected_decisions),
        "revealed decisions",
    )
    expected_component = component_payload(
        REVEAL, candidates, DECISIONS, expected_decisions
    )
    require_equal(heldout.read_json(COMPONENT), expected_component, "heldout component")


def validate(_: argparse.Namespace) -> None:
    validate_checked()
    component = heldout.read_json(COMPONENT)
    print(
        f"held-out reveal OK: {len(component['families'])} decisions, "
        f"{sum(row['worthy'] for row in component['families'])} worthy"
    )


def self_test(_: argparse.Namespace) -> None:
    seed = b"x" * 32
    candidates = [
        {"candidate_key": "a"},
        {"candidate_key": "b"},
        {"candidate_key": "c"},
    ]
    first = ordered_keys(candidates, seed)
    second = ordered_keys(candidates, seed)
    require_equal(first, second, "deterministic reveal order")
    if set(first) != {"a", "b", "c"}:
        raise AssertionError("reveal order lost candidates")
    if blind_id(seed, "a") == blind_id(seed, "b"):
        raise AssertionError("synthetic reveal blind IDs collided")
    votes = {
        persona: {"worthy": True, "reason": "extract-helper", "rationale": persona}
        for persona in PERSONA_ORDER
    }
    decisions = final_decisions(
        [{"candidate_key": "a"}], {"a": votes}, {}
    )
    require_equal(decisions[0]["arbiter"], None, "unanimous reveal decision")
    changed_votes = copy.deepcopy(votes)
    changed_votes["skeptic"] = {
        "worthy": False,
        "reason": "trivial",
        "rationale": "no",
    }
    resolution = {"worthy": True, "reason": "extract-helper", "rationale": "yes"}
    decisions = final_decisions(
        [{"candidate_key": "a"}], {"a": changed_votes}, {"a": resolution}
    )
    require_equal(decisions[0]["arbiter"], resolution, "arbitrated reveal decision")
    arbiter_seed = heldout.persona_seed(seed, "arbiter")
    arbiter_id = blind_id(arbiter_seed, "b")
    revealed = revealed_arbiter_ids(
        candidates, seed, {"votes": [{"blind_id": arbiter_id}]}
    )
    require_equal(
        revealed,
        {"a": None, "b": arbiter_id, "c": None},
        "synthetic arbitration mapping",
    )
    invalid_result = {"votes": [{"blind_id": "case-" + "0" * 24}]}
    try:
        revealed_arbiter_ids(candidates, seed, invalid_result)
    except ValueError:
        pass
    else:
        raise AssertionError("unknown arbitration ID was accepted")
    try:
        final_decisions(
            [{"candidate_key": "a"}], {"a": changed_votes}, {}
        )
    except ValueError:
        pass
    else:
        raise AssertionError("missing arbitration was accepted")
    synthetic_member = {
        "file": "bench/repos/example/source.py",
        "start_line": 1,
        "end_line": 2,
        "name": None,
    }
    synthetic_family = {
        key: None for key in REVEALED_FAMILY_KEYS
    }
    synthetic_family["members"] = [synthetic_member, copy.deepcopy(synthetic_member)]
    synthetic_family["member_count"] = 2
    synthetic_candidate = {key: None for key in REVEALED_CANDIDATE_KEYS}
    synthetic_candidate["family"] = synthetic_family
    validate_revealed_candidate(synthetic_candidate, "synthetic candidate")
    changed_candidate = copy.deepcopy(synthetic_candidate)
    changed_candidate["uncommitted_final_worthy"] = True
    try:
        validate_revealed_candidate(changed_candidate, "synthetic candidate")
    except ValueError:
        pass
    else:
        raise AssertionError("uncommitted reveal candidate field was accepted")
    changed_candidate = copy.deepcopy(synthetic_candidate)
    changed_candidate["family"]["uncommitted_final_worthy"] = True
    try:
        validate_revealed_candidate(changed_candidate, "synthetic candidate")
    except ValueError:
        pass
    else:
        raise AssertionError("uncommitted reveal family field was accepted")
    changed_candidate = copy.deepcopy(synthetic_candidate)
    changed_candidate["family"]["members"][0]["source_identity"] = "hidden"
    try:
        validate_revealed_candidate(changed_candidate, "synthetic candidate")
    except ValueError:
        pass
    else:
        raise AssertionError("uncommitted reveal member field was accepted")
    head = heldout.git_text(["rev-parse", "HEAD"])
    try:
        require_ancestor(head, result_receipt.RESULT_COMMIT, "synthetic reverse ancestry")
    except ValueError:
        pass
    else:
        raise AssertionError("reverse reveal ancestry was accepted")
    with tempfile.TemporaryDirectory(prefix="nose-reveal-publish-self-test-") as directory:
        root = Path(directory)
        outputs = [root / "first.json", root / "second.json"]
        contents = {outputs[0]: b"first\n", outputs[1]: b"second\n"}
        transaction = root / "transaction.json"

        def reject_publish() -> None:
            raise RuntimeError("synthetic post-publish validation failure")

        try:
            publish_outputs(
                contents,
                transaction=transaction,
                validator=reject_publish,
                staging_parent=root,
            )
        except RuntimeError:
            pass
        else:
            raise AssertionError("synthetic failed publication was accepted")
        if transaction.exists() or any(path.exists() for path in outputs):
            raise AssertionError("failed publication was not rolled back")

        def partial_marker(path: Path, content: bytes) -> None:
            heldout.write_exclusive(path, content[:1], 0o600)
            raise OSError("synthetic partial marker write")

        try:
            publish_outputs(
                contents,
                transaction=transaction,
                staging_parent=root,
                marker_writer=partial_marker,
            )
        except OSError:
            pass
        else:
            raise AssertionError("partial marker publication was accepted")
        if transaction.exists() or any(path.exists() for path in outputs):
            raise AssertionError("partial marker publication was not rolled back")

        marker = heldout.packet_bytes(transaction_payload(contents))
        heldout.write_exclusive(transaction, marker, 0o600)
        try:
            publish_outputs(
                contents, transaction=transaction, staging_parent=root
            )
        except FileExistsError:
            pass
        else:
            raise AssertionError("identical foreign transaction was accepted")
        require_equal(
            transaction.read_bytes(), marker, "identical foreign transaction"
        )
        if any(path.exists() for path in outputs):
            raise AssertionError("foreign transaction published outputs")
        transaction.unlink()

        heldout.write_exclusive(outputs[0], b"foreign\n", 0o644)
        try:
            publish_outputs(
                contents, transaction=transaction, staging_parent=root
            )
        except FileExistsError:
            pass
        else:
            raise AssertionError("foreign reveal output was overwritten")
        require_equal(outputs[0].read_bytes(), b"foreign\n", "foreign reveal output")
        if transaction.exists() or outputs[1].exists():
            raise AssertionError("no-clobber publication was not rolled back")
        outputs[0].unlink()

        def reject_staged_unlink(_: Path) -> None:
            raise OSError("synthetic staged unlink failure")

        try:
            publish_outputs(
                contents,
                transaction=transaction,
                staging_parent=root,
                source_unlinker=reject_staged_unlink,
            )
        except OSError:
            pass
        else:
            raise AssertionError("mid-promotion unlink failure was accepted")
        if transaction.exists() or any(path.exists() for path in outputs):
            raise AssertionError("mid-promotion unlink failure was not rolled back")

        def remove_marker() -> None:
            transaction.unlink()

        try:
            publish_outputs(
                contents,
                transaction=transaction,
                validator=remove_marker,
                staging_parent=root,
            )
        except ValueError:
            pass
        else:
            raise AssertionError("missing success marker was accepted")
        if transaction.exists() or any(path.exists() for path in outputs):
            raise AssertionError("missing success marker was not rolled back")

        def mutate_publish() -> None:
            outputs[0].write_bytes(b"mutated\n")
            raise RuntimeError("synthetic mutated publication")

        try:
            publish_outputs(
                contents,
                transaction=transaction,
                validator=mutate_publish,
                staging_parent=root,
            )
        except RuntimeError:
            pass
        else:
            raise AssertionError("mutated failed publication was accepted")
        require_equal(outputs[0].read_bytes(), b"mutated\n", "mutated reveal output")
        if not transaction.exists() or not outputs[1].exists():
            raise AssertionError("mismatched rollback did not preserve transaction")
        try:
            require_completed_publish(transaction)
        except ValueError:
            pass
        else:
            raise AssertionError("outstanding reveal transaction was accepted")
        for path in outputs:
            path.unlink(missing_ok=True)
        transaction.unlink()

        heldout.write_exclusive(
            transaction,
            heldout.packet_bytes(transaction_payload(contents)),
            0o600,
        )
        for path, content in contents.items():
            heldout.write_exclusive(path, content, 0o644)
        recover_interrupted_publish(outputs, transaction)
        if transaction.exists() or any(path.exists() for path in outputs):
            raise AssertionError("interrupted publication was not recovered")
    print("default-head held-out reveal self-test passed")


def add_live_arguments(parser: argparse.ArgumentParser) -> None:
    heldout.add_live_arguments(parser)


def parser() -> argparse.ArgumentParser:
    root = argparse.ArgumentParser()
    commands = root.add_subparsers(dest="command", required=True)
    freeze_parser = commands.add_parser("freeze", allow_abbrev=False)
    add_live_arguments(freeze_parser)
    freeze_parser.add_argument("--private-panel-dir", type=Path, required=True)
    freeze_parser.add_argument("--private-arbiter-packet", type=Path, required=True)
    freeze_parser.set_defaults(run=freeze)
    validate_parser = commands.add_parser("validate")
    validate_parser.set_defaults(run=validate)
    self_parser = commands.add_parser("self-test")
    self_parser.set_defaults(run=self_test)
    return root


def main() -> None:
    args = parser().parse_args()
    try:
        args.run(args)
    except (KeyError, ValueError) as error:
        raise SystemExit(str(error)) from error


if __name__ == "__main__":
    main()
