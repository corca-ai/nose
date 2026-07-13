#!/usr/bin/env python3
"""Freeze and validate the split-safe #841 default-head taxonomy.

The collector deliberately opens only dev repositories and dev label
components.  It never resolves the composite v7 labelset because that would
also resolve the held-out component.  The checked artifact keeps truth labels,
mechanical predicates, and independent source-review votes as separate layers.
"""

from __future__ import annotations

import argparse
from collections import Counter, defaultdict
import copy
from functools import lru_cache
import hashlib
import json
import math
from pathlib import Path, PurePosixPath
import re
import shlex
import statistics
import subprocess
import sys
from typing import Any, Iterable

from query_schema import QUERY_SCHEMA_VERSION, query_families


ROOT = Path(__file__).resolve().parents[2]
REPOS_ROOT = ROOT / "bench" / "repos"
CORPUS = ROOT / "bench" / "goldens" / "corpus.json"
RUNWAY = ROOT / "bench" / "labels" / "default_head_label_runway_2026_07_13.dev.v1.json"
V5 = ROOT / "bench" / "labels" / "refactoring_families.v5.json"
V5_DEV = ROOT / "bench" / "labels" / "refactoring_families.v5.dev.json"
V6_DEV = ROOT / "bench" / "labels" / "refactoring_families.v6.dev.json"
V7_DEV = ROOT / "bench" / "labels" / "refactoring_families.v7.dev.json"
DEFAULT_NOSE = (
    ROOT
    / "target"
    / "issue-839"
    / "official-v0.19.0"
    / "nose-cli-aarch64-apple-darwin"
    / "nose"
)
CHECKED_CORE = ROOT / "bench" / "labels" / "default_head_taxonomy_2026_07_13.dev.core.v1.json"
CHECKED_AUDIT = ROOT / "bench" / "labels" / "default_head_taxonomy_audit_packets_2026_07_13.dev.v1.json"

CORE_SCHEMA = "nose.default_head_taxonomy_core.v1"
FINAL_SCHEMA = "nose.default_head_taxonomy.v1"
VOTE_SCHEMA = "nose.default_head_taxonomy_vote.v1"
AUDIT_SCHEMA = "nose.default_head_taxonomy_audit_packets.v1"
V5_DEV_SCHEMA = "nose.refactoring_families.v5.dev_projection.v1"
AUDIT_PERSONAS = ("pragmatic", "dedupe", "skeptic")
TRUTH_REASONS = (
    "extract-helper",
    "parameterize",
    "extract-base",
    "extract-data-table",
    "parallel-by-design",
    "trivial",
    "generated",
    "coincidental-shape",
    "type-def",
)
WORTHY_REASONS = {
    "extract-helper",
    "parameterize",
    "extract-base",
    "extract-data-table",
}
EXPECTED_TRUTH = {
    "extract-helper": 251,
    "parameterize": 95,
    "extract-base": 19,
    "extract-data-table": 17,
    "parallel-by-design": 148,
    "trivial": 48,
    "generated": 41,
    "coincidental-shape": 30,
    "type-def": 9,
}
SOURCE_READ_LIMIT = 65_536
FROZEN_V5_SHA256 = "e18b65543f4b6373d7eadbc93159adda69699eafe8f5f814d9ba53e245a6d9f9"
FROZEN_V5_DEV_FAMILIES_SHA256 = (
    "01702dde6576a035fe0aa497123d910a79c196db264c7b25e38513ff64a4f969"
)
FROZEN_DEEP_LABELED_KEYS_SHA256 = (
    "5662e63d1cf3d1589ea6043ceb38a5f601861110ae11f87c0b7dcc7c339833aa"
)
FROZEN_AUDIT_PACKET_SET_SHA256 = (
    "ce9bfadb3fca20dac489e0e55b69d39ea9751ab9d2a0042045ad4f0827b5c61e"
)
FROZEN_LEVER_AUDIT_PACKET_SET_SHA256 = {
    "generated-provenance.v1": "7eefa33034ac9e85d1eba25063e6e75e204196cafa5305c6fef75c824d99196e",
    "declaration-only-type.v1": "e7b9b5dddbc7766d2613188db81e80889664f1976f64e0f1eb1317c4042c5c4f",
    "proof-actionability.v1": "4f53cda18c2baa0c0354bb5f9a3ecbe5ed12ab4d8e11ba873c2f11161202b945",
}
FROZEN_COLLECT_COMMAND = (
    "python3 bench/labels/default_head_taxonomy.py collect "
    "--nose target/issue-839/official-v0.19.0/nose-cli-aarch64-apple-darwin/nose "
    "--output bench/labels/default_head_taxonomy_2026_07_13.dev.core.v1.json "
    "--audit-output "
    "bench/labels/default_head_taxonomy_audit_packets_2026_07_13.dev.v1.json"
)
REVIEWED_REBIND_SOURCE_CORE_SHA256 = (
    "d39648d0950c8680dbb821e1a823803d719591ee6f84717e483ccf8811aea036"
)
REVIEWED_REBIND_SOURCE_AUDIT_SHA256 = (
    "6352f61366b0888ef345e310c434b3d02131432ab574b88f5871ff75c0970b59"
)
REVIEWED_REBIND_SOURCE_VOTE_SHA256 = {
    "pragmatic": "c9d1b24ecc7b19e941a2ec792fbb3cf2d0bdda7db2a35ddd8f6d2c389f3df7ed",
    "dedupe": "c3ec64c7ca2ea017de07540ef1abedd544b3f30e80527d0a31c53f8c7a263d89",
    "skeptic": "8533c4462bdacdc5ddeb35d877b3a108d5ff3fbe4b8cce0f36a1a7e6b647fa44",
}
HELDOUT_POLICY = "closed; no held-out component, source path, or judgment was read"
AUDIT_QUESTION = (
    "Does the frozen mechanical premise hold for every member, and would moving this "
    "family out of the bare default avoid hiding an actionable refactoring?"
)


class TaxonomyError(ValueError):
    """The artifact or a frozen input violates the #841 contract."""


def fail(message: str) -> None:
    raise TaxonomyError(message)


def canonical_bytes(value: object) -> bytes:
    return json.dumps(value, sort_keys=True, separators=(",", ":")).encode()


def canonical_sha256(value: object) -> str:
    return hashlib.sha256(canonical_bytes(value)).hexdigest()


def file_sha256(path: Path) -> str:
    digest = hashlib.sha256()
    with path.open("rb") as stream:
        for chunk in iter(lambda: stream.read(1 << 20), b""):
            digest.update(chunk)
    return digest.hexdigest()


def load_json(path: Path) -> dict[str, Any]:
    try:
        value = json.loads(path.read_text(encoding="utf-8"))
    except (OSError, json.JSONDecodeError) as error:
        fail(f"cannot read {path}: {error}")
    if not isinstance(value, dict):
        fail(f"{path}: expected a JSON object")
    return value


def exact_keys(value: object, allowed: set[str], source: str) -> dict[str, Any]:
    if not isinstance(value, dict):
        fail(f"{source}: expected an object")
    actual = set(value)
    if actual != allowed:
        fail(
            f"{source}: exact keys differ; missing={sorted(allowed - actual)}, "
            f"extra={sorted(actual - allowed)}"
        )
    return value


def is_int(value: object) -> bool:
    return isinstance(value, int) and not isinstance(value, bool)


def safe_repo_relative_path(path_text: str, repo: str) -> bool:
    path = PurePosixPath(path_text)
    return (
        not path.is_absolute()
        and ".." not in path.parts
        and path.parts[:3] == ("bench", "repos", repo)
        and len(path.parts) > 3
    )


def write_json(path: Path, value: object) -> None:
    path.write_text(json.dumps(value, indent=2, sort_keys=True) + "\n", encoding="utf-8")


def rel(path: Path | str) -> str:
    resolved = Path(path).resolve()
    try:
        return resolved.relative_to(ROOT).as_posix()
    except ValueError:
        return str(path)


def run(args: list[str], *, cwd: Path = ROOT) -> bytes:
    result = subprocess.run(args, cwd=cwd, check=False, capture_output=True)
    if result.returncode != 0:
        fail(
            f"{shlex.join(args)} failed with {result.returncode}: "
            f"{result.stderr.decode(errors='replace').strip()}"
        )
    return result.stdout


def repository_head(path: Path) -> str:
    return run(["git", "-C", str(path), "rev-parse", "HEAD"]).decode().strip()


def nose_version(path: Path) -> str:
    return run([str(path), "--version"]).decode().strip()


def member(location: dict[str, Any]) -> dict[str, Any]:
    return {
        "file": location["file"],
        "start_line": location["start"],
        "end_line": location["end"],
        "name": location.get("name"),
    }


def overlaps(left: dict[str, Any], right: dict[str, Any]) -> bool:
    return left["file"] == right["file"] and not (
        left["end_line"] < right["start_line"]
        or right["end_line"] < left["start_line"]
    )


def label_overlap(members: list[dict[str, Any]], label: dict[str, Any]) -> int:
    return sum(
        overlaps(query_member, label_member)
        for query_member in members
        for label_member in label["members"]
    )


def freeze_v5_dev(output: Path) -> dict[str, Any]:
    """Create the one-time dev projection; normal #841 collection never calls this."""

    parent = load_json(V5)
    families = parent.get("families")
    if not isinstance(families, list):
        fail(f"{V5}: families must be an array")
    dev = [family for family in families if family.get("split") == "dev"]
    if len(dev) != 5_445 or any(family.get("split") != "dev" for family in dev):
        fail("frozen v5 dev projection must contain exactly 5,445 dev rows")
    artifact = {
        "schema": V5_DEV_SCHEMA,
        "split": "dev",
        "parent": {
            "path": rel(V5),
            "sha256": file_sha256(V5),
            "schema_version": parent.get("schema_version"),
        },
        "families_sha256": canonical_sha256(dev),
        "families": dev,
    }
    write_json(output, artifact)
    return artifact


def validate_v5_dev(payload: dict[str, Any]) -> None:
    exact_keys(
        payload,
        {"schema", "split", "parent", "families_sha256", "families"},
        "v5 dev projection",
    )
    if payload["schema"] != V5_DEV_SCHEMA or payload["split"] != "dev":
        fail("unsupported v5 dev projection")
    exact_keys(payload["parent"], {"path", "sha256", "schema_version"}, "v5 parent")
    if payload["parent"] != {
        "path": rel(V5),
        "sha256": FROZEN_V5_SHA256,
        "schema_version": "0.1.0",
    }:
        fail("v5 dev projection parent commitment drift")
    families = payload["families"]
    if not isinstance(families, list) or len(families) != 5_445:
        fail("v5 dev projection must contain exactly 5,445 rows")
    if any(not isinstance(row, dict) or row.get("split") != "dev" for row in families):
        fail("v5 dev projection contains a non-dev row")
    digest = canonical_sha256(families)
    if digest != payload["families_sha256"]:
        fail("v5 dev family projection digest mismatch")
    if digest != FROZEN_V5_DEV_FAMILIES_SHA256:
        fail("v5 dev projection differs from the frozen parent dev subset")


def load_dev_labels() -> tuple[list[dict[str, Any]], dict[str, dict[str, Any]]]:
    """Load only explicit dev components; never resolve a composite labelset."""

    rows: list[dict[str, Any]] = []
    by_candidate: dict[str, dict[str, Any]] = {}
    corpus = load_json(CORPUS)
    dev_repos = {
        row["id"] for row in corpus.get("repositories", []) if row.get("split") == "dev"
    }
    sources = ((V5_DEV, "v5-dev"), (V6_DEV, "v6-dev"), (V7_DEV, "v7-dev"))
    for path, source in sources:
        payload = load_json(path)
        if path == V5_DEV:
            validate_v5_dev(payload)
        families = payload.get("families")
        if not isinstance(families, list):
            fail(f"{path}: families must be an array")
        for original in families:
            if original.get("split") != "dev":
                fail(f"{path}: non-dev row in an explicit dev component")
            repo = original.get("repo")
            members = original.get("members")
            if repo not in dev_repos or not isinstance(members, list) or not members:
                fail(f"{path}: label is outside the exact dev repository set")
            for index, label_member in enumerate(members):
                exact_keys(
                    label_member,
                    {"file", "start_line", "end_line", "name"},
                    f"{path}: {original.get('family_id')}.members[{index}]",
                )
                if not safe_repo_relative_path(label_member.get("file", ""), repo):
                    fail(f"{path}: label member escapes its dev repository")
                if (
                    not is_int(label_member.get("start_line"))
                    or not is_int(label_member.get("end_line"))
                    or label_member["start_line"] < 1
                    or label_member["end_line"] < label_member["start_line"]
                ):
                    fail(f"{path}: label member has invalid line bounds")
            row = copy.deepcopy(original)
            row["_label_source"] = source
            row["_label_sha256"] = canonical_sha256(original)
            rows.append(row)
            candidate_key = row.get("candidate_key")
            if isinstance(candidate_key, str):
                if candidate_key in by_candidate:
                    fail(f"duplicate exact label candidate key: {candidate_key}")
                by_candidate[candidate_key] = row
    return rows, by_candidate


def match_label(
    *,
    repo: str,
    candidate_key: str,
    members: list[dict[str, Any]],
    labels_by_repo: dict[str, list[dict[str, Any]]],
    labels_by_candidate: dict[str, dict[str, Any]],
    labels_by_id: dict[str, dict[str, Any]],
    preferred_family_id: str | None,
) -> tuple[dict[str, Any], int, int]:
    exact = labels_by_candidate.get(candidate_key)
    if exact is not None:
        return exact, label_overlap(members, exact), 0
    if preferred_family_id is not None:
        preferred = labels_by_id.get(preferred_family_id)
        if preferred is None:
            fail(f"{candidate_key}: #840 matched label {preferred_family_id} is absent")
        overlap_count = label_overlap(members, preferred)
        runner_up = max(
            (
                label_overlap(members, label)
                for label in labels_by_repo.get(repo, [])
                if label["family_id"] != preferred_family_id
            ),
            default=0,
        )
        return preferred, overlap_count, runner_up
    scored = sorted(
        (
            (label_overlap(members, label), label["family_id"], label)
            for label in labels_by_repo.get(repo, [])
        ),
        key=lambda item: (-item[0], item[1]),
    )
    if not scored or scored[0][0] == 0:
        fail(f"{candidate_key}: no dev label overlaps this head position")
    best_overlap, _, best = scored[0]
    runner_up = scored[1][0] if len(scored) > 1 else 0
    ties = [item for item in scored if item[0] == best_overlap]
    truth = {(item[2]["worthy"], item[2]["reason"]) for item in ties}
    if len(truth) > 1:
        fail(f"{candidate_key}: ambiguous best label match with conflicting truth")
    return best, best_overlap, runner_up


def source_path(path_text: str, repo: str) -> Path:
    path = (ROOT / path_text).resolve()
    allowed = (REPOS_ROOT / repo).resolve()
    try:
        path.relative_to(allowed)
    except ValueError as error:
        fail(f"{repo}: source path escapes its dev repository: {path_text}")
    if not path.is_file():
        fail(f"{repo}: missing source file: {path_text}")
    return path


@lru_cache(maxsize=None)
def live_source_record(path_text: str, repo: str) -> tuple[int, str]:
    path = source_path(path_text, repo)
    return path.stat().st_size, file_sha256(path)


@lru_cache(maxsize=None)
def jazzy_evidence(path_text: str, repo: str) -> dict[str, Any] | None:
    path = source_path(path_text, repo)
    with path.open("rb") as stream:
        bounded = stream.read(SOURCE_READ_LIMIT)
    lower = bounded.lower()
    asset_tokens = (b"jazzy.css", b"jazzy.js")
    anchor_tokens = (b'class="dashanchor"', b"//apple_ref/")
    asset = next((token for token in asset_tokens if token in lower), None)
    anchor = next((token for token in anchor_tokens if token in lower), None)
    if path.suffix.lower() != ".html" or asset is None or anchor is None:
        return None

    def signal(token: bytes) -> dict[str, Any]:
        index = lower.index(token)
        line = bounded[:index].count(b"\n") + 1
        return {
            "kind": token.decode(),
            "line": line,
            "digest": hashlib.sha256(token + b"\0" + str(line).encode()).hexdigest(),
        }

    return {
        "kind": "jazzy-generated-documentation",
        "path": path_text,
        "source_bytes": live_source_record(path_text, repo)[0],
        "source_sha256": live_source_record(path_text, repo)[1],
        "read_bytes": len(bounded),
        "read_limit": SOURCE_READ_LIMIT,
        "signals": [signal(asset), signal(anchor)],
    }


def generated_provenance(family: dict[str, Any], repo: str) -> dict[str, Any]:
    files = sorted({location["file"] for location in family["locations"]})
    evidence = [jazzy_evidence(path, repo) for path in files]
    matched = bool(files) and all(item is not None for item in evidence)
    return {
        "rule": "all-member-files-jazzy-asset-and-apple-symbol-anchor.v1",
        "matched": matched,
        "files": [item for item in evidence if item is not None],
    }


def declaration_only_type(family: dict[str, Any]) -> bool:
    locations = family["locations"]
    if not locations:
        return False
    for location in locations:
        origin = location.get("origin")
        if not isinstance(origin, dict):
            return False
        domains = origin.get("domains")
        flags = origin.get("evidence_flags")
        if not isinstance(domains, list) or not isinstance(flags, list):
            return False
        if "type-contract" not in domains or origin.get("body_kind") != "declaration-only":
            return False
        if origin.get("source_granularity") != "whole-unit":
            return False
        if not {"declaration-only", "type-only"}.issubset(flags):
            return False
        if {"has-runtime-body", "has-reusable-body"} & set(flags):
            return False
        if {"runtime", "implementation-type", "data"} & set(domains):
            return False
    return True


def origin_facets(family: dict[str, Any]) -> dict[str, Any]:
    origins = [location.get("origin") for location in family["locations"]]
    present = [origin for origin in origins if isinstance(origin, dict)]
    coverage = "all" if len(present) == len(origins) else "none" if not present else "partial"

    def values(key: str) -> list[str]:
        return sorted({str(origin[key]) for origin in present if origin.get(key) is not None})

    return {
        "coverage": coverage,
        "body_kinds": values("body_kind"),
        "region_kinds": values("region_kind"),
        "source_granularities": values("source_granularity"),
        "subkinds": values("subkind"),
        "domains": sorted(
            {str(value) for origin in present for value in origin.get("domains", [])}
        ),
        "evidence_flags": sorted(
            {str(value) for origin in present for value in origin.get("evidence_flags", [])}
        ),
    }


def finite_number(value: object, field: str) -> float:
    if isinstance(value, bool) or not isinstance(value, (int, float)):
        fail(f"{field}: expected a number")
    result = float(value)
    if not math.isfinite(result):
        fail(f"{field}: expected a finite number")
    return result


def derived_features(family: dict[str, Any]) -> dict[str, Any]:
    metrics = family.get("metrics")
    if not isinstance(metrics, dict):
        fail(f"family {family.get('id')}: metrics must be an object")
    spans = [location["end"] - location["start"] + 1 for location in family["locations"]]
    mean_span = statistics.fmean(spans)
    span_cv = statistics.pstdev(spans) / mean_span if mean_span else None
    actual_languages = sorted({location.get("lang", "unknown") for location in family["locations"]})
    files = sorted({location["file"] for location in family["locations"]})
    modules = sorted({str(Path(path).parent) for path in files})
    if len(actual_languages) > 1:
        path_relation = "cross-language"
    elif len(modules) > 1:
        path_relation = "cross-module"
    elif len(files) > 1:
        path_relation = "same-module-multi-file"
    else:
        path_relation = "same-file"
    rep_lines = finite_number(family.get("rep_lines"), "rep_lines")
    tightness = None
    if len(actual_languages) == 1 and rep_lines > 0:
        tightness = max(0.0, min(1.0, finite_number(metrics.get("shared_weight"), "shared_weight") / rep_lines))
    shared = finite_number(family.get("shared"), "shared")
    params = finite_number(family.get("params"), "params")
    param_ratio = params / shared if shared > 0 else None
    return {
        "actual_member_languages": actual_languages,
        "member_span_mean": mean_span,
        "member_span_population_stddev": statistics.pstdev(spans),
        "member_span_cv": span_cv,
        "path_relation": path_relation,
        "ownership_relation": "unknown",
        "ranking_tightness": tightness,
        "param_to_shared_ratio": param_ratio,
        "top_level": {
            key: family.get(key)
            for key in ("shared", "params", "removable", "rep_lines")
        },
        "ranking_metrics": {
            key: metrics.get(key)
            for key in (
                "mean_sem",
                "members",
                "modules",
                "files",
                "languages",
                "mean_score",
                "mean_lines",
                "shared_weight",
                "params",
                "scope",
                "value",
                "dup_lines",
                "shared_lines",
            )
        },
    }


def proof_flags(family: dict[str, Any]) -> dict[str, bool]:
    witness = family.get("witness")
    mean_lines = finite_number(family["metrics"].get("mean_lines"), "mean_lines")
    shared = finite_number(family.get("shared"), "shared")
    params = finite_number(family.get("params"), "params")
    proven = witness in {"exact", "subdag"}
    return {
        "proof_backed": proven,
        "existing_trivial": proven and mean_lines <= 4,
        "existing_shallow": proven and mean_lines > 4 and shared > 0 and params >= 0.33 * shared,
    }


def truth_bucket(label: dict[str, Any]) -> str:
    worthy = label.get("worthy")
    reason = label.get("reason")
    if reason not in TRUTH_REASONS or worthy != (reason in WORTHY_REASONS):
        fail(f"label {label.get('family_id')}: invalid worthy/reason pairing")
    return ("worthy." if worthy else "non_action.") + reason


def classify_mechanical(
    *, label: dict[str, Any], generated: bool, declaration: bool, proof: bool
) -> tuple[str, list[str], str | None]:
    matched = []
    if generated:
        matched.append("generated-provenance.v1")
    if declaration:
        matched.append("declaration-only-type.v1")
    if len(matched) > 1:
        fail(f"selected mechanical predicates overlap: {matched}")
    if generated:
        return "mechanical.generated-provenance", matched, matched[0]
    if declaration:
        return "mechanical.declaration-only-type", matched, matched[0]
    if proof:
        return "protected.proof-actionability", matched, None
    reason = label["reason"]
    if label["worthy"]:
        return f"actionable.{reason}", matched, None
    if reason in {"parallel-by-design", "coincidental-shape"}:
        return f"judgment-deep.{reason}", matched, None
    return f"unproven.{reason}", matched, None


def source_bounds(family: dict[str, Any], repo: str) -> list[dict[str, Any]]:
    records = []
    for location in family["locations"]:
        source_path(location["file"], repo)
        size, digest = live_source_record(location["file"], repo)
        records.append(
            {
                "id": location.get("id"),
                "file": location["file"],
                "start": location["start"],
                "end": location["end"],
                "name": location.get("name"),
                "lang": location.get("lang"),
                "source_bytes": size,
                "source_sha256": digest,
            }
        )
    return records


def make_row(
    *,
    repo: str,
    rank: int,
    primary_language: str,
    runway_candidate: dict[str, Any],
    family: dict[str, Any],
    label: dict[str, Any],
    label_overlap_count: int,
    label_runner_up_overlap: int,
) -> dict[str, Any]:
    generated = generated_provenance(family, repo)
    declaration = declaration_only_type(family)
    origin = origin_facets(family)
    derived = derived_features(family)
    proofs = proof_flags(family)
    bucket, matched, selected = classify_mechanical(
        label=label,
        generated=generated["matched"],
        declaration=declaration,
        proof=proofs["proof_backed"],
    )
    truth = {
        "worthy": label["worthy"],
        "reason": label["reason"],
        "bucket": truth_bucket(label),
        "confidence": label.get("confidence"),
        "label_family_id": label["family_id"],
        "label_source": label["_label_source"],
        "label_sha256": label["_label_sha256"],
        "member_overlap": label_overlap_count,
        "runner_up_overlap": label_runner_up_overlap,
    }
    raw_hash = canonical_sha256(family)
    if raw_hash != runway_candidate.get("raw_family_sha256"):
        fail(f"{runway_candidate['candidate_key']}: raw family hash drift")
    row = {
        "position_key": runway_candidate["candidate_key"],
        "candidate_sha256": runway_candidate["candidate_sha256"],
        "repo": repo,
        "rank": rank,
        "primary_language": primary_language,
        "query_family_id": family["id"],
        "raw_family_sha256": raw_hash,
        "raw_family": family,
        "source_bounds": source_bounds(family, repo),
        "truth": truth,
        "facets": {
            "witness": family.get("witness"),
            "surface": family.get("surface"),
            "actionability_classifier": "unavailable-in-query-schema-v7",
            "scope": family.get("scope"),
            "extraction_shape": family.get("extraction_shape"),
            "same_symbol": family.get("same_symbol"),
            "source_comparable": family.get("source_comparable"),
            "origin": origin,
            "generated_provenance": generated,
            **derived,
        },
        "predicate_results": {
            "generated-provenance.v1": generated["matched"],
            "declaration-only-type.v1": declaration,
            **proofs,
        },
        "mechanical_bucket": bucket,
        "matched_levers": matched,
        "selected_lever": selected,
    }
    row["row_sha256"] = canonical_sha256({key: value for key, value in row.items() if key != "row_sha256"})
    return row


def make_unlabeled_audit_row(
    *,
    repo: str,
    rank: int,
    primary_language: str,
    runway_candidate: dict[str, Any],
    family: dict[str, Any],
) -> dict[str, Any] | None:
    """Return a truth-blind deep positive, or ``None`` for a non-positive.

    All rank 11-30 rows are eligible for an independent source audit, while
    only the deterministic 65-row v7 sample has a truth label.  Keeping this
    representation separate prevents unlabeled rows from entering taxonomy or
    precision denominators as if they were ground truth.
    """

    generated = generated_provenance(family, repo)
    declaration = declaration_only_type(family)
    matched = []
    if generated["matched"]:
        matched.append("generated-provenance.v1")
    if declaration:
        matched.append("declaration-only-type.v1")
    if not matched:
        return None
    if len(matched) > 1:
        fail(f"{runway_candidate['candidate_key']}: deep audit predicates overlap")
    raw_hash = canonical_sha256(family)
    if raw_hash != runway_candidate.get("raw_family_sha256"):
        fail(f"{runway_candidate['candidate_key']}: raw family hash drift")
    row = {
        "position_key": runway_candidate["candidate_key"],
        "candidate_sha256": runway_candidate["candidate_sha256"],
        "repo": repo,
        "rank": rank,
        "primary_language": primary_language,
        "query_family_id": family["id"],
        "raw_family_sha256": raw_hash,
        "source_bounds": source_bounds(family, repo),
        "facets": {
            "witness": family.get("witness"),
            "extraction_shape": family.get("extraction_shape"),
            "origin": origin_facets(family),
            "generated_provenance": generated,
        },
        "selected_lever": matched[0],
    }
    row["row_sha256"] = canonical_sha256(
        {key: value for key, value in row.items() if key != "row_sha256"}
    )
    return row


def band(value: float | None, boundaries: tuple[float, ...]) -> str:
    if value is None:
        return "unknown"
    low = 0.0
    for high in boundaries:
        if value < high:
            return f"[{low:g},{high:g})"
        low = high
    return f"[{low:g},inf)"


def cross_tab(rows: list[dict[str, Any]], extractor: Any) -> dict[str, Any]:
    grouped: dict[str, list[dict[str, Any]]] = defaultdict(list)
    for row in rows:
        values = extractor(row)
        if not isinstance(values, list):
            values = [values]
        for value in values or ["unknown"]:
            grouped[str(value)].append(row)
    result = {}
    for value, cohort in sorted(grouped.items()):
        reasons = Counter(row["truth"]["reason"] for row in cohort)
        result[value] = {
            "positions": len(cohort),
            "worthy": sum(row["truth"]["worthy"] for row in cohort),
            "non_action": sum(not row["truth"]["worthy"] for row in cohort),
            "reasons": dict(sorted(reasons.items())),
            "repository_count": len({row["repo"] for row in cohort}),
            "repositories": sorted({row["repo"] for row in cohort}),
        }
    return result


def build_cross_tabs(rows: list[dict[str, Any]]) -> dict[str, Any]:
    return {
        "truth_reason": cross_tab(rows, lambda row: row["truth"]["reason"]),
        "witness": cross_tab(rows, lambda row: row["facets"]["witness"]),
        "surface": cross_tab(rows, lambda row: row["facets"]["surface"]),
        "actionability_classifier": cross_tab(
            rows, lambda row: row["facets"]["actionability_classifier"]
        ),
        "scope": cross_tab(rows, lambda row: row["facets"]["scope"]),
        "extraction_shape": cross_tab(rows, lambda row: row["facets"]["extraction_shape"]),
        "origin_coverage": cross_tab(rows, lambda row: row["facets"]["origin"]["coverage"]),
        "origin_domains": cross_tab(rows, lambda row: row["facets"]["origin"]["domains"]),
        "origin_body_kinds": cross_tab(rows, lambda row: row["facets"]["origin"]["body_kinds"]),
        "origin_region_kinds": cross_tab(rows, lambda row: row["facets"]["origin"]["region_kinds"]),
        "origin_granularities": cross_tab(rows, lambda row: row["facets"]["origin"]["source_granularities"]),
        "origin_subkinds": cross_tab(rows, lambda row: row["facets"]["origin"]["subkinds"]),
        "generated_provenance": cross_tab(rows, lambda row: row["facets"]["generated_provenance"]["matched"]),
        "shared": cross_tab(rows, lambda row: band(row["facets"]["top_level"]["shared"], (4, 8, 16, 32))),
        "params": cross_tab(rows, lambda row: band(row["facets"]["top_level"]["params"], (1, 2, 4, 8))),
        "ranking_tightness": cross_tab(rows, lambda row: band(row["facets"]["ranking_tightness"], (0.25, 0.5, 0.75))),
        "member_span_cv": cross_tab(rows, lambda row: band(row["facets"]["member_span_cv"], (0.1, 0.3, 0.5))),
        "path_relation": cross_tab(rows, lambda row: row["facets"]["path_relation"]),
        "ownership_relation": cross_tab(rows, lambda row: row["facets"]["ownership_relation"]),
        "primary_language": cross_tab(rows, lambda row: row["primary_language"]),
        "actual_member_languages": cross_tab(rows, lambda row: row["facets"]["actual_member_languages"]),
        "repository": cross_tab(rows, lambda row: row["repo"]),
        "mechanical_bucket": cross_tab(rows, lambda row: row["mechanical_bucket"]),
    }


def predicate_stat(rows: list[dict[str, Any]], predicate: Any) -> dict[str, Any]:
    matches = [row for row in rows if predicate(row)]
    worthy = [row for row in matches if row["truth"]["worthy"]]
    return {
        "positions": len(matches),
        "non_action": len(matches) - len(worthy),
        "worthy": len(worthy),
        "non_action_precision": (len(matches) - len(worthy)) / len(matches) if matches else None,
        "position_keys": [row["position_key"] for row in matches],
        "worthy_hard_negatives": [row["position_key"] for row in worthy],
    }


def heuristic_rules() -> list[tuple[str, dict[str, Any], Any, str]]:
    return [
        ("scope-test.v1", {"field": "scope", "eq": "test"}, lambda r: r["facets"]["scope"] == "test", "test scope contains many valuable refactorings"),
        ("same-symbol.v1", {"field": "same_symbol", "eq": True}, lambda r: r["facets"]["same_symbol"] is True, "symbol equality is not an actionability verdict"),
        ("same-file.v1", {"field": "path_relation", "eq": "same-file"}, lambda r: r["facets"]["path_relation"] == "same-file", "file proximity is not an actionability verdict"),
        ("type-contract-only.v1", {"field": "origin.domains", "contains": "type-contract"}, lambda r: "type-contract" in r["facets"]["origin"]["domains"], "type contracts with implementations remain actionable"),
        ("declaration-only-only.v1", {"field": "origin.body_kinds", "contains": "declaration-only"}, lambda r: "declaration-only" in r["facets"]["origin"]["body_kinds"], "mixed and non-contract declarations are not proven non-actions"),
        ("exact-subdag-witness-only.v1", {"field": "witness", "in": ["exact", "subdag"]}, lambda r: r["predicate_results"]["proof_backed"], "proof strength does not establish worthiness"),
        ("proven-trivial.v1", {"predicate": "existing_trivial"}, lambda r: r["predicate_results"]["existing_trivial"], "worthy hard negatives make blanket proof exemption removal unsafe"),
        ("proven-shallow.v1", {"predicate": "existing_shallow"}, lambda r: r["predicate_results"]["existing_shallow"], "worthy hard negatives make blanket proof exemption removal unsafe"),
        ("params-shared-third.v1", {"field": "param_to_shared_ratio", "gte": 0.33}, lambda r: (r["facets"]["param_to_shared_ratio"] or 0) >= 0.33, "parameter density alone is not actionability"),
        ("span-cv-0.30.v1", {"field": "member_span_cv", "gte": 0.30, "same_language": True}, lambda r: r["facets"]["member_span_cv"] is not None and r["facets"]["member_span_cv"] >= 0.30 and len(r["facets"]["actual_member_languages"]) == 1, "head-only precision does not generalize to labeled deep hard negatives"),
        ("tightness-0.25.v1", {"field": "ranking_tightness", "lte": 0.25}, lambda r: r["facets"]["ranking_tightness"] is not None and r["facets"]["ranking_tightness"] <= 0.25, "ranking tightness has worthy hard negatives"),
        ("current-generated-evidence.v1", {"field": "surface", "eq": "generated"}, lambda r: r["facets"]["surface"] == "generated", "the current default head exposes no generated-surface positives"),
    ]


def rejected_heuristics(rows: list[dict[str, Any]]) -> list[dict[str, Any]]:
    return [
        {
            "heuristic_id": name,
            "status": "rejected",
            "predicate_ast": ast,
            "reason": reason,
            "dev_head": predicate_stat(rows, predicate),
        }
        for name, ast, predicate, reason in heuristic_rules()
    ]


def split_rejected_heuristic_stats(artifact: dict[str, Any]) -> None:
    """Separate head results from the frozen 65-row deep generalization sample."""

    by_id = {name: predicate for name, _, predicate, _ in heuristic_rules()}
    for record in artifact["rejected_heuristics"]:
        predicate = by_id[record["heuristic_id"]]
        record["frozen_labeled_pool"] = record.pop("dev_head")
        record["dev_head"] = predicate_stat(artifact["head_rows"], predicate)
        record["dev_deep_labeled"] = predicate_stat(
            artifact["deep_labeled_rows"], predicate
        )


def audit_packet(row: dict[str, Any], lever: str) -> dict[str, Any]:
    packet = {
        "audit_key": f"{lever}:{row['position_key']}",
        "lever_id": lever,
        "candidate_sha256": row["candidate_sha256"],
        "raw_family_sha256": row["raw_family_sha256"],
        "repo": row["repo"],
        "rank": row["rank"],
        "query_family_id": row["query_family_id"],
        "source_bounds": row["source_bounds"],
        "review_question": AUDIT_QUESTION,
        "frozen_evidence": {
            "generated_provenance": row["facets"]["generated_provenance"],
            "origin": row["facets"]["origin"],
            "witness": row["facets"]["witness"],
            "extraction_shape": row["facets"]["extraction_shape"],
        },
    }
    packet["packet_sha256"] = canonical_sha256({key: value for key, value in packet.items() if key != "packet_sha256"})
    return packet


def lever_contracts() -> list[dict[str, Any]]:
    return [
        {
            "lever_id": "generated-provenance.v1",
            "status": "selected-pending-independent-audit",
            "predicate_ast": {
                "op": "all_unique_member_files",
                "suffix": ".html",
                "normalization": "ascii-lowercase",
                "bounded_prefix_bytes": SOURCE_READ_LIMIT,
                "requires_any": [["jazzy.css", "jazzy.js"], ["class=\"dashanchor\"", "//apple_ref/"]],
            },
            "missing_value_policy": "fail-closed",
            "runtime_cost": "read at most 64 KiB once per unique member file; only for candidate HTML files",
            "output_cost": "one reason-coded surface transition; family remains in all top=0",
        },
        {
            "lever_id": "declaration-only-type.v1",
            "status": "selected-pending-independent-audit",
            "predicate_ast": {
                "op": "all_locations",
                "requires": {
                    "origin.body_kind": "declaration-only",
                    "origin.domains_contains": "type-contract",
                    "origin.evidence_flags_contains_all": ["declaration-only", "type-only"],
                    "origin.source_granularity": "whole-unit",
                },
                "forbids": {
                    "origin.domains": ["runtime", "implementation-type", "data"],
                    "origin.evidence_flags": ["has-runtime-body", "has-reusable-body"],
                },
            },
            "missing_value_policy": "fail-closed",
            "runtime_cost": "existing query-origin facets only; one all-members predicate",
            "output_cost": "one reason-coded surface transition; family remains in all top=0",
        },
        {
            "lever_id": "proof-actionability.v1",
            "status": "rejected-no-go",
            "predicate_ast": {"field": "witness", "in": ["exact", "subdag"]},
            "missing_value_policy": "not-matched",
            "runtime_cost": "none beyond existing witness metadata",
            "output_cost": "none; no product behavior change",
            "reason": "blanket exemption removal has worthy hard negatives and fails the 90% gate",
        },
    ]


def make_levers(
    head: list[dict[str, Any]], deep: list[dict[str, Any]]
) -> tuple[list[dict[str, Any]], list[dict[str, Any]]]:
    definitions = lever_contracts()
    result = []
    packets: list[dict[str, Any]] = []
    for definition in definitions:
        lever = definition["lever_id"]
        if lever == "proof-actionability.v1":
            positives = [row for row in head if row["predicate_results"]["proof_backed"]]
            audit = []
        else:
            positives = [row for row in head if row["selected_lever"] == lever]
            audit = [audit_packet(row, lever) for row in deep if row["selected_lever"] == lever]
        worthy = [row for row in positives if row["truth"]["worthy"]]
        definition.update(
            {
                "positive_position_keys": [row["position_key"] for row in positives],
                "head_movement": len(positives),
                "head_non_action": len(positives) - len(worthy),
                "worthy_false_demotions": [
                    {
                        "position_key": row["position_key"],
                        "truth_reason": row["truth"]["reason"],
                        "reviewed_explanation": None,
                    }
                    for row in worthy
                ],
                "head_false_demotion_rate": len(worthy) / len(positives) if positives else 0.0,
                "repository_breadth": sorted({row["repo"] for row in positives}),
                "member_language_breadth": sorted(
                    {lang for row in positives for lang in row["facets"]["actual_member_languages"]}
                ),
                "audit_packet_keys": [packet["audit_key"] for packet in audit],
                "audit_packet_count": len(audit),
                "audit_packet_set_sha256": canonical_sha256(audit),
                "replacement_effect": {
                    "vacated_head_slots": len(positives),
                    "rank_11_replacements": "not modeled in #841; measured after product implementation",
                },
            }
        )
        packets.extend(audit)
        result.append(definition)
    return result, packets


def collect(nose: Path) -> tuple[dict[str, Any], dict[str, Any]]:
    corpus = load_json(CORPUS)
    runway = load_json(RUNWAY)
    if runway.get("split") != "dev":
        fail("runway must be the explicit dev artifact")
    metadata = {
        row["id"]: row for row in corpus.get("repositories", []) if row.get("split") == "dev"
    }
    if len(metadata) != 66:
        fail(f"expected 66 dev repositories, got {len(metadata)}")
    runway_repos = runway.get("repositories")
    runway_candidates = {
        (row["repo"], row["rank"]): row for row in runway.get("candidates", [])
    }
    if not isinstance(runway_repos, dict) or set(runway_repos) != set(metadata):
        fail("runway repository set does not equal the corpus dev split")
    labels, labels_by_candidate = load_dev_labels()
    labels_by_repo: dict[str, list[dict[str, Any]]] = defaultdict(list)
    labels_by_id: dict[str, dict[str, Any]] = {}
    for label in labels:
        labels_by_repo[label["repo"]].append(label)
        family_id = label["family_id"]
        if family_id in labels_by_id:
            fail(f"duplicate dev label family ID: {family_id}")
        labels_by_id[family_id] = label

    head_rows: list[dict[str, Any]] = []
    deep_rows: list[dict[str, Any]] = []
    deep_audit_rows: list[dict[str, Any]] = []
    repository_records: dict[str, Any] = {}
    for repo, meta in sorted(metadata.items()):
        repo_path = REPOS_ROOT / repo
        if not repo_path.is_dir():
            fail(f"missing dev repository: {repo}")
        commit = repository_head(repo_path)
        if commit != meta["commit"]:
            fail(f"{repo}: pinned commit drift: {commit} != {meta['commit']}")
        query_args = [
            str(nose.resolve()), "query", rel(repo_path), "top=30", "--format", "json"
        ]
        stdout = run(query_args)
        expected_stdout = runway_repos[repo]["query_stdout_sha256"]
        if hashlib.sha256(stdout).hexdigest() != expected_stdout:
            fail(f"{repo}: official v0.19.0 query stdout drifted from #840")
        families = query_families(stdout, source=f"#841 dev query {repo}")
        if len(families) != runway_repos[repo]["top_30_reported"]:
            fail(f"{repo}: top-30 count drift")
        repository_records[repo] = {
            "commit": commit,
            "primary_language": meta["primary_language"],
            "query_command": shlex.join(
                [rel(nose), "query", rel(repo_path), "top=30", "--format", "json"]
            ),
            "query_stdout_sha256": expected_stdout,
            "top_30_reported": len(families),
            "top_10_reported": min(10, len(families)),
        }
        for rank, family in enumerate(families, start=1):
            candidate = runway_candidates.get((repo, rank))
            if candidate is None or candidate["family"]["id"] != family["id"]:
                fail(f"{repo}: rank {rank} differs from #840 runway")
            if rank <= 10 or candidate["candidate_key"] in labels_by_candidate:
                members = [member(location) for location in family["locations"]]
                label, overlap_count, runner_up = match_label(
                    repo=repo,
                    candidate_key=candidate["candidate_key"],
                    members=members,
                    labels_by_repo=labels_by_repo,
                    labels_by_candidate=labels_by_candidate,
                    labels_by_id=labels_by_id,
                    preferred_family_id=candidate["family"].get("matched_v6_family_id"),
                )
                row = make_row(
                    repo=repo,
                    rank=rank,
                    primary_language=meta["primary_language"],
                    runway_candidate=candidate,
                    family=family,
                    label=label,
                    label_overlap_count=overlap_count,
                    label_runner_up_overlap=runner_up,
                )
                if rank <= 10:
                    head_rows.append(row)
                else:
                    deep_rows.append(row)
                    if row["selected_lever"] is not None:
                        deep_audit_rows.append(row)
            elif rank > 10:
                audit_row = make_unlabeled_audit_row(
                    repo=repo,
                    rank=rank,
                    primary_language=meta["primary_language"],
                    runway_candidate=candidate,
                    family=family,
                )
                if audit_row is not None:
                    deep_audit_rows.append(audit_row)

    levers, audit_packets = make_levers(head_rows, deep_audit_rows)
    input_paths = (CORPUS, RUNWAY, V5_DEV, V6_DEV, V7_DEV, Path(__file__))
    artifact = {
        "schema": CORE_SCHEMA,
        "split": "dev",
        "heldout_policy": HELDOUT_POLICY,
        "query_schema_version": QUERY_SCHEMA_VERSION,
        "provenance": {
            "command": shlex.join(["python3", *sys.argv]),
            "nose_binary": rel(nose),
            "nose_binary_sha256": file_sha256(nose),
            "nose_version": nose_version(nose),
            "inputs": [
                {"path": rel(path), "sha256": file_sha256(path)} for path in input_paths
            ],
        },
        "repositories": repository_records,
        "summary": {
            "head_positions": len(head_rows),
            "deep_labeled_audit_pool": len(deep_rows),
            "deep_mechanical_audit_positives": len(deep_audit_rows),
            "worthy": sum(row["truth"]["worthy"] for row in head_rows),
            "non_action": sum(not row["truth"]["worthy"] for row in head_rows),
            "truth_reasons": dict(sorted(Counter(row["truth"]["reason"] for row in head_rows).items())),
        },
        "head_rows": head_rows,
        "deep_labeled_rows": deep_rows,
        "cross_tabs": build_cross_tabs(head_rows),
        "levers": levers,
        "rejected_heuristics": rejected_heuristics(head_rows + deep_rows),
    }
    artifact["core_sha256"] = canonical_sha256({key: value for key, value in artifact.items() if key != "core_sha256"})
    validate_core(artifact, live_sources=True)
    audit_artifact = {
        "schema": AUDIT_SCHEMA,
        "split": "dev",
        "core_sha256": artifact["core_sha256"],
        "packet_set_sha256": canonical_sha256(audit_packets),
        "packets": audit_packets,
    }
    audit_artifact["artifact_sha256"] = canonical_sha256(
        {key: value for key, value in audit_artifact.items() if key != "artifact_sha256"}
    )
    validate_audit_artifact(artifact, audit_artifact, live_sources=True)
    return artifact, audit_artifact


def find_lever(artifact: dict[str, Any], lever_id: str) -> dict[str, Any]:
    matches = [lever for lever in artifact.get("levers", []) if lever.get("lever_id") == lever_id]
    if len(matches) != 1:
        fail(f"expected exactly one {lever_id} lever")
    return matches[0]


def validate_source_bound(
    bound: object,
    *,
    repo: str,
    source: str,
    runway_sources: dict[str, dict[str, Any]],
    live_sources: bool,
) -> dict[str, Any]:
    row = exact_keys(
        bound,
        {"id", "file", "start", "end", "name", "lang", "source_bytes", "source_sha256"},
        source,
    )
    path_text = row.get("file")
    if not isinstance(path_text, str) or not safe_repo_relative_path(path_text, repo):
        fail(f"{source}: path is outside its exact dev repository")
    if not is_int(row.get("start")) or not is_int(row.get("end")):
        fail(f"{source}: line bounds must be integers")
    if row["start"] < 1 or row["end"] < row["start"]:
        fail(f"{source}: invalid inclusive line bounds")
    frozen = runway_sources.get(path_text)
    expected_source = {
        "source_bytes": frozen.get("bytes") if isinstance(frozen, dict) else None,
        "source_sha256": frozen.get("sha256") if isinstance(frozen, dict) else None,
    }
    if {key: row.get(key) for key in expected_source} != expected_source:
        fail(f"{source}: source bytes/hash differ from the #840 runway")
    if live_sources:
        size, digest = live_source_record(path_text, repo)
        if size != row["source_bytes"] or digest != row["source_sha256"]:
            fail(f"{source}: live source bytes/hash drift")
    return row


def validate_generated_record(
    value: object,
    *,
    repo: str,
    member_files: list[str],
    source: str,
    runway_sources: dict[str, dict[str, Any]],
    live_sources: bool,
) -> dict[str, Any]:
    record = exact_keys(value, {"rule", "matched", "files"}, source)
    if record["rule"] != "all-member-files-jazzy-asset-and-apple-symbol-anchor.v1":
        fail(f"{source}: generated provenance rule drift")
    if not isinstance(record["matched"], bool) or not isinstance(record["files"], list):
        fail(f"{source}: invalid generated provenance shape")
    evidence_paths = []
    for index, item in enumerate(record["files"]):
        evidence = exact_keys(
            item,
            {"kind", "path", "source_bytes", "source_sha256", "read_bytes", "read_limit", "signals"},
            f"{source}.files[{index}]",
        )
        if evidence["kind"] != "jazzy-generated-documentation":
            fail(f"{source}: unexpected generated evidence kind")
        path_text = evidence["path"]
        if path_text not in member_files or not safe_repo_relative_path(path_text, repo):
            fail(f"{source}: generated evidence is not a member file")
        frozen = runway_sources.get(path_text)
        if not isinstance(frozen, dict) or {
            "source_bytes": frozen.get("bytes"),
            "source_sha256": frozen.get("sha256"),
        } != {key: evidence[key] for key in ("source_bytes", "source_sha256")}:
            fail(f"{source}: generated evidence source binding drift")
        if evidence["read_limit"] != SOURCE_READ_LIMIT or evidence["read_bytes"] != min(
            SOURCE_READ_LIMIT, evidence["source_bytes"]
        ):
            fail(f"{source}: generated evidence read bound drift")
        signals = evidence["signals"]
        if not isinstance(signals, list) or len(signals) != 2:
            fail(f"{source}: expected exactly two generated signals")
        kinds = []
        for signal_index, item_signal in enumerate(signals):
            signal = exact_keys(
                item_signal,
                {"kind", "line", "digest"},
                f"{source}.files[{index}].signals[{signal_index}]",
            )
            if signal["kind"] not in {"jazzy.css", "jazzy.js", 'class="dashanchor"', "//apple_ref/"}:
                fail(f"{source}: unexpected generated signal")
            if not is_int(signal["line"]) or signal["line"] < 1:
                fail(f"{source}: invalid generated signal line")
            expected_digest = hashlib.sha256(
                signal["kind"].encode() + b"\0" + str(signal["line"]).encode()
            ).hexdigest()
            if signal["digest"] != expected_digest:
                fail(f"{source}: generated signal digest drift")
            kinds.append(signal["kind"])
        if not ({"jazzy.css", "jazzy.js"} & set(kinds)) or not (
            {'class="dashanchor"', "//apple_ref/"} & set(kinds)
        ):
            fail(f"{source}: generated signal classes are incomplete")
        if live_sources:
            actual = jazzy_evidence(path_text, repo)
            if actual != evidence:
                fail(f"{source}: live generated evidence drift")
        evidence_paths.append(path_text)
    if len(evidence_paths) != len(set(evidence_paths)):
        fail(f"{source}: duplicate generated evidence file")
    expected_match = bool(member_files) and set(evidence_paths) == set(member_files) and all(
        path.lower().endswith(".html") for path in member_files
    )
    if record["matched"] != expected_match:
        fail(f"{source}: generated matched flag does not reproduce from evidence")
    return record


def validate_core(artifact: dict[str, Any], *, live_sources: bool = False) -> None:
    exact_keys(
        artifact,
        {
            "schema", "split", "heldout_policy", "query_schema_version", "provenance",
            "repositories", "summary", "head_rows", "deep_labeled_rows", "cross_tabs",
            "levers", "rejected_heuristics", "core_sha256",
        },
        "taxonomy core",
    )
    if artifact["schema"] != CORE_SCHEMA or artifact["split"] != "dev":
        fail("unsupported taxonomy core schema/split")
    if artifact["heldout_policy"] != HELDOUT_POLICY:
        fail("taxonomy held-out policy drift")
    if artifact["query_schema_version"] != QUERY_SCHEMA_VERSION:
        fail("taxonomy query schema version drift")
    corpus = load_json(CORPUS)
    runway = load_json(RUNWAY)
    dev_metadata = {
        row["id"]: row for row in corpus.get("repositories", []) if row.get("split") == "dev"
    }
    if len(dev_metadata) != 66 or set(artifact["repositories"]) != set(dev_metadata):
        fail("taxonomy repository set must equal the exact corpus dev split")
    runway_repos = runway.get("repositories")
    runway_sources = runway.get("source_files")
    if not isinstance(runway_repos, dict) or not isinstance(runway_sources, dict):
        fail("#840 runway repository/source bindings are absent")
    runway_by_key = {row["candidate_key"]: row for row in runway.get("candidates", [])}

    provenance = exact_keys(
        artifact["provenance"],
        {"command", "nose_binary", "nose_binary_sha256", "nose_version", "inputs"},
        "taxonomy provenance",
    )
    expected_inputs = (CORPUS, RUNWAY, V5_DEV, V6_DEV, V7_DEV, Path(__file__))
    expected_input_records = [
        {"path": rel(path), "sha256": file_sha256(path)} for path in expected_inputs
    ]
    if provenance["inputs"] != expected_input_records:
        fail("taxonomy provenance inputs differ from the current checked dev-only files")
    if provenance["command"] != FROZEN_COLLECT_COMMAND:
        fail("taxonomy collection command differs from the frozen invocation")
    runway_provenance = runway.get("provenance", {})
    for field in ("nose_binary", "nose_binary_sha256", "nose_version"):
        if provenance[field] != runway_provenance.get(field):
            fail(f"taxonomy provenance {field} differs from #840")

    for repo, record in artifact["repositories"].items():
        exact_keys(
            record,
            {"commit", "primary_language", "query_command", "query_stdout_sha256", "top_30_reported", "top_10_reported"},
            f"repositories.{repo}",
        )
        meta = dev_metadata[repo]
        frozen = runway_repos.get(repo, {})
        expected = {
            "commit": meta["commit"],
            "primary_language": meta["primary_language"],
            "query_command": shlex.join(
                [
                    provenance["nose_binary"], "query", rel(REPOS_ROOT / repo),
                    "top=30", "--format", "json",
                ]
            ),
            "query_stdout_sha256": frozen.get("query_stdout_sha256"),
            "top_30_reported": frozen.get("top_30_reported"),
            "top_10_reported": frozen.get("top_10_reported"),
        }
        if {key: record[key] for key in expected} != expected:
            fail(f"{repo}: repository record differs from corpus/#840")
        if live_sources and repository_head(REPOS_ROOT / repo) != record["commit"]:
            fail(f"{repo}: live repository commit drift")

    labels, labels_by_candidate = load_dev_labels()
    labels_by_repo: dict[str, list[dict[str, Any]]] = defaultdict(list)
    labels_by_id: dict[str, dict[str, Any]] = {}
    for label in labels:
        labels_by_repo[label["repo"]].append(label)
        if label["family_id"] in labels_by_id:
            fail(f"duplicate dev label family ID: {label['family_id']}")
        labels_by_id[label["family_id"]] = label

    rows = artifact["head_rows"]
    deep = artifact["deep_labeled_rows"]
    if not isinstance(rows, list) or len(rows) != 658 or not isinstance(deep, list) or len(deep) != 65:
        fail("taxonomy must contain exactly 658 head and 65 deep labeled rows")
    if canonical_sha256([row.get("position_key") for row in deep]) != (
        FROZEN_DEEP_LABELED_KEYS_SHA256
    ):
        fail("deep labeled rows differ from the frozen deterministic cohort")
    all_keys = [row.get("position_key") for row in [*rows, *deep]]
    if len(all_keys) != len(set(all_keys)):
        fail("taxonomy position keys must be globally unique")
    by_repo: dict[str, list[int]] = defaultdict(list)
    for index, row in enumerate([*rows, *deep]):
        exact_keys(
            row,
            {
                "position_key", "candidate_sha256", "repo", "rank", "primary_language",
                "query_family_id", "raw_family_sha256", "raw_family", "source_bounds",
                "truth", "facets", "predicate_results", "mechanical_bucket",
                "matched_levers", "selected_lever", "row_sha256",
            },
            f"rows[{index}]",
        )
        repo = row["repo"]
        candidate = runway_by_key.get(row["position_key"])
        if not isinstance(candidate, dict) or candidate.get("repo") != repo or candidate.get("rank") != row["rank"]:
            fail(f"{row['position_key']}: position differs from the #840 runway")
        if row["candidate_sha256"] != candidate.get("candidate_sha256"):
            fail(f"{row['position_key']}: candidate digest differs from #840")
        if row["query_family_id"] != candidate.get("family", {}).get("id"):
            fail(f"{row['position_key']}: family ID differs from #840")
        if row["raw_family_sha256"] != candidate.get("raw_family_sha256"):
            fail(f"{row['position_key']}: raw family digest differs from #840")
        if canonical_sha256(row["raw_family"]) != row["raw_family_sha256"]:
            fail(f"{row['position_key']}: raw family content digest mismatch")
        locations = row["raw_family"].get("locations")
        if not isinstance(locations, list) or not locations:
            fail(f"{row['position_key']}: raw family locations are absent")
        projected = [
            {
                "id": location.get("id"), "file": location["file"], "start": location["start"],
                "end": location["end"], "name": location.get("name"), "lang": location.get("lang"),
            }
            for location in locations
        ]
        bounds = row["source_bounds"]
        if not isinstance(bounds, list) or len(bounds) != len(projected):
            fail(f"{row['position_key']}: source bound count mismatch")
        for bound_index, bound in enumerate(bounds):
            checked = validate_source_bound(
                bound,
                repo=repo,
                source=f"{row['position_key']}.source_bounds[{bound_index}]",
                runway_sources=runway_sources,
                live_sources=live_sources,
            )
            if {key: checked[key] for key in projected[bound_index]} != projected[bound_index]:
                fail(f"{row['position_key']}: source bound differs from raw location")

        members = [member(location) for location in locations]
        label, overlap_count, runner_up = match_label(
            repo=repo,
            candidate_key=row["position_key"],
            members=members,
            labels_by_repo=labels_by_repo,
            labels_by_candidate=labels_by_candidate,
            labels_by_id=labels_by_id,
            preferred_family_id=candidate["family"].get("matched_v6_family_id"),
        )
        expected_truth = {
            "worthy": label["worthy"], "reason": label["reason"],
            "bucket": truth_bucket(label), "confidence": label.get("confidence"),
            "label_family_id": label["family_id"], "label_source": label["_label_source"],
            "label_sha256": label["_label_sha256"], "member_overlap": overlap_count,
            "runner_up_overlap": runner_up,
        }
        exact_keys(
            row["truth"],
            {"worthy", "reason", "bucket", "confidence", "label_family_id", "label_source", "label_sha256", "member_overlap", "runner_up_overlap"},
            f"{row['position_key']}.truth",
        )
        if row["truth"] != expected_truth:
            fail(f"{row['position_key']}: truth does not reproduce from dev labels")

        stored_generated = validate_generated_record(
            row["facets"].get("generated_provenance"),
            repo=repo,
            member_files=sorted({location["file"] for location in locations}),
            source=f"{row['position_key']}.generated_provenance",
            runway_sources=runway_sources,
            live_sources=live_sources,
        )
        expected_generated = generated_provenance(row["raw_family"], repo)
        if stored_generated != expected_generated:
            fail(f"{row['position_key']}: generated evidence differs from bounded source evidence")
        expected_facets = {
            "witness": row["raw_family"].get("witness"),
            "surface": row["raw_family"].get("surface"),
            "actionability_classifier": "unavailable-in-query-schema-v7",
            "scope": row["raw_family"].get("scope"),
            "extraction_shape": row["raw_family"].get("extraction_shape"),
            "same_symbol": row["raw_family"].get("same_symbol"),
            "source_comparable": row["raw_family"].get("source_comparable"),
            "origin": origin_facets(row["raw_family"]),
            "generated_provenance": stored_generated,
            **derived_features(row["raw_family"]),
        }
        exact_keys(row["facets"], set(expected_facets), f"{row['position_key']}.facets")
        if row["facets"] != expected_facets:
            fail(f"{row['position_key']}: facets do not reproduce from the raw family")
        expected_predicates = {
            "generated-provenance.v1": stored_generated["matched"],
            "declaration-only-type.v1": declaration_only_type(row["raw_family"]),
            **proof_flags(row["raw_family"]),
        }
        exact_keys(row["predicate_results"], set(expected_predicates), f"{row['position_key']}.predicates")
        if row["predicate_results"] != expected_predicates:
            fail(f"{row['position_key']}: predicate results do not reproduce")
        expected_bucket, expected_matches, expected_selected = classify_mechanical(
            label=label,
            generated=expected_predicates["generated-provenance.v1"],
            declaration=expected_predicates["declaration-only-type.v1"],
            proof=expected_predicates["proof_backed"],
        )
        if (
            row["mechanical_bucket"] != expected_bucket
            or row["matched_levers"] != expected_matches
            or row["selected_lever"] != expected_selected
        ):
            fail(f"{row['position_key']}: mechanical classification does not reproduce")
        if expected_selected and label["worthy"]:
            fail(f"{row['position_key']}: selected cohort contains a worthy row")
        content = {key: value for key, value in row.items() if key != "row_sha256"}
        if canonical_sha256(content) != row["row_sha256"]:
            fail(f"{row['position_key']}: row digest mismatch")
        if index < len(rows):
            by_repo[repo].append(row["rank"])

    for repo, ranks in by_repo.items():
        if sorted(ranks) != list(range(1, artifact["repositories"][repo]["top_10_reported"] + 1)):
            fail(f"{repo}: incomplete or duplicate head ranks")
    reasons = Counter(row["truth"]["reason"] for row in rows)
    if dict(reasons) != EXPECTED_TRUTH:
        fail(f"truth distribution drift: {dict(reasons)}")
    expected_summary = {
        "head_positions": 658, "deep_labeled_audit_pool": 65,
        "deep_mechanical_audit_positives": 24,
        "worthy": 382, "non_action": 276,
        "truth_reasons": dict(sorted(reasons.items())),
    }
    if artifact["summary"] != expected_summary:
        fail("taxonomy summary does not reproduce from its rows")
    if artifact["cross_tabs"] != build_cross_tabs(rows):
        fail("taxonomy cross-tabs do not reproduce from the head rows")
    if artifact["rejected_heuristics"] != rejected_heuristics(rows + deep):
        fail("rejected heuristic statistics do not reproduce from the labeled rows")

    contracts = {row["lever_id"]: row for row in lever_contracts()}
    levers = artifact["levers"]
    if not isinstance(levers, list) or {row.get("lever_id") for row in levers} != set(contracts):
        fail("taxonomy lever set differs from the frozen contracts")
    for lever in levers:
        contract = contracts[lever["lever_id"]]
        for key, expected in contract.items():
            if lever.get(key) != expected:
                fail(f"{lever['lever_id']}: frozen lever contract field {key} drift")
        if set(lever) != set(contract) | {
            "positive_position_keys", "head_movement", "head_non_action",
            "worthy_false_demotions", "head_false_demotion_rate", "repository_breadth",
            "member_language_breadth", "audit_packet_keys", "audit_packet_count",
            "audit_packet_set_sha256", "replacement_effect",
        }:
            fail(f"{lever['lever_id']}: lever schema drift")
        if lever["lever_id"] == "proof-actionability.v1":
            positives = [row for row in rows if row["predicate_results"]["proof_backed"]]
            expected_audit = 0
        else:
            positives = [row for row in rows if row["selected_lever"] == lever["lever_id"]]
            expected_audit = 20 if lever["lever_id"] == "generated-provenance.v1" else 4
        worthy = [row for row in positives if row["truth"]["worthy"]]
        expected_fields = {
            "positive_position_keys": [row["position_key"] for row in positives],
            "head_movement": len(positives),
            "head_non_action": len(positives) - len(worthy),
            "worthy_false_demotions": [
                {"position_key": row["position_key"], "truth_reason": row["truth"]["reason"], "reviewed_explanation": None}
                for row in worthy
            ],
            "head_false_demotion_rate": len(worthy) / len(positives) if positives else 0.0,
            "repository_breadth": sorted({row["repo"] for row in positives}),
            "member_language_breadth": sorted({lang for row in positives for lang in row["facets"]["actual_member_languages"]}),
            "audit_packet_count": expected_audit,
            "replacement_effect": {
                "vacated_head_slots": len(positives),
                "rank_11_replacements": "not modeled in #841; measured after product implementation",
            },
        }
        for key, expected in expected_fields.items():
            if lever[key] != expected:
                fail(f"{lever['lever_id']}: derived lever field {key} drift")
        keys = lever["audit_packet_keys"]
        if not isinstance(keys, list) or len(keys) != expected_audit or len(keys) != len(set(keys)):
            fail(f"{lever['lever_id']}: audit packet key count/uniqueness drift")
        if lever["audit_packet_set_sha256"] != FROZEN_LEVER_AUDIT_PACKET_SET_SHA256[
            lever["lever_id"]
        ]:
            fail(f"{lever['lever_id']}: audit cohort differs from the frozen commitment")
        if lever["lever_id"] != "proof-actionability.v1" and worthy:
            fail(f"{lever['lever_id']}: worthy row enters a selected/no-go demotion cohort")
    if canonical_sha256({key: value for key, value in artifact.items() if key != "core_sha256"}) != artifact["core_sha256"]:
        fail("taxonomy core digest mismatch")


def validate_audit_packet(
    packet: object,
    *,
    runway_by_key: dict[str, dict[str, Any]],
    runway_sources: dict[str, dict[str, Any]],
    live_sources: bool,
) -> dict[str, Any]:
    row = exact_keys(
        packet,
        {
            "audit_key", "lever_id", "candidate_sha256", "raw_family_sha256",
            "repo", "rank", "query_family_id", "source_bounds",
            "review_question", "frozen_evidence", "packet_sha256",
        },
        "audit packet",
    )
    candidate_key = row["audit_key"].removeprefix(row["lever_id"] + ":")
    candidate = runway_by_key.get(candidate_key)
    if not isinstance(candidate, dict):
        fail(f"{row['audit_key']}: packet candidate is absent from #840")
    expected_identity = {
        "candidate_sha256": candidate["candidate_sha256"],
        "raw_family_sha256": candidate["raw_family_sha256"],
        "repo": candidate["repo"],
        "rank": candidate["rank"],
        "query_family_id": candidate["family"]["id"],
    }
    if {key: row[key] for key in expected_identity} != expected_identity:
        fail(f"{row['audit_key']}: packet identity differs from #840")
    if row["lever_id"] not in {"generated-provenance.v1", "declaration-only-type.v1"}:
        fail(f"{row['audit_key']}: packet uses an unselected lever")
    if row["review_question"] != AUDIT_QUESTION:
        fail(f"{row['audit_key']}: review question drift")
    bounds = row["source_bounds"]
    members = candidate["family"]["members"]
    if not isinstance(bounds, list) or len(bounds) != len(members):
        fail(f"{row['audit_key']}: packet source bound count mismatch")
    for index, bound in enumerate(bounds):
        checked = validate_source_bound(
            bound,
            repo=row["repo"],
            source=f"{row['audit_key']}.source_bounds[{index}]",
            runway_sources=runway_sources,
            live_sources=live_sources,
        )
        compact = members[index]
        if {
            "file": checked["file"], "start_line": checked["start"],
            "end_line": checked["end"],
        } != {key: compact.get(key) for key in ("file", "start_line", "end_line")}:
            fail(f"{row['audit_key']}: packet bounds differ from #840")
        if compact.get("name") is not None and checked["name"] != compact["name"]:
            fail(f"{row['audit_key']}: packet member name differs from #840")
    evidence = exact_keys(
        row["frozen_evidence"],
        {"generated_provenance", "origin", "witness", "extraction_shape"},
        f"{row['audit_key']}.frozen_evidence",
    )
    exact_keys(
        evidence["origin"],
        {"coverage", "body_kinds", "region_kinds", "source_granularities", "subkinds", "domains", "evidence_flags"},
        f"{row['audit_key']}.origin",
    )
    generated = validate_generated_record(
        evidence["generated_provenance"],
        repo=row["repo"],
        member_files=sorted({bound["file"] for bound in bounds}),
        source=f"{row['audit_key']}.generated_provenance",
        runway_sources=runway_sources,
        live_sources=live_sources,
    )
    if evidence["witness"] != candidate["family"].get("witness") or evidence[
        "extraction_shape"
    ] != candidate["family"].get("extraction_shape"):
        fail(f"{row['audit_key']}: review facets differ from #840")
    if row["lever_id"] == "generated-provenance.v1" and not generated["matched"]:
        fail(f"{row['audit_key']}: generated audit packet does not satisfy its premise")
    if row["lever_id"] == "declaration-only-type.v1":
        origin = evidence["origin"]
        if not (
            origin["coverage"] == "all"
            and origin["body_kinds"] == ["declaration-only"]
            and "type-contract" in origin["domains"]
            and origin["source_granularities"] == ["whole-unit"]
            and {"declaration-only", "type-only"}.issubset(origin["evidence_flags"])
            and not {"runtime", "implementation-type", "data"}.intersection(origin["domains"])
            and not {"has-runtime-body", "has-reusable-body"}.intersection(
                origin["evidence_flags"]
            )
        ):
            fail(f"{row['audit_key']}: declaration audit packet does not satisfy its premise")
    if canonical_sha256({key: value for key, value in row.items() if key != "packet_sha256"}) != row["packet_sha256"]:
        fail(f"{row['audit_key']}: packet digest mismatch")
    return row


def validate_audit_artifact(
    core: dict[str, Any], audit: dict[str, Any], *, live_sources: bool = False
) -> None:
    exact_keys(
        audit,
        {"schema", "split", "core_sha256", "packet_set_sha256", "packets", "artifact_sha256"},
        "audit artifact",
    )
    if audit["schema"] != AUDIT_SCHEMA or audit["split"] != "dev" or audit["core_sha256"] != core["core_sha256"]:
        fail("audit artifact schema/split/core binding mismatch")
    if audit["packet_set_sha256"] != FROZEN_AUDIT_PACKET_SET_SHA256:
        fail("audit packet set differs from the frozen reviewed cohort")
    runway = load_json(RUNWAY)
    runway_by_key = {row["candidate_key"]: row for row in runway["candidates"]}
    packets = audit["packets"]
    if not isinstance(packets, list) or len(packets) != 24:
        fail("audit artifact must contain exactly 24 packets")
    checked = [
        validate_audit_packet(
            packet,
            runway_by_key=runway_by_key,
            runway_sources=runway["source_files"],
            live_sources=live_sources,
        )
        for packet in packets
    ]
    keys = [packet["audit_key"] for packet in checked]
    if len(keys) != len(set(keys)):
        fail("audit keys must be globally unique")
    if canonical_sha256(checked) != audit["packet_set_sha256"]:
        fail("audit packet set digest mismatch")
    expected_keys = [key for lever in core["levers"] for key in lever["audit_packet_keys"]]
    if keys != expected_keys:
        fail("audit packet order/keys differ from the core commitment")
    by_lever: dict[str, list[dict[str, Any]]] = defaultdict(list)
    for packet in checked:
        by_lever[packet["lever_id"]].append(packet)
    for lever in core["levers"]:
        if canonical_sha256(by_lever[lever["lever_id"]]) != lever["audit_packet_set_sha256"]:
            fail(f"{lever['lever_id']}: packet subset digest differs from core")
    if canonical_sha256({key: value for key, value in audit.items() if key != "artifact_sha256"}) != audit["artifact_sha256"]:
        fail("audit artifact digest mismatch")


def vote_template(
    core: dict[str, Any], audit: dict[str, Any], persona: str
) -> dict[str, Any]:
    if persona not in AUDIT_PERSONAS:
        fail(f"unknown audit persona: {persona}")
    items = []
    validate_audit_artifact(core, audit)
    for packet in audit["packets"]:
        items.append(
            {
                "audit_key": packet["audit_key"],
                "packet_sha256": packet["packet_sha256"],
                "premise_holds": None,
                "verdict": None,
                "rationale": "",
            }
        )
    return {
        "schema": VOTE_SCHEMA,
        "persona": persona,
        "core_sha256": core["core_sha256"],
        "audit_artifact_sha256": audit["artifact_sha256"],
        "audit_packet_set_sha256": audit["packet_set_sha256"],
        "items": items,
    }


def validate_vote(
    core: dict[str, Any], audit: dict[str, Any], vote: dict[str, Any], persona: str
) -> None:
    exact_keys(
        vote,
        {"schema", "persona", "core_sha256", "audit_artifact_sha256", "audit_packet_set_sha256", "items"},
        f"{persona} vote",
    )
    if vote["schema"] != VOTE_SCHEMA or vote["persona"] != persona:
        fail(f"{persona}: invalid vote schema or persona")
    template = vote_template(core, audit, persona)
    if vote["core_sha256"] != core["core_sha256"]:
        fail(f"{persona}: vote was cast against another taxonomy core")
    if vote["audit_artifact_sha256"] != audit["artifact_sha256"]:
        fail(f"{persona}: vote was cast against another audit artifact")
    if vote["audit_packet_set_sha256"] != template["audit_packet_set_sha256"]:
        fail(f"{persona}: audit packet set digest mismatch")
    expected = [(item["audit_key"], item["packet_sha256"]) for item in template["items"]]
    actual = [(item.get("audit_key"), item.get("packet_sha256")) for item in vote.get("items", [])]
    if actual != expected:
        fail(f"{persona}: vote items do not exactly match the frozen packet order")
    for item in vote["items"]:
        exact_keys(
            item,
            {"audit_key", "packet_sha256", "premise_holds", "verdict", "rationale"},
            f"{persona}/{item.get('audit_key')}",
        )
        if not isinstance(item.get("premise_holds"), bool):
            fail(f"{persona}/{item['audit_key']}: premise_holds must be boolean")
        if item.get("verdict") not in {"non-actionable", "actionable", "uncertain"}:
            fail(f"{persona}/{item['audit_key']}: invalid verdict")
        if not isinstance(item.get("rationale"), str) or not item["rationale"].strip():
            fail(f"{persona}/{item['audit_key']}: rationale is required")
        referenced_repos = set(re.findall(r"bench/repos/([^/\s]+)", item["rationale"]))
        if not referenced_repos.issubset(core["repositories"]):
            fail(f"{persona}/{item['audit_key']}: rationale references a non-dev repository path")


def rebind_vote(
    old_core: dict[str, Any],
    old_audit: dict[str, Any],
    old_vote: dict[str, Any],
    new_core: dict[str, Any],
    new_audit: dict[str, Any],
    persona: str,
) -> dict[str, Any]:
    """Carry the one reviewed vote set across a truth-free binding-only re-freeze."""

    if old_core.get("core_sha256") != REVIEWED_REBIND_SOURCE_CORE_SHA256 or canonical_sha256(
        {key: value for key, value in old_core.items() if key != "core_sha256"}
    ) != old_core.get("core_sha256"):
        fail(f"{persona}: source core is not the frozen reviewed core")
    if (
        old_audit.get("schema") != AUDIT_SCHEMA
        or old_audit.get("core_sha256") != old_core["core_sha256"]
        or old_audit.get("artifact_sha256") != REVIEWED_REBIND_SOURCE_AUDIT_SHA256
        or canonical_sha256(
            {key: value for key, value in old_audit.items() if key != "artifact_sha256"}
        )
        != old_audit.get("artifact_sha256")
        or old_audit.get("packet_set_sha256") != FROZEN_AUDIT_PACKET_SET_SHA256
        or canonical_sha256(old_audit.get("packets")) != FROZEN_AUDIT_PACKET_SET_SHA256
    ):
        fail(f"{persona}: source audit is not the frozen reviewed audit")
    if (
        old_vote.get("schema") != VOTE_SCHEMA
        or old_vote.get("persona") != persona
        or canonical_sha256(old_vote) != REVIEWED_REBIND_SOURCE_VOTE_SHA256[persona]
        or old_vote.get("core_sha256") != old_core["core_sha256"]
        or old_vote.get("audit_artifact_sha256") != old_audit["artifact_sha256"]
        or old_vote.get("audit_packet_set_sha256") != old_audit["packet_set_sha256"]
    ):
        fail(f"{persona}: source vote is not the frozen independently reviewed vote")
    validate_core(new_core)
    validate_audit_artifact(new_core, new_audit)
    old_packets = old_audit["packets"]
    new_packets = new_audit["packets"]
    if len(old_packets) != len(new_packets):
        fail(f"{persona}: packet count changed; fresh source review is required")

    def projection(packet: dict[str, Any]) -> dict[str, Any]:
        return {
            key: value
            for key, value in packet.items()
            if key
            not in {
                "candidate_sha256",
                "raw_family_sha256",
                "packet_sha256",
            }
        }

    if [projection(packet) for packet in old_packets] != [
        projection(packet) for packet in new_packets
    ]:
        fail(f"{persona}: reviewer-visible packet content changed; fresh review is required")
    old_items = old_vote.get("items")
    if not isinstance(old_items, list) or len(old_items) != len(old_packets):
        fail(f"{persona}: source vote item count mismatch")
    for item, packet in zip(old_items, old_packets, strict=True):
        exact_keys(
            item,
            {"audit_key", "packet_sha256", "premise_holds", "verdict", "rationale"},
            f"{persona} source vote item",
        )
        if item["audit_key"] != packet["audit_key"] or item["packet_sha256"] != packet["packet_sha256"]:
            fail(f"{persona}: source vote packet binding mismatch")
        if not isinstance(item["premise_holds"], bool) or item["verdict"] not in {
            "non-actionable", "actionable", "uncertain"
        } or not isinstance(item["rationale"], str) or not item["rationale"].strip():
            fail(f"{persona}: source vote judgment is incomplete")
    rebound = vote_template(new_core, new_audit, persona)
    for target, source_item in zip(rebound["items"], old_items, strict=True):
        target.update(
            premise_holds=source_item["premise_holds"],
            verdict=source_item["verdict"],
            rationale=source_item["rationale"],
        )
    validate_vote(new_core, new_audit, rebound, persona)
    return rebound


def hard_negative_record(row: dict[str, Any], boundary: str) -> dict[str, Any]:
    return {
        "position_key": row["position_key"],
        "truth_reason": row["truth"]["reason"],
        "boundary": boundary,
        "source_bounds_sha256": canonical_sha256(row["source_bounds"]),
    }


def bound_hard_negatives(artifact: dict[str, Any]) -> None:
    """Attach deterministic worthy boundary rows to every proposed lever."""

    head = artifact["head_rows"]
    generated = find_lever(artifact, "generated-provenance.v1")
    generated["hard_negatives"] = [
        hard_negative_record(row, "HTML is not generator provenance")
        for row in head
        if row["truth"]["worthy"]
        and any(
            location["file"].lower().endswith(".html")
            for location in row["raw_family"]["locations"]
        )
    ]

    partial_origin = [
        row
        for row in head
        if row["truth"]["worthy"] and row["facets"]["origin"]["coverage"] == "partial"
    ]
    missing_origin = [
        row
        for row in head
        if row["truth"]["worthy"] and row["facets"]["origin"]["coverage"] == "none"
    ][:3]
    runtime_body = [
        row
        for row in head
        if row["truth"]["worthy"]
        and "has-reusable-body" in row["facets"]["origin"]["evidence_flags"]
    ][:3]
    declaration = find_lever(artifact, "declaration-only-type.v1")
    declaration["hard_negatives"] = [
        *(
            hard_negative_record(row, "partial origin coverage must fail closed")
            for row in partial_origin
        ),
        *(
            hard_negative_record(row, "missing origin coverage must fail closed")
            for row in missing_origin
        ),
        *(
            hard_negative_record(row, "reusable implementation body must remain visible")
            for row in runtime_body
        ),
    ]

    proof = find_lever(artifact, "proof-actionability.v1")
    proof["hard_negatives"] = [
        hard_negative_record(row, "proof strength does not remove an actionable refactoring")
        for row in head
        if row["truth"]["worthy"] and row["predicate_results"]["proof_backed"]
    ]
    for lever in (generated, declaration, proof):
        lever["hard_negative_position_keys"] = [
            row["position_key"] for row in lever["hard_negatives"]
        ]
        lever["hard_negative_set_sha256"] = canonical_sha256(lever["hard_negatives"])
        lever["head_non_action_precision"] = (
            lever["head_non_action"] / lever["head_movement"]
            if lever["head_movement"]
            else None
        )


def build_final_overlay(
    core: dict[str, Any],
    core_path: Path,
    audit_artifact: dict[str, Any],
    audit_path: Path,
    vote_paths: dict[str, Path],
) -> dict[str, Any]:
    validate_core(core)
    validate_audit_artifact(core, audit_artifact)
    votes = {}
    for persona in AUDIT_PERSONAS:
        path = vote_paths[persona]
        vote = load_json(path)
        validate_vote(core, audit_artifact, vote, persona)
        votes[persona] = vote
    working = copy.deepcopy(core)
    bound_hard_negatives(working)
    split_rejected_heuristic_stats(working)
    audit = {
        "policy": "three independent source reviews; existing truth labels hidden from packets",
        "threshold": 0.90,
        "votes": [
            {"persona": persona, "path": rel(vote_paths[persona]), "sha256": file_sha256(vote_paths[persona])}
            for persona in AUDIT_PERSONAS
        ],
        "levers": {},
    }
    by_persona = {
        persona: {item["audit_key"]: item for item in vote["items"]}
        for persona, vote in votes.items()
    }
    decisions = []
    packets_by_lever: dict[str, list[dict[str, Any]]] = defaultdict(list)
    for packet in audit_artifact["packets"]:
        packets_by_lever[packet["lever_id"]].append(packet)
    for lever in working["levers"]:
        packets = packets_by_lever[lever["lever_id"]]
        if packets:
            summaries = {}
            for persona in AUDIT_PERSONAS:
                reviewed = [by_persona[persona][packet["audit_key"]] for packet in packets]
                non_action = sum(
                    item["premise_holds"] and item["verdict"] == "non-actionable"
                    for item in reviewed
                )
                precision = non_action / len(reviewed)
                summaries[persona] = {
                    "reviewed": len(reviewed),
                    "premise_holds": sum(item["premise_holds"] for item in reviewed),
                    "non_actionable": non_action,
                    "precision": precision,
                    "passed": precision >= 0.90,
                }
            passed = all(summary["passed"] for summary in summaries.values())
            lever["status"] = "selected-audit-passed" if passed else "rejected-audit-failed"
            lever["independent_audit"] = summaries
            audit["levers"][lever["lever_id"]] = summaries
            if not passed:
                fail(f"{lever['lever_id']}: independent audit did not reach 90% for every reviewer")
        decision = copy.deepcopy(lever)
        decisions.append(decision)
    artifact = {
        "schema": FINAL_SCHEMA,
        "split": "dev",
        "heldout_policy": core["heldout_policy"],
        "core_input": {
            "path": rel(core_path),
            "file_sha256": file_sha256(core_path),
            "core_sha256": core["core_sha256"],
        },
        "audit_input": {
            "path": rel(audit_path),
            "file_sha256": file_sha256(audit_path),
            "artifact_sha256": audit_artifact["artifact_sha256"],
        },
        "summary": core["summary"],
        "lever_decisions": decisions,
        "rejected_heuristics": working["rejected_heuristics"],
        "independent_audit": audit,
    }
    artifact["artifact_sha256"] = canonical_sha256(
        {key: value for key, value in artifact.items() if key != "artifact_sha256"}
    )
    return artifact


def finalize(
    core: dict[str, Any],
    core_path: Path,
    audit_artifact: dict[str, Any],
    audit_path: Path,
    vote_paths: dict[str, Path],
) -> dict[str, Any]:
    artifact = build_final_overlay(
        core, core_path, audit_artifact, audit_path, vote_paths
    )
    validate_final(artifact, vote_paths=vote_paths)
    return artifact


def validate_final(
    artifact: dict[str, Any],
    *,
    vote_paths: dict[str, Path] | None = None,
    live_sources: bool = False,
) -> None:
    exact_keys(
        artifact,
        {
            "schema", "split", "heldout_policy", "core_input", "audit_input",
            "summary", "lever_decisions", "rejected_heuristics", "independent_audit",
            "artifact_sha256",
        },
        "final taxonomy",
    )
    if artifact["schema"] != FINAL_SCHEMA or artifact["split"] != "dev":
        fail("unsupported final taxonomy schema")
    content = {key: value for key, value in artifact.items() if key != "artifact_sha256"}
    if canonical_sha256(content) != artifact.get("artifact_sha256"):
        fail("final taxonomy digest mismatch")
    core_input = artifact.get("core_input")
    if not isinstance(core_input, dict) or not isinstance(core_input.get("path"), str):
        fail("final taxonomy is not bound to a core input")
    core_path = (ROOT / core_input["path"]).resolve()
    try:
        core_path.relative_to(ROOT.resolve())
    except ValueError:
        fail("final taxonomy core path escapes the repository")
    if file_sha256(core_path) != core_input.get("file_sha256"):
        fail("final taxonomy core file digest mismatch")
    core = load_json(core_path)
    validate_core(core, live_sources=live_sources)
    if core["core_sha256"] != core_input.get("core_sha256"):
        fail("final taxonomy core semantic digest mismatch")
    audit_input = exact_keys(
        artifact["audit_input"],
        {"path", "file_sha256", "artifact_sha256"},
        "final taxonomy audit input",
    )
    audit_path = (ROOT / audit_input["path"]).resolve()
    try:
        audit_path.relative_to(ROOT.resolve())
    except ValueError:
        fail("final taxonomy audit path escapes the repository")
    if file_sha256(audit_path) != audit_input["file_sha256"]:
        fail("final taxonomy audit file digest mismatch")
    audit_artifact = load_json(audit_path)
    validate_audit_artifact(core, audit_artifact, live_sources=live_sources)
    if audit_artifact["artifact_sha256"] != audit_input["artifact_sha256"]:
        fail("final taxonomy audit semantic digest mismatch")
    decisions = {row["lever_id"]: row for row in artifact.get("lever_decisions", [])}
    for lever_id in ("generated-provenance.v1", "declaration-only-type.v1"):
        lever = decisions.get(lever_id, {})
        if lever.get("status") != "selected-audit-passed":
            fail(f"{lever_id}: selected classifier did not pass independent audit")
        summaries = lever.get("independent_audit", {})
        if set(summaries) != set(AUDIT_PERSONAS) or any(
            summary.get("precision", 0) < 0.90 for summary in summaries.values()
        ):
            fail(f"{lever_id}: missing independent 90% precision evidence")
    if set(decisions) != {
        "generated-provenance.v1",
        "declaration-only-type.v1",
        "proof-actionability.v1",
    }:
        fail("final taxonomy lever decision set mismatch")
    for lever in decisions.values():
        hard_negatives = lever.get("hard_negatives")
        if not isinstance(hard_negatives, list) or not hard_negatives:
            fail(f"{lever['lever_id']}: bound hard negatives are required")
        if canonical_sha256(hard_negatives) != lever.get("hard_negative_set_sha256"):
            fail(f"{lever['lever_id']}: hard-negative digest mismatch")
        if [row["position_key"] for row in hard_negatives] != lever.get(
            "hard_negative_position_keys"
        ):
            fail(f"{lever['lever_id']}: hard-negative key projection mismatch")
    recorded = {row["persona"]: row for row in artifact["independent_audit"]["votes"]}
    if set(recorded) != set(AUDIT_PERSONAS):
        fail("final taxonomy vote set mismatch")
    effective_paths = vote_paths or {
        persona: (ROOT / recorded[persona]["path"]).resolve()
        for persona in AUDIT_PERSONAS
    }
    for persona, path in effective_paths.items():
        if file_sha256(path) != recorded[persona]["sha256"]:
            fail(f"{persona}: live vote file digest mismatch")
        validate_vote(core, audit_artifact, load_json(path), persona)
    expected = build_final_overlay(
        core, core_path, audit_artifact, audit_path, effective_paths
    )
    if canonical_bytes(expected) != canonical_bytes(artifact):
        fail("final taxonomy does not reproduce from its bound core and votes")


def self_test() -> None:
    def expect_error(callback: Any, expected: str) -> None:
        try:
            callback()
        except TaxonomyError as error:
            assert expected in str(error), (expected, str(error))
        else:
            raise AssertionError(f"expected TaxonomyError containing {expected!r}")

    sample = {
        "id": "a",
        "locations": [
            {
                "file": "x.ts",
                "start": 1,
                "end": 4,
                "lang": "typescript",
                "origin": {
                    "body_kind": "declaration-only",
                    "domains": ["type-contract"],
                    "evidence_flags": ["declaration-only", "type-only"],
                    "source_granularity": "whole-unit",
                },
            }
        ],
    }
    assert declaration_only_type(sample)
    missing = copy.deepcopy(sample)
    del missing["locations"][0]["origin"]
    assert not declaration_only_type(missing)
    reusable = copy.deepcopy(sample)
    reusable["locations"][0]["origin"]["evidence_flags"].append("has-reusable-body")
    assert not declaration_only_type(reusable)
    cross = copy.deepcopy(sample)
    cross["locations"].append({**cross["locations"][0], "file": "y.rs", "lang": "rust"})
    cross.update(
        {
            "metrics": {"shared_weight": 3, "mean_lines": 4},
            "rep_lines": 4,
            "shared": 3,
            "params": 1,
            "removable": 2,
        }
    )
    assert derived_features(cross)["ranking_tightness"] is None
    label = {"worthy": True, "reason": "extract-helper", "family_id": "x"}
    assert truth_bucket(label) == "worthy.extract-helper"
    try:
        classify_mechanical(label=label, generated=True, declaration=True, proof=False)
    except TaxonomyError as error:
        assert "overlap" in str(error)
    else:
        raise AssertionError("overlapping selected cohorts must fail")
    expect_error(lambda: finite_number(True, "boolean"), "expected a number")
    assert safe_repo_relative_path("bench/repos/alacritty/src/a.rs", "alacritty")
    assert not safe_repo_relative_path(
        "bench/repos/alacritty/../../labels/secret.json", "alacritty"
    )
    expect_error(
        lambda: exact_keys({"allowed": 1, "gold": 2}, {"allowed"}, "exact"),
        "extra=['gold']",
    )

    v5_drift = load_json(V5_DEV)
    v5_drift["families"][0]["confidence"] = "self-test-drift"
    v5_drift["families_sha256"] = canonical_sha256(v5_drift["families"])
    expect_error(
        lambda: validate_v5_dev(v5_drift),
        "differs from the frozen parent dev subset",
    )

    if CHECKED_CORE.is_file() and CHECKED_AUDIT.is_file():
        checked_core = load_json(CHECKED_CORE)
        checked_audit = load_json(CHECKED_AUDIT)
        validate_core(checked_core)
        validate_audit_artifact(checked_core, checked_audit)

        wrong_source = copy.deepcopy(checked_core)
        wrong_source["head_rows"][0]["source_bounds"][0]["source_sha256"] = "0" * 64
        expect_error(lambda: validate_core(wrong_source), "differ from the #840 runway")

        escaping = copy.deepcopy(checked_core)
        escaping["head_rows"][0]["source_bounds"][0]["file"] = (
            "bench/repos/alacritty/../../labels/secret.json"
        )
        expect_error(lambda: validate_core(escaping), "outside its exact dev repository")

        raw_drift = copy.deepcopy(checked_core)
        raw_drift["head_rows"][0]["raw_family"]["surface"] = "generated"
        raw_drift["head_rows"][0]["raw_family_sha256"] = canonical_sha256(
            raw_drift["head_rows"][0]["raw_family"]
        )
        expect_error(lambda: validate_core(raw_drift), "differs from #840")

        bucket_drift = copy.deepcopy(checked_core)
        bucket_drift["head_rows"][0]["mechanical_bucket"] = "banana.not-a-bucket"
        expect_error(
            lambda: validate_core(bucket_drift),
            "mechanical classification does not reproduce",
        )

        provenance_drift = copy.deepcopy(checked_core)
        provenance_drift["provenance"]["inputs"][-1]["sha256"] = "0" * 64
        expect_error(
            lambda: validate_core(provenance_drift),
            "provenance inputs differ",
        )

        command_drift = copy.deepcopy(checked_core)
        command_drift["provenance"]["command"] = "python3 wrong.py collect"
        expect_error(
            lambda: validate_core(command_drift),
            "collection command differs from the frozen invocation",
        )

        query_command_drift = copy.deepcopy(checked_core)
        first_repo = next(iter(query_command_drift["repositories"]))
        query_command_drift["repositories"][first_repo]["query_command"] = "nose query elsewhere"
        expect_error(
            lambda: validate_core(query_command_drift),
            "repository record differs from corpus/#840",
        )

        deep_drift = copy.deepcopy(checked_core)
        deep_drift["deep_labeled_rows"][0], deep_drift["deep_labeled_rows"][1] = (
            deep_drift["deep_labeled_rows"][1],
            deep_drift["deep_labeled_rows"][0],
        )
        expect_error(
            lambda: validate_core(deep_drift),
            "deep labeled rows differ from the frozen deterministic cohort",
        )

        generated_drift = copy.deepcopy(checked_core)
        generated_row = next(
            row
            for row in generated_drift["head_rows"]
            if row["predicate_results"]["generated-provenance.v1"]
        )
        signal = generated_row["facets"]["generated_provenance"]["files"][0]["signals"][0]
        signal["line"] += 1
        signal["digest"] = hashlib.sha256(
            signal["kind"].encode() + b"\0" + str(signal["line"]).encode()
        ).hexdigest()
        expect_error(
            lambda: validate_core(generated_drift),
            "generated evidence differs from bounded source evidence",
        )

        packet_extra = copy.deepcopy(checked_audit)
        packet_extra["packets"][0]["gold"] = {"classification": "hidden"}
        expect_error(
            lambda: validate_audit_artifact(checked_core, packet_extra),
            "extra=['gold']",
        )

        packet_truth_hint = copy.deepcopy(checked_audit)
        packet_truth_hint["packets"][0]["source_bounds"][0]["name"] = (
            "worthy extract-helper"
        )
        packet_truth_hint["packets"][0]["packet_sha256"] = canonical_sha256(
            {
                key: value
                for key, value in packet_truth_hint["packets"][0].items()
                if key != "packet_sha256"
            }
        )
        packet_truth_hint["packet_set_sha256"] = canonical_sha256(
            packet_truth_hint["packets"]
        )
        packet_truth_hint["artifact_sha256"] = canonical_sha256(
            {
                key: value
                for key, value in packet_truth_hint.items()
                if key != "artifact_sha256"
            }
        )
        expect_error(
            lambda: validate_audit_artifact(checked_core, packet_truth_hint),
            "audit packet set differs from the frozen reviewed cohort",
        )

        expect_error(
            lambda: rebind_vote(
                {"core_sha256": "synthetic"},
                {},
                {},
                checked_core,
                checked_audit,
                "pragmatic",
            ),
            "source core is not the frozen reviewed core",
        )

        duplicate_core = copy.deepcopy(checked_core)
        duplicate_core["levers"][0]["audit_packet_keys"][1] = duplicate_core["levers"][0][
            "audit_packet_keys"
        ][0]
        expect_error(lambda: validate_core(duplicate_core), "key count/uniqueness drift")
    print("default-head taxonomy self-test passed")


def parser() -> argparse.ArgumentParser:
    result = argparse.ArgumentParser(description=__doc__)
    sub = result.add_subparsers(dest="command")
    freeze_parser = sub.add_parser("freeze-v5-dev")
    freeze_parser.add_argument("--output", type=Path, default=V5_DEV)
    collect_parser = sub.add_parser("collect")
    collect_parser.add_argument("--nose", type=Path, default=DEFAULT_NOSE)
    collect_parser.add_argument("--output", type=Path, required=True)
    collect_parser.add_argument("--audit-output", type=Path, required=True)
    validate_parser = sub.add_parser("validate")
    validate_parser.add_argument("artifact", type=Path)
    validate_parser.add_argument("--pragmatic", type=Path)
    validate_parser.add_argument("--dedupe", type=Path)
    validate_parser.add_argument("--skeptic", type=Path)
    validate_parser.add_argument("--live-sources", action="store_true")
    template_parser = sub.add_parser("vote-template")
    template_parser.add_argument("core", type=Path)
    template_parser.add_argument("audit", type=Path)
    template_parser.add_argument("--persona", choices=AUDIT_PERSONAS, required=True)
    template_parser.add_argument("--output", type=Path, required=True)
    rebind_parser = sub.add_parser("rebind-vote")
    rebind_parser.add_argument("old_core", type=Path)
    rebind_parser.add_argument("old_audit", type=Path)
    rebind_parser.add_argument("old_vote", type=Path)
    rebind_parser.add_argument("new_core", type=Path)
    rebind_parser.add_argument("new_audit", type=Path)
    rebind_parser.add_argument("--persona", choices=AUDIT_PERSONAS, required=True)
    rebind_parser.add_argument("--output", type=Path, required=True)
    finalize_parser = sub.add_parser("finalize")
    finalize_parser.add_argument("core", type=Path)
    finalize_parser.add_argument("audit", type=Path)
    finalize_parser.add_argument("--pragmatic", type=Path, required=True)
    finalize_parser.add_argument("--dedupe", type=Path, required=True)
    finalize_parser.add_argument("--skeptic", type=Path, required=True)
    finalize_parser.add_argument("--output", type=Path, required=True)
    result.add_argument("--self-test", action="store_true")
    return result


def main() -> None:
    args = parser().parse_args()
    try:
        if args.self_test:
            self_test()
        elif args.command == "freeze-v5-dev":
            artifact = freeze_v5_dev(args.output)
            print(f"wrote {args.output} ({len(artifact['families'])} dev labels)")
        elif args.command == "collect":
            artifact, audit = collect(args.nose.resolve())
            write_json(args.output, artifact)
            write_json(args.audit_output, audit)
            print(f"wrote {args.output} ({artifact['summary']['head_positions']} head rows)")
            print(f"wrote {args.audit_output} ({len(audit['packets'])} blind packets)")
        elif args.command == "vote-template":
            core = load_json(args.core)
            validate_core(core)
            audit = load_json(args.audit)
            validate_audit_artifact(core, audit)
            write_json(args.output, vote_template(core, audit, args.persona))
            print(f"wrote {args.output}")
        elif args.command == "rebind-vote":
            rebound = rebind_vote(
                load_json(args.old_core),
                load_json(args.old_audit),
                load_json(args.old_vote),
                load_json(args.new_core),
                load_json(args.new_audit),
                args.persona,
            )
            write_json(args.output, rebound)
            print(f"wrote {args.output} (review-visible packets unchanged)")
        elif args.command == "finalize":
            paths = {persona: getattr(args, persona) for persona in AUDIT_PERSONAS}
            artifact = finalize(
                load_json(args.core),
                args.core.resolve(),
                load_json(args.audit),
                args.audit.resolve(),
                paths,
            )
            write_json(args.output, artifact)
            print(f"wrote {args.output}")
        elif args.command == "validate":
            artifact = load_json(args.artifact)
            if artifact.get("schema") == CORE_SCHEMA:
                validate_core(artifact, live_sources=args.live_sources)
            elif artifact.get("schema") == AUDIT_SCHEMA:
                fail("validate an audit artifact through its bound final overlay")
            elif artifact.get("schema") == FINAL_SCHEMA:
                vote_paths = None
                if any(getattr(args, persona) for persona in AUDIT_PERSONAS):
                    if not all(getattr(args, persona) for persona in AUDIT_PERSONAS):
                        fail("provide all three vote paths together")
                    vote_paths = {persona: getattr(args, persona) for persona in AUDIT_PERSONAS}
                validate_final(
                    artifact,
                    vote_paths=vote_paths,
                    live_sources=args.live_sources,
                )
            else:
                fail("unsupported artifact schema")
            print(f"validated {args.artifact}")
        else:
            parser().print_help()
            raise SystemExit(2)
    except TaxonomyError as error:
        raise SystemExit(str(error)) from error


if __name__ == "__main__":
    main()
