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
DEFAULT_EXECUTABLE_EXPECTATIONS = HERE / "executable_expectations.v1.json"
DEFAULT_REAL_FRONTIER_REPLAY = HERE / "real_frontier_replay.v1.json"
DEFAULT_REAL_FRONTIER_REPLAY_STATUS = HERE / "real_frontier_replay_status.v1.json"
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
    "real_frontier_replay_ids",
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
EXECUTABLE_EXPECTATION_STATUS = {"same-family", "split"}
EXECUTABLE_REPORT_TOOL_VERSION = "type4-exec-check/1"
REAL_FRONTIER_REPLAY_TOOL_VERSION = "real-frontier-replay/1"
REAL_FRONTIER_REPLAY_STATUS = {"passed", "failed", "unavailable"}

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
        if not packet["real_frontier_replay_ids"]:
            raise FrontierError(f"packet {packet_id} must link at least one real-frontier replay")
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


def focused_executable_expectations(
    focused_cases: dict[str, Any],
) -> dict[str, dict[str, Any]]:
    expected: dict[str, dict[str, Any]] = {}
    for case in focused_cases.get("cases", []):
        case_id = case.get("id")
        for expectation in case.get("executable_expectations") or []:
            expectation_id = expectation.get("id")
            if not expectation_id:
                raise FrontierError(
                    f"focused case {case_id} has executable expectation without id"
                )
            if expectation_id in expected:
                raise FrontierError(f"duplicate executable expectation id: {expectation_id}")
            expect = expectation.get("expect")
            if expect not in EXECUTABLE_EXPECTATION_STATUS:
                raise FrontierError(
                    f"executable expectation {expectation_id} has unknown expect: {expect}"
                )
            fixture = expectation.get("fixture")
            if not isinstance(fixture, str) or not fixture:
                raise FrontierError(f"executable expectation {expectation_id} needs fixture")
            expected[expectation_id] = {
                "case_id": case_id,
                "expectation_id": expectation_id,
                "expect": expect,
                "fixture": fixture,
            }
    return expected


def validate_executable_report(
    report: dict[str, Any], focused_cases: dict[str, Any]
) -> dict[str, Any]:
    if report.get("schema_version") != SCHEMA_VERSION:
        raise FrontierError("executable expectations report schema_version must be 1")
    if report.get("tool_version") != EXECUTABLE_REPORT_TOOL_VERSION:
        raise FrontierError(
            "executable expectations report tool_version must be "
            f"{EXECUTABLE_REPORT_TOOL_VERSION}"
        )
    results = report.get("results")
    if not isinstance(results, list):
        raise FrontierError("executable expectations report must contain results list")
    expected_by_id = focused_executable_expectations(focused_cases)
    if report.get("expectation_count") != len(results):
        raise FrontierError("executable expectations report count does not match results")

    seen: set[str] = set()
    results_by_id: dict[str, dict[str, Any]] = {}
    stale_ids: list[str] = []
    failed_ids: list[str] = []
    extra_ids: list[str] = []
    for result in results:
        if not isinstance(result, dict):
            raise FrontierError("executable expectations report results must be objects")
        expectation_id = result.get("expectation_id")
        if not expectation_id:
            raise FrontierError("executable expectations report result missing expectation_id")
        if expectation_id in seen:
            raise FrontierError(f"duplicate executable result id: {expectation_id}")
        seen.add(expectation_id)
        results_by_id[expectation_id] = result
        expected = expected_by_id.get(expectation_id)
        if expected is None:
            extra_ids.append(expectation_id)
            continue
        for field in ("case_id", "expect", "fixture"):
            if result.get(field) != expected[field]:
                stale_ids.append(expectation_id)
                break
        observed = result.get("observed")
        if observed not in EXECUTABLE_EXPECTATION_STATUS:
            stale_ids.append(expectation_id)
            continue
        if bool(result.get("ok")) != (observed == result.get("expect")):
            stale_ids.append(expectation_id)
            continue
        if not result.get("ok"):
            failed_ids.append(expectation_id)

    missing_ids = sorted(set(expected_by_id) - set(results_by_id))
    passed_ids = sorted(
        expectation_id
        for expectation_id, result in results_by_id.items()
        if (
            expectation_id in expected_by_id
            and result.get("ok")
            and expectation_id not in stale_ids
        )
    )
    declared_by_case: dict[str, list[str]] = {}
    for expectation in expected_by_id.values():
        declared_by_case.setdefault(expectation["case_id"], []).append(
            expectation["expectation_id"]
        )
    for ids in declared_by_case.values():
        ids.sort()

    passed_count = sum(1 for result in results if result.get("ok"))
    failed_count = len(results) - passed_count
    if report.get("passed") != passed_count or report.get("failed") != failed_count:
        raise FrontierError("executable expectations report pass/fail counts are stale")

    return {
        "expected_by_id": expected_by_id,
        "results_by_id": results_by_id,
        "declared_by_case": declared_by_case,
        "missing_ids": sorted(missing_ids),
        "stale_ids": sorted(set(stale_ids)),
        "failed_ids": sorted(set(failed_ids)),
        "extra_ids": sorted(extra_ids),
        "passed_ids": passed_ids,
        "report_summary": {
            "nose_binary": report.get("nose_binary"),
            "declared_expectation_count": len(expected_by_id),
            "result_count": len(results),
            "query_count": report.get("query_count"),
            "passed": passed_count,
            "failed": failed_count,
            "missing": len(missing_ids),
            "stale": len(set(stale_ids)),
            "extra": len(extra_ids),
        },
    }


def executable_coverage_blockers(coverage: dict[str, Any] | None) -> list[str]:
    if coverage is None:
        return ["executable focused-case coverage was not evaluated for this packet"]
    status = coverage.get("coverage_status")
    if status == "covered":
        return []
    if status == "manifest-only":
        return ["no executable focused-case expectations are declared for this packet"]
    blockers = []
    if coverage.get("missing_expectation_ids"):
        blockers.append(
            "missing executable focused-case results: "
            + ", ".join(coverage["missing_expectation_ids"])
        )
    if coverage.get("stale_expectation_ids"):
        blockers.append(
            "stale executable focused-case results: "
            + ", ".join(coverage["stale_expectation_ids"])
        )
    if coverage.get("failed_expectation_ids"):
        blockers.append(
            "failing executable focused-case results: "
            + ", ".join(coverage["failed_expectation_ids"])
        )
    return blockers or [f"executable focused-case coverage is {status}"]


def summarize_executable_witness_coverage(
    packets: list[dict[str, Any]],
    hard_negative_linkage: list[dict[str, Any]],
    focused_cases: dict[str, Any],
    executable_report: dict[str, Any],
) -> dict[str, Any]:
    validated = validate_executable_report(executable_report, focused_cases)
    declared_by_case = validated["declared_by_case"]
    expected_by_id = validated["expected_by_id"]
    report_missing = set(validated["missing_ids"])
    report_stale = set(validated["stale_ids"])
    report_failed = set(validated["failed_ids"])
    report_passed = set(validated["passed_ids"])

    case_ids_by_packet: dict[str, set[str]] = {
        packet["packet_id"]: set() for packet in packets
    }
    positive_case_ids_by_packet: dict[str, set[str]] = {
        packet["packet_id"]: set() for packet in packets
    }
    hard_negative_case_ids_by_packet: dict[str, set[str]] = {
        packet["packet_id"]: set() for packet in packets
    }
    for row in hard_negative_linkage:
        packet_id = row["packet_id"]
        positive = set(row["positive_cases"])
        hard_negative = set(row["hard_negative_cases"])
        positive_case_ids_by_packet[packet_id].update(positive)
        hard_negative_case_ids_by_packet[packet_id].update(hard_negative)
        case_ids_by_packet[packet_id].update(positive | hard_negative)

    by_packet: dict[str, dict[str, Any]] = {}
    by_status: Counter[str] = Counter()
    for packet in packets:
        packet_id = packet["packet_id"]
        case_ids = sorted(case_ids_by_packet.get(packet_id, set()))
        declared_ids = sorted(
            expectation_id
            for case_id in case_ids
            for expectation_id in declared_by_case.get(case_id, [])
        )
        missing_ids = sorted(set(declared_ids) & report_missing)
        stale_ids = sorted(set(declared_ids) & report_stale)
        failed_ids = sorted(set(declared_ids) & report_failed)
        passed_ids = sorted(set(declared_ids) & report_passed)
        manifest_only_case_ids = sorted(
            case_id for case_id in case_ids if not declared_by_case.get(case_id)
        )
        if not declared_ids:
            coverage_status = "manifest-only"
        elif missing_ids or stale_ids or failed_ids:
            coverage_status = "blocked"
        else:
            coverage_status = "covered"
        by_status[coverage_status] += 1
        by_expectation = [
            {
                "expectation_id": expectation_id,
                "case_id": expected_by_id[expectation_id]["case_id"],
                "expect": expected_by_id[expectation_id]["expect"],
                "status": (
                    "missing"
                    if expectation_id in missing_ids
                    else "stale"
                    if expectation_id in stale_ids
                    else "failed"
                    if expectation_id in failed_ids
                    else "passed"
                ),
            }
            for expectation_id in declared_ids
        ]
        by_packet[packet_id] = {
            "packet_id": packet_id,
            "coverage_status": coverage_status,
            "manifest_case_ids": case_ids,
            "manifest_positive_case_ids": sorted(positive_case_ids_by_packet[packet_id]),
            "manifest_hard_negative_case_ids": sorted(
                hard_negative_case_ids_by_packet[packet_id]
            ),
            "manifest_only_case_ids": manifest_only_case_ids,
            "declared_expectation_ids": declared_ids,
            "passed_expectation_ids": passed_ids,
            "missing_expectation_ids": missing_ids,
            "stale_expectation_ids": stale_ids,
            "failed_expectation_ids": failed_ids,
            "declared_expectation_count": len(declared_ids),
            "passed_expectation_count": len(passed_ids),
            "missing_expectation_count": len(missing_ids),
            "stale_expectation_count": len(stale_ids),
            "failed_expectation_count": len(failed_ids),
            "by_expectation": by_expectation,
        }

    return {
        "summary": {
            **validated["report_summary"],
            "packet_count": len(packets),
            "packet_count_by_coverage": dict(sorted(by_status.items())),
            "fully_covered_packet_count": by_status.get("covered", 0),
            "report_missing_expectation_ids": validated["missing_ids"],
            "report_stale_expectation_ids": validated["stale_ids"],
            "report_failed_expectation_ids": validated["failed_ids"],
            "report_extra_expectation_ids": validated["extra_ids"],
        },
        "packets": [by_packet[packet["packet_id"]] for packet in packets],
        "by_packet": by_packet,
    }


def replay_entries_by_id(manifest: dict[str, Any]) -> dict[str, dict[str, Any]]:
    if manifest.get("schema_version") != SCHEMA_VERSION:
        raise FrontierError("real frontier replay manifest schema_version must be 1")
    if manifest.get("tool_version") != REAL_FRONTIER_REPLAY_TOOL_VERSION:
        raise FrontierError(
            "real frontier replay manifest tool_version must be "
            f"{REAL_FRONTIER_REPLAY_TOOL_VERSION}"
        )
    entries = manifest.get("entries")
    if not isinstance(entries, list):
        raise FrontierError("real frontier replay manifest must contain entries")
    by_id: dict[str, dict[str, Any]] = {}
    for entry in entries:
        if not isinstance(entry, dict):
            raise FrontierError("real frontier replay entries must be objects")
        replay_id = entry.get("replay_id")
        if not replay_id:
            raise FrontierError("real frontier replay entry missing replay_id")
        if replay_id in by_id:
            raise FrontierError(f"duplicate real frontier replay id: {replay_id}")
        if entry.get("expect") not in EXECUTABLE_EXPECTATION_STATUS:
            raise FrontierError(f"real frontier replay {replay_id} has unknown expect")
        if not isinstance(entry.get("sources"), list) or not entry["sources"]:
            raise FrontierError(f"real frontier replay {replay_id} must list sources")
        if not isinstance(entry.get("members"), list) or len(entry["members"]) < 2:
            raise FrontierError(f"real frontier replay {replay_id} must list members")
        by_id[replay_id] = entry
    return by_id


def validate_real_frontier_replay_manifest(
    packets: list[dict[str, Any]],
    real_frontier: dict[str, Any],
    manifest: dict[str, Any],
) -> dict[str, dict[str, Any]]:
    entries = replay_entries_by_id(manifest)
    cases = by_case_id(real_frontier)
    packet_by_id = {packet["packet_id"]: packet for packet in packets}
    for packet in packets:
        replay_ids = packet.get("real_frontier_replay_ids")
        if not isinstance(replay_ids, list) or not replay_ids:
            raise FrontierError(
                f"packet {packet['packet_id']} must cite real_frontier_replay_ids"
            )
        for replay_id in replay_ids:
            if replay_id not in entries:
                raise FrontierError(
                    f"packet {packet['packet_id']} references unknown replay {replay_id}"
                )
    for replay_id, entry in entries.items():
        packet_id = entry.get("packet_id")
        case_id = entry.get("case_id")
        packet = packet_by_id.get(packet_id)
        if packet is None:
            raise FrontierError(f"real frontier replay {replay_id} references unknown packet")
        if replay_id not in packet["real_frontier_replay_ids"]:
            raise FrontierError(
                f"real frontier replay {replay_id} is not linked from packet {packet_id}"
            )
        if case_id not in packet["evidence_case_ids"]:
            raise FrontierError(
                f"real frontier replay {replay_id} case {case_id} is not linked evidence"
            )
        case = cases.get(case_id)
        if case is None:
            raise FrontierError(f"real frontier replay {replay_id} references unknown case")
        if case.get("status") != "real-miss":
            raise FrontierError(f"real frontier replay {replay_id} must link a real-miss")
    return entries


def validate_real_frontier_replay_status(
    status_report: dict[str, Any],
    replay_entries: dict[str, dict[str, Any]],
    expected_artifacts: dict[str, Any],
) -> dict[str, Any]:
    if status_report.get("schema_version") != SCHEMA_VERSION:
        raise FrontierError("real frontier replay status schema_version must be 1")
    if status_report.get("tool_version") != REAL_FRONTIER_REPLAY_TOOL_VERSION:
        raise FrontierError(
            "real frontier replay status tool_version must be "
            f"{REAL_FRONTIER_REPLAY_TOOL_VERSION}"
        )
    if status_report.get("input_artifacts") != expected_artifacts:
        raise FrontierError("real frontier replay status input_artifacts are stale")
    results = status_report.get("results")
    if not isinstance(results, list):
        raise FrontierError("real frontier replay status must contain results")
    seen: set[str] = set()
    by_id: dict[str, dict[str, Any]] = {}
    failed_ids: list[str] = []
    unavailable_ids: list[str] = []
    for result in results:
        if not isinstance(result, dict):
            raise FrontierError("real frontier replay status results must be objects")
        replay_id = result.get("replay_id")
        if not replay_id:
            raise FrontierError("real frontier replay status result missing replay_id")
        if replay_id in seen:
            raise FrontierError(f"duplicate real frontier replay status: {replay_id}")
        seen.add(replay_id)
        entry = replay_entries.get(replay_id)
        if entry is None:
            raise FrontierError(f"extra real frontier replay status: {replay_id}")
        for field in ("packet_id", "case_id", "expect"):
            if result.get(field) != entry.get(field):
                raise FrontierError(
                    f"real frontier replay status {replay_id} field {field} is stale"
                )
        status = result.get("status")
        if status not in REAL_FRONTIER_REPLAY_STATUS:
            raise FrontierError(
                f"real frontier replay status {replay_id} has unknown status {status}"
            )
        observed = result.get("observed")
        if status == "unavailable":
            unavailable_ids.append(replay_id)
            if observed is not None or result.get("ok") is not None:
                raise FrontierError(
                    f"unavailable real frontier replay {replay_id} must not carry ok/observed"
                )
        else:
            if observed not in EXECUTABLE_EXPECTATION_STATUS:
                raise FrontierError(
                    f"real frontier replay status {replay_id} has unknown observed"
                )
            if status == "passed" and observed != result["expect"]:
                raise FrontierError(
                    f"real frontier replay status {replay_id} passed with wrong observed"
                )
            if status == "failed" and observed == result["expect"]:
                raise FrontierError(
                    f"real frontier replay status {replay_id} failed with matching observed"
                )
            if bool(result.get("ok")) != (status == "passed"):
                raise FrontierError(f"real frontier replay status {replay_id} ok flag drifted")
            if status == "failed":
                failed_ids.append(replay_id)
        by_id[replay_id] = result
    missing_ids = sorted(set(replay_entries) - set(by_id))
    if missing_ids:
        raise FrontierError(f"missing real frontier replay status: {missing_ids}")
    counts = Counter(result["status"] for result in results)
    if status_report.get("entry_count") != len(results):
        raise FrontierError("real frontier replay status entry_count is stale")
    if status_report.get("passed") != counts.get("passed", 0):
        raise FrontierError("real frontier replay status passed count is stale")
    if status_report.get("failed") != counts.get("failed", 0):
        raise FrontierError("real frontier replay status failed count is stale")
    if status_report.get("unavailable") != counts.get("unavailable", 0):
        raise FrontierError("real frontier replay status unavailable count is stale")
    return {
        "results_by_id": by_id,
        "missing_ids": missing_ids,
        "failed_ids": sorted(failed_ids),
        "unavailable_ids": sorted(unavailable_ids),
        "summary": {
            "nose_binary": status_report.get("nose_binary"),
            "declared_replay_count": len(replay_entries),
            "result_count": len(results),
            "query_count": status_report.get("query_count"),
            "passed": counts.get("passed", 0),
            "failed": counts.get("failed", 0),
            "unavailable": counts.get("unavailable", 0),
            "report_missing_replay_ids": missing_ids,
            "report_failed_replay_ids": sorted(failed_ids),
            "report_unavailable_replay_ids": sorted(unavailable_ids),
        },
    }


def real_frontier_replay_blockers(replay_coverage: dict[str, Any] | None) -> list[str]:
    if replay_coverage is None:
        return ["real-frontier replay coverage was not evaluated for this packet"]
    status = replay_coverage.get("coverage_status")
    if status == "passed":
        return []
    blockers = []
    if replay_coverage.get("missing_replay_ids"):
        blockers.append(
            "missing real-frontier replay results: "
            + ", ".join(replay_coverage["missing_replay_ids"])
        )
    if replay_coverage.get("failed_replay_ids"):
        blockers.append(
            "failing real-frontier replay results: "
            + ", ".join(replay_coverage["failed_replay_ids"])
        )
    if replay_coverage.get("unavailable_replay_ids"):
        blockers.append(
            "unavailable real-frontier replay results: "
            + ", ".join(replay_coverage["unavailable_replay_ids"])
        )
    return blockers or [f"real-frontier replay coverage is {status}"]


def summarize_real_frontier_replay(
    packets: list[dict[str, Any]],
    replay_manifest: dict[str, Any],
    replay_status_report: dict[str, Any],
    real_frontier: dict[str, Any],
    expected_artifacts: dict[str, Any],
) -> dict[str, Any]:
    replay_entries = validate_real_frontier_replay_manifest(
        packets,
        real_frontier,
        replay_manifest,
    )
    validated = validate_real_frontier_replay_status(
        replay_status_report,
        replay_entries,
        expected_artifacts,
    )
    results_by_id = validated["results_by_id"]
    report_failed = set(validated["failed_ids"])
    report_unavailable = set(validated["unavailable_ids"])
    by_packet: dict[str, dict[str, Any]] = {}
    by_status: Counter[str] = Counter()
    for packet in packets:
        packet_id = packet["packet_id"]
        replay_ids = sorted(packet["real_frontier_replay_ids"])
        missing_ids = sorted(replay_id for replay_id in replay_ids if replay_id not in results_by_id)
        failed_ids = sorted(set(replay_ids) & report_failed)
        unavailable_ids = sorted(set(replay_ids) & report_unavailable)
        passed_ids = sorted(
            replay_id
            for replay_id in replay_ids
            if results_by_id.get(replay_id, {}).get("status") == "passed"
        )
        if missing_ids or failed_ids:
            coverage_status = "blocked"
        elif unavailable_ids:
            coverage_status = "unavailable"
        else:
            coverage_status = "passed"
        by_status[coverage_status] += 1
        by_replay = []
        for replay_id in replay_ids:
            entry = replay_entries[replay_id]
            result = results_by_id.get(replay_id)
            by_replay.append(
                {
                    "replay_id": replay_id,
                    "case_id": entry["case_id"],
                    "expect": entry["expect"],
                    "status": result.get("status") if result else "missing",
                    "observed": result.get("observed") if result else None,
                    "proof_gap": entry.get("proof_gap", []),
                }
            )
        by_packet[packet_id] = {
            "packet_id": packet_id,
            "coverage_status": coverage_status,
            "declared_replay_ids": replay_ids,
            "passed_replay_ids": passed_ids,
            "missing_replay_ids": missing_ids,
            "failed_replay_ids": failed_ids,
            "unavailable_replay_ids": unavailable_ids,
            "declared_replay_count": len(replay_ids),
            "passed_replay_count": len(passed_ids),
            "missing_replay_count": len(missing_ids),
            "failed_replay_count": len(failed_ids),
            "unavailable_replay_count": len(unavailable_ids),
            "by_replay": by_replay,
        }
    return {
        "summary": {
            **validated["summary"],
            "packet_count": len(packets),
            "packet_count_by_coverage": dict(sorted(by_status.items())),
            "fully_replayed_packet_count": by_status.get("passed", 0),
        },
        "packets": [by_packet[packet["packet_id"]] for packet in packets],
        "by_packet": by_packet,
    }


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


def readiness_for(
    packet: dict[str, Any],
    executable_coverage: dict[str, Any] | None = None,
    real_frontier_replay: dict[str, Any] | None = None,
) -> dict[str, Any]:
    blockers = list(packet.get("blocked_by") or [])
    coverage_blockers = executable_coverage_blockers(executable_coverage)
    replay_blockers = real_frontier_replay_blockers(real_frontier_replay)
    blockers.extend(coverage_blockers)
    blockers.extend(replay_blockers)
    admission = packet.get("detector_admission") or {}
    if coverage_blockers:
        return {
            "status": "blocked",
            "can_open_exact_admission": False,
            "reason": "packet lacks fresh executable focused-case witness coverage",
            "blocking_items": blockers,
        }
    if replay_blockers:
        return {
            "status": "blocked",
            "can_open_exact_admission": False,
            "reason": "packet lacks passing real-frontier replay coverage",
            "blocking_items": blockers,
        }
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


def summarize_packets(
    packets: list[dict[str, Any]],
    executable_coverage_by_packet: dict[str, dict[str, Any]] | None = None,
    real_frontier_replay_by_packet: dict[str, dict[str, Any]] | None = None,
) -> tuple[list[dict[str, Any]], dict[str, Any]]:
    rows = []
    by_status: Counter[str] = Counter()
    by_route: Counter[str] = Counter()
    by_detector_admission: Counter[str] = Counter()
    by_executable_coverage: Counter[str] = Counter()
    by_real_frontier_replay: Counter[str] = Counter()
    executable_coverage_by_packet = executable_coverage_by_packet or {}
    real_frontier_replay_by_packet = real_frontier_replay_by_packet or {}
    for packet in packets:
        executable_coverage = executable_coverage_by_packet.get(packet["packet_id"])
        real_frontier_replay = real_frontier_replay_by_packet.get(packet["packet_id"])
        readiness = readiness_for(packet, executable_coverage, real_frontier_replay)
        by_status[readiness["status"]] += 1
        by_route[packet["owner_route"]] += 1
        admission = packet["detector_admission"]
        by_detector_admission[admission["status"]] += 1
        if executable_coverage:
            by_executable_coverage[executable_coverage["coverage_status"]] += 1
        if real_frontier_replay:
            by_real_frontier_replay[real_frontier_replay["coverage_status"]] += 1
        rows.append(
            {
                "packet_id": packet["packet_id"],
                "candidate_axis": packet["candidate_axis"],
                "semantic_claim": packet["semantic_claim"],
                "owner_route": packet["owner_route"],
                "owner_issue": packet["owner_issue"],
                "evidence_case_ids": packet["evidence_case_ids"],
                "real_frontier_replay_ids": packet["real_frontier_replay_ids"],
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
                "executable_witness_coverage": executable_coverage,
                "real_frontier_replay": real_frontier_replay,
                "readiness": readiness,
            }
        )
    summary = {
        "packet_count": len(packets),
        "by_readiness": dict(sorted(by_status.items())),
        "by_owner_route": dict(sorted(by_route.items())),
        "by_detector_admission": dict(sorted(by_detector_admission.items())),
        "by_executable_witness_coverage": dict(sorted(by_executable_coverage.items())),
        "by_real_frontier_replay": dict(sorted(by_real_frontier_replay.items())),
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
    executable_expectations_path: Path,
    real_frontier_replay_path: Path,
    real_frontier_replay_status_path: Path,
) -> dict[str, Any]:
    packet_doc = load_json(target_packets_path)
    real_frontier = load_json(real_frontier_path)
    coevo = load_json(coevo_ledger_path)
    focused_cases = load_json(focused_cases_path)
    executable_report = load_json(executable_expectations_path)
    replay_manifest = load_json(real_frontier_replay_path)
    replay_status_report = load_json(real_frontier_replay_status_path)
    packets = validate_packet_doc(packet_doc)
    evidence_links = validate_evidence_links(packets, real_frontier)
    hard_negative_linkage = validate_hard_negative_linkage(packets, focused_cases)
    executable_coverage = summarize_executable_witness_coverage(
        packets,
        hard_negative_linkage,
        focused_cases,
        executable_report,
    )
    replay_coverage = summarize_real_frontier_replay(
        packets,
        replay_manifest,
        replay_status_report,
        real_frontier,
        {
            "manifest": artifact_ref(real_frontier_replay_path),
            "target_packets": artifact_ref(target_packets_path),
            "real_frontier": artifact_ref(real_frontier_path),
        },
    )
    packet_rows, packet_summary = summarize_packets(
        packets,
        executable_coverage["by_packet"],
        replay_coverage["by_packet"],
    )
    public_executable_coverage = {
        "summary": executable_coverage["summary"],
        "packets": executable_coverage["packets"],
    }
    public_replay_coverage = {
        "summary": replay_coverage["summary"],
        "packets": replay_coverage["packets"],
    }
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
                "executable_expectations": artifact_ref(executable_expectations_path),
                "real_frontier_replay": artifact_ref(real_frontier_replay_path),
                "real_frontier_replay_status": artifact_ref(
                    real_frontier_replay_status_path
                ),
            },
            "target_packet_identity": packet_doc.get("identity", {}),
        },
        "admission_policy": {
            "exact_admission_requires": [
                "linked real_frontier real-miss evidence",
                "proof invariant narrow enough to defend",
                "adjacent hard-negative siblings",
                "packet-level hard-negative group linkage",
                "fresh executable focused-case witness coverage for declared expectations",
                "passing real-frontier replay status for linked real-corpus evidence",
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
            "executable_witness_coverage": public_executable_coverage,
            "real_frontier_replay": public_replay_coverage,
        },
        "coevolution_guardrails": coevo_summary,
    }


def markdown_report(report: dict[str, Any]) -> str:
    target = report["target_packets"]["summary"]
    coevo = report["coevolution_guardrails"]
    by_readiness = json.dumps(target["by_readiness"], sort_keys=True)
    by_owner_route = json.dumps(target["by_owner_route"], sort_keys=True)
    by_detector_admission = json.dumps(target["by_detector_admission"], sort_keys=True)
    by_executable_coverage = json.dumps(
        target["by_executable_witness_coverage"], sort_keys=True
    )
    by_real_frontier_replay = json.dumps(target["by_real_frontier_replay"], sort_keys=True)
    by_verdict = json.dumps(coevo["by_verdict"], sort_keys=True)
    executable_summary = report["target_packets"]["executable_witness_coverage"]["summary"]
    replay_summary = report["target_packets"]["real_frontier_replay"]["summary"]
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
        f"- by executable witness coverage: `{by_executable_coverage}`",
        f"- by real-frontier replay: `{by_real_frontier_replay}`",
        (
            "- executable expectations: "
            f"{executable_summary['passed']}/{executable_summary['declared_expectation_count']} "
            "passed"
        ),
        (
            "- real-frontier replays: "
            f"{replay_summary['passed']}/{replay_summary['declared_replay_count']} "
            f"passed; {replay_summary['unavailable']} unavailable"
        ),
        "",
        "## Admission Policy",
        "",
    ]
    lines.extend(f"- {item}" for item in report["admission_policy"]["exact_admission_requires"])
    lines += [
        "",
        "## Target Packets",
        "",
        "| packet | axis | route | readiness | exec witnesses | real replay | proof facts | hard negatives | groups |",
        "|---|---|---|---|---|---|---:|---:|---:|",
    ]
    for packet in report["target_packets"]["packets"]:
        readiness = packet["readiness"]
        coverage = packet["executable_witness_coverage"]
        exec_cell = (
            f"{coverage['coverage_status']} "
            f"({coverage['passed_expectation_count']}/"
            f"{coverage['declared_expectation_count']})"
        )
        replay = packet["real_frontier_replay"]
        replay_cell = (
            f"{replay['coverage_status']} "
            f"({replay['passed_replay_count']}/"
            f"{replay['declared_replay_count']})"
        )
        lines.append(
            f"| `{packet['packet_id']}` | `{packet['candidate_axis']}` | "
            f"`{packet['owner_route']}` | `{readiness['status']}` | "
            f"`{exec_cell}` | "
            f"`{replay_cell}` | "
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
        coverage = packet["executable_witness_coverage"]
        lines.append(
            "- executable witness coverage: "
            f"`{coverage['coverage_status']}` "
            f"({coverage['passed_expectation_count']}/"
            f"{coverage['declared_expectation_count']} passed)"
        )
        replay = packet["real_frontier_replay"]
        lines.append(
            "- real-frontier replay: "
            f"`{replay['coverage_status']}` "
            f"({replay['passed_replay_count']}/"
            f"{replay['declared_replay_count']} passed)"
        )
        if replay["unavailable_replay_ids"]:
            replay_ids = ", ".join(f"`{replay_id}`" for replay_id in replay["unavailable_replay_ids"])
            lines.append(f"- unavailable real-frontier replays: {replay_ids}")
        if coverage["manifest_only_case_ids"]:
            cases = ", ".join(f"`{case_id}`" for case_id in coverage["manifest_only_case_ids"])
            lines.append(f"- manifest-only cases: {cases}")
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
    coverage = packet.get("executable_witness_coverage") or {}
    replay = packet.get("real_frontier_replay") or {}
    if executable_coverage_blockers(coverage):
        return (
            f"{packet_id}: exact admission for {axis} remains closed because executable "
            f"focused-case witness coverage is {coverage.get('coverage_status', 'missing')}."
        )
    if real_frontier_replay_blockers(replay):
        return (
            f"{packet_id}: exact admission for {axis} remains closed because "
            f"real-frontier replay coverage is {replay.get('coverage_status', 'missing')}."
        )
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
    coverage = packet.get("executable_witness_coverage") or {}
    replay = packet.get("real_frontier_replay") or {}
    if executable_coverage_blockers(coverage):
        return (
            "Refresh executable focused-case witness coverage before changing detector "
            "admission status."
        )
    if real_frontier_replay_blockers(replay):
        return (
            "Refresh or make available real-frontier replay coverage before changing "
            "detector admission status."
        )
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
        "executable_witness_coverage": packet["executable_witness_coverage"],
        "real_frontier_replay": packet["real_frontier_replay"],
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
            "fresh_executable_witness_coverage_required_for_ready_rows": True,
            "passing_real_frontier_replay_required_for_ready_rows": True,
        },
        "executable_witness_coverage": report["target_packets"][
            "executable_witness_coverage"
        ]["summary"],
        "real_frontier_replay": report["target_packets"]["real_frontier_replay"][
            "summary"
        ],
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
    coverage = summary["executable_witness_coverage"]
    replay = summary["real_frontier_replay"]
    by_coverage = json.dumps(coverage["packet_count_by_coverage"], sort_keys=True)
    by_replay = json.dumps(replay["packet_count_by_coverage"], sort_keys=True)
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
        f"- executable witness coverage: `{by_coverage}`",
        f"- real-frontier replay: `{by_replay}`",
        (
            "- executable expectations: "
            f"{coverage['passed']}/{coverage['declared_expectation_count']} passed"
        ),
        (
            "- real-frontier replays: "
            f"{replay['passed']}/{replay['declared_replay_count']} passed; "
            f"{replay['unavailable']} unavailable"
        ),
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
            "| packet | axis | readiness | exec coverage | real replay | detector admission | hard-negative groups | action |",
            "|---|---|---|---|---|---|---|---|",
        ]
        for packet in data["packets"]:
            groups = ", ".join(
                f"`{markdown_cell(group)}`" for group in packet["hard_negative_group_ids"]
            )
            exec_coverage = packet["executable_witness_coverage"]
            exec_cell = (
                f"{exec_coverage['coverage_status']} "
                f"({exec_coverage['passed_expectation_count']}/"
                f"{exec_coverage['declared_expectation_count']})"
            )
            replay_coverage = packet["real_frontier_replay"]
            replay_cell = (
                f"{replay_coverage['coverage_status']} "
                f"({replay_coverage['passed_replay_count']}/"
                f"{replay_coverage['declared_replay_count']})"
            )
            lines.append(
                f"| `{markdown_cell(packet['packet_id'])}` | "
                f"`{markdown_cell(packet['candidate_axis'])}` | "
                f"`{markdown_cell(packet['readiness_status'])}` | "
                f"`{markdown_cell(exec_cell)}` | "
                f"`{markdown_cell(replay_cell)}` | "
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
        "real_frontier_replay_ids": ["r"],
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
                "executable_expectations": [
                    {
                        "id": "positive_exec",
                        "fixture": "bench/type4/adversarial/cases/selftest.py",
                        "expect": "same-family",
                    }
                ],
            },
            {
                "id": "negative",
                "kind": "hard-negative",
                "semantic_family": "axis.family",
                "claim": "negative",
                "executable_expectations": [
                    {
                        "id": "negative_exec",
                        "fixture": "bench/type4/adversarial/cases/selftest.py",
                        "expect": "split",
                    }
                ],
            },
        ],
    }
    real_frontier = {
        "schema_version": 1,
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
    executable_report = {
        "schema_version": 1,
        "tool_version": EXECUTABLE_REPORT_TOOL_VERSION,
        "nose_binary": "nose",
        "expectation_count": 2,
        "query_count": 1,
        "passed": 2,
        "failed": 0,
        "results": [
            {
                "case_id": "positive",
                "expectation_id": "positive_exec",
                "fixture": "bench/type4/adversarial/cases/selftest.py",
                "expect": "same-family",
                "observed": "same-family",
                "ok": True,
            },
            {
                "case_id": "negative",
                "expectation_id": "negative_exec",
                "fixture": "bench/type4/adversarial/cases/selftest.py",
                "expect": "split",
                "observed": "split",
                "ok": True,
            },
        ],
    }
    executable_coverage = summarize_executable_witness_coverage(
        packets,
        linkage,
        focused_cases,
        executable_report,
    )
    replay_manifest = {
        "schema_version": 1,
        "tool_version": REAL_FRONTIER_REPLAY_TOOL_VERSION,
        "entries": [
            {
                "replay_id": "r",
                "packet_id": "p",
                "case_id": "c",
                "expect": "split",
                "sources": [{"kind": "workspace", "path": "bench/type4/selftest.py"}],
                "members": [
                    {"kind": "workspace", "file": "a.py"},
                    {"kind": "workspace", "file": "b.py"},
                ],
                "query": {"mode": "semantic", "min_size": 1, "min_lines": 1},
                "proof_gap": ["missing reusable proof"],
            }
        ],
    }
    replay_artifacts = {"manifest": {}, "target_packets": {}, "real_frontier": {}}
    replay_status = {
        "schema_version": 1,
        "tool_version": REAL_FRONTIER_REPLAY_TOOL_VERSION,
        "input_artifacts": replay_artifacts,
        "nose_binary": "nose",
        "entry_count": 1,
        "query_count": 1,
        "passed": 1,
        "failed": 0,
        "unavailable": 0,
        "results": [
            {
                "replay_id": "r",
                "packet_id": "p",
                "case_id": "c",
                "expect": "split",
                "status": "passed",
                "observed": "split",
                "ok": True,
            }
        ],
    }
    replay_coverage = summarize_real_frontier_replay(
        packets,
        replay_manifest,
        replay_status,
        real_frontier,
        replay_artifacts,
    )
    assert executable_coverage["by_packet"]["p"]["coverage_status"] == "covered"
    assert replay_coverage["by_packet"]["p"]["coverage_status"] == "passed"
    assert readiness_for(
        packet,
        executable_coverage["by_packet"]["p"],
        replay_coverage["by_packet"]["p"],
    )["status"] == (
        "blocked-on-proof"
    )
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
    assert readiness_for(
        ready,
        executable_coverage["by_packet"]["p"],
        replay_coverage["by_packet"]["p"],
    )[
        "can_open_exact_admission"
    ]
    missing_executable_report = json.loads(json.dumps(executable_report))
    missing_executable_report["results"] = missing_executable_report["results"][:1]
    missing_executable_report["expectation_count"] = 1
    missing_executable_report["passed"] = 1
    missing_coverage = summarize_executable_witness_coverage(
        packets,
        linkage,
        focused_cases,
        missing_executable_report,
    )
    assert missing_coverage["by_packet"]["p"]["coverage_status"] == "blocked"
    assert not readiness_for(
        ready,
        missing_coverage["by_packet"]["p"],
        replay_coverage["by_packet"]["p"],
    )[
        "can_open_exact_admission"
    ]
    unavailable_replay_status = json.loads(json.dumps(replay_status))
    unavailable_replay_status["passed"] = 0
    unavailable_replay_status["unavailable"] = 1
    unavailable_replay_status["results"][0]["status"] = "unavailable"
    unavailable_replay_status["results"][0]["observed"] = None
    unavailable_replay_status["results"][0]["ok"] = None
    unavailable_replay_coverage = summarize_real_frontier_replay(
        packets,
        replay_manifest,
        unavailable_replay_status,
        real_frontier,
        replay_artifacts,
    )
    assert unavailable_replay_coverage["by_packet"]["p"]["coverage_status"] == "unavailable"
    assert not readiness_for(
        ready,
        executable_coverage["by_packet"]["p"],
        unavailable_replay_coverage["by_packet"]["p"],
    )["can_open_exact_admission"]
    admitted = dict(packet)
    admitted["detector_admission"] = {
        "status": "controlled-slice-admitted",
        "scope": "controlled",
        "capabilities": ["cap"],
        "positive_gates": ["positive"],
        "hard_negative_gates": ["negative"],
        "remaining_real_pair_gap": "still open",
    }
    admitted_readiness = readiness_for(
        admitted,
        executable_coverage["by_packet"]["p"],
        replay_coverage["by_packet"]["p"],
    )
    assert admitted_readiness["status"] == "detector-admitted-controlled"
    assert not admitted_readiness["can_open_exact_admission"]
    admitted_doc = {"schema_version": 1, "packet_count": 1, "packets": [admitted]}
    admitted_rows, admitted_summary = summarize_packets(
        validate_packet_doc(admitted_doc),
        executable_coverage["by_packet"],
        replay_coverage["by_packet"],
    )
    assert admitted_summary["ready_packet_count"] == 0
    assert admitted_summary["detector_admitted_packet_count"] == 1
    assert admitted_rows[0]["readiness"]["status"] == "detector-admitted-controlled"
    assert readiness_group_for(admitted_rows[0]) == "admitted/resolved"
    assert admission_verdict(admitted_summary["ready_packet_count"]) == (
        "no-exact-admission-ready-packets"
    )
    ready_rows, _ready_summary = summarize_packets(
        validate_packet_doc({"schema_version": 1, "packet_count": 1, "packets": [ready]}),
        executable_coverage["by_packet"],
        replay_coverage["by_packet"],
    )
    report = {
        "schema_version": 1,
        "tool_version": TOOL_VERSION,
        "identity": {"artifacts": {}},
        "verdict": "exact-admission-ready",
        "target_packets": {
            "summary": {},
            "packets": ready_rows,
            "evidence_links": [],
            "executable_witness_coverage": executable_coverage,
            "real_frontier_replay": replay_coverage,
        },
    }
    readiness_summary = build_readiness_summary(report)
    assert readiness_summary["next_work"]["group"] == "ready-for-defender"
    assert readiness_summary["group_order"] == list(READINESS_GROUP_ORDER)
    assert [group["group"] for group in readiness_summary["group_list"]] == list(
        READINESS_GROUP_ORDER
    )
    assert readiness_summary["groups"]["ready-for-defender"]["count"] == 1
    blocked_rows, _blocked_summary = summarize_packets(
        validate_packet_doc({"schema_version": 1, "packet_count": 1, "packets": [packet]}),
        executable_coverage["by_packet"],
        replay_coverage["by_packet"],
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
    parser.add_argument(
        "--executable-expectations",
        type=Path,
        default=DEFAULT_EXECUTABLE_EXPECTATIONS,
    )
    parser.add_argument(
        "--real-frontier-replay",
        type=Path,
        default=DEFAULT_REAL_FRONTIER_REPLAY,
    )
    parser.add_argument(
        "--real-frontier-replay-status",
        type=Path,
        default=DEFAULT_REAL_FRONTIER_REPLAY_STATUS,
    )
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
            args.executable_expectations,
            args.real_frontier_replay,
            args.real_frontier_replay_status,
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
