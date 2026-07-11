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
    rationale = vote.get("rationale")
    if not isinstance(worthy, bool):
        raise ValueError(f"{label}.worthy: expected a boolean")
    if reason not in REASONS:
        raise ValueError(f"{label}.reason: unsupported reason {reason!r}")
    if worthy != (reason in WORTHY_REASONS):
        raise ValueError(f"{label}: worthiness and reason disagree")
    if not isinstance(rationale, str) or not rationale.strip():
        raise ValueError(f"{label}.rationale: expected a non-empty string")
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


def load_labelset(path: Path) -> LoadedLabelset:
    path = path.resolve()
    payload = load_object(path, "labelset")
    if payload.get("schema") != COMPOSITE_SCHEMA:
        families = load_flat(path, payload, "labelset")
        return LoadedLabelset(
            path=path,
            version="v5",
            families=families,
            inputs=[{"path": path.as_posix(), "sha256": sha256_file(path)}],
        )

    if payload.get("version") != 6:
        raise ValueError("labelset.version: expected 6")
    parent = path.parent
    base_path = resolve_checked_file(parent, payload.get("base"), "labelset.base")
    base_payload = load_object(base_path, "labelset.base")
    families = list(load_flat(base_path, base_payload, "labelset.base"))
    inputs = [{"path": base_path.as_posix(), "sha256": sha256_file(base_path)}]
    components = payload.get("components")
    if not isinstance(components, list) or not components:
        raise ValueError("labelset.components: expected a non-empty array")
    seen_splits: set[str] = set()
    for index, record in enumerate(components):
        record_label = f"labelset.components[{index}]"
        if not isinstance(record, dict) or record.get("split") not in {"dev", "heldout"}:
            raise ValueError(f"{record_label}.split: expected dev or heldout")
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
        component_families = component.get("families")
        if not isinstance(component_families, list) or not component_families:
            raise ValueError(f"{record_label}.families: expected a non-empty array")
        for family_index, family in enumerate(component_families):
            validate_component_family(family, split, f"{record_label}.families[{family_index}]")
        families.extend(component_families)
        inputs.append({"path": component_path.as_posix(), "sha256": sha256_file(component_path)})
    if seen_splits != {"dev", "heldout"}:
        raise ValueError("labelset.components: both dev and heldout components are required")

    identities: set[tuple[str, str]] = set()
    for index, family in enumerate(families):
        identity = (family.get("repo"), family.get("family_id"))
        if not all(isinstance(value, str) and value for value in identity):
            raise ValueError(f"labelset family {index}: missing repo/family_id")
        if identity in identities:
            raise ValueError(f"labelset family {index}: duplicate identity {identity}")
        identities.add(identity)
    return LoadedLabelset(path=path, version="v6", families=families, inputs=inputs)


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
