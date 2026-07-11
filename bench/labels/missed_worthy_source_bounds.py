#!/usr/bin/env python3
"""Freeze and validate source line bounds for the #816 dev audit evidence."""

from __future__ import annotations

import argparse
import json
from pathlib import Path

from missed_worthy_frontier import (
    ROOT,
    build_source_bounds,
    load_and_validate_source_bounds,
    source_line_count,
)
from missed_worthy_stage_audit import git_output


DEFAULT_ARTIFACT = ROOT / "bench" / "labels" / "recall_ceiling_probe_2026_07_11.v2.json"
DEFAULT_DECISIONS = (
    ROOT / "bench" / "labels" / "missed_worthy_audit_decisions_2026_07_11.dev.v1.json"
)


def parse_args() -> argparse.Namespace:
    parser = argparse.ArgumentParser(description=__doc__)
    modes = parser.add_mutually_exclusive_group()
    modes.add_argument("--validate", type=Path, metavar="SOURCE_BOUNDS")
    modes.add_argument("--self-test", action="store_true")
    parser.add_argument("--artifact", type=Path, default=DEFAULT_ARTIFACT)
    parser.add_argument("--decisions", type=Path, default=DEFAULT_DECISIONS)
    parser.add_argument("--json-out", type=Path)
    parser.add_argument("--check-sources", action="store_true")
    return parser.parse_args()


def main() -> None:
    args = parse_args()
    if args.self_test:
        assert source_line_count(Path(__file__)) > 0
        print("missed-worthy source-bounds self-test passed")
        return
    if args.validate is not None:
        load_and_validate_source_bounds(
            args.validate,
            args.artifact,
            args.decisions,
            check_sources=args.check_sources,
        )
        print(f"validated {args.validate}")
        return
    if args.json_out is None:
        raise SystemExit("--json-out is required when collecting source bounds")
    status = git_output("status", "--porcelain=v1", "--untracked-files=all")
    if status:
        raise SystemExit("refusing to freeze source bounds from a dirty worktree")
    payload = build_source_bounds(args.artifact, args.decisions)
    payload["provenance"] = {
        "git_sha": git_output("rev-parse", "HEAD"),
        "working_tree_status_before_measurement": status,
    }
    args.json_out.write_text(
        json.dumps(payload, indent=2, sort_keys=True) + "\n",
        encoding="utf-8",
    )
    load_and_validate_source_bounds(
        args.json_out,
        args.artifact,
        args.decisions,
        check_sources=True,
    )
    print(f"wrote and validated {args.json_out}")


if __name__ == "__main__":
    main()
