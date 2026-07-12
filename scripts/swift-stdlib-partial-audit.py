#!/usr/bin/env python3
"""Audit Swift stdlib partial-coverage prevalence in the pinned corpus.

This is a lexical pricing report, not semantic proof. It masks comments and
strings before counting common Swift collection factories, sequence HOFs,
cardinality properties, ordering/view APIs, and mutation/effect methods. The
report separates currently covered-partial surfaces from capability buckets that
should stay closed until the semantic kernel has reusable proof.
"""

from __future__ import annotations

import argparse
import json
import re
from collections import Counter, defaultdict
from dataclasses import dataclass
from pathlib import Path
from typing import Any


DEFAULT_MANIFEST = "bench/goldens/corpus.json"
DEFAULT_REPOS_ROOT = "bench/repos"
DEFAULT_OUTPUT = "target/swift-stdlib-partial-audit.v2.json"
HIGH_VOLUME_PROCESSING_THRESHOLD = 5000

SKIP_DIRS = {
    ".build",
    ".git",
    "Carthage",
    "DerivedData",
    "Pods",
    "build",
    "dist",
    "node_modules",
    "target",
}

METHOD_RE = re.compile(r"\.\s*(?P<method>[A-Za-z_][A-Za-z0-9_]*)\s*(?:<[^;\n(){}]*>)?\s*\(")
PROPERTY_RE = re.compile(r"\.\s*(?P<property>count|isEmpty)\b(?!\s*\()")


@dataclass(frozen=True)
class LexicalPattern:
    surface: str
    operation: str
    regex: str
    status: str
    boundary: str
    capability: str
    note: str

    def compile(self) -> re.Pattern[str]:
        return re.compile(self.regex)


LEXICAL_PATTERNS = [
    LexicalPattern(
        "swift.stdlib.collection_factories",
        "Array(repeating:count:)",
        r"\bArray\s*(?:<[^>\n]+>)?\s*\(\s*repeating\s*:",
        "unsupported",
        "repeat-factory-shape",
        "collection-factory",
        "Array(repeating:count:) needs repeat-count shape and element copy semantics.",
    ),
    LexicalPattern(
        "swift.stdlib.collection_factories",
        "Array",
        r"\bArray\s*(?:<[^>\n]+>)?\s*\(\s*(?!repeating\s*:)",
        "supported-partial",
        "collection-factory-proof",
        "collection-factory",
        "Array(sequence) is admitted only with exact sequence and unshadowed type proof.",
    ),
    LexicalPattern(
        "swift.stdlib.collection_factories",
        "Set",
        r"\bSet\s*(?:<[^>\n]+>)?\s*\(",
        "supported-partial",
        "collection-factory-proof",
        "collection-factory",
        "Set(sequence) is admitted only with exact sequence and unshadowed type proof.",
    ),
    LexicalPattern(
        "swift.stdlib.collection_factories",
        "Dictionary(uniqueKeysWithValues:)",
        r"\bDictionary\s*(?:<[^>\n]+>)?\s*\(\s*uniqueKeysWithValues\s*:",
        "supported-partial",
        "map-factory-proof",
        "map-factory",
        "Dictionary(uniqueKeysWithValues:) needs entry-shape and duplicate-key proof.",
    ),
    LexicalPattern(
        "swift.stdlib.sequence_views",
        "zip",
        r"\bzip\s*\(",
        "unsupported",
        "zip-view",
        "iterator-hof-materialization",
        "zip view shape and lifecycle are not modeled.",
    ),
]

SUPPORTED_PARTIAL_METHODS: dict[str, tuple[str, str, str, str]] = {
    "allSatisfy": (
        "swift.stdlib.sequence_hof",
        "terminal-hof",
        "iterator-hof-materialization",
        "terminal HOF needs receiver and predicate proof",
    ),
    "compactMap": (
        "swift.stdlib.sequence_hof",
        "hof-callback-proof",
        "iterator-hof-materialization",
        "compactMap needs receiver, callback, and optional-result proof",
    ),
    "contains": (
        "swift.stdlib.membership",
        "collection-membership-proof",
        "collection-membership",
        "contains needs exact collection/sequence receiver proof and label-sensitive callback handling",
    ),
    "filter": (
        "swift.stdlib.sequence_hof",
        "hof-callback-proof",
        "iterator-hof-materialization",
        "filter needs receiver, callback, and demand/effect proof",
    ),
    "flatMap": (
        "swift.stdlib.sequence_hof",
        "flat-map-one-level-proof",
        "iterator-hof-materialization",
        "flatMap needs direct outer/inner source, one-level output, callback, and dispatch proof",
    ),
    "map": (
        "swift.stdlib.sequence_hof",
        "hof-callback-proof",
        "iterator-hof-materialization",
        "map needs receiver, callback, and demand/effect proof",
    ),
}

SUPPORTED_PARTIAL_PROPERTIES: dict[str, tuple[str, str, str, str]] = {
    "count": (
        "swift.stdlib.cardinality",
        "cardinality-property-proof",
        "collection-cardinality",
        "count is admitted only with collection/array receiver proof",
    ),
    "isEmpty": (
        "swift.stdlib.cardinality",
        "cardinality-property-proof",
        "collection-cardinality",
        "isEmpty is admitted only with collection/array receiver proof",
    ),
}

PROCESSING_DECISIONS: dict[str, dict[str, Any]] = {
    "swift-cardinality-receiver-proof": {
        "sequence": 5,
        "status": "processed-existing-contract",
        "semantic_admission_delta": 0,
        "strictness_effect": "unchanged",
        "decision": (
            "Keep Swift count/isEmpty on the existing property-builtin and generic "
            "method-call contracts with ExactCollection receiver proof."
        ),
        "closed_boundary": (
            "Selector-only count/isEmpty on custom, optional, scalar, or otherwise "
            "unproven receivers remains closed."
        ),
        "next_metric": (
            "Track cardinality misses by property occurrence, method-call occurrence, "
            "collection receiver proof, and shadow/typealias boundary."
        ),
        "subgroups": [
            {
                "surface": "swift.stdlib.cardinality.count",
                "semantic": "Builtin::Len",
                "receiver": "ExactCollection",
            },
            {
                "surface": "swift.stdlib.cardinality.isEmpty",
                "semantic": "Builtin::IsEmpty",
                "receiver": "ExactCollection",
            },
        ],
    },
}

UNSUPPORTED_METHODS: dict[str, tuple[str, str, str, str]] = {
    "append": (
        "swift.stdlib.mutation",
        "mutation-effect",
        "effect-preserving-contracts",
        "append mutates collection state",
    ),
    "dropFirst": (
        "swift.stdlib.sequence_views",
        "slice-view",
        "iterator-hof-materialization",
        "slice view shape/lifecycle is not modeled",
    ),
    "dropLast": (
        "swift.stdlib.sequence_views",
        "slice-view",
        "iterator-hof-materialization",
        "slice view shape/lifecycle is not modeled",
    ),
    "enumerated": (
        "swift.stdlib.sequence_views",
        "index-view",
        "iterator-hof-materialization",
        "index-producing view shape is not modeled",
    ),
    "forEach": (
        "swift.stdlib.sequence_hof",
        "effect-callback",
        "effect-preserving-contracts",
        "forEach is effect-oriented and callback-driven",
    ),
    "insert": (
        "swift.stdlib.mutation",
        "mutation-effect",
        "effect-preserving-contracts",
        "insert mutates collection state",
    ),
    "max": (
        "swift.stdlib.ordering",
        "ordering-reduction",
        "collection-ordering",
        "ordering reduction needs comparator/order proof",
    ),
    "min": (
        "swift.stdlib.ordering",
        "ordering-reduction",
        "collection-ordering",
        "ordering reduction needs comparator/order proof",
    ),
    "prefix": (
        "swift.stdlib.sequence_views",
        "slice-view",
        "iterator-hof-materialization",
        "slice view shape/lifecycle is not modeled",
    ),
    "reduce": (
        "swift.stdlib.sequence_hof",
        "reduction-callback",
        "iterator-hof-materialization",
        "callback reduction needs accumulator demand/effect semantics",
    ),
    "remove": (
        "swift.stdlib.mutation",
        "mutation-effect",
        "effect-preserving-contracts",
        "remove mutates collection state",
    ),
    "removeAll": (
        "swift.stdlib.mutation",
        "mutation-effect",
        "effect-preserving-contracts",
        "removeAll mutates collection state",
    ),
    "reversed": (
        "swift.stdlib.ordering",
        "ordering-view",
        "collection-ordering",
        "reverse view needs sequence/order proof",
    ),
    "shuffle": (
        "swift.stdlib.mutation",
        "mutation-effect",
        "effect-preserving-contracts",
        "shuffle mutates collection order and is nondeterministic",
    ),
    "sort": (
        "swift.stdlib.ordering",
        "mutation-effect",
        "collection-ordering",
        "sort mutates collection order",
    ),
    "sorted": (
        "swift.stdlib.ordering",
        "ordering-materializer",
        "collection-ordering",
        "sorted needs comparator/order proof and result-domain semantics",
    ),
    "suffix": (
        "swift.stdlib.sequence_views",
        "slice-view",
        "iterator-hof-materialization",
        "slice view shape/lifecycle is not modeled",
    ),
}


def parse_args() -> argparse.Namespace:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--manifest", default=DEFAULT_MANIFEST)
    parser.add_argument("--repos-root", default=DEFAULT_REPOS_ROOT)
    parser.add_argument("--output", default=DEFAULT_OUTPUT)
    return parser.parse_args()


def swift_repos(manifest: Path) -> list[str]:
    data = json.loads(manifest.read_text())
    return [
        repo["id"]
        for repo in data.get("repositories", [])
        if repo.get("primary_language") == "Swift"
    ]


def swift_files(root: Path) -> list[Path]:
    if not root.exists():
        return []
    files: list[Path] = []
    for path in root.rglob("*.swift"):
        if any(part in SKIP_DIRS for part in path.parts):
            continue
        files.append(path)
    return files


def mask_comments_and_strings(text: str) -> str:
    chars = list(text)
    i = 0
    block_depth = 0
    while i < len(chars):
        if block_depth > 0:
            if text.startswith("/*", i):
                chars[i : i + 2] = "  "
                block_depth += 1
                i += 2
            elif text.startswith("*/", i):
                chars[i : i + 2] = "  "
                block_depth -= 1
                i += 2
            else:
                if chars[i] != "\n":
                    chars[i] = " "
                i += 1
            continue
        if text.startswith("//", i):
            while i < len(chars) and chars[i] != "\n":
                chars[i] = " "
                i += 1
            continue
        if text.startswith("/*", i):
            chars[i : i + 2] = "  "
            block_depth = 1
            i += 2
            continue
        raw = raw_string_bounds(text, i)
        if raw is not None:
            end = raw
            while i <= end and i < len(chars):
                if chars[i] != "\n":
                    chars[i] = " "
                i += 1
            continue
        if chars[i] == '"':
            i = mask_quoted(chars, i)
            continue
        i += 1
    return "".join(chars)


def raw_string_bounds(text: str, index: int) -> int | None:
    if text[index] != "#":
        return None
    j = index
    hashes = 0
    while j < len(text) and text[j] == "#":
        hashes += 1
        j += 1
    if j >= len(text) or text[j] != '"':
        return None
    terminator = '"' + ("#" * hashes)
    end = text.find(terminator, j + 1)
    if end == -1:
        return len(text) - 1
    return end + len(terminator) - 1


def mask_quoted(chars: list[str], index: int) -> int:
    chars[index] = " "
    i = index + 1
    while i < len(chars):
        ch = chars[i]
        if ch != "\n":
            chars[i] = " "
        if ch == "\\":
            i += 2
            continue
        if ch == '"':
            return i + 1
        i += 1
    return i


def classify_method(method: str) -> tuple[str, str, str, str, str, str]:
    partial = SUPPORTED_PARTIAL_METHODS.get(method)
    if partial is not None:
        surface, boundary, capability, note = partial
        return surface, method, "supported-partial", boundary, capability, note
    unsupported = UNSUPPORTED_METHODS.get(method)
    if unsupported is not None:
        surface, boundary, capability, note = unsupported
        return surface, method, "unsupported", boundary, capability, note
    return (
        "swift.stdlib.unclassified_methods",
        method,
        "unknown",
        "unknown-boundary",
        "unknown",
        "observed method is not classified by this audit",
    )


def next_work_group(status: str, boundary: str, capability: str) -> tuple[str, str, str] | None:
    if status == "supported":
        return None
    if boundary in {"hof-callback-proof", "terminal-hof"}:
        return (
            "swift-sequence-hof-proof",
            "sequence HOF receiver and callback proof",
            "Swift HOFs should keep consuming reusable receiver/callback proof, not selectors.",
        )
    if boundary in {"collection-factory-proof", "map-factory-proof"}:
        return (
            "swift-factory-domain-proof",
            "collection/map factory domain proof",
            "Factories need type-name proof, label proof, entry-shape proof, and exact-safe inputs.",
        )
    if boundary == "repeat-factory-shape":
        return (
            "swift-repeat-factory-shape",
            "repeat factory shape contracts",
            "Repeated-element factories need count and element-copy/value semantics before admission.",
        )
    if boundary == "cardinality-property-proof":
        return (
            "swift-cardinality-receiver-proof",
            "cardinality receiver proof",
            "count/isEmpty should stay receiver-proof-gated.",
        )
    if boundary in {"mutation-effect", "effect-callback"} or capability == "effect-preserving-contracts":
        return (
            "swift-mutation-effect-contracts",
            "mutation/effect contracts",
            "Mutating and effect-oriented APIs need place/effect summaries before admission.",
        )
    if "ordering" in boundary or capability == "collection-ordering":
        return (
            "swift-ordering-semantics",
            "ordering and comparator semantics",
            "Ordering APIs need comparator/key obligations and sortedness/order proof.",
        )
    if boundary in {"index-view", "slice-view", "zip-view"}:
        return (
            "swift-sequence-view-lifecycle",
            "sequence view lifecycle and shape contracts",
            "Views need lifecycle/cardinality policies before exact admission.",
        )
    if boundary == "reduction-callback":
        return (
            "swift-reduction-contracts",
            "reduction contracts",
            "Reductions need accumulator and callback demand/effect semantics.",
        )
    if boundary == "collection-membership-proof":
        return (
            "swift-membership-receiver-proof",
            "membership receiver proof",
            "contains needs exact collection/sequence receiver proof and label-sensitive callback handling.",
        )
    return (
        "swift-unclassified-boundary",
        "unclassified boundary",
        "Observed rows should stay closed until classified by capability.",
    )


def processed_high_volume_groups(ranked_next_work: list[dict[str, Any]]) -> list[dict[str, Any]]:
    processed: list[dict[str, Any]] = []
    for group in ranked_next_work:
        if group["occurrences"] < HIGH_VOLUME_PROCESSING_THRESHOLD:
            continue
        decision = PROCESSING_DECISIONS.get(group["id"])
        if decision is None:
            decision = {
                "status": "unprocessed-high-volume-group",
                "semantic_admission_delta": 0,
                "strictness_effect": "unchanged",
                "decision": "No processing decision is recorded for this high-volume group yet.",
                "next_metric": "Add a processing decision before using this group for implementation.",
            }
        processed.append(
            {
                "id": group["id"],
                "capability": group["capability"],
                "occurrences": group["occurrences"],
                "repos": group["repos"],
                "top_surfaces": group["top_surfaces"],
                **decision,
            }
        )
    return sorted(processed, key=lambda group: group.get("sequence", 999))


def main() -> int:
    args = parse_args()
    repos_root = Path(args.repos_root)
    compiled = [(pattern, pattern.compile()) for pattern in LEXICAL_PATTERNS]
    rows: Counter[tuple[str, str, str, str, str, str]] = Counter()
    row_repos: dict[tuple[str, str, str, str, str, str], Counter[str]] = defaultdict(Counter)
    scanned_files = 0

    for repo in swift_repos(Path(args.manifest)):
        for path in swift_files(repos_root / repo):
            scanned_files += 1
            try:
                raw_text = path.read_text(errors="ignore")
            except OSError:
                continue
            text = mask_comments_and_strings(raw_text)
            for pattern, regex in compiled:
                count = len(regex.findall(text))
                if count == 0:
                    continue
                key = (
                    pattern.surface,
                    pattern.operation,
                    pattern.status,
                    pattern.boundary,
                    pattern.capability,
                    pattern.note,
                )
                rows[key] += count
                row_repos[key][repo] += count
            for match in METHOD_RE.finditer(text):
                method = match.group("method")
                if method not in SUPPORTED_PARTIAL_METHODS and method not in UNSUPPORTED_METHODS:
                    continue
                surface, operation, status, boundary, capability, note = classify_method(method)
                key = (surface, operation, status, boundary, capability, note)
                rows[key] += 1
                row_repos[key][repo] += 1
            for match in PROPERTY_RE.finditer(text):
                prop = match.group("property")
                surface, boundary, capability, note = SUPPORTED_PARTIAL_PROPERTIES[prop]
                key = (surface, prop, "supported-partial", boundary, capability, note)
                rows[key] += 1
                row_repos[key][repo] += 1

    report_rows: list[dict[str, Any]] = []
    for key, occurrences in sorted(rows.items(), key=lambda item: (-item[1], item[0])):
        surface, operation, status, boundary, capability, note = key
        report_rows.append(
            {
                "surface": surface,
                "operation": operation,
                "occurrences": occurrences,
                "repos": len(row_repos[key]),
                "status": status,
                "boundary": boundary,
                "capability": capability,
                "note": note,
                "top_repos": [
                    {"repo": repo, "occurrences": count}
                    for repo, count in row_repos[key].most_common(8)
                ],
            }
        )

    status_counts = Counter(row["status"] for row in report_rows for _ in range(row["occurrences"]))
    boundary_counts = Counter(
        row["boundary"] for row in report_rows for _ in range(row["occurrences"])
    )
    surface_counts = Counter(row["surface"] for row in report_rows for _ in range(row["occurrences"]))
    next_counts: Counter[tuple[str, str, str]] = Counter()
    next_repos: dict[tuple[str, str, str], set[str]] = defaultdict(set)
    next_surfaces: dict[tuple[str, str, str], Counter[str]] = defaultdict(Counter)
    for key, occurrences in rows.items():
        surface, operation, status, boundary, capability, _note = key
        group = next_work_group(status, boundary, capability)
        if group is None:
            continue
        next_counts[group] += occurrences
        next_surfaces[group][f"{surface}.{operation}:{boundary}"] += occurrences
        next_repos[group].update(row_repos[key].keys())

    ranked_next_work = [
        {
            "id": group_id,
            "capability": capability,
            "occurrences": count,
            "repos": len(next_repos[group]),
            "policy": policy,
            "top_surfaces": [
                {"surface": surface, "occurrences": surface_count}
                for surface, surface_count in next_surfaces[group].most_common(8)
            ],
        }
        for group, count in sorted(next_counts.items(), key=lambda item: (-item[1], item[0][0]))
        for group_id, capability, policy in [group]
    ]

    report = {
        "report_kind": "swift-stdlib-partial-audit",
        "schema_version": 2,
        "manifest": args.manifest,
        "repos_root": args.repos_root,
        "scanned_swift_repos": len(swift_repos(Path(args.manifest))),
        "scanned_swift_files": scanned_files,
        "totals": {
            "occurrences": sum(rows.values()),
            "supported_occurrences": status_counts.get("supported", 0),
            "supported_partial_occurrences": status_counts.get("supported-partial", 0),
            "unsupported_occurrences": status_counts.get("unsupported", 0),
            "unknown_boundary_occurrences": boundary_counts.get("unknown-boundary", 0),
        },
        "status_counts": dict(sorted(status_counts.items())),
        "surface_counts": dict(sorted(surface_counts.items())),
        "boundary_counts": dict(sorted(boundary_counts.items())),
        "ranked_next_work": ranked_next_work,
        "processing_threshold_occurrences": HIGH_VOLUME_PROCESSING_THRESHOLD,
        "processed_high_volume_groups": processed_high_volume_groups(ranked_next_work),
        "operations": report_rows,
    }
    output = Path(args.output)
    output.parent.mkdir(parents=True, exist_ok=True)
    output.write_text(json.dumps(report, indent=2, sort_keys=False) + "\n")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
