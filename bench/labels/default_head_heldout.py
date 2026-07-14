#!/usr/bin/env python3
"""One-shot, secretly permuted held-out packet builder for issue #846."""

from __future__ import annotations

import argparse
import copy
import getpass
import hashlib
import hmac
import json
import os
import shlex
import subprocess
from collections import defaultdict
from pathlib import Path
from typing import Any

import label_refresh as runway


ROOT = Path(__file__).resolve().parents[2]
SEAL = ROOT / "bench/labels/default_head_label_runway_2026_07_13.heldout.seal.v1.json"
COMMITMENT = (
    ROOT / "bench/labels/default_head_heldout_commitment_2026_07_14.v3.json"
)
CORPUS = ROOT / "bench/goldens/corpus.json"
BASE_LABELSET = ROOT / "bench/labels/refactoring_families.v6.json"
RUBRIC = ROOT / "bench/labels/RUBRIC.md"
OFFICIAL_NOSE = (
    ROOT
    / "target/issue-839/official-v0.19.0/"
    "nose-cli-aarch64-apple-darwin/nose"
)
REPOS_ROOT = ROOT / "bench/repos"

COMMITMENT_SCHEMA = "nose.default_head_heldout_commitment.v3"
PRIVATE_PACKET_SCHEMA = "nose.default_head_heldout_private_packet.v3"
FREEZE_COMMAND = (
    "python3 bench/labels/default_head_heldout.py freeze "
    "--private-dir <outside-repository>"
)
SEAL_COMMIT = "f945053520506c92c0dc72fe09c7fdb685d29e77"
SEAL_TREE = "78797deeaaa2aef346bad051ef96f33424352364"
SEAL_SHA256 = "b99c396544848af84a522d5b023c0304bef835ac5dafc6dbc744c2aab6843004"
SEAL_PATH = SEAL.relative_to(ROOT).as_posix()
CONTEXT_LINES = 12
MAX_TEXT_CHARS = 20_000
HIDDEN_FIELDS = [
    "candidate_key",
    "candidate_sha256",
    "raw_family_sha256",
    "repo",
    "rank",
    "lane",
    "base_matched",
    "selection_reason",
    "selection_order",
    "family.id",
    "member.file",
    "member.start_line",
    "member.end_line",
    "matched_v6_family_id",
    "matched_v6_member_overlap",
]
PERSONAS = ("dedupe", "pragmatic", "skeptic")
VISIBLE_CANDIDATE_KEYS = {"blind_id", "language", "family"}
VISIBLE_FAMILY_KEYS = {"members", "member_count"}
VISIBLE_MEMBER_KEYS = {
    "source_id",
    "context_before",
    "source",
    "context_after",
}
REVIEWER_ATTESTATION = {
    "assigned_material_only": True,
    "no_git_or_corpus_lookup": True,
    "no_network_or_source_identity_lookup": True,
    "no_other_votes_or_reviewer_contact": True,
}


def read_json(path: Path) -> dict[str, Any]:
    try:
        value = json.loads(path.read_text(encoding="utf-8"))
    except (OSError, json.JSONDecodeError) as error:
        raise ValueError(f"cannot read {path}: {error}") from error
    if not isinstance(value, dict):
        raise ValueError(f"{path}: expected an object")
    return value


def canonical_bytes(value: object) -> bytes:
    return json.dumps(value, sort_keys=True, separators=(",", ":")).encode()


def canonical_sha256(value: object) -> str:
    return hashlib.sha256(canonical_bytes(value)).hexdigest()


def sha256_file(path: Path) -> str:
    return hashlib.sha256(path.read_bytes()).hexdigest()


def relative(path: Path) -> str:
    return path.resolve().relative_to(ROOT).as_posix()


def path_record(path: Path) -> dict[str, str]:
    return {"path": relative(path), "sha256": sha256_file(path)}


def require_equal(actual: object, expected: object, label: str) -> None:
    if actual != expected:
        raise ValueError(f"{label}: mismatch")


def require_exact_keys(value: object, expected: set[str], label: str) -> dict[str, Any]:
    if not isinstance(value, dict) or set(value) != expected:
        raise ValueError(f"{label}: expected exact keys {sorted(expected)}")
    return value


def require_hex(value: object, size: int, label: str) -> str:
    if (
        not isinstance(value, str)
        or len(value) != size
        or any(character not in "0123456789abcdef" for character in value)
    ):
        raise ValueError(f"{label}: expected {size} lowercase hex characters")
    return value


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


def seal_receipt() -> dict[str, str]:
    return {
        "commit": SEAL_COMMIT,
        "tree": SEAL_TREE,
        "path": SEAL_PATH,
        "sha256": SEAL_SHA256,
    }


def validate_seal_receipt(*, live_binary: Path | None = None) -> dict[str, Any]:
    require_equal(
        git_text(["rev-parse", f"{SEAL_COMMIT}^{{tree}}"]), SEAL_TREE, "seal tree"
    )
    frozen = git_bytes(["show", f"{SEAL_COMMIT}:{SEAL_PATH}"])
    require_equal(hashlib.sha256(frozen).hexdigest(), SEAL_SHA256, "seal blob")
    require_equal(sha256_file(SEAL), SEAL_SHA256, "current seal bytes")
    subprocess.run(
        ["git", "merge-base", "--is-ancestor", SEAL_COMMIT, "HEAD"],
        cwd=ROOT,
        check=True,
    )
    seal = read_json(SEAL)
    runway.validate_heldout_seal(seal)
    provenance = seal["provenance"]
    revision = provenance["git_sha"]
    for record in provenance["collection_sources"]:
        frozen_source = git_bytes(["show", f"{revision}:{record['path']}"])
        require_equal(
            hashlib.sha256(frozen_source).hexdigest(),
            record["sha256"],
            f"frozen collection source {record['path']}",
        )
    for field, path in (
        ("corpus_manifest_sha256", CORPUS),
        ("base_labelset_sha256", BASE_LABELSET),
        ("rubric_sha256", RUBRIC),
    ):
        require_equal(sha256_file(path), provenance[field], field)
    if live_binary is not None:
        require_equal(sha256_file(live_binary), provenance["nose_binary_sha256"], "binary")
        require_equal(runway.nose_version(live_binary), provenance["nose_version"], "version")
    return seal


def compact_candidate(
    repo_id: str,
    metadata: dict[str, Any],
    rank: int,
    family: dict[str, Any],
    base_by_repo: dict[str, list[dict[str, Any]]],
) -> dict[str, Any]:
    members = runway.normalized_members(family)
    match_id, match_overlap = runway.best_label_match(
        members, base_by_repo.get(repo_id, [])
    )
    base_matched = match_id is not None
    lane = (
        "base-matched-default-head"
        if rank <= 10 and base_matched
        else (
            "unmatched-default-head"
            if rank <= 10
            else "base-matched-rank-11-30"
            if base_matched
            else "unmatched-rank-11-30"
        )
    )
    compact_family = {
        "id": family["id"],
        "members": members,
        "member_count": len(members),
        "scope": family.get("scope"),
        "surface": family.get("surface"),
        "witness": family.get("witness"),
        "extraction_shape": family.get("extraction_shape"),
        "value": family.get("value"),
        "matched_v6_family_id": match_id,
        "matched_v6_member_overlap": match_overlap,
    }
    candidate = {
        "candidate_key": f"{repo_id}:{family['id']}:rank-{rank}",
        "repo": repo_id,
        "split": "heldout",
        "language": metadata["primary_language"],
        "lane": lane,
        "rank": rank,
        "base_matched": base_matched,
        "family": compact_family,
        "raw_family_sha256": canonical_sha256(family),
    }
    candidate["candidate_sha256"] = canonical_sha256(
        runway.runway_candidate_content(candidate)
    )
    return candidate


def commitment(candidate: dict[str, Any]) -> dict[str, Any]:
    return {
        key: candidate[key]
        for key in (
            "candidate_key",
            "candidate_sha256",
            "repo",
            "split",
            "language",
            "lane",
            "rank",
            "base_matched",
            "selected",
            "selection_reason",
            "selection_order",
        )
    }


def replay(args: argparse.Namespace) -> tuple[dict[str, Any], list[dict[str, Any]]]:
    seal = validate_seal_receipt(live_binary=args.nose)
    corpus_payload = read_json(args.corpus)
    corpus_rows = corpus_payload.get("repositories")
    if not isinstance(corpus_rows, list):
        raise ValueError("corpus needs a repositories array")
    corpus = {row["id"]: row for row in corpus_rows}
    base = runway.load_labelset(args.base_labelset)
    require_equal(base.version, "v6", "base labelset version")
    base_by_repo: dict[str, list[dict[str, Any]]] = defaultdict(list)
    for family in base.families:
        base_by_repo[family["repo"]].append(family)

    nose_command = Path(relative(args.nose))
    candidates: list[dict[str, Any]] = []
    repositories: dict[str, dict[str, Any]] = {}
    for repo_id in sorted(seal["repositories"]):
        metadata = corpus[repo_id]
        require_equal(metadata["split"], "heldout", f"{repo_id} split")
        repo = args.repos_root / repo_id
        if not repo.is_dir():
            raise ValueError(f"missing pinned repository: {repo_id}")
        actual_commit = runway.repository_head(repo)
        require_equal(actual_commit, metadata["commit"], f"{repo_id} commit")
        stdout, families, query_command = runway.query_default_runway_repo(
            nose_command, repo
        )
        repo_candidates = [
            compact_candidate(repo_id, metadata, rank, family, base_by_repo)
            for rank, family in enumerate(families, start=1)
        ]
        candidates.extend(repo_candidates)
        top_10 = repo_candidates[:10]
        repositories[repo_id] = {
            "commit": actual_commit,
            "language": metadata["primary_language"],
            "split": "heldout",
            "query_command": shlex.join(query_command),
            "query_stdout_sha256": hashlib.sha256(stdout).hexdigest(),
            "top_30_reported": len(repo_candidates),
            "top_10_reported": len(top_10),
            "base_matched_top_10": sum(row["base_matched"] for row in top_10),
            "unmatched_top_10": sum(not row["base_matched"] for row in top_10),
        }
    require_equal(repositories, seal["repositories"], "repository replay")
    selected_keys = runway.apply_runway_selection(candidates)
    require_equal(selected_keys, seal["selection"]["selected_candidate_keys"], "selection")
    require_equal(
        runway.runway_pool_summary(candidates, repositories), seal["pool"], "pool replay"
    )
    ordered = sorted(candidates, key=lambda row: (row["repo"], row["rank"]))
    require_equal(
        [commitment(candidate) for candidate in ordered],
        seal["candidate_commitments"],
        "candidate commitments",
    )
    by_key = {candidate["candidate_key"]: candidate for candidate in candidates}
    return seal, [by_key[key] for key in selected_keys]


def hmac_hex(seed: bytes, domain: str, value: str) -> str:
    return hmac.new(seed, f"{domain}\0{value}".encode(), hashlib.sha256).hexdigest()


def persona_seed(root_seed: bytes, persona: str) -> bytes:
    return hmac.new(
        root_seed, f"persona\0{persona}".encode(), hashlib.sha256
    ).digest()


def bounded_text(text: str) -> str:
    if len(text) <= MAX_TEXT_CHARS:
        return text
    half = MAX_TEXT_CHARS // 2
    return text[:half] + "\n… <blind excerpt truncated> …\n" + text[-half:]


def opaque_member(member: dict[str, Any], seed: bytes) -> dict[str, Any]:
    path_text = member["file"]
    path = ROOT / path_text
    lines = path.read_text(encoding="utf-8", errors="replace").splitlines()
    start = member["start_line"]
    end = member["end_line"]
    if start < 1 or end < start or end > len(lines):
        raise ValueError(f"invalid source span in {path_text}")
    record: dict[str, Any] = {
        "source_id": f"source-{hmac_hex(seed, 'source', path_text)[:24]}",
        "context_before": bounded_text(
            "\n".join(lines[max(0, start - 1 - CONTEXT_LINES) : start - 1])
        ),
        "source": bounded_text("\n".join(lines[start - 1 : end])),
        "context_after": bounded_text("\n".join(lines[end : end + CONTEXT_LINES])),
    }
    return record


def blind_candidate(candidate: dict[str, Any], seed: bytes) -> dict[str, Any]:
    family = candidate["family"]
    return {
        "blind_id": f"case-{hmac_hex(seed, 'blind-id', candidate['candidate_key'])[:24]}",
        "language": candidate["language"],
        "family": {
            "members": [opaque_member(member, seed) for member in family["members"]],
            "member_count": family["member_count"],
        },
    }


def blind_projection(
    selected: list[dict[str, Any]], seed: bytes
) -> list[dict[str, Any]]:
    ordered = sorted(
        selected,
        key=lambda candidate: hmac_hex(seed, "order", candidate["candidate_key"]),
    )
    return [blind_candidate(candidate, seed) for candidate in ordered]


def private_packet(
    persona: str,
    selected: list[dict[str, Any]],
    root_seed: bytes,
    rubric: Path,
) -> dict[str, Any]:
    seed = persona_seed(root_seed, persona)
    return {
        "schema": PRIVATE_PACKET_SCHEMA,
        "issue": 846,
        "split": "heldout",
        "persona": persona,
        "judgment_status": "procedurally-blind-unjudged",
        "packet_nonce": hmac_hex(seed, "packet-nonce", persona),
        "rubric_sha256": sha256_file(rubric),
        "reviewer_protocol": {
            "guarantee": "procedural-product-metadata-blindness",
            "not_guaranteed": (
                "identity hiding from a reviewer who searches remembered or public source"
            ),
            "allowed_material": ["assigned packet", "bound rubric"],
            "prohibited_actions": [
                "inspect Git, the corpus, repositories, or unassigned files",
                "use network access or search for source identity",
                "read another reviewer's packet or vote",
                "contact another reviewer",
            ],
            "required_vote_attestation": REVIEWER_ATTESTATION,
        },
        "candidates": blind_projection(selected, seed),
    }


def packet_bytes(payload: dict[str, Any]) -> bytes:
    return canonical_bytes(payload) + b"\n"


def packet_commitment(persona: str, payload: dict[str, Any]) -> dict[str, Any]:
    content = packet_bytes(payload)
    return {
        "persona": persona,
        "schema": PRIVATE_PACKET_SCHEMA,
        "sha256": hashlib.sha256(content).hexdigest(),
        "byte_length": len(content),
        "candidate_count": len(payload["candidates"]),
    }


def collect_commitment(
    args: argparse.Namespace, root_seed: bytes
) -> tuple[dict[str, Any], dict[str, dict[str, Any]]]:
    status = git_text(["status", "--short"])
    if status:
        raise ValueError("held-out unseal requires a clean working tree")
    if len(root_seed) != 32:
        raise ValueError("blind root seed must contain exactly 32 bytes")
    seal, selected = replay(args)
    packets = {
        persona: private_packet(persona, selected, root_seed, args.rubric)
        for persona in PERSONAS
    }
    collector = Path(__file__).resolve()
    commitment = {
        "schema": COMMITMENT_SCHEMA,
        "issue": 846,
        "split": "heldout",
        "state": "private-packets-committed",
        "seal_receipt": seal_receipt(),
        "selection": {
            "count": len(selected),
            "sealed_candidate_keys_sha256": seal["selection"][
                "selected_candidate_keys_sha256"
            ],
        },
        "rubric": path_record(args.rubric),
        "protocol": {
            "guarantee": "procedural-product-metadata-blindness",
            "not_guaranteed": (
                "identity hiding from a reviewer who searches remembered or public source"
            ),
            "hidden_fields": HIDDEN_FIELDS,
            "visible_fields": sorted(VISIBLE_CANDIDATE_KEYS),
            "root_seed_commitment_sha256": hashlib.sha256(root_seed).hexdigest(),
            "persona_isolation": "independent derived seed, permutation, case IDs, and source IDs",
            "private_packet_location": "outside Git and the project workspace",
            "raw_vote_freeze": "all three persona votes in one commit",
            "mapping_release": "after blind-ID arbitration is frozen",
            "release_order": [
                "private packet commitments",
                "three raw votes frozen atomically",
                "blind-ID arbitration frozen",
                "packet, seeds, and exact-key mapping revealed",
                "decisions and metrics",
            ],
        },
        "packets": [
            packet_commitment(persona, packets[persona]) for persona in PERSONAS
        ],
        "provenance": {
            "command": FREEZE_COMMAND,
            "unseal_commit": git_text(["rev-parse", "HEAD"]),
            "unseal_tree": git_text(["rev-parse", "HEAD^{tree}"]),
            "working_tree_status_before_unseal": status,
            "collector": path_record(collector),
            "nose_binary": relative(args.nose),
            "nose_binary_sha256": sha256_file(args.nose),
            "nose_version": runway.nose_version(args.nose),
            "corpus": path_record(args.corpus),
            "base_labelset": path_record(args.base_labelset),
        },
    }
    return commitment, packets


def validate_commitment(payload: dict[str, Any]) -> None:
    require_exact_keys(
        payload,
        {
            "schema",
            "issue",
            "split",
            "state",
            "seal_receipt",
            "selection",
            "rubric",
            "protocol",
            "packets",
            "provenance",
        },
        "commitment artifact",
    )
    require_equal(payload["schema"], COMMITMENT_SCHEMA, "schema")
    require_equal(payload["issue"], 846, "issue")
    require_equal(payload["split"], "heldout", "split")
    require_equal(payload["state"], "private-packets-committed", "state")
    seal = validate_seal_receipt()
    require_equal(payload["seal_receipt"], seal_receipt(), "seal receipt")
    require_equal(payload["rubric"], path_record(RUBRIC), "rubric")
    selection = require_exact_keys(
        payload["selection"],
        {
            "count",
            "sealed_candidate_keys_sha256",
        },
        "selection",
    )
    require_equal(selection["count"], 214, "selection count")
    require_equal(
        selection["sealed_candidate_keys_sha256"],
        seal["selection"]["selected_candidate_keys_sha256"],
        "sealed selection digest",
    )
    protocol = require_exact_keys(
        payload["protocol"],
        {
            "guarantee",
            "not_guaranteed",
            "hidden_fields",
            "visible_fields",
            "root_seed_commitment_sha256",
            "persona_isolation",
            "private_packet_location",
            "raw_vote_freeze",
            "mapping_release",
            "release_order",
        },
        "protocol",
    )
    require_equal(
        protocol["guarantee"],
        "procedural-product-metadata-blindness",
        "protocol guarantee",
    )
    require_equal(
        protocol["not_guaranteed"],
        "identity hiding from a reviewer who searches remembered or public source",
        "protocol limitation",
    )
    require_equal(protocol["hidden_fields"], HIDDEN_FIELDS, "hidden fields")
    require_equal(
        protocol["visible_fields"], sorted(VISIBLE_CANDIDATE_KEYS), "visible fields"
    )
    require_hex(
        protocol["root_seed_commitment_sha256"], 64, "root seed commitment"
    )
    require_equal(
        protocol["mapping_release"],
        "after blind-ID arbitration is frozen",
        "mapping release",
    )
    require_equal(
        protocol["persona_isolation"],
        "independent derived seed, permutation, case IDs, and source IDs",
        "persona isolation",
    )
    require_equal(
        protocol["private_packet_location"],
        "outside Git and the project workspace",
        "private packet location",
    )
    require_equal(
        protocol["raw_vote_freeze"],
        "all three persona votes in one commit",
        "raw vote freeze",
    )
    require_equal(
        protocol["release_order"],
        [
            "private packet commitments",
            "three raw votes frozen atomically",
            "blind-ID arbitration frozen",
            "packet, seeds, and exact-key mapping revealed",
            "decisions and metrics",
        ],
        "release order",
    )
    packets = payload["packets"]
    if not isinstance(packets, list) or len(packets) != len(PERSONAS):
        raise ValueError("expected exactly three private packet commitments")
    for index, (record, persona) in enumerate(zip(packets, PERSONAS, strict=True)):
        row = require_exact_keys(
            record,
            {"persona", "schema", "sha256", "byte_length", "candidate_count"},
            f"packets[{index}]",
        )
        require_equal(row["persona"], persona, f"packets[{index}].persona")
        require_equal(row["schema"], PRIVATE_PACKET_SCHEMA, f"packets[{index}].schema")
        require_hex(row["sha256"], 64, f"packets[{index}].sha256")
        if not isinstance(row["byte_length"], int) or row["byte_length"] < 1:
            raise ValueError(f"packets[{index}].byte_length: invalid")
        require_equal(row["candidate_count"], 214, f"packets[{index}].candidate_count")
    provenance = require_exact_keys(
        payload["provenance"],
        {
            "command",
            "unseal_commit",
            "unseal_tree",
            "working_tree_status_before_unseal",
            "collector",
            "nose_binary",
            "nose_binary_sha256",
            "nose_version",
            "corpus",
            "base_labelset",
        },
        "provenance",
    )
    require_equal(provenance["command"], FREEZE_COMMAND, "unseal command")
    require_equal(provenance["working_tree_status_before_unseal"], "", "clean unseal")
    require_equal(provenance["corpus"], path_record(CORPUS), "corpus")
    require_equal(provenance["base_labelset"], path_record(BASE_LABELSET), "base labelset")
    require_equal(
        provenance["nose_binary_sha256"],
        seal["provenance"]["nose_binary_sha256"],
        "nose binary",
    )
    require_equal(
        provenance["nose_binary"], seal["provenance"]["nose_binary"], "nose path"
    )
    require_equal(
        provenance["nose_version"], seal["provenance"]["nose_version"], "nose version"
    )
    require_equal(
        git_text(["rev-parse", f"{provenance['unseal_commit']}^{{tree}}"]),
        provenance["unseal_tree"],
        "unseal tree",
    )
    subprocess.run(
        ["git", "merge-base", "--is-ancestor", provenance["unseal_commit"], "HEAD"],
        cwd=ROOT,
        check=True,
    )
    collector = require_exact_keys(
        provenance["collector"], {"path", "sha256"}, "collector"
    )
    collector_blob = git_bytes(
        ["show", f"{provenance['unseal_commit']}:{collector['path']}"]
    )
    require_equal(
        hashlib.sha256(collector_blob).hexdigest(), collector["sha256"], "collector blob"
    )


def validate_private_packet(payload: dict[str, Any], persona: str) -> None:
    require_exact_keys(
        payload,
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
        f"{persona} private packet",
    )
    require_equal(payload["schema"], PRIVATE_PACKET_SCHEMA, f"{persona} schema")
    require_equal(payload["issue"], 846, f"{persona} issue")
    require_equal(payload["split"], "heldout", f"{persona} split")
    require_equal(payload["persona"], persona, f"{persona} persona")
    require_equal(
        payload["judgment_status"],
        "procedurally-blind-unjudged",
        f"{persona} judgment status",
    )
    require_hex(payload["packet_nonce"], 64, f"{persona} packet nonce")
    require_equal(payload["rubric_sha256"], sha256_file(RUBRIC), f"{persona} rubric")
    reviewer_protocol = require_exact_keys(
        payload["reviewer_protocol"],
        {
            "guarantee",
            "not_guaranteed",
            "allowed_material",
            "prohibited_actions",
            "required_vote_attestation",
        },
        f"{persona} reviewer protocol",
    )
    require_equal(
        reviewer_protocol["guarantee"],
        "procedural-product-metadata-blindness",
        f"{persona} protocol guarantee",
    )
    require_equal(
        reviewer_protocol["not_guaranteed"],
        "identity hiding from a reviewer who searches remembered or public source",
        f"{persona} protocol limitation",
    )
    require_equal(
        reviewer_protocol["allowed_material"],
        ["assigned packet", "bound rubric"],
        f"{persona} allowed material",
    )
    require_equal(
        reviewer_protocol["prohibited_actions"],
        [
            "inspect Git, the corpus, repositories, or unassigned files",
            "use network access or search for source identity",
            "read another reviewer's packet or vote",
            "contact another reviewer",
        ],
        f"{persona} prohibited actions",
    )
    require_equal(
        reviewer_protocol["required_vote_attestation"],
        REVIEWER_ATTESTATION,
        f"{persona} vote attestation",
    )
    candidates = payload["candidates"]
    if not isinstance(candidates, list) or len(candidates) != 214:
        raise ValueError(f"{persona}: private candidate count mismatch")
    blind_ids: set[str] = set()
    for index, candidate in enumerate(candidates):
        row = require_exact_keys(
            candidate, VISIBLE_CANDIDATE_KEYS, f"{persona}.candidates[{index}]"
        )
        blind_id = row["blind_id"]
        if (
            not isinstance(blind_id, str)
            or not blind_id.startswith("case-")
            or len(blind_id) != 29
        ):
            raise ValueError(f"{persona}.candidates[{index}].blind_id: invalid")
        require_hex(
            blind_id.removeprefix("case-"),
            24,
            f"{persona}.candidates[{index}].blind_id",
        )
        blind_ids.add(blind_id)
        if not isinstance(row["language"], str) or not row["language"]:
            raise ValueError(f"{persona}.candidates[{index}].language: invalid")
        family = require_exact_keys(
            row["family"],
            VISIBLE_FAMILY_KEYS,
            f"{persona}.candidates[{index}].family",
        )
        members = family["members"]
        if (
            not isinstance(members, list)
            or not members
            or family["member_count"] != len(members)
        ):
            raise ValueError(f"{persona}.candidates[{index}].members: invalid")
        for member_index, member in enumerate(members):
            label = f"{persona}.candidates[{index}].members[{member_index}]"
            record = require_exact_keys(member, VISIBLE_MEMBER_KEYS, label)
            source_id = record["source_id"]
            if (
                not isinstance(source_id, str)
                or not source_id.startswith("source-")
                or len(source_id) != 31
            ):
                raise ValueError(f"{label}.source_id: invalid")
            require_hex(source_id.removeprefix("source-"), 24, f"{label}.source_id")
            for field in ("context_before", "source", "context_after"):
                if not isinstance(record[field], str):
                    raise ValueError(f"{label}.{field}: invalid")
    if len(blind_ids) != 214:
        raise ValueError(f"{persona}: blind IDs must be unique")


def read_root_seed() -> bytes:
    value = getpass.getpass("held-out blind root seed (64 hex): ").strip()
    try:
        seed = bytes.fromhex(value)
    except ValueError as error:
        raise ValueError("blind root seed must be hexadecimal") from error
    if len(seed) != 32:
        raise ValueError("blind root seed must contain exactly 32 bytes")
    return seed


def require_private_directory(path: Path, *, empty: bool) -> Path:
    resolved = path.expanduser().resolve()
    try:
        resolved.relative_to(ROOT)
    except ValueError:
        pass
    else:
        raise ValueError("private packet directory must be outside the project workspace")
    if not resolved.is_dir():
        raise ValueError(f"private packet directory does not exist: {resolved}")
    if empty and any(resolved.iterdir()):
        raise ValueError(f"private packet directory must be empty: {resolved}")
    return resolved


def write_exclusive(path: Path, content: bytes, mode: int) -> None:
    descriptor = os.open(path, os.O_WRONLY | os.O_CREAT | os.O_EXCL, mode)
    with os.fdopen(descriptor, "wb") as output:
        output.write(content)


def freeze(args: argparse.Namespace) -> None:
    if args.output.resolve() != COMMITMENT.resolve():
        raise ValueError(f"commitment output must be {relative(COMMITMENT)}")
    private_dir = require_private_directory(args.private_dir, empty=True)
    root_seed = read_root_seed()
    commitment, packets = collect_commitment(args, root_seed)
    for persona in PERSONAS:
        write_exclusive(private_dir / f"{persona}.json", packet_bytes(packets[persona]), 0o600)
    write_exclusive(args.output, packet_bytes(commitment), 0o644)
    print(
        f"unsealed commitments={len(read_json(SEAL)['candidate_commitments'])} "
        f"private packets={len(packets)} candidates={commitment['selection']['count']}"
    )


def validate(args: argparse.Namespace) -> None:
    payload = read_json(args.commitment)
    validate_commitment(payload)
    print(f"validated {args.commitment}")


def validate_private(args: argparse.Namespace) -> None:
    payload = read_json(args.commitment)
    validate_commitment(payload)
    private_dir = require_private_directory(args.private_dir, empty=False)
    root_seed = read_root_seed()
    require_equal(
        hashlib.sha256(root_seed).hexdigest(),
        payload["protocol"]["root_seed_commitment_sha256"],
        "root seed commitment",
    )
    _, selected = replay(args)
    records = {record["persona"]: record for record in payload["packets"]}
    for persona in PERSONAS:
        path = private_dir / f"{persona}.json"
        private = read_json(path)
        validate_private_packet(private, persona)
        expected = private_packet(persona, selected, root_seed, args.rubric)
        require_equal(private, expected, f"{persona} private packet replay")
        require_equal(
            sha256_file(path), records[persona]["sha256"], f"{persona} packet SHA-256"
        )
        require_equal(
            path.stat().st_size,
            records[persona]["byte_length"],
            f"{persona} packet byte length",
        )
    print(f"validated private packets in {private_dir}")


def self_test(args: argparse.Namespace) -> None:
    payload = read_json(args.commitment)
    validate_commitment(payload)
    mutations: list[dict[str, Any]] = []
    changed = copy.deepcopy(payload)
    changed["state"] = "votes-frozen"
    mutations.append(changed)
    changed = copy.deepcopy(payload)
    changed["packets"].reverse()
    mutations.append(changed)
    changed = copy.deepcopy(payload)
    changed["packets"][0]["sha256"] = "x" * 64
    mutations.append(changed)
    changed = copy.deepcopy(payload)
    changed["selection"]["count"] = 213
    mutations.append(changed)
    changed = copy.deepcopy(payload)
    changed["seal_receipt"]["commit"] = "0" * 40
    mutations.append(changed)
    for mutation in mutations:
        try:
            validate_commitment(mutation)
        except (ValueError, subprocess.CalledProcessError):
            continue
        raise AssertionError("invalid held-out commitment mutation was accepted")
    print("default-head held-out commitment self-test passed")


def add_live_arguments(parser: argparse.ArgumentParser) -> None:
    parser.add_argument("--nose", type=Path, default=OFFICIAL_NOSE)
    parser.add_argument("--repos-root", type=Path, default=REPOS_ROOT)
    parser.add_argument("--corpus", type=Path, default=CORPUS)
    parser.add_argument("--base-labelset", type=Path, default=BASE_LABELSET)
    parser.add_argument("--rubric", type=Path, default=RUBRIC)


def parser() -> argparse.ArgumentParser:
    root = argparse.ArgumentParser()
    commands = root.add_subparsers(dest="command", required=True)
    freeze_parser = commands.add_parser("freeze", allow_abbrev=False)
    add_live_arguments(freeze_parser)
    freeze_parser.add_argument("--private-dir", type=Path, required=True)
    freeze_parser.add_argument("--output", type=Path, default=COMMITMENT)
    freeze_parser.set_defaults(run=freeze)
    validate_parser = commands.add_parser("validate")
    validate_parser.add_argument(
        "commitment", type=Path, nargs="?", default=COMMITMENT
    )
    validate_parser.set_defaults(run=validate)
    private_parser = commands.add_parser("validate-private", allow_abbrev=False)
    add_live_arguments(private_parser)
    private_parser.add_argument("--private-dir", type=Path, required=True)
    private_parser.add_argument(
        "--commitment", type=Path, default=COMMITMENT
    )
    private_parser.set_defaults(run=validate_private)
    self_parser = commands.add_parser("self-test")
    self_parser.add_argument("--commitment", type=Path, default=COMMITMENT)
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
