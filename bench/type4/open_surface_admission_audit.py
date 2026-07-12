#!/usr/bin/env python3
"""Audit open Type-4 language surfaces for proof-backed admission."""

from __future__ import annotations

import argparse
import json
import re
import sys
from collections import Counter, defaultdict
from pathlib import Path
from typing import Any

from semantic_pattern_cards import (
    DEFAULT_CARDS,
    DEFAULT_FOCUSED_CASES,
    DEFAULT_PROOF_FACT_REGISTRY,
    DEFAULT_TARGET_PACKETS,
    PatternCardError,
    load_json,
    repo_rel,
    validate_cards,
)

HERE = Path(__file__).resolve().parent
ROOT = HERE.parents[1]

SCHEMA_VERSION = 1
TOOL_VERSION = "open-surface-admission-audit/1"
DEFAULT_JSON_OUT = HERE / "open_surface_admission_audit.v1.json"
DEFAULT_MARKDOWN_OUT = HERE / "open_surface_admission_audit.md"

AUDITED_SURFACE_STATUSES = {"open"}
MODELED_FACT_STATUSES = {"modeled-controlled"}
PRIORITY_ORDER = {
    "proof-fact-ready": 0,
    "probe-to-focused-candidate": 1,
    "needs-surface-focused-perimeter": 2,
    "blocked-by-unmodeled-facts": 3,
}
ACTIONABLE_PRIORITIES = {
    "proof-fact-ready",
    "probe-to-focused-candidate",
    "needs-surface-focused-perimeter",
}

EPIC_778_ACTIONABLE_ROWS = [
    {
        "order": 1,
        "issue": 779,
        "pattern_id": "collection.membership.proven-receiver-element",
        "language": "Swift",
        "candidate_priority": "probe-to-focused-candidate",
        "work": "promote Swift collection membership probe evidence into focused admission",
    },
    {
        "order": 2,
        "issue": 780,
        "pattern_id": "collection.empty-check.proven-receiver-domain",
        "language": "Swift",
        "candidate_priority": "probe-to-focused-candidate",
        "work": "promote Swift collection empty-check probe evidence into focused admission",
    },
    {
        "order": 3,
        "issue": 782,
        "pattern_id": "string.affix.proven-receiver-coordinate",
        "language": "Swift",
        "candidate_priority": "probe-to-focused-candidate",
        "work": "complete the Swift hasPrefix/hasSuffix focused perimeter",
    },
    {
        "order": 4,
        "issue": 784,
        "pattern_id": "quantifier.universal.counterexample-loop",
        "language": "JavaScript",
        "candidate_priority": "needs-surface-focused-perimeter",
        "work": "add JavaScript Array.prototype.every focused fixtures and expectations",
    },
    {
        "order": 5,
        "issue": 783,
        "pattern_id": "quantifier.universal.counterexample-loop",
        "language": "Rust",
        "candidate_priority": "needs-surface-focused-perimeter",
        "work": "add Rust Iterator::all focused fixtures and expectations",
    },
    {
        "order": 6,
        "issue": 724,
        "pattern_id": "numeric.clamp.proven-integer-bounds",
        "language": "Go",
        "candidate_priority": "needs-surface-focused-perimeter",
        "work": "connect Go numeric clamp bound-order and integer-domain proof evidence",
    },
]

EPIC_778_OUT_OF_SCOPE_ROWS = [
    {
        "pattern_id": "hof.filter-map.option-emission",
        "language": "Swift",
        "reason": "needs optional-result channel and callback-effect facts",
    },
    {
        "pattern_id": "hof.flat-map.aggregate-reduction",
        "language": "Java",
        "reason": "one-level flat-map source facts are modeled; aggregate-guard coordinates remain",
    },
    {
        "pattern_id": "hof.flat-map.aggregate-reduction",
        "language": "Swift",
        "reason": "one-level flat-map source facts are modeled; aggregate-guard coordinates remain",
    },
    {
        "pattern_id": "hof.flat-map.one-level-flatten",
        "language": "Swift",
        "reason": "needs one-level flatten, nested-order, emitted-value, and callback facts",
    },
    {
        "pattern_id": "map.default.absence-lookup",
        "language": "Swift",
        "reason": "needs dictionary receiver, key/fallback coordinate, and mutation facts",
    },
    {
        "pattern_id": "option.presence-default.proven-channel-coordinate",
        "language": "Ruby",
        "reason": "needs absence-channel identity before nil? admission",
    },
    {
        "pattern_id": "option.presence-default.proven-channel-coordinate",
        "language": "Swift",
        "reason": "needs absence-channel identity before Optional presence/defaulting admission",
    },
]

EPIC_791_BLOCKED_ROWS = [
    {
        "order": 1,
        "issue": 793,
        "pattern_id": "option.presence-default.proven-channel-coordinate",
        "language": "Ruby",
        "candidate_priority": "blocked-by-unmodeled-facts",
        "planned_unmodeled_facts": ["option.absence-channel.identity"],
        "work": "model absence-channel identity before Ruby nil?/nil comparison admission",
        "reason": "only the absence-channel fact is unmodeled, but the focused Ruby nil? perimeter is not sound until that fact lands",
    },
    {
        "order": 2,
        "issue": 793,
        "pattern_id": "option.presence-default.proven-channel-coordinate",
        "language": "Swift",
        "candidate_priority": "blocked-by-unmodeled-facts",
        "planned_unmodeled_facts": ["option.absence-channel.identity"],
        "work": "model absence-channel identity before Swift Optional presence/defaulting admission",
        "reason": "probe evidence exists, but Optional presence/defaulting still needs a proven absence channel before focused admission",
    },
    {
        "order": 3,
        "issue": 795,
        "pattern_id": "hof.filter-map.option-emission",
        "language": "Swift",
        "candidate_priority": "blocked-by-unmodeled-facts",
        "planned_unmodeled_facts": [
            "effect.pure-callback",
            "hof.filter-map.drop-condition-coordinate",
            "hof.filter-map.emitted-value-coordinate",
            "option.absence-channel.identity",
        ],
        "work": "model Swift compactMap drop-condition and emitted-value coordinates",
        "reason": "after option channel and callback purity, compactMap still needs filter-map coordinate facts before admission",
    },
    {
        "order": 4,
        "issue": 796,
        "pattern_id": "hof.flat-map.one-level-flatten",
        "language": "Swift",
        "candidate_priority": "blocked-by-unmodeled-facts",
        "planned_unmodeled_facts": [
            "effect.pure-callback",
            "hof.flat-map.nested-iteration-order",
            "hof.flat-map.emitted-value-coordinate",
            "collection.flatten-depth.one-level",
        ],
        "work": "model one-level flatten, nested-order, and emitted-value facts for Swift flatMap",
        "reason": "Sequence.flatMap admission requires a reusable one-level flatten proof, not Swift API spelling alone",
    },
    {
        "order": 5,
        "issue": 797,
        "pattern_id": "hof.flat-map.aggregate-reduction",
        "language": "Java",
        "candidate_priority": "blocked-by-unmodeled-facts",
        "planned_unmodeled_facts": [
            "effect.pure-callback",
            "hof.flat-map.nested-iteration-order",
            "hof.flat-map.emitted-value-coordinate",
            "collection.flatten-depth.one-level",
            "hof.flat-map.aggregate-guard-coordinate",
        ],
        "work": "connect Java Stream.flatMap aggregate reductions using the modeled one-level source facts",
        "reason": "one-level traversal and emitted-stream facts are modeled; aggregate guard placement is the remaining proof gap",
    },
    {
        "order": 6,
        "issue": 797,
        "pattern_id": "hof.flat-map.aggregate-reduction",
        "language": "Swift",
        "candidate_priority": "blocked-by-unmodeled-facts",
        "planned_unmodeled_facts": [
            "effect.pure-callback",
            "hof.flat-map.nested-iteration-order",
            "hof.flat-map.emitted-value-coordinate",
            "collection.flatten-depth.one-level",
            "hof.flat-map.aggregate-guard-coordinate",
        ],
        "work": "connect Swift flatMap terminal aggregates using the modeled one-level source facts",
        "reason": "the flattened element stream is now reusable; aggregate guard placement remains unproven",
    },
    {
        "order": 7,
        "issue": 798,
        "pattern_id": "map.default.absence-lookup",
        "language": "Swift",
        "candidate_priority": "blocked-by-unmodeled-facts",
        "planned_unmodeled_facts": [
            "map.default.absence-fallback",
            "map.receiver.source-identity",
            "map.default.key-fallback-coordinate",
            "map.receiver.no-intervening-mutation",
        ],
        "work": "model Swift Dictionary default lookup receiver, key, fallback, and mutation facts",
        "reason": "default subscript admission needs absent-key fallback and receiver-coordinate proof instead of subscript spelling",
    },
]

EPIC_791_FACT_GROUPS = [
    {
        "order": 1,
        "issue": 793,
        "group_id": "option.absence-channel",
        "title": "Option absence-channel identity",
        "facts": ["option.absence-channel.identity"],
        "unblocks": [
            "option.presence-default.proven-channel-coordinate:Ruby",
            "option.presence-default.proven-channel-coordinate:Swift",
            "hof.filter-map.option-emission:Swift",
        ],
        "focused_admission_after_group_lands": [
            "option.presence-default.proven-channel-coordinate:Ruby",
            "option.presence-default.proven-channel-coordinate:Swift",
        ],
        "still_open_until": [
            "hof.filter-map.option-emission:Swift waits for callback purity and filter-map coordinates",
        ],
    },
    {
        "order": 2,
        "issue": 794,
        "group_id": "effect.hof-callback-purity",
        "title": "Higher-order callback effect safety",
        "facts": ["effect.pure-callback"],
        "unblocks": [
            "hof.filter-map.option-emission:Swift",
            "hof.flat-map.one-level-flatten:Swift",
            "hof.flat-map.aggregate-reduction:Java",
            "hof.flat-map.aggregate-reduction:Swift",
        ],
        "focused_admission_after_group_lands": [],
        "still_open_until": [
            "compactMap waits for option-emission coordinate facts",
            "flatMap waits for one-level flatten and nested traversal facts",
        ],
    },
    {
        "order": 3,
        "issue": 795,
        "group_id": "hof.filter-map.coordinates",
        "title": "Filter-map drop and emitted-value coordinates",
        "facts": [
            "hof.filter-map.drop-condition-coordinate",
            "hof.filter-map.emitted-value-coordinate",
        ],
        "unblocks": ["hof.filter-map.option-emission:Swift"],
        "focused_admission_after_group_lands": ["hof.filter-map.option-emission:Swift"],
        "still_open_until": [],
    },
    {
        "order": 4,
        "issue": 796,
        "group_id": "hof.flat-map.one-level-stream",
        "title": "One-level flat-map source and emitted stream",
        "facts": [
            "collection.flatten-depth.one-level",
            "hof.flat-map.nested-iteration-order",
            "hof.flat-map.emitted-value-coordinate",
        ],
        "unblocks": [
            "hof.flat-map.one-level-flatten:Swift",
            "hof.flat-map.aggregate-reduction:Java",
            "hof.flat-map.aggregate-reduction:Swift",
        ],
        "focused_admission_after_group_lands": ["hof.flat-map.one-level-flatten:Swift"],
        "still_open_until": [
            "flat-map aggregate reductions wait for aggregate guard-coordinate proof",
        ],
    },
    {
        "order": 5,
        "issue": 797,
        "group_id": "hof.flat-map.aggregate-guard",
        "title": "Flat-map aggregate guard coordinate",
        "facts": ["hof.flat-map.aggregate-guard-coordinate"],
        "unblocks": [
            "hof.flat-map.aggregate-reduction:Java",
            "hof.flat-map.aggregate-reduction:Swift",
        ],
        "focused_admission_after_group_lands": [
            "hof.flat-map.aggregate-reduction:Java",
            "hof.flat-map.aggregate-reduction:Swift",
        ],
        "still_open_until": [],
    },
    {
        "order": 6,
        "issue": 798,
        "group_id": "map.default.receiver-fallback",
        "title": "Map default receiver and fallback coordinates",
        "facts": [
            "map.default.absence-fallback",
            "map.receiver.source-identity",
            "map.default.key-fallback-coordinate",
            "map.receiver.no-intervening-mutation",
        ],
        "unblocks": ["map.default.absence-lookup:Swift"],
        "focused_admission_after_group_lands": ["map.default.absence-lookup:Swift"],
        "still_open_until": [],
    },
    {
        "order": 7,
        "issue": 799,
        "group_id": "blocked-surface-closeout",
        "title": "Blocked-surface closeout and replay evidence",
        "facts": [],
        "unblocks": [],
        "focused_admission_after_group_lands": [],
        "still_open_until": [
            "any row still open after #793-#798 must carry stronger blocker or replay evidence",
        ],
    },
]


class OpenSurfaceAuditError(RuntimeError):
    pass


def case_indexes(focused_cases: dict[str, Any]) -> tuple[dict[str, str], dict[str, dict[str, int]]]:
    kinds: dict[str, str] = {}
    by_family: dict[str, dict[str, int]] = defaultdict(lambda: {"positive": 0, "hard_negative": 0})
    for case in focused_cases.get("cases", []):
        if not isinstance(case, dict):
            continue
        case_id = case.get("id")
        family = case.get("semantic_family")
        kind = case.get("kind")
        if not isinstance(case_id, str) or not isinstance(family, str):
            continue
        if kind == "positive":
            kinds[case_id] = "positive"
            by_family[family]["positive"] += 1
        elif kind == "hard-negative":
            kinds[case_id] = "hard_negative"
            by_family[family]["hard_negative"] += 1
    for group in focused_cases.get("hard_negative_groups", []):
        if not isinstance(group, dict):
            continue
        group_id = group.get("id")
        family = group.get("semantic_family")
        if not isinstance(group_id, str) or not isinstance(family, str):
            continue
        positive_count = len(group.get("positive_cases") or [])
        hard_negative_count = len(group.get("hard_negative_cases") or [])
        kinds[group_id] = "hard_negative_group"
        by_family[family]["positive"] += positive_count
        by_family[family]["hard_negative"] += hard_negative_count
    return kinds, by_family


def fact_statuses(proof_fact_registry: dict[str, Any]) -> dict[str, str]:
    statuses: dict[str, str] = {}
    for fact in proof_fact_registry.get("facts", []):
        if not isinstance(fact, dict):
            continue
        fact_id = fact.get("fact_id")
        status = fact.get("status")
        if isinstance(fact_id, str) and isinstance(status, str):
            statuses[fact_id] = status
    return statuses


def target_packet_ids(target_packets: dict[str, Any]) -> set[str]:
    return {
        packet["packet_id"]
        for packet in target_packets.get("packets", [])
        if isinstance(packet, dict) and isinstance(packet.get("packet_id"), str)
    }


def coverage_indexes(coverage_evidence: dict[str, Any]) -> dict[tuple[str, str], list[dict[str, Any]]]:
    indexes: dict[tuple[str, str], list[dict[str, Any]]] = defaultdict(list)
    for row_id, item in enumerate(coverage_evidence.get("evidence", [])):
        if not isinstance(item, dict):
            continue
        axis = item.get("axis")
        language = item.get("language")
        if not isinstance(axis, str) or not isinstance(language, str):
            continue
        indexes[(axis, language.casefold())].append({
            "row_id": row_id,
            "axis": axis,
            "gen_axis": item.get("gen_axis") or "",
            "language": language.casefold(),
            "source": item.get("source") or "",
            "status": item.get("status") or "",
            "positive_hits": int(item.get("pos_hit") or 0),
            "positives": int(item.get("pos") or 0),
            "hard_negatives": int(item.get("neg") or 0),
            "false_merges": int(item.get("false_merges") or 0),
        })
    return indexes


def unresolved_surface_evidence_refs(
    surface: dict[str, Any],
    case_kind_by_id: dict[str, str],
    coverage_by_axis_language: dict[tuple[str, str], list[dict[str, Any]]],
    allowed_target_packet_ids: set[str],
) -> list[str]:
    evidence = str(surface.get("evidence") or "")
    unresolved: list[str] = []
    for ref in surface_refs(surface):
        ref_id = local_id(ref)
        if ref_id not in case_kind_by_id:
            unresolved.append(ref)
    for match in re.finditer(
        r"bench/type4/frontier_target_packets\.v1\.json::([A-Za-z0-9_.-]+)",
        evidence,
    ):
        packet_id = match.group(1)
        if packet_id not in allowed_target_packet_ids:
            unresolved.append(match.group(0))
    for match in re.finditer(
        r"bench/type4/coverage_evidence\.v1\.json::(?:probe:)?([A-Za-z0-9_-]+)(?:/([A-Za-z0-9_-]+))?",
        evidence,
    ):
        axis = match.group(1)
        language = match.group(2)
        if language:
            key = (axis, normalize_language_key(language))
            if key not in coverage_by_axis_language:
                unresolved.append(match.group(0))
        elif not any(key_axis == axis for key_axis, _ in coverage_by_axis_language):
            unresolved.append(match.group(0))
    for match in re.finditer(r"bench/type4/coverage_probes/[A-Za-z0-9_./-]+", evidence):
        probe_path = match.group(0).rstrip(".,;:")
        path = Path(probe_path)
        if not path.is_absolute():
            path = ROOT / path
        if not path.exists():
            unresolved.append(probe_path)
    return unresolved


def local_id(ref: str) -> str | None:
    if "::" not in ref:
        return None
    return ref.split("::", 1)[1]


def focused_counts_from_refs(refs: list[str], case_kind_by_id: dict[str, str]) -> dict[str, int]:
    counts = {"positive": 0, "hard_negative": 0, "hard_negative_group": 0}
    for ref in refs:
        if not ref.startswith("bench/type4/adversarial/cases/cases.v1.json::"):
            continue
        ref_id = local_id(ref)
        if ref_id is None:
            continue
        kind = case_kind_by_id.get(ref_id)
        if kind in counts:
            counts[kind] += 1
    return counts


def surface_refs(surface: dict[str, Any]) -> list[str]:
    evidence = str(surface.get("evidence") or "")
    return re.findall(r"bench/type4/adversarial/cases/cases\.v1\.json::[A-Za-z0-9_.-]+", evidence)


def has_focused_boundary(support: dict[str, int]) -> bool:
    return (
        support["positive"] > 0
        and (support["hard_negative"] + support["hard_negative_group"]) > 0
    )


def evidence_level(surface: dict[str, Any], coverage: dict[str, Any] | None = None) -> str:
    if coverage:
        sources = set(coverage.get("sources") or [])
        gen_axes = [str(axis) for axis in coverage.get("gen_axes") or []]
        if "probe" in sources or any(axis.startswith("probe:") for axis in gen_axes):
            return "probe-only"
        if sources:
            return "coverage-sweep"
    evidence = str(surface.get("evidence") or "")
    lowered = evidence.casefold()
    if "bench/type4/adversarial/cases/cases.v1.json::" in evidence:
        return "focused-surface"
    if "coverage_probes" in lowered or "probe:" in lowered or " probe " in lowered:
        return "probe-only"
    if "coverage_evidence.v1.json" in evidence:
        return "coverage-sweep"
    if "frontier_target_packets.v1.json::" in evidence:
        return "target-packet"
    if evidence:
        return "documented"
    return "missing"


def likely_blocker(evidence_level_value: str, unmodeled_facts: list[str]) -> str:
    if unmodeled_facts:
        return "model required facts before detector admission"
    if evidence_level_value == "probe-only":
        return "promote probe evidence into focused positives and hard negatives"
    if evidence_level_value == "coverage-sweep":
        return "convert sweep coverage into focused executable perimeter"
    if evidence_level_value == "target-packet":
        return "connect packet evidence to focused executable expectations"
    if evidence_level_value == "missing":
        return "add surface-specific focused fixtures and executable expectations"
    return "attach focused admission gates for this language surface"


def candidate_priority(
    evidence_level_value: str,
    unmodeled_facts: list[str],
    surface_focused_support: dict[str, int],
    coverage: dict[str, Any] | None,
) -> str:
    if unmodeled_facts:
        return "blocked-by-unmodeled-facts"
    if has_focused_boundary(surface_focused_support):
        return "proof-fact-ready"
    if evidence_level_value in {"probe-only", "coverage-sweep"} and coverage:
        if coverage["probe_ready"]:
            return "probe-to-focused-candidate"
    return "needs-surface-focused-perimeter"


def normalize_language_key(language: str) -> str:
    folded = language.casefold().strip()
    if folded == "js":
        return "javascript"
    if folded == "ts":
        return "typescript"
    return folded


def language_keys(language: str) -> list[str]:
    folded = language.casefold()
    if folded == "js/ts":
        return ["javascript", "typescript"]
    if "/" in folded:
        return [
            normalize_language_key(part)
            for part in folded.split("/")
            if part.strip()
        ]
    return [normalize_language_key(folded)]


def explicit_coverage_queries(evidence: str) -> list[tuple[str, str | None]]:
    queries: list[tuple[str, str | None]] = []
    for match in re.finditer(
        r"coverage_evidence\.v1\.json::(?:probe:)?([A-Za-z0-9_-]+)(?:/([A-Za-z0-9_-]+))?",
        evidence,
    ):
        language = match.group(2)
        queries.append(
            (
                match.group(1),
                normalize_language_key(language) if language else None,
            )
        )
    for match in re.finditer(r"coverage_probes/([A-Za-z0-9_-]+)/([A-Za-z0-9_-]+)", evidence):
        queries.append((match.group(1), normalize_language_key(match.group(2))))
    for match in re.finditer(r"probe:([A-Za-z0-9_-]+)(?:/([A-Za-z0-9_-]+))?", evidence):
        language = match.group(2)
        queries.append(
            (
                match.group(1),
                normalize_language_key(language) if language else None,
            )
        )
    return list(dict.fromkeys(queries))


def aggregate_coverage(
    rows: list[dict[str, Any]],
    language_components: list[str],
    matched_languages: set[str],
) -> dict[str, Any]:
    statuses = sorted({str(row["status"]) for row in rows if row["status"]})
    sources = sorted({str(row["source"]) for row in rows if row["source"]})
    gen_axes = sorted({str(row["gen_axis"]) for row in rows if row["gen_axis"]})
    missing_languages = [
        language for language in language_components if language not in matched_languages
    ]
    positive_hits = sum(int(row["positive_hits"]) for row in rows)
    positives = sum(int(row["positives"]) for row in rows)
    hard_negatives = sum(int(row["hard_negatives"]) for row in rows)
    false_merges = sum(int(row["false_merges"]) for row in rows)
    return {
        "row_count": len(rows),
        "statuses": statuses,
        "sources": sources,
        "gen_axes": gen_axes,
        "language_components": language_components,
        "matched_languages": sorted(matched_languages),
        "missing_languages": missing_languages,
        "positive_hits": positive_hits,
        "positives": positives,
        "hard_negatives": hard_negatives,
        "false_merges": false_merges,
        "probe_ready": (
            bool(rows)
            and not missing_languages
            and statuses == ["covered"]
            and positives > 0
            and positive_hits == positives
            and hard_negatives > 0
            and false_merges == 0
        ),
    }


def surface_coverage(
    pattern: dict[str, Any],
    surface: dict[str, Any],
    coverage_by_axis_language: dict[tuple[str, str], list[dict[str, Any]]],
) -> dict[str, Any] | None:
    pattern_id = pattern["pattern_id"]
    evidence = str(surface.get("evidence") or "")
    explicit_queries = explicit_coverage_queries(evidence)
    if explicit_queries:
        axis_language_queries = [
            (axis, [language] if language else language_keys(surface["language"]))
            for axis, language in explicit_queries
        ]
    else:
        axis_candidates = [
            pattern_id,
            pattern_id.replace(".", "_"),
            pattern_id.split(".")[0],
        ]
        for ref in pattern.get("evidence_refs", []):
            ref_id = local_id(ref)
            if ref_id:
                axis_candidates.append(ref_id)
                axis_candidates.append(ref_id.replace("-", "_"))
        axis_language_queries = [
            (axis, language_keys(surface["language"]))
            for axis in dict.fromkeys(axis_candidates)
        ]
    coverage_rows: list[dict[str, Any]] = []
    matched_languages: set[str] = set()
    seen_row_ids: set[int] = set()
    languages = language_keys(surface["language"])
    for axis, query_languages in axis_language_queries:
        for language in query_languages:
            for coverage in coverage_by_axis_language.get((axis, language), []):
                row_id = int(coverage["row_id"])
                if row_id in seen_row_ids:
                    continue
                seen_row_ids.add(row_id)
                matched_languages.add(language)
                coverage_rows.append(coverage)
    if not coverage_rows:
        return None
    return aggregate_coverage(coverage_rows, languages, matched_languages)


def build_report(
    patterns: list[dict[str, Any]],
    proof_fact_registry: dict[str, Any],
    focused_cases: dict[str, Any],
    target_packets: dict[str, Any],
    coverage_evidence: dict[str, Any],
) -> dict[str, Any]:
    fact_status_by_id = fact_statuses(proof_fact_registry)
    case_kind_by_id, focused_by_family = case_indexes(focused_cases)
    allowed_target_packet_ids = target_packet_ids(target_packets)
    coverage_by_axis_language = coverage_indexes(coverage_evidence)

    rows: list[dict[str, Any]] = []
    for pattern in patterns:
        focused_counts = focused_counts_from_refs(pattern["evidence_refs"], case_kind_by_id)
        family_counts = focused_by_family.get(pattern["pattern_id"], {"positive": 0, "hard_negative": 0})
        pattern_focused_support = {
            "positive": focused_counts["positive"] + family_counts["positive"],
            "hard_negative": focused_counts["hard_negative"] + family_counts["hard_negative"],
            "hard_negative_group": focused_counts["hard_negative_group"],
        }
        required_facts = pattern["required_facts"]
        fact_statuses_for_pattern = {
            fact_id: fact_status_by_id.get(fact_id, "missing")
            for fact_id in required_facts
        }
        unmodeled_facts = [
            fact_id
            for fact_id, status in fact_statuses_for_pattern.items()
            if status not in MODELED_FACT_STATUSES
        ]

        for surface in pattern["language_surfaces"]:
            if surface["status"] not in AUDITED_SURFACE_STATUSES:
                continue
            coverage = surface_coverage(pattern, surface, coverage_by_axis_language)
            surface_focused_support = focused_counts_from_refs(surface_refs(surface), case_kind_by_id)
            level = evidence_level(surface, coverage)
            unresolved_refs = unresolved_surface_evidence_refs(
                surface,
                case_kind_by_id,
                coverage_by_axis_language,
                allowed_target_packet_ids,
            )
            row = {
                "pattern_id": pattern["pattern_id"],
                "pattern_status": pattern["status"],
                "language": surface["language"],
                "surface_status": surface["status"],
                "surface": surface["surface"],
                "required_facts": required_facts,
                "fact_statuses": fact_statuses_for_pattern,
                "unmodeled_facts": unmodeled_facts,
                "evidence_level": level,
                "surface_evidence": surface.get("evidence") or "",
                "surface_focused_support": surface_focused_support,
                "pattern_focused_support": pattern_focused_support,
                "coverage": coverage or {},
                "unresolved_surface_evidence_refs": unresolved_refs,
                "likely_blocker": likely_blocker(level, unmodeled_facts),
            }
            row["candidate_priority"] = candidate_priority(
                level,
                unmodeled_facts,
                surface_focused_support,
                coverage,
            )
            rows.append(row)

    rows.sort(
        key=lambda row: (
            PRIORITY_ORDER.get(row["candidate_priority"], 99),
            row["pattern_id"],
            row["language"],
        )
    )
    by_status = Counter(row["surface_status"] for row in rows)
    by_priority = Counter(row["candidate_priority"] for row in rows)
    by_evidence_level = Counter(row["evidence_level"] for row in rows)
    by_language_count = Counter(row["language"] for row in rows)
    unresolved_ref_count = sum(
        len(row["unresolved_surface_evidence_refs"]) for row in rows
    )
    by_fact: dict[str, list[dict[str, str]]] = defaultdict(list)
    by_pattern: dict[str, list[dict[str, str]]] = defaultdict(list)
    by_blocker: dict[str, list[dict[str, str]]] = defaultdict(list)
    by_language: dict[str, list[dict[str, str]]] = defaultdict(list)
    by_surface_status: dict[str, list[dict[str, str]]] = defaultdict(list)
    for row in rows:
        entry = group_entry(row)
        by_pattern[row["pattern_id"]].append(entry)
        by_blocker[row["likely_blocker"]].append(entry)
        by_language[row["language"]].append(entry)
        by_surface_status[row["surface_status"]].append(entry)
        for fact_id in row["required_facts"]:
            by_fact[fact_id].append(entry)

    epic_778_slice = build_epic_778_slice(rows)
    epic_791_slice = build_epic_791_slice(rows, fact_status_by_id)

    return {
        "schema_version": SCHEMA_VERSION,
        "tool_version": TOOL_VERSION,
        "summary": {
            "audited_surface_statuses": sorted(AUDITED_SURFACE_STATUSES),
            "open_surface_count": len(rows),
            "by_status": dict(sorted(by_status.items())),
            "by_priority": dict(sorted(by_priority.items())),
            "by_evidence_level": dict(sorted(by_evidence_level.items())),
            "by_language": dict(sorted(by_language_count.items())),
            "unresolved_surface_evidence_ref_count": unresolved_ref_count,
        },
        "by_pattern": {key: value for key, value in sorted(by_pattern.items())},
        "by_blocker": {key: value for key, value in sorted(by_blocker.items())},
        "by_fact": {key: value for key, value in sorted(by_fact.items())},
        "by_language": {key: value for key, value in sorted(by_language.items())},
        "by_surface_status": {key: value for key, value in sorted(by_surface_status.items())},
        "epic_slices": {
            "epic_778": epic_778_slice,
            "epic_791": epic_791_slice,
        },
        "rows": rows,
    }


def group_entry(row: dict[str, Any]) -> dict[str, str]:
    return {
        "pattern_id": row["pattern_id"],
        "language": row["language"],
        "surface_status": row["surface_status"],
        "candidate_priority": row["candidate_priority"],
        "likely_blocker": row["likely_blocker"],
        "surface": row["surface"],
    }


def group_entry_label(entry: dict[str, str]) -> str:
    return (
        f"{entry['pattern_id']}:{entry['language']}:{entry['surface_status']}:"
        f"{entry['candidate_priority']}:{entry['likely_blocker']}"
    )


def row_selector(pattern_id: str, language: str, candidate_priority_value: str) -> str:
    return f"{pattern_id}:{language}:{candidate_priority_value}"


def row_surface_key(pattern_id: str, language: str) -> str:
    return f"{pattern_id}:{language}"


def epic_row_record(
    spec: dict[str, Any],
    row: dict[str, Any] | None,
    *,
    current_open_audit_state: str = "present",
) -> dict[str, Any]:
    record = {
        "selector": row_selector(
            spec["pattern_id"],
            spec["language"],
            spec["candidate_priority"],
        ),
        "pattern_id": spec["pattern_id"],
        "language": spec["language"],
        "candidate_priority": spec["candidate_priority"],
        "current_open_audit_state": "not-in-current-open-audit",
    }
    if "issue" in spec:
        record["issue"] = spec["issue"]
    if "order" in spec:
        record["order"] = spec["order"]
    if "work" in spec:
        record["work"] = spec["work"]
    if "reason" in spec:
        record["reason"] = spec["reason"]
    if "planned_unmodeled_facts" in spec:
        record["planned_unmodeled_facts"] = spec["planned_unmodeled_facts"]
    if row is None:
        return record
    record.update({
        "current_open_audit_state": current_open_audit_state,
        "current_candidate_priority": row["candidate_priority"],
        "surface_status": row["surface_status"],
        "surface": row["surface"],
        "evidence_level": row["evidence_level"],
        "likely_blocker": row["likely_blocker"],
        "unmodeled_facts": row["unmodeled_facts"],
        "surface_focused_support": row["surface_focused_support"],
        "pattern_focused_support": row["pattern_focused_support"],
    })
    return record


def build_epic_778_slice(rows: list[dict[str, Any]]) -> dict[str, Any]:
    rows_by_selector: dict[str, dict[str, Any]] = {}
    rows_by_surface: dict[str, list[dict[str, Any]]] = defaultdict(list)
    errors: list[str] = []
    for row in rows:
        selector = row_selector(
            row["pattern_id"],
            row["language"],
            row["candidate_priority"],
        )
        if selector in rows_by_selector:
            errors.append(f"duplicate open audit row selector: {selector}")
        rows_by_selector[selector] = row
        rows_by_surface[row_surface_key(row["pattern_id"], row["language"])].append(row)

    in_scope: list[dict[str, Any]] = []
    in_scope_selectors = {
        row_selector(spec["pattern_id"], spec["language"], spec["candidate_priority"])
        for spec in EPIC_778_ACTIONABLE_ROWS
    }
    in_scope_surface_keys = {
        row_surface_key(spec["pattern_id"], spec["language"])
        for spec in EPIC_778_ACTIONABLE_ROWS
    }
    for spec in EPIC_778_ACTIONABLE_ROWS:
        selector = row_selector(
            spec["pattern_id"],
            spec["language"],
            spec["candidate_priority"],
        )
        surface_key = row_surface_key(spec["pattern_id"], spec["language"])
        row = rows_by_selector.get(selector)
        current_state = "present"
        if row is None:
            current_surface_rows = rows_by_surface.get(surface_key, [])
            if current_surface_rows:
                row = current_surface_rows[0]
                current_state = "present-with-different-priority"
            for current_row in current_surface_rows:
                if current_row["candidate_priority"] == "blocked-by-unmodeled-facts":
                    errors.append(
                        f"epic #778 in-scope row is now blocked by unmodeled facts: "
                        f"{surface_key}"
                    )
        elif row["unmodeled_facts"]:
            errors.append(
                f"epic #778 in-scope row has unmodeled facts: {selector}"
            )
        in_scope.append(
            epic_row_record(spec, row, current_open_audit_state=current_state)
        )

    out_of_scope: list[dict[str, Any]] = []
    for spec in EPIC_778_OUT_OF_SCOPE_ROWS:
        spec = {
            **spec,
            "candidate_priority": "blocked-by-unmodeled-facts",
        }
        selector = row_selector(
            spec["pattern_id"],
            spec["language"],
            spec["candidate_priority"],
        )
        row = rows_by_selector.get(selector)
        current_state = "present"
        if row is None:
            current_surface_rows = rows_by_surface.get(
                row_surface_key(spec["pattern_id"], spec["language"]),
                [],
            )
            if current_surface_rows:
                row = current_surface_rows[0]
                current_state = "present-with-different-priority"
        out_of_scope.append(
            epic_row_record(spec, row, current_open_audit_state=current_state)
        )

    current_actionable = [
        row
        for row in rows
        if row["candidate_priority"] in ACTIONABLE_PRIORITIES
    ]
    unexpected_actionable = [
        group_entry(row)
        for row in current_actionable
        if row_selector(row["pattern_id"], row["language"], row["candidate_priority"])
        not in in_scope_selectors
        and row_surface_key(row["pattern_id"], row["language"])
        not in in_scope_surface_keys
    ]
    current_blocked = [
        row
        for row in rows
        if row["candidate_priority"] == "blocked-by-unmodeled-facts"
    ]
    return {
        "issue": 778,
        "title": "Audit-ready focused admissions for open Type-4 surfaces",
        "setup_issue": 781,
        "closeout_issue": 785,
        "source": "bench/type4/open_surface_admission_audit.v1.json",
        "scope_policy": {
            "in_scope_priorities": [
                "probe-to-focused-candidate",
                "needs-surface-focused-perimeter",
            ],
            "current_actionable_priorities": sorted(ACTIONABLE_PRIORITIES),
            "out_of_scope_priorities": ["blocked-by-unmodeled-facts"],
            "blocked_rows_require_fact_modeling_epic": True,
        },
        "summary": {
            "in_scope_count": len(in_scope),
            "in_scope_currently_open": sum(
                1
                for row in in_scope
                if row["current_open_audit_state"] != "not-in-current-open-audit"
            ),
            "out_of_scope_count": len(out_of_scope),
            "out_of_scope_currently_open": sum(
                1
                for row in out_of_scope
                if row["current_open_audit_state"] != "not-in-current-open-audit"
            ),
            "current_actionable_open_count": len(current_actionable),
            "current_blocked_open_count": len(current_blocked),
            "unexpected_actionable_open_count": len(unexpected_actionable),
            "validation_error_count": len(errors),
        },
        "in_scope": sorted(in_scope, key=lambda row: int(row["order"])),
        "out_of_scope": sorted(
            out_of_scope,
            key=lambda row: (row["pattern_id"], row["language"]),
        ),
        "unexpected_actionable_open_rows": unexpected_actionable,
        "validation_errors": errors,
    }


def build_epic_791_slice(
    rows: list[dict[str, Any]],
    fact_status_by_id: dict[str, str],
) -> dict[str, Any]:
    rows_by_selector: dict[str, dict[str, Any]] = {}
    rows_by_surface: dict[str, list[dict[str, Any]]] = defaultdict(list)
    errors: list[str] = []
    for row in rows:
        selector = row_selector(
            row["pattern_id"],
            row["language"],
            row["candidate_priority"],
        )
        if selector in rows_by_selector:
            errors.append(f"duplicate open audit row selector: {selector}")
        rows_by_selector[selector] = row
        rows_by_surface[row_surface_key(row["pattern_id"], row["language"])].append(row)

    frozen_selectors: set[str] = set()
    frozen_surface_keys: set[str] = set()
    planned_facts_by_surface: dict[str, set[str]] = {}
    planned_fact_ids = {
        fact_id
        for group in EPIC_791_FACT_GROUPS
        for fact_id in group["facts"]
    }
    for spec in EPIC_791_BLOCKED_ROWS:
        selector = row_selector(
            spec["pattern_id"],
            spec["language"],
            spec["candidate_priority"],
        )
        surface_key = row_surface_key(spec["pattern_id"], spec["language"])
        if selector in frozen_selectors:
            errors.append(f"duplicate epic #791 frozen selector: {selector}")
        if surface_key in frozen_surface_keys:
            errors.append(f"duplicate epic #791 frozen surface: {surface_key}")
        frozen_selectors.add(selector)
        frozen_surface_keys.add(surface_key)
        planned_row_facts = set(spec.get("planned_unmodeled_facts", []))
        unplanned_spec_facts = sorted(planned_row_facts - planned_fact_ids)
        if unplanned_spec_facts:
            errors.append(
                f"epic #791 row {surface_key} references facts outside planned "
                f"fact groups: {', '.join(unplanned_spec_facts)}"
            )
        planned_facts_by_surface[surface_key] = planned_row_facts
    frozen_rows: list[dict[str, Any]] = []
    for spec in EPIC_791_BLOCKED_ROWS:
        selector = row_selector(
            spec["pattern_id"],
            spec["language"],
            spec["candidate_priority"],
        )
        surface_key = row_surface_key(spec["pattern_id"], spec["language"])
        row = rows_by_selector.get(selector)
        current_state = "present"
        if row is None:
            current_surface_rows = rows_by_surface.get(surface_key, [])
            if current_surface_rows:
                row = current_surface_rows[0]
                current_state = "present-with-different-priority"
        if row is not None and row["candidate_priority"] == "blocked-by-unmodeled-facts":
            current_unmodeled_facts = set(row.get("unmodeled_facts", []))
            planned_row_facts = planned_facts_by_surface[surface_key]
            facts_outside_plan = sorted(current_unmodeled_facts - planned_row_facts)
            if facts_outside_plan:
                errors.append(
                    f"epic #791 row {surface_key} is blocked by unplanned fact(s): "
                    + ", ".join(facts_outside_plan)
                )
            modeled_blockers = sorted(
                fact_id
                for fact_id in current_unmodeled_facts
                if fact_status_by_id.get(fact_id) in MODELED_FACT_STATUSES
            )
            if modeled_blockers:
                errors.append(
                    f"epic #791 row {surface_key} is still blocked by modeled fact(s): "
                    + ", ".join(modeled_blockers)
                )
        frozen_rows.append(
            epic_row_record(spec, row, current_open_audit_state=current_state)
        )

    current_blocked = [
        row
        for row in rows
        if row["candidate_priority"] == "blocked-by-unmodeled-facts"
    ]
    current_actionable = [
        row
        for row in rows
        if row["candidate_priority"] in ACTIONABLE_PRIORITIES
    ]
    unexpected_blocked = [
        group_entry(row)
        for row in current_blocked
        if row_selector(row["pattern_id"], row["language"], row["candidate_priority"])
        not in frozen_selectors
        and row_surface_key(row["pattern_id"], row["language"])
        not in frozen_surface_keys
    ]
    if unexpected_blocked:
        errors.extend(
            "unexpected blocked open row outside epic #791 slice: "
            + group_entry_label(row)
            for row in unexpected_blocked
        )

    fact_groups: list[dict[str, Any]] = []
    for group in EPIC_791_FACT_GROUPS:
        for fact_id in group["facts"]:
            if fact_id not in fact_status_by_id:
                errors.append(
                    f"epic #791 fact group {group['group_id']} references "
                    f"unknown proof fact: {fact_id}"
                )
        for surface_key in (
            group["unblocks"] + group["focused_admission_after_group_lands"]
        ):
            if surface_key not in frozen_surface_keys:
                errors.append(
                    f"epic #791 fact group {group['group_id']} references "
                    f"unknown frozen surface: {surface_key}"
                )
        if group["facts"]:
            group_fact_ids = set(group["facts"])
            for surface_key in group["unblocks"]:
                planned_row_facts = planned_facts_by_surface.get(surface_key, set())
                if surface_key in frozen_surface_keys and not (
                    group_fact_ids & planned_row_facts
                ):
                    errors.append(
                        f"epic #791 fact group {group['group_id']} does not "
                        f"cover any planned blocker for {surface_key}"
                    )
        fact_groups.append({
            **group,
            "fact_statuses": {
                fact_id: fact_status_by_id.get(fact_id, "missing")
                for fact_id in group["facts"]
            },
        })

    return {
        "issue": 791,
        "title": "Model neutral facts for blocked Type-4 surfaces",
        "setup_issue": 792,
        "closeout_issue": 799,
        "predecessor_issue": 778,
        "source": "bench/type4/open_surface_admission_audit.v1.json",
        "scope_policy": {
            "frozen_priority": "blocked-by-unmodeled-facts",
            "predecessor_must_be_closed": 778,
            "group_by": "neutral proof fact before language surface",
            "unexpected_blocked_rows_are_errors": True,
            "resolved_or_promoted_rows_are_allowed": True,
        },
        "summary": {
            "frozen_blocked_count": len(frozen_rows),
            "frozen_currently_blocked": sum(
                1
                for row in frozen_rows
                if row["current_open_audit_state"] == "present"
                and row.get("current_candidate_priority")
                == "blocked-by-unmodeled-facts"
            ),
            "frozen_promoted_or_resolved": sum(
                1
                for row in frozen_rows
                if row["current_open_audit_state"] != "present"
                or row.get("current_candidate_priority")
                != "blocked-by-unmodeled-facts"
            ),
            "current_blocked_open_count": len(current_blocked),
            "current_actionable_open_count": len(current_actionable),
            "unexpected_blocked_open_count": len(unexpected_blocked),
            "fact_group_count": len(fact_groups),
            "validation_error_count": len(errors),
        },
        "frozen_rows": sorted(frozen_rows, key=lambda row: int(row["order"])),
        "fact_groups": sorted(fact_groups, key=lambda group: int(group["order"])),
        "unexpected_blocked_open_rows": unexpected_blocked,
        "validation_errors": errors,
    }


def selftest_row(
    pattern_id: str,
    language: str,
    candidate_priority_value: str,
    *,
    unmodeled_facts: list[str] | None = None,
) -> dict[str, Any]:
    return {
        "pattern_id": pattern_id,
        "language": language,
        "candidate_priority": candidate_priority_value,
        "surface_status": "open",
        "surface": "selftest surface",
        "evidence_level": "missing",
        "likely_blocker": "selftest blocker",
        "unmodeled_facts": unmodeled_facts or [],
        "surface_focused_support": {
            "positive": 0,
            "hard_negative": 0,
            "hard_negative_group": 0,
        },
        "pattern_focused_support": {
            "positive": 0,
            "hard_negative": 0,
            "hard_negative_group": 0,
        },
    }


def selftest() -> None:
    rows = [
        selftest_row(
            spec["pattern_id"],
            spec["language"],
            spec["candidate_priority"],
        )
        for spec in EPIC_778_ACTIONABLE_ROWS
    ] + [
        selftest_row(
            spec["pattern_id"],
            spec["language"],
            "blocked-by-unmodeled-facts",
            unmodeled_facts=["selftest.unmodeled-fact"],
        )
        for spec in EPIC_778_OUT_OF_SCOPE_ROWS
    ]
    slice_report = build_epic_778_slice(rows)
    if slice_report["validation_errors"]:
        raise OpenSurfaceAuditError(
            "selftest expected clean #778 slice, got "
            + "; ".join(slice_report["validation_errors"])
        )
    expected_summary = {
        "in_scope_count": 6,
        "in_scope_currently_open": 6,
        "out_of_scope_count": 7,
        "out_of_scope_currently_open": 7,
        "current_actionable_open_count": 6,
        "current_blocked_open_count": 7,
        "unexpected_actionable_open_count": 0,
        "validation_error_count": 0,
    }
    if slice_report["summary"] != expected_summary:
        raise OpenSurfaceAuditError(
            f"selftest #778 summary drifted: {slice_report['summary']}"
        )

    resolved_rows = rows[1:]
    resolved_slice = build_epic_778_slice(resolved_rows)
    if resolved_slice["validation_errors"]:
        raise OpenSurfaceAuditError(
            "selftest expected resolved rows to be allowed, got "
            + "; ".join(resolved_slice["validation_errors"])
        )

    changed_priority_rows = rows[1:] + [
        selftest_row(
            EPIC_778_ACTIONABLE_ROWS[0]["pattern_id"],
            EPIC_778_ACTIONABLE_ROWS[0]["language"],
            "proof-fact-ready",
        )
    ]
    changed_priority_slice = build_epic_778_slice(changed_priority_rows)
    if changed_priority_slice["validation_errors"]:
        raise OpenSurfaceAuditError(
            "selftest expected changed non-blocked priority to be visible, got "
            + "; ".join(changed_priority_slice["validation_errors"])
        )
    changed_priority_entry = changed_priority_slice["in_scope"][0]
    if (
        changed_priority_entry["current_open_audit_state"]
        != "present-with-different-priority"
        or changed_priority_entry.get("current_candidate_priority") != "proof-fact-ready"
    ):
        raise OpenSurfaceAuditError(
            f"selftest changed priority was not recorded: {changed_priority_entry}"
        )
    if changed_priority_slice["summary"]["current_actionable_open_count"] != 6:
        raise OpenSurfaceAuditError(
            "selftest proof-fact-ready row was not counted as actionable"
        )
    if changed_priority_slice["summary"]["unexpected_actionable_open_count"] != 0:
        raise OpenSurfaceAuditError(
            "selftest in-scope proof-fact-ready row was treated as unexpected"
        )

    regressed_rows = rows[1:] + [
        selftest_row(
            EPIC_778_ACTIONABLE_ROWS[0]["pattern_id"],
            EPIC_778_ACTIONABLE_ROWS[0]["language"],
            "blocked-by-unmodeled-facts",
            unmodeled_facts=["selftest.regressed-fact"],
        )
    ]
    regressed_slice = build_epic_778_slice(regressed_rows)
    if not regressed_slice["validation_errors"]:
        raise OpenSurfaceAuditError(
            "selftest expected blocked in-scope regression to fail"
        )

    fact_status_by_id = {
        fact_id: "specified-not-modeled"
        for group in EPIC_791_FACT_GROUPS
        for fact_id in group["facts"]
    }
    blocked_fact_rows = [
        selftest_row(
            spec["pattern_id"],
            spec["language"],
            spec["candidate_priority"],
            unmodeled_facts=list(spec["planned_unmodeled_facts"]),
        )
        for spec in EPIC_791_BLOCKED_ROWS
    ]
    blocked_fact_slice = build_epic_791_slice(blocked_fact_rows, fact_status_by_id)
    if blocked_fact_slice["validation_errors"]:
        raise OpenSurfaceAuditError(
            "selftest expected clean #791 slice, got "
            + "; ".join(blocked_fact_slice["validation_errors"])
        )
    expected_blocked_summary = {
        "frozen_blocked_count": 7,
        "frozen_currently_blocked": 7,
        "frozen_promoted_or_resolved": 0,
        "current_blocked_open_count": 7,
        "current_actionable_open_count": 0,
        "unexpected_blocked_open_count": 0,
        "fact_group_count": 7,
        "validation_error_count": 0,
    }
    if blocked_fact_slice["summary"] != expected_blocked_summary:
        raise OpenSurfaceAuditError(
            f"selftest #791 summary drifted: {blocked_fact_slice['summary']}"
        )

    resolved_blocked_rows = blocked_fact_rows[:-1]
    resolved_blocked_slice = build_epic_791_slice(
        resolved_blocked_rows,
        fact_status_by_id,
    )
    if resolved_blocked_slice["validation_errors"]:
        raise OpenSurfaceAuditError(
            "selftest expected resolved #791 rows to be allowed, got "
            + "; ".join(resolved_blocked_slice["validation_errors"])
        )
    if resolved_blocked_slice["summary"]["frozen_promoted_or_resolved"] != 1:
        raise OpenSurfaceAuditError(
            "selftest #791 resolved row was not counted as promoted/resolved"
        )

    promoted_spec = EPIC_791_BLOCKED_ROWS[-1]
    promoted_surface_key = row_surface_key(
        promoted_spec["pattern_id"],
        promoted_spec["language"],
    )
    promoted_blocked_rows = [
        row
        for row in blocked_fact_rows
        if row_surface_key(row["pattern_id"], row["language"]) != promoted_surface_key
    ] + [
        selftest_row(
            promoted_spec["pattern_id"],
            promoted_spec["language"],
            "proof-fact-ready",
        )
    ]
    promoted_blocked_slice = build_epic_791_slice(
        promoted_blocked_rows,
        fact_status_by_id,
    )
    if promoted_blocked_slice["validation_errors"]:
        raise OpenSurfaceAuditError(
            "selftest expected promoted #791 row to be allowed, got "
            + "; ".join(promoted_blocked_slice["validation_errors"])
        )
    promoted_entry = promoted_blocked_slice["frozen_rows"][-1]
    if (
        promoted_entry["current_open_audit_state"]
        != "present-with-different-priority"
        or promoted_entry.get("current_candidate_priority") != "proof-fact-ready"
    ):
        raise OpenSurfaceAuditError(
            f"selftest #791 promoted row was not recorded: {promoted_entry}"
        )

    unexpected_blocked_rows = blocked_fact_rows + [
        selftest_row(
            "selftest.unexpected-pattern",
            "Swift",
            "blocked-by-unmodeled-facts",
            unmodeled_facts=["selftest.unexpected-fact"],
        )
    ]
    unexpected_blocked_slice = build_epic_791_slice(
        unexpected_blocked_rows,
        fact_status_by_id,
    )
    if not unexpected_blocked_slice["validation_errors"]:
        raise OpenSurfaceAuditError(
            "selftest expected unexpected #791 blocked row to fail"
        )

    unplanned_blocker_rows = [
        {
            **blocked_fact_rows[0],
            "unmodeled_facts": ["selftest.unplanned-fact"],
        }
    ] + blocked_fact_rows[1:]
    unplanned_blocker_slice = build_epic_791_slice(
        unplanned_blocker_rows,
        fact_status_by_id,
    )
    if not unplanned_blocker_slice["validation_errors"]:
        raise OpenSurfaceAuditError(
            "selftest expected unplanned #791 blocker fact to fail"
        )

    modeled_fact_status_by_id = {
        fact_id: "modeled-controlled"
        for group in EPIC_791_FACT_GROUPS
        for fact_id in group["facts"]
    }
    modeled_blocker_slice = build_epic_791_slice(
        blocked_fact_rows,
        modeled_fact_status_by_id,
    )
    if not modeled_blocker_slice["validation_errors"]:
        raise OpenSurfaceAuditError(
            "selftest expected modeled #791 blocker fact to fail"
        )

    missing_fact_status_by_id = dict(fact_status_by_id)
    missing_fact_status_by_id.pop("option.absence-channel.identity")
    missing_fact_slice = build_epic_791_slice(
        blocked_fact_rows,
        missing_fact_status_by_id,
    )
    if not missing_fact_slice["validation_errors"]:
        raise OpenSurfaceAuditError(
            "selftest expected unknown #791 fact group id to fail"
        )
    print("selftest OK")


def format_value(value: Any) -> str:
    if isinstance(value, list):
        return "[" + ", ".join(str(item) for item in value) + "]"
    if isinstance(value, bool):
        return str(value).lower()
    return str(value)


def format_count_map(values: dict[str, Any]) -> str:
    if not values:
        return ""
    return ", ".join(f"{key}={format_value(value)}" for key, value in values.items())


def markdown_cell(value: Any) -> str:
    return str(value).replace("\n", " ").replace("|", "\\|")


def render_epic_778_slice(slice_report: dict[str, Any]) -> list[str]:
    summary = slice_report["summary"]
    lines = [
        "## Epic #778 Audit Slice",
        "",
        "This frozen slice tracks the rows selected for #778. It separates the",
        "current epic's focused-admission work from rows that still require new",
        "neutral proof facts before admission work is sound.",
        "",
        f"- tracker issue: #{slice_report['issue']}",
        f"- setup issue: #{slice_report['setup_issue']}",
        f"- closeout issue: #{slice_report['closeout_issue']}",
        f"- in-scope rows: {summary['in_scope_count']} "
        f"({summary['in_scope_currently_open']} currently present in the open audit)",
        f"- out-of-scope blocked rows: {summary['out_of_scope_count']} "
        f"({summary['out_of_scope_currently_open']} currently present in the open audit)",
        f"- unexpected actionable open rows: {summary['unexpected_actionable_open_count']}",
        "",
        "### In Scope",
        "",
        "| order | issue | priority | pattern | language | current state | blocker | work | surface |",
        "|---|---|---|---|---|---|---|---|---|",
    ]
    for row in slice_report["in_scope"]:
        current_state = row["current_open_audit_state"]
        if row.get("current_candidate_priority") != row["candidate_priority"]:
            current_state = (
                f"{current_state} (`{row.get('current_candidate_priority', '')}`)"
            )
        lines.append(
            "| "
            f"{row['order']} | "
            f"#{row['issue']} | "
            f"`{row['candidate_priority']}` | "
            f"`{row['pattern_id']}` | "
            f"{row['language']} | "
            f"{current_state} | "
            f"{markdown_cell(row.get('likely_blocker', ''))} | "
            f"{markdown_cell(row['work'])} | "
            f"{markdown_cell(row.get('surface', ''))} |"
        )
    lines.extend([
        "",
        "### Out Of Scope For #778",
        "",
        "| priority | pattern | language | current state | reason | missing facts | surface |",
        "|---|---|---|---|---|---|---|",
    ])
    for row in slice_report["out_of_scope"]:
        current_state = row["current_open_audit_state"]
        if row.get("current_candidate_priority") != row["candidate_priority"]:
            current_state = (
                f"{current_state} (`{row.get('current_candidate_priority', '')}`)"
            )
        lines.append(
            "| "
            f"`{row['candidate_priority']}` | "
            f"`{row['pattern_id']}` | "
            f"{row['language']} | "
            f"{current_state} | "
            f"{markdown_cell(row['reason'])} | "
            f"{', '.join(f'`{fact}`' for fact in row.get('unmodeled_facts', []))} | "
            f"{markdown_cell(row.get('surface', ''))} |"
        )
    if slice_report["unexpected_actionable_open_rows"]:
        lines.extend([
            "",
            "### Unexpected Actionable Rows",
            "",
            "| pattern | language | priority | blocker |",
            "|---|---|---|---|",
        ])
        for row in slice_report["unexpected_actionable_open_rows"]:
            lines.append(
                "| "
                f"`{row['pattern_id']}` | "
                f"{row['language']} | "
                f"`{row['candidate_priority']}` | "
                f"{markdown_cell(row['likely_blocker'])} |"
            )
    return lines


def format_selector_items(items: list[str]) -> str:
    if not items:
        return ""
    return ", ".join(f"`{item}`" for item in items)


def render_epic_791_slice(slice_report: dict[str, Any]) -> list[str]:
    summary = slice_report["summary"]
    lines = [
        "## Epic #791 Neutral-Fact Blocked Slice",
        "",
        "This frozen slice tracks the open rows that are intentionally blocked",
        "until missing language-neutral proof facts are modeled. It is grouped by",
        "reusable proof fact stage before language surface so the next PRs do not",
        "admit Ruby, Swift, Java, or map/HOF spellings directly.",
        "",
        f"- tracker issue: #{slice_report['issue']}",
        f"- setup issue: #{slice_report['setup_issue']}",
        f"- predecessor issue: #{slice_report['predecessor_issue']}",
        f"- closeout issue: #{slice_report['closeout_issue']}",
        f"- frozen blocked rows: {summary['frozen_blocked_count']} "
        f"({summary['frozen_currently_blocked']} currently blocked)",
        f"- promoted or resolved frozen rows: {summary['frozen_promoted_or_resolved']}",
        f"- current blocked open rows: {summary['current_blocked_open_count']}",
        f"- unexpected blocked open rows: {summary['unexpected_blocked_open_count']}",
        "",
        "### Fact Groups And Admission Order",
        "",
        "| order | issue | fact group | fact statuses | unblocks | focused admission candidates after group lands | intentionally still open |",
        "|---|---|---|---|---|---|---|",
    ]
    for group in slice_report["fact_groups"]:
        statuses = ", ".join(
            f"`{fact}`:{status}"
            for fact, status in group["fact_statuses"].items()
        )
        lines.append(
            "| "
            f"{group['order']} | "
            f"#{group['issue']} | "
            f"`{group['group_id']}` {markdown_cell(group['title'])} | "
            f"{statuses} | "
            f"{format_selector_items(group['unblocks'])} | "
            f"{format_selector_items(group['focused_admission_after_group_lands'])} | "
            f"{markdown_cell('; '.join(group['still_open_until']))} |"
        )
    lines.extend([
        "",
        "### Frozen Rows",
        "",
        "| order | issue | priority | pattern | language | current state | blocker | missing facts | work | surface |",
        "|---|---|---|---|---|---|---|---|---|---|",
    ])
    for row in slice_report["frozen_rows"]:
        current_state = row["current_open_audit_state"]
        if row.get("current_candidate_priority") != row["candidate_priority"]:
            current_state = (
                f"{current_state} (`{row.get('current_candidate_priority', '')}`)"
            )
        lines.append(
            "| "
            f"{row['order']} | "
            f"#{row['issue']} | "
            f"`{row['candidate_priority']}` | "
            f"`{row['pattern_id']}` | "
            f"{row['language']} | "
            f"{current_state} | "
            f"{markdown_cell(row.get('likely_blocker', ''))} | "
            f"{', '.join(f'`{fact}`' for fact in row.get('unmodeled_facts', []))} | "
            f"{markdown_cell(row['work'])} | "
            f"{markdown_cell(row.get('surface', ''))} |"
        )
    if slice_report["unexpected_blocked_open_rows"]:
        lines.extend([
            "",
            "### Unexpected Blocked Rows",
            "",
            "| pattern | language | priority | blocker |",
            "|---|---|---|---|",
        ])
        for row in slice_report["unexpected_blocked_open_rows"]:
            lines.append(
                "| "
                f"`{row['pattern_id']}` | "
                f"{row['language']} | "
                f"`{row['candidate_priority']}` | "
                f"{markdown_cell(row['likely_blocker'])} |"
            )
    return lines


def render_markdown(report: dict[str, Any]) -> str:
    summary = report["summary"]
    lines = [
        "# Open Type-4 surface admission audit",
        "",
        "Generated by `bench/type4/open_surface_admission_audit.py` from",
        "`semantic_pattern_cards.v1.json`.",
        "",
        "This report lists language surfaces that remain open in the checked",
        "semantic pattern catalog. Use it to choose the next proof-backed",
        "surface admission issue without re-triaging every pattern card by hand.",
        "",
        "## Summary",
        "",
        f"- audited surface statuses: {', '.join(summary['audited_surface_statuses'])}",
        f"- open surfaces: {summary['open_surface_count']}",
        f"- priorities: {format_count_map(summary['by_priority'])}",
        f"- evidence levels: {format_count_map(summary['by_evidence_level'])}",
        f"- languages: {format_count_map(summary['by_language'])}",
        f"- unresolved surface evidence refs: {summary['unresolved_surface_evidence_ref_count']}",
        "",
    ]
    lines.extend(render_epic_778_slice(report["epic_slices"]["epic_778"]))
    lines.extend([""])
    lines.extend(render_epic_791_slice(report["epic_slices"]["epic_791"]))
    lines.extend([
        "",
        "## Candidate Rows",
        "",
        "| priority | pattern | language | surface | status | evidence | blocker | facts | surface focused | pattern perimeter | coverage |",
        "|---|---|---|---|---|---|---|---|---|---|---|",
    ])
    for row in report["rows"]:
        facts = ", ".join(
            f"`{fact}`:{row['fact_statuses'][fact]}" for fact in row["required_facts"]
        )
        surface_support = (
            f"positive={row['surface_focused_support']['positive']}, "
            f"hard_negative={row['surface_focused_support']['hard_negative']}, "
            f"group={row['surface_focused_support']['hard_negative_group']}"
        )
        pattern_support = (
            f"positive={row['pattern_focused_support']['positive']}, "
            f"hard_negative={row['pattern_focused_support']['hard_negative']}, "
            f"group={row['pattern_focused_support']['hard_negative_group']}"
        )
        coverage = format_count_map(row["coverage"])
        lines.append(
            "| "
            f"`{row['candidate_priority']}` | "
            f"`{row['pattern_id']}` | "
            f"{row['language']} | "
            f"{markdown_cell(row['surface'])} | "
            f"`{row['surface_status']}` | "
            f"`{row['evidence_level']}`"
            + (f"<br>{markdown_cell(row['surface_evidence'])}" if row["surface_evidence"] else "")
            + " | "
            f"{row['likely_blocker']} | "
            f"{facts} | "
            f"{surface_support} | "
            f"{pattern_support} | "
            f"{coverage} |"
        )
    lines.extend(
        [
            "",
            "## By Pattern",
            "",
            "| pattern | open surfaces |",
            "|---|---|",
        ]
    )
    for pattern_id, surfaces in report["by_pattern"].items():
        lines.append(
            f"| `{pattern_id}` | {', '.join(group_entry_label(entry) for entry in surfaces)} |"
        )
    lines.extend(
        [
            "",
            "## By Blocker",
            "",
            "| blocker | open surfaces |",
            "|---|---|",
        ]
    )
    for blocker, surfaces in report["by_blocker"].items():
        lines.append(
            f"| {blocker} | {', '.join(group_entry_label(entry) for entry in surfaces)} |"
        )
    lines.extend(
        [
            "",
            "## By Language",
            "",
            "| language | open surfaces |",
            "|---|---|",
        ]
    )
    for language, surfaces in report["by_language"].items():
        lines.append(
            f"| {language} | {', '.join(group_entry_label(entry) for entry in surfaces)} |"
        )
    lines.extend(
        [
            "",
            "## By Surface Status",
            "",
            "| status | open surfaces |",
            "|---|---|",
        ]
    )
    for status, surfaces in report["by_surface_status"].items():
        lines.append(
            f"| `{status}` | {', '.join(group_entry_label(entry) for entry in surfaces)} |"
        )
    lines.extend(
        [
            "",
            "## By Proof Fact",
            "",
            "| proof fact | open surfaces |",
            "|---|---|",
        ]
    )
    for fact_id, surfaces in report["by_fact"].items():
        lines.append(
            f"| `{fact_id}` | {', '.join(group_entry_label(entry) for entry in surfaces)} |"
        )
    lines.extend(
        [
            "",
            "## How To Use",
            "",
            "1. Start with rows marked `proof-fact-ready` or",
            "   `probe-to-focused-candidate` when choosing the next admission target.",
            "2. Treat `needs-surface-focused-perimeter` as actionable setup: the",
            "   neutral facts are modeled, but this exact language surface still needs",
            "   focused positives, adjacent hard negatives, and executable",
            "   expectations before admission.",
            "3. Before detector admission, require surface-focused positives, adjacent",
            "   hard negatives, and executable expectations for the exact language",
            "   surface.",
            "4. Keep rows marked `blocked-by-unmodeled-facts` open until their neutral",
            "   proof facts become modeled-controlled.",
            "",
            "Regenerate with:",
            "",
            "```sh",
            "python3 bench/type4/open_surface_admission_audit.py",
            "```",
            "",
            "Check the committed artifacts with:",
            "",
            "```sh",
            "python3 bench/type4/open_surface_admission_audit.py --check",
            "```",
            "",
        ]
    )
    return "\n".join(lines)


def check_artifact(path: Path, expected: str) -> None:
    if not path.exists() or path.read_text() != expected:
        raise OpenSurfaceAuditError(f"open surface admission audit artifact is stale: {repo_rel(path)}")


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--cards", type=Path, default=DEFAULT_CARDS)
    parser.add_argument("--proof-fact-registry", type=Path, default=DEFAULT_PROOF_FACT_REGISTRY)
    parser.add_argument("--focused-cases", type=Path, default=DEFAULT_FOCUSED_CASES)
    parser.add_argument("--target-packets", type=Path, default=DEFAULT_TARGET_PACKETS)
    parser.add_argument("--coverage-evidence", type=Path, default=HERE / "coverage_evidence.v1.json")
    parser.add_argument("--json-out", type=Path, default=DEFAULT_JSON_OUT)
    parser.add_argument("--markdown-out", type=Path, default=DEFAULT_MARKDOWN_OUT)
    parser.add_argument("--check", action="store_true", help="fail if generated artifacts are stale")
    parser.add_argument("--selftest", action="store_true", help="run helper self-test")
    args = parser.parse_args()

    try:
        if args.selftest:
            selftest()
            return 0
        cards = load_json(args.cards)
        proof_fact_registry = load_json(args.proof_fact_registry)
        focused_cases = load_json(args.focused_cases)
        target_packets = load_json(args.target_packets)
        coverage_evidence = load_json(args.coverage_evidence)
        patterns = validate_cards(cards, proof_fact_registry, focused_cases, target_packets)
        report = build_report(
            patterns,
            proof_fact_registry,
            focused_cases,
            target_packets,
            coverage_evidence,
        )
        unresolved_ref_count = report["summary"]["unresolved_surface_evidence_ref_count"]
        if unresolved_ref_count:
            raise OpenSurfaceAuditError(
                f"{unresolved_ref_count} unresolved open-surface evidence ref(s)"
            )
        epic_778_errors = report["epic_slices"]["epic_778"]["validation_errors"]
        if epic_778_errors:
            raise OpenSurfaceAuditError(
                "epic #778 audit slice is invalid: " + "; ".join(epic_778_errors)
            )
        epic_791_errors = report["epic_slices"]["epic_791"]["validation_errors"]
        if epic_791_errors:
            raise OpenSurfaceAuditError(
                "epic #791 audit slice is invalid: " + "; ".join(epic_791_errors)
            )
        json_text = json.dumps(report, indent=2, sort_keys=True) + "\n"
        markdown = render_markdown(report)
        if args.check:
            check_artifact(args.json_out, json_text)
            check_artifact(args.markdown_out, markdown)
        else:
            args.json_out.write_text(json_text)
            args.markdown_out.write_text(markdown)
            print(f"wrote {repo_rel(args.json_out)}")
            print(f"wrote {repo_rel(args.markdown_out)}")
    except (OpenSurfaceAuditError, PatternCardError) as exc:
        print(f"error: {exc}", file=sys.stderr)
        return 1
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
