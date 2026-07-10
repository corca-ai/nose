#!/usr/bin/env python3
"""Merge checker and Ruby scaling results into one semantic-smoke status."""

from __future__ import annotations

import argparse
import json
import tempfile
from pathlib import Path
from typing import Any


def load_scaling(path: Path) -> tuple[dict[str, Any], str]:
    scaling = json.loads(path.read_text(encoding="utf-8"))
    evaluation = scaling["evaluation"]
    summary = (
        f"`{evaluation['status']}`; growth exponent "
        f"{evaluation['growth_exponent']:.2f} "
        f"(limit {evaluation['max_growth_exponent']:.2f})."
    )
    return scaling, summary


def merge_results(
    *,
    scaling_path: Path,
    summary_path: Path,
    status_path: Path,
    scaling_return_code: int,
    checker_return_code: int,
) -> str:
    overall = "pass" if scaling_return_code == 0 and checker_return_code == 0 else "fail"
    try:
        scaling, scaling_summary = load_scaling(scaling_path)
    except (OSError, json.JSONDecodeError, KeyError, TypeError, ValueError) as error:
        scaling = {"status": "error", "error": str(error)}
        scaling_summary = f"`error`; report unavailable ({error})."
        overall = "fail"

    summary_text = (
        summary_path.read_text(encoding="utf-8")
        if summary_path.exists()
        else "## Semantic regression smoke\n\n"
    )
    status_line = f"**Status:** `{overall}`"
    if "**Status:** `" in summary_text:
        lines = summary_text.splitlines()
        lines = [status_line if line.startswith("**Status:** `") else line for line in lines]
        summary_text = "\n".join(lines) + "\n"
    else:
        summary_text += status_line + "\n"
    summary_text += f"\nRuby scaling: {scaling_summary}\n"
    summary_path.parent.mkdir(parents=True, exist_ok=True)
    summary_path.write_text(summary_text, encoding="utf-8")

    try:
        status = json.loads(status_path.read_text(encoding="utf-8"))
    except (OSError, json.JSONDecodeError, TypeError, ValueError):
        status = {"schema": "nose.semantic_regression_check.v1", "status": "error"}
    status["ruby_scaling"] = scaling
    status["overall_status"] = overall
    status_path.parent.mkdir(parents=True, exist_ok=True)
    status_path.write_text(json.dumps(status, indent=2, sort_keys=True) + "\n", encoding="utf-8")
    return overall


def run_self_test() -> None:
    with tempfile.TemporaryDirectory(prefix="nose-semantic-summary-") as directory:
        root = Path(directory)
        scaling = root / "scaling.json"
        summary = root / "summary.md"
        status = root / "status.json"
        scaling.write_text(
            json.dumps(
                {
                    "evaluation": {
                        "status": "within-threshold",
                        "growth_exponent": 0.7,
                        "max_growth_exponent": 1.35,
                    }
                }
            ),
            encoding="utf-8",
        )
        summary.write_text("## Semantic regression smoke\n\n**Status:** `pass`\n")
        status.write_text('{"schema":"nose.semantic_regression_check.v1","status":"pass"}')
        assert merge_results(
            scaling_path=scaling,
            summary_path=summary,
            status_path=status,
            scaling_return_code=0,
            checker_return_code=0,
        ) == "pass"
        assert "**Status:** `pass`" in summary.read_text()

        assert merge_results(
            scaling_path=scaling,
            summary_path=summary,
            status_path=status,
            scaling_return_code=1,
            checker_return_code=0,
        ) == "fail"
        assert "**Status:** `fail`" in summary.read_text()
        assert json.loads(status.read_text())["overall_status"] == "fail"

        scaling.unlink()
        assert merge_results(
            scaling_path=scaling,
            summary_path=summary,
            status_path=status,
            scaling_return_code=0,
            checker_return_code=0,
        ) == "fail"
        assert json.loads(status.read_text())["ruby_scaling"]["status"] == "error"
    print("semantic regression summary self-test passed")


def parse_args() -> argparse.Namespace:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--scaling-report", type=Path)
    parser.add_argument("--summary", type=Path)
    parser.add_argument("--status", type=Path)
    parser.add_argument("--scaling-return-code", type=int, default=0)
    parser.add_argument("--checker-return-code", type=int, default=0)
    parser.add_argument("--self-test", action="store_true")
    return parser.parse_args()


def main() -> int:
    args = parse_args()
    if args.self_test:
        run_self_test()
        return 0
    if args.scaling_report is None or args.summary is None or args.status is None:
        raise SystemExit("--scaling-report, --summary, and --status are required")
    merge_results(
        scaling_path=args.scaling_report,
        summary_path=args.summary,
        status_path=args.status,
        scaling_return_code=args.scaling_return_code,
        checker_return_code=args.checker_return_code,
    )
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
