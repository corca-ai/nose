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
DEFAULT_JSON_OUT = HERE / "proof_carrying_frontier.v1.json"
DEFAULT_MARKDOWN_OUT = HERE / "proof_carrying_frontier.md"

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
    "breadth",
    "evidence_tier",
    "curated",
    "why_now",
    "proof_fact_model",
    "blocked_by",
    "notes",
}

REQUIRED_DETECTOR_FIELDS = {
    "baseline_command",
    "baseline_result",
    "current_detector_miss",
}

OWNER_ROUTE = {"proof-fact-prerequisite", "team-a-detector", "team-c-product"}
FRONTIER_STATUSES = {"real-miss", "already-covered", "hard-negative", "unsupported", "closed"}
COEVO_VERDICTS = {
    "violation-fixed",
    "refuted",
    "recorded-low-prevalence",
    "deferred-issue",
    "green-confirmed",
}


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


def readiness_for(packet: dict[str, Any]) -> dict[str, Any]:
    blockers = list(packet.get("blocked_by") or [])
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
    for packet in packets:
        readiness = readiness_for(packet)
        by_status[readiness["status"]] += 1
        by_route[packet["owner_route"]] += 1
        rows.append(
            {
                "packet_id": packet["packet_id"],
                "candidate_axis": packet["candidate_axis"],
                "owner_route": packet["owner_route"],
                "evidence_case_ids": packet["evidence_case_ids"],
                "evidence_tier": packet["evidence_tier"],
                "proof_invariant": packet["proof_invariant"],
                "hard_negative_count": len(packet["hard_negative_siblings"]),
                "proof_fact_model": {
                    "model_status": packet["proof_fact_model"].get("model_status", "unknown"),
                    "fact_ids": [
                        fact["fact_id"] for fact in packet["proof_fact_model"]["facts"]
                    ],
                    "fact_count": len(packet["proof_fact_model"]["facts"]),
                },
                "readiness": readiness,
            }
        )
    summary = {
        "packet_count": len(packets),
        "by_readiness": dict(sorted(by_status.items())),
        "by_owner_route": dict(sorted(by_route.items())),
        "ready_packet_count": sum(
            1 for row in rows if row["readiness"]["can_open_exact_admission"]
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


def build_report(
    target_packets_path: Path,
    real_frontier_path: Path,
    coevo_ledger_path: Path,
) -> dict[str, Any]:
    packet_doc = load_json(target_packets_path)
    real_frontier = load_json(real_frontier_path)
    coevo = load_json(coevo_ledger_path)
    packets = validate_packet_doc(packet_doc)
    evidence_links = validate_evidence_links(packets, real_frontier)
    packet_rows, packet_summary = summarize_packets(packets)
    coevo_summary = summarize_coevo(coevo)
    ready_count = packet_summary["ready_packet_count"]
    verdict = (
        "exact-admission-ready"
        if ready_count
        else "no-exact-admission-ready-packets"
    )
    return {
        "schema_version": SCHEMA_VERSION,
        "tool_version": TOOL_VERSION,
        "identity": {
            "artifacts": {
                "target_packets": artifact_ref(target_packets_path),
                "real_frontier": artifact_ref(real_frontier_path),
                "coevo_ledger": artifact_ref(coevo_ledger_path),
            },
            "target_packet_identity": packet_doc.get("identity", {}),
        },
        "admission_policy": {
            "exact_admission_requires": [
                "linked real_frontier real-miss evidence",
                "proof invariant narrow enough to defend",
                "adjacent hard-negative siblings",
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
        },
        "coevolution_guardrails": coevo_summary,
    }


def markdown_report(report: dict[str, Any]) -> str:
    target = report["target_packets"]["summary"]
    coevo = report["coevolution_guardrails"]
    by_readiness = json.dumps(target["by_readiness"], sort_keys=True)
    by_owner_route = json.dumps(target["by_owner_route"], sort_keys=True)
    by_verdict = json.dumps(coevo["by_verdict"], sort_keys=True)
    lines = [
        "# Proof-carrying Type-4 frontier",
        "",
        "Admission report for evidence-priced Type-4 expansion. Generated by",
        "`bench/type4/proof_carrying_frontier.py` from target packets, real-frontier",
        "evidence, and the co-evolution packet ledger.",
        "",
        "## Verdict",
        "",
        f"**{report['verdict']}**",
        "",
        f"- target packets: {target['packet_count']}",
        f"- ready for exact admission: {target['ready_packet_count']}",
        f"- by readiness: `{by_readiness}`",
        f"- by owner route: `{by_owner_route}`",
        "",
        "## Admission Policy",
        "",
    ]
    lines.extend(f"- {item}" for item in report["admission_policy"]["exact_admission_requires"])
    lines += [
        "",
        "## Target Packets",
        "",
        "| packet | axis | route | readiness | proof facts | hard negatives |",
        "|---|---|---|---|---:|---:|",
    ]
    for packet in report["target_packets"]["packets"]:
        readiness = packet["readiness"]
        lines.append(
            f"| `{packet['packet_id']}` | `{packet['candidate_axis']}` | "
            f"`{packet['owner_route']}` | `{readiness['status']}` | "
            f"{packet['proof_fact_model']['fact_count']} | "
            f"{packet['hard_negative_count']} |"
        )
        if readiness["blocking_items"]:
            lines.append("")
            facts = ", ".join(f"`{fact}`" for fact in packet["proof_fact_model"]["fact_ids"])
            lines.append(f"Proof facts modeled for `{packet['packet_id']}`: {facts}")
            lines.append("")
            lines.append(f"Blocked by `{packet['packet_id']}`:")
            lines.extend(f"- {item}" for item in readiness["blocking_items"])
    lines += [
        "",
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


def canonical_json(doc: dict[str, Any]) -> str:
    return json.dumps(doc, indent=2, sort_keys=True) + "\n"


def check_artifacts(report: dict[str, Any], json_out: Path, markdown_out: Path) -> None:
    expected_json = canonical_json(report)
    expected_md = markdown_report(report)
    mismatches = []
    if not json_out.exists() or json_out.read_text() != expected_json:
        mismatches.append(repo_rel(json_out))
    if not markdown_out.exists() or markdown_out.read_text() != expected_md:
        mismatches.append(repo_rel(markdown_out))
    if mismatches:
        joined = ", ".join(mismatches)
        raise FrontierError(f"proof-carrying frontier artifacts are stale: {joined}")


def write_artifacts(report: dict[str, Any], json_out: Path, markdown_out: Path) -> None:
    json_out.write_text(canonical_json(report))
    markdown_out.write_text(markdown_report(report))


def selftest() -> None:
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
        "blocked_by": ["missing proof"],
        "notes": "n/a",
    }
    packet_doc = {"schema_version": 1, "packet_count": 1, "packets": [packet]}
    packets = validate_packet_doc(packet_doc)
    assert readiness_for(packet)["status"] == "blocked-on-proof"
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
    try:
        bad = dict(packet)
        bad["hard_negative_siblings"] = []
        validate_packet_doc({"schema_version": 1, "packet_count": 1, "packets": [bad]})
        raise AssertionError("missing hard negative was not detected")
    except FrontierError:
        pass
    no_model = dict(packet)
    no_model["proof_fact_model"] = {}
    try:
        validate_packet_doc({"schema_version": 1, "packet_count": 1, "packets": [no_model]})
        raise AssertionError("missing proof fact model was not detected")
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
    parser.add_argument("--json-out", type=Path, default=DEFAULT_JSON_OUT)
    parser.add_argument("--markdown-out", type=Path, default=DEFAULT_MARKDOWN_OUT)
    parser.add_argument("--check", action="store_true", help="fail if committed artifacts are stale")
    parser.add_argument("--selftest", action="store_true", help="run corpus-free checks")
    args = parser.parse_args()

    try:
        if args.selftest:
            selftest()
            return 0
        report = build_report(args.target_packets, args.real_frontier, args.coevo_ledger)
        if args.check:
            check_artifacts(report, args.json_out, args.markdown_out)
        else:
            write_artifacts(report, args.json_out, args.markdown_out)
    except FrontierError as exc:
        print(f"error: {exc}", file=sys.stderr)
        return 1
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
