#!/usr/bin/env python3
"""Proof-carrying Type-4 frontier admission report.

This tool sits one layer above ``frontier_platform.py``. The platform decides
which evidence-backed Type-4 frontier packets exist; this report answers whether
any packet is ready to open exact semantic admission, and what proof/adversarial
evidence is still blocking it.

The gate is intentionally conservative:

* target packets must link real-frontier evidence;
* evidence fields copied into target packets must still match the source record;
* hard-negative siblings and proof invariants are required;
* proof-prerequisite packets must remain blocked until the proof fact exists;
* the adversarial co-evolution ledger is summarized as guardrail context, not as
  permission to widen semantics.
"""

from __future__ import annotations

import argparse
import hashlib
import json
import sys
from collections import Counter
from pathlib import Path
from typing import Any

HERE = Path(__file__).resolve().parent
ROOT = HERE.parents[1]

SCHEMA_VERSION = 1
TOOL_VERSION = "proof-carrying-frontier/1"

DEFAULT_TARGET_PACKETS = HERE / "frontier_target_packets.v1.json"
DEFAULT_REAL_FRONTIER = HERE / "real_frontier.v1.json"
DEFAULT_COEVO_LEDGER = ROOT / "bench" / "coevo" / "packets.v1.json"
DEFAULT_FOCUSED_CASES = HERE / "adversarial" / "cases" / "cases.v1.json"
DEFAULT_JSON_OUT = HERE / "proof_carrying_frontier.v1.json"
DEFAULT_MARKDOWN_OUT = HERE / "proof_carrying_frontier.md"
DEFAULT_READINESS_JSON_OUT = HERE / "frontier_readiness.v1.json"
DEFAULT_READINESS_MARKDOWN_OUT = HERE / "frontier_readiness.md"

REQUIRED_PACKET_FIELDS = {
    "packet_id",
    "candidate_axis",
    "semantic_claim",
    "locations",
    "current_detector_result",
    "proof_invariant",
    "hard_negative_siblings",
    "owner_route",
    "owner_issue",
    "evidence_case_ids",
    "hard_negative_group_ids",
    "breadth",
    "evidence_tier",
    "curated",
    "why_now",
    "proof_fact_model",
    "detector_admission",
    "blocked_by",
    "notes",
}

REQUIRED_DETECTOR_FIELDS = {
    "baseline_command",
    "baseline_result",
    "current_detector_miss",
}

OWNER_ROUTE = {"proof-fact-prerequisite", "team-a-detector", "team-c-product"}
DETECTOR_ADMISSION_STATUS = {
    "not-admitted",
    "controlled-slice-admitted",
    "real-pair-admitted",
}
FRONTIER_STATUSES = {"real-miss", "already-covered", "hard-negative", "unsupported", "closed"}
COEVO_VERDICTS = {
    "violation-fixed",
    "refuted",
    "recorded-low-prevalence",
    "deferred-issue",
    "green-confirmed",
}

HARD_NEGATIVE_CASE_REF_PREFIX = "bench/type4/adversarial/cases/cases.v1.json::"
REGRESSION_GATE_SEPARATOR = "::"
HARD_NEGATIVE_CONVENTION_CATEGORIES = {
    "numeric",
    "boolean",
    "loop",
    "collection",
    "protocol-boundary",
}

READINESS_GROUPS = {
    "ready-for-defender": {
        "rank": 0,
        "title": "Ready For Defender",
        "description": (
            "Packets with linked evidence, proof invariant, hard negatives, and no "
            "unresolved blockers. Only rows in this group may start an exact detector "
            "admission PR."
        ),
    },
    "blocked-on-proof": {
        "rank": 1,
        "title": "Blocked On Proof",
        "description": (
            "Packets where the next useful work is reusable proof evidence. Detector "
            "admission remains closed until the listed proof facts and hard-negative "
            "perimeter are satisfied."
        ),
    },
    "blocked-on-product": {
        "rank": 2,
        "title": "Blocked On Product",
        "description": (
            "Packets where proof evidence is not the current bottleneck but product "
            "output, runtime, or ownership blockers still prevent exact admission."
        ),
    },
    "admitted/resolved": {
        "rank": 3,
        "title": "Admitted Or Resolved",
        "description": (
            "Packets with an admitted detector scope or already-resolved current "
            "scope. Controlled rows can still list broader real-corpus gaps."
        ),
    },
}

READINESS_GROUP_ORDER = tuple(
    group
    for group, _meta in sorted(READINESS_GROUPS.items(), key=lambda item: item[1]["rank"])
)


class FrontierError(RuntimeError):
    pass


def load_json(path: Path) -> dict[str, Any]:
    try:
        return json.loads(path.read_text())
    except FileNotFoundError as exc:
        raise FrontierError(f"missing artifact: {repo_rel(path)}") from exc
    except json.JSONDecodeError as exc:
        raise FrontierError(f"invalid JSON in {repo_rel(path)}: {exc}") from exc


def repo_rel(path: Path) -> str:
    try:
        return str(path.resolve().relative_to(ROOT))
    except ValueError:
        return str(path)


def sha256_file(path: Path) -> str:
    return hashlib.sha256(path.read_bytes()).hexdigest()


def artifact_ref(path: Path) -> dict[str, Any]:
    return {
        "path": repo_rel(path),
        "sha256": sha256_file(path),
        "size_bytes": path.stat().st_size,
    }


def by_case_id(real_frontier: dict[str, Any]) -> dict[str, dict[str, Any]]:
    items = real_frontier.get("items")
    if not isinstance(items, list):
        raise FrontierError("real_frontier.v1.json must contain an items list")
    result = {}
    for item in items:
        case_id = item.get("case_id")
        if not case_id:
            raise FrontierError("real frontier item missing case_id")
        if case_id in result:
            raise FrontierError(f"duplicate real frontier case_id: {case_id}")
        status = item.get("status")
        if status not in FRONTIER_STATUSES:
            raise FrontierError(f"real frontier case {case_id} has unknown status: {status}")
        result[case_id] = item
    return result


def validate_packet_doc(packet_doc: dict[str, Any]) -> list[dict[str, Any]]:
    if packet_doc.get("schema_version") != SCHEMA_VERSION:
        raise FrontierError("frontier_target_packets.v1.json schema_version must be 1")
    packets = packet_doc.get("packets")
    if not isinstance(packets, list):
        raise FrontierError("frontier_target_packets.v1.json must contain a packets list")
    if packet_doc.get("packet_count") != len(packets):
        raise FrontierError("frontier target packet_count does not match packets length")
    seen = set()
    for packet in packets:
        packet_id = packet.get("packet_id")
        if not packet_id:
            raise FrontierError("target packet missing packet_id")
        if packet_id in seen:
            raise FrontierError(f"duplicate target packet_id: {packet_id}")
        seen.add(packet_id)
        missing = sorted(REQUIRED_PACKET_FIELDS - set(packet))
        if missing:
            raise FrontierError(f"packet {packet_id} missing fields: {missing}")
        if packet["owner_route"] not in OWNER_ROUTE:
            raise FrontierError(f"packet {packet_id} has unknown owner_route: {packet['owner_route']}")
        if not packet["evidence_case_ids"]:
            raise FrontierError(f"packet {packet_id} must link at least one evidence case")
        if not packet["proof_invariant"]:
            raise FrontierError(f"packet {packet_id} must name a proof invariant")
        if not packet["hard_negative_siblings"]:
            raise FrontierError(f"packet {packet_id} must name hard negatives")
        validate_proof_fact_model(packet_id, packet["proof_fact_model"])
        validate_detector_admission(packet_id, packet["detector_admission"])
        detector_missing = sorted(REQUIRED_DETECTOR_FIELDS - set(packet["current_detector_result"]))
        if detector_missing:
            raise FrontierError(
                f"packet {packet_id} detector result missing fields: {detector_missing}"
            )
        if packet["owner_route"] == "proof-fact-prerequisite" and not packet["blocked_by"]:
            raise FrontierError(
                f"packet {packet_id} routes to proof-fact-prerequisite but has no blockers"
            )
    return packets


def validate_detector_admission(packet_id: str, admission: Any) -> None:
    if not isinstance(admission, dict):
        raise FrontierError(f"packet {packet_id} detector_admission must be an object")
    status = admission.get("status")
    if status not in DETECTOR_ADMISSION_STATUS:
        raise FrontierError(f"packet {packet_id} has unknown detector admission status: {status}")
    for field in ("scope", "capabilities", "positive_gates", "hard_negative_gates"):
        if field not in admission:
            raise FrontierError(f"packet {packet_id} detector_admission missing {field}")
        if not admission[field]:
            raise FrontierError(f"packet {packet_id} detector_admission {field} is empty")
        if field in ("capabilities", "positive_gates", "hard_negative_gates"):
            value = admission[field]
            if not isinstance(value, list) or not all(
                isinstance(item, str) for item in value
            ):
                raise FrontierError(
                    f"packet {packet_id} detector_admission {field} must be list[str]"
                )
    if status != "real-pair-admitted" and not admission.get("remaining_real_pair_gap"):
        raise FrontierError(
            f"packet {packet_id} detector_admission needs remaining_real_pair_gap"
        )


def validate_proof_fact_model(packet_id: str, model: Any) -> None:
    if not isinstance(model, dict):
        raise FrontierError(f"packet {packet_id} proof_fact_model must be an object")
    facts = model.get("facts")
    if not isinstance(facts, list) or not facts:
        raise FrontierError(f"packet {packet_id} proof_fact_model must contain facts")
    seen = set()
    for fact in facts:
        if not isinstance(fact, dict):
            raise FrontierError(f"packet {packet_id} proof fact entries must be objects")
        fact_id = fact.get("fact_id")
        if not fact_id:
            raise FrontierError(f"packet {packet_id} proof fact missing fact_id")
        if fact_id in seen:
            raise FrontierError(f"packet {packet_id} duplicate proof fact: {fact_id}")
        seen.add(fact_id)
        for field in (
            "description",
            "accepted_evidence",
            "rejected_evidence",
            "current_real_pair_status",
        ):
            if field not in fact:
                raise FrontierError(f"packet {packet_id} proof fact {fact_id} missing {field}")
        if not isinstance(fact["accepted_evidence"], list) or not fact["accepted_evidence"]:
            raise FrontierError(
                f"packet {packet_id} proof fact {fact_id} needs accepted_evidence"
            )
        if not isinstance(fact["rejected_evidence"], list) or not fact["rejected_evidence"]:
            raise FrontierError(
                f"packet {packet_id} proof fact {fact_id} needs rejected_evidence"
            )


def validate_evidence_links(
    packets: list[dict[str, Any]], real_frontier: dict[str, Any]
) -> list[dict[str, Any]]:
    cases = by_case_id(real_frontier)
    link_rows = []
    for packet in packets:
        packet_id = packet["packet_id"]
        primary = cases.get(packet["evidence_case_ids"][0])
        if primary is None:
            raise FrontierError(
                f"packet {packet_id} links unknown evidence case {packet['evidence_case_ids'][0]}"
            )
        for case_id in packet["evidence_case_ids"]:
            case = cases.get(case_id)
            if case is None:
                raise FrontierError(f"packet {packet_id} links unknown evidence case {case_id}")
            link_rows.append(
                {
                    "packet_id": packet_id,
                    "case_id": case_id,
                    "status": case["status"],
                    "candidate_axis": case["candidate_axis"],
                }
            )
        drift_checks = {
            "semantic_claim": primary.get("semantic_claim"),
            "proof_invariant": primary.get("proof_invariant"),
            "hard_negative_siblings": primary.get("hard_negative_siblings"),
            "current_detector_result": primary.get("detector"),
        }
        for field, expected in drift_checks.items():
            if packet.get(field) != expected:
                raise FrontierError(
                    f"packet {packet_id} field {field} drifted from real frontier case "
                    f"{primary['case_id']}"
                )
        if primary["status"] != "real-miss":
            raise FrontierError(
                f"packet {packet_id} primary evidence must be real-miss, got {primary['status']}"
            )
    return link_rows


def case_ref_id(ref: str) -> str | None:
    if ref.startswith(HARD_NEGATIVE_CASE_REF_PREFIX):
        return ref.removeprefix(HARD_NEGATIVE_CASE_REF_PREFIX)
    return None


def case_refs(values: list[str]) -> set[str]:
    return {case_id for ref in values if (case_id := case_ref_id(ref))}


def validate_regression_gate_ref(
    gate_ref: str, case_by_id: dict[str, dict[str, Any]], group_id: str
) -> None:
    if not isinstance(gate_ref, str) or not gate_ref:
        raise FrontierError(f"hard-negative group {group_id} has an empty regression gate")
    if (case_id := case_ref_id(gate_ref)) is not None:
        if case_id not in case_by_id:
            raise FrontierError(
                f"hard-negative group {group_id} regression gate references "
                f"unknown focused case {case_id}"
            )
        return
    if REGRESSION_GATE_SEPARATOR not in gate_ref:
        raise FrontierError(
            f"hard-negative group {group_id} regression gate {gate_ref!r} must be "
            "a focused case ref or path::symbol"
        )
    path_text, symbol = gate_ref.split(REGRESSION_GATE_SEPARATOR, 1)
    if not path_text or not symbol:
        raise FrontierError(
            f"hard-negative group {group_id} regression gate {gate_ref!r} must be "
            "a focused case ref or path::symbol"
        )
    gate_path = ROOT / path_text
    if not gate_path.is_file():
        raise FrontierError(
            f"hard-negative group {group_id} regression gate file does not exist: {path_text}"
        )
    if symbol not in gate_path.read_text():
        raise FrontierError(
            f"hard-negative group {group_id} regression gate symbol {symbol!r} "
            f"not found in {path_text}"
        )


def validate_focused_cases_doc(focused_cases: dict[str, Any]) -> tuple[
    dict[str, dict[str, Any]], dict[str, dict[str, Any]], set[str]
]:
    if focused_cases.get("schema_version") != SCHEMA_VERSION:
        raise FrontierError("cases.v1.json schema_version must be 1")
    cases = focused_cases.get("cases")
    if not isinstance(cases, list):
        raise FrontierError("cases.v1.json must contain a cases list")
    case_by_id: dict[str, dict[str, Any]] = {}
    for case in cases:
        case_id = case.get("id")
        if not case_id:
            raise FrontierError("focused case missing id")
        if case_id in case_by_id:
            raise FrontierError(f"duplicate focused case id: {case_id}")
        case_by_id[case_id] = case
        for field in ("kind", "semantic_family", "claim"):
            if not case.get(field):
                raise FrontierError(f"focused case {case_id} missing {field}")

    conventions = focused_cases.get("hard_negative_conventions")
    if not isinstance(conventions, dict) or not conventions:
        raise FrontierError("cases.v1.json must define hard_negative_conventions")
    if set(conventions) != HARD_NEGATIVE_CONVENTION_CATEGORIES:
        missing = sorted(HARD_NEGATIVE_CONVENTION_CATEGORIES - set(conventions))
        extra = sorted(set(conventions) - HARD_NEGATIVE_CONVENTION_CATEGORIES)
        raise FrontierError(
            "hard_negative_conventions must define exactly "
            f"{sorted(HARD_NEGATIVE_CONVENTION_CATEGORIES)}; missing={missing}, extra={extra}"
        )
    convention_ids: set[str] = set()
    for category, ids in conventions.items():
        if category not in HARD_NEGATIVE_CONVENTION_CATEGORIES:
            raise FrontierError(f"unknown hard-negative convention category: {category}")
        if not isinstance(ids, list) or not ids:
            raise FrontierError(f"hard-negative convention category {category} is empty")
        for convention_id in ids:
            if not isinstance(convention_id, str) or not convention_id.startswith(f"{category}."):
                raise FrontierError(
                    f"hard-negative convention {convention_id!r} must start with {category}."
                )
            convention_ids.add(convention_id)

    groups = focused_cases.get("hard_negative_groups")
    if not isinstance(groups, list) or not groups:
        raise FrontierError("cases.v1.json must define hard_negative_groups")
    group_by_id: dict[str, dict[str, Any]] = {}
    for group in groups:
        group_id = group.get("id")
        if not group_id:
            raise FrontierError("hard-negative group missing id")
        if group_id in group_by_id:
            raise FrontierError(f"duplicate hard-negative group id: {group_id}")
        group_by_id[group_id] = group
        for field in (
            "semantic_family",
            "packet_ids",
            "conventions",
            "positive_cases",
            "hard_negative_cases",
            "regression_gates",
            "claim",
        ):
            if not group.get(field):
                raise FrontierError(f"hard-negative group {group_id} missing {field}")
        for convention_id in group["conventions"]:
            if convention_id not in convention_ids:
                raise FrontierError(
                    f"hard-negative group {group_id} references unknown convention {convention_id}"
                )
        for case_id in group["positive_cases"]:
            case = case_by_id.get(case_id)
            if case is None or case.get("kind") != "positive":
                raise FrontierError(
                    f"hard-negative group {group_id} positive case {case_id} "
                    "is missing or not positive"
                )
        for case_id in group["hard_negative_cases"]:
            case = case_by_id.get(case_id)
            if case is None or case.get("kind") != "hard-negative":
                raise FrontierError(
                    f"hard-negative group {group_id} hard-negative case {case_id} "
                    "is missing or not hard-negative"
                )
        expected_case_gates = {
            f"{HARD_NEGATIVE_CASE_REF_PREFIX}{case_id}"
            for case_id in group["positive_cases"] + group["hard_negative_cases"]
        }
        for gate_ref in group["regression_gates"]:
            validate_regression_gate_ref(gate_ref, case_by_id, group_id)
        missing_case_gates = sorted(expected_case_gates - set(group["regression_gates"]))
        if missing_case_gates:
            raise FrontierError(
                f"hard-negative group {group_id} regression_gates missing case refs: "
                f"{missing_case_gates}"
            )
    return case_by_id, group_by_id, convention_ids


def validate_hard_negative_linkage(
    packets: list[dict[str, Any]], focused_cases: dict[str, Any]
) -> list[dict[str, Any]]:
    case_by_id, group_by_id, _convention_ids = validate_focused_cases_doc(focused_cases)
    packet_ids = {packet["packet_id"] for packet in packets}
    for group_id, group in group_by_id.items():
        for packet_id in group["packet_ids"]:
            if packet_id not in packet_ids:
                raise FrontierError(
                    f"hard-negative group {group_id} references unknown packet {packet_id}"
                )

    rows = []
    for packet in packets:
        packet_id = packet["packet_id"]
        group_ids = packet.get("hard_negative_group_ids")
        if not isinstance(group_ids, list) or not group_ids:
            raise FrontierError(f"packet {packet_id} must cite hard_negative_group_ids")
        admission = packet["detector_admission"]
        positive_refs = case_refs(admission["positive_gates"])
        hard_negative_refs = case_refs(admission["hard_negative_gates"])
        if not positive_refs:
            raise FrontierError(f"packet {packet_id} must cite at least one focused positive gate")
        if not hard_negative_refs:
            raise FrontierError(
                f"packet {packet_id} must cite at least one focused hard-negative gate"
            )

        for group_id in group_ids:
            group = group_by_id.get(group_id)
            if group is None:
                raise FrontierError(
                    f"packet {packet_id} cites unknown hard-negative group {group_id}"
                )
            if packet_id not in group["packet_ids"]:
                raise FrontierError(
                    f"packet {packet_id} cites hard-negative group {group_id} "
                    "that does not list the packet"
                )
            missing_positive = sorted(set(group["positive_cases"]) - positive_refs)
            missing_negative = sorted(set(group["hard_negative_cases"]) - hard_negative_refs)
            gate_set = set(admission["positive_gates"]) | set(admission["hard_negative_gates"])
            missing_gates = sorted(set(group["regression_gates"]) - gate_set)
            if missing_positive:
                raise FrontierError(
                    f"packet {packet_id} hard-negative group {group_id} missing "
                    f"positive gates: {missing_positive}"
                )
            if missing_negative:
                raise FrontierError(
                    f"packet {packet_id} hard-negative group {group_id} missing "
                    f"hard-negative gates: {missing_negative}"
                )
            if missing_gates:
                raise FrontierError(
                    f"packet {packet_id} hard-negative group {group_id} missing "
                    f"regression gates: {missing_gates}"
                )
            for case_id in group["positive_cases"]:
                if case_by_id[case_id]["semantic_family"] != group["semantic_family"]:
                    raise FrontierError(
                        f"group {group_id} positive case {case_id} semantic family drifted"
                    )
            for case_id in group["hard_negative_cases"]:
                if case_by_id[case_id]["semantic_family"] != group["semantic_family"]:
                    raise FrontierError(
                        f"group {group_id} hard-negative case {case_id} semantic family drifted"
                    )
            rows.append(
                {
                    "packet_id": packet_id,
                    "group_id": group_id,
                    "semantic_family": group["semantic_family"],
                    "conventions": list(group["conventions"]),
                    "positive_cases": list(group["positive_cases"]),
                    "hard_negative_cases": list(group["hard_negative_cases"]),
                    "regression_gates": list(group["regression_gates"]),
                }
            )
    return rows


def readiness_for(packet: dict[str, Any]) -> dict[str, Any]:
    blockers = list(packet.get("blocked_by") or [])
    admission = packet.get("detector_admission") or {}
    if admission.get("status") == "controlled-slice-admitted":
        return {
            "status": "detector-admitted-controlled",
            "can_open_exact_admission": False,
            "reason": (
                "proof-backed controlled detector slice is admitted; real-corpus exact "
                "admission still needs the remaining proof evidence"
            ),
            "blocking_items": blockers,
        }
    if admission.get("status") == "real-pair-admitted":
        return {
            "status": "detector-admitted",
            "can_open_exact_admission": False,
            "reason": "linked real-corpus packet has already been admitted",
            "blocking_items": blockers,
        }
    if packet["owner_route"] == "proof-fact-prerequisite":
        return {
            "status": "blocked-on-proof",
            "can_open_exact_admission": False,
            "reason": "packet still requires reusable proof evidence before detector work",
            "blocking_items": blockers,
        }
    if blockers:
        return {
            "status": "blocked",
            "can_open_exact_admission": False,
            "reason": "packet has unresolved blockers",
            "blocking_items": blockers,
        }
    return {
        "status": "ready-for-defender",
        "can_open_exact_admission": True,
        "reason": "packet has linked evidence, proof invariant, hard negatives, and no blockers",
        "blocking_items": [],
    }


def summarize_packets(packets: list[dict[str, Any]]) -> tuple[list[dict[str, Any]], dict[str, Any]]:
    rows = []
    by_status: Counter[str] = Counter()
    by_route: Counter[str] = Counter()
    by_detector_admission: Counter[str] = Counter()
    for packet in packets:
        readiness = readiness_for(packet)
        by_status[readiness["status"]] += 1
        by_route[packet["owner_route"]] += 1
        admission = packet["detector_admission"]
        by_detector_admission[admission["status"]] += 1
        rows.append(
            {
                "packet_id": packet["packet_id"],
                "candidate_axis": packet["candidate_axis"],
                "semantic_claim": packet["semantic_claim"],
                "owner_route": packet["owner_route"],
                "owner_issue": packet["owner_issue"],
                "evidence_case_ids": packet["evidence_case_ids"],
                "hard_negative_group_ids": packet["hard_negative_group_ids"],
                "evidence_tier": packet["evidence_tier"],
                "proof_invariant": packet["proof_invariant"],
                "hard_negative_count": len(packet["hard_negative_siblings"]),
                "why_now": packet["why_now"],
                "proof_fact_model": {
                    "model_status": packet["proof_fact_model"].get("model_status", "unknown"),
                    "fact_ids": [
                        fact["fact_id"] for fact in packet["proof_fact_model"]["facts"]
                    ],
                    "fact_count": len(packet["proof_fact_model"]["facts"]),
                },
                "detector_admission": {
                    "status": admission["status"],
                    "scope": admission["scope"],
                    "capabilities": list(admission["capabilities"]),
                    "positive_gate_count": len(admission["positive_gates"]),
                    "hard_negative_gate_count": len(admission["hard_negative_gates"]),
                    "remaining_real_pair_gap": admission.get("remaining_real_pair_gap"),
                },
                "readiness": readiness,
            }
        )
    summary = {
        "packet_count": len(packets),
        "by_readiness": dict(sorted(by_status.items())),
        "by_owner_route": dict(sorted(by_route.items())),
        "by_detector_admission": dict(sorted(by_detector_admission.items())),
        "ready_packet_count": sum(
            1 for row in rows if row["readiness"]["can_open_exact_admission"]
        ),
        "detector_admitted_packet_count": sum(
            1
            for row in rows
            if row["detector_admission"]["status"]
            in {"controlled-slice-admitted", "real-pair-admitted"}
        ),
    }
    return rows, summary


def summarize_coevo(coevo: dict[str, Any]) -> dict[str, Any]:
    packets = coevo.get("packets")
    if not isinstance(packets, list):
        raise FrontierError("coevo packets.v1.json must contain a packets list")
    verdicts: Counter[str] = Counter()
    surfaces: Counter[str] = Counter()
    modes: Counter[str] = Counter()
    personas: Counter[str] = Counter()
    latest_series = 0
    executable = 0
    for packet in packets:
        packet_id = packet.get("packet_id", "<missing>")
        verdict = packet.get("verdict")
        if verdict not in COEVO_VERDICTS:
            raise FrontierError(f"coevo packet {packet_id} has unknown verdict: {verdict}")
        verdicts[verdict] += 1
        surfaces[str(packet.get("surface", "unknown"))] += 1
        modes[str(packet.get("mode", "unknown"))] += 1
        personas[str(packet.get("persona", "unknown"))] += 1
        latest_series = max(latest_series, int(packet.get("series") or 0))
        mode = str(packet.get("mode", ""))
        if "executable" in mode or mode == "measurement":
            executable += 1
    return {
        "packet_count": len(packets),
        "latest_series": latest_series,
        "executable_or_measurement_packets": executable,
        "by_verdict": dict(sorted(verdicts.items())),
        "by_surface": dict(sorted(surfaces.items())),
        "by_mode": dict(sorted(modes.items())),
        "by_persona": dict(sorted(personas.items())),
        "guardrail_interpretation": (
            "The co-evolution ledger is active guardrail evidence. It does not by itself "
            "open exact admission; target packets still need proof invariants and hard negatives."
        ),
    }


def admission_verdict(ready_count: int) -> str:
    return "exact-admission-ready" if ready_count else "no-exact-admission-ready-packets"


def build_report(
    target_packets_path: Path,
    real_frontier_path: Path,
    coevo_ledger_path: Path,
    focused_cases_path: Path,
) -> dict[str, Any]:
    packet_doc = load_json(target_packets_path)
    real_frontier = load_json(real_frontier_path)
    coevo = load_json(coevo_ledger_path)
    focused_cases = load_json(focused_cases_path)
    packets = validate_packet_doc(packet_doc)
    evidence_links = validate_evidence_links(packets, real_frontier)
    hard_negative_linkage = validate_hard_negative_linkage(packets, focused_cases)
    packet_rows, packet_summary = summarize_packets(packets)
    coevo_summary = summarize_coevo(coevo)
    verdict = admission_verdict(packet_summary["ready_packet_count"])
    return {
        "schema_version": SCHEMA_VERSION,
        "tool_version": TOOL_VERSION,
        "identity": {
            "artifacts": {
                "target_packets": artifact_ref(target_packets_path),
                "real_frontier": artifact_ref(real_frontier_path),
                "coevo_ledger": artifact_ref(coevo_ledger_path),
                "focused_cases": artifact_ref(focused_cases_path),
            },
            "target_packet_identity": packet_doc.get("identity", {}),
        },
        "admission_policy": {
            "exact_admission_requires": [
                "linked real_frontier real-miss evidence",
                "proof invariant narrow enough to defend",
                "adjacent hard-negative siblings",
                "packet-level hard-negative group linkage",
                "current detector result showing the present boundary",
                "no unresolved proof or soundness blockers",
                "product-output and runtime evidence before merge",
            ],
            "co_evolution_role": (
                "adversarial packets price and defend claims; they are guardrails, not a "
                "substitute for proof-carrying target packets"
            ),
        },
        "verdict": verdict,
        "target_packets": {
            "summary": packet_summary,
            "packets": packet_rows,
            "evidence_links": evidence_links,
            "hard_negative_linkage": hard_negative_linkage,
        },
        "coevolution_guardrails": coevo_summary,
    }


def markdown_report(report: dict[str, Any]) -> str:
    target = report["target_packets"]["summary"]
    coevo = report["coevolution_guardrails"]
    by_readiness = json.dumps(target["by_readiness"], sort_keys=True)
    by_owner_route = json.dumps(target["by_owner_route"], sort_keys=True)
    by_detector_admission = json.dumps(target["by_detector_admission"], sort_keys=True)
    by_verdict = json.dumps(coevo["by_verdict"], sort_keys=True)
    lines = [
        "# Proof-carrying Type-4 frontier",
        "",
        "Admission report for evidence-priced Type-4 expansion. Generated by",
        "`bench/type4/proof_carrying_frontier.py` from target packets, real-frontier",
        "evidence, and the co-evolution packet ledger.",
        "",
        "For release and roadmap triage, start with `frontier_readiness.md`; this",
        "report keeps the fuller evidence and admission boundary.",
        "",
        "## Verdict",
        "",
        f"**{report['verdict']}**",
        "",
        f"- target packets: {target['packet_count']}",
        f"- ready for exact admission: {target['ready_packet_count']}",
        f"- detector admitted packets: {target['detector_admitted_packet_count']}",
        f"- by readiness: `{by_readiness}`",
        f"- by owner route: `{by_owner_route}`",
        f"- by detector admission: `{by_detector_admission}`",
        "",
        "## Admission Policy",
        "",
    ]
    lines.extend(f"- {item}" for item in report["admission_policy"]["exact_admission_requires"])
    lines += [
        "",
        "## Target Packets",
        "",
        "| packet | axis | route | readiness | proof facts | hard negatives | groups |",
        "|---|---|---|---|---:|---:|---:|",
    ]
    for packet in report["target_packets"]["packets"]:
        readiness = packet["readiness"]
        lines.append(
            f"| `{packet['packet_id']}` | `{packet['candidate_axis']}` | "
            f"`{packet['owner_route']}` | `{readiness['status']}` | "
            f"{packet['proof_fact_model']['fact_count']} | "
            f"{packet['hard_negative_count']} | "
            f"{len(packet['hard_negative_group_ids'])} |"
        )
    lines += ["", "## Packet Details", ""]
    for packet in report["target_packets"]["packets"]:
        readiness = packet["readiness"]
        admission = packet["detector_admission"]
        lines.append(f"### `{packet['packet_id']}`")
        lines.append("")
        lines.append(
            f"- detector admission: `{admission['status']}` over {admission['scope']}"
        )
        if admission.get("remaining_real_pair_gap"):
            lines.append(f"- remaining real-pair gap: {admission['remaining_real_pair_gap']}")
        lines.append(
            f"- gates: {admission['positive_gate_count']} positive, "
            f"{admission['hard_negative_gate_count']} hard-negative"
        )
        if readiness["blocking_items"]:
            model_status = packet["proof_fact_model"]["model_status"]
            facts = ", ".join(f"`{fact}`" for fact in packet["proof_fact_model"]["fact_ids"])
            lines.append(f"- proof fact model: `{model_status}`; facts: {facts}")
            lines.append("- blocked by:")
            lines.extend(f"  - {item}" for item in readiness["blocking_items"])
        groups = ", ".join(f"`{group}`" for group in packet["hard_negative_group_ids"])
        lines.append(f"- hard-negative groups: {groups}")
        lines.append("")
    lines += ["## Hard-Negative Linkage", ""]
    for row in report["target_packets"]["hard_negative_linkage"]:
        conventions = ", ".join(f"`{item}`" for item in row["conventions"])
        lines.append(f"### `{row['packet_id']}` / `{row['group_id']}`")
        lines.append("")
        lines.append(f"- semantic family: `{row['semantic_family']}`")
        lines.append(f"- conventions: {conventions}")
        lines.append(
            f"- cases: {len(row['positive_cases'])} positive, "
            f"{len(row['hard_negative_cases'])} hard-negative"
        )
        lines.append(f"- regression gates: {len(row['regression_gates'])}")
        lines.append("")
    lines += [
        "## Co-Evolution Guardrails",
        "",
        f"- packets: {coevo['packet_count']} across {len(coevo['by_surface'])} surfaces",
        f"- latest series: {coevo['latest_series']}",
        f"- executable or measurement packets: {coevo['executable_or_measurement_packets']}",
        f"- by verdict: `{by_verdict}`",
        "",
        coevo["guardrail_interpretation"],
    ]
    return "\n".join(lines).rstrip() + "\n"


def readiness_group_for(packet: dict[str, Any]) -> str:
    status = packet["readiness"]["status"]
    if status == "ready-for-defender":
        return "ready-for-defender"
    if status in {"detector-admitted-controlled", "detector-admitted"}:
        return "admitted/resolved"
    if status == "blocked-on-proof":
        return "blocked-on-proof"
    return "blocked-on-product"


def readiness_sort_key(packet: dict[str, Any]) -> tuple[int, str, str]:
    group = readiness_group_for(packet)
    admission = packet["detector_admission"]["status"]
    return (READINESS_GROUPS[group]["rank"], admission, packet["packet_id"])


def release_note_for(packet: dict[str, Any], group: str) -> str:
    packet_id = packet["packet_id"]
    axis = packet["candidate_axis"]
    admission = packet["detector_admission"]
    facts = ", ".join(packet["proof_fact_model"]["fact_ids"])
    if group == "ready-for-defender":
        return (
            f"{packet_id}: ready to open exact detector admission for {axis}; "
            f"the packet has linked real-frontier evidence, a proof invariant, and "
            f"{packet['hard_negative_count']} hard-negative siblings."
        )
    if group == "blocked-on-proof":
        return (
            f"{packet_id}: blocked on proof evidence for {axis}; no detector admission "
            f"is claimed until these facts are satisfied: {facts}."
        )
    if group == "blocked-on-product":
        return (
            f"{packet_id}: exact admission for {axis} remains closed until the listed "
            f"product, runtime, or ownership blockers are resolved."
        )
    if admission["status"] == "controlled-slice-admitted":
        return (
            f"{packet_id}: a controlled detector slice is admitted for {axis} "
            f"({admission['scope']}); linked real-corpus exact admission remains closed "
            f"because {admission['remaining_real_pair_gap']}."
        )
    return (
        f"{packet_id}: the linked real-corpus detector scope for {axis} is already "
        "admitted or resolved; no new detector behavior is opened from this row."
    )


def next_action_for(packet: dict[str, Any], group: str) -> str:
    admission = packet["detector_admission"]
    facts = ", ".join(packet["proof_fact_model"]["fact_ids"])
    blockers = packet["readiness"]["blocking_items"]
    if group == "ready-for-defender":
        return (
            "Open a defender PR scoped to this packet's proof invariant, positive "
            "gates, and hard-negative gates."
        )
    if group == "blocked-on-proof":
        return (
            f"Model or cite reusable proof facts ({facts}) before opening detector "
            "admission."
        )
    if group == "blocked-on-product":
        first = blockers[0] if blockers else "resolve the packet's product blockers"
        return f"Resolve blocker first: {first}"
    if admission["status"] == "controlled-slice-admitted":
        return (
            "Do not widen this detector row from the readiness queue; use the remaining "
            "real-pair gap as proof follow-up."
        )
    return "No follow-up is queued from the readiness artifact."


def readiness_packet_view(packet: dict[str, Any]) -> dict[str, Any]:
    group = readiness_group_for(packet)
    return {
        "packet_id": packet["packet_id"],
        "candidate_axis": packet["candidate_axis"],
        "group": group,
        "readiness_status": packet["readiness"]["status"],
        "detector_admission_status": packet["detector_admission"]["status"],
        "can_open_exact_admission": packet["readiness"]["can_open_exact_admission"],
        "owner_route": packet["owner_route"],
        "owner_issue": packet["owner_issue"],
        "evidence_case_ids": packet["evidence_case_ids"],
        "hard_negative_group_ids": packet["hard_negative_group_ids"],
        "evidence_tier": packet["evidence_tier"],
        "proof_fact_model_status": packet["proof_fact_model"]["model_status"],
        "proof_fact_ids": packet["proof_fact_model"]["fact_ids"],
        "hard_negative_count": packet["hard_negative_count"],
        "positive_gate_count": packet["detector_admission"]["positive_gate_count"],
        "hard_negative_gate_count": packet["detector_admission"]["hard_negative_gate_count"],
        "detector_scope": packet["detector_admission"]["scope"],
        "remaining_real_pair_gap": packet["detector_admission"]["remaining_real_pair_gap"],
        "blockers": packet["readiness"]["blocking_items"],
        "semantic_claim": packet["semantic_claim"],
        "proof_invariant": packet["proof_invariant"],
        "why_now": packet["why_now"],
        "planning_summary": next_action_for(packet, group),
        "release_note": release_note_for(packet, group),
    }


def build_readiness_summary(report: dict[str, Any]) -> dict[str, Any]:
    grouped: dict[str, dict[str, Any]] = {
        group: {
            "title": READINESS_GROUPS[group]["title"],
            "description": READINESS_GROUPS[group]["description"],
            "count": 0,
            "packets": [],
        }
        for group in READINESS_GROUP_ORDER
    }
    packets = sorted(report["target_packets"]["packets"], key=readiness_sort_key)
    for packet in packets:
        view = readiness_packet_view(packet)
        group = view["group"]
        grouped[group]["packets"].append(view)
        grouped[group]["count"] += 1

    next_work = next_work_from_groups(grouped)
    group_list = [
        {"group": group, **grouped[group]}
        for group in READINESS_GROUP_ORDER
    ]
    return {
        "schema_version": SCHEMA_VERSION,
        "tool_version": TOOL_VERSION,
        "source_report_verdict": report["verdict"],
        "source_artifacts": report["identity"]["artifacts"],
        "group_order": list(READINESS_GROUP_ORDER),
        "group_list": group_list,
        "policy": {
            "exact_detector_admission_opens_only_from": "ready-for-defender",
            "non_ready_rows_do_not_claim_exact_real_pair_admission": True,
            "detector_admitted_controlled_is_not_real_pair_admission": True,
        },
        "next_work": next_work,
        "groups": grouped,
    }


def next_work_from_groups(groups: dict[str, dict[str, Any]]) -> dict[str, Any]:
    for group in ("ready-for-defender", "blocked-on-proof", "blocked-on-product"):
        packets = groups[group]["packets"]
        if packets:
            packet = packets[0]
            return {
                "group": group,
                "packet_id": packet["packet_id"],
                "candidate_axis": packet["candidate_axis"],
                "why": packet["planning_summary"],
            }
    packets = groups["admitted/resolved"]["packets"]
    if packets:
        return {
            "group": "admitted/resolved",
            "packet_id": None,
            "candidate_axis": None,
            "why": (
                "No non-admitted frontier packet is queued; admitted/resolved rows "
                "list only follow-up gaps."
            ),
        }
    return {
        "group": "none",
        "packet_id": None,
        "candidate_axis": None,
        "why": "No target packets are present.",
    }


def markdown_cell(value: Any) -> str:
    return str(value).replace("|", "\\|").replace("\n", " ")


def markdown_readiness(summary: dict[str, Any]) -> str:
    next_work = summary["next_work"]
    lines = [
        "# Type-4 frontier readiness",
        "",
        "Compact roadmap view for proof-carrying Type-4 packets. Generated by",
        "`bench/type4/proof_carrying_frontier.py` from the same checked inputs as",
        "`proof_carrying_frontier.v1.json`.",
        "",
        "## Next Work",
        "",
        f"- source verdict: `{summary['source_report_verdict']}`",
        f"- next group: `{next_work['group']}`",
        f"- next packet: `{next_work['packet_id'] or 'none'}`",
        f"- next axis: `{next_work['candidate_axis'] or 'none'}`",
        f"- action: {next_work['why']}",
        "",
        "Exact detector admission may only open from `ready-for-defender` rows. Rows",
        "in other groups are planning evidence and do not open exact real-pair admission.",
        "",
    ]
    for group in READINESS_GROUP_ORDER:
        data = summary["groups"][group]
        lines += [
            f"## {data['title']}",
            "",
            data["description"],
            "",
            f"Count: {data['count']}",
            "",
        ]
        if not data["packets"]:
            lines += ["_None._", ""]
            continue
        lines += [
            "| packet | axis | readiness | detector admission | hard-negative groups | action |",
            "|---|---|---|---|---|---|",
        ]
        for packet in data["packets"]:
            groups = ", ".join(
                f"`{markdown_cell(group)}`" for group in packet["hard_negative_group_ids"]
            )
            lines.append(
                f"| `{markdown_cell(packet['packet_id'])}` | "
                f"`{markdown_cell(packet['candidate_axis'])}` | "
                f"`{markdown_cell(packet['readiness_status'])}` | "
                f"`{markdown_cell(packet['detector_admission_status'])}` | "
                f"{groups} | "
                f"{markdown_cell(packet['planning_summary'])} |"
            )
        lines += ["", "Release-note wording:", ""]
        for packet in data["packets"]:
            lines.append(f"- {packet['release_note']}")
        lines.append("")
        blocked_packets = [packet for packet in data["packets"] if packet["blockers"]]
        if blocked_packets:
            lines += ["Blockers:", ""]
            for packet in blocked_packets:
                lines.append(f"- `{packet['packet_id']}`")
                lines.extend(f"  - {item}" for item in packet["blockers"])
            lines.append("")
    return "\n".join(lines).rstrip() + "\n"


def canonical_json(doc: dict[str, Any]) -> str:
    return json.dumps(doc, indent=2, sort_keys=True) + "\n"


def check_artifacts(
    report: dict[str, Any],
    json_out: Path,
    markdown_out: Path,
    readiness_json_out: Path,
    readiness_markdown_out: Path,
) -> None:
    expected_json = canonical_json(report)
    expected_md = markdown_report(report)
    readiness = build_readiness_summary(report)
    expected_readiness_json = canonical_json(readiness)
    expected_readiness_md = markdown_readiness(readiness)
    mismatches = []
    if not json_out.exists() or json_out.read_text() != expected_json:
        mismatches.append(repo_rel(json_out))
    if not markdown_out.exists() or markdown_out.read_text() != expected_md:
        mismatches.append(repo_rel(markdown_out))
    if (
        not readiness_json_out.exists()
        or readiness_json_out.read_text() != expected_readiness_json
    ):
        mismatches.append(repo_rel(readiness_json_out))
    if (
        not readiness_markdown_out.exists()
        or readiness_markdown_out.read_text() != expected_readiness_md
    ):
        mismatches.append(repo_rel(readiness_markdown_out))
    if mismatches:
        joined = ", ".join(mismatches)
        raise FrontierError(f"proof-carrying frontier artifacts are stale: {joined}")


def write_artifacts(
    report: dict[str, Any],
    json_out: Path,
    markdown_out: Path,
    readiness_json_out: Path,
    readiness_markdown_out: Path,
) -> None:
    readiness = build_readiness_summary(report)
    json_out.write_text(canonical_json(report))
    markdown_out.write_text(markdown_report(report))
    readiness_json_out.write_text(canonical_json(readiness))
    readiness_markdown_out.write_text(markdown_readiness(readiness))


def selftest() -> None:
    file_gate = "bench/type4/proof_carrying_frontier.py::selftest"
    packet = {
        "packet_id": "p",
        "candidate_axis": "axis",
        "semantic_claim": "same behavior",
        "locations": [{"repo": "r", "path": "a.py", "span": "1", "snippet": "x"}],
        "current_detector_result": {
            "baseline_command": "nose query",
            "baseline_result": "miss",
            "current_detector_miss": True,
        },
        "proof_invariant": "narrow proof",
        "hard_negative_siblings": ["changed predicate"],
        "owner_route": "proof-fact-prerequisite",
        "owner_issue": None,
        "evidence_case_ids": ["c"],
        "hard_negative_group_ids": ["g"],
        "breadth": {},
        "evidence_tier": "frontier-recorded",
        "curated": {},
        "why_now": "priced",
        "proof_fact_model": {
            "model_status": "modeled-for-controlled-evidence",
            "facts": [
                {
                    "fact_id": "numeric-clamp.bound-order",
                    "description": "lo <= hi proof",
                    "accepted_evidence": ["asserted guard evidence"],
                    "rejected_evidence": ["parameter names"],
                    "current_real_pair_status": "unsatisfied",
                }
            ],
        },
        "detector_admission": {
            "status": "not-admitted",
            "scope": "none",
            "capabilities": ["none"],
            "positive_gates": [f"{HARD_NEGATIVE_CASE_REF_PREFIX}positive"],
            "hard_negative_gates": [f"{HARD_NEGATIVE_CASE_REF_PREFIX}negative", file_gate],
            "remaining_real_pair_gap": "missing proof",
        },
        "blocked_by": ["missing proof"],
        "notes": "n/a",
    }
    packet_doc = {"schema_version": 1, "packet_count": 1, "packets": [packet]}
    packets = validate_packet_doc(packet_doc)
    assert readiness_for(packet)["status"] == "blocked-on-proof"
    focused_cases = {
        "schema_version": 1,
        "hard_negative_conventions": {
            "numeric": ["numeric.precondition"],
            "boolean": ["boolean.truth-table"],
            "loop": ["loop.short-circuit"],
            "collection": ["collection.cardinality"],
            "protocol-boundary": ["protocol-boundary.api-identity"],
        },
        "hard_negative_groups": [
            {
                "id": "g",
                "semantic_family": "axis.family",
                "packet_ids": ["p"],
                "conventions": ["numeric.precondition"],
                "positive_cases": ["positive"],
                "hard_negative_cases": ["negative"],
                "regression_gates": [
                    file_gate,
                    f"{HARD_NEGATIVE_CASE_REF_PREFIX}positive",
                    f"{HARD_NEGATIVE_CASE_REF_PREFIX}negative",
                ],
                "claim": "positive and hard-negative gates move together",
            }
        ],
        "cases": [
            {
                "id": "positive",
                "kind": "positive",
                "semantic_family": "axis.family",
                "claim": "positive",
            },
            {
                "id": "negative",
                "kind": "hard-negative",
                "semantic_family": "axis.family",
                "claim": "negative",
            },
        ],
    }
    real_frontier = {
        "items": [
            {
                "case_id": "c",
                "status": "real-miss",
                "candidate_axis": "axis",
                "semantic_claim": "same behavior",
                "proof_invariant": "narrow proof",
                "hard_negative_siblings": ["changed predicate"],
                "detector": packet["current_detector_result"],
            }
        ]
    }
    links = validate_evidence_links(packets, real_frontier)
    assert links == [
        {
            "packet_id": "p",
            "case_id": "c",
            "status": "real-miss",
            "candidate_axis": "axis",
        }
    ]
    linkage = validate_hard_negative_linkage(packets, focused_cases)
    assert linkage[0]["group_id"] == "g"
    drifted = json.loads(json.dumps(packet_doc))
    drifted["packets"][0]["proof_invariant"] = "changed"
    try:
        validate_evidence_links(validate_packet_doc(drifted), real_frontier)
        raise AssertionError("drift was not detected")
    except FrontierError:
        pass
    ready = dict(packet)
    ready["owner_route"] = "team-a-detector"
    ready["blocked_by"] = []
    assert readiness_for(ready)["can_open_exact_admission"]
    admitted = dict(packet)
    admitted["detector_admission"] = {
        "status": "controlled-slice-admitted",
        "scope": "controlled",
        "capabilities": ["cap"],
        "positive_gates": ["positive"],
        "hard_negative_gates": ["negative"],
        "remaining_real_pair_gap": "still open",
    }
    admitted_readiness = readiness_for(admitted)
    assert admitted_readiness["status"] == "detector-admitted-controlled"
    assert not admitted_readiness["can_open_exact_admission"]
    admitted_doc = {"schema_version": 1, "packet_count": 1, "packets": [admitted]}
    admitted_rows, admitted_summary = summarize_packets(validate_packet_doc(admitted_doc))
    assert admitted_summary["ready_packet_count"] == 0
    assert admitted_summary["detector_admitted_packet_count"] == 1
    assert admitted_rows[0]["readiness"]["status"] == "detector-admitted-controlled"
    assert readiness_group_for(admitted_rows[0]) == "admitted/resolved"
    assert admission_verdict(admitted_summary["ready_packet_count"]) == (
        "no-exact-admission-ready-packets"
    )
    ready_rows, _ready_summary = summarize_packets(
        validate_packet_doc({"schema_version": 1, "packet_count": 1, "packets": [ready]})
    )
    report = {
        "schema_version": 1,
        "tool_version": TOOL_VERSION,
        "identity": {"artifacts": {}},
        "verdict": "exact-admission-ready",
        "target_packets": {"summary": {}, "packets": ready_rows, "evidence_links": []},
    }
    readiness_summary = build_readiness_summary(report)
    assert readiness_summary["next_work"]["group"] == "ready-for-defender"
    assert readiness_summary["group_order"] == list(READINESS_GROUP_ORDER)
    assert [group["group"] for group in readiness_summary["group_list"]] == list(
        READINESS_GROUP_ORDER
    )
    assert readiness_summary["groups"]["ready-for-defender"]["count"] == 1
    blocked_rows, _blocked_summary = summarize_packets(
        validate_packet_doc({"schema_version": 1, "packet_count": 1, "packets": [packet]})
    )
    report["verdict"] = "no-exact-admission-ready-packets"
    report["target_packets"]["packets"] = blocked_rows
    readiness_summary = build_readiness_summary(report)
    assert readiness_summary["next_work"]["group"] == "blocked-on-proof"
    assert readiness_summary["groups"]["blocked-on-proof"]["count"] == 1
    try:
        bad = dict(packet)
        bad["hard_negative_siblings"] = []
        validate_packet_doc({"schema_version": 1, "packet_count": 1, "packets": [bad]})
        raise AssertionError("missing hard negative was not detected")
    except FrontierError:
        pass
    try:
        bad = json.loads(json.dumps(packet))
        bad["detector_admission"]["hard_negative_gates"] = []
        validate_hard_negative_linkage(
            validate_packet_doc({"schema_version": 1, "packet_count": 1, "packets": [bad]}),
            focused_cases,
        )
        raise AssertionError("missing focused hard-negative gate was not detected")
    except FrontierError:
        pass
    try:
        bad = json.loads(json.dumps(packet))
        bad["hard_negative_group_ids"] = ["unknown"]
        validate_hard_negative_linkage(
            validate_packet_doc({"schema_version": 1, "packet_count": 1, "packets": [bad]}),
            focused_cases,
        )
        raise AssertionError("unknown hard-negative group was not detected")
    except FrontierError:
        pass
    try:
        bad_cases = json.loads(json.dumps(focused_cases))
        bad_cases["hard_negative_groups"][0]["regression_gates"] = [
            "bench/type4/proof_carrying_frontier.py::missing_selftest_symbol",
            f"{HARD_NEGATIVE_CASE_REF_PREFIX}positive",
            f"{HARD_NEGATIVE_CASE_REF_PREFIX}negative",
        ]
        validate_hard_negative_linkage(packets, bad_cases)
        raise AssertionError("unknown file regression gate symbol was not detected")
    except FrontierError:
        pass
    no_model = dict(packet)
    no_model["proof_fact_model"] = {}
    try:
        validate_packet_doc({"schema_version": 1, "packet_count": 1, "packets": [no_model]})
        raise AssertionError("missing proof fact model was not detected")
    except FrontierError:
        pass
    malformed_admission = json.loads(json.dumps(packet))
    malformed_admission["detector_admission"]["positive_gates"] = "positive"
    try:
        validate_packet_doc(
            {"schema_version": 1, "packet_count": 1, "packets": [malformed_admission]}
        )
        raise AssertionError("scalar detector admission gate was not detected")
    except FrontierError:
        pass
    malformed_admission = json.loads(json.dumps(packet))
    malformed_admission["detector_admission"]["capabilities"] = [1]
    try:
        validate_packet_doc(
            {"schema_version": 1, "packet_count": 1, "packets": [malformed_admission]}
        )
        raise AssertionError("non-string detector admission item was not detected")
    except FrontierError:
        pass
    coevo = {
        "packets": [
            {
                "packet_id": "x",
                "series": 3,
                "surface": "exact-channel-gates",
                "mode": "blind-executable",
                "persona": "skeptic",
                "verdict": "green-confirmed",
            }
        ]
    }
    summary = summarize_coevo(coevo)
    assert summary["latest_series"] == 3
    assert summary["executable_or_measurement_packets"] == 1
    print("selftest OK")


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--target-packets", type=Path, default=DEFAULT_TARGET_PACKETS)
    parser.add_argument("--real-frontier", type=Path, default=DEFAULT_REAL_FRONTIER)
    parser.add_argument("--coevo-ledger", type=Path, default=DEFAULT_COEVO_LEDGER)
    parser.add_argument("--focused-cases", type=Path, default=DEFAULT_FOCUSED_CASES)
    parser.add_argument("--json-out", type=Path, default=DEFAULT_JSON_OUT)
    parser.add_argument("--markdown-out", type=Path, default=DEFAULT_MARKDOWN_OUT)
    parser.add_argument("--readiness-json-out", type=Path, default=DEFAULT_READINESS_JSON_OUT)
    parser.add_argument(
        "--readiness-markdown-out",
        type=Path,
        default=DEFAULT_READINESS_MARKDOWN_OUT,
    )
    parser.add_argument("--check", action="store_true", help="fail if committed artifacts are stale")
    parser.add_argument("--selftest", action="store_true", help="run corpus-free checks")
    args = parser.parse_args()

    try:
        if args.selftest:
            selftest()
            return 0
        report = build_report(
            args.target_packets,
            args.real_frontier,
            args.coevo_ledger,
            args.focused_cases,
        )
        if args.check:
            check_artifacts(
                report,
                args.json_out,
                args.markdown_out,
                args.readiness_json_out,
                args.readiness_markdown_out,
            )
        else:
            write_artifacts(
                report,
                args.json_out,
                args.markdown_out,
                args.readiness_json_out,
                args.readiness_markdown_out,
            )
    except FrontierError as exc:
        print(f"error: {exc}", file=sys.stderr)
        return 1
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
