#!/usr/bin/env python3
"""Collect and validate the checked, corpus-wide #842 behavior evidence."""

from __future__ import annotations

import argparse
import copy
import hashlib
import json
import subprocess
import sys
from collections import Counter
from pathlib import Path
from typing import Any


ROOT = Path(__file__).resolve().parents[2]
DEFAULT = ROOT / "bench/labels/generated_provenance_behavior_2026_07_13.dev.v1.json"
CORPUS = ROOT / "bench/goldens/corpus.json"
TAXONOMY = ROOT / "bench/labels/default_head_taxonomy_2026_07_13.dev.v1.json"
CORPUS_SHA = "87b3defc02c87e53f5ce20d10b68afdbc7190a6db5d5bfdb6b655b305bbc7ba8"
TAXONOMY_FILE_SHA = "68eff466212f0322a45a16648c1fcfd51a301bd5351c93f0795147f2baa33969"
TAXONOMY_SEMANTIC_SHA = "206f7e6c2eb9e5bb3750dd6e12f6f920228d719868b2e4922395433a2315c71a"
PARENT_COMMIT = "bf6298ad"
PARENT_BINARY_SHA = "5424827feb55c997c2cabd8bdb1ea445747457f8f6f93c4b8889ce2593efe642"
CURRENT_COMMIT = "1f5d6b450a2a68b1382e6ce843843fe8f195c898"
CURRENT_BINARY_SHA = "6d906e88270994a6ac2589977b2ce9b7616788c1bba67f9dc1b66791161de3dc"
# A canonical digest over the entire artifact except this field. Updating the
# evidence requires an explicit, reviewable rebind here; internally coherent
# summary substitutions cannot silently pass validation.
EXPECTED_EVIDENCE_DIGEST = "17158a23270a2ba902dfd58b916b0f0720f9bbaffbe9760cf52bf732cecef6a8"


def fail(message: str) -> None:
    raise SystemExit(message)


def require(condition: bool, message: str) -> None:
    if not condition:
        fail(message)


def sha256_bytes(value: bytes) -> str:
    return hashlib.sha256(value).hexdigest()


def sha256_file(path: Path) -> str:
    return sha256_bytes(path.read_bytes())


def canonical(value: Any) -> bytes:
    return json.dumps(value, sort_keys=True, separators=(",", ":"), ensure_ascii=False).encode()


def digest(value: Any) -> str:
    return sha256_bytes(canonical(value))


def load(path: Path) -> dict[str, Any]:
    try:
        value = json.loads(path.read_text(encoding="utf-8"))
    except (OSError, json.JSONDecodeError) as error:
        fail(f"{path}: {error}")
    require(isinstance(value, dict), f"{path}: expected a JSON object")
    return value


def dev_repositories() -> list[dict[str, str]]:
    require(sha256_file(CORPUS) == CORPUS_SHA, "corpus manifest SHA-256 changed")
    repos = [
        {"repo": row["id"], "commit": row["commit"]}
        for row in load(CORPUS)["repositories"]
        if row["split"] == "dev"
    ]
    require(len(repos) == 66, "expected exactly 66 dev repositories")
    return repos


def generated_lever() -> dict[str, Any]:
    require(sha256_file(TAXONOMY) == TAXONOMY_FILE_SHA, "#841 taxonomy file SHA-256 changed")
    taxonomy = load(TAXONOMY)
    require(taxonomy.get("artifact_sha256") == TAXONOMY_SEMANTIC_SHA, "#841 taxonomy semantic binding changed")
    return next(row for row in taxonomy["lever_decisions"] if row["lever_id"] == "generated-provenance.v1")


def query(binary: Path, repo: str, mode: str) -> tuple[bytes, list[dict[str, Any]]]:
    args = [str(binary), "query", f"bench/repos/{repo}"]
    if mode == "expanded":
        args += ["all", "top=0"]
    elif mode == "default":
        args += ["top=30"]
    elif mode == "semantic":
        args += ["all", "top=0", "--mode", "semantic"]
    else:
        raise AssertionError(mode)
    args += ["--format", "json"]
    run = subprocess.run(args, cwd=ROOT, check=True, capture_output=True)
    report = json.loads(run.stdout)
    return run.stdout, report["families"]


def without_surface(families: list[dict[str, Any]]) -> list[dict[str, Any]]:
    projected = copy.deepcopy(families)
    for family in projected:
        family.pop("surface", None)
    return projected


def comparison(parent_stdout: bytes, parent: list[dict[str, Any]], current_stdout: bytes, current: list[dict[str, Any]]) -> dict[str, Any]:
    parent_ids = [row["id"] for row in parent]
    current_ids = [row["id"] for row in current]
    transitions = Counter(
        f"{before['surface']}->{after['surface']}"
        for before, after in zip(parent, current, strict=True)
        if before["surface"] != after["surface"]
    ) if parent_ids == current_ids else Counter()
    return {
        "parent_stdout_sha256": sha256_bytes(parent_stdout),
        "current_stdout_sha256": sha256_bytes(current_stdout),
        "parent_families": len(parent),
        "current_families": len(current),
        "parent_id_order_sha256": digest(parent_ids),
        "current_id_order_sha256": digest(current_ids),
        "parent_non_surface_sha256": digest(without_surface(parent)),
        "current_non_surface_sha256": digest(without_surface(current)),
        "surface_transitions": dict(sorted(transitions.items())),
    }


def default_comparison(parent_stdout: bytes, parent: list[dict[str, Any]], current_stdout: bytes, current: list[dict[str, Any]]) -> dict[str, Any]:
    parent_ids = [row["id"] for row in parent]
    current_ids = [row["id"] for row in current]
    return {
        "parent_stdout_sha256": sha256_bytes(parent_stdout),
        "current_stdout_sha256": sha256_bytes(current_stdout),
        "parent_ids": parent_ids,
        "current_ids": current_ids,
        "removed": [value for value in parent_ids if value not in current_ids],
        "added": [value for value in current_ids if value not in parent_ids],
    }


def parse_position_key(value: str) -> tuple[str, str]:
    parts = value.split(":")
    return parts[-3], parts[-2]


def cohort_rows(keys: list[str], expanded: dict[str, list[dict[str, Any]]]) -> list[dict[str, str]]:
    rows = []
    for key in keys:
        repo, family_id = parse_position_key(key)
        family = next((row for row in expanded[repo] if row["id"] == family_id), None)
        require(family is not None, f"{key}: family missing from current expanded output")
        rows.append({"position_key": key, "surface": family["surface"]})
    return rows


def summarize(rows: list[dict[str, Any]], field: str) -> dict[str, Any]:
    changed = [row["repo"] for row in rows if row[field]["parent_stdout_sha256"] != row[field]["current_stdout_sha256"]]
    transitions = Counter()
    for row in rows:
        transitions.update(row[field]["surface_transitions"])
    return {
        "repositories": len(rows),
        "families_before": sum(row[field]["parent_families"] for row in rows),
        "families_after": sum(row[field]["current_families"] for row in rows),
        "byte_identical_repositories": len(rows) - len(changed),
        "changed_repositories": changed,
        "family_id_order_equal": all(
            row[field]["parent_id_order_sha256"] == row[field]["current_id_order_sha256"] for row in rows
        ),
        "non_surface_fields_equal": all(
            row[field]["parent_non_surface_sha256"] == row[field]["current_non_surface_sha256"] for row in rows
        ),
        "surface_transitions": dict(sorted(transitions.items())),
    }


def collect(parent_binary: Path, current_binary: Path, output: Path) -> None:
    require(sha256_file(parent_binary) == PARENT_BINARY_SHA, "wrong parent binary")
    require(sha256_file(current_binary) == CURRENT_BINARY_SHA, "wrong current binary")
    repos = dev_repositories()
    rows = []
    current_expanded: dict[str, list[dict[str, Any]]] = {}
    for index, repo_row in enumerate(repos, 1):
        repo = repo_row["repo"]
        print(f"[{index:02d}/66] {repo}", file=sys.stderr, flush=True)
        p_all_raw, p_all = query(parent_binary, repo, "expanded")
        c_all_raw, c_all = query(current_binary, repo, "expanded")
        p_default_raw, p_default = query(parent_binary, repo, "default")
        c_default_raw, c_default = query(current_binary, repo, "default")
        p_sem_raw, p_sem = query(parent_binary, repo, "semantic")
        c_sem_raw, c_sem = query(current_binary, repo, "semantic")
        current_expanded[repo] = c_all
        rows.append({
            **repo_row,
            "expanded": comparison(p_all_raw, p_all, c_all_raw, c_all),
            "default_top30": default_comparison(p_default_raw, p_default, c_default_raw, c_default),
            "semantic": comparison(p_sem_raw, p_sem, c_sem_raw, c_sem),
        })
    lever = generated_lever()
    artifact = {
        "schema": "nose.generated_provenance_behavior.v1",
        "issue": 842,
        "split": "dev",
        "heldout_policy": "closed; no held-out checkout or judgment was opened",
        "parent": {"commit": PARENT_COMMIT, "binary_sha256": PARENT_BINARY_SHA},
        "current": {"commit": CURRENT_COMMIT, "binary_sha256": CURRENT_BINARY_SHA},
        "corpus": {"manifest": "bench/goldens/corpus.json", "manifest_sha256": CORPUS_SHA, "repositories": repos},
        "taxonomy": {
            "path": "bench/labels/default_head_taxonomy_2026_07_13.dev.v1.json",
            "file_sha256": TAXONOMY_FILE_SHA,
            "artifact_sha256": TAXONOMY_SEMANTIC_SHA,
        },
        "commands": {
            "expanded": "nose query bench/repos/<repo> all top=0 --format json",
            "default_top30": "nose query bench/repos/<repo> top=30 --format json",
            "semantic": "nose query bench/repos/<repo> all top=0 --mode semantic --format json",
        },
        "expanded_summary": summarize(rows, "expanded"),
        "semantic_summary": summarize(rows, "semantic"),
        "default_changed_repositories": [
            row["repo"] for row in rows
            if row["default_top30"]["parent_stdout_sha256"] != row["default_top30"]["current_stdout_sha256"]
        ],
        "cohorts": {
            "head_positives": cohort_rows(lever["positive_position_keys"], current_expanded),
            "deep_audit_positives": cohort_rows(lever["audit_packet_keys"], current_expanded),
            "html_hard_negatives": cohort_rows(lever["hard_negative_position_keys"], current_expanded),
        },
        "rows": rows,
    }
    artifact["evidence_digest"] = digest(artifact)
    output.write_text(json.dumps(artifact, indent=2, sort_keys=True) + "\n", encoding="utf-8")
    print(f"wrote {output} ({artifact['evidence_digest']})")


def validate(path: Path) -> None:
    artifact = load(path)
    bound = artifact.pop("evidence_digest", None)
    require(bound == EXPECTED_EVIDENCE_DIGEST, "behavior artifact digest is not the reviewed binding")
    require(digest(artifact) == bound, "behavior artifact contents do not match its digest")
    require(artifact["schema"] == "nose.generated_provenance_behavior.v1", "wrong behavior schema")
    require(artifact["issue"] == 842 and artifact["split"] == "dev", "wrong behavior scope")
    require(artifact["parent"] == {"commit": PARENT_COMMIT, "binary_sha256": PARENT_BINARY_SHA}, "wrong parent role")
    require(artifact["current"] == {"commit": CURRENT_COMMIT, "binary_sha256": CURRENT_BINARY_SHA}, "wrong current role")
    require(artifact["corpus"]["manifest_sha256"] == CORPUS_SHA, "wrong corpus binding")
    require(artifact["corpus"]["repositories"] == dev_repositories(), "wrong corpus repository set")
    require(artifact["taxonomy"] == {
        "path": "bench/labels/default_head_taxonomy_2026_07_13.dev.v1.json",
        "file_sha256": TAXONOMY_FILE_SHA,
        "artifact_sha256": TAXONOMY_SEMANTIC_SHA,
    }, "wrong taxonomy binding")
    rows = artifact["rows"]
    require(artifact["expanded_summary"] == summarize(rows, "expanded"), "expanded summary is not derived from rows")
    require(artifact["semantic_summary"] == summarize(rows, "semantic"), "semantic summary is not derived from rows")
    expanded = artifact["expanded_summary"]
    semantic = artifact["semantic_summary"]
    require(expanded["families_before"] == expanded["families_after"] == 54754, "expanded family total changed")
    require(semantic["families_before"] == semantic["families_after"] == 9850, "semantic family total changed")
    for summary in (expanded, semantic):
        require(summary["byte_identical_repositories"] == 65, "unexpected drift breadth")
        require(summary["changed_repositories"] == ["alamofire"], "unexpected changed repository")
        require(summary["family_id_order_equal"], "family id order changed")
        require(summary["non_surface_fields_equal"], "non-surface fields changed")
    require(expanded["surface_transitions"] == {
        "default->generated": 387,
        "hidden->generated": 89,
        "shallow->generated": 31,
    }, "expanded surface transitions changed")
    require(semantic["surface_transitions"] == {
        "default->generated": 3326,
        "hidden->generated": 426,
    }, "semantic surface transitions changed")
    require(artifact["default_changed_repositories"] == ["alamofire"], "unexpected default-head drift")
    lever = generated_lever()
    expected_keys = {
        "head_positives": lever["positive_position_keys"],
        "deep_audit_positives": lever["audit_packet_keys"],
        "html_hard_negatives": lever["hard_negative_position_keys"],
    }
    for cohort, keys in expected_keys.items():
        rows = artifact["cohorts"][cohort]
        require([row["position_key"] for row in rows] == keys, f"{cohort}: taxonomy keys changed")
        expected_surface = "default" if cohort == "html_hard_negatives" else "generated"
        require(all(row["surface"] == expected_surface for row in rows), f"{cohort}: wrong surface")
    print(f"generated provenance behavior OK: {path.relative_to(ROOT)}")


def self_test() -> None:
    artifact = load(DEFAULT)
    original = artifact["evidence_digest"]
    mutated = copy.deepcopy(artifact)
    mutated["rows"][0]["expanded"]["current_families"] += 1
    mutated.pop("evidence_digest")
    require(digest(mutated) != original, "coherent-substitution mutation was not detected")
    validate(DEFAULT)
    print("generated provenance behavior self-test passed")


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--self-test", action="store_true")
    sub = parser.add_subparsers(dest="command")
    collect_parser = sub.add_parser("collect")
    collect_parser.add_argument("--parent-binary", type=Path, required=True)
    collect_parser.add_argument("--current-binary", type=Path, required=True)
    collect_parser.add_argument("--output", type=Path, default=DEFAULT)
    validate_parser = sub.add_parser("validate")
    validate_parser.add_argument("artifact", nargs="?", type=Path, default=DEFAULT)
    args = parser.parse_args()
    if args.self_test:
        self_test()
    elif args.command == "collect":
        collect(args.parent_binary.resolve(), args.current_binary.resolve(), args.output.resolve())
    elif args.command == "validate":
        validate(args.artifact.resolve())
    else:
        parser.error("choose collect or validate")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
