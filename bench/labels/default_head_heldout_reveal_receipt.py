#!/usr/bin/env python3
"""External Git receipt for the atomic #846 held-out reveal."""

from __future__ import annotations

import argparse
import hashlib
import subprocess
from collections import Counter
from pathlib import Path

import default_head_heldout as heldout
import default_head_heldout_arbitration_result_receipt as result_receipt
import default_head_heldout_reveal as reveal


ROOT = Path(__file__).resolve().parents[2]
REVEAL_COMMIT = "e1bafefea5242194c7d7ea8b2b3f2e2fc6c15a6a"
REVEAL_TREE = "cdce9053430b6b298abbd1504db60f09b33d22d0"
REVEAL_PARENT = "3558f6f674c3290dbab365787313fbeaca063974"
COLLECTOR_TREE = "64599534c15c895a52dae7b093811c17a3e03ed9"
COLLECTOR_SHA256 = "e957ba10c2ed07ca355050e9372b7025ab7b0c2bbc901218693d2d3838e9cb32"
COLLECTOR_BYTES = 59_371
ARTIFACTS = (
    (
        "bench/labels/default_head_heldout_arbitration_packet_reveal_2026_07_14.heldout.v3.json",
        599_244,
        "b0426488847d400a232e4feaed352422919ea825148dc6c0179dce9f9e764005",
    ),
    (
        "bench/labels/default_head_heldout_packet_reveal_2026_07_14.heldout.dedupe.v3.json",
        6_083_360,
        "57d4ddf561a4df142cb85f9b0970033cf639399145f2ab5eac5f0186685d3de3",
    ),
    (
        "bench/labels/default_head_heldout_packet_reveal_2026_07_14.heldout.pragmatic.v3.json",
        6_083_363,
        "5375c63a612f2e36f60f1e2b91c51a87af8b8078b3bd0baf7a2757e709d6e92d",
    ),
    (
        "bench/labels/default_head_heldout_packet_reveal_2026_07_14.heldout.skeptic.v3.json",
        6_083_361,
        "32db34ec0e441ea1a79de3fd776fa2b83a78082a0e03fded7afef061f99ad3b1",
    ),
    (
        "bench/labels/default_head_heldout_reveal_2026_07_14.heldout.v3.json",
        556_937,
        "a401ebe7a9c5120a0edc618e6cbfee80bb5bd2d4b28ba45c4b823de8f58f9241",
    ),
    (
        "bench/labels/default_head_label_decisions_2026_07_14.heldout.v3.json",
        157_498,
        "8a1d7056af733e28332372016269a5c62326b056fcad25795912174b8b40657a",
    ),
    (
        "bench/labels/refactoring_families.v7.heldout.json",
        634_108,
        "a80a2bea6fb60c78be1f619f6f2067d51f0583fc71ee6520722580027070b93d",
    ),
)
EXPECTED_REASONS = {
    "coincidental-shape": 2,
    "extract-base": 17,
    "extract-data-table": 5,
    "extract-helper": 75,
    "generated": 17,
    "parallel-by-design": 24,
    "parameterize": 50,
    "trivial": 21,
    "type-def": 3,
}


def git_bytes(args: list[str]) -> bytes:
    completed = subprocess.run(
        ["git", *args], cwd=ROOT, check=False, capture_output=True
    )
    if completed.returncode != 0:
        raise ValueError(
            f"git {' '.join(args)} failed: "
            f"{completed.stderr.decode(errors='replace').strip()}"
        )
    return completed.stdout


def git_text(args: list[str]) -> str:
    return git_bytes(args).decode().strip()


def require_equal(actual: object, expected: object, label: str) -> None:
    reveal.require_equal(actual, expected, label)


def require_ancestor(ancestor: str, descendant: str, label: str) -> None:
    completed = subprocess.run(
        ["git", "merge-base", "--is-ancestor", ancestor, descendant],
        cwd=ROOT,
        check=False,
        capture_output=True,
    )
    if completed.returncode != 0:
        raise ValueError(f"{label}: mismatch")


def validate_git_receipt() -> None:
    require_equal(
        git_text(["rev-parse", f"{REVEAL_COMMIT}^{{tree}}"]),
        REVEAL_TREE,
        "reveal tree",
    )
    require_equal(
        git_text(["show", "-s", "--format=%P", REVEAL_COMMIT]),
        REVEAL_PARENT,
        "reveal parent",
    )
    paths = [path for path, _, _ in ARTIFACTS]
    require_equal(
        git_text(
            ["diff-tree", "--no-commit-id", "--name-only", "-r", REVEAL_COMMIT]
        ).splitlines(),
        paths,
        "atomic reveal paths",
    )
    require_equal(
        git_text(
            [
                "diff-tree",
                "--no-commit-id",
                "--name-only",
                "--diff-filter=A",
                "-r",
                REVEAL_COMMIT,
            ]
        ).splitlines(),
        paths,
        "atomic reveal additions",
    )
    for path, expected_bytes, expected_sha in ARTIFACTS:
        frozen = git_bytes(["show", f"{REVEAL_COMMIT}:{path}"])
        require_equal(len(frozen), expected_bytes, f"{path} frozen bytes")
        require_equal(
            hashlib.sha256(frozen).hexdigest(), expected_sha, f"{path} frozen SHA"
        )
        require_equal((ROOT / path).read_bytes(), frozen, f"{path} current bytes")
        mode = git_text(["ls-tree", REVEAL_COMMIT, "--", path]).split(maxsplit=1)[0]
        require_equal(mode, "100644", f"{path} Git mode")
    require_equal(
        git_text(["rev-parse", f"{REVEAL_PARENT}^{{tree}}"]),
        COLLECTOR_TREE,
        "reveal collector tree",
    )
    collector_path = "bench/labels/default_head_heldout_reveal.py"
    collector = git_bytes(["show", f"{REVEAL_PARENT}:{collector_path}"])
    require_equal(len(collector), COLLECTOR_BYTES, "reveal collector bytes")
    require_equal(
        hashlib.sha256(collector).hexdigest(),
        COLLECTOR_SHA256,
        "reveal collector SHA",
    )
    require_ancestor(
        result_receipt.RESULT_COMMIT,
        REVEAL_PARENT,
        "result-before-reveal collector",
    )
    require_ancestor(REVEAL_COMMIT, "HEAD", "reveal commit ancestry")


def validate_summary() -> None:
    component = heldout.read_json(reveal.COMPONENT)
    families = component["families"]
    require_equal(len(families), 214, "held-out decision count")
    require_equal(sum(row["worthy"] for row in families), 147, "held-out worthy count")
    require_equal(
        dict(sorted(Counter(row["reason"] for row in families).items())),
        EXPECTED_REASONS,
        "held-out reason summary",
    )
    require_equal(
        dict(sorted(Counter(row["labeler"] for row in families).items())),
        {"llm-arbiter": 90, "panel": 124},
        "held-out labeler summary",
    )


def validate(_: argparse.Namespace) -> None:
    validate_git_receipt()
    reveal.validate_checked()
    validate_summary()
    print("held-out reveal receipt OK: 7 atomic artifacts, 214 decisions, 147 worthy")


def self_test(_: argparse.Namespace) -> None:
    validate_git_receipt()
    reveal.validate_checked()
    validate_summary()
    paths = [path for path, _, _ in ARTIFACTS]
    if len(paths) != len(set(paths)) or paths != sorted(paths):
        raise AssertionError("reveal receipt paths must be unique and sorted")
    if any(isinstance(size, bool) or size < 1 for _, size, _ in ARTIFACTS):
        raise AssertionError("reveal receipt byte lengths must be positive integers")
    for _, _, digest in ARTIFACTS:
        heldout.require_hex(digest, 64, "reveal receipt SHA")
    print("default-head held-out reveal receipt self-test passed")


def parser() -> argparse.ArgumentParser:
    root = argparse.ArgumentParser()
    commands = root.add_subparsers(dest="command", required=True)
    validate_parser = commands.add_parser("validate")
    validate_parser.set_defaults(run=validate)
    self_parser = commands.add_parser("self-test")
    self_parser.set_defaults(run=self_test)
    return root


def main() -> None:
    args = parser().parse_args()
    try:
        args.run(args)
    except (KeyError, ValueError) as error:
        raise SystemExit(str(error)) from error


if __name__ == "__main__":
    main()
