#!/usr/bin/env python3
"""One-shot unsealing and blind projection for issue #846."""

from __future__ import annotations

import argparse
import copy
import hashlib
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
BLIND = ROOT / "bench/labels/default_head_heldout_blind_2026_07_14.v1.json"
CORPUS = ROOT / "bench/goldens/corpus.json"
BASE_LABELSET = ROOT / "bench/labels/refactoring_families.v6.json"
RUBRIC = ROOT / "bench/labels/RUBRIC.md"
OFFICIAL_NOSE = (
    ROOT
    / "target/issue-839/official-v0.19.0/"
    "nose-cli-aarch64-apple-darwin/nose"
)
REPOS_ROOT = ROOT / "bench/repos"

SCHEMA = "nose.default_head_heldout_blind.v1"
FREEZE_COMMAND = "python3 bench/labels/default_head_heldout.py freeze"
SEAL_COMMIT = "f945053520506c92c0dc72fe09c7fdb685d29e77"
SEAL_TREE = "78797deeaaa2aef346bad051ef96f33424352364"
SEAL_SHA256 = "b99c396544848af84a522d5b023c0304bef835ac5dafc6dbc744c2aab6843004"
SEAL_PATH = SEAL.relative_to(ROOT).as_posix()
VISIBLE_CANDIDATE_KEYS = {
    "blind_id",
    "repo",
    "split",
    "language",
    "sealed_candidate_sha256",
    "raw_family_sha256",
    "family",
}
VISIBLE_FAMILY_KEYS = {
    "id",
    "members",
    "member_count",
    "scope",
    "surface",
    "witness",
    "extraction_shape",
    "value",
}
HIDDEN_FIELDS = [
    "candidate_key",
    "rank",
    "lane",
    "base_matched",
    "selection_reason",
    "selection_order",
    "matched_v6_family_id",
    "matched_v6_member_overlap",
]


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
        git_text(["rev-parse", f"{SEAL_COMMIT}^{{tree}}"]),
        SEAL_TREE,
        "seal tree",
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


def blind_family(family: dict[str, Any]) -> dict[str, Any]:
    return {key: family[key] for key in VISIBLE_FAMILY_KEYS}


def blind_candidate(
    candidate: dict[str, Any], commitment_row: dict[str, Any], index: int
) -> dict[str, Any]:
    return {
        "blind_id": f"heldout-{index:04d}",
        "repo": candidate["repo"],
        "split": candidate["split"],
        "language": candidate["language"],
        "sealed_candidate_sha256": commitment_row["candidate_sha256"],
        "raw_family_sha256": candidate["raw_family_sha256"],
        "family": blind_family(candidate["family"]),
    }


def source_inventory(candidates: list[dict[str, Any]]) -> dict[str, dict[str, Any]]:
    paths = {
        member["file"]
        for candidate in candidates
        for member in candidate["family"]["members"]
    }
    return {path: runway.source_file_record(path) for path in sorted(paths)}


def collect_blind(args: argparse.Namespace) -> dict[str, Any]:
    status = git_text(["status", "--short"])
    if status:
        raise ValueError("held-out unseal requires a clean working tree")
    command = shlex.join(["python3", *sys.argv])
    require_equal(command, FREEZE_COMMAND, "unseal command")
    seal = validate_seal_receipt(live_binary=args.nose)
    nose_command = Path(relative(args.nose))
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
        stdout, families, command = runway.query_default_runway_repo(nose_command, repo)
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
            "query_command": shlex.join(command),
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
    ordered_candidates = sorted(candidates, key=lambda row: (row["repo"], row["rank"]))
    commitments = [commitment(candidate) for candidate in ordered_candidates]
    require_equal(commitments, seal["candidate_commitments"], "candidate commitments")

    by_key = {candidate["candidate_key"]: candidate for candidate in candidates}
    commitment_by_key = {
        row["candidate_key"]: row for row in seal["candidate_commitments"]
    }
    selected = [by_key[key] for key in selected_keys]
    visible = [
        blind_candidate(candidate, commitment_by_key[candidate["candidate_key"]], index)
        for index, candidate in enumerate(selected, start=1)
    ]
    sources = source_inventory(selected)
    collector = Path(__file__).resolve()
    return {
        "schema": SCHEMA,
        "issue": 846,
        "split": "heldout",
        "judgment_status": "unsealed-blind-unjudged",
        "seal_receipt": seal_receipt(),
        "selection": {
            "count": len(selected_keys),
            "sealed_candidate_keys_sha256": seal["selection"][
                "selected_candidate_keys_sha256"
            ],
            "blind_ids_sha256": canonical_sha256(
                [candidate["blind_id"] for candidate in visible]
            ),
        },
        "rubric": path_record(args.rubric),
        "blinding": {
            "hidden_fields": HIDDEN_FIELDS,
            "visible_fields": sorted(VISIBLE_CANDIDATE_KEYS),
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
            "source_files_sha256": canonical_sha256(sources),
        },
        "source_files": sources,
        "candidates": visible,
    }


def reconstruct_candidate(
    visible: dict[str, Any], commitment_row: dict[str, Any]
) -> dict[str, Any]:
    family = {
        **visible["family"],
        "matched_v6_family_id": None,
        "matched_v6_member_overlap": 0,
    }
    return {
        "candidate_key": commitment_row["candidate_key"],
        "repo": commitment_row["repo"],
        "split": commitment_row["split"],
        "language": commitment_row["language"],
        "lane": commitment_row["lane"],
        "rank": commitment_row["rank"],
        "base_matched": commitment_row["base_matched"],
        "family": family,
        "raw_family_sha256": visible["raw_family_sha256"],
    }


def validate_payload(payload: dict[str, Any], *, live_sources: bool = False) -> None:
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
            "source_files",
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
    require_exact_keys(
        payload["selection"],
        {"count", "sealed_candidate_keys_sha256", "blind_ids_sha256"},
        "selection",
    )
    require_exact_keys(
        payload["blinding"], {"hidden_fields", "visible_fields"}, "blinding"
    )
    require_equal(payload["blinding"]["hidden_fields"], HIDDEN_FIELDS, "hidden fields")
    require_equal(
        payload["blinding"]["visible_fields"], sorted(VISIBLE_CANDIDATE_KEYS), "visible fields"
    )
    candidates = payload["candidates"]
    selected_keys = seal["selection"]["selected_candidate_keys"]
    require_equal(payload["selection"]["count"], len(selected_keys), "selection count")
    require_equal(
        payload["selection"]["sealed_candidate_keys_sha256"],
        seal["selection"]["selected_candidate_keys_sha256"],
        "selection digest",
    )
    if not isinstance(candidates, list) or len(candidates) != len(selected_keys):
        raise ValueError("blind candidate count mismatch")
    commitments = {
        row["candidate_key"]: row for row in seal["candidate_commitments"]
    }
    sources = payload["source_files"]
    if not isinstance(sources, dict):
        raise ValueError("source_files: expected an object")
    for path, record in sources.items():
        if (
            not isinstance(path, str)
            or not path.startswith("bench/repos/")
            or "/../" in f"/{path}/"
        ):
            raise ValueError("source_files: invalid path")
        source_record = require_exact_keys(record, {"bytes", "sha256"}, f"source_files[{path}]")
        if not isinstance(source_record["bytes"], int) or source_record["bytes"] < 0:
            raise ValueError(f"source_files[{path}].bytes: invalid")
        digest = source_record["sha256"]
        if (
            not isinstance(digest, str)
            or len(digest) != 64
            or any(character not in "0123456789abcdef" for character in digest)
        ):
            raise ValueError(f"source_files[{path}].sha256: invalid")
    seen_files: set[str] = set()
    blind_ids = []
    for index, (visible, key) in enumerate(zip(candidates, selected_keys, strict=True), start=1):
        row = require_exact_keys(visible, VISIBLE_CANDIDATE_KEYS, f"candidates[{index - 1}]")
        expected_id = f"heldout-{index:04d}"
        require_equal(row["blind_id"], expected_id, f"{expected_id}.blind_id")
        blind_ids.append(expected_id)
        for digest_field in ("sealed_candidate_sha256", "raw_family_sha256"):
            digest = row[digest_field]
            if (
                not isinstance(digest, str)
                or len(digest) != 64
                or any(character not in "0123456789abcdef" for character in digest)
            ):
                raise ValueError(f"{expected_id}.{digest_field}: invalid")
        family = require_exact_keys(row["family"], VISIBLE_FAMILY_KEYS, f"{expected_id}.family")
        commitment_row = commitments[key]
        for field in ("repo", "split", "language"):
            require_equal(row[field], commitment_row[field], f"{expected_id}.{field}")
        require_equal(
            row["sealed_candidate_sha256"],
            commitment_row["candidate_sha256"],
            f"{expected_id}.sealed digest",
        )
        if commitment_row["base_matched"] or commitment_row["selected"] is not True:
            raise ValueError(f"{expected_id}: selected candidate must be v6-unmatched")
        reconstructed = reconstruct_candidate(row, commitment_row)
        require_equal(
            canonical_sha256(runway.runway_candidate_content(reconstructed)),
            commitment_row["candidate_sha256"],
            f"{expected_id}.candidate commitment",
        )
        if family["member_count"] != len(family["members"]):
            raise ValueError(f"{expected_id}: member count mismatch")
        for member in family["members"]:
            path = member.get("file") if isinstance(member, dict) else None
            if not isinstance(path, str) or path not in sources:
                raise ValueError(f"{expected_id}: unbound member source")
            seen_files.add(path)
    require_equal(set(sources), seen_files, "source inventory")
    require_equal(
        payload["selection"]["blind_ids_sha256"], canonical_sha256(blind_ids), "blind IDs"
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
            "source_files_sha256",
        },
        "provenance",
    )
    require_equal(provenance["working_tree_status_before_unseal"], "", "clean unseal")
    require_equal(provenance["command"], FREEZE_COMMAND, "unseal command")
    require_equal(provenance["collector"], path_record(Path(__file__)), "collector")
    require_equal(provenance["corpus"], path_record(CORPUS), "corpus")
    require_equal(provenance["base_labelset"], path_record(BASE_LABELSET), "base labelset")
    require_equal(provenance["source_files_sha256"], canonical_sha256(sources), "sources")
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
    collector_blob = git_bytes(
        ["show", f"{provenance['unseal_commit']}:{provenance['collector']['path']}"]
    )
    require_equal(
        hashlib.sha256(collector_blob).hexdigest(),
        provenance["collector"]["sha256"],
        "collector blob",
    )
    require_equal(
        provenance["nose_binary_sha256"],
        seal["provenance"]["nose_binary_sha256"],
        "nose binary",
    )
    require_equal(
        provenance["nose_binary"], seal["provenance"]["nose_binary"], "nose path"
    )
    require_equal(
        provenance["nose_version"],
        seal["provenance"]["nose_version"],
        "nose version",
    )
    if live_sources:
        for path, record in sources.items():
            source = ROOT / path
            require_equal(source.stat().st_size, record["bytes"], f"{path} bytes")
            require_equal(sha256_file(source), record["sha256"], f"{path} sha256")


def freeze(args: argparse.Namespace) -> None:
    payload = collect_blind(args)
    args.output.write_text(
        json.dumps(payload, sort_keys=True, separators=(",", ":")) + "\n",
        encoding="utf-8",
    )
    print(
        f"unsealed commitments={len(read_json(SEAL)['candidate_commitments'])} "
        f"blind candidates={len(payload['candidates'])} sources={len(payload['source_files'])}"
    )


def validate(args: argparse.Namespace) -> None:
    validate_payload(read_json(args.blind), live_sources=args.live_sources)
    print(f"validated {args.blind}")


def self_test(args: argparse.Namespace) -> None:
    payload = read_json(args.blind)
    validate_payload(payload)
    mutations: list[dict[str, Any]] = []
    changed = copy.deepcopy(payload)
    changed["candidates"][0]["rank"] = 1
    mutations.append(changed)
    changed = copy.deepcopy(payload)
    changed["candidates"].reverse()
    mutations.append(changed)
    changed = copy.deepcopy(payload)
    changed["candidates"][0]["family"]["value"] = 999
    mutations.append(changed)
    changed = copy.deepcopy(payload)
    changed["source_files"].pop(next(iter(changed["source_files"])))
    changed["provenance"]["source_files_sha256"] = canonical_sha256(
        changed["source_files"]
    )
    mutations.append(changed)
    changed = copy.deepcopy(payload)
    changed["seal_receipt"]["commit"] = "0" * 40
    mutations.append(changed)
    changed = copy.deepcopy(payload)
    changed["provenance"]["unseal_commit"] = SEAL_COMMIT
    mutations.append(changed)
    for mutation in mutations:
        try:
            validate_payload(mutation)
        except (ValueError, subprocess.CalledProcessError):
            continue
        raise AssertionError("invalid held-out blind mutation was accepted")
    print("default-head held-out blind self-test passed")


def parser() -> argparse.ArgumentParser:
    root = argparse.ArgumentParser()
    commands = root.add_subparsers(dest="command", required=True)
    freeze_parser = commands.add_parser("freeze", allow_abbrev=False)
    freeze_parser.add_argument("--nose", type=Path, default=OFFICIAL_NOSE)
    freeze_parser.add_argument("--repos-root", type=Path, default=REPOS_ROOT)
    freeze_parser.add_argument("--corpus", type=Path, default=CORPUS)
    freeze_parser.add_argument("--base-labelset", type=Path, default=BASE_LABELSET)
    freeze_parser.add_argument("--rubric", type=Path, default=RUBRIC)
    freeze_parser.add_argument("--output", type=Path, default=BLIND)
    freeze_parser.set_defaults(run=freeze)
    validate_parser = commands.add_parser("validate")
    validate_parser.add_argument("blind", type=Path, nargs="?", default=BLIND)
    validate_parser.add_argument("--live-sources", action="store_true")
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
