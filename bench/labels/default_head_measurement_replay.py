#!/usr/bin/env python3
"""Create and validate the clean, exact-binary replay receipt for #846."""

from __future__ import annotations

import argparse
import datetime as dt
import hashlib
import json
import os
from pathlib import Path
import subprocess
import sys
import tempfile
from typing import Any


ROOT = Path(__file__).resolve().parents[2]
sys.path.insert(0, str(ROOT / "scripts"))
from binary_identity import binary_identity  # noqa: E402


DEFAULT = ROOT / "bench/labels/default_head_measurement_replay_2026_07_14.v1.json"
GENERATOR = "bench/labels/default_head_measurement_replay.py"
SOUNDNESS = "bench/recall_loss/issue-846-crates-verify-2026-07-14.v1.json"
HELDOUT = "bench/recall_loss/issue-846-heldout-thread-determinism-2026-07-14.v1.tsv"
SCALING = "bench/recall_loss/issue-846-ruby-scaling-2026-07-14.v1.json"
CORPUS = "bench/goldens/corpus.json"
CORPUS_STATE = "bench/default_head_closeout_corpus.v1.json"
CURRENT_SOURCE = "cdab416706c32ea94bf808ec7ebb36781e483e65"
CURRENT_SOURCE_TREE = "0f42757629a79ce7be0cd0cd5cd90c2d5b78c3da"
CURRENT_SHA = "f7fcda30aa63662f95000af7029eaf028c71ef074a18ba5e1e2048fe27c47fd0"
CURRENT_CODE_SHA = "03cc5827cdadc225478a34266de78805c6e495810f90e8642f2ae2807b3a4f5a"
CODE_ALGORITHM = "sha256/mach-o-zero-uuid-signature-v1"


def require(condition: bool, message: str) -> None:
    if not condition:
        raise ValueError(message)


def sha256_bytes(value: bytes) -> str:
    return hashlib.sha256(value).hexdigest()


def sha256(path: Path) -> str:
    return sha256_bytes(path.read_bytes())


def load(path: Path) -> dict[str, Any]:
    value = json.loads(path.read_text(encoding="utf-8"))
    require(isinstance(value, dict), f"{path}: expected object")
    return value


def git(*args: str) -> str:
    result = subprocess.run(
        ["git", *args], cwd=ROOT, capture_output=True, text=True
    )
    require(result.returncode == 0, f"git {' '.join(args)}: {result.stderr.strip()}")
    return result.stdout.strip()


def run(command: list[str], *, env: dict[str, str] | None = None) -> bytes:
    result = subprocess.run(command, cwd=ROOT, capture_output=True, env=env)
    require(
        result.returncode == 0,
        f"command failed ({result.returncode}): {' '.join(command)}\n"
        f"{result.stderr.decode(errors='replace')}",
    )
    return result.stdout


def parse_tsv(path: Path) -> dict[str, dict[str, str]]:
    rows: dict[str, dict[str, str]] = {}
    for number, line in enumerate(path.read_text(encoding="utf-8").splitlines(), 1):
        fields = line.split("\t")
        require(len(fields) == 4, f"{path}:{number}: malformed row")
        repository, one, four, verdict = fields
        require(repository not in rows, f"{path}:{number}: duplicate repository")
        require(one == four and verdict == "pass", f"{path}:{number}: failed row")
        rows[repository] = {
            "one_thread_sha256": one,
            "four_thread_sha256": four,
        }
    return rows


def corpus_rows() -> list[dict[str, Any]]:
    corpus = load(ROOT / CORPUS)
    rows = [row for row in corpus["repositories"] if row["split"] == "heldout"]
    require(len(rows) == 54, "heldout corpus must contain 54 repositories")
    return sorted(rows, key=lambda row: row["id"])


def validate_repository_state(rows: list[dict[str, Any]]) -> None:
    state = load(ROOT / CORPUS_STATE)
    state_commits = {row["repo"]: row["commit"] for row in state["repositories"]}
    for row in rows:
        repository = row["id"]
        expected = row["commit"]
        require(state_commits.get(repository) == expected, f"state mismatch: {repository}")
        actual = subprocess.run(
            ["git", "-C", str(ROOT / "bench/repos" / repository), "rev-parse", "HEAD"],
            capture_output=True,
            text=True,
        )
        require(actual.returncode == 0, f"missing corpus checkout: {repository}")
        require(actual.stdout.strip() == expected, f"checkout drift: {repository}")


def freeze(binary: Path, output: Path) -> None:
    output = output.resolve()
    require(not output.exists(), f"refusing to overwrite {output}")
    require(git("status", "--porcelain") == "", "replay must start from a clean tree")
    harness_commit = git("rev-parse", "HEAD")
    require(
        git("rev-parse", f"{harness_commit}:crates") == CURRENT_SOURCE_TREE,
        "replay checkout does not contain the frozen product tree",
    )
    require(
        subprocess.run(
            ["git", "merge-base", "--is-ancestor", CURRENT_SOURCE, harness_commit],
            cwd=ROOT,
        ).returncode
        == 0,
        "frozen product source is not replay history",
    )
    identity = binary_identity(binary.resolve())
    require(identity.file_sha256 == CURRENT_SHA, "wrong replay binary")
    require(identity.code_sha256 == CURRENT_CODE_SHA, "wrong replay code")
    require(identity.code_sha256_algorithm == CODE_ALGORITHM, "wrong code identity")

    rows = corpus_rows()
    validate_repository_state(rows)
    expected_heldout = parse_tsv(ROOT / HELDOUT)
    require(set(expected_heldout) == {row["id"] for row in rows}, "heldout TSV set drift")

    with tempfile.TemporaryDirectory(prefix="nose-846-measurement-replay-") as directory:
        temporary = Path(directory)
        soundness_output = temporary / "soundness.json"
        run(
            [
                str(binary.resolve()),
                "verify",
                "crates",
                "--max-violations",
                "0",
                "--recall-loss-report",
                str(soundness_output),
            ]
        )
        soundness_sha = sha256(soundness_output)
        expected_soundness_sha = sha256(ROOT / SOUNDNESS)
        require(soundness_sha == expected_soundness_sha, "soundness replay drift")

        replay_rows: list[dict[str, str]] = []
        for row in rows:
            repository = row["id"]
            hashes: dict[int, str] = {}
            for threads in (1, 4):
                environment = dict(os.environ)
                environment["RAYON_NUM_THREADS"] = str(threads)
                stdout = run(
                    [
                        str(binary.resolve()),
                        "query",
                        f"bench/repos/{repository}",
                        "all",
                        "top=0",
                        "--format",
                        "json",
                    ],
                    env=environment,
                )
                hashes[threads] = sha256_bytes(stdout)
            require(hashes[1] == hashes[4], f"thread drift: {repository}")
            require(
                hashes[1] == expected_heldout[repository]["one_thread_sha256"],
                f"checked heldout drift: {repository}",
            )
            replay_rows.append(
                {
                    "repository": repository,
                    "commit": row["commit"],
                    "one_thread_sha256": hashes[1],
                    "four_thread_sha256": hashes[4],
                }
            )

        scaling_output = temporary / "ruby-scaling.json"
        run(
            [
                sys.executable,
                "scripts/ruby-redefinition-scaling.py",
                "--binary",
                str(binary.resolve()),
                "--output",
                str(scaling_output),
            ]
        )
        scaling = load(scaling_output)
        historical_scaling = load(ROOT / SCALING)
        require(
            scaling["fixture_sha256_by_case_count"]
            == historical_scaling["fixture_sha256_by_case_count"],
            "Ruby scaling fixtures changed",
        )
        require(scaling["evaluation"]["status"] == "within-threshold", "Ruby scaling failed")

        require(
            git("status", "--porcelain") == "",
            "replay modified the tree before receipt creation",
        )
        receipt = {
            "schema": "nose.default_head_measurement_replay.v1",
            "issue": 846,
            "tracker": 838,
            "created_at_utc": dt.datetime.now(dt.timezone.utc).isoformat(),
            "generator": {
                "path": GENERATOR,
                "sha256": sha256(ROOT / GENERATOR),
                "commit": harness_commit,
            },
            "replay_context": {
                "working_tree_status_before_measurement": "",
                "working_tree_status_after_measurement_before_receipt": "",
                "product_source_commit": CURRENT_SOURCE,
                "product_source_tree_sha1": CURRENT_SOURCE_TREE,
                "binary_sha256": identity.file_sha256,
                "binary_code_sha256": identity.code_sha256,
                "binary_code_sha256_algorithm": identity.code_sha256_algorithm,
                "corpus_manifest": CORPUS,
                "corpus_manifest_sha256": sha256(ROOT / CORPUS),
                "corpus_state": CORPUS_STATE,
                "corpus_state_sha256": sha256(ROOT / CORPUS_STATE),
            },
            "soundness": {
                "command": "nose verify crates --max-violations 0 --recall-loss-report <temporary>",
                "checked_artifact": SOUNDNESS,
                "checked_artifact_sha256": expected_soundness_sha,
                "replay_output_sha256": soundness_sha,
                "byte_identical": True,
            },
            "heldout_thread_determinism": {
                "command": "RAYON_NUM_THREADS={1,4} nose query bench/repos/<repo> all top=0 --format json",
                "checked_artifact": HELDOUT,
                "checked_artifact_sha256": sha256(ROOT / HELDOUT),
                "repositories": replay_rows,
                "all_thread_and_checked_hashes_identical": True,
            },
            "ruby_redefinition_scaling": {
                "command": "python3 scripts/ruby-redefinition-scaling.py --binary <nose> --output <temporary>",
                "checked_artifact": SCALING,
                "checked_artifact_sha256": sha256(ROOT / SCALING),
                "replay_output_sha256": sha256(scaling_output),
                "fixture_sha256_by_case_count": scaling["fixture_sha256_by_case_count"],
                "evaluation": scaling["evaluation"],
            },
        }

    output.parent.mkdir(parents=True, exist_ok=True)
    payload = (json.dumps(receipt, indent=2, sort_keys=True) + "\n").encode()
    descriptor = os.open(output, os.O_WRONLY | os.O_CREAT | os.O_EXCL, 0o644)
    with os.fdopen(descriptor, "wb") as handle:
        handle.write(payload)


def validate(path: Path) -> None:
    value = load(path)
    require(value.get("schema") == "nose.default_head_measurement_replay.v1", "wrong schema")
    require(value.get("issue") == 846 and value.get("tracker") == 838, "wrong issue")
    generator = value.get("generator", {})
    require(generator.get("path") == GENERATOR, "wrong generator path")
    commit = generator.get("commit", "")
    require(git("cat-file", "-t", commit) == "commit", "missing generator commit")
    require(
        subprocess.run(
            ["git", "merge-base", "--is-ancestor", commit, "HEAD"], cwd=ROOT
        ).returncode
        == 0,
        "generator commit is not replay history",
    )
    generator_bytes = subprocess.run(
        ["git", "show", f"{commit}:{GENERATOR}"], cwd=ROOT, capture_output=True
    )
    require(generator_bytes.returncode == 0, "generator absent from recorded commit")
    require(
        sha256_bytes(generator_bytes.stdout) == generator.get("sha256"),
        "generator hash changed",
    )
    context = value.get("replay_context", {})
    require(context.get("working_tree_status_before_measurement") == "", "dirty replay start")
    require(
        context.get("working_tree_status_after_measurement_before_receipt") == "",
        "dirty replay finish",
    )
    require(context.get("product_source_commit") == CURRENT_SOURCE, "wrong product source")
    require(context.get("product_source_tree_sha1") == CURRENT_SOURCE_TREE, "wrong product tree")
    require(context.get("binary_sha256") == CURRENT_SHA, "wrong binary")
    require(context.get("binary_code_sha256") == CURRENT_CODE_SHA, "wrong code")
    require(context.get("binary_code_sha256_algorithm") == CODE_ALGORITHM, "wrong code hash")
    require(context.get("corpus_manifest_sha256") == sha256(ROOT / CORPUS), "wrong corpus")
    require(context.get("corpus_state_sha256") == sha256(ROOT / CORPUS_STATE), "wrong state")

    soundness = value.get("soundness", {})
    require(soundness.get("checked_artifact") == SOUNDNESS, "wrong soundness artifact")
    require(soundness.get("checked_artifact_sha256") == sha256(ROOT / SOUNDNESS), "soundness hash drift")
    require(soundness.get("replay_output_sha256") == soundness.get("checked_artifact_sha256"), "soundness replay mismatch")
    require(soundness.get("byte_identical") is True, "soundness replay not identical")

    heldout = value.get("heldout_thread_determinism", {})
    require(heldout.get("checked_artifact") == HELDOUT, "wrong heldout artifact")
    require(heldout.get("checked_artifact_sha256") == sha256(ROOT / HELDOUT), "heldout hash drift")
    expected = parse_tsv(ROOT / HELDOUT)
    rows = heldout.get("repositories", [])
    require(len(rows) == 54, "wrong replay repository count")
    observed = {row["repository"]: row for row in rows}
    require(set(observed) == set(expected), "wrong replay repository set")
    for repository, hashes in expected.items():
        row = observed[repository]
        require(row["one_thread_sha256"] == hashes["one_thread_sha256"], f"one-thread drift: {repository}")
        require(row["four_thread_sha256"] == hashes["four_thread_sha256"], f"four-thread drift: {repository}")
    require(heldout.get("all_thread_and_checked_hashes_identical") is True, "heldout replay failed")

    scaling = value.get("ruby_redefinition_scaling", {})
    require(scaling.get("checked_artifact") == SCALING, "wrong scaling artifact")
    historical = load(ROOT / SCALING)
    require(scaling.get("checked_artifact_sha256") == sha256(ROOT / SCALING), "scaling hash drift")
    require(scaling.get("fixture_sha256_by_case_count") == historical["fixture_sha256_by_case_count"], "scaling fixture drift")
    require(scaling.get("evaluation", {}).get("status") == "within-threshold", "scaling replay failed")


def self_test() -> None:
    with tempfile.TemporaryDirectory() as directory:
        valid = Path(directory) / "valid.tsv"
        valid.write_text("repo\t" + "a" * 64 + "\t" + "a" * 64 + "\tpass\n")
        assert parse_tsv(valid)["repo"]["one_thread_sha256"] == "a" * 64
        invalid = Path(directory) / "invalid.tsv"
        invalid.write_text("repo\t" + "a" * 64 + "\t" + "b" * 64 + "\tpass\n")
        try:
            parse_tsv(invalid)
        except ValueError:
            pass
        else:
            raise AssertionError("mismatched determinism hashes were accepted")


def main() -> None:
    parser = argparse.ArgumentParser()
    subparsers = parser.add_subparsers(dest="command", required=True)
    freeze_parser = subparsers.add_parser("freeze")
    freeze_parser.add_argument("--nose", type=Path, required=True)
    freeze_parser.add_argument("--output", type=Path, default=DEFAULT)
    validate_parser = subparsers.add_parser("validate")
    validate_parser.add_argument("path", nargs="?", type=Path, default=DEFAULT)
    subparsers.add_parser("self-test")
    args = parser.parse_args()
    if args.command == "freeze":
        freeze(args.nose, args.output)
        print(f"wrote {args.output}")
    elif args.command == "validate":
        validate(args.path)
        print(f"validated {args.path}")
    else:
        self_test()
        print("default-head measurement replay self-test passed")


if __name__ == "__main__":
    main()
