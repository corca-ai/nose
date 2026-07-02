#!/usr/bin/env python3
"""Deterministic diff for recall_loss_report.v1.json artifacts."""

from __future__ import annotations

import argparse
import json
import sys
from copy import deepcopy
from typing import Any


Metric = tuple[str, str, str]
OracleExclusionClassificationKey = tuple[str, str]
OracleExclusionObligationKey = tuple[str, str, str, str]

METRICS: list[Metric] = [
    ("false_merges", "soundness_gate", "false_merges"),
    (
        "canon_preservation_violations",
        "soundness_gate",
        "canon_preservation_violations",
    ),
    ("completeness_percent", "completeness", "completeness_percent"),
    ("under_merged_behavior_groups", "completeness", "under_merged_behavior_groups"),
    ("admission_rejections", "summary", "admission_rejections"),
    ("interpretable_units", "summary", "interpretable_units"),
    ("excluded_units", "summary", "excluded_units"),
]


def load_report(path: str) -> dict[str, Any]:
    with open(path, encoding="utf-8") as handle:
        report = json.load(handle)
    if report.get("schema_version") != 1:
        raise SystemExit(f"{path}: expected schema_version=1")
    if report.get("report_kind") != "recall-loss-diagnostics":
        raise SystemExit(f"{path}: expected recall-loss-diagnostics report")
    return report


def number(report: dict[str, Any], section: str, key: str) -> int | float:
    value = report.get(section, {}).get(key)
    if isinstance(value, (int, float)):
        return value
    return 0


def count_map(rows: list[dict[str, Any]], key_field: str = "reason") -> dict[str, int]:
    counts: dict[str, int] = {}
    for row in rows:
        key = str(row.get(key_field, "unknown"))
        counts[key] = counts.get(key, 0) + int(row.get("count", 0))
    return counts


def reason_counts(report: dict[str, Any]) -> dict[str, int]:
    return count_map(report.get("by_reason", []))


def exclusion_counts(report: dict[str, Any]) -> dict[str, int]:
    return count_map(report.get("oracle_exclusions", {}).get("counts", []))


def oracle_exclusion_obligation_counts(
    report: dict[str, Any],
) -> dict[OracleExclusionObligationKey, int]:
    counts: dict[OracleExclusionObligationKey, int] = {}
    for row in report.get("oracle_exclusions", {}).get("by_obligation", []):
        key = (
            str(row.get("exclusion_reason", "unknown")),
            str(row.get("attribution_reason", "unknown")),
            str(row.get("obligation_family", "unknown")),
            str(row.get("obligation_subreason", "unknown")),
        )
        count = row.get("count", row.get("oracle_excluded", 0))
        counts[key] = counts.get(key, 0) + int(count)
    return counts


def oracle_exclusion_classification_counts(
    report: dict[str, Any],
) -> dict[OracleExclusionClassificationKey, int]:
    counts: dict[OracleExclusionClassificationKey, int] = {}
    for row in report.get("oracle_exclusions", {}).get("by_classification", []):
        key = (
            str(row.get("exclusion_reason", "unknown")),
            str(row.get("classification", "unknown")),
        )
        count = row.get("count", row.get("oracle_excluded", 0))
        counts[key] = counts.get(key, 0) + int(count)
    return counts


def delta_rows(before: dict[str, int], after: dict[str, int]) -> list[dict[str, Any]]:
    rows = [
        {
            "key": key,
            "before": before.get(key, 0),
            "after": after.get(key, 0),
            "delta": after.get(key, 0) - before.get(key, 0),
        }
        for key in sorted(set(before) | set(after))
    ]
    rows.sort(key=lambda row: (-abs(row["delta"]), row["key"]))
    return rows


def oracle_exclusion_classification_delta_rows(
    before: dict[OracleExclusionClassificationKey, int],
    after: dict[OracleExclusionClassificationKey, int],
) -> list[dict[str, Any]]:
    rows = [
        {
            "exclusion_reason": key[0],
            "classification": key[1],
            "before": before.get(key, 0),
            "after": after.get(key, 0),
            "delta": after.get(key, 0) - before.get(key, 0),
        }
        for key in sorted(set(before) | set(after))
    ]
    rows.sort(
        key=lambda row: (
            -abs(row["delta"]),
            row["exclusion_reason"],
            row["classification"],
        )
    )
    return rows


def oracle_exclusion_obligation_delta_rows(
    before: dict[OracleExclusionObligationKey, int],
    after: dict[OracleExclusionObligationKey, int],
) -> list[dict[str, Any]]:
    rows = [
        {
            "exclusion_reason": key[0],
            "attribution_reason": key[1],
            "obligation_family": key[2],
            "obligation_subreason": key[3],
            "before": before.get(key, 0),
            "after": after.get(key, 0),
            "delta": after.get(key, 0) - before.get(key, 0),
        }
        for key in sorted(set(before) | set(after))
    ]
    rows.sort(
        key=lambda row: (
            -abs(row["delta"]),
            row["exclusion_reason"],
            row["attribution_reason"],
            row["obligation_family"],
            row["obligation_subreason"],
        )
    )
    return rows


def loc_key(loc: dict[str, Any]) -> str:
    return (
        f"{loc.get('file', '')}:"
        f"{loc.get('start_line', 0)}:"
        f"{loc.get('end_line', 0)}"
    )


def opportunity_key(row: dict[str, Any]) -> str:
    return "|".join(
        [
            str(row.get("reason", "")),
            loc_key(row.get("a", {})),
            loc_key(row.get("b", {})),
        ]
    )


def opportunity_summary(row: dict[str, Any]) -> dict[str, Any]:
    return {
        "reason": row.get("reason"),
        "a": loc_key(row.get("a", {})),
        "b": loc_key(row.get("b", {})),
        "value_jaccard": row.get("value_jaccard"),
        "structurally_near": row.get("structurally_near"),
    }


def changed_opportunities(
    before: dict[str, Any], after: dict[str, Any], limit: int
) -> dict[str, list[dict[str, Any]]]:
    before_rows = {
        opportunity_key(row): row for row in before.get("top_opportunities", [])
    }
    after_rows = {opportunity_key(row): row for row in after.get("top_opportunities", [])}
    added = [
        opportunity_summary(after_rows[key])
        for key in sorted(set(after_rows) - set(before_rows))
    ][:limit]
    removed = [
        opportunity_summary(before_rows[key])
        for key in sorted(set(before_rows) - set(after_rows))
    ][:limit]
    return {"added": added, "removed": removed}


def build_diff(
    before: dict[str, Any], after: dict[str, Any], opportunity_limit: int = 10
) -> dict[str, Any]:
    metrics = [
        {
            "metric": label,
            "before": number(before, section, key),
            "after": number(after, section, key),
            "delta": number(after, section, key) - number(before, section, key),
        }
        for label, section, key in METRICS
    ]
    return {
        "schema_version": 1,
        "report_kind": "recall-loss-diff",
        "metrics": metrics,
        "admission_rejection_deltas": delta_rows(
            reason_counts(before), reason_counts(after)
        ),
        "oracle_exclusion_deltas": delta_rows(
            exclusion_counts(before), exclusion_counts(after)
        ),
        "oracle_exclusion_classification_deltas": oracle_exclusion_classification_delta_rows(
            oracle_exclusion_classification_counts(before),
            oracle_exclusion_classification_counts(after),
        ),
        "oracle_exclusion_obligation_deltas": oracle_exclusion_obligation_delta_rows(
            oracle_exclusion_obligation_counts(before),
            oracle_exclusion_obligation_counts(after),
        ),
        "top_opportunity_changes": changed_opportunities(
            before, after, opportunity_limit
        ),
    }


def format_value(value: int | float) -> str:
    if isinstance(value, float):
        return f"{value:.2f}"
    return str(value)


def markdown_table(headers: list[str], rows: list[list[str]]) -> str:
    out = ["| " + " | ".join(headers) + " |"]
    out.append("| " + " | ".join(["---"] * len(headers)) + " |")
    out.extend("| " + " | ".join(row) + " |" for row in rows)
    return "\n".join(out)


def render_markdown(diff: dict[str, Any]) -> str:
    metric_rows = [
        [
            row["metric"],
            format_value(row["before"]),
            format_value(row["after"]),
            format_value(row["delta"]),
        ]
        for row in diff["metrics"]
    ]
    reason_rows = [
        [row["key"], str(row["before"]), str(row["after"]), str(row["delta"])]
        for row in diff["admission_rejection_deltas"]
        if row["before"] or row["after"]
    ]
    exclusion_rows = [
        [row["key"], str(row["before"]), str(row["after"]), str(row["delta"])]
        for row in diff["oracle_exclusion_deltas"]
        if row["before"] or row["after"]
    ]
    exclusion_classification_rows = [
        [
            row["exclusion_reason"],
            row["classification"],
            str(row["before"]),
            str(row["after"]),
            str(row["delta"]),
        ]
        for row in diff["oracle_exclusion_classification_deltas"]
        if row["before"] or row["after"]
    ]
    exclusion_obligation_rows = [
        [
            row["exclusion_reason"],
            row["attribution_reason"],
            row["obligation_family"],
            row["obligation_subreason"],
            str(row["before"]),
            str(row["after"]),
            str(row["delta"]),
        ]
        for row in diff["oracle_exclusion_obligation_deltas"]
        if row["before"] or row["after"]
    ]
    added = [
        [
            row["reason"] or "",
            row["a"],
            row["b"],
            format_value(row["value_jaccard"] or 0),
        ]
        for row in diff["top_opportunity_changes"]["added"]
    ]
    removed = [
        [
            row["reason"] or "",
            row["a"],
            row["b"],
            format_value(row["value_jaccard"] or 0),
        ]
        for row in diff["top_opportunity_changes"]["removed"]
    ]
    sections = [
        "## Recall-loss report diff",
        "",
        "### Gate and recall metrics",
        markdown_table(["metric", "before", "after", "delta"], metric_rows),
        "",
        "### Admission rejections by reason",
        markdown_table(["reason", "before", "after", "delta"], reason_rows),
        "",
        "### Oracle exclusions by reason",
        markdown_table(["reason", "before", "after", "delta"], exclusion_rows),
        "",
        "### Oracle exclusions by classification",
        markdown_table(
            ["exclusion_reason", "classification", "before", "after", "delta"],
            exclusion_classification_rows,
        ),
        "",
        "### Oracle exclusions by obligation",
        markdown_table(
            [
                "exclusion_reason",
                "attribution_reason",
                "obligation_family",
                "obligation_subreason",
                "before",
                "after",
                "delta",
            ],
            exclusion_obligation_rows,
        ),
        "",
        "### Top opportunities added",
        markdown_table(["reason", "a", "b", "value_jaccard"], added),
        "",
        "### Top opportunities removed",
        markdown_table(["reason", "a", "b", "value_jaccard"], removed),
    ]
    return "\n".join(sections) + "\n"


def sample_report() -> dict[str, Any]:
    return {
        "schema_version": 1,
        "report_kind": "recall-loss-diagnostics",
        "summary": {
            "interpretable_units": 10,
            "excluded_units": 2,
            "admission_rejections": 3,
        },
        "soundness_gate": {
            "false_merges": 0,
            "canon_preservation_violations": 0,
        },
        "completeness": {
            "completeness_percent": 50.0,
            "under_merged_behavior_groups": 2,
        },
        "oracle_exclusions": {
            "counts": [
                {"reason": "uninterpretable", "count": 2},
                {"reason": "path-bail", "count": 0},
            ],
            "by_classification": [
                {
                    "exclusion_reason": "uninterpretable",
                    "classification": "missing-oracle-support",
                    "count": 2,
                    "oracle_excluded": 2,
                }
            ],
            "by_obligation": [],
        },
        "by_reason": [
            {"reason": "strict-exact-unsafe", "count": 3},
        ],
        "top_opportunities": [
            {
                "reason": "a:strict-exact-unsafe",
                "a": {"file": "a.py", "start_line": 1, "end_line": 2},
                "b": {"file": "b.py", "start_line": 3, "end_line": 4},
                "value_jaccard": 0.8,
                "structurally_near": True,
            }
        ],
    }


def self_test() -> None:
    before = sample_report()
    after = deepcopy(before)
    after["summary"]["admission_rejections"] = 4
    after["completeness"]["completeness_percent"] = 75.0
    after["by_reason"] = [
        {"reason": "import-symbol-callee-identity-proof-missing", "count": 2},
        {"reason": "receiver-domain-proof-missing", "count": 2},
    ]
    after["oracle_exclusions"]["counts"] = [
        {"reason": "uninterpretable", "count": 1},
        {"reason": "path-bail", "count": 1},
    ]
    after["oracle_exclusions"]["by_obligation"] = [
        {
            "exclusion_reason": "uninterpretable",
            "attribution_reason": "unsupported-runtime-boundary",
            "obligation_family": "scheduling-boundary",
            "obligation_subreason": "async-await-scheduling-contract-missing",
            "count": 1,
            "oracle_excluded": 1,
        }
    ]
    after["oracle_exclusions"]["by_classification"] = [
        {
            "exclusion_reason": "uninterpretable",
            "classification": "missing-oracle-support",
            "count": 1,
            "oracle_excluded": 1,
        },
        {
            "exclusion_reason": "path-bail",
            "classification": "path-exploration-budget",
            "count": 1,
            "oracle_excluded": 1,
        },
    ]
    after["top_opportunities"] = [
        {
            "reason": "b:receiver-domain-proof-missing",
            "a": {"file": "c.py", "start_line": 5, "end_line": 6},
            "b": {"file": "d.py", "start_line": 7, "end_line": 8},
            "value_jaccard": 0.7,
            "structurally_near": True,
        }
    ]
    diff = build_diff(before, after)
    by_metric = {row["metric"]: row for row in diff["metrics"]}
    assert by_metric["admission_rejections"]["delta"] == 1
    assert by_metric["completeness_percent"]["delta"] == 25.0
    by_reason = {row["key"]: row for row in diff["admission_rejection_deltas"]}
    assert by_reason["strict-exact-unsafe"]["delta"] == -3
    assert by_reason["receiver-domain-proof-missing"]["delta"] == 2
    by_exclusion = {row["key"]: row for row in diff["oracle_exclusion_deltas"]}
    assert by_exclusion["path-bail"]["delta"] == 1
    by_exclusion_classification = {
        row["classification"]: row
        for row in diff["oracle_exclusion_classification_deltas"]
    }
    assert by_exclusion_classification["path-exploration-budget"]["delta"] == 1
    by_exclusion_obligation = {
        row["obligation_subreason"]: row
        for row in diff["oracle_exclusion_obligation_deltas"]
    }
    assert (
        by_exclusion_obligation["async-await-scheduling-contract-missing"]["delta"]
        == 1
    )
    assert diff["top_opportunity_changes"]["added"][0]["a"] == "c.py:5:6"
    markdown = render_markdown(diff)
    assert "strict-exact-unsafe" in markdown
    assert "path-exploration-budget" in markdown
    assert "async-await-scheduling-contract-missing" in markdown
    assert "| completeness_percent | 50.00 | 75.00 | 25.00 |" in markdown


def main(argv: list[str]) -> int:
    parser = argparse.ArgumentParser(
        description="Compare two recall_loss_report.v1.json artifacts."
    )
    parser.add_argument("before", nargs="?")
    parser.add_argument("after", nargs="?")
    parser.add_argument("--format", choices=["markdown", "json"], default="markdown")
    parser.add_argument("--opportunity-limit", type=int, default=10)
    parser.add_argument("--self-test", action="store_true")
    args = parser.parse_args(argv)

    if args.self_test:
        self_test()
        return 0
    if not args.before or not args.after:
        parser.error("before and after reports are required unless --self-test is used")
    diff = build_diff(
        load_report(args.before),
        load_report(args.after),
        opportunity_limit=args.opportunity_limit,
    )
    if args.format == "json":
        print(json.dumps(diff, indent=2, sort_keys=True))
    else:
        print(render_markdown(diff), end="")
    return 0


if __name__ == "__main__":
    raise SystemExit(main(sys.argv[1:]))
