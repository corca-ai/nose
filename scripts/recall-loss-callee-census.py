#!/usr/bin/env python3
"""Census the callee-identity bucket in a recall_loss_report.v1.json artifact."""

from __future__ import annotations

import argparse
import json
import sys
from collections import Counter, defaultdict
from copy import deepcopy
from typing import Any


CALLEE_REASON = "import-symbol-callee-identity-proof-missing"

SURFACE_LABELS = [
    "local-or-parameter-call-target-proof",
    "scoped-path-call-target-proof",
    "member-call-target-proof",
    "imported-binding-call-target-proof",
    "imported-member-call-target-proof",
    "qualified-global-call-target-proof",
    "unshadowed-global-call-target-proof",
    "call-target-evidence-rejected",
    "direct-function-target-present-call-contract-proof",
    "direct-method-target-present-call-contract-proof",
    "imported-function-target-present-call-contract-proof",
    "imported-member-target-present-call-contract-proof",
    "dynamic-dispatch-target-present-concrete-target-proof",
    "unknown-call-target-proof",
    "import-or-call-target-proof",
]


def load_report(path: str) -> dict[str, Any]:
    with open(path, encoding="utf-8") as handle:
        report = json.load(handle)
    if report.get("schema_version") != 1:
        raise SystemExit(f"{path}: expected schema_version=1")
    if report.get("report_kind") != "recall-loss-diagnostics":
        raise SystemExit(f"{path}: expected recall-loss-diagnostics report")
    return report


def count_rows(counter: Counter[str], key_name: str) -> list[dict[str, Any]]:
    rows = [{key_name: key, "count": count} for key, count in counter.items()]
    rows.sort(key=lambda row: (-row["count"], row[key_name]))
    return rows


def primary_surface(rejection: dict[str, Any]) -> str:
    evidence = rejection.get("missing_evidence", [])
    for label in SURFACE_LABELS:
        if label in evidence:
            return label
    return "unclassified-callee-identity"


def loc_key(loc: dict[str, Any]) -> str:
    return f"{loc.get('file', '')}:{loc.get('start_line', 0)}:{loc.get('end_line', 0)}"


def representative_row(rejection: dict[str, Any]) -> dict[str, Any]:
    loc = rejection.get("loc", {})
    return {
        "file": loc.get("file"),
        "start_line": loc.get("start_line"),
        "end_line": loc.get("end_line"),
        "language": loc.get("language"),
        "tokens": loc.get("tokens"),
        "missing_evidence": rejection.get("missing_evidence", []),
    }


def build_census(
    report: dict[str, Any],
    *,
    issue: int,
    parent_issue: int,
    generated_on: str,
    representative_limit: int,
) -> dict[str, Any]:
    rejections = [
        row
        for row in report.get("admission_rejections", [])
        if row.get("reason") == CALLEE_REASON
    ]
    by_language: Counter[str] = Counter()
    by_surface: Counter[str] = Counter()
    language_surface: Counter[tuple[str, str]] = Counter()
    representatives: dict[str, list[dict[str, Any]]] = defaultdict(list)

    for rejection in sorted(rejections, key=lambda row: loc_key(row.get("loc", {}))):
        loc = rejection.get("loc", {})
        language = str(loc.get("language", "unknown"))
        surface = primary_surface(rejection)
        by_language[language] += 1
        by_surface[surface] += 1
        language_surface[(language, surface)] += 1
        if len(representatives[surface]) < representative_limit:
            representatives[surface].append(representative_row(rejection))

    return {
        "schema_version": 1,
        "report_kind": "callee-identity-census",
        "issue": issue,
        "parent_issue": parent_issue,
        "generated_on": generated_on,
        "source_report_kind": report.get("report_kind"),
        "source_command": report.get("command"),
        "selected_reason": CALLEE_REASON,
        "hard_gate": {
            "false_merges": report.get("soundness_gate", {}).get("false_merges", 0),
            "canon_preservation_violations": report.get("soundness_gate", {}).get(
                "canon_preservation_violations", 0
            ),
            "gate_passed": report.get("soundness_gate", {}).get("gate_passed"),
        },
        "summary": {
            "total_callee_identity_rejections": len(rejections),
            "admission_rejections": report.get("summary", {}).get(
                "admission_rejections", 0
            ),
            "unattributed_strict_exact_unsafe": reason_count(
                report, "unattributed-strict-exact-unsafe"
            ),
        },
        "by_language": count_rows(by_language, "language"),
        "by_primary_surface": count_rows(by_surface, "surface"),
        "by_language_and_surface": [
            {"language": language, "surface": surface, "count": count}
            for (language, surface), count in sorted(
                language_surface.items(), key=lambda item: (-item[1], item[0][0], item[0][1])
            )
        ],
        "representative_units": dict(sorted(representatives.items())),
        "first_recovery_slice": {
            "target": "Rust scoped-path and local-or-parameter call target proof census",
            "rationale": (
                "The current crates signal is overwhelmingly Rust, so the next "
                "implementation slice should prove reusable call-target identity "
                "before expanding import-backed immutable value provenance."
            ),
            "hard_negatives": [
                "provider mutation",
                "consumer rebinding",
                "wildcard or ambiguous import",
                "dynamic import or export",
                "side-effecting module initialization",
                "map key/value or coordinate confusion",
            ],
            "non_goals": [
                "broad cross-file constant propagation",
                "package ecosystem semantics",
                "dynamic import/eval/reflection",
                "exact admission without dependency-backed identity evidence",
            ],
        },
    }


def reason_count(report: dict[str, Any], reason: str) -> int:
    for row in report.get("by_reason", []):
        if row.get("reason") == reason:
            return int(row.get("count", 0))
    return 0


def markdown_table(headers: list[str], rows: list[list[str]]) -> str:
    out = ["| " + " | ".join(headers) + " |"]
    out.append("| " + " | ".join(["---"] * len(headers)) + " |")
    out.extend("| " + " | ".join(row) + " |" for row in rows)
    return "\n".join(out)


def render_markdown(census: dict[str, Any]) -> str:
    language_rows = [
        [row["language"], str(row["count"])] for row in census["by_language"]
    ]
    surface_rows = [
        [row["surface"], str(row["count"])]
        for row in census["by_primary_surface"]
    ]
    summary = census["summary"]
    gate = census["hard_gate"]
    sections = [
        "## Callee-identity census",
        "",
        f"- selected reason: `{census['selected_reason']}`",
        f"- total: `{summary['total_callee_identity_rejections']}`",
        f"- false merges: `{gate['false_merges']}`",
        f"- canon-preservation violations: `{gate['canon_preservation_violations']}`",
        f"- unattributed strict-exact unsafe: `{summary['unattributed_strict_exact_unsafe']}`",
        "",
        "### By language",
        markdown_table(["language", "count"], language_rows),
        "",
        "### By primary surface",
        markdown_table(["surface", "count"], surface_rows),
    ]
    return "\n".join(sections) + "\n"


def sample_report() -> dict[str, Any]:
    return {
        "schema_version": 1,
        "report_kind": "recall-loss-diagnostics",
        "command": {"paths": ["crates"]},
        "summary": {"admission_rejections": 3},
        "soundness_gate": {
            "false_merges": 0,
            "canon_preservation_violations": 0,
            "gate_passed": True,
        },
        "by_reason": [
            {"reason": CALLEE_REASON, "count": 2},
            {"reason": "unattributed-strict-exact-unsafe", "count": 0},
        ],
        "admission_rejections": [
            {
                "reason": CALLEE_REASON,
                "missing_evidence": ["local-or-parameter-call-target-proof"],
                "loc": {
                    "file": "a.rs",
                    "start_line": 1,
                    "end_line": 3,
                    "tokens": 7,
                    "language": "rust",
                },
            },
            {
                "reason": CALLEE_REASON,
                "missing_evidence": ["scoped-path-call-target-proof"],
                "loc": {
                    "file": "b.ts",
                    "start_line": 5,
                    "end_line": 6,
                    "tokens": 5,
                    "language": "typescript",
                },
            },
            {
                "reason": "receiver-domain-proof-missing",
                "missing_evidence": ["receiver-domain-proof"],
                "loc": {
                    "file": "c.rs",
                    "start_line": 8,
                    "end_line": 9,
                    "tokens": 5,
                    "language": "rust",
                },
            },
        ],
    }


def self_test() -> None:
    census = build_census(
        deepcopy(sample_report()),
        issue=574,
        parent_issue=567,
        generated_on="2026-06-27",
        representative_limit=2,
    )
    assert census["summary"]["total_callee_identity_rejections"] == 2
    by_language = {row["language"]: row["count"] for row in census["by_language"]}
    assert by_language == {"rust": 1, "typescript": 1}
    by_surface = {row["surface"]: row["count"] for row in census["by_primary_surface"]}
    assert by_surface["local-or-parameter-call-target-proof"] == 1
    assert by_surface["scoped-path-call-target-proof"] == 1
    rendered = render_markdown(census)
    assert "local-or-parameter-call-target-proof" in rendered


def main(argv: list[str]) -> int:
    parser = argparse.ArgumentParser(
        description="Census import-symbol callee-identity recall-loss surfaces."
    )
    parser.add_argument("report", nargs="?")
    parser.add_argument("--format", choices=["json", "markdown"], default="json")
    parser.add_argument("--issue", type=int, default=574)
    parser.add_argument("--parent-issue", type=int, default=567)
    parser.add_argument("--generated-on", default="2026-06-27")
    parser.add_argument("--representative-limit", type=int, default=3)
    parser.add_argument("--self-test", action="store_true")
    args = parser.parse_args(argv)

    if args.self_test:
        self_test()
        return 0
    if not args.report:
        parser.error("report is required unless --self-test is used")

    census = build_census(
        load_report(args.report),
        issue=args.issue,
        parent_issue=args.parent_issue,
        generated_on=args.generated_on,
        representative_limit=args.representative_limit,
    )
    if args.format == "json":
        print(json.dumps(census, indent=2, sort_keys=True))
    else:
        print(render_markdown(census), end="")
    return 0


if __name__ == "__main__":
    raise SystemExit(main(sys.argv[1:]))
