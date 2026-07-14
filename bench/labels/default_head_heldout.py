#!/usr/bin/env python3
"""One-shot, secretly permuted held-out packet builder for issue #846."""

from __future__ import annotations

import argparse
import copy
import getpass
import hashlib
import hmac
import json
import shlex
import subprocess
import sys
from collections import defaultdict
from pathlib import Path
from typing import Any

import label_refresh as runway


ROOT = Path(__file__).resolve().parents[2]
SEAL = ROOT / "bench/labels/default_head_label_runway_2026_07_13.heldout.seal.v1.json"
BLIND = ROOT / "bench/labels/default_head_heldout_blind_2026_07_14.v2.json"
CORPUS = ROOT / "bench/goldens/corpus.json"
BASE_LABELSET = ROOT / "bench/labels/refactoring_families.v6.json"
RUBRIC = ROOT / "bench/labels/RUBRIC.md"
OFFICIAL_NOSE = (
    ROOT
    / "target/issue-839/official-v0.19.0/"
    "nose-cli-aarch64-apple-darwin/nose"
)
REPOS_ROOT = ROOT / "bench/repos"

SCHEMA = "nose.default_head_heldout_blind.v2"
FREEZE_COMMAND = "python3 bench/labels/default_head_heldout.py freeze"
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
VISIBLE_CANDIDATE_KEYS = {"blind_id", "language", "family"}
VISIBLE_FAMILY_KEYS = {
    "members",
    "member_count",
    "scope",
    "surface",
    "witness",
    "extraction_shape",
    "value",
}
VISIBLE_MEMBER_KEYS = {
    "source_id",
    "name",
    "span_lines",
    "context_before",
    "source",
    "context_after",
    "excerpt_sha256",
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
        "name": member.get("name"),
        "span_lines": end - start + 1,
        "context_before": bounded_text(
            "\n".join(lines[max(0, start - 1 - CONTEXT_LINES) : start - 1])
        ),
        "source": bounded_text("\n".join(lines[start - 1 : end])),
        "context_after": bounded_text("\n".join(lines[end : end + CONTEXT_LINES])),
    }
    record["excerpt_sha256"] = canonical_sha256(record)
    return record


def blind_candidate(candidate: dict[str, Any], seed: bytes) -> dict[str, Any]:
    family = candidate["family"]
    return {
        "blind_id": f"case-{hmac_hex(seed, 'blind-id', candidate['candidate_key'])[:24]}",
        "language": candidate["language"],
        "family": {
            "members": [opaque_member(member, seed) for member in family["members"]],
            "member_count": family["member_count"],
            "scope": family["scope"],
            "surface": family["surface"],
            "witness": family["witness"],
            "extraction_shape": family["extraction_shape"],
            "value": family["value"],
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


def source_ids(candidates: list[dict[str, Any]]) -> list[str]:
    return sorted(
        {
            member["source_id"]
            for candidate in candidates
            for member in candidate["family"]["members"]
        }
    )


def collect_blind(args: argparse.Namespace, seed: bytes) -> dict[str, Any]:
    status = git_text(["status", "--short"])
    if status:
        raise ValueError("held-out unseal requires a clean working tree")
    command = shlex.join(["python3", *sys.argv])
    require_equal(command, FREEZE_COMMAND, "unseal command")
    if len(seed) != 32:
        raise ValueError("blind seed must contain exactly 32 bytes")
    seal, selected = replay(args)
    visible = blind_projection(selected, seed)
    ids = source_ids(visible)
    collector = Path(__file__).resolve()
    return {
        "schema": SCHEMA,
        "issue": 846,
        "split": "heldout",
        "judgment_status": "unsealed-blind-unjudged",
        "seal_receipt": seal_receipt(),
        "selection": {
            "count": len(selected),
            "sealed_candidate_keys_sha256": seal["selection"][
                "selected_candidate_keys_sha256"
            ],
            "blind_ids_sha256": canonical_sha256(
                [candidate["blind_id"] for candidate in visible]
            ),
            "source_count": len(ids),
            "source_ids_sha256": canonical_sha256(ids),
        },
        "rubric": path_record(args.rubric),
        "blinding": {
            "hidden_fields": HIDDEN_FIELDS,
            "visible_fields": sorted(VISIBLE_CANDIDATE_KEYS),
            "seed_commitment_sha256": hashlib.sha256(seed).hexdigest(),
            "permutation": "HMAC-SHA256(secret, order\\0candidate_key)",
            "blind_id": "case- + first 24 hex of HMAC-SHA256(secret, blind-id\\0candidate_key)",
            "source_id": "source- + first 24 hex of HMAC-SHA256(secret, source\\0path)",
            "mapping_release": "after all three raw vote artifacts are frozen",
        },
        "provenance": {
            "command": command,
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
        "candidates": visible,
    }


def validate_public(payload: dict[str, Any]) -> None:
    require_exact_keys(
        payload,
        {
            "schema",
            "issue",
            "split",
            "judgment_status",
            "seal_receipt",
            "selection",
            "rubric",
            "blinding",
            "provenance",
            "candidates",
        },
        "blind artifact",
    )
    require_equal(payload["schema"], SCHEMA, "schema")
    require_equal(payload["issue"], 846, "issue")
    require_equal(payload["split"], "heldout", "split")
    require_equal(
        payload["judgment_status"], "unsealed-blind-unjudged", "judgment status"
    )
    seal = validate_seal_receipt()
    require_equal(payload["seal_receipt"], seal_receipt(), "seal receipt")
    require_equal(payload["rubric"], path_record(RUBRIC), "rubric")
    selection = require_exact_keys(
        payload["selection"],
        {
            "count",
            "sealed_candidate_keys_sha256",
            "blind_ids_sha256",
            "source_count",
            "source_ids_sha256",
        },
        "selection",
    )
    require_equal(selection["count"], 214, "selection count")
    require_equal(
        selection["sealed_candidate_keys_sha256"],
        seal["selection"]["selected_candidate_keys_sha256"],
        "sealed selection digest",
    )
    blinding = require_exact_keys(
        payload["blinding"],
        {
            "hidden_fields",
            "visible_fields",
            "seed_commitment_sha256",
            "permutation",
            "blind_id",
            "source_id",
            "mapping_release",
        },
        "blinding",
    )
    require_equal(blinding["hidden_fields"], HIDDEN_FIELDS, "hidden fields")
    require_equal(
        blinding["visible_fields"], sorted(VISIBLE_CANDIDATE_KEYS), "visible fields"
    )
    require_hex(blinding["seed_commitment_sha256"], 64, "seed commitment")
    require_equal(
        blinding["mapping_release"],
        "after all three raw vote artifacts are frozen",
        "mapping release",
    )
    candidates = payload["candidates"]
    if not isinstance(candidates, list) or len(candidates) != selection["count"]:
        raise ValueError("blind candidate count mismatch")
    blind_ids: list[str] = []
    observed_sources: set[str] = set()
    for index, candidate in enumerate(candidates):
        row = require_exact_keys(candidate, VISIBLE_CANDIDATE_KEYS, f"candidates[{index}]")
        blind_id = row["blind_id"]
        if (
            not isinstance(blind_id, str)
            or not blind_id.startswith("case-")
            or len(blind_id) != 29
        ):
            raise ValueError(f"candidates[{index}].blind_id: invalid")
        require_hex(blind_id.removeprefix("case-"), 24, f"candidates[{index}].blind_id")
        blind_ids.append(blind_id)
        if not isinstance(row["language"], str) or not row["language"]:
            raise ValueError(f"candidates[{index}].language: invalid")
        family = require_exact_keys(
            row["family"], VISIBLE_FAMILY_KEYS, f"candidates[{index}].family"
        )
        members = family["members"]
        if (
            not isinstance(members, list)
            or not members
            or family["member_count"] != len(members)
        ):
            raise ValueError(f"candidates[{index}].members: invalid")
        for member_index, member in enumerate(members):
            label = f"candidates[{index}].members[{member_index}]"
            record = require_exact_keys(member, VISIBLE_MEMBER_KEYS, label)
            source_id = record["source_id"]
            if (
                not isinstance(source_id, str)
                or not source_id.startswith("source-")
                or len(source_id) != 31
            ):
                raise ValueError(f"{label}.source_id: invalid")
            require_hex(source_id.removeprefix("source-"), 24, f"{label}.source_id")
            observed_sources.add(source_id)
            if not isinstance(record["span_lines"], int) or record["span_lines"] < 1:
                raise ValueError(f"{label}.span_lines: invalid")
            for field in ("context_before", "source", "context_after"):
                if not isinstance(record[field], str):
                    raise ValueError(f"{label}.{field}: invalid")
            digest_payload = {
                key: value
                for key, value in record.items()
                if key != "excerpt_sha256"
            }
            require_equal(
                record["excerpt_sha256"], canonical_sha256(digest_payload), f"{label}.digest"
            )
    if len(blind_ids) != len(set(blind_ids)):
        raise ValueError("blind IDs must be unique")
    require_equal(selection["blind_ids_sha256"], canonical_sha256(blind_ids), "blind IDs")
    require_equal(selection["source_count"], len(observed_sources), "source count")
    require_equal(
        selection["source_ids_sha256"],
        canonical_sha256(sorted(observed_sources)),
        "source IDs",
    )
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


def validate_revealed(
    payload: dict[str, Any], args: argparse.Namespace, seed: bytes
) -> None:
    validate_public(payload)
    if len(seed) != 32:
        raise ValueError("blind seed must contain exactly 32 bytes")
    require_equal(
        hashlib.sha256(seed).hexdigest(),
        payload["blinding"]["seed_commitment_sha256"],
        "blind seed commitment",
    )
    _, selected = replay(args)
    require_equal(payload["candidates"], blind_projection(selected, seed), "blind projection")


def read_seed() -> bytes:
    value = getpass.getpass("held-out blind seed (64 hex): ").strip()
    try:
        seed = bytes.fromhex(value)
    except ValueError as error:
        raise ValueError("blind seed must be hexadecimal") from error
    if len(seed) != 32:
        raise ValueError("blind seed must contain exactly 32 bytes")
    return seed


def freeze(args: argparse.Namespace) -> None:
    seed = read_seed()
    payload = collect_blind(args, seed)
    args.output.write_text(
        json.dumps(payload, sort_keys=True, separators=(",", ":")) + "\n",
        encoding="utf-8",
    )
    print(
        f"unsealed commitments={len(read_json(SEAL)['candidate_commitments'])} "
        f"blind candidates={len(payload['candidates'])} "
        f"opaque sources={payload['selection']['source_count']}"
    )


def validate(args: argparse.Namespace) -> None:
    payload = read_json(args.blind)
    validate_public(payload)
    if args.reveal:
        validate_revealed(payload, args, read_seed())
    print(f"validated {args.blind}")


def self_test(args: argparse.Namespace) -> None:
    payload = read_json(args.blind)
    validate_public(payload)
    mutations: list[dict[str, Any]] = []
    changed = copy.deepcopy(payload)
    changed["candidates"][0]["rank"] = 1
    mutations.append(changed)
    changed = copy.deepcopy(payload)
    changed["candidates"].reverse()
    mutations.append(changed)
    changed = copy.deepcopy(payload)
    changed["candidates"][0]["family"]["members"][0]["source"] += "tamper"
    mutations.append(changed)
    changed = copy.deepcopy(payload)
    changed["candidates"][0]["family"]["members"][0]["source_id"] = "source-" + "0" * 24
    mutations.append(changed)
    changed = copy.deepcopy(payload)
    changed["seal_receipt"]["commit"] = "0" * 40
    mutations.append(changed)
    for mutation in mutations:
        try:
            validate_public(mutation)
        except (ValueError, subprocess.CalledProcessError):
            continue
        raise AssertionError("invalid held-out blind mutation was accepted")
    print("default-head held-out public-packet self-test passed")


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
    freeze_parser.add_argument("--output", type=Path, default=BLIND)
    freeze_parser.set_defaults(run=freeze)
    validate_parser = commands.add_parser("validate")
    add_live_arguments(validate_parser)
    validate_parser.add_argument("blind", type=Path, nargs="?", default=BLIND)
    validate_parser.add_argument("--reveal", action="store_true")
    validate_parser.set_defaults(run=validate)
    self_parser = commands.add_parser("self-test")
    self_parser.add_argument("--blind", type=Path, default=BLIND)
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
