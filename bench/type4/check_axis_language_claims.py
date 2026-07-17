#!/usr/bin/env python3
"""Validate the executable Tier-A Type-4 axis/language claim perimeter."""

from __future__ import annotations

import argparse
import copy
import sys
from pathlib import Path

import axis_claim_gate as gate


def expect_invalid(
    registry: dict, evidence: dict, blind: dict, declarative: dict, label: str
) -> None:
    try:
        gate.validate(registry, evidence, blind, declarative)
    except ValueError:
        return
    raise AssertionError(f"self-test mutation unexpectedly passed: {label}")


def self_test(registry: dict, evidence: dict, blind: dict, declarative: dict) -> None:
    gate.validate(registry, evidence, blind, declarative)

    missing = copy.deepcopy(registry)
    missing["axes"].pop()
    expect_invalid(missing, evidence, blind, declarative, "unregistered exact axis")

    unguarded = copy.deepcopy(evidence)
    first = registry["axes"][0]
    for row in unguarded["evidence"]:
        if row.get("axis") == first["axis_id"] and row.get("language") in first["exact_languages"]:
            row["neg"] = 0
    expect_invalid(registry, unguarded, blind, declarative, "exact cell without hard negative")

    masked_gap = copy.deepcopy(evidence)
    for claim in registry["axes"]:
        found = False
        for language in claim["exact_languages"]:
            positive_rows = [
                row
                for row in masked_gap["evidence"]
                if row.get("axis") == claim["axis_id"]
                and row.get("language") == language
                and row.get("pos", 0) > 0
            ]
            if len(positive_rows) > 1:
                positive_rows[0]["pos_hit"] -= 1
                found = True
                break
        if found:
            break
    if not found:
        raise AssertionError("self-test fixture needs a multi-producer exact cell")
    expect_invalid(registry, masked_gap, blind, declarative, "one producer masking a gap")

    drift = copy.deepcopy(registry)
    first_closed = next(claim for claim in drift["axes"] if claim["closed_languages"])
    language = first_closed["closed_languages"].pop()
    first_closed["exact_languages"].append(language)
    expect_invalid(drift, evidence, blind, declarative, "unsupported exact-language claim")

    fake_group = copy.deepcopy(registry)
    fake_group["axes"][0]["hard_negative_group"] = "axis.not-real.adjacent"
    expect_invalid(fake_group, evidence, blind, declarative, "unlinked hard-negative group")

    blind_failure = copy.deepcopy(blind)
    blind_failure["hard_gate"]["false_merges"] = 1
    expect_invalid(registry, evidence, blind_failure, declarative, "blind false merge")

    empty_blind = copy.deepcopy(blind)
    empty_blind["summary"] = {}
    empty_blind["oracle_exclusions"] = {}
    expect_invalid(registry, evidence, empty_blind, declarative, "empty blind receipt")

    disconnected = copy.deepcopy(declarative)
    disconnected["cases"][0]["hard_negatives"] = []
    expect_invalid(registry, evidence, blind, disconnected, "disconnected declarative row")

    unknown = copy.deepcopy(evidence)
    unknown["evidence"].append(
        {
            "axis": "unregistered_exact_axis",
            "gen_axis": "unregistered_exact_axis",
            "language": "rust",
            "status": "covered",
            "pos_hit": 1,
            "pos": 1,
            "false_merges": 0,
            "neg": 1,
            "source": "sweep",
        }
    )
    expect_invalid(registry, unknown, blind, declarative, "unknown exact taxonomy axis")

    manual = copy.deepcopy(evidence)
    manual["evidence"].append(
        {
            "axis": registry["axes"][0]["axis_id"],
            "gen_axis": "manual-credit",
            "language": registry["axes"][0]["exact_languages"][0],
            "status": "covered",
            "pos_hit": 1,
            "pos": 1,
            "false_merges": 0,
            "neg": 1,
            "source": "manual",
        }
    )
    expect_invalid(registry, manual, blind, declarative, "unregistered evidence producer")

    regressed = copy.deepcopy(registry)
    claim = next(item for item in regressed["axes"] if item["exact_languages"])
    language = claim["exact_languages"].pop()
    claim["closed_languages"].append(language)
    try:
        gate.validate_ratchet(regressed, declarative, registry, declarative)
    except ValueError:
        pass
    else:
        raise AssertionError("self-test mutation unexpectedly passed: exact-cell ratchet")

    fewer_cases = copy.deepcopy(declarative)
    fewer_cases["cases"].pop()
    try:
        gate.validate_ratchet(registry, fewer_cases, registry, declarative)
    except ValueError:
        pass
    else:
        raise AssertionError("self-test mutation unexpectedly passed: declarative ratchet")


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--self-test", action="store_true")
    parser.add_argument("--nose", type=Path, help="execute the CSS/HTML matrix with this binary")
    parser.add_argument("--ratchet-base", help="git revision whose exact cells must not regress")
    args = parser.parse_args()
    registry, evidence, blind, declarative = (
        gate.load(gate.REGISTRY),
        gate.load(gate.EVIDENCE),
        gate.load(gate.BLIND_RECEIPT),
        gate.load(gate.DECLARATIVE_MATRIX),
    )
    if args.self_test:
        self_test(registry, evidence, blind, declarative)
        print("axis-language claim checker self-test: ok")
        return 0
    try:
        summary = gate.validate(registry, evidence, blind, declarative)
        if args.ratchet_base:
            gate.validate_ratchet(
                registry,
                declarative,
                gate.load_at_revision(args.ratchet_base, gate.REGISTRY),
                gate.load_at_revision(args.ratchet_base, gate.DECLARATIVE_MATRIX),
            )
        if args.nose:
            gate.execute_declarative_matrix(args.nose, declarative)
    except ValueError as exc:
        print(f"axis-language claim gate failed:\n{exc}", file=sys.stderr)
        return 1
    print(
        "axis-language claim gate: "
        f"{summary['axes']} Tier-A axes, {summary['cells']} exact cells, "
        f"{summary['closed_cells']} explicit closed cells; "
        f"declarative {summary['declarative_rows']} rows/"
        f"{summary['declarative_negatives']} hard negatives; "
        "0 informed/blind false merges, 0 canon violations"
    )
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
