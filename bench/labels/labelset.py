#!/usr/bin/env python3
"""Load and validate frozen or composite refactoring-family labelsets."""

from __future__ import annotations

import argparse
from dataclasses import dataclass
import hashlib
import json
import tempfile
from pathlib import Path
from typing import Any


COMPOSITE_SCHEMA = "nose.refactoring_family_labelset.v1"
COMPONENT_SCHEMA = "nose.refactoring_family_labels.v1"
HELDOUT_SEAL_SCHEMA = "nose.default_head_heldout_seal.v1"
FLAT_SCHEMA_VERSION = "0.1.0"
PRECISION_METRIC = "precision_at_10"
RECALL_METRIC = "worthy_recall"
VOTE_NAMES = ("pragmatic", "dedupe", "skeptic")
WORTHY_REASONS = {
    "extract-helper",
    "extract-base",
    "extract-data-table",
    "parameterize",
}
NOT_WORTHY_REASONS = {
    "parallel-by-design",
    "coincidental-shape",
    "type-def",
    "generated",
    "trivial",
}
REASONS = WORTHY_REASONS | NOT_WORTHY_REASONS


@dataclass(frozen=True)
class LoadedLabelset:
    path: Path
    version: str
    families: list[dict[str, Any]]
    inputs: list[dict[str, str]]


def sha256_file(path: Path) -> str:
    digest = hashlib.sha256()
    with path.open("rb") as handle:
        for chunk in iter(lambda: handle.read(1024 * 1024), b""):
            digest.update(chunk)
    return digest.hexdigest()


def canonical_sha256(value: object) -> str:
    encoded = json.dumps(value, sort_keys=True, separators=(",", ":")).encode()
    return hashlib.sha256(encoded).hexdigest()


def require_exact_keys(value: object, expected: set[str], label: str) -> dict[str, Any]:
    if not isinstance(value, dict):
        raise ValueError(f"{label}: expected an object")
    actual = set(value)
    if actual != expected:
        raise ValueError(
            f"{label}: fields differ; missing={sorted(expected - actual)}, "
            f"unknown={sorted(actual - expected)}"
        )
    return value


def validate_heldout_seal_shape(seal: object, label: str) -> None:
    """Fail closed on every field that a source-free held-out seal may contain."""
    payload = require_exact_keys(
        seal,
        {
            "schema",
            "split",
            "judgment_status",
            "query_schema_version",
            "provenance",
            "selection_contract",
            "selection",
            "pool",
            "repositories",
            "candidate_commitments",
            "commitment_sha256",
        },
        label,
    )
    if (
        payload["schema"] != HELDOUT_SEAL_SCHEMA
        or payload["split"] != "heldout"
        or payload["judgment_status"] != "sealed-unjudged"
    ):
        raise ValueError(f"{label}: held-out seal contract mismatch")
    if isinstance(payload["query_schema_version"], bool) or not isinstance(
        payload["query_schema_version"], int
    ):
        raise ValueError(f"{label}.query_schema_version: expected an integer")

    provenance = require_exact_keys(
        payload["provenance"],
        {
            "command",
            "git_sha",
            "working_tree_status_before_collection",
            "nose_binary",
            "nose_binary_sha256",
            "nose_version",
            "corpus_manifest",
            "corpus_manifest_sha256",
            "corpus_commit_digest",
            "base_labelset",
            "base_labelset_sha256",
            "base_labelset_version",
            "rubric",
            "rubric_sha256",
            "collection_sources",
        },
        f"{label}.provenance",
    )
    collection_sources = provenance["collection_sources"]
    if not isinstance(collection_sources, list) or not collection_sources:
        raise ValueError(f"{label}.provenance.collection_sources: expected a non-empty array")
    for index, record in enumerate(collection_sources):
        require_exact_keys(
            record,
            {"path", "sha256"},
            f"{label}.provenance.collection_sources[{index}]",
        )

    require_exact_keys(
        payload["selection_contract"],
        {"seed", "head_rule", "deep_rule", "heldout_policy"},
        f"{label}.selection_contract",
    )
    require_exact_keys(
        payload["selection"],
        {"selected_candidate_keys", "selected_candidate_keys_sha256"},
        f"{label}.selection",
    )
    require_exact_keys(
        payload["pool"],
        {
            "repositories",
            "default_head_positions",
            "base_matched_default_head",
            "unmatched_default_head",
            "rank_11_30_candidates",
            "rank_11_30_unmatched",
            "selected_unmatched_default_head",
            "selected_rank_11_30",
            "selected_count",
        },
        f"{label}.pool",
    )
    repositories = payload["repositories"]
    if not isinstance(repositories, dict) or not repositories:
        raise ValueError(f"{label}.repositories: expected a non-empty object")
    for repo, record in repositories.items():
        require_exact_keys(
            record,
            {
                "commit",
                "language",
                "split",
                "query_command",
                "query_stdout_sha256",
                "top_10_reported",
                "top_30_reported",
                "base_matched_top_10",
                "unmatched_top_10",
            },
            f"{label}.repositories.{repo}",
        )
    commitments = payload["candidate_commitments"]
    if not isinstance(commitments, list) or not commitments:
        raise ValueError(f"{label}.candidate_commitments: expected a non-empty array")
    for index, commitment in enumerate(commitments):
        require_exact_keys(
            commitment,
            {
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
            },
            f"{label}.candidate_commitments[{index}]",
        )
    commitment = payload["commitment_sha256"]
    content = dict(payload)
    del content["commitment_sha256"]
    if commitment != canonical_sha256(content):
        raise ValueError(f"{label}: commitment mismatch")


def load_object(path: Path, label: str) -> dict[str, Any]:
    try:
        value = json.loads(path.read_text(encoding="utf-8"))
    except (OSError, json.JSONDecodeError) as error:
        raise ValueError(f"{label}: cannot read {path}: {error}") from error
    if not isinstance(value, dict):
        raise ValueError(f"{label}: top-level value must be an object")
    return value


def resolve_checked_file(parent: Path, record: object, label: str) -> Path:
    if not isinstance(record, dict):
        raise ValueError(f"{label}: expected a file record")
    raw_path = record.get("path")
    expected_hash = record.get("sha256")
    if not isinstance(raw_path, str) or not raw_path:
        raise ValueError(f"{label}.path: expected a non-empty string")
    if not isinstance(expected_hash, str) or len(expected_hash) != 64:
        raise ValueError(f"{label}.sha256: expected a SHA-256 hex digest")
    path = (parent / raw_path).resolve()
    actual_hash = sha256_file(path)
    if actual_hash != expected_hash:
        raise ValueError(f"{label}: hash mismatch for {path}: {actual_hash} != {expected_hash}")
    return path


def validate_vote(vote: object, label: str) -> tuple[bool, str]:
    if not isinstance(vote, dict):
        raise ValueError(f"{label}: expected an object")
    worthy = vote.get("worthy")
    reason = vote.get("reason")
    if not isinstance(worthy, bool):
        raise ValueError(f"{label}.worthy: expected a boolean")
    if reason not in REASONS:
        raise ValueError(f"{label}.reason: unsupported reason {reason!r}")
    if worthy != (reason in WORTHY_REASONS):
        raise ValueError(f"{label}: worthiness and reason disagree")
    rationale = vote.get("rationale")
    if rationale is not None and (not isinstance(rationale, str) or not rationale.strip()):
        raise ValueError(f"{label}.rationale: expected a non-empty string when present")
    return worthy, reason


def validate_component_family(family: object, split: str, label: str) -> None:
    if not isinstance(family, dict):
        raise ValueError(f"{label}: expected an object")
    required_strings = (
        "family_id",
        "candidate_key",
        "candidate_sha256",
        "repo",
        "language",
        "scope",
        "channel",
        "confidence",
        "labeler",
        "note",
    )
    for key in required_strings:
        if not isinstance(family.get(key), str) or not family[key]:
            raise ValueError(f"{label}.{key}: expected a non-empty string")
    if family.get("split") != split:
        raise ValueError(f"{label}.split: expected {split!r}")
    if family["channel"] != "current-default":
        raise ValueError(f"{label}.channel: expected 'current-default'")
    if family.get("metric_eligibility") != [PRECISION_METRIC]:
        raise ValueError(f"{label}.metric_eligibility: top-10 refresh labels are precision-only")
    if family["confidence"] not in {"high", "medium", "low"}:
        raise ValueError(f"{label}.confidence: unsupported value")
    final_worthy = family.get("worthy")
    final_reason = family.get("reason")
    if not isinstance(final_worthy, bool) or final_reason not in REASONS:
        raise ValueError(f"{label}: invalid final judgment")
    if final_worthy != (final_reason in WORTHY_REASONS):
        raise ValueError(f"{label}: final worthiness and reason disagree")
    members = family.get("members")
    if not isinstance(members, list) or len(members) < 2:
        raise ValueError(f"{label}.members: expected at least two members")
    for member_index, member in enumerate(members):
        member_label = f"{label}.members[{member_index}]"
        if not isinstance(member, dict):
            raise ValueError(f"{member_label}: expected an object")
        if not isinstance(member.get("file"), str) or not member["file"]:
            raise ValueError(f"{member_label}.file: expected a non-empty string")
        for key in ("start_line", "end_line"):
            if isinstance(member.get(key), bool) or not isinstance(member.get(key), int):
                raise ValueError(f"{member_label}.{key}: expected an integer")
        if member["start_line"] <= 0 or member["end_line"] < member["start_line"]:
            raise ValueError(f"{member_label}: invalid line interval")

    votes = family.get("votes")
    if not isinstance(votes, dict) or set(votes) != set(VOTE_NAMES):
        raise ValueError(f"{label}.votes: expected exactly {VOTE_NAMES}")
    decisions = [validate_vote(votes[name], f"{label}.votes.{name}") for name in VOTE_NAMES]
    unanimous = len(set(decisions)) == 1
    arbiter = family.get("arbiter")
    if unanimous:
        if family["labeler"] != "panel" or arbiter is not None:
            raise ValueError(f"{label}: unanimous panels must be final with no arbiter")
        if decisions[0] != (final_worthy, final_reason):
            raise ValueError(f"{label}: final judgment differs from unanimous panel")
    else:
        if family["labeler"] != "llm-arbiter":
            raise ValueError(f"{label}: split panels require labeler 'llm-arbiter'")
        arbiter_decision = validate_vote(arbiter, f"{label}.arbiter")
        if arbiter_decision != (final_worthy, final_reason):
            raise ValueError(f"{label}: final judgment differs from arbiter")


def load_flat(path: Path, payload: dict[str, Any], label: str) -> list[dict[str, Any]]:
    if payload.get("schema_version") != FLAT_SCHEMA_VERSION:
        raise ValueError(f"{label}: unsupported flat schema version")
    families = payload.get("families")
    if not isinstance(families, list):
        raise ValueError(f"{label}.families: expected an array")
    return families


def composite_version(version: str) -> int:
    if not version.startswith("v") or not version[1:].isdigit():
        raise ValueError(f"invalid loaded labelset version: {version!r}")
    return int(version[1:])


def _load_labelset(path: Path, stack: tuple[Path, ...]) -> LoadedLabelset:
    path = path.resolve()
    if path in stack:
        chain = " -> ".join(row.name for row in (*stack, path))
        raise ValueError(f"labelset base cycle: {chain}")
    payload = load_object(path, "labelset")
    if payload.get("schema") != COMPOSITE_SCHEMA:
        families = load_flat(path, payload, "labelset")
        return LoadedLabelset(
            path=path,
            version="v5",
            families=families,
            inputs=[{"path": path.as_posix(), "sha256": sha256_file(path)}],
        )

    version = payload.get("version")
    if not isinstance(version, int) or isinstance(version, bool) or version not in {6, 7}:
        raise ValueError("labelset.version: expected 6 or 7")
    parent = path.parent
    base_path = resolve_checked_file(parent, payload.get("base"), "labelset.base")
    base = _load_labelset(base_path, (*stack, path))
    base_version = composite_version(base.version)
    if base_version >= version:
        raise ValueError(
            f"labelset.base: expected a version older than v{version}, got {base.version}"
        )
    if version == 6 and base.version != "v5":
        raise ValueError("labelset.base: v6 must extend the frozen v5 flat labelset")
    if version == 7 and base.version != "v6":
        raise ValueError("labelset.base: v7 must extend the frozen v6 composite")
    families = list(base.families)
    inputs = list(base.inputs)
    if base_version >= 6:
        inputs.append({"path": base_path.as_posix(), "sha256": sha256_file(base_path)})
    components = payload.get("components")
    if not isinstance(components, list) or not components:
        raise ValueError("labelset.components: expected a non-empty array")
    seen_splits: set[str] = set()
    for index, record in enumerate(components):
        record_label = f"labelset.components[{index}]"
        if not isinstance(record, dict) or record.get("split") not in {"dev", "heldout"}:
            raise ValueError(f"{record_label}.split: expected dev or heldout")
        if version == 7 and record.get("kind") != "precision-overlay":
            raise ValueError(f"{record_label}.kind: expected precision-overlay")
        split = record["split"]
        if split in seen_splits:
            raise ValueError(f"{record_label}: duplicate split {split}")
        seen_splits.add(split)
        component_path = resolve_checked_file(parent, record, record_label)
        component = load_object(component_path, record_label)
        if component.get("schema") != COMPONENT_SCHEMA or component.get("split") != split:
            raise ValueError(f"{record_label}: component schema/split mismatch")
        resolve_checked_file(component_path.parent, component.get("source_artifact"), f"{record_label}.source_artifact")
        resolve_checked_file(component_path.parent, component.get("rubric"), f"{record_label}.rubric")
        resolve_checked_file(component_path.parent, component.get("decision_input"), f"{record_label}.decision_input")
        component_families = component.get("families")
        if not isinstance(component_families, list) or not component_families:
            raise ValueError(f"{record_label}.families: expected a non-empty array")
        for family_index, family in enumerate(component_families):
            validate_component_family(family, split, f"{record_label}.families[{family_index}]")
        families.extend(component_families)
        inputs.append({"path": component_path.as_posix(), "sha256": sha256_file(component_path)})
    if version == 6 and seen_splits != {"dev", "heldout"}:
        raise ValueError("labelset.components: both dev and heldout components are required")
    if version == 7 and seen_splits != {"dev"}:
        raise ValueError("labelset.components: v7 requires exactly one dev precision overlay")

    seals = payload.get("seals", [])
    if version == 6 and seals:
        raise ValueError("labelset.seals: v6 does not support held-out seals")
    if version == 7:
        if not isinstance(seals, list) or len(seals) != 1:
            raise ValueError("labelset.seals: v7 requires exactly one held-out seal")
        seal_record = seals[0]
        if not isinstance(seal_record, dict) or seal_record.get("split") != "heldout":
            raise ValueError("labelset.seals[0].split: expected heldout")
        seal_path = resolve_checked_file(parent, seal_record, "labelset.seals[0]")
        seal = load_object(seal_path, "labelset.seals[0]")
        validate_heldout_seal_shape(seal, "labelset.seals[0]")
        inputs.append({"path": seal_path.as_posix(), "sha256": sha256_file(seal_path)})

    identities: set[tuple[str, str]] = set()
    for index, family in enumerate(families):
        identity = (family.get("repo"), family.get("family_id"))
        if not all(isinstance(value, str) and value for value in identity):
            raise ValueError(f"labelset family {index}: missing repo/family_id")
        if identity in identities:
            raise ValueError(f"labelset family {index}: duplicate identity {identity}")
        identities.add(identity)
    return LoadedLabelset(path=path, version=f"v{version}", families=families, inputs=inputs)


def load_labelset(path: Path) -> LoadedLabelset:
    return _load_labelset(path, ())


def metric_eligible(family: dict[str, Any], metric: str) -> bool:
    eligibility = family.get("metric_eligibility")
    if eligibility is None:
        return True
    return isinstance(eligibility, list) and metric in eligibility


def run_self_test() -> None:
    with tempfile.TemporaryDirectory(prefix="nose-labelset-self-test-") as directory:
        root = Path(directory)
        base = root / "base.json"
        base.write_text(
            json.dumps(
                {
                    "schema_version": FLAT_SCHEMA_VERSION,
                    "families": [{"repo": "base", "family_id": "base-family", "members": []}],
                }
            )
        )
        source = root / "candidates.json"
        source.write_text("{}\n")
        rubric = root / "RUBRIC.md"
        rubric.write_text("rubric\n")

        def component(split: str, family_id: str) -> dict[str, Any]:
            vote = {
                "worthy": True,
                "reason": "extract-helper",
                "rationale": "The repeated computation has one coherent helper boundary.",
            }
            return {
                "schema": COMPONENT_SCHEMA,
                "split": split,
                "source_artifact": {"path": source.name, "sha256": sha256_file(source)},
                "rubric": {"path": rubric.name, "sha256": sha256_file(rubric)},
                "decision_input": {"path": source.name, "sha256": sha256_file(source)},
                "families": [
                    {
                        "family_id": family_id,
                        "candidate_key": f"repo-{split}:{family_id}",
                        "candidate_sha256": "a" * 64,
                        "repo": f"repo-{split}",
                        "split": split,
                        "language": "Test",
                        "scope": "prod",
                        "channel": "current-default",
                        "metric_eligibility": [PRECISION_METRIC],
                        "members": [
                            {"file": "a", "start_line": 1, "end_line": 2},
                            {"file": "b", "start_line": 1, "end_line": 2},
                        ],
                        "worthy": True,
                        "reason": "extract-helper",
                        "confidence": "high",
                        "labeler": "panel",
                        "votes": {name: dict(vote) for name in VOTE_NAMES},
                        "arbiter": None,
                        "note": "Unanimous synthetic panel decision.",
                    }
                ],
            }

        component_records = []
        for split in ("dev", "heldout"):
            component_path = root / f"{split}.json"
            component_path.write_text(json.dumps(component(split, f"family-{split}")))
            component_records.append(
                {"split": split, "path": component_path.name, "sha256": sha256_file(component_path)}
            )
        manifest = root / "v6.json"
        manifest.write_text(
            json.dumps(
                {
                    "schema": COMPOSITE_SCHEMA,
                    "version": 6,
                    "base": {"path": base.name, "sha256": sha256_file(base)},
                    "components": component_records,
                }
            )
        )
        loaded = load_labelset(manifest)
        assert loaded.version == "v6"
        assert len(loaded.families) == 3
        assert metric_eligible(loaded.families[0], RECALL_METRIC)
        assert not metric_eligible(loaded.families[-1], RECALL_METRIC)

        v7_component_path = root / "dev-v7.json"
        v7_component_path.write_text(json.dumps(component("dev", "family-v7-dev")))
        seal_path = root / "heldout-seal.json"
        synthetic_seal = {
            "schema": HELDOUT_SEAL_SCHEMA,
            "split": "heldout",
            "judgment_status": "sealed-unjudged",
            "query_schema_version": 7,
            "provenance": {
                "command": "synthetic",
                "git_sha": "0" * 40,
                "working_tree_status_before_collection": "",
                "nose_binary": "nose",
                "nose_binary_sha256": "0" * 64,
                "nose_version": "nose test",
                "corpus_manifest": "corpus.json",
                "corpus_manifest_sha256": "0" * 64,
                "corpus_commit_digest": "0" * 64,
                "base_labelset": manifest.name,
                "base_labelset_sha256": sha256_file(manifest),
                "base_labelset_version": "v6",
                "rubric": rubric.name,
                "rubric_sha256": sha256_file(rubric),
                "collection_sources": [{"path": "collector.py", "sha256": "0" * 64}],
            },
            "selection_contract": {
                "seed": "synthetic",
                "head_rule": "synthetic",
                "deep_rule": "synthetic",
                "heldout_policy": "selection commitments only",
            },
            "selection": {
                "selected_candidate_keys": ["heldout:candidate"],
                "selected_candidate_keys_sha256": "0" * 64,
            },
            "pool": {
                "repositories": 1,
                "default_head_positions": 1,
                "base_matched_default_head": 0,
                "unmatched_default_head": 1,
                "rank_11_30_candidates": 0,
                "rank_11_30_unmatched": 0,
                "selected_unmatched_default_head": 1,
                "selected_rank_11_30": 0,
                "selected_count": 1,
            },
            "repositories": {
                "heldout": {
                    "commit": "0" * 40,
                    "language": "Test",
                    "split": "heldout",
                    "query_command": "nose query heldout",
                    "query_stdout_sha256": "0" * 64,
                    "top_10_reported": 1,
                    "top_30_reported": 1,
                    "base_matched_top_10": 0,
                    "unmatched_top_10": 1,
                }
            },
            "candidate_commitments": [
                {
                    "candidate_key": "heldout:candidate",
                    "candidate_sha256": "0" * 64,
                    "repo": "heldout",
                    "split": "heldout",
                    "language": "Test",
                    "lane": "synthetic",
                    "rank": 1,
                    "base_matched": False,
                    "selected": True,
                    "selection_reason": "unmatched-default-head",
                    "selection_order": 1,
                }
            ],
        }
        synthetic_seal["commitment_sha256"] = canonical_sha256(synthetic_seal)
        seal_path.write_text(
            json.dumps(synthetic_seal)
        )
        v7_manifest = root / "v7.json"
        v7_manifest.write_text(
            json.dumps(
                {
                    "schema": COMPOSITE_SCHEMA,
                    "version": 7,
                    "base": {"path": manifest.name, "sha256": sha256_file(manifest)},
                    "components": [
                        {
                            "kind": "precision-overlay",
                            "split": "dev",
                            "path": v7_component_path.name,
                            "sha256": sha256_file(v7_component_path),
                        }
                    ],
                    "seals": [
                        {
                            "split": "heldout",
                            "path": seal_path.name,
                            "sha256": sha256_file(seal_path),
                        }
                    ],
                }
            )
        )
        loaded_v7 = load_labelset(v7_manifest)
        assert loaded_v7.version == "v7"
        assert len(loaded_v7.families) == 4
        assert len(loaded_v7.inputs) == 6
        assert not metric_eligible(loaded_v7.families[-1], RECALL_METRIC)

        leaked_seal = dict(synthetic_seal)
        leaked_seal["source_excerpt"] = "held-out source"
        leaked_seal["commitment_sha256"] = canonical_sha256(
            {key: value for key, value in leaked_seal.items() if key != "commitment_sha256"}
        )
        try:
            validate_heldout_seal_shape(leaked_seal, "synthetic leaked seal")
        except ValueError as error:
            assert "unknown=['source_excerpt']" in str(error)
        else:
            raise AssertionError("unknown held-out seal fields must fail closed")

        heldout_v7_component = root / "heldout-v7.json"
        heldout_v7_component.write_text(
            json.dumps(component("heldout", "family-v7-heldout"))
        )
        leaked_manifest = root / "v7-with-heldout-judgments.json"
        leaked_manifest.write_text(
            json.dumps(
                {
                    "schema": COMPOSITE_SCHEMA,
                    "version": 7,
                    "base": {"path": manifest.name, "sha256": sha256_file(manifest)},
                    "components": [
                        {
                            "kind": "precision-overlay",
                            "split": "dev",
                            "path": v7_component_path.name,
                            "sha256": sha256_file(v7_component_path),
                        },
                        {
                            "kind": "precision-overlay",
                            "split": "heldout",
                            "path": heldout_v7_component.name,
                            "sha256": sha256_file(heldout_v7_component),
                        },
                    ],
                    "seals": [
                        {
                            "split": "heldout",
                            "path": seal_path.name,
                            "sha256": sha256_file(seal_path),
                        }
                    ],
                }
            )
        )
        try:
            load_labelset(leaked_manifest)
        except ValueError as error:
            assert "exactly one dev precision overlay" in str(error)
        else:
            raise AssertionError("v7 held-out judgment components must fail closed")

        component_records[0]["sha256"] = "0" * 64
        manifest.write_text(
            json.dumps(
                {
                    "schema": COMPOSITE_SCHEMA,
                    "version": 6,
                    "base": {"path": base.name, "sha256": sha256_file(base)},
                    "components": component_records,
                }
            )
        )
        try:
            load_labelset(manifest)
        except ValueError as error:
            assert "hash mismatch" in str(error)
        else:
            raise AssertionError("component hash drift must fail closed")
    print("labelset loader self-test passed")


def main() -> None:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("labelset", nargs="?", type=Path)
    parser.add_argument("--self-test", action="store_true")
    args = parser.parse_args()
    if args.self_test:
        run_self_test()
        return
    if args.labelset is None:
        parser.error("labelset path is required")
    loaded = load_labelset(args.labelset)
    print(f"ok {loaded.version} labelset: {len(loaded.families)} families")


if __name__ == "__main__":
    main()
