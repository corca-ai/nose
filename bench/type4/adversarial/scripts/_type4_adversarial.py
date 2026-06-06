#!/usr/bin/env python3
"""Shared helpers for the Type-4 adversarial coverage harness."""

from __future__ import annotations

from collections import Counter
import json
from pathlib import Path
from typing import Any


ROOT = Path(__file__).resolve().parents[1]
REPO_ROOT = ROOT.parents[2]
MATRIX_PATH = ROOT / "coverage_matrix.v1.json"
REGISTRY_PATH = ROOT / "rule_registry.v1.json"
CASES_PATH = ROOT / "cases" / "cases.v1.json"

VALID_STATUSES = {
    "candidate",
    "covered",
    "under-merged",
    "false-merged",
    "oracle-blocked",
    "proof-fact-blocked",
    "perf-blocked",
    "unsafe",
    "not-applicable",
}
VALID_ACTIONS = {
    "engine",
    "monitor",
    "oracle",
    "performance",
    "proof-facts",
    "soundness",
    "survey",
}
ACTIONABLE_STATUSES = {
    "candidate",
    "under-merged",
    "false-merged",
    "oracle-blocked",
    "proof-fact-blocked",
    "perf-blocked",
}
STATUS_WEIGHT = {
    "false-merged": 1000,
    "under-merged": 500,
    "oracle-blocked": 360,
    "proof-fact-blocked": 330,
    "candidate": 260,
    "perf-blocked": 160,
    "unsafe": -100,
    "covered": -500,
    "not-applicable": -1000,
}
ACTION_WEIGHT = {
    "soundness": 120,
    "engine": 90,
    "oracle": 70,
    "proof-facts": 60,
    "survey": 35,
    "performance": 20,
    "monitor": 0,
}


def load_json(path: Path) -> dict[str, Any]:
    return json.loads(path.read_text())


def load_all() -> tuple[dict[str, Any], dict[str, Any], dict[str, Any]]:
    return load_json(MATRIX_PATH), load_json(REGISTRY_PATH), load_json(CASES_PATH)


def registry_rules(registry: dict[str, Any]) -> dict[str, dict[str, Any]]:
    return {rule["id"]: rule for rule in registry.get("rules", [])}


def case_index(cases: dict[str, Any]) -> dict[str, dict[str, Any]]:
    return {case["id"]: case for case in cases.get("cases", [])}


def cell_score(cell: dict[str, Any]) -> int:
    status = cell.get("status", "")
    action = cell.get("next_action", "")
    return (
        int(cell.get("priority", 0))
        + STATUS_WEIGHT.get(status, -100)
        + ACTION_WEIGHT.get(action, 0)
    )


def actionable_cells(matrix: dict[str, Any], include_covered: bool = False) -> list[dict[str, Any]]:
    cells = matrix.get("cells", [])
    if include_covered:
        return list(cells)
    return [cell for cell in cells if cell.get("status") in ACTIONABLE_STATUSES]


def find_cell(matrix: dict[str, Any], item_id: str) -> dict[str, Any] | None:
    for cell in matrix.get("cells", []):
        if cell.get("id") == item_id:
            return cell
    return None


def validate_all(
    matrix: dict[str, Any], registry: dict[str, Any], cases: dict[str, Any]
) -> list[str]:
    errors: list[str] = []

    if matrix.get("schema_version") != 1:
        errors.append("coverage_matrix.v1.json schema_version must be 1")
    if registry.get("schema_version") != 1:
        errors.append("rule_registry.v1.json schema_version must be 1")
    if cases.get("schema_version") != 1:
        errors.append("cases.v1.json schema_version must be 1")

    rules = registry_rules(registry)
    known_cases = case_index(cases)

    _check_unique("rule", registry.get("rules", []), errors)
    _check_unique("case", cases.get("cases", []), errors)
    _check_unique("cell", matrix.get("cells", []), errors)

    for rule in registry.get("rules", []):
        _require(rule, "id", "rule", errors)
        _require(rule, "kind", f"rule {rule.get('id', '?')}", errors)
        _require(rule, "status", f"rule {rule.get('id', '?')}", errors)
        for field in ("positive_cases", "hard_negatives"):
            for case_id in rule.get(field, []):
                if case_id not in known_cases:
                    errors.append(f"rule {rule['id']} references unknown case {case_id}")

    for case in cases.get("cases", []):
        _require(case, "id", "case", errors)
        _require(case, "kind", f"case {case.get('id', '?')}", errors)
        _require(case, "semantic_family", f"case {case.get('id', '?')}", errors)
        _require(case, "claim", f"case {case.get('id', '?')}", errors)
        for fixture in case.get("fixtures", []):
            if not (REPO_ROOT / fixture).exists():
                errors.append(f"case {case['id']} fixture does not exist: {fixture}")

    for cell in matrix.get("cells", []):
        cell_id = cell.get("id", "?")
        for field in (
            "id",
            "semantic_family",
            "title",
            "status",
            "next_action",
            "priority",
            "languages",
            "representations",
            "rule_ids",
            "positive_cases",
            "adversarial_cases",
            "required_gates",
            "docs",
        ):
            _require(cell, field, f"cell {cell_id}", errors)
        if cell.get("status") not in VALID_STATUSES:
            errors.append(f"cell {cell_id} has invalid status {cell.get('status')}")
        if cell.get("next_action") not in VALID_ACTIONS:
            errors.append(f"cell {cell_id} has invalid next_action {cell.get('next_action')}")
        for rule_id in cell.get("rule_ids", []):
            if rule_id not in rules:
                errors.append(f"cell {cell_id} references unknown rule {rule_id}")
        for field in ("positive_cases", "adversarial_cases"):
            for case_id in cell.get(field, []):
                if case_id not in known_cases:
                    errors.append(f"cell {cell_id} references unknown case {case_id}")
        if cell.get("status") in ACTIONABLE_STATUSES:
            if not cell.get("agent_task"):
                errors.append(f"actionable cell {cell_id} must include agent_task")
            if not cell.get("required_gates"):
                errors.append(f"actionable cell {cell_id} must include required_gates")
        for doc in cell.get("docs", []):
            if not (REPO_ROOT / doc).exists():
                errors.append(f"cell {cell_id} doc does not exist: {doc}")
    return errors


def _require(item: dict[str, Any], field: str, label: str, errors: list[str]) -> None:
    if field not in item or item[field] in ("", None, []):
        errors.append(f"{label} missing required field {field}")


def _check_unique(label: str, items: list[dict[str, Any]], errors: list[str]) -> None:
    ids = [item.get("id") for item in items]
    counts = Counter(ids)
    for item_id, count in counts.items():
        if not item_id:
            errors.append(f"{label} list contains item without id")
        elif count > 1:
            errors.append(f"{label} id {item_id} appears {count} times")


def task_card(cell: dict[str, Any], rules: dict[str, Any], cases: dict[str, Any]) -> str:
    lines = [
        f"ID: {cell['id']}",
        f"Score: {cell_score(cell)}",
        f"Status: {cell['status']}",
        f"Action: {cell['next_action']}",
        f"Family: {cell['semantic_family']}",
        f"Title: {cell['title']}",
    ]
    if cell.get("evidence"):
        lines.extend(["", "Evidence:", f"  {cell['evidence']}"])
    if cell.get("agent_task"):
        lines.extend(["", "Agent task:", f"  {cell['agent_task']}"])

    lines.append("")
    lines.append("Rules:")
    for rule_id in cell.get("rule_ids", []):
        rule = rules.get(rule_id, {})
        lines.append(f"  - {rule_id}: {rule.get('summary', 'unknown rule')}")

    lines.append("")
    lines.append("Positive cases:")
    for case_id in cell.get("positive_cases", []):
        case = cases.get(case_id, {})
        lines.append(f"  - {case_id}: {case.get('claim', 'unknown case')}")

    lines.append("")
    lines.append("Adversarial cases:")
    for case_id in cell.get("adversarial_cases", []):
        case = cases.get(case_id, {})
        lines.append(f"  - {case_id}: {case.get('claim', 'unknown case')}")

    lines.append("")
    lines.append("Required gates:")
    for gate in cell.get("required_gates", []):
        lines.append(f"  - {gate}")

    if cell.get("docs"):
        lines.append("")
        lines.append("Docs:")
        for doc in cell["docs"]:
            lines.append(f"  - {doc}")
    return "\n".join(lines)
