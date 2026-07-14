#!/usr/bin/env python3
"""Freeze and validate the leakage-resistant divergent-gate 0.20 protocol (#848).

The checked artifact contains only development evidence, preregistered policy, and
opaque HMAC identities.  Repository/commit identities and source-bearing diffs stay
in a private directory outside the repository until the one-shot evaluation.
"""

from __future__ import annotations

import argparse
import copy
import hashlib
import hmac
import json
import math
from pathlib import Path
import re
import secrets
import stat
import subprocess
import sys
from typing import Any

import replay as replay_harness


ROOT = Path(__file__).resolve().parents[2]
PUBLIC_PATH = ROOT / "eval/divergence_fire/precision_protocol_2026_07_14.v1.json"
SIDECAR_PATH = PUBLIC_PATH.with_suffix(PUBLIC_PATH.suffix + ".sha256")
CORPUS_PATH = ROOT / "bench/goldens/corpus.json"
PRUNE_PATH = ROOT / "bench/labels/prune_manifest.json"
DEV_SAMPLES = ROOT / "eval/divergence_fire/sampled_findings_2026_07_06.jsonl"
DEV_VERDICTS = ROOT / "eval/divergence_fire/verdicts_2026_07_06.jsonl"
DEV_POLICY = ROOT / "eval/divergence_fire/policy_eval_2026_07_06.json"
FINAL_REPLAY = (
    ROOT / "eval/divergence_fire/replay_summary_final_head_a38ecb8b_2026_07_06.json"
)

SCHEMA = "nose.divergent_precision_protocol.v1"
PRIVATE_SCHEMA = "nose.divergent_precision_private_population.v1"
PRIVATE_MANIFEST_SCHEMA = "nose.divergent_precision_private_manifest.v1"
VERDICT_SCHEMA = "nose.divergent_precision_verdict.v1"
SEALED_AT = "2026-07-14T14:10:00Z"
SUPPORTED_LANGUAGES = ("C", "Go", "Java", "Python", "Ruby", "Rust", "TypeScript")
BLIND_REPOS_PER_LANGUAGE = 4
TEMPORAL_REPOS_PER_LANGUAGE = 4
BLIND_CHANGES_PER_REPO = 40
TARGET_STRICT_MINIMUM = 100
CANARY_CHANGES = 1000
ONE_SIDED_CONFIDENCE = 0.95
ONE_SIDED_Z = 1.6448536269514722

OFFICIAL_BINARY_SHA256 = "0f73ea544da06cc175e01c31c383cc4cb86daf3d37a49d74de61dea3724fe0f3"
OFFICIAL_BINARY_VERSION = "nose 0.19.0"
OFFICIAL_RELEASE_TAG = "v0.19.0"
OFFICIAL_RELEASE_ASSET = "nose-cli-aarch64-apple-darwin.tar.gz"

DEV_REPOSITORIES = (
    "git", "redis", "curl", "hugo", "minio", "cobra", "prometheus",
    "netty", "rxjava", "guava", "gson", "scrapy", "sympy", "black",
    "requests", "rubocop", "sidekiq", "devise", "clap", "tokio", "regex",
    "fd", "jest", "rxjs", "prettier", "axios", "date-fns", "execa",
)

PRIVATE_SEED = "root-seed.hex"
PRIVATE_PACKET = "blind-population.private.jsonl"
PRIVATE_MANIFEST = "private-manifest.json"

FORBIDDEN_PUBLIC_KEYS = {
    "repo", "repository", "commit", "parent", "subject", "diff", "source",
    "file", "path", "url", "name",
}
HEX40 = re.compile(r"(?<![0-9a-f])[0-9a-f]{40}(?![0-9a-f])")


def require(condition: bool, message: str) -> None:
    if not condition:
        raise AssertionError(message)


def canonical(value: Any) -> bytes:
    return json.dumps(
        value, sort_keys=True, separators=(",", ":"), ensure_ascii=False
    ).encode("utf-8")


def sha256_bytes(value: bytes) -> str:
    return hashlib.sha256(value).hexdigest()


def sha256_file(path: Path) -> str:
    digest = hashlib.sha256()
    with path.open("rb") as handle:
        for chunk in iter(lambda: handle.read(1024 * 1024), b""):
            digest.update(chunk)
    return digest.hexdigest()


def git(*args: str, cwd: Path = ROOT, check: bool = True) -> subprocess.CompletedProcess[str]:
    result = subprocess.run(
        ["git", *args], cwd=cwd, capture_output=True, text=True, errors="replace"
    )
    if check and result.returncode != 0:
        raise AssertionError(f"git {' '.join(args)}: {result.stderr.strip()}")
    return result


def hmac_hex(key: bytes, label: str, value: Any) -> str:
    payload = label.encode("utf-8") + b"\0" + canonical(value)
    return hmac.new(key, payload, hashlib.sha256).hexdigest()


def derive_key(root_seed: bytes, label: str) -> bytes:
    return hmac.new(root_seed, label.encode("utf-8"), hashlib.sha256).digest()


def wilson_lower(successes: int, total: int, z: float = ONE_SIDED_Z) -> float | None:
    if total == 0:
        return None
    require(0 <= successes <= total, "invalid Wilson count")
    p = successes / total
    z2 = z * z
    center = p + z2 / (2 * total)
    spread = z * math.sqrt((p * (1 - p) + z2 / (4 * total)) / total)
    return (center - spread) / (1 + z2 / total)


def checked_file(path: Path) -> dict[str, Any]:
    return {
        "path": str(path.relative_to(ROOT)),
        "bytes": path.stat().st_size,
        "sha256": sha256_file(path),
        "git_blob": git("hash-object", str(path)).stdout.strip(),
    }


def development_baseline() -> dict[str, Any]:
    samples = replay_harness.load_jsonl(DEV_SAMPLES)
    verdicts = replay_harness.load_jsonl(DEV_VERDICTS)
    report = replay_harness.compute_policy_eval(samples, verdicts)
    strict = replay_harness.policy_row_by_name(
        report["finding_level"], "V2 strict: gate.fail_default"
    )
    require(report["labeled"] == 179, "development labeled count")
    require(strict == {
        "policy": "V2 strict: gate.fail_default",
        "fires": 80,
        "tp": 45,
        "fp": 35,
        "precision": 0.562,
    }, "development v2 strict baseline")
    final_replay = json.loads(FINAL_REPLAY.read_text())
    metadata = final_replay.get("metadata") or {}
    require(tuple(metadata.get("repos") or ()) == DEV_REPOSITORIES,
            "development repository identity")
    return {
        "role": "development-only",
        "labeled_findings": 179,
        "strict_findings": 80,
        "true_positive": 45,
        "false_positive": 35,
        "precision_reported": 0.562,
        "precision_exact": 45 / 80,
        "one_sided_wilson_95_lower": wilson_lower(45, 80),
        "repositories": list(DEV_REPOSITORIES),
        "repository_count": len(DEV_REPOSITORIES),
        "historical_replay_source_commit": metadata.get("source_commit"),
        "historical_replay_binary_sha256": metadata.get("nose_binary_sha256"),
        "historical_replay_binary_version": metadata.get("nose_version"),
        "inputs": {
            "samples": checked_file(DEV_SAMPLES),
            "verdicts": checked_file(DEV_VERDICTS),
            "policy": checked_file(DEV_POLICY),
            "final_replay": checked_file(FINAL_REPLAY),
        },
    }


def protocol_contract() -> dict[str, Any]:
    return {
        "objective_order": [
            "fail-closed evidence and byte determinism",
            "maximize strict target precision lower confidence bound",
            "retain non-degenerate strict support",
            "use strict recall only as a tie-breaker",
        ],
        "units": {
            "target": "one direct changed-member to skipped-sibling propagation edge",
            "finding": "one emitted family finding; positive when at least one direct strict target should propagate",
            "change": "one repository first-parent commit; a true block has at least one should-propagate strict target",
        },
        "precision": {
            "target": "should-propagate strict targets / all adjudicated strict targets",
            "finding": "positive strict findings / all adjudicated strict findings",
            "change": "changes with at least one should-propagate strict target / changes with at least one strict target",
        },
        "interval": {
            "method": "Wilson score lower confidence bound",
            "sidedness": "one-sided",
            "confidence": ONE_SIDED_CONFIDENCE,
            "z": ONE_SIDED_Z,
            "zero_denominator": None,
        },
        "primary_arm": {
            "mode": "syntax,semantic",
            "command": "nose query . base=<parent> top=0 --format json --mode syntax,semantic",
            "enters_gate_decision": True,
        },
        "advisory_arm": {
            "mode": "syntax,semantic,near",
            "command": "nose query . base=<parent> top=0 --format json --mode syntax,semantic,near",
            "enters_gate_decision": False,
        },
        "required_slices": [
            "language", "witness kind", "semantic change kind", "repository",
            "evidence caveat", "finding", "target", "change",
        ],
        "error_policy": {
            "integrity_or_identity_error": "invalid-evaluation",
            "query_timeout_parse_or_lossy_error": "count and retain; never resample",
            "missing_or_incomplete_evidence": "review; cannot promote to strict",
            "pool_exhausted_below_support": "insufficient-evidence",
        },
    }


def stop_rule() -> dict[str, Any]:
    return {
        "minimum_strict_targets": TARGET_STRICT_MINIMUM,
        "ordering": "secret HMAC repository order, then secret HMAC change order",
        "repository_atomic": True,
        "procedure": [
            "run the frozen binary and primary arm over every change of the next repository",
            "adjudicate every emitted strict target from that complete repository",
            "stop after the first complete repository whose cumulative strict-target count is at least 100",
            "never select, discard, or stop within a repository, change, finding, or target",
        ],
        "exhaustion": "If every sealed blind repository yields fewer than 100 strict targets, verdict is insufficient-evidence.",
        "errors": "A failed replay remains in its repository and cannot be replaced by another change.",
    }


def decision_matrix() -> dict[str, Any]:
    return {
        "allowed_verdicts": [
            "default-on-ready", "improved-opt-in-only", "failed",
            "insufficient-evidence",
        ],
        "blind_policy_gate": {
            "strict_target_precision_min": 0.95,
            "strict_target_wilson_lower_min": 0.90,
            "strict_target_support_min": TARGET_STRICT_MINIMUM,
            "finding_and_change_precision_reported": True,
            "no_post_reveal_tuning": True,
        },
        "default_on_gate": {
            "change_block_precision_min": 0.99,
            "change_block_wilson_lower_min": 0.95,
            "strict_target_support_min": TARGET_STRICT_MINIMUM,
            "repository_support_min": 20,
            "per_language_claim_requires_preregistered_support": True,
            "temporal_canary_changes": CANARY_CHANGES,
            "confirmed_false_required_check_blocks_max": 0,
        },
        "classification": {
            "insufficient-evidence": "population exhausted below support or temporal canary cannot complete",
            "failed": "blind target gate fails or integrity is invalid",
            "improved-opt-in-only": "blind target gate passes but any default-on gate fails",
            "default-on-ready": "every blind and temporal default-on gate passes",
        },
    }


def verdict_protocol() -> dict[str, Any]:
    rubric = {
        "schema": VERDICT_SCHEMA,
        "classes": [
            "should_propagate", "intentional_divergence", "not_a_clone",
            "no_propagation_needed", "test_scaffolding", "unclear",
        ],
        "reviewers_per_target": 2,
        "resolver_for_disagreement": True,
        "identity": "opaque target id only until all raw verdicts and resolution are sealed",
        "required_fields": [
            "opaque_target_id", "reviewer", "verdict", "reason", "attestation",
        ],
        "forbidden_before_seal": [
            "repository", "commit", "path", "rank", "policy predicate",
            "another reviewer verdict",
        ],
    }
    return {
        "state": "unopened-no-quality-labels-exist",
        "verdict_count": 0,
        "rubric": rubric,
        "rubric_sha256": sha256_bytes(canonical(rubric)),
        "seal_transition": "raw reviewer verdicts are frozen atomically, then disagreements are resolved, then identities may be revealed once",
        "implementation_access": "none",
    }


def manifest_repositories() -> list[dict[str, Any]]:
    document = json.loads(CORPUS_PATH.read_text())
    repositories = document.get("repositories") or []
    require(len(repositories) == 120, "corpus repository count")
    return repositories


def repository_partitions(root_seed: bytes) -> tuple[list[dict[str, Any]], list[dict[str, Any]]]:
    repo_key = derive_key(root_seed, "repository-selection")
    candidates: dict[str, list[dict[str, Any]]] = {lang: [] for lang in SUPPORTED_LANGUAGES}
    for row in manifest_repositories():
        if row["primary_language"] not in candidates or row["id"] in DEV_REPOSITORIES:
            continue
        candidates[row["primary_language"]].append(row)
    blind: list[dict[str, Any]] = []
    temporal: list[dict[str, Any]] = []
    for language in SUPPORTED_LANGUAGES:
        ordered = sorted(
            candidates[language],
            key=lambda row: hmac_hex(repo_key, f"repository:{language}", row["id"]),
        )
        require(len(ordered) >= BLIND_REPOS_PER_LANGUAGE + TEMPORAL_REPOS_PER_LANGUAGE,
                f"not enough repositories for {language}")
        blind.extend(ordered[:BLIND_REPOS_PER_LANGUAGE])
        temporal.extend(ordered[
            BLIND_REPOS_PER_LANGUAGE:
            BLIND_REPOS_PER_LANGUAGE + TEMPORAL_REPOS_PER_LANGUAGE
        ])
    require(not ({r["id"] for r in blind} & set(DEV_REPOSITORIES)),
            "blind/development overlap")
    require(not ({r["id"] for r in blind} & {r["id"] for r in temporal}),
            "blind/temporal overlap")
    return blind, temporal


def supported_changed_paths(repo: Path, parent: str, commit: str) -> list[str]:
    result = git("diff", "--name-only", "--diff-filter=ACDMRT", parent, commit,
                 cwd=repo)
    return sorted({
        line for line in result.stdout.splitlines()
        if Path(line).suffix.lower() in replay_harness.SUPPORTED_EXTS
    })


def source_diff(repo: Path, parent: str, commit: str, paths: list[str]) -> str:
    require(paths, "source diff paths")
    return git(
        "diff", "--no-ext-diff", "--no-color", "--binary", parent, commit,
        "--", *paths, cwd=repo,
    ).stdout


def timestamp(repo: Path, commit: str, field: str) -> str:
    fmt = "%aI" if field == "author" else "%cI"
    return git("show", "-s", f"--format={fmt}", commit, cwd=repo).stdout.strip()


def collect_private_population(
    root_seed: bytes, repos_root: Path, blind: list[dict[str, Any]],
) -> tuple[list[dict[str, Any]], list[dict[str, Any]]]:
    identity_key = derive_key(root_seed, "change-identity")
    commitment_key = derive_key(root_seed, "change-commitment")
    order_key = derive_key(root_seed, "change-order")
    repository_key = derive_key(root_seed, "repository-order")
    private_repositories = []
    changes = []
    for row in blind:
        repo = repos_root / row["id"]
        require(repo.is_dir(), f"missing repository {row['id']}")
        head = git("rev-parse", "HEAD", cwd=repo).stdout.strip()
        require(head == row["commit"], f"repository head {row['id']}")
        eligible = replay_harness.eligible_commits(repo)
        selected = replay_harness.even_sample(eligible, BLIND_CHANGES_PER_REPO)
        require(selected, f"no eligible changes {row['id']}")
        opaque_repo = hmac_hex(identity_key, "repository", row["id"])
        private_repositories.append({
            "id": row["id"],
            "language": row["primary_language"],
            "pinned_head": head,
            "eligible_count_capped": len(eligible),
            "selected_count": len(selected),
            "opaque_repository_id": opaque_repo,
            "order_key": hmac_hex(repository_key, "order", row["id"]),
        })
        for candidate in selected:
            paths = supported_changed_paths(repo, candidate["parent"], candidate["commit"])
            diff = source_diff(repo, candidate["parent"], candidate["commit"], paths)
            identity = {
                "repository": row["id"],
                "commit": candidate["commit"],
                "parent": candidate["parent"],
            }
            opaque_id = hmac_hex(identity_key, "change", identity)
            private = {
                "record_type": "change",
                "opaque_change_id": opaque_id,
                "opaque_repository_id": opaque_repo,
                "repository": row["id"],
                "language": row["primary_language"],
                "pinned_head": head,
                "commit": candidate["commit"],
                "parent": candidate["parent"],
                "subject": candidate["subject"],
                "author_time": timestamp(repo, candidate["commit"], "author"),
                "commit_time": timestamp(repo, candidate["commit"], "commit"),
                "source_files": candidate["src_files"],
                "source_lines": candidate["src_lines"],
                "paths": paths,
                "source_diff": diff,
                "source_diff_bytes": len(diff.encode("utf-8")),
                "source_diff_sha256": sha256_bytes(diff.encode("utf-8")),
            }
            private["commitment"] = hmac_hex(commitment_key, "private-row", private)
            private["order_key"] = hmac_hex(order_key, "change-order", identity)
            changes.append(private)
    repo_order = {
        row["opaque_repository_id"]: row["order_key"] for row in private_repositories
    }
    changes.sort(key=lambda row: (repo_order[row["opaque_repository_id"]], row["order_key"]))
    private_repositories.sort(key=lambda row: row["order_key"])
    return private_repositories, changes


def write_private_packet(path: Path, header: dict[str, Any], changes: list[dict[str, Any]]) -> None:
    with path.open("wb") as handle:
        handle.write(canonical({"record_type": "header", **header}) + b"\n")
        for row in changes:
            handle.write(canonical(row) + b"\n")


def public_rows(changes: list[dict[str, Any]]) -> list[dict[str, Any]]:
    return [
        {
            "ordinal": index,
            "opaque_change_id": row["opaque_change_id"],
            "opaque_repository_id": row["opaque_repository_id"],
            "commitment": row["commitment"],
        }
        for index, row in enumerate(changes)
    ]


def public_temporal_rows(
    root_seed: bytes, temporal: list[dict[str, Any]],
) -> list[dict[str, Any]]:
    identity_key = derive_key(root_seed, "temporal-identity")
    commitment_key = derive_key(root_seed, "temporal-commitment")
    rows = []
    for row in temporal:
        private = {
            "repository": row["id"],
            "language": row["primary_language"],
            "pinned_head_at_seal": row["commit"],
            "eligible_after": SEALED_AT,
        }
        rows.append({
            "opaque_repository_id": hmac_hex(identity_key, "temporal-repository", private),
            "commitment": hmac_hex(commitment_key, "temporal-row", private),
        })
    return sorted(rows, key=lambda row: row["opaque_repository_id"])


def count_by_language(rows: list[dict[str, Any]]) -> dict[str, int]:
    counts = {language: 0 for language in SUPPORTED_LANGUAGES}
    for row in rows:
        counts[row["primary_language"]] += 1
    return counts


def freeze(args: argparse.Namespace) -> None:
    private_dir = args.private_dir.resolve()
    require(private_dir != ROOT and ROOT not in private_dir.parents,
            "private directory must be outside repository")
    private_dir.mkdir(parents=True, exist_ok=True)
    require(not any(private_dir.iterdir()), "private directory must start empty")
    require(not git("status", "--short").stdout.strip(), "working tree must be clean")

    binary = args.nose.resolve()
    require(binary.is_file(), "official binary missing")
    require(sha256_file(binary) == OFFICIAL_BINARY_SHA256, "official binary sha256")
    version = subprocess.run(
        [str(binary), "--version"], capture_output=True, text=True, check=True
    ).stdout.strip()
    require(version == OFFICIAL_BINARY_VERSION, "official binary version")

    root_seed = secrets.token_bytes(32)
    seed_path = private_dir / PRIVATE_SEED
    seed_path.write_text(root_seed.hex() + "\n")
    seed_path.chmod(stat.S_IRUSR | stat.S_IWUSR)
    blind, temporal = repository_partitions(root_seed)
    private_repos, changes = collect_private_population(root_seed, args.repos_root, blind)
    require(len(private_repos) == len(SUPPORTED_LANGUAGES) * BLIND_REPOS_PER_LANGUAGE,
            "blind repository count")

    header = {
        "schema": PRIVATE_SCHEMA,
        "sealed_at": SEALED_AT,
        "protocol_schema": SCHEMA,
        "root_seed_commitment": sha256_bytes(b"nose-848-root\0" + root_seed),
        "blind_repository_count": len(private_repos),
        "change_count": len(changes),
    }
    packet_path = private_dir / PRIVATE_PACKET
    write_private_packet(packet_path, header, changes)
    temporal_private = [
        {
            "repository": row["id"], "language": row["primary_language"],
            "pinned_head_at_seal": row["commit"], "eligible_after": SEALED_AT,
        }
        for row in temporal
    ]
    private_manifest = {
        "schema": PRIVATE_MANIFEST_SCHEMA,
        "sealed_at": SEALED_AT,
        "blind_repositories": private_repos,
        "temporal_reserve": temporal_private,
        "packet": {
            "name": PRIVATE_PACKET,
            "bytes": packet_path.stat().st_size,
            "sha256": sha256_file(packet_path),
        },
    }
    (private_dir / PRIVATE_MANIFEST).write_text(
        json.dumps(private_manifest, indent=2, sort_keys=True) + "\n"
    )

    public = {
        "schema": SCHEMA,
        "sealed_at": SEALED_AT,
        "state": "sealed-unjudged",
        "development_baseline": development_baseline(),
        "protocol": protocol_contract(),
        "sampling_stop_rule": stop_rule(),
        "decision_matrix": decision_matrix(),
        "verdict_protocol": verdict_protocol(),
        "population": {
            "split_rule": "repository-disjoint only; no finding-level split",
            "development_repository_count": len(DEV_REPOSITORIES),
            "blind": {
                "repository_count": len(private_repos),
                "repositories_per_language": {
                    language: BLIND_REPOS_PER_LANGUAGE
                    for language in SUPPORTED_LANGUAGES
                },
                "changes_per_repository_cap": BLIND_CHANGES_PER_REPO,
                "selected_change_count": len(changes),
                "query_depth": replay_harness.QUERY_DEPTH,
                "eligible_pool_cap": replay_harness.ELIGIBLE_POOL_CAP,
                "min_changed_source_lines": replay_harness.MIN_CHANGED_SRC_LINES,
                "max_changed_source_lines": replay_harness.MAX_CHANGED_SRC_LINES,
                "rows": public_rows(changes),
            },
            "temporal_canary_reserve": {
                "repository_count": len(temporal),
                "repositories_per_language": count_by_language(temporal),
                "eligible_after": SEALED_AT,
                "target_change_count": CANARY_CHANGES,
                "rows": public_temporal_rows(root_seed, temporal),
            },
        },
        "privacy": {
            "root_seed_commitment": header["root_seed_commitment"],
            "private_packet_bytes": packet_path.stat().st_size,
            "private_packet_sha256": sha256_file(packet_path),
            "private_packet_location": "outside repository; path deliberately undisclosed",
            "public_identity": "HMAC-derived opaque repository/change ids and row commitments",
            "quality_labels_available_to_implementation": False,
            "source_available_in_git": False,
        },
        "provenance": {
            "official_binary": {
                "release_tag": OFFICIAL_RELEASE_TAG,
                "asset": OFFICIAL_RELEASE_ASSET,
                "version": version,
                "sha256": sha256_file(binary),
            },
            "corpus": checked_file(CORPUS_PATH),
            "prune_manifest": checked_file(PRUNE_PATH),
            "collector": checked_file(Path(__file__).resolve()),
            "replay_harness": checked_file(ROOT / "eval/divergence_fire/replay.py"),
            "freeze_parent": git("rev-parse", "HEAD").stdout.strip(),
            "working_tree_clean_before_freeze": True,
        },
    }
    validate_document(public)
    PUBLIC_PATH.write_text(json.dumps(public, indent=2, sort_keys=True) + "\n")
    digest = sha256_file(PUBLIC_PATH)
    SIDECAR_PATH.write_text(f"{digest}  {PUBLIC_PATH.name}\n")
    print(json.dumps({
        "public": str(PUBLIC_PATH.relative_to(ROOT)),
        "private_packet_bytes": packet_path.stat().st_size,
        "blind_repositories": len(private_repos),
        "blind_changes": len(changes),
        "temporal_repositories": len(temporal),
        "root_seed_commitment": header["root_seed_commitment"],
    }, indent=2))


def exact_keys(value: dict[str, Any], keys: set[str], label: str) -> None:
    require(set(value) == keys, f"{label} keys")


def assert_no_public_source(rows: list[dict[str, Any]], label: str) -> None:
    for index, row in enumerate(rows):
        require(not (set(row) & FORBIDDEN_PUBLIC_KEYS), f"{label}[{index}] source key")
        require(HEX40.search(json.dumps(row, sort_keys=True)) is None,
                f"{label}[{index}] raw commit")


def validate_document(document: dict[str, Any]) -> None:
    exact_keys(document, {
        "schema", "sealed_at", "state", "development_baseline", "protocol",
        "sampling_stop_rule", "decision_matrix", "verdict_protocol",
        "population", "privacy", "provenance",
    }, "protocol document")
    require(document.get("schema") == SCHEMA, "protocol schema")
    require(document.get("sealed_at") == SEALED_AT, "sealed timestamp")
    require(document.get("state") == "sealed-unjudged", "protocol state")
    require(document.get("development_baseline") == development_baseline(),
            "development baseline drift")
    require(document.get("protocol") == protocol_contract(), "protocol drift")
    require(document.get("sampling_stop_rule") == stop_rule(), "stop rule drift")
    require(document.get("decision_matrix") == decision_matrix(), "decision matrix drift")
    require(document.get("verdict_protocol") == verdict_protocol(), "verdict protocol drift")

    population = document.get("population") or {}
    require(population.get("split_rule") ==
            "repository-disjoint only; no finding-level split", "split rule")
    require(population.get("development_repository_count") == len(DEV_REPOSITORIES),
            "development repository count")
    blind = population.get("blind") or {}
    expected_blind_repos = len(SUPPORTED_LANGUAGES) * BLIND_REPOS_PER_LANGUAGE
    require(blind.get("repository_count") == expected_blind_repos,
            "blind repository count")
    require(blind.get("repositories_per_language") == {
        language: BLIND_REPOS_PER_LANGUAGE for language in SUPPORTED_LANGUAGES
    }, "blind language balance")
    require(blind.get("changes_per_repository_cap") == BLIND_CHANGES_PER_REPO,
            "blind change cap")
    rows = blind.get("rows") or []
    require(len(rows) == blind.get("selected_change_count"), "blind row count")
    require(rows and len(rows) <= expected_blind_repos * BLIND_CHANGES_PER_REPO,
            "blind population size")
    require(len(rows) >= TARGET_STRICT_MINIMUM, "blind population support")
    require([row.get("ordinal") for row in rows] == list(range(len(rows))),
            "blind row order")
    require(len({row.get("opaque_change_id") for row in rows}) == len(rows),
            "blind opaque ids")
    require(len({row.get("commitment") for row in rows}) == len(rows),
            "blind commitments")
    for row in rows:
        exact_keys(row, {
            "ordinal", "opaque_change_id", "opaque_repository_id", "commitment",
        }, "blind public row")
        for field in ("opaque_change_id", "opaque_repository_id", "commitment"):
            require(re.fullmatch(r"[0-9a-f]{64}", row.get(field, "")),
                    f"blind {field}")
    assert_no_public_source(rows, "blind rows")

    temporal = population.get("temporal_canary_reserve") or {}
    expected_temporal = len(SUPPORTED_LANGUAGES) * TEMPORAL_REPOS_PER_LANGUAGE
    require(temporal.get("repository_count") == expected_temporal,
            "temporal repository count")
    require(temporal.get("repositories_per_language") == {
        language: TEMPORAL_REPOS_PER_LANGUAGE for language in SUPPORTED_LANGUAGES
    }, "temporal language balance")
    require(temporal.get("eligible_after") == SEALED_AT, "temporal cutoff")
    require(temporal.get("target_change_count") == CANARY_CHANGES,
            "temporal change target")
    temporal_rows = temporal.get("rows") or []
    require(len(temporal_rows) == expected_temporal, "temporal rows")
    require(len({row.get("opaque_repository_id") for row in temporal_rows})
            == expected_temporal, "temporal opaque ids")
    for row in temporal_rows:
        exact_keys(row, {"opaque_repository_id", "commitment"},
                   "temporal public row")
        require(re.fullmatch(r"[0-9a-f]{64}", row.get("opaque_repository_id", "")),
                "temporal opaque id")
        require(re.fullmatch(r"[0-9a-f]{64}", row.get("commitment", "")),
                "temporal commitment")
    assert_no_public_source(temporal_rows, "temporal rows")

    privacy = document.get("privacy") or {}
    require(privacy.get("quality_labels_available_to_implementation") is False,
            "quality data privacy")
    require(privacy.get("source_available_in_git") is False, "source privacy")
    require(re.fullmatch(r"[0-9a-f]{64}", privacy.get("root_seed_commitment", "")),
            "root seed commitment")
    require(re.fullmatch(r"[0-9a-f]{64}", privacy.get("private_packet_sha256", "")),
            "private packet sha")
    require(privacy.get("private_packet_bytes", 0) > 0, "private packet bytes")

    provenance = document.get("provenance") or {}
    exact_keys(provenance, {
        "official_binary", "corpus", "prune_manifest", "collector",
        "replay_harness", "freeze_parent", "working_tree_clean_before_freeze",
    }, "provenance")
    require(provenance.get("official_binary") == {
        "release_tag": OFFICIAL_RELEASE_TAG,
        "asset": OFFICIAL_RELEASE_ASSET,
        "version": OFFICIAL_BINARY_VERSION,
        "sha256": OFFICIAL_BINARY_SHA256,
    }, "official binary provenance")
    require(provenance.get("corpus") == checked_file(CORPUS_PATH), "corpus provenance")
    require(provenance.get("prune_manifest") == checked_file(PRUNE_PATH),
            "prune provenance")
    require(provenance.get("collector") == checked_file(Path(__file__).resolve()),
            "collector provenance")
    require(provenance.get("replay_harness") ==
            checked_file(ROOT / "eval/divergence_fire/replay.py"),
            "replay provenance")
    require(provenance.get("working_tree_clean_before_freeze") is True,
            "freeze cleanliness")
    freeze_parent = provenance.get("freeze_parent", "")
    require(re.fullmatch(r"[0-9a-f]{40}", freeze_parent), "freeze parent")
    git("cat-file", "-e", f"{freeze_parent}^{{commit}}")
    require(git("merge-base", "--is-ancestor", freeze_parent, "HEAD", check=False).returncode
            == 0, "freeze parent ancestry")
    for field in ("collector", "replay_harness"):
        checked = provenance[field]
        historical_blob = git(
            "rev-parse", f"{freeze_parent}:{checked['path']}"
        ).stdout.strip()
        require(historical_blob == checked["git_blob"],
                f"{field} freeze-parent blob")


def validate_public() -> None:
    require(PUBLIC_PATH.is_file(), "missing public precision protocol")
    require(SIDECAR_PATH.is_file(), "missing precision protocol sidecar")
    expected_sidecar = f"{sha256_file(PUBLIC_PATH)}  {PUBLIC_PATH.name}\n"
    require(SIDECAR_PATH.read_text() == expected_sidecar, "protocol sidecar")
    validate_document(json.loads(PUBLIC_PATH.read_text()))


def read_seed(private_dir: Path) -> bytes:
    path = private_dir / PRIVATE_SEED
    require(path.is_file() and not path.is_symlink(), "private seed")
    require(stat.S_IMODE(path.stat().st_mode) & 0o077 == 0, "private seed permissions")
    return bytes.fromhex(path.read_text().strip())


def load_private_packet(path: Path) -> tuple[dict[str, Any], list[dict[str, Any]]]:
    rows = [json.loads(line) for line in path.read_text().splitlines() if line]
    require(rows and rows[0].get("record_type") == "header", "private packet header")
    return rows[0], rows[1:]


def validate_private(args: argparse.Namespace) -> None:
    validate_public()
    private_dir = args.private_dir.resolve()
    require(private_dir != ROOT and ROOT not in private_dir.parents,
            "private directory must be outside repository")
    seed = read_seed(private_dir)
    public = json.loads(PUBLIC_PATH.read_text())
    packet_path = private_dir / PRIVATE_PACKET
    private_manifest_path = private_dir / PRIVATE_MANIFEST
    require(packet_path.is_file() and private_manifest_path.is_file(), "private files")
    header, changes = load_private_packet(packet_path)
    manifest = json.loads(private_manifest_path.read_text())
    require(header.get("schema") == PRIVATE_SCHEMA, "private schema")
    require(manifest.get("schema") == PRIVATE_MANIFEST_SCHEMA, "private manifest schema")
    root_commitment = sha256_bytes(b"nose-848-root\0" + seed)
    require(header.get("root_seed_commitment") == root_commitment,
            "private root commitment")
    require(public["privacy"]["root_seed_commitment"] == root_commitment,
            "public root commitment")
    require(public["privacy"]["private_packet_sha256"] == sha256_file(packet_path),
            "private packet sha")
    require(public["privacy"]["private_packet_bytes"] == packet_path.stat().st_size,
            "private packet bytes")
    require(manifest.get("packet") == {
        "name": PRIVATE_PACKET,
        "bytes": packet_path.stat().st_size,
        "sha256": sha256_file(packet_path),
    }, "private manifest packet")

    blind, temporal = repository_partitions(seed)
    require([row["repository"] for row in manifest["temporal_reserve"]]
            == [row["id"] for row in temporal], "temporal selection")
    private_repos, reproduced = collect_private_population(seed, args.repos_root, blind)
    require(private_repos == manifest.get("blind_repositories"),
            "blind repository replay")
    require(reproduced == changes, "private population replay")
    require(public_rows(reproduced) == public["population"]["blind"]["rows"],
            "public blind projection")
    require(public_temporal_rows(seed, temporal) ==
            public["population"]["temporal_canary_reserve"]["rows"],
            "public temporal projection")
    print(f"private precision protocol OK: {len(private_repos)} repos, {len(changes)} changes")


def expect_failure(document: dict[str, Any], label: str) -> None:
    try:
        validate_document(document)
    except AssertionError:
        return
    raise AssertionError(f"mutation accepted: {label}")


def selftest_embedded() -> None:
    require(abs(wilson_lower(45, 80) - 0.4707078148504071) < 1e-12,
            "Wilson development fixture")
    require(wilson_lower(0, 0) is None, "Wilson zero denominator")
    require(wilson_lower(100, 100) < 1.0, "Wilson perfect lower bound")
    if not PUBLIC_PATH.exists():
        return
    document = json.loads(PUBLIC_PATH.read_text())
    validate_document(document)
    mutations = []
    mutated = copy.deepcopy(document)
    mutated["sampling_stop_rule"]["minimum_strict_targets"] = 99
    mutations.append((mutated, "stop rule"))
    mutated = copy.deepcopy(document)
    mutated["decision_matrix"]["blind_policy_gate"]["strict_target_precision_min"] = 0.94
    mutations.append((mutated, "target threshold"))
    mutated = copy.deepcopy(document)
    mutated["verdict_protocol"]["state"] = "labeled"
    mutations.append((mutated, "verdict state"))
    mutated = copy.deepcopy(document)
    mutated["population"]["blind"]["rows"][0]["commitment"] = "0" * 64
    mutations.append((mutated, "row commitment"))
    mutated = copy.deepcopy(document)
    mutated["privacy"]["quality_labels_available_to_implementation"] = True
    mutations.append((mutated, "quality leak"))
    mutated = copy.deepcopy(document)
    mutated["provenance"]["official_binary"]["sha256"] = "0" * 64
    mutations.append((mutated, "binary identity"))
    for mutated, label in mutations:
        expect_failure(mutated, label)


def self_test() -> None:
    selftest_embedded()
    if PUBLIC_PATH.exists():
        validate_public()
        print("precision protocol self-test OK")
    else:
        print("precision protocol utility self-test OK (public seal not frozen yet)")


def main() -> None:
    parser = argparse.ArgumentParser(description=__doc__)
    sub = parser.add_subparsers(dest="command", required=True)

    freeze_parser = sub.add_parser("freeze")
    freeze_parser.add_argument("--private-dir", type=Path, required=True)
    freeze_parser.add_argument("--repos-root", type=Path, default=ROOT / "bench/repos")
    freeze_parser.add_argument("--nose", type=Path, required=True)
    freeze_parser.set_defaults(function=freeze)

    validate_parser = sub.add_parser("validate")
    validate_parser.set_defaults(function=lambda _args: validate_public())

    private_parser = sub.add_parser("validate-private")
    private_parser.add_argument("--private-dir", type=Path, required=True)
    private_parser.add_argument("--repos-root", type=Path, default=ROOT / "bench/repos")
    private_parser.set_defaults(function=validate_private)

    selftest_parser = sub.add_parser("self-test")
    selftest_parser.set_defaults(function=lambda _args: self_test())

    args = parser.parse_args()
    args.function(args)


if __name__ == "__main__":
    try:
        main()
    except AssertionError as error:
        sys.exit(str(error))
