#!/usr/bin/env python3
"""Blind panel protocol for the frozen issue #845 residual-ranking frontier."""

from __future__ import annotations

import argparse
import copy
import hashlib
import json
import subprocess
from pathlib import Path
from typing import Any

import residual_ranking as ranking
import residual_ranking_topup as topup


ROOT = Path(__file__).resolve().parents[2]
SELECTION = topup.DEFAULT_SELECTION
BLIND = ROOT / "bench/labels/residual_ranking_topup_blind_2026_07_14.dev.v1.json"
SELECTION_COMMIT = "6e9a2d08903b34f35ef6e5e6f007b9185378dbc1"
SELECTION_TREE = "174680364bbeac0d693d5b67c256402f5973aee3"
SELECTION_SHA256 = "f3b4ec65f6b8d8a5d92282a11447aeb14a6a3f551e39d168c6e3bc6820da058f"
PERSONAS = ("dedupe", "pragmatic", "skeptic")
WORTHY_REASONS = {"extract-base", "extract-data-table", "extract-helper", "parameterize"}
NOT_WORTHY_REASONS = {
    "coincidental-shape",
    "generated",
    "parallel-by-design",
    "trivial",
    "type-def",
}
ALL_REASONS = WORTHY_REASONS | NOT_WORTHY_REASONS


def read_json(path: Path) -> dict[str, Any]:
    value = json.loads(path.read_text(encoding="utf-8"))
    if not isinstance(value, dict):
        raise ValueError(f"{path}: expected an object")
    return value


def canonical_bytes(value: object) -> bytes:
    return json.dumps(value, sort_keys=True, separators=(",", ":")).encode()


def canonical_sha256(value: object) -> str:
    return hashlib.sha256(canonical_bytes(value)).hexdigest()


def sha256_file(path: Path) -> str:
    return hashlib.sha256(path.read_bytes()).hexdigest()


def path_record(path: Path) -> dict[str, str]:
    return {"path": path.relative_to(ROOT).as_posix(), "sha256": sha256_file(path)}


def require_equal(actual: object, expected: object, label: str) -> None:
    if actual != expected:
        raise ValueError(f"{label}: mismatch")


def require_exact_keys(value: object, expected: set[str], label: str) -> dict[str, Any]:
    if not isinstance(value, dict) or set(value) != expected:
        raise ValueError(f"{label}: expected exact keys {sorted(expected)}")
    return value


def git_bytes(args: list[str]) -> bytes:
    return subprocess.run(
        ["git", *args], cwd=ROOT, check=True, capture_output=True
    ).stdout


def selection_receipt() -> dict[str, str]:
    return {
        "commit": SELECTION_COMMIT,
        "tree": SELECTION_TREE,
        "path": SELECTION.relative_to(ROOT).as_posix(),
        "sha256": SELECTION_SHA256,
    }


def validate_selection_receipt() -> None:
    require_equal(
        git_bytes(["rev-parse", f"{SELECTION_COMMIT}^{{tree}}"]).decode().strip(),
        SELECTION_TREE,
        "selection tree",
    )
    frozen = git_bytes(["show", f"{SELECTION_COMMIT}:{SELECTION.relative_to(ROOT).as_posix()}"])
    require_equal(hashlib.sha256(frozen).hexdigest(), SELECTION_SHA256, "selection blob")
    require_equal(sha256_file(SELECTION), SELECTION_SHA256, "current selection bytes")
    subprocess.run(
        ["git", "merge-base", "--is-ancestor", SELECTION_COMMIT, "HEAD"],
        cwd=ROOT,
        check=True,
    )


def blind_candidate(candidate: dict[str, Any]) -> dict[str, Any]:
    return {
        "candidate_key": candidate["candidate_key"],
        "repo": candidate["repo"],
        "split": candidate["split"],
        "language": candidate["language"],
        "raw_family_sha256": candidate["raw_family_sha256"],
        "raw_family": candidate["raw_family"],
    }


def build_blind(selection: dict[str, Any]) -> dict[str, Any]:
    topup.validate_payload(selection)
    return {
        "schema": "nose.residual_ranking_blind_panel.v1",
        "issue": 845,
        "split": "dev",
        "heldout_policy": ranking.HELDOUT_POLICY,
        "selection_receipt": selection_receipt(),
        "source_artifact": path_record(SELECTION),
        "rubric": path_record(topup.RUBRIC),
        "blinding": {
            "hidden_fields": ["current_rank", "proposal_membership", "truth_status"],
            "visible_fields": [
                "candidate_key",
                "repo",
                "split",
                "language",
                "raw_family_sha256",
                "raw_family",
            ],
        },
        "source_files": selection["source_files"],
        "candidates": [blind_candidate(candidate) for candidate in selection["candidates"]],
    }


def freeze_blind(args: argparse.Namespace) -> None:
    validate_selection_receipt()
    payload = build_blind(read_json(SELECTION))
    args.output.write_text(
        json.dumps(payload, sort_keys=True, separators=(",", ":")) + "\n",
        encoding="utf-8",
    )
    print(f"blind candidates={len(payload['candidates'])}")


def validate_blind_payload(payload: dict[str, Any], *, live_sources: bool = False) -> None:
    require_exact_keys(
        payload,
        {
            "schema",
            "issue",
            "split",
            "heldout_policy",
            "selection_receipt",
            "source_artifact",
            "rubric",
            "blinding",
            "source_files",
            "candidates",
        },
        "blind artifact",
    )
    require_equal(payload["schema"], "nose.residual_ranking_blind_panel.v1", "schema")
    require_equal(payload["issue"], 845, "issue")
    require_equal(payload["split"], "dev", "split")
    require_equal(payload["heldout_policy"], ranking.HELDOUT_POLICY, "held-out policy")
    validate_selection_receipt()
    require_equal(payload["selection_receipt"], selection_receipt(), "selection receipt")
    require_equal(payload["source_artifact"], path_record(SELECTION), "source artifact")
    require_equal(payload["rubric"], path_record(topup.RUBRIC), "rubric")
    expected = build_blind(read_json(SELECTION))
    require_equal(payload, expected, "blind projection")
    serialized = canonical_bytes(payload["candidates"])
    for hidden in payload["blinding"]["hidden_fields"]:
        if f'"{hidden}"'.encode() in serialized:
            raise ValueError(f"blind artifact leaks {hidden}")
    if live_sources:
        for record in payload["source_files"]:
            require_equal(sha256_file(ROOT / record["path"]), record["sha256"], record["path"])


def validate_blind(args: argparse.Namespace) -> None:
    validate_blind_payload(read_json(args.blind), live_sources=args.live_sources)
    print(f"validated {args.blind}")


def vote_path(persona: str) -> Path:
    return ROOT / f"bench/labels/residual_ranking_topup_votes_2026_07_14.dev.{persona}.v1.json"


def validate_vote_record(vote: object, candidate_key: str, label: str) -> None:
    row = require_exact_keys(vote, {"candidate_key", "worthy", "reason", "rationale"}, label)
    require_equal(row["candidate_key"], candidate_key, f"{label}.candidate_key")
    if not isinstance(row["worthy"], bool):
        raise ValueError(f"{label}.worthy: expected bool")
    if row["reason"] not in ALL_REASONS:
        raise ValueError(f"{label}.reason: invalid reason")
    require_equal(row["worthy"], row["reason"] in WORTHY_REASONS, f"{label}.reason polarity")
    if not isinstance(row["rationale"], str) or not row["rationale"].strip():
        raise ValueError(f"{label}.rationale: non-empty rationale required")


def validate_vote_payload(payload: dict[str, Any], persona: str) -> None:
    require_exact_keys(payload, {"schema", "persona", "source_artifact", "votes"}, "vote artifact")
    require_equal(payload["schema"], "nose.residual_ranking_panel_vote.v1", "vote schema")
    require_equal(payload["persona"], persona, "persona")
    require_equal(payload["source_artifact"], path_record(BLIND), "vote source")
    blind = read_json(BLIND)
    validate_blind_payload(blind)
    if not isinstance(payload["votes"], list) or len(payload["votes"]) != len(blind["candidates"]):
        raise ValueError("vote count mismatch")
    for index, (vote, candidate) in enumerate(zip(payload["votes"], blind["candidates"], strict=True)):
        validate_vote_record(vote, candidate["candidate_key"], f"votes[{index}]")


def validate_vote(args: argparse.Namespace) -> None:
    validate_vote_payload(read_json(args.vote), args.persona)
    print(f"validated {args.persona} vote: {args.vote}")


def self_test(args: argparse.Namespace) -> None:
    payload = read_json(args.blind)
    validate_blind_payload(payload)
    mutations = []
    changed = copy.deepcopy(payload)
    changed["candidates"][0]["current_rank"] = 1
    mutations.append(changed)
    changed = copy.deepcopy(payload)
    changed["candidates"] = changed["candidates"][1:]
    mutations.append(changed)
    changed = copy.deepcopy(payload)
    changed["selection_receipt"]["commit"] = "0" * 40
    mutations.append(changed)
    changed = copy.deepcopy(payload)
    changed["heldout_result"] = {"precision_at_10": 100}
    mutations.append(changed)
    for mutation in mutations:
        try:
            validate_blind_payload(mutation)
        except (ValueError, subprocess.CalledProcessError):
            continue
        raise AssertionError("invalid blind-panel mutation was accepted")
    print("residual-ranking blind-panel self-test passed")


def parser() -> argparse.ArgumentParser:
    root = argparse.ArgumentParser()
    commands = root.add_subparsers(dest="command", required=True)
    freeze_parser = commands.add_parser("freeze-blind")
    freeze_parser.add_argument("--output", type=Path, default=BLIND)
    freeze_parser.set_defaults(run=freeze_blind)
    validate_parser = commands.add_parser("validate-blind")
    validate_parser.add_argument("blind", type=Path, nargs="?", default=BLIND)
    validate_parser.add_argument("--live-sources", action="store_true")
    validate_parser.set_defaults(run=validate_blind)
    vote_parser = commands.add_parser("validate-vote")
    vote_parser.add_argument("--persona", choices=PERSONAS, required=True)
    vote_parser.add_argument("--vote", type=Path, required=True)
    vote_parser.set_defaults(run=validate_vote)
    self_parser = commands.add_parser("self-test")
    self_parser.add_argument("--blind", type=Path, default=BLIND)
    self_parser.set_defaults(run=self_test)
    return root


def main() -> None:
    args = parser().parse_args()
    args.run(args)


if __name__ == "__main__":
    main()
