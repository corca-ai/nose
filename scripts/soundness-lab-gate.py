#!/usr/bin/env python3
"""Compose existing Soundness Lab evidence into CI, nightly, and release gates."""

from __future__ import annotations

import argparse
import copy
import hashlib
import json
import re
import subprocess
import sys
from collections import Counter
from pathlib import Path
from typing import Any

try:
    import tomllib
except ModuleNotFoundError:  # pragma: no cover - CI and supported local Python provide it.
    tomllib = None  # type: ignore[assignment]

ROOT = Path(__file__).resolve().parents[1]
CORPUS = ROOT / "bench/goldens/corpus.json"
BASELINE = ROOT / "bench/soundness/0.19.0"
CURRENT = ROOT / "bench/soundness/0.20.0"
TYPE4 = ROOT / "bench/type4"
FORMAL = ROOT / "formal/obligations"
WORKFLOW = ROOT / ".github/workflows/corpus-verify.yml"
ADVISORY_BASELINE = CURRENT / "nightly-advisory-baseline.v1.json"


class GateError(ValueError):
    pass


def load(path: Path) -> dict[str, Any]:
    try:
        value = json.loads(path.read_text())
    except FileNotFoundError as error:
        raise GateError(f"required artifact is missing: {path.relative_to(ROOT)}") from error
    except json.JSONDecodeError as error:
        raise GateError(f"invalid JSON artifact {path.relative_to(ROOT)}: {error}") from error
    if not isinstance(value, dict):
        raise GateError(f"artifact root must be an object: {path.relative_to(ROOT)}")
    return value


def canonical(value: Any) -> bytes:
    return (json.dumps(value, indent=2, sort_keys=True) + "\n").encode()


def sha256(path: Path) -> str:
    return hashlib.sha256(path.read_bytes()).hexdigest()


def corpus_rows() -> list[dict[str, Any]]:
    rows = load(CORPUS).get("repositories")
    if not isinstance(rows, list) or not rows:
        raise GateError("pinned corpus manifest has no repositories")
    return sorted(rows, key=lambda row: row["id"])


def formal_coverage() -> dict[str, Any]:
    if tomllib is None:
        raise GateError("Python tomllib is required to read formal claim metadata")
    theorem: Counter[str] = Counter()
    preconditions: Counter[str] = Counter()
    surfaces: Counter[str] = Counter()
    claims: dict[str, set[str]] = {}
    claim_files: dict[str, set[str]] = {}
    obligations: list[dict[str, Any]] = []
    for meta_path in sorted(FORMAL.rglob("meta.toml")):
        meta = tomllib.loads(meta_path.read_text())
        claim = meta.get("claim", {}).get("id")
        obligation_id = meta.get("id")
        if not isinstance(claim, str) or not isinstance(obligation_id, str):
            raise GateError(f"claim metadata missing in {meta_path.relative_to(ROOT)}")
        if claim in claims:
            raise GateError(f"duplicate formal claim: {claim}")
        rust_files = set(meta.get("rust", {}).get("files", []))
        claims[claim] = rust_files
        proof = meta.get("lean", {}).get("proof")
        evidence_files = {
            reference.partition("#")[0]
            for field in ("executable_tests", "counterexamples")
            for reference in meta.get("evidence", {}).get(field, [])
        }
        for precondition in meta.get("preconditions", {}).values():
            if isinstance(precondition, dict):
                evidence_files.update(
                    reference.partition("#")[0]
                    for reference in precondition.get("evidence", [])
                )
        claim_files[claim] = {
            str(meta_path.relative_to(ROOT)),
            *rust_files,
            *evidence_files,
            *([str((meta_path.parent / proof).relative_to(ROOT))] if proof else []),
        }
        theorem[str(meta.get("theorem", {}).get("status", "missing"))] += 1
        surfaces[str(meta.get("product", {}).get("surface", "missing"))] += 1
        for item in meta.get("preconditions", {}).values():
            if isinstance(item, dict):
                preconditions[str(item.get("status", "missing"))] += 1
        obligations.append({"id": obligation_id, "claim": claim})

    markers: dict[str, set[str]] = {}
    marker_re = re.compile(r"\bproof-claim:\s*(nose\.claim\.[a-z0-9_.-]+)\b")
    for source in sorted((ROOT / "crates").rglob("*.rs")):
        rel = str(source.relative_to(ROOT))
        for match in marker_re.finditer(source.read_text(encoding="utf-8")):
            markers.setdefault(match.group(1), set()).add(rel)
    unregistered = sorted(set(markers) - set(claims))
    unlisted = sorted(
        f"{claim}:{path}"
        for claim, paths in markers.items()
        if claim in claims
        for path in paths - claims[claim]
    )
    return {
        "obligations": len(obligations),
        "theorems": dict(sorted(theorem.items())),
        "preconditions": dict(sorted(preconditions.items())),
        "product_surfaces": dict(sorted(surfaces.items())),
        "unregistered_claims": unregistered,
        "unlisted_claim_markers": unlisted,
        "claim_files": {key: sorted(value) for key, value in sorted(claim_files.items())},
    }


def type4_coverage() -> dict[str, Any]:
    sys.path.insert(0, str(TYPE4))
    import axis_claim_gate as gate  # type: ignore[import-not-found]

    registry = gate.load(gate.REGISTRY)
    summary = gate.validate(
        registry,
        gate.load(gate.EVIDENCE),
        gate.load(gate.BLIND_RECEIPT),
        gate.load(gate.DECLARATIVE_MATRIX),
    )
    return {
        "axes": summary["axes"],
        "exact_cells": summary["cells"],
        "closed_cells": summary["closed_cells"],
        "unguarded_tier_a_cells": [],
    }


def static_snapshot() -> dict[str, Any]:
    manifest = load(BASELINE / "manifest.v1.json")
    baseline = load(BASELINE / "scorecard.v1.json")
    overlay = load(CURRENT / "oracle-expansion-overlay.v2.json")
    receipt = load(CURRENT / "oracle-expansion-859.v1.json")
    blind = load(TYPE4 / "blind_attack.v1.json")
    return {
        "official_baseline": {
            "version": manifest["baseline"],
            "source_commit": manifest["source"]["release_commit"],
            "published_binary_sha256": manifest["published_asset_identity"]["binary_sha256"],
            "corpus": manifest["corpus"],
            "risk_weighted_coverage": baseline["summary"],
        },
        "candidate": {
            "risk_weighted_coverage": overlay["summary"],
            "coverage_gates": overlay["gates"],
            "focused_falsification": receipt["falsification"],
            "blind_attack": blind["hard_gate"],
        },
        "formal": formal_coverage(),
        "type4": type4_coverage(),
        "attribution": {
            "generic_unattributed_exclusions": manifest["exclusion_attribution"][
                "generic_unattributed_exclusions"
            ]
        },
    }


def validate_snapshot(snapshot: dict[str, Any], corpus: dict[str, Any] | None = None) -> None:
    errors: list[str] = []
    baseline = snapshot["official_baseline"]["risk_weighted_coverage"]
    candidate = snapshot["candidate"]
    coverage = candidate["risk_weighted_coverage"]
    gates = candidate["coverage_gates"]
    if not gates.get("passed") or not gates.get("release_target_met"):
        errors.append("risk-weighted coverage regression or release target failure")
    for language, floor in baseline["by_language"].items():
        current = coverage.get("by_language", {}).get(language)
        if not current or current["macro_ppm"] < floor["macro_ppm"]:
            errors.append(f"language coverage floor regressed: {language}")
    if snapshot["formal"]["unregistered_claims"]:
        errors.append("unregistered soundness-bearing claim")
    if snapshot["formal"]["unlisted_claim_markers"]:
        errors.append("claim marker is outside its registered source set")
    if snapshot["type4"]["unguarded_tier_a_cells"]:
        errors.append("unguarded Tier-A exact cell")
    if snapshot["attribution"]["generic_unattributed_exclusions"]:
        errors.append("generic/unattributed exclusion returned")
    for label in ("focused_falsification", "blind_attack"):
        evidence = candidate[label]
        if evidence.get("false_merges", 0):
            errors.append(f"{label} has a hard false merge")
        canon = evidence.get(
            "canon_preservation_violations", evidence.get("canon_changes", 0)
        )
        if canon:
            errors.append(f"{label} has a canon-preservation violation")
    if corpus is not None:
        totals = corpus.get("totals", {})
        if not corpus.get("complete"):
            errors.append("nightly corpus evidence is incomplete")
        if totals.get("failed_repositories", 0):
            errors.append("nightly corpus has failed or timed-out repositories")
        if totals.get("false_merges", 0):
            errors.append("nightly corpus has a hard false merge")
        if totals.get("canon_changes", 0):
            errors.append("nightly corpus has a canon-preservation violation")
    if errors:
        raise GateError("; ".join(errors))


def shard_plan(count: int) -> list[dict[str, Any]]:
    if count < 1:
        raise GateError("shard count must be positive")
    shards = [{"id": str(index + 1), "repos": []} for index in range(count)]
    for index, row in enumerate(corpus_rows()):
        shards[index % count]["repos"].append(row["id"])
    return shards


def validate_shard_evidence(evidence: dict[str, Any]) -> None:
    if evidence.get("schema") != "nose-corpus-verify-evidence/v2":
        raise GateError("nightly shard has an unsupported evidence schema")
    results = evidence.get("results")
    repositories = evidence.get("repositories")
    if not isinstance(results, list) or not isinstance(repositories, list):
        raise GateError("nightly shard is missing repository results")
    result_ids = [row["id"] for row in results]
    repository_ids = [row["id"] for row in repositories]
    if len(result_ids) != len(set(result_ids)) or len(repository_ids) != len(
        set(repository_ids)
    ):
        raise GateError("nightly shard contains duplicate repositories")
    if set(result_ids) != set(repository_ids):
        raise GateError("nightly shard identities and results disagree")
    expected_totals = {
        "repositories": len(results),
        "failed_repositories": sum(row["status"] != "pass" for row in results),
        "false_merges": sum(row["false_merges"] for row in results),
        "canon_changes": sum(row["canon_changes"] for row in results),
        "advisory": sum(row["advisory"] for row in results),
    }
    if evidence.get("totals") != expected_totals:
        raise GateError("nightly shard totals do not match repository results")
    nose = evidence.get("nose", {})
    if not nose.get("sha256") or not nose.get("version"):
        raise GateError("nightly shard is not bound to an explicit nose binary")


def advisory_baseline(path: Path = ADVISORY_BASELINE) -> dict[str, Any]:
    value = load(path)
    if value.get("schema") != "nose-corpus-advisory-baseline/v1":
        raise GateError("advisory baseline schema changed")
    rows = value.get("repositories", [])
    ids = [row["id"] for row in rows]
    if len(ids) != len(set(ids)) or set(ids) != {row["id"] for row in corpus_rows()}:
        raise GateError("advisory baseline does not cover the pinned corpus")
    if value.get("total") != sum(row["advisory"] for row in rows):
        raise GateError("advisory baseline total is inconsistent")
    return value


def freeze_advisory_baseline(evidence_path: Path, output: Path) -> None:
    evidence = load(evidence_path)
    validate_shard_evidence(evidence)
    snapshot = static_snapshot()
    expected_sha = snapshot["official_baseline"]["published_binary_sha256"]
    if not evidence.get("complete") or evidence.get("nose", {}).get("sha256") != expected_sha:
        raise GateError("advisory baseline requires a complete official v0.19.0 binary replay")
    results = sorted(evidence["results"], key=lambda row: row["id"])
    expected = {row["id"] for row in corpus_rows()}
    if {row["id"] for row in results} != expected:
        raise GateError("advisory baseline replay did not cover the exact pinned corpus")
    if any(
        row["status"] != "pass" or row["exit_code"] or row["false_merges"] or row["canon_changes"]
        for row in results
    ):
        raise GateError("official advisory baseline replay failed a hard gate")
    value = {
        "schema": "nose-corpus-advisory-baseline/v1",
        "source": {
            "version": "0.19.0",
            "binary_sha256": expected_sha,
            "corpus_manifest_sha256": sha256(CORPUS),
        },
        "repositories": [
            {"id": row["id"], "advisory": row["advisory"]} for row in results
        ],
        "total": sum(row["advisory"] for row in results),
    }
    official_total = snapshot["official_baseline"]["corpus"]["advisory_disagreements"]
    if value["total"] != official_total:
        raise GateError(
            f"official advisory total changed: expected {official_total}, got {value['total']}"
        )
    output.parent.mkdir(parents=True, exist_ok=True)
    output.write_bytes(canonical(value))


def merge_shards(inputs: Path, baseline_path: Path) -> dict[str, Any]:
    paths = sorted(inputs.rglob("evidence.json"))
    if not paths:
        raise GateError(f"no shard evidence found under {inputs}")
    shards = [load(path) for path in paths]
    for shard in shards:
        validate_shard_evidence(shard)
    results = [row for shard in shards for row in shard["results"]]
    repositories = [row for shard in shards for row in shard["repositories"]]
    expected = {row["id"]: row["commit"] for row in corpus_rows()}
    ids = [row["id"] for row in results]
    if len(ids) != len(set(ids)):
        raise GateError("nightly shards overlap")
    if set(ids) != set(expected):
        missing = sorted(set(expected) - set(ids))
        extra = sorted(set(ids) - set(expected))
        raise GateError(f"nightly shard coverage mismatch: missing={missing}, extra={extra}")
    by_repository = {row["id"]: row for row in repositories}
    for repo_id, commit in expected.items():
        observed = by_repository.get(repo_id)
        if not observed or observed.get("expected_commit") != commit or observed.get(
            "observed_commit"
        ) != commit:
            raise GateError(f"nightly repository pin mismatch: {repo_id}")
    nose_identities = {json.dumps(shard["nose"], sort_keys=True) for shard in shards}
    if len(nose_identities) != 1:
        raise GateError("nightly shards used different nose binaries")
    manifest_hashes = {shard["corpus_manifest_sha256"] for shard in shards}
    if manifest_hashes != {sha256(CORPUS)}:
        raise GateError("nightly shards used a different corpus manifest")
    source_commits = {shard.get("source_commit") for shard in shards}
    if len(source_commits) != 1 or not re.fullmatch(
        r"[0-9a-f]{40}", next(iter(source_commits), "") or ""
    ):
        raise GateError("nightly shards belong to different source commits")

    results = sorted(results, key=lambda row: row["id"])
    baseline = advisory_baseline(baseline_path)
    before = {row["id"]: row["advisory"] for row in baseline["repositories"]}
    advisory_diff = [
        {
            "id": row["id"],
            "baseline": before[row["id"]],
            "candidate": row["advisory"],
            "delta": row["advisory"] - before[row["id"]],
        }
        for row in results
    ]
    totals = {
        "repositories": len(results),
        "failed_repositories": sum(row["status"] != "pass" for row in results),
        "false_merges": sum(row["false_merges"] for row in results),
        "canon_changes": sum(row["canon_changes"] for row in results),
        "advisory": sum(row["advisory"] for row in results),
    }
    merged = {
        "schema": "nose-corpus-verify-merged/v1",
        "complete": True,
        "nose": shards[0]["nose"],
        "source_commit": shards[0].get("source_commit"),
        "corpus_manifest_sha256": sha256(CORPUS),
        "repositories": sorted(repositories, key=lambda row: row["id"]),
        "results": results,
        "totals": totals,
        "canonical_result_sha256": hashlib.sha256(
            ("\n".join(
                "\t".join(
                    str(row[key])
                    for key in (
                        "id", "status", "exit_code", "false_merges", "canon_changes", "advisory"
                    )
                )
                for row in results
            ) + "\n").encode()
        ).hexdigest(),
        "advisory": {
            "blocking": False,
            "baseline_total": baseline["total"],
            "candidate_total": totals["advisory"],
            "delta": totals["advisory"] - baseline["total"],
            "per_repository_diff": advisory_diff,
        },
    }
    validate_snapshot(static_snapshot(), merged)
    return merged


def language_floors(snapshot: dict[str, Any]) -> list[dict[str, Any]]:
    baseline = snapshot["official_baseline"]["risk_weighted_coverage"]["by_language"]
    current = snapshot["candidate"]["risk_weighted_coverage"]["by_language"]
    return [
        {
            "language": language,
            "baseline_macro_ppm": baseline[language]["macro_ppm"],
            "candidate_macro_ppm": current[language]["macro_ppm"],
            "passed": current[language]["macro_ppm"] >= baseline[language]["macro_ppm"],
        }
        for language in sorted(baseline)
    ]


def release_report(corpus: dict[str, Any], deep: dict[str, Any], commit: str) -> dict[str, Any]:
    snapshot = static_snapshot()
    validate_snapshot(snapshot, corpus)
    if deep.get("schema") != "nose-soundness-deep-evidence/v1":
        raise GateError("deep campaign evidence schema changed")
    if corpus.get("source_commit") != commit:
        raise GateError("nightly corpus evidence belongs to another source commit")
    if deep.get("source_commit") != commit or not all(deep.get("checks", {}).values()):
        raise GateError("deep campaign is missing, failed, or belongs to another commit")
    return {
        "schema": "nose-soundness-release-gate/v1",
        "source_commit": commit,
        "gate_passed": True,
        "official_baseline": snapshot["official_baseline"],
        "hard_gates": {
            "pinned_corpus": corpus["totals"],
            "focused_falsification": snapshot["candidate"]["focused_falsification"],
            "blind_attack": snapshot["candidate"]["blind_attack"],
            "deep_campaign": deep["checks"],
            "registered_claims": not snapshot["formal"]["unregistered_claims"],
            "guarded_tier_a_cells": not snapshot["type4"]["unguarded_tier_a_cells"],
            "attributed_exclusions": (
                snapshot["attribution"]["generic_unattributed_exclusions"] == 0
            ),
        },
        "risk_weighted_coverage": snapshot["candidate"]["risk_weighted_coverage"],
        "language_floors": language_floors(snapshot),
        "proof_coverage": {
            key: snapshot["formal"][key]
            for key in ("obligations", "theorems", "preconditions", "product_surfaces")
        },
        "type4_claim_perimeter": snapshot["type4"],
        "advisory": corpus["advisory"],
    }


def markdown_summary(report: dict[str, Any]) -> str:
    if report["schema"] == "nose-corpus-verify-merged/v1":
        totals = report["totals"]
        advisory = report["advisory"]
        return (
            "## Soundness Lab nightly\n\n"
            f"- pinned repositories: {totals['repositories']}\n"
            f"- failed / false merges / canon violations: "
            f"{totals['failed_repositories']} / {totals['false_merges']} / {totals['canon_changes']}\n"
            f"- advisory disagreements: {advisory['candidate_total']} "
            f"({advisory['delta']:+d}, non-blocking)\n"
            f"- deterministic result: `{report['canonical_result_sha256']}`\n"
        )
    coverage = report["risk_weighted_coverage"]
    proof = report["proof_coverage"]
    return (
        "## Soundness Lab 0.20 release gate\n\n"
        f"- result: **{'PASS' if report['gate_passed'] else 'FAIL'}**\n"
        f"- official baseline: v{report['official_baseline']['version']}\n"
        f"- risk-weighted coverage: {coverage['macro_ppm'] / 10000:.2f}% "
        f"(target {coverage['release_target_ppm'] / 10000:.2f}%)\n"
        f"- frozen verified pairs: {coverage['verified_pair_mass']}/{coverage['baseline_pair_mass']}\n"
        f"- proof coverage: {proof['theorems']}\n"
        f"- precondition coverage: {proof['preconditions']}\n"
    )


def write_report(value: dict[str, Any], output: Path, markdown: Path | None) -> None:
    output.parent.mkdir(parents=True, exist_ok=True)
    output.write_bytes(canonical(value))
    if markdown:
        markdown.parent.mkdir(parents=True, exist_ok=True)
        markdown.write_text(markdown_summary(value))


def write_deep_evidence(args: argparse.Namespace) -> None:
    checks = {
        "source_runtime_calibration": args.domain == "success",
        "metamorphic_equivalence": args.equivalence == "success",
        "multi_seed_falsification": args.falsification == "success",
    }
    value = {
        "schema": "nose-soundness-deep-evidence/v1",
        "source_commit": args.commit,
        "checks": checks,
    }
    args.output.parent.mkdir(parents=True, exist_ok=True)
    args.output.write_bytes(canonical(value))


def pr_plan(base: str, output: Path, markdown: Path) -> None:
    completed = subprocess.run(
        ["git", "diff", "--name-only", f"{base}...HEAD"],
        cwd=ROOT,
        text=True,
        capture_output=True,
        check=False,
    )
    if completed.returncode:
        raise GateError(f"cannot diff PR base {base}: {completed.stderr.strip()}")
    changed = sorted(line for line in completed.stdout.splitlines() if line)
    formal = formal_coverage()
    touched = [
        claim
        for claim, files in formal["claim_files"].items()
        if set(files) & set(changed)
    ]
    value = {
        "schema": "nose-soundness-pr-plan/v1",
        "base": base,
        "changed_files": changed,
        "touched_claims": sorted(touched),
        "checks": [
            "formal-registry-and-proofs",
            "risk-weighted-scorecard",
            "Tier-A-axis-language-perimeter",
            "source-runtime-calibration",
            "focused-equivalence-battery",
        ],
        "selection": "All Soundness Lab checks run conservatively; touched claims are the review index.",
    }
    output.parent.mkdir(parents=True, exist_ok=True)
    output.write_bytes(canonical(value))
    lines = ["## Soundness Lab PR plan", "", f"- changed files: {len(changed)}"]
    lines.append(f"- touched registered claims: {len(touched)}")
    lines.extend(f"  - `{claim}`" for claim in sorted(touched))
    markdown.write_text("\n".join(lines) + "\n")


def check_workflow() -> None:
    try:
        text = WORKFLOW.read_text()
    except FileNotFoundError as error:
        raise GateError("Soundness Lab workflow is missing") from error
    required = (
        "pull_request:",
        'cron: "17 18 * * *"',
        'cron: "43 17 * * 0"',
        "plan-nightly:",
        "nightly-shard:",
        "merge-nightly:",
        "deep-campaign:",
        "release-gate:",
        "if-no-files-found: error",
        "failure() || cancelled() || success()",
        '--source-commit "${{ github.sha }}"',
        "corpus-verify-nightly.sh --self-test",
        "check-soundness-scorecard.py --self-test",
        "check-formal-obligations.py --self-test",
        "check_axis_language_claims.py --self-test",
        "soundness_exclusions.py --self-test",
    )
    missing = [item for item in required if item not in text]
    if missing:
        raise GateError(f"Soundness Lab workflow contract is incomplete: {missing}")


def self_test() -> None:
    snapshot = static_snapshot()
    empty_corpus = {
        "complete": True,
        "totals": {
            "repositories": 1,
            "failed_repositories": 0,
            "false_merges": 0,
            "canon_changes": 0,
            "advisory": 1,
        },
    }
    validate_snapshot(snapshot, empty_corpus)
    mutations = (
        ("false merge", lambda item: item["totals"].update(false_merges=1), "corpus"),
        ("canon violation", lambda item: item["totals"].update(canon_changes=1), "corpus"),
        (
            "coverage regression",
            lambda item: item["candidate"]["coverage_gates"].update(passed=False),
            "snapshot",
        ),
        (
            "unregistered claim",
            lambda item: item["formal"]["unregistered_claims"].append("nose.claim.mutant"),
            "snapshot",
        ),
        (
            "unguarded Tier-A cell",
            lambda item: item["type4"]["unguarded_tier_a_cells"].append("mutant/rust"),
            "snapshot",
        ),
        (
            "generic attribution",
            lambda item: item["attribution"].update(generic_unattributed_exclusions=1),
            "snapshot",
        ),
    )
    for label, mutate, target in mutations:
        changed_snapshot = copy.deepcopy(snapshot)
        changed_corpus = copy.deepcopy(empty_corpus)
        mutate(changed_corpus if target == "corpus" else changed_snapshot)
        try:
            validate_snapshot(changed_snapshot, changed_corpus)
        except GateError:
            pass
        else:
            raise AssertionError(f"self-test mutation escaped: {label}")
    first = canonical(snapshot)
    second = canonical(static_snapshot())
    if first != second:
        raise AssertionError("same-source static release evidence is not byte-deterministic")
    plan = shard_plan(7)
    ids = [repo for shard in plan for repo in shard["repos"]]
    if sorted(ids) != sorted(row["id"] for row in corpus_rows()) or len(ids) != len(set(ids)):
        raise AssertionError("shard plan did not cover every pinned repository exactly once")
    check_workflow()
    print("Soundness Lab gate self-test: six fail-closed mutations rejected")


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    sub = parser.add_subparsers(dest="command", required=True)
    sub.add_parser("check")
    sub.add_parser("self-test")
    plan = sub.add_parser("plan-shards")
    plan.add_argument("--count", type=int, default=4)
    merge = sub.add_parser("merge-nightly")
    merge.add_argument("--inputs", type=Path, required=True)
    merge.add_argument("--advisory-baseline", type=Path, default=ADVISORY_BASELINE)
    merge.add_argument("--output", type=Path, required=True)
    merge.add_argument("--markdown", type=Path)
    freeze_advisory = sub.add_parser("freeze-advisory-baseline")
    freeze_advisory.add_argument("--evidence", type=Path, required=True)
    freeze_advisory.add_argument("--output", type=Path, default=ADVISORY_BASELINE)
    release = sub.add_parser("release")
    release.add_argument("--corpus", type=Path, required=True)
    release.add_argument("--deep", type=Path, required=True)
    release.add_argument("--commit", required=True)
    release.add_argument("--output", type=Path, required=True)
    release.add_argument("--markdown", type=Path)
    deep = sub.add_parser("deep-evidence")
    deep.add_argument("--commit", required=True)
    deep.add_argument("--domain", required=True)
    deep.add_argument("--equivalence", required=True)
    deep.add_argument("--falsification", required=True)
    deep.add_argument("--output", type=Path, required=True)
    pr = sub.add_parser("pr-plan")
    pr.add_argument("--base", required=True)
    pr.add_argument("--output", type=Path, required=True)
    pr.add_argument("--markdown", type=Path, required=True)
    args = parser.parse_args()
    try:
        if args.command == "check":
            snapshot = static_snapshot()
            validate_snapshot(snapshot)
            check_workflow()
            print(
                "Soundness Lab static gate: "
                f"{snapshot['formal']['obligations']} obligations, "
                f"{snapshot['type4']['exact_cells']} guarded Tier-A cells"
            )
        elif args.command == "self-test":
            self_test()
        elif args.command == "plan-shards":
            print(json.dumps(shard_plan(args.count), separators=(",", ":")))
        elif args.command == "merge-nightly":
            report = merge_shards(args.inputs, args.advisory_baseline)
            write_report(report, args.output, args.markdown)
        elif args.command == "freeze-advisory-baseline":
            freeze_advisory_baseline(args.evidence, args.output)
        elif args.command == "release":
            report = release_report(load(args.corpus), load(args.deep), args.commit)
            write_report(report, args.output, args.markdown)
        elif args.command == "deep-evidence":
            write_deep_evidence(args)
        elif args.command == "pr-plan":
            pr_plan(args.base, args.output, args.markdown)
    except (GateError, KeyError, OSError, TypeError, subprocess.SubprocessError) as error:
        print(f"Soundness Lab gate failed: {error}", file=sys.stderr)
        return 1
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
