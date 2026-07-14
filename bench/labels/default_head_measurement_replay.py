#!/usr/bin/env python3
"""Create and validate the clean, exact-binary replay receipt for #846."""

from __future__ import annotations

import argparse
import copy
import datetime as dt
import hashlib
import json
import math
import os
from pathlib import Path
import statistics
import subprocess
import sys
import tempfile
from typing import Any


ROOT = Path(__file__).resolve().parents[2]
sys.path.insert(0, str(ROOT))
sys.path.insert(0, str(ROOT / "scripts"))
from bench.corpus_prune.core import corpus_digest  # noqa: E402
from binary_identity import binary_identity  # noqa: E402


DEFAULT_RELATIVE = "bench/labels/default_head_measurement_replay_2026_07_14.v2.json"
DEFAULT = ROOT / DEFAULT_RELATIVE
SIDECAR = Path(f"{DEFAULT}.sha256")
GENERATOR = "bench/labels/default_head_measurement_replay.py"
SOUNDNESS = "bench/recall_loss/issue-846-crates-verify-2026-07-14.v1.json"
HELDOUT = "bench/recall_loss/issue-846-heldout-thread-determinism-2026-07-14.v1.tsv"
SCALING = "bench/recall_loss/issue-846-ruby-scaling-2026-07-14.v1.json"
CORPUS = "bench/goldens/corpus.json"
CORPUS_STATE = "bench/default_head_closeout_corpus.v1.json"
RUNTIME_CORPUS_STATE = "bench/repos/.nose-corpus-state.json"
PRUNE_MANIFEST = "bench/labels/prune_manifest.json"
CURRENT_SOURCE = "cdab416706c32ea94bf808ec7ebb36781e483e65"
CURRENT_SOURCE_TREE = "0f42757629a79ce7be0cd0cd5cd90c2d5b78c3da"
CURRENT_SHA = "f7fcda30aa63662f95000af7029eaf028c71ef074a18ba5e1e2048fe27c47fd0"
CURRENT_CODE_SHA = "03cc5827cdadc225478a34266de78805c6e495810f90e8642f2ae2807b3a4f5a"
CODE_ALGORITHM = "sha256/mach-o-zero-uuid-signature-v1"
SOUNDNESS_COMMAND = (
    "nose verify crates --max-violations 0 --recall-loss-report <temporary>"
)
HELDOUT_COMMAND = (
    "RAYON_NUM_THREADS={1,4} nose query bench/repos/<repo> all top=0 --format json"
)
SCALING_COMMAND = (
    "python3 scripts/ruby-redefinition-scaling.py --binary <nose> "
    "--output <temporary>"
)
RUBY_SCHEMA = "nose.ruby_redefinition_scaling.v1"
RUBY_CASE_COUNTS = (64, 256)
RUBY_ITERATIONS = 5
RUBY_MAX_EXPONENT = 1.35
RUBY_MIN_DELTA_MS = 5.0


def require(condition: bool, message: str) -> None:
    if not condition:
        raise ValueError(message)


def require_exact_keys(value: Any, keys: set[str], label: str) -> dict[str, Any]:
    require(isinstance(value, dict), f"{label}: expected object")
    require(set(value) == keys, f"{label}: keys changed")
    return value


def require_sha(value: Any, label: str) -> str:
    require(
        isinstance(value, str)
        and len(value) == 64
        and all(character in "0123456789abcdef" for character in value),
        f"{label}: expected lowercase SHA-256",
    )
    return value


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
        require_sha(one, f"{path}:{number}: one-thread hash")
        require_sha(four, f"{path}:{number}: four-thread hash")
        require(one == four and verdict == "pass", f"{path}:{number}: failed row")
        rows[repository] = {
            "one_thread_sha256": one,
            "four_thread_sha256": four,
        }
    return rows


def all_corpus_rows() -> list[dict[str, Any]]:
    corpus = load(ROOT / CORPUS)
    rows = corpus.get("repositories")
    require(isinstance(rows, list) and len(rows) == 120, "corpus must contain 120 repositories")
    require(all(isinstance(row, dict) for row in rows), "malformed corpus repository")
    identifiers = [row.get("id") for row in rows]
    require(
        all(isinstance(identifier, str) and identifier for identifier in identifiers),
        "malformed corpus repository id",
    )
    require(len(set(identifiers)) == len(identifiers), "duplicate corpus repository id")
    return sorted(rows, key=lambda row: row["id"])


def heldout_corpus_rows() -> list[dict[str, Any]]:
    rows = [row for row in all_corpus_rows() if row.get("split") == "heldout"]
    require(len(rows) == 54, "heldout corpus must contain 54 repositories")
    return rows


def validate_corpus_digest(actual: Any, expected: Any) -> None:
    keys = {"algorithm", "hex", "files", "bytes"}
    actual = require_exact_keys(actual, keys, "actual corpus digest")
    expected = require_exact_keys(expected, keys, "expected corpus digest")
    require_sha(actual["hex"], "actual corpus digest")
    require_sha(expected["hex"], "expected corpus digest")
    for name in ("files", "bytes"):
        require(type(actual[name]) is int and actual[name] > 0, f"actual corpus {name}")
        require(type(expected[name]) is int and expected[name] > 0, f"expected corpus {name}")
    require(actual == expected, "corpus bytes differ from the frozen post-prune digest")


def validate_runtime_corpus_state(
    value: Any, expected_ids: set[str], expected_digest: Any
) -> None:
    runtime_state = require_exact_keys(
        value,
        {
            "schema",
            "manifest",
            "manifest_sha256",
            "repositories",
            "subset_digest_after_prune",
        },
        "runtime corpus state",
    )
    require(
        runtime_state["schema"] == "nose.pinned_corpus_subset.v1",
        "wrong runtime corpus state schema",
    )
    require(
        runtime_state["manifest"] == PRUNE_MANIFEST,
        "wrong runtime prune manifest path",
    )
    require(
        runtime_state["manifest_sha256"] == sha256(ROOT / PRUNE_MANIFEST),
        "wrong runtime prune manifest",
    )
    repositories = runtime_state["repositories"]
    require(
        isinstance(repositories, list)
        and len(repositories) == len(expected_ids)
        and all(isinstance(repository, str) for repository in repositories),
        "malformed runtime corpus repository set",
    )
    require(set(repositories) == expected_ids, "runtime corpus repository set drift")
    validate_corpus_digest(runtime_state["subset_digest_after_prune"], expected_digest)


def validate_repository_state(rows: list[dict[str, Any]]) -> dict[str, Any]:
    state = load(ROOT / CORPUS_STATE)
    state_repositories = state.get("repositories")
    require(isinstance(state_repositories, list), "corpus state repositories missing")
    expected_ids = {row["id"] for row in rows}
    require(set(state_repositories) == expected_ids, "corpus state repository set drift")
    runtime_state = load(ROOT / RUNTIME_CORPUS_STATE)
    for row in rows:
        repository = row["id"]
        expected = row.get("commit")
        require(isinstance(expected, str) and len(expected) == 40, f"bad commit: {repository}")
        actual = subprocess.run(
            ["git", "-C", str(ROOT / "bench/repos" / repository), "rev-parse", "HEAD"],
            capture_output=True,
            text=True,
        )
        require(actual.returncode == 0, f"missing corpus checkout: {repository}")
        require(actual.stdout.strip() == expected, f"checkout drift: {repository}")

    expected_digest = state.get("subset_digest_after_prune")
    validate_runtime_corpus_state(runtime_state, expected_ids, expected_digest)
    actual_digest = corpus_digest(
        ROOT / "bench/repos", {ROOT / RUNTIME_CORPUS_STATE}
    )
    validate_corpus_digest(actual_digest, expected_digest)
    return actual_digest


def evaluate_scaling(medians_ms: dict[int, float]) -> dict[str, Any]:
    small, large = RUBY_CASE_COUNTS
    small_ms = medians_ms[small]
    large_ms = medians_ms[large]
    exponent = math.log(large_ms / small_ms) / math.log(large / small)
    delta_ms = large_ms - small_ms
    material = delta_ms > RUBY_MIN_DELTA_MS
    regression = material and exponent > RUBY_MAX_EXPONENT
    return {
        "delta_ms": delta_ms,
        "growth_exponent": exponent,
        "material": material,
        "max_growth_exponent": RUBY_MAX_EXPONENT,
        "min_material_delta_ms": RUBY_MIN_DELTA_MS,
        "status": "regression" if regression else "within-threshold",
    }


def write_exclusive(path: Path, payload: bytes) -> None:
    descriptor = os.open(path, os.O_WRONLY | os.O_CREAT | os.O_EXCL, 0o644)
    with os.fdopen(descriptor, "wb") as handle:
        handle.write(payload)


def freeze(binary: Path, output: Path) -> None:
    output = output.resolve()
    sidecar = Path(f"{output}.sha256")
    require(not output.exists(), f"refusing to overwrite {output}")
    require(not sidecar.exists(), f"refusing to overwrite {sidecar}")
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

    all_rows = all_corpus_rows()
    corpus_state_digest = validate_repository_state(all_rows)
    rows = [row for row in all_rows if row.get("split") == "heldout"]
    require(len(rows) == 54, "heldout corpus must contain 54 repositories")
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
            "schema": "nose.default_head_measurement_replay.v2",
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
                "runtime_corpus_state": RUNTIME_CORPUS_STATE,
                "runtime_corpus_state_sha256": sha256(ROOT / RUNTIME_CORPUS_STATE),
                "runtime_corpus_state_payload": load(ROOT / RUNTIME_CORPUS_STATE),
                "corpus_subset_digest_after_prune": corpus_state_digest,
            },
            "soundness": {
                "command": SOUNDNESS_COMMAND,
                "checked_artifact": SOUNDNESS,
                "checked_artifact_sha256": expected_soundness_sha,
                "replay_output_sha256": soundness_sha,
                "byte_identical": True,
            },
            "heldout_thread_determinism": {
                "command": HELDOUT_COMMAND,
                "checked_artifact": HELDOUT,
                "checked_artifact_sha256": sha256(ROOT / HELDOUT),
                "repositories": replay_rows,
                "all_thread_and_checked_hashes_identical": True,
            },
            "ruby_redefinition_scaling": {
                "command": SCALING_COMMAND,
                "checked_artifact": SCALING,
                "checked_artifact_sha256": sha256(ROOT / SCALING),
                "replay_output_sha256": sha256(scaling_output),
                "replay_report": scaling,
            },
        }

    output.parent.mkdir(parents=True, exist_ok=True)
    payload = (json.dumps(receipt, indent=2, sort_keys=True) + "\n").encode()
    receipt_sha = sha256_bytes(payload)
    write_exclusive(output, payload)
    write_exclusive(sidecar, f"{receipt_sha}  {output.name}\n".encode())


def validate_generator(value: Any) -> None:
    generator = require_exact_keys(
        value, {"path", "sha256", "commit"}, "generator"
    )
    require(generator["path"] == GENERATOR, "wrong generator path")
    require_sha(generator["sha256"], "generator hash")
    commit = generator["commit"]
    require(isinstance(commit, str) and len(commit) == 40, "wrong generator commit")
    require(git("cat-file", "-t", commit) == "commit", "missing generator commit")
    require(
        subprocess.run(
            ["git", "merge-base", "--is-ancestor", commit, "HEAD"], cwd=ROOT
        ).returncode
        == 0,
        "generator commit is not replay history",
    )
    require(
        git("rev-parse", f"{commit}:crates") == CURRENT_SOURCE_TREE,
        "generator commit has the wrong product tree",
    )
    generator_bytes = subprocess.run(
        ["git", "show", f"{commit}:{GENERATOR}"], cwd=ROOT, capture_output=True
    )
    require(generator_bytes.returncode == 0, "generator absent from recorded commit")
    require(
        sha256_bytes(generator_bytes.stdout) == generator["sha256"],
        "generator hash changed",
    )


def validate_context(value: Any) -> None:
    context = require_exact_keys(
        value,
        {
            "working_tree_status_before_measurement",
            "working_tree_status_after_measurement_before_receipt",
            "product_source_commit",
            "product_source_tree_sha1",
            "binary_sha256",
            "binary_code_sha256",
            "binary_code_sha256_algorithm",
            "corpus_manifest",
            "corpus_manifest_sha256",
            "corpus_state",
            "corpus_state_sha256",
            "runtime_corpus_state",
            "runtime_corpus_state_sha256",
            "runtime_corpus_state_payload",
            "corpus_subset_digest_after_prune",
        },
        "replay context",
    )
    require(context["working_tree_status_before_measurement"] == "", "dirty replay start")
    require(
        context["working_tree_status_after_measurement_before_receipt"] == "",
        "dirty replay finish",
    )
    require(context["product_source_commit"] == CURRENT_SOURCE, "wrong product source")
    require(context["product_source_tree_sha1"] == CURRENT_SOURCE_TREE, "wrong product tree")
    require(context["binary_sha256"] == CURRENT_SHA, "wrong binary")
    require(context["binary_code_sha256"] == CURRENT_CODE_SHA, "wrong code")
    require(context["binary_code_sha256_algorithm"] == CODE_ALGORITHM, "wrong code hash")
    require(context["corpus_manifest"] == CORPUS, "wrong corpus manifest path")
    require(context["corpus_manifest_sha256"] == sha256(ROOT / CORPUS), "wrong corpus")
    require(context["corpus_state"] == CORPUS_STATE, "wrong corpus state path")
    require(context["corpus_state_sha256"] == sha256(ROOT / CORPUS_STATE), "wrong state")
    require(
        context["runtime_corpus_state"] == RUNTIME_CORPUS_STATE,
        "wrong runtime corpus state path",
    )
    require(
        context["runtime_corpus_state_sha256"]
        == sha256_bytes(
            (
                json.dumps(
                    context["runtime_corpus_state_payload"], indent=2, sort_keys=True
                )
                + "\n"
            ).encode()
        ),
        "runtime corpus state payload hash changed",
    )
    state = load(ROOT / CORPUS_STATE)
    validate_runtime_corpus_state(
        context["runtime_corpus_state_payload"],
        {row["id"] for row in all_corpus_rows()},
        state.get("subset_digest_after_prune"),
    )
    validate_corpus_digest(
        context["corpus_subset_digest_after_prune"],
        state.get("subset_digest_after_prune"),
    )


def validate_soundness(value: Any) -> None:
    soundness = require_exact_keys(
        value,
        {
            "command",
            "checked_artifact",
            "checked_artifact_sha256",
            "replay_output_sha256",
            "byte_identical",
        },
        "soundness replay",
    )
    require(soundness["command"] == SOUNDNESS_COMMAND, "wrong soundness command")
    require(soundness["checked_artifact"] == SOUNDNESS, "wrong soundness artifact")
    require(
        soundness["checked_artifact_sha256"] == sha256(ROOT / SOUNDNESS),
        "soundness hash drift",
    )
    require_sha(soundness["replay_output_sha256"], "soundness replay hash")
    require(
        soundness["replay_output_sha256"] == soundness["checked_artifact_sha256"],
        "soundness replay mismatch",
    )
    require(soundness["byte_identical"] is True, "soundness replay not identical")


def validate_heldout(value: Any) -> None:
    heldout = require_exact_keys(
        value,
        {
            "command",
            "checked_artifact",
            "checked_artifact_sha256",
            "repositories",
            "all_thread_and_checked_hashes_identical",
        },
        "heldout replay",
    )
    require(heldout["command"] == HELDOUT_COMMAND, "wrong heldout command")
    require(heldout["checked_artifact"] == HELDOUT, "wrong heldout artifact")
    require(
        heldout["checked_artifact_sha256"] == sha256(ROOT / HELDOUT),
        "heldout hash drift",
    )
    expected_hashes = parse_tsv(ROOT / HELDOUT)
    expected_rows = {row["id"]: row for row in heldout_corpus_rows()}
    require(set(expected_rows) == set(expected_hashes), "heldout inputs disagree")
    rows = heldout["repositories"]
    require(isinstance(rows, list) and len(rows) == 54, "wrong replay repository count")
    observed: dict[str, dict[str, Any]] = {}
    for index, raw_row in enumerate(rows):
        row = require_exact_keys(
            raw_row,
            {"repository", "commit", "one_thread_sha256", "four_thread_sha256"},
            f"heldout replay row {index}",
        )
        repository = row["repository"]
        require(isinstance(repository, str), f"heldout replay row {index}: bad repository")
        require(repository not in observed, f"duplicate replay repository: {repository}")
        require(repository in expected_rows, f"unknown replay repository: {repository}")
        require(
            row["commit"] == expected_rows[repository]["commit"],
            f"commit drift: {repository}",
        )
        for name in ("one_thread_sha256", "four_thread_sha256"):
            require_sha(row[name], f"{repository}: {name}")
            require(
                row[name] == expected_hashes[repository][name],
                f"{name} drift: {repository}",
            )
        require(
            row["one_thread_sha256"] == row["four_thread_sha256"],
            f"thread drift: {repository}",
        )
        observed[repository] = row
    require(set(observed) == set(expected_rows), "wrong replay repository set")
    require(
        heldout["all_thread_and_checked_hashes_identical"] is True,
        "heldout replay failed",
    )


def validate_scaling_report(report: Any) -> None:
    report = require_exact_keys(
        report,
        {
            "schema",
            "binary",
            "binary_sha256",
            "fixture_sha256_by_case_count",
            "iterations",
            "medians_ms",
            "runs",
            "evaluation",
        },
        "Ruby scaling report",
    )
    require(report["schema"] == RUBY_SCHEMA, "wrong Ruby scaling schema")
    binary = report["binary"]
    require(
        isinstance(binary, str) and Path(binary).is_absolute() and Path(binary).name == "nose",
        "wrong Ruby scaling binary path",
    )
    require(report["binary_sha256"] == CURRENT_SHA, "wrong Ruby scaling binary")
    historical = load(ROOT / SCALING)
    require(
        report["fixture_sha256_by_case_count"]
        == historical["fixture_sha256_by_case_count"],
        "scaling fixture drift",
    )
    require(type(report["iterations"]) is int and report["iterations"] == RUBY_ITERATIONS, "wrong scaling iterations")

    medians = require_exact_keys(
        report["medians_ms"], {"64", "256"}, "Ruby scaling medians"
    )
    numeric_medians: dict[int, float] = {}
    for case_count in RUBY_CASE_COUNTS:
        value = medians[str(case_count)]
        require(
            isinstance(value, (int, float))
            and not isinstance(value, bool)
            and math.isfinite(value)
            and value > 0,
            f"invalid Ruby median: {case_count}",
        )
        numeric_medians[case_count] = float(value)

    runs = report["runs"]
    require(isinstance(runs, list) and len(runs) == RUBY_ITERATIONS * 2, "wrong Ruby run count")
    by_case: dict[int, list[float]] = {case_count: [] for case_count in RUBY_CASE_COUNTS}
    expected_order: list[tuple[int, int]] = []
    for iteration in range(1, RUBY_ITERATIONS + 1):
        order = RUBY_CASE_COUNTS if iteration % 2 else tuple(reversed(RUBY_CASE_COUNTS))
        expected_order.extend((iteration, case_count) for case_count in order)
    for index, (raw_row, expected) in enumerate(zip(runs, expected_order, strict=True)):
        row = require_exact_keys(
            raw_row, {"case_count", "elapsed_ms", "iteration"}, f"Ruby run {index}"
        )
        require(
            (row["iteration"], row["case_count"]) == expected,
            f"Ruby run order changed at {index}",
        )
        elapsed = row["elapsed_ms"]
        require(
            isinstance(elapsed, (int, float))
            and not isinstance(elapsed, bool)
            and math.isfinite(elapsed)
            and elapsed > 0,
            f"invalid Ruby elapsed time at {index}",
        )
        by_case[row["case_count"]].append(float(elapsed))
    for case_count in RUBY_CASE_COUNTS:
        observed_median = statistics.median(by_case[case_count])
        require(
            math.isclose(
                numeric_medians[case_count], observed_median, rel_tol=1e-12, abs_tol=1e-9
            ),
            f"Ruby median does not match runs: {case_count}",
        )

    evaluation = require_exact_keys(
        report["evaluation"],
        {
            "delta_ms",
            "growth_exponent",
            "material",
            "max_growth_exponent",
            "min_material_delta_ms",
            "status",
        },
        "Ruby scaling evaluation",
    )
    require(evaluation["max_growth_exponent"] == RUBY_MAX_EXPONENT, "wrong Ruby threshold")
    require(evaluation["min_material_delta_ms"] == RUBY_MIN_DELTA_MS, "wrong Ruby material threshold")
    expected_evaluation = evaluate_scaling(numeric_medians)
    for name in ("delta_ms", "growth_exponent"):
        actual = evaluation[name]
        require(
            isinstance(actual, (int, float))
            and not isinstance(actual, bool)
            and math.isfinite(actual),
            f"invalid Ruby evaluation: {name}",
        )
        require(
            math.isclose(float(actual), expected_evaluation[name], rel_tol=1e-12, abs_tol=1e-9),
            f"Ruby evaluation mismatch: {name}",
        )
    require(evaluation["material"] is expected_evaluation["material"], "wrong Ruby material verdict")
    require(evaluation["status"] == expected_evaluation["status"], "wrong Ruby status")
    require(evaluation["status"] == "within-threshold", "Ruby scaling replay failed")


def validate_scaling(value: Any) -> None:
    scaling = require_exact_keys(
        value,
        {
            "command",
            "checked_artifact",
            "checked_artifact_sha256",
            "replay_output_sha256",
            "replay_report",
        },
        "Ruby scaling replay",
    )
    require(scaling["command"] == SCALING_COMMAND, "wrong scaling command")
    require(scaling["checked_artifact"] == SCALING, "wrong scaling artifact")
    require(
        scaling["checked_artifact_sha256"] == sha256(ROOT / SCALING),
        "scaling hash drift",
    )
    replay_sha = require_sha(scaling["replay_output_sha256"], "scaling replay hash")
    validate_scaling_report(scaling["replay_report"])
    replay_payload = (
        json.dumps(scaling["replay_report"], indent=2, sort_keys=True) + "\n"
    ).encode()
    require(sha256_bytes(replay_payload) == replay_sha, "scaling replay digest mismatch")


def validate_payload(value: dict[str, Any]) -> None:
    require_exact_keys(
        value,
        {
            "schema",
            "issue",
            "tracker",
            "created_at_utc",
            "generator",
            "replay_context",
            "soundness",
            "heldout_thread_determinism",
            "ruby_redefinition_scaling",
        },
        "measurement replay receipt",
    )
    require(value["schema"] == "nose.default_head_measurement_replay.v2", "wrong schema")
    require(value["issue"] == 846 and value["tracker"] == 838, "wrong issue")
    created = value["created_at_utc"]
    require(isinstance(created, str), "wrong receipt timestamp")
    try:
        timestamp = dt.datetime.fromisoformat(created)
    except ValueError as error:
        raise ValueError("wrong receipt timestamp") from error
    require(timestamp.tzinfo is not None and timestamp.utcoffset() == dt.timedelta(0), "receipt timestamp is not UTC")
    validate_generator(value["generator"])
    validate_context(value["replay_context"])
    validate_soundness(value["soundness"])
    validate_heldout(value["heldout_thread_determinism"])
    validate_scaling(value["ruby_redefinition_scaling"])


def validate(path: Path, *, expected_sha256: str | None = None) -> None:
    path = path.resolve()
    if path == DEFAULT.resolve():
        expected, name = SIDECAR.read_text(encoding="utf-8").strip().split()
        require(name == DEFAULT.name, "measurement replay sidecar filename changed")
        require_sha(expected, "measurement replay sidecar hash")
        require(sha256(DEFAULT) == expected, "measurement replay sidecar hash changed")
        if expected_sha256 is not None:
            require(expected_sha256 == expected, "measurement replay pinned hash changed")
    elif expected_sha256 is not None:
        require(sha256(path) == expected_sha256, "measurement replay pinned hash changed")
    validate_payload(load(path))


def set_nested(value: dict[str, Any], path: tuple[str | int, ...], replacement: Any) -> None:
    current: Any = value
    for part in path[:-1]:
        current = current[part]
    current[path[-1]] = replacement


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

    expected_digest = load(ROOT / CORPUS_STATE)["subset_digest_after_prune"]
    changed_digest = copy.deepcopy(expected_digest)
    changed_digest["hex"] = "0" * 64
    try:
        validate_corpus_digest(changed_digest, expected_digest)
    except ValueError:
        pass
    else:
        raise AssertionError("modified corpus digest was accepted")

    validate(DEFAULT)
    original = load(DEFAULT)
    mutations: list[tuple[str, tuple[str | int, ...], Any]] = [
        ("corpus path", ("replay_context", "corpus_manifest"), "/tmp/corpus.json"),
        (
            "corpus digest",
            ("replay_context", "corpus_subset_digest_after_prune", "hex"),
            "0" * 64,
        ),
        (
            "runtime corpus state",
            ("replay_context", "runtime_corpus_state_payload", "repositories", 0),
            "wrong-repository",
        ),
        ("soundness command", ("soundness", "command"), "true"),
        ("heldout command", ("heldout_thread_determinism", "command"), "true"),
        (
            "heldout commit",
            ("heldout_thread_determinism", "repositories", 0, "commit"),
            "0" * 40,
        ),
        ("scaling command", ("ruby_redefinition_scaling", "command"), "true"),
        (
            "scaling replay digest",
            ("ruby_redefinition_scaling", "replay_output_sha256"),
            "0" * 64,
        ),
        (
            "scaling exponent",
            ("ruby_redefinition_scaling", "replay_report", "evaluation", "growth_exponent"),
            99.0,
        ),
        (
            "scaling threshold",
            ("ruby_redefinition_scaling", "replay_report", "evaluation", "max_growth_exponent"),
            0.1,
        ),
        (
            "scaling delta",
            ("ruby_redefinition_scaling", "replay_report", "evaluation", "delta_ms"),
            -123.0,
        ),
        (
            "scaling median",
            ("ruby_redefinition_scaling", "replay_report", "medians_ms", "256"),
            1.0,
        ),
    ]
    for label, path, replacement in mutations:
        changed = copy.deepcopy(original)
        set_nested(changed, path, replacement)
        try:
            validate_payload(changed)
        except ValueError:
            continue
        raise AssertionError(f"forged receipt field was accepted: {label}")


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
