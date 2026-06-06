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

    route_vocabulary = set(packet_doc.get("owner_route_vocabulary", []))
    evidence_ids = real_frontier_case_ids(real_frontier)

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
        for idx, loc in enumerate(packet.get("locations", []), start=1):
            for field in ("repo", "path", "span", "primary_language", "split"):
                _require(loc, field, f"target packet {packet_id} location {idx}", errors)
        if packet.get("owner_route") == "proof-fact-prerequisite" and not packet.get("blocked_by"):
            errors.append(f"target packet {packet_id} is proof-fact-prerequisite but has no blocked_by list")

    for case in case_items(cases):
        case_id = case.get("id", "?")
        for field in ("id", "kind", "semantic_family", "claim"):
            _require(case, field, f"focused case {case_id}", errors)
        for fixture in case.get("fixtures", []):
            if not (REPO_ROOT / fixture).exists():
                errors.append(f"focused case {case_id} fixture does not exist: {fixture}")

    return errors


def _require(item: dict[str, Any], field: str, label: str, errors: list[str]) -> None:
    if field not in item or item[field] in ("", None, []):
        errors.append(f"{label} missing required field {field}")


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
        for field in ("baseline_result", "semantic_scan_result", "default_scan_result"):
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
