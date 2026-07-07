#!/usr/bin/env python3
"""Shared helpers for Type-4 focused cases and frontier target packets."""

from __future__ import annotations

from collections import Counter
import json
from pathlib import Path
from typing import Any


ROOT = Path(__file__).resolve().parents[1]
TYPE4_ROOT = ROOT.parent
REPO_ROOT = ROOT.parents[2]
PACKETS_PATH = TYPE4_ROOT / "frontier_target_packets.v1.json"
REAL_FRONTIER_PATH = TYPE4_ROOT / "real_frontier.v1.json"
CASES_PATH = ROOT / "cases" / "cases.v1.json"
CASE_REF_PREFIX = "bench/type4/adversarial/cases/cases.v1.json::"
REGRESSION_GATE_SEPARATOR = "::"
EXECUTABLE_EXPECTATIONS = {"same-family", "split"}
HARD_NEGATIVE_CONVENTION_CATEGORIES = {
    "numeric",
    "boolean",
    "loop",
    "collection",
    "protocol-boundary",
}

ROUTE_WEIGHT = {
    "team-a-detector": 300,
    "proof-fact-prerequisite": 240,
    "team-c-product": 120,
}
EVIDENCE_WEIGHT = {
    "frontier-recorded": 120,
    "manually-audited": 90,
    "detector-suggested": 50,
    "pattern-signal": 10,
}


def load_json(path: Path) -> dict[str, Any]:
    return json.loads(path.read_text())


def load_all() -> tuple[dict[str, Any], dict[str, Any], dict[str, Any]]:
    return load_json(PACKETS_PATH), load_json(CASES_PATH), load_json(REAL_FRONTIER_PATH)


def packet_items(packet_doc: dict[str, Any]) -> list[dict[str, Any]]:
    return list(packet_doc.get("packets", []))


def case_items(cases: dict[str, Any]) -> list[dict[str, Any]]:
    return list(cases.get("cases", []))


def hard_negative_groups(cases: dict[str, Any]) -> list[dict[str, Any]]:
    return list(cases.get("hard_negative_groups", []))


def hard_negative_convention_ids(cases: dict[str, Any]) -> set[str]:
    conventions = cases.get("hard_negative_conventions", {})
    if not isinstance(conventions, dict):
        return set()
    result: set[str] = set()
    for values in conventions.values():
        if isinstance(values, list):
            result.update(item for item in values if isinstance(item, str))
    return result


def executable_expectation_items(cases: dict[str, Any]) -> list[tuple[dict[str, Any], dict[str, Any]]]:
    result = []
    for case in case_items(cases):
        for expectation in case.get("executable_expectations", []):
            result.append((case, expectation))
    return result


def case_index(cases: dict[str, Any]) -> dict[str, dict[str, Any]]:
    return {case["id"]: case for case in case_items(cases)}


def real_frontier_case_ids(real_frontier: dict[str, Any]) -> set[str]:
    return {item["case_id"] for item in real_frontier.get("items", []) if item.get("case_id")}


def find_packet(packet_doc: dict[str, Any], packet_id: str) -> dict[str, Any] | None:
    for packet in packet_items(packet_doc):
        if packet.get("packet_id") == packet_id:
            return packet
    return None


def find_case(cases: dict[str, Any], case_id: str) -> dict[str, Any] | None:
    for case in case_items(cases):
        if case.get("id") == case_id:
            return case
    return None


def validate_all(
    packet_doc: dict[str, Any], cases: dict[str, Any], real_frontier: dict[str, Any]
) -> list[str]:
    errors: list[str] = []

    if packet_doc.get("schema_version") != 1:
        errors.append("frontier_target_packets.v1.json schema_version must be 1")
    if cases.get("schema_version") != 1:
        errors.append("cases.v1.json schema_version must be 1")
    if real_frontier.get("schema_version") != 1:
        errors.append("real_frontier.v1.json schema_version must be 1")

    packets = packet_items(packet_doc)
    if packet_doc.get("packet_count") != len(packets):
        errors.append("frontier_target_packets.v1.json packet_count does not match packets length")

    _check_unique("target packet", packets, "packet_id", errors)
    _check_unique("focused case", case_items(cases), "id", errors)
    _check_unique("hard-negative group", hard_negative_groups(cases), "id", errors)

    route_vocabulary = set(packet_doc.get("owner_route_vocabulary", []))
    evidence_ids = real_frontier_case_ids(real_frontier)
    packet_ids = {packet.get("packet_id") for packet in packets}
    case_by_id = case_index(cases)
    group_by_id = {group.get("id"): group for group in hard_negative_groups(cases)}
    convention_ids = hard_negative_convention_ids(cases)

    for packet in packets:
        packet_id = packet.get("packet_id", "?")
        for field in (
            "packet_id",
            "candidate_axis",
            "owner_route",
            "evidence_tier",
            "evidence_case_ids",
            "semantic_claim",
            "proof_invariant",
            "hard_negative_siblings",
            "hard_negative_group_ids",
            "current_detector_result",
            "why_now",
            "locations",
        ):
            _require(packet, field, f"target packet {packet_id}", errors)

        if route_vocabulary and packet.get("owner_route") not in route_vocabulary:
            errors.append(f"target packet {packet_id} has invalid owner_route {packet.get('owner_route')}")
        for case_id in packet.get("evidence_case_ids", []):
            if case_id not in evidence_ids:
                errors.append(f"target packet {packet_id} references unknown real_frontier case {case_id}")
        for group_id in packet.get("hard_negative_group_ids", []):
            group = group_by_id.get(group_id)
            if group is None:
                errors.append(f"target packet {packet_id} references unknown hard-negative group {group_id}")
            elif packet_id not in group.get("packet_ids", []):
                errors.append(
                    f"target packet {packet_id} references hard-negative group {group_id} "
                    "that does not list the packet"
                )
        for idx, loc in enumerate(packet.get("locations", []), start=1):
            for field in ("repo", "path", "span", "primary_language", "split"):
                _require(loc, field, f"target packet {packet_id} location {idx}", errors)
        if packet.get("owner_route") == "proof-fact-prerequisite" and not packet.get("blocked_by"):
            errors.append(f"target packet {packet_id} is proof-fact-prerequisite but has no blocked_by list")

    expectation_ids: set[str] = set()
    for case in case_items(cases):
        case_id = case.get("id", "?")
        for field in ("id", "kind", "semantic_family", "claim"):
            _require(case, field, f"focused case {case_id}", errors)
        for fixture in case.get("fixtures", []):
            if not (REPO_ROOT / fixture).exists():
                errors.append(f"focused case {case_id} fixture does not exist: {fixture}")
        _validate_executable_expectations(case, expectation_ids, errors)

    if not convention_ids:
        errors.append("cases.v1.json must define hard_negative_conventions")
    conventions = cases.get("hard_negative_conventions", {})
    if isinstance(conventions, dict) and set(conventions) != HARD_NEGATIVE_CONVENTION_CATEGORIES:
        missing = sorted(HARD_NEGATIVE_CONVENTION_CATEGORIES - set(conventions))
        extra = sorted(set(conventions) - HARD_NEGATIVE_CONVENTION_CATEGORIES)
        errors.append(
            "hard_negative_conventions must define exactly "
            f"{sorted(HARD_NEGATIVE_CONVENTION_CATEGORIES)}; missing={missing}, extra={extra}"
        )
    for group in hard_negative_groups(cases):
        group_id = group.get("id", "?")
        for field in (
            "id",
            "semantic_family",
            "packet_ids",
            "conventions",
            "positive_cases",
            "hard_negative_cases",
            "regression_gates",
            "claim",
        ):
            _require(group, field, f"hard-negative group {group_id}", errors)
        for packet_id in group.get("packet_ids", []):
            if packet_id not in packet_ids:
                errors.append(
                    f"hard-negative group {group_id} references unknown packet {packet_id}"
                )
        for convention in group.get("conventions", []):
            if convention not in convention_ids:
                errors.append(
                    f"hard-negative group {group_id} references unknown convention {convention}"
                )
        for case_id in group.get("positive_cases", []):
            case = case_by_id.get(case_id)
            if case is None:
                errors.append(
                    f"hard-negative group {group_id} references unknown positive case {case_id}"
                )
            elif case.get("kind") != "positive":
                errors.append(
                    f"hard-negative group {group_id} positive case {case_id} is "
                    f"{case.get('kind')}"
                )
        for case_id in group.get("hard_negative_cases", []):
            case = case_by_id.get(case_id)
            if case is None:
                errors.append(
                    f"hard-negative group {group_id} references unknown hard-negative case {case_id}"
                )
            elif case.get("kind") != "hard-negative":
                errors.append(
                    f"hard-negative group {group_id} hard-negative case {case_id} is {case.get('kind')}"
                )
        expected_case_gates = {
            f"{CASE_REF_PREFIX}{case_id}"
            for case_id in group.get("positive_cases", []) + group.get("hard_negative_cases", [])
        }
        for gate_ref in group.get("regression_gates", []):
            _validate_regression_gate_ref(gate_ref, case_by_id, group_id, errors)
        missing_case_gates = sorted(expected_case_gates - set(group.get("regression_gates", [])))
        if missing_case_gates:
            errors.append(
                f"hard-negative group {group_id} regression_gates missing case refs: "
                f"{missing_case_gates}"
            )

    return errors


def _require(item: dict[str, Any], field: str, label: str, errors: list[str]) -> None:
    if field not in item or item[field] in ("", None, []):
        errors.append(f"{label} missing required field {field}")


def _validate_executable_expectations(
    case: dict[str, Any], expectation_ids: set[str], errors: list[str]
) -> None:
    case_id = case.get("id", "?")
    expectations = case.get("executable_expectations", [])
    if expectations in (None, []):
        return
    if not isinstance(expectations, list):
        errors.append(f"focused case {case_id} executable_expectations must be a list")
        return
    for expectation in expectations:
        label = f"focused case {case_id} executable expectation {expectation.get('id', '?')}"
        for field in ("id", "fixture", "expect", "members"):
            _require(expectation, field, label, errors)
        expectation_id = expectation.get("id")
        if expectation_id:
            if expectation_id in expectation_ids:
                errors.append(f"executable expectation id {expectation_id} appears more than once")
            expectation_ids.add(expectation_id)
        if expectation.get("expect") not in EXECUTABLE_EXPECTATIONS:
            errors.append(
                f"{label} expect must be one of {sorted(EXECUTABLE_EXPECTATIONS)}"
            )
        fixture = expectation.get("fixture")
        if isinstance(fixture, str) and not (REPO_ROOT / fixture).exists():
            errors.append(f"{label} fixture does not exist: {fixture}")
        query = expectation.get("query", {})
        if query and not isinstance(query, dict):
            errors.append(f"{label} query must be an object")
        elif isinstance(query, dict):
            if query.get("mode", "semantic") != "semantic":
                errors.append(f"{label} query.mode must be semantic")
            for field in ("min_size", "min_lines"):
                if field in query and not isinstance(query[field], int):
                    errors.append(f"{label} query.{field} must be an integer")
        members = expectation.get("members")
        if not isinstance(members, list) or len(members) < 2:
            errors.append(f"{label} members must contain at least two entries")
            continue
        for idx, member in enumerate(members, start=1):
            member_label = f"{label} member {idx}"
            if not isinstance(member, dict):
                errors.append(f"{member_label} must be an object")
                continue
            _require(member, "file", member_label, errors)
            member_file = member.get("file")
            if isinstance(member_file, str) and not (REPO_ROOT / member_file).is_file():
                errors.append(f"{member_label} file does not exist: {member_file}")
            has_name = isinstance(member.get("name"), str) and bool(member["name"])
            has_span = "start_line" in member and "end_line" in member
            if not has_name and not has_span:
                errors.append(f"{member_label} must define name or start_line/end_line")
            if has_span:
                if not isinstance(member.get("start_line"), int) or not isinstance(
                    member.get("end_line"), int
                ):
                    errors.append(f"{member_label} start_line/end_line must be integers")
                elif member["end_line"] < member["start_line"]:
                    errors.append(f"{member_label} end_line must be >= start_line")


def _validate_regression_gate_ref(
    gate_ref: Any,
    case_by_id: dict[str, dict[str, Any]],
    group_id: str,
    errors: list[str],
) -> None:
    if not isinstance(gate_ref, str) or not gate_ref:
        errors.append(f"hard-negative group {group_id} has an empty regression gate")
        return
    if gate_ref.startswith(CASE_REF_PREFIX):
        case_id = gate_ref.removeprefix(CASE_REF_PREFIX)
        if case_id not in case_by_id:
            errors.append(
                f"hard-negative group {group_id} regression gate references "
                f"unknown focused case {case_id}"
            )
        return
    if REGRESSION_GATE_SEPARATOR not in gate_ref:
        errors.append(
            f"hard-negative group {group_id} regression gate {gate_ref!r} must be "
            "a focused case ref or path::symbol"
        )
        return
    path_text, symbol = gate_ref.split(REGRESSION_GATE_SEPARATOR, 1)
    if not path_text or not symbol:
        errors.append(
            f"hard-negative group {group_id} regression gate {gate_ref!r} must be "
            "a focused case ref or path::symbol"
        )
        return
    gate_path = REPO_ROOT / path_text
    if not gate_path.is_file():
        errors.append(
            f"hard-negative group {group_id} regression gate file does not exist: {path_text}"
        )
        return
    if symbol not in gate_path.read_text():
        errors.append(
            f"hard-negative group {group_id} regression gate symbol {symbol!r} "
            f"not found in {path_text}"
        )


def _check_unique(
    label: str, items: list[dict[str, Any]], id_field: str, errors: list[str]
) -> None:
    ids = [item.get(id_field) for item in items]
    counts = Counter(ids)
    for item_id, count in counts.items():
        if not item_id:
            errors.append(f"{label} list contains item without {id_field}")
        elif count > 1:
            errors.append(f"{label} id {item_id} appears {count} times")


def packet_score(packet: dict[str, Any]) -> int:
    breadth = packet.get("breadth", {})
    return (
        ROUTE_WEIGHT.get(packet.get("owner_route"), 0)
        + EVIDENCE_WEIGHT.get(packet.get("evidence_tier"), 0)
        + int(10 * breadth.get("primary_language_presence", 0))
        + int(breadth.get("repo_presence", 0))
    )


def packet_summary(packet: dict[str, Any]) -> dict[str, Any]:
    return {
        "packet_id": packet["packet_id"],
        "score": packet_score(packet),
        "candidate_axis": packet["candidate_axis"],
        "owner_route": packet["owner_route"],
        "owner_issue": packet.get("owner_issue"),
        "evidence_tier": packet["evidence_tier"],
        "semantic_claim": packet["semantic_claim"],
        "blocked_by": packet.get("blocked_by", []),
        "evidence_case_ids": packet.get("evidence_case_ids", []),
    }


def packet_card(packet: dict[str, Any]) -> str:
    lines = [
        f"Packet: {packet['packet_id']}",
        f"Score: {packet_score(packet)}",
        f"Route: {packet['owner_route']}"
        + (f" ({packet['owner_issue']})" if packet.get("owner_issue") else ""),
        f"Axis: {packet['candidate_axis']}",
        f"Evidence tier: {packet['evidence_tier']}",
        "",
        "Semantic claim:",
        f"  {packet['semantic_claim']}",
        "",
        "Why now:",
        f"  {packet['why_now']}",
        "",
        "Proof invariant:",
        f"  {packet['proof_invariant']}",
    ]
    if packet.get("blocked_by"):
        lines.extend(["", "Blocked by:"])
        lines.extend(f"  - {item}" for item in packet["blocked_by"])
    if packet.get("hard_negative_siblings"):
        lines.extend(["", "Hard-negative siblings:"])
        lines.extend(f"  - {item}" for item in packet["hard_negative_siblings"])
    if packet.get("evidence_case_ids"):
        lines.extend(["", "Evidence cases:"])
        lines.extend(f"  - {item}" for item in packet["evidence_case_ids"])
    if packet.get("locations"):
        lines.extend(["", "Locations:"])
        for loc in packet["locations"]:
            lines.append(
                f"  - {loc['repo']}:{loc['path']}:{loc['span']}"
                f" ({loc.get('primary_language', '?')}, {loc.get('split', '?')})"
            )
    result = packet.get("current_detector_result", {})
    if result:
        lines.extend(["", "Current detector result:"])
        for field in ("baseline_result", "semantic_query_result", "default_query_result"):
            if result.get(field):
                lines.append(f"  - {field}: {result[field]}")
    return "\n".join(lines)


def case_card(case: dict[str, Any]) -> str:
    lines = [
        f"Case: {case['id']}",
        f"Kind: {case['kind']}",
        f"Family: {case['semantic_family']}",
        "",
        "Claim:",
        f"  {case['claim']}",
    ]
    if case.get("fixtures"):
        lines.extend(["", "Fixtures:"])
        lines.extend(f"  - {fixture}" for fixture in case["fixtures"])
    if case.get("mutation"):
        lines.extend(["", "Mutation:", f"  {case['mutation']}"])
    if case.get("evidence"):
        lines.extend(["", "Evidence:", f"  {case['evidence']}"])
    return "\n".join(lines)
