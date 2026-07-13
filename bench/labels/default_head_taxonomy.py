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
import hashlib
import json
import math
from pathlib import Path
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

CORE_SCHEMA = "nose.default_head_taxonomy_core.v1"
FINAL_SCHEMA = "nose.default_head_taxonomy.v1"
VOTE_SCHEMA = "nose.default_head_taxonomy_vote.v1"
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


def load_dev_labels() -> tuple[list[dict[str, Any]], dict[str, dict[str, Any]]]:
    """Load only explicit dev components; never resolve a composite labelset."""

    rows: list[dict[str, Any]] = []
    by_candidate: dict[str, dict[str, Any]] = {}
    sources = ((V5, "v5-dev"), (V6_DEV, "v6-dev"), (V7_DEV, "v7-dev"))
    for path, source in sources:
        payload = load_json(path)
        families = payload.get("families")
        if not isinstance(families, list):
            fail(f"{path}: families must be an array")
        for original in families:
            if original.get("split") != "dev":
                if path == V5:
                    continue
                fail(f"{path}: non-dev row in an explicit dev component")
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


def jazzy_evidence(path_text: str, repo: str) -> dict[str, Any] | None:
    path = source_path(path_text, repo)
    bounded = path.read_bytes()[:SOURCE_READ_LIMIT]
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
        "source_bytes": path.stat().st_size,
        "source_sha256": file_sha256(path),
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
        path = source_path(location["file"], repo)
        records.append(
            {
                "id": location.get("id"),
                "file": location["file"],
                "start": location["start"],
                "end": location["end"],
                "name": location.get("name"),
                "lang": location.get("lang"),
                "source_bytes": path.stat().st_size,
                "source_sha256": file_sha256(path),
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


def rejected_heuristics(rows: list[dict[str, Any]]) -> list[dict[str, Any]]:
    rules = [
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
    return [
        {
            "heuristic_id": name,
            "status": "rejected",
            "predicate_ast": ast,
            "reason": reason,
            "dev_head": predicate_stat(rows, predicate),
        }
        for name, ast, predicate, reason in rules
    ]


def audit_packet(row: dict[str, Any], lever: str) -> dict[str, Any]:
    packet = {
        "audit_key": f"{lever}:{row['position_key']}",
        "lever_id": lever,
        "repo": row["repo"],
        "rank": row["rank"],
        "query_family_id": row["query_family_id"],
        "source_bounds": row["source_bounds"],
        "review_question": (
            "Does the frozen mechanical premise hold for every member, and would "
            "moving this family out of the bare default avoid hiding an actionable refactoring?"
        ),
        "frozen_evidence": {
            "generated_provenance": row["facets"]["generated_provenance"],
            "origin": row["facets"]["origin"],
            "witness": row["facets"]["witness"],
            "extraction_shape": row["facets"]["extraction_shape"],
        },
    }
    packet["packet_sha256"] = canonical_sha256({key: value for key, value in packet.items() if key != "packet_sha256"})
    return packet


def make_levers(head: list[dict[str, Any]], deep: list[dict[str, Any]]) -> list[dict[str, Any]]:
    definitions = [
        {
            "lever_id": "generated-provenance.v1",
            "status": "selected-pending-independent-audit",
            "predicate_ast": {
                "op": "all_unique_member_files",
                "suffix": ".html",
                "bounded_prefix_bytes": SOURCE_READ_LIMIT,
                "requires_any": [["jazzy.css", "jazzy.js"], ["class=\"dashAnchor\"", "//apple_ref/"]],
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
    result = []
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
                "audit_packets": audit,
                "audit_packet_set_sha256": canonical_sha256(audit),
                "replacement_effect": {
                    "vacated_head_slots": len(positives),
                    "rank_11_replacements": "not modeled in #841; measured after product implementation",
                },
            }
        )
        result.append(definition)
    return result


def collect(nose: Path) -> dict[str, Any]:
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
        command = [str(nose.resolve()), "query", rel(repo_path), "top=30", "--format", "json"]
        stdout = run(command)
        expected_stdout = runway_repos[repo]["query_stdout_sha256"]
        if hashlib.sha256(stdout).hexdigest() != expected_stdout:
            fail(f"{repo}: official v0.19.0 query stdout drifted from #840")
        families = query_families(stdout, source=f"#841 dev query {repo}")
        if len(families) != runway_repos[repo]["top_30_reported"]:
            fail(f"{repo}: top-30 count drift")
        repository_records[repo] = {
            "commit": commit,
            "primary_language": meta["primary_language"],
            "query_command": shlex.join(command),
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

    levers = make_levers(head_rows, deep_audit_rows)
    input_paths = (CORPUS, RUNWAY, V5, V6_DEV, V7_DEV, Path(__file__))
    artifact = {
        "schema": CORE_SCHEMA,
        "split": "dev",
        "heldout_policy": "closed; no held-out component, source path, or judgment was read",
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
    validate_core(artifact)
    return artifact


def find_lever(artifact: dict[str, Any], lever_id: str) -> dict[str, Any]:
    matches = [lever for lever in artifact.get("levers", []) if lever.get("lever_id") == lever_id]
    if len(matches) != 1:
        fail(f"expected exactly one {lever_id} lever")
    return matches[0]


def validate_core(artifact: dict[str, Any]) -> None:
    if artifact.get("schema") != CORE_SCHEMA:
        fail("unsupported taxonomy core schema")
    if artifact.get("split") != "dev" or len(artifact.get("repositories", {})) != 66:
        fail("taxonomy must contain exactly the 66 dev repositories")
    rows = artifact.get("head_rows")
    deep = artifact.get("deep_labeled_rows")
    if not isinstance(rows, list) or len(rows) != 658:
        fail("taxonomy must contain exactly 658 head rows")
    if not isinstance(deep, list) or len(deep) != 65:
        fail("taxonomy must contain exactly 65 frozen labeled deep rows")
    keys = [row.get("position_key") for row in rows]
    if len(keys) != len(set(keys)):
        fail("head position keys must be unique")
    by_repo: dict[str, list[int]] = defaultdict(list)
    for row in rows:
        by_repo[row["repo"]].append(row["rank"])
        raw_hash = canonical_sha256(row["raw_family"])
        if raw_hash != row.get("raw_family_sha256"):
            fail(f"{row.get('position_key')}: raw family digest mismatch")
        content = {key: value for key, value in row.items() if key != "row_sha256"}
        if canonical_sha256(content) != row.get("row_sha256"):
            fail(f"{row.get('position_key')}: row digest mismatch")
        if row["truth"]["bucket"] != (
            ("worthy." if row["truth"]["worthy"] else "non_action.") + row["truth"]["reason"]
        ):
            fail(f"{row['position_key']}: truth bucket mismatch")
        selected = row.get("selected_lever")
        matches = row.get("matched_levers")
        if not isinstance(matches, list) or len(matches) > 1 or selected not in (*matches, None):
            fail(f"{row['position_key']}: selected lever overlap or mismatch")
        if selected and row["truth"]["worthy"]:
            fail(f"{row['position_key']}: worthy row lacks a reviewed demotion explanation")
        if row["facets"]["ownership_relation"] != "unknown":
            fail(f"{row['position_key']}: ownership was inferred without evidence")
        if row["predicate_results"]["declaration-only-type.v1"]:
            if row["facets"]["origin"]["coverage"] != "all":
                fail(f"{row['position_key']}: declaration cohort must fail closed")
    for repo, ranks in by_repo.items():
        expected = list(range(1, artifact["repositories"][repo]["top_10_reported"] + 1))
        if sorted(ranks) != expected:
            fail(f"{repo}: incomplete or duplicate head ranks")
    reasons = Counter(row["truth"]["reason"] for row in rows)
    if dict(reasons) != EXPECTED_TRUTH:
        fail(f"truth distribution drift: {dict(reasons)}")
    if artifact.get("summary", {}).get("worthy") != 382:
        fail("worthy summary must remain 382/658")
    generated = find_lever(artifact, "generated-provenance.v1")
    declaration = find_lever(artifact, "declaration-only-type.v1")
    proof = find_lever(artifact, "proof-actionability.v1")
    if generated.get("head_movement") != 10 or declaration.get("head_movement") != 1:
        fail("selected bounded cohort head movement drifted")
    if proof.get("status") != "rejected-no-go":
        fail("proof/actionability blanket removal must remain a no-go")
    for lever in (generated, declaration):
        if lever.get("worthy_false_demotions"):
            fail(f"{lever['lever_id']}: selected cohort contains a worthy row")
        packets = lever.get("audit_packets")
        expected_count = 20 if lever is generated else 4
        if not isinstance(packets, list) or len(packets) != expected_count:
            fail(f"{lever['lever_id']}: expected {expected_count} audit packets")
        if canonical_sha256(packets) != lever.get("audit_packet_set_sha256"):
            fail(f"{lever['lever_id']}: audit packet set digest mismatch")
        for packet in packets:
            content = {key: value for key, value in packet.items() if key != "packet_sha256"}
            if canonical_sha256(content) != packet.get("packet_sha256"):
                fail(f"{packet.get('audit_key')}: audit packet digest mismatch")
    core = {key: value for key, value in artifact.items() if key != "core_sha256"}
    if canonical_sha256(core) != artifact.get("core_sha256"):
        fail("taxonomy core digest mismatch")
    serialized = json.dumps(artifact, sort_keys=True)
    forbidden = ("refactoring_families.v6.heldout", "refactoring_families.v7.heldout", "heldout.seal")
    if any(value in serialized for value in forbidden):
        fail("taxonomy artifact contains a held-out input or source reference")


def vote_template(core: dict[str, Any], persona: str) -> dict[str, Any]:
    if persona not in AUDIT_PERSONAS:
        fail(f"unknown audit persona: {persona}")
    items = []
    for lever in core["levers"]:
        for packet in lever.get("audit_packets", []):
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
        "audit_packet_set_sha256": canonical_sha256(
            [packet for lever in core["levers"] for packet in lever.get("audit_packets", [])]
        ),
        "items": items,
    }


def validate_vote(core: dict[str, Any], vote: dict[str, Any], persona: str) -> None:
    if vote.get("schema") != VOTE_SCHEMA or vote.get("persona") != persona:
        fail(f"{persona}: invalid vote schema or persona")
    template = vote_template(core, persona)
    if vote.get("core_sha256") != core["core_sha256"]:
        fail(f"{persona}: vote was cast against another taxonomy core")
    if vote.get("audit_packet_set_sha256") != template["audit_packet_set_sha256"]:
        fail(f"{persona}: audit packet set digest mismatch")
    expected = [(item["audit_key"], item["packet_sha256"]) for item in template["items"]]
    actual = [(item.get("audit_key"), item.get("packet_sha256")) for item in vote.get("items", [])]
    if actual != expected:
        fail(f"{persona}: vote items do not exactly match the frozen packet order")
    for item in vote["items"]:
        if not isinstance(item.get("premise_holds"), bool):
            fail(f"{persona}/{item['audit_key']}: premise_holds must be boolean")
        if item.get("verdict") not in {"non-actionable", "actionable", "uncertain"}:
            fail(f"{persona}/{item['audit_key']}: invalid verdict")
        if not isinstance(item.get("rationale"), str) or not item["rationale"].strip():
            fail(f"{persona}/{item['audit_key']}: rationale is required")


def finalize(core: dict[str, Any], vote_paths: dict[str, Path]) -> dict[str, Any]:
    validate_core(core)
    votes = {}
    for persona in AUDIT_PERSONAS:
        path = vote_paths[persona]
        vote = load_json(path)
        validate_vote(core, vote, persona)
        votes[persona] = vote
    artifact = copy.deepcopy(core)
    artifact["schema"] = FINAL_SCHEMA
    artifact.pop("core_sha256")
    artifact["core_input_sha256"] = core["core_sha256"]
    artifact["independent_audit"] = {
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
    for lever in artifact["levers"]:
        packets = lever.get("audit_packets", [])
        if not packets:
            continue
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
        artifact["independent_audit"]["levers"][lever["lever_id"]] = summaries
        if not passed:
            fail(f"{lever['lever_id']}: independent audit did not reach 90% for every reviewer")
    artifact["artifact_sha256"] = canonical_sha256(
        {key: value for key, value in artifact.items() if key != "artifact_sha256"}
    )
    validate_final(artifact, vote_paths=vote_paths)
    return artifact


def validate_final(artifact: dict[str, Any], *, vote_paths: dict[str, Path] | None = None) -> None:
    if artifact.get("schema") != FINAL_SCHEMA:
        fail("unsupported final taxonomy schema")
    content = {key: value for key, value in artifact.items() if key != "artifact_sha256"}
    if canonical_sha256(content) != artifact.get("artifact_sha256"):
        fail("final taxonomy digest mismatch")
    if artifact.get("core_input_sha256") is None:
        fail("final taxonomy is not bound to a core input")
    for lever_id in ("generated-provenance.v1", "declaration-only-type.v1"):
        lever = find_lever(artifact, lever_id)
        if lever.get("status") != "selected-audit-passed":
            fail(f"{lever_id}: selected classifier did not pass independent audit")
        summaries = lever.get("independent_audit", {})
        if set(summaries) != set(AUDIT_PERSONAS) or any(
            summary.get("precision", 0) < 0.90 for summary in summaries.values()
        ):
            fail(f"{lever_id}: missing independent 90% precision evidence")
    if vote_paths is not None:
        recorded = {row["persona"]: row for row in artifact["independent_audit"]["votes"]}
        for persona, path in vote_paths.items():
            if file_sha256(path) != recorded[persona]["sha256"]:
                fail(f"{persona}: live vote file digest mismatch")


def self_test() -> None:
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
    assert not isinstance(True, float)
    print("default-head taxonomy self-test passed")


def parser() -> argparse.ArgumentParser:
    result = argparse.ArgumentParser(description=__doc__)
    sub = result.add_subparsers(dest="command")
    collect_parser = sub.add_parser("collect")
    collect_parser.add_argument("--nose", type=Path, default=DEFAULT_NOSE)
    collect_parser.add_argument("--output", type=Path, required=True)
    validate_parser = sub.add_parser("validate")
    validate_parser.add_argument("artifact", type=Path)
    validate_parser.add_argument("--pragmatic", type=Path)
    validate_parser.add_argument("--dedupe", type=Path)
    validate_parser.add_argument("--skeptic", type=Path)
    template_parser = sub.add_parser("vote-template")
    template_parser.add_argument("core", type=Path)
    template_parser.add_argument("--persona", choices=AUDIT_PERSONAS, required=True)
    template_parser.add_argument("--output", type=Path, required=True)
    finalize_parser = sub.add_parser("finalize")
    finalize_parser.add_argument("core", type=Path)
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
        elif args.command == "collect":
            artifact = collect(args.nose.resolve())
            write_json(args.output, artifact)
            print(f"wrote {args.output} ({artifact['summary']['head_positions']} head rows)")
        elif args.command == "vote-template":
            core = load_json(args.core)
            validate_core(core)
            write_json(args.output, vote_template(core, args.persona))
            print(f"wrote {args.output}")
        elif args.command == "finalize":
            paths = {persona: getattr(args, persona) for persona in AUDIT_PERSONAS}
            artifact = finalize(load_json(args.core), paths)
            write_json(args.output, artifact)
            print(f"wrote {args.output}")
        elif args.command == "validate":
            artifact = load_json(args.artifact)
            if artifact.get("schema") == CORE_SCHEMA:
                validate_core(artifact)
            elif artifact.get("schema") == FINAL_SCHEMA:
                vote_paths = None
                if any(getattr(args, persona) for persona in AUDIT_PERSONAS):
                    if not all(getattr(args, persona) for persona in AUDIT_PERSONAS):
                        fail("provide all three vote paths together")
                    vote_paths = {persona: getattr(args, persona) for persona in AUDIT_PERSONAS}
                validate_final(artifact, vote_paths=vote_paths)
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
