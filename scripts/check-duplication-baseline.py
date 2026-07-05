#!/usr/bin/env python3
"""Verify nose's self-duplication families against the accepted baseline."""

from __future__ import annotations

import argparse
import json
import subprocess
import sys
from pathlib import Path
from typing import Any


DEFAULT_BASELINE = Path("scripts/duplication-baseline.json")
DEFAULT_BIN = Path("./target/release/nose")


def load_baseline(path: Path) -> dict[str, Any]:
    try:
        baseline = json.loads(path.read_text(encoding="utf-8"))
    except FileNotFoundError:
        raise SystemExit(f"missing duplication baseline: {path}") from None
    except json.JSONDecodeError as err:
        raise SystemExit(f"invalid JSON in {path}: {err}") from None

    ids = baseline.get("accepted_family_ids")
    if not isinstance(ids, list) or not all(isinstance(item, str) for item in ids):
        raise SystemExit(f"{path}: accepted_family_ids must be a list of strings")
    if len(ids) != len(set(ids)):
        raise SystemExit(f"{path}: accepted_family_ids contains duplicates")
    if not isinstance(baseline.get("budget"), int) or baseline["budget"] < 0:
        raise SystemExit(f"{path}: budget must be a non-negative integer")
    if not isinstance(baseline.get("min_value"), int) or baseline["min_value"] <= 0:
        raise SystemExit(f"{path}: min_value must be a positive integer")
    if not isinstance(baseline.get("mode"), str) or not baseline["mode"]:
        raise SystemExit(f"{path}: mode must be a non-empty string")
    if not isinstance(baseline.get("recommended_surface"), str):
        raise SystemExit(f"{path}: recommended_surface must be a string")
    if len(ids) > baseline["budget"]:
        raise SystemExit(
            f"{path}: {len(ids)} accepted IDs exceeds budget {baseline['budget']}"
        )
    return baseline


def current_family_ids(nose_bin: Path, baseline: dict[str, Any]) -> list[str]:
    command = [
        str(nose_bin),
        "query",
        "crates",
        "all",
        "top=0",
        "--mode",
        baseline["mode"],
        "--min-value",
        str(baseline["min_value"]),
        "--format",
        "json",
    ]
    result = subprocess.run(
        command,
        check=False,
        stdout=subprocess.PIPE,
        stderr=subprocess.PIPE,
        text=True,
    )
    if result.returncode != 0:
        print(result.stderr, file=sys.stderr, end="")
        raise SystemExit(result.returncode)
    try:
        payload = json.loads(result.stdout)
    except json.JSONDecodeError as err:
        raise SystemExit(f"nose JSON output was invalid: {err}") from None

    surface = baseline["recommended_surface"]
    return sorted(
        family_id
        for family in payload.get("families", [])
        if (family_id := family.get("id") or family.get("family_id"))
        and (family.get("surface") or family.get("recommended_surface")) == surface
    )


def check(nose_bin: Path, baseline_path: Path) -> int:
    baseline = load_baseline(baseline_path)
    accepted = sorted(baseline["accepted_family_ids"])
    current = current_family_ids(nose_bin, baseline)
    budget = baseline["budget"]
    surface = baseline["recommended_surface"]
    min_value = baseline["min_value"]

    print(
        "duplication gate: "
        f"{len(current)} substantial {surface}-surface near-duplicate families "
        f"(value >= {min_value}), budget {budget}"
    )

    accepted_set = set(accepted)
    current_set = set(current)
    unreviewed = sorted(current_set - accepted_set)
    resolved_or_changed = sorted(accepted_set - current_set)
    failures: list[str] = []
    if len(current) > budget:
        failures.append(f"{len(current)} current families exceeds budget {budget}")
    if unreviewed:
        failures.append("unreviewed current family IDs: " + ", ".join(unreviewed))
    if resolved_or_changed:
        failures.append(
            "baseline IDs no longer reported: " + ", ".join(resolved_or_changed)
        )

    if failures:
        print("", file=sys.stderr)
        print("FAILED: duplication baseline drifted.", file=sys.stderr)
        for failure in failures:
            print(f"  - {failure}", file=sys.stderr)
        print("", file=sys.stderr)
        print(
            "Evaluate the family delta, update docs/dogfooding-history.md, then "
            f"update {baseline_path} in the same change.",
            file=sys.stderr,
        )
        return 1

    print("OK")
    return 0


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--bin", type=Path, default=DEFAULT_BIN)
    parser.add_argument("--baseline", type=Path, default=DEFAULT_BASELINE)
    args = parser.parse_args()

    if not args.bin.exists():
        raise SystemExit(f"nose binary not found at {args.bin}; build with: cargo build --release")
    return check(args.bin, args.baseline)


if __name__ == "__main__":
    raise SystemExit(main())
