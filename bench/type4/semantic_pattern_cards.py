#!/usr/bin/env python3
"""Validate and render Type-4 semantic pattern cards."""

from __future__ import annotations

import argparse
import json
import sys
from pathlib import Path
from typing import Any

HERE = Path(__file__).resolve().parent
ROOT = HERE.parents[1]

SCHEMA_VERSION = 1
TOOL_VERSION = "semantic-pattern-cards/1"
DEFAULT_CARDS = HERE / "semantic_pattern_cards.v1.json"
DEFAULT_MARKDOWN = HERE / "semantic_pattern_cards.md"
DEFAULT_PROOF_FACT_REGISTRY = HERE / "proof_fact_registry.v1.json"
DEFAULT_FOCUSED_CASES = HERE / "adversarial" / "cases" / "cases.v1.json"
DEFAULT_TARGET_PACKETS = HERE / "frontier_target_packets.v1.json"

PATTERN_STATUS = {
    "open-candidate",
    "pattern-carded",
    "controlled-slice-admitted",
    "real-pair-admitted",
    "closed-boundary",
}
SURFACE_STATUS = {"open", "modeled-controlled", "admitted", "closed", "not-applicable"}


class PatternCardError(RuntimeError):
    pass


def repo_rel(path: Path) -> str:
    try:
        return str(path.resolve().relative_to(ROOT))
    except ValueError:
        return str(path)


def load_json(path: Path) -> dict[str, Any]:
    try:
        return json.loads(path.read_text())
    except FileNotFoundError as exc:
        raise PatternCardError(f"missing artifact: {repo_rel(path)}") from exc
    except json.JSONDecodeError as exc:
        raise PatternCardError(f"invalid JSON in {repo_rel(path)}: {exc}") from exc


def require_non_empty_list(obj: dict[str, Any], field: str, context: str) -> list[Any]:
    value = obj.get(field)
    if not isinstance(value, list) or not value:
        raise PatternCardError(f"{context} {field} must be a non-empty list")
    return value


def require_strings(values: list[Any], field: str, context: str) -> list[str]:
    if not all(isinstance(item, str) and item for item in values):
        raise PatternCardError(f"{context} {field} must contain non-empty strings")
    return list(values)


def case_ids(focused_cases: dict[str, Any]) -> set[str]:
    ids = {case["id"] for case in focused_cases.get("cases", []) if isinstance(case, dict)}
    ids.update(
        group["id"]
        for group in focused_cases.get("hard_negative_groups", [])
        if isinstance(group, dict)
    )
    return ids


def packet_ids(target_packets: dict[str, Any]) -> set[str]:
    return {
        packet["packet_id"]
        for packet in target_packets.get("packets", [])
        if isinstance(packet, dict)
    }


def fact_ids(proof_fact_registry: dict[str, Any]) -> set[str]:
    return {
        fact["fact_id"]
        for fact in proof_fact_registry.get("facts", [])
        if isinstance(fact, dict)
    }


def validate_ref(ref: str, allowed_refs: dict[str, set[str]], context: str) -> None:
    if "::" not in ref:
        raise PatternCardError(f"{context} evidence ref must use path::id: {ref}")
    path, local_id = ref.split("::", 1)
    if path not in allowed_refs:
        raise PatternCardError(f"{context} evidence ref path is not supported: {path}")
    if local_id not in allowed_refs[path]:
        raise PatternCardError(f"{context} evidence ref id does not exist: {ref}")


def validate_cards(
    cards: dict[str, Any],
    proof_fact_registry: dict[str, Any],
    focused_cases: dict[str, Any],
    target_packets: dict[str, Any],
) -> list[dict[str, Any]]:
    if cards.get("schema_version") != SCHEMA_VERSION:
        raise PatternCardError("semantic pattern cards schema_version must be 1")
    if cards.get("tool_version") != TOOL_VERSION:
        raise PatternCardError(f"semantic pattern cards tool_version must be {TOOL_VERSION}")

    patterns = cards.get("patterns")
    if not isinstance(patterns, list) or not patterns:
        raise PatternCardError("semantic pattern cards must contain patterns")

    allowed_facts = fact_ids(proof_fact_registry)
    allowed_refs = {
        "bench/type4/adversarial/cases/cases.v1.json": case_ids(focused_cases),
        "bench/type4/frontier_target_packets.v1.json": packet_ids(target_packets),
        "bench/type4/proof_fact_registry.v1.json": allowed_facts,
    }

    seen: set[str] = set()
    for pattern in patterns:
        if not isinstance(pattern, dict):
            raise PatternCardError("semantic pattern entries must be objects")
        pattern_id = pattern.get("pattern_id")
        context = f"pattern {pattern_id or '<missing>'}"
        if not isinstance(pattern_id, str) or not pattern_id:
            raise PatternCardError("semantic pattern missing pattern_id")
        if pattern_id in seen:
            raise PatternCardError(f"duplicate semantic pattern id: {pattern_id}")
        seen.add(pattern_id)
        for field in ("title", "law", "rationale"):
            if not isinstance(pattern.get(field), str) or not pattern[field]:
                raise PatternCardError(f"{context} {field} must be a non-empty string")
        if pattern.get("status") not in PATTERN_STATUS:
            raise PatternCardError(f"{context} has unknown status: {pattern.get('status')}")

        required_facts = require_strings(
            require_non_empty_list(pattern, "required_facts", context),
            "required_facts",
            context,
        )
        for fact_id in required_facts:
            if fact_id not in allowed_facts:
                raise PatternCardError(f"{context} references unknown proof fact {fact_id}")

        for field in ("hard_negative_templates", "boundaries", "evidence_refs"):
            values = require_strings(require_non_empty_list(pattern, field, context), field, context)
            if field == "evidence_refs":
                for ref in values:
                    validate_ref(ref, allowed_refs, context)

        surfaces = require_non_empty_list(pattern, "language_surfaces", context)
        seen_surfaces: set[str] = set()
        for surface in surfaces:
            if not isinstance(surface, dict):
                raise PatternCardError(f"{context} language_surfaces must contain objects")
            language = surface.get("language")
            if not isinstance(language, str) or not language:
                raise PatternCardError(f"{context} language surface missing language")
            if language in seen_surfaces:
                raise PatternCardError(f"{context} duplicates language surface {language}")
            seen_surfaces.add(language)
            if surface.get("status") not in SURFACE_STATUS:
                raise PatternCardError(
                    f"{context} surface {language} has unknown status: {surface.get('status')}"
                )
            if not isinstance(surface.get("surface"), str) or not surface["surface"]:
                raise PatternCardError(f"{context} surface {language} missing surface text")
            evidence = surface.get("evidence")
            if evidence is not None and not isinstance(evidence, str):
                raise PatternCardError(f"{context} surface {language} evidence must be string")
    return patterns


def render_markdown(patterns: list[dict[str, Any]]) -> str:
    lines = [
        "# Type-4 semantic pattern cards",
        "",
        "Generated by `bench/type4/semantic_pattern_cards.py` from",
        "`semantic_pattern_cards.v1.json`.",
        "",
        "These cards record reusable semantic laws before new detector behavior is",
        "opened. A language surface is not admitted merely because another surface",
        "looks similar; each surface still needs evidence, hard negatives, and the",
        "proof-carrying frontier gates required by the linked pattern.",
        "",
        "## Summary",
        "",
        "| pattern | status | proof facts | surfaces |",
        "|---|---|---:|---:|",
    ]
    for pattern in patterns:
        lines.append(
            f"| `{pattern['pattern_id']}` | `{pattern['status']}` | "
            f"{len(pattern['required_facts'])} | {len(pattern['language_surfaces'])} |"
        )
    for pattern in patterns:
        lines.extend(
            [
                "",
                f"## `{pattern['pattern_id']}`",
                "",
                f"**{pattern['title']}**",
                "",
                pattern["law"],
                "",
                f"- status: `{pattern['status']}`",
                f"- rationale: {pattern['rationale']}",
                "- required facts: "
                + ", ".join(f"`{fact}`" for fact in pattern["required_facts"]),
                "- hard-negative templates: "
                + ", ".join(f"`{item}`" for item in pattern["hard_negative_templates"]),
                "- boundaries: " + "; ".join(pattern["boundaries"]),
                "- evidence: " + ", ".join(f"`{ref}`" for ref in pattern["evidence_refs"]),
                "",
                "| language | surface | status | evidence |",
                "|---|---|---|---|",
            ]
        )
        for surface in pattern["language_surfaces"]:
            evidence = surface.get("evidence") or ""
            lines.append(
                f"| {surface['language']} | {surface['surface']} | "
                f"`{surface['status']}` | {evidence} |"
            )
    lines.append("")
    return "\n".join(lines)


def check_artifact(path: Path, expected: str) -> None:
    if not path.exists() or path.read_text() != expected:
        raise PatternCardError(f"semantic pattern markdown artifact is stale: {repo_rel(path)}")


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--cards", type=Path, default=DEFAULT_CARDS)
    parser.add_argument("--markdown-out", type=Path, default=DEFAULT_MARKDOWN)
    parser.add_argument("--proof-fact-registry", type=Path, default=DEFAULT_PROOF_FACT_REGISTRY)
    parser.add_argument("--focused-cases", type=Path, default=DEFAULT_FOCUSED_CASES)
    parser.add_argument("--target-packets", type=Path, default=DEFAULT_TARGET_PACKETS)
    parser.add_argument("--check", action="store_true", help="fail if markdown is stale")
    args = parser.parse_args()

    try:
        patterns = validate_cards(
            load_json(args.cards),
            load_json(args.proof_fact_registry),
            load_json(args.focused_cases),
            load_json(args.target_packets),
        )
        markdown = render_markdown(patterns)
        if args.check:
            check_artifact(args.markdown_out, markdown)
        else:
            args.markdown_out.write_text(markdown)
            print(f"wrote {repo_rel(args.markdown_out)}")
    except PatternCardError as exc:
        print(f"error: {exc}", file=sys.stderr)
        return 1
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
