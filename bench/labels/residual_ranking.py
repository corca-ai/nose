#!/usr/bin/env python3
"""Dev-only residual-ranking calibration and no-go validation for issue #845.

The tool intentionally accepts only the three explicit dev label components.  It does
not resolve the v7 composite manifest, a held-out seal, held-out labels, or held-out
repositories.  `collect` runs the current product over the complete dev default universe;
`freeze` reduces that raw output to the allowed runtime facts and evaluates a
pre-registered proposal family; `validate` reproduces the checked decision offline.
"""

from __future__ import annotations

import argparse
import copy
import hashlib
import json
import math
import subprocess
from concurrent.futures import ThreadPoolExecutor, as_completed
from dataclasses import asdict, dataclass, replace
from pathlib import Path
from typing import Any, Iterable


ROOT = Path(__file__).resolve().parents[2]
DEV_LABELS = (
    ROOT / "bench/labels/refactoring_families.v5.dev.json",
    ROOT / "bench/labels/refactoring_families.v6.dev.json",
    ROOT / "bench/labels/refactoring_families.v7.dev.json",
)
CORPUS = ROOT / "bench/goldens/corpus.json"
DEFAULT_ARTIFACT = (
    ROOT / "bench/labels/residual_ranking_calibration_2026_07_14.dev.v1.json"
)
PARENT_QUALITY = (
    ROOT / "bench/labels/declaration_type_contract_product_quality_2026_07_14.dev.v1.json"
)
EXPECTED_BASE_COMMIT = "a0b2730d4ee1f5393cc5a37ddd2b6c8b7a22b928"
EXPECTED_BASE_TREE = "399b5b816c4c19a61b549445111356921d69c3cb"
EXPECTED_BINARY_SHA256 = "f7fcda30aa63662f95000af7029eaf028c71ef074a18ba5e1e2048fe27c47fd0"
EXPECTED_BINARY_VERSION = "nose 0.19.0"
EXPECTED_DATASET_SHA256 = "dd8832fe094f97d85ab34a09af5adc7d7db3e763b53a562eb463465cd0de0299"
EXPECTED_COLLECTOR_COMMIT = "6e9a2d08903b34f35ef6e5e6f007b9185378dbc1"
EXPECTED_COLLECTOR_SHA256 = "6dd2ab081187ce92586eec44c88106a91d9af9818de1f2aee5da0b413af7595c"
EXPECTED_NEXT_STEP = (
    "Freeze and independently panel-label the complete unresolved top-10 union of the "
    "46 pre-registered formulas before deciding go or no-go; keep held-out closed."
)
LANGUAGES = ("C", "Go", "Java", "Python", "Ruby", "Rust", "Swift", "TypeScript")
FOLD_COUNT = 8

HELDOUT_POLICY = {
    "evaluation_runs": 0,
    "judgments_opened": False,
    "labels_opened": False,
    "repository_sources_opened": False,
    "status": "closed-until-issue-846",
}

CONTRACT = {
    "precision_at_10_min_pct": 70.0,
    "label_coverage_min_pct": 85.0,
    "language_precision_floor_pct": 50.0,
    "language_floor_min_positions": 30,
    "max_language_regression_pp": 5.0,
    "selection_unit": "repository",
    "runtime_forbidden_inputs": [
        "labels",
        "repository identifiers",
        "language-specific coefficients",
        "source/path/symbol allowlists",
        "scope=test",
        "JSX/presentational-shape",
    ],
}


@dataclass(frozen=True)
class Proposal:
    id: str
    spread_exponent: float = 1.0
    module_bonus_scale: float = 1.0
    same_symbol_weight: float = 1.0
    multi_module_non_same_weight: float = 1.0
    connected_weight: float = 1.0
    exact_weight: float = 1.0
    bounded_window_weight: float = 1.0
    implementation_type_weight: float = 1.0
    param_coefficient: float = 0.5
    tightness_exponent: float = 1.0
    homogeneity_exponent: float = 1.0


@dataclass(frozen=True)
class RepositoryRanking:
    hits: int
    matched: int
    reported: int
    top_keys: tuple[str, ...]


BASELINE = Proposal("current")


def proposal_family() -> tuple[Proposal, ...]:
    """The complete, pre-registered transparent search family.

    The first grid covers the three signals independently identified as the most
    promising by all reviewers.  The remaining one-axis ablations ensure the other
    issue-listed facts were tested without turning the exercise into coefficient mining.
    """
    proposals: list[Proposal] = []
    for spread in (-1.0, -0.5, 0.0, 0.5, 1.0):
        for same in (0.65, 0.80, 1.0):
            for connected in (1.0, 1.15):
                if spread == 1.0 and same == 1.0 and connected == 1.0:
                    # This is exactly BASELINE; a second ID would not be a second formula.
                    continue
                proposals.append(
                    Proposal(
                        id=f"grid-s{spread:+.2f}-same{same:.2f}-conn{connected:.2f}",
                        spread_exponent=spread,
                        same_symbol_weight=same,
                        connected_weight=connected,
                    )
                )
    proposals.extend(
        [
            replace(BASELINE, id="module-neutral", module_bonus_scale=0.0),
            replace(
                BASELINE,
                id="module-neutral-nonsame-075",
                module_bonus_scale=0.0,
                multi_module_non_same_weight=0.75,
            ),
            replace(
                BASELINE,
                id="module-neutral-nonsame-050",
                module_bonus_scale=0.0,
                multi_module_non_same_weight=0.50,
            ),
            replace(BASELINE, id="multi-module-nonsame-075", multi_module_non_same_weight=0.75),
            replace(BASELINE, id="multi-module-nonsame-050", multi_module_non_same_weight=0.50),
            replace(BASELINE, id="params-025", param_coefficient=0.25),
            replace(BASELINE, id="params-075", param_coefficient=0.75),
            replace(BASELINE, id="tightness-075", tightness_exponent=0.75),
            replace(BASELINE, id="tightness-125", tightness_exponent=1.25),
            replace(BASELINE, id="homogeneity-050", homogeneity_exponent=0.50),
            replace(BASELINE, id="homogeneity-150", homogeneity_exponent=1.50),
            replace(BASELINE, id="implementation-type-085", implementation_type_weight=0.85),
            replace(BASELINE, id="implementation-type-070", implementation_type_weight=0.70),
            replace(BASELINE, id="bounded-window-080", bounded_window_weight=0.80),
            replace(BASELINE, id="exact-115", exact_weight=1.15),
            Proposal(
                id="reviewer-composite",
                module_bonus_scale=0.0,
                multi_module_non_same_weight=0.80,
                connected_weight=1.25,
                homogeneity_exponent=1.50,
            ),
        ]
    )
    unique: dict[str, Proposal] = {proposal.id: proposal for proposal in proposals}
    unique[BASELINE.id] = BASELINE
    return tuple(unique[key] for key in sorted(unique))


PROPOSALS = proposal_family()


def assert_unique_proposals() -> None:
    ids = [proposal.id for proposal in PROPOSALS]
    formulas = [
        tuple(value for key, value in asdict(proposal).items() if key != "id")
        for proposal in PROPOSALS
    ]
    if len(ids) != 46 or len(set(ids)) != len(ids) or len(set(formulas)) != len(formulas):
        raise AssertionError("expected 46 unique proposal IDs and formulas")


assert_unique_proposals()


def read_json(path: Path) -> dict[str, Any]:
    return json.loads(path.read_text(encoding="utf-8"))


def sha256_bytes(data: bytes) -> str:
    return hashlib.sha256(data).hexdigest()


def sha256_file(path: Path) -> str:
    return sha256_bytes(path.read_bytes())


def canonical_bytes(value: object) -> bytes:
    return json.dumps(value, sort_keys=True, separators=(",", ":")).encode()


def canonical_sha256(value: object) -> str:
    return sha256_bytes(canonical_bytes(value))


def git_output(args: list[str], cwd: Path = ROOT) -> str:
    return subprocess.run(
        ["git", *args], cwd=cwd, check=True, capture_output=True, text=True
    ).stdout.strip()


def load_dev_labels() -> dict[str, list[dict[str, Any]]]:
    by_repo: dict[str, list[dict[str, Any]]] = {}
    identities: set[tuple[str, str]] = set()
    for path in DEV_LABELS:
        payload = read_json(path)
        families = payload.get("families")
        if not isinstance(families, list):
            raise ValueError(f"{path}: families must be an array")
        for row in families:
            if row.get("split") != "dev":
                raise ValueError(f"{path}: non-dev row rejected")
            identity = (row.get("repo"), row.get("family_id"))
            if not all(isinstance(value, str) and value for value in identity):
                raise ValueError(f"{path}: malformed label identity")
            if identity in identities:
                raise ValueError(f"{path}: duplicate label identity {identity}")
            identities.add(identity)
            eligibility = row.get("metric_eligibility")
            if eligibility is not None and "precision_at_10" not in eligibility:
                continue
            by_repo.setdefault(identity[0], []).append(row)
    return by_repo


def dev_repositories() -> dict[str, dict[str, Any]]:
    rows = read_json(CORPUS).get("repositories")
    if not isinstance(rows, list):
        raise ValueError("corpus repositories must be an array")
    return {row["id"]: row for row in rows if row.get("split") == "dev"}


def query_repo(nose: Path, repos_root: Path, repo: str, expected_commit: str) -> dict[str, Any]:
    checkout = repos_root / repo
    actual_commit = git_output(["rev-parse", "HEAD"], checkout)
    if actual_commit != expected_commit:
        raise ValueError(f"{repo}: expected {expected_commit}, got {actual_commit}")
    command = [
        str(nose.resolve()),
        "query",
        str(checkout),
        "top=0",
        "--format",
        "json",
    ]
    result = subprocess.run(command, cwd=ROOT, capture_output=True, text=True, timeout=300)
    if result.returncode != 0:
        raise ValueError(f"query failed for {repo}: {result.stderr.strip()}")
    payload = json.loads(result.stdout)
    families = payload.get("families")
    if not isinstance(families, list):
        raise ValueError(f"query output for {repo} has no families array")
    if any(family.get("surface") != "default" for family in families):
        raise ValueError(f"query output for {repo} contains a non-default family")
    return {
        "repo": repo,
        "command": command,
        "stdout_sha256": sha256_bytes(result.stdout.encode()),
        "families": families,
    }


def collect(args: argparse.Namespace) -> None:
    labels = load_dev_labels()
    corpus = dev_repositories()
    if set(labels) != set(corpus):
        raise ValueError("dev label repositories must exactly match the dev corpus")
    nose = args.nose.resolve()
    if sha256_file(nose) != EXPECTED_BINARY_SHA256:
        raise ValueError("collection binary is not the frozen #843 current binary")
    version = subprocess.run(
        [str(nose), "--version"], check=True, capture_output=True, text=True
    ).stdout.strip()
    rows: dict[str, dict[str, Any]] = {}
    with ThreadPoolExecutor(max_workers=args.jobs) as executor:
        pending = {
            executor.submit(
                query_repo, nose, args.repos_root, repo, corpus[repo]["commit"]
            ): repo
            for repo in sorted(labels)
        }
        for future in as_completed(pending):
            repo = pending[future]
            result = future.result()
            rows[repo] = {
                "language": corpus[repo]["primary_language"],
                "commit": corpus[repo]["commit"],
                "command": result["command"],
                "stdout_sha256": result["stdout_sha256"],
                "families": result["families"],
            }
            print(f"collected {repo}", flush=True)
    output = {
        "schema": "nose.residual_ranking_collection.v1",
        "split": "dev",
        "heldout_policy": HELDOUT_POLICY,
        "nose": {"path": str(nose), "sha256": sha256_file(nose), "version": version},
        "repositories": {repo: rows[repo] for repo in sorted(rows)},
    }
    args.output.parent.mkdir(parents=True, exist_ok=True)
    args.output.write_text(
        json.dumps(output, sort_keys=True, separators=(",", ":")) + "\n",
        encoding="utf-8",
    )


def overlaps(left: dict[str, Any], right: dict[str, Any]) -> bool:
    return left["file"] == right["file"] and not (
        left["end_line"] < right["start_line"]
        or right["end_line"] < left["start_line"]
    )


def locations(family: dict[str, Any]) -> list[dict[str, Any]]:
    return [
        {
            "file": location["file"],
            "start_line": location["start"],
            "end_line": location["end"],
        }
        for location in family["locations"]
    ]


def truth_resolution(
    family: dict[str, Any], labels: list[dict[str, Any]]
) -> tuple[bool | None, str]:
    sites = locations(family)
    best: list[dict[str, Any]] = []
    best_overlap = 0
    for label in labels:
        count = sum(
            1
            for site in sites
            for member in label["members"]
            if overlaps(site, member)
        )
        if count > best_overlap:
            best = [label]
            best_overlap = count
        elif count == best_overlap and count:
            best.append(label)
    if not best:
        return None, "unmatched"
    judgments = {bool(label["worthy"]) for label in best}
    if len(judgments) != 1:
        return None, "conflicting-best-overlap"
    return judgments.pop(), "matched"


def effective_copies(members: int) -> float:
    copies = max(0, members - 1)
    return float(copies) if copies <= 6 else 6.0 + math.sqrt(copies - 6.0)


def spread_parts(files: int, modules: int, languages: int) -> tuple[float, float, float]:
    return (
        0.30 * max(0, min(files, 8) - 1),
        0.50 * max(0, min(modules, 6) - 1),
        0.50 * max(0, languages - 1),
    )


def span_homogeneity(family: dict[str, Any]) -> float:
    if family["languages"] > 1:
        return 1.0
    lengths = [location["end"] - location["start"] + 1 for location in family["locations"]]
    mean = sum(lengths) / len(lengths)
    variance = sum((length - mean) ** 2 for length in lengths) / len(lengths)
    return 1.0 / (1.0 + math.sqrt(variance) / mean)


def compact_family(
    family: dict[str, Any], labels: list[dict[str, Any]], current_rank: int
) -> dict[str, Any]:
    metrics = family["metrics"]
    file_bonus, module_bonus, language_bonus = spread_parts(
        metrics["files"], metrics["modules"], metrics["languages"]
    )
    design_spread = 1.0 + file_bonus + module_bonus + language_bonus
    raw_value = (
        metrics["mean_lines"]
        * effective_copies(metrics["members"])
        * metrics["mean_score"]
        * design_spread
    )
    discount = metrics["value"] / raw_value if raw_value else 1.0
    if metrics["languages"] > 1:
        extract_lines = metrics["mean_lines"] * metrics["mean_score"]
        tightness = max(0.0, min(1.0, (metrics["mean_lines"] - 3.0) / 6.0))
    else:
        extract_lines = metrics["shared_weight"]
        tightness = min(1.0, metrics["shared_weight"] / max(1, family["rep_lines"]))
    origins = [location.get("origin") for location in family["locations"]]
    implementation_type = bool(origins) and all(
        origin is not None and "implementation-type" in origin.get("domains", [])
        for origin in origins
    )
    anchor = family["locations"][0] if family["locations"] else {"file": "", "start": 0}
    family_key = f"{family['id']}@{anchor['file']}:{anchor['start']}#rank-{current_rank}"
    resolved_truth, truth_status = truth_resolution(family, labels)
    return {
        "anchor": [anchor["file"], anchor["start"]],
        "base_core": extract_lines * effective_copies(metrics["members"]) * discount,
        "file_bonus": file_bonus,
        "homogeneity": span_homogeneity(family),
        "id": family["id"],
        "implementation_type": implementation_type,
        "key": family_key,
        "language_bonus": language_bonus,
        "module_bonus": module_bonus,
        "modules": metrics["modules"],
        "params": metrics["params"],
        "current_rank": current_rank,
        "same_symbol": family["same_symbol"],
        "tightness": tightness,
        "truth": resolved_truth,
        "truth_status": truth_status,
        "value": family["value"],
        "witness": family.get("witness"),
    }


def proposal_score(family: dict[str, Any], proposal: Proposal) -> float:
    design_spread = (
        1.0
        + family["file_bonus"]
        + proposal.module_bonus_scale * family["module_bonus"]
        + family["language_bonus"]
    )
    score = (
        family["base_core"]
        * design_spread**proposal.spread_exponent
        * (1.0 / (1.0 + proposal.param_coefficient * family["params"]))
        * family["tightness"] ** proposal.tightness_exponent
        * family["homogeneity"] ** proposal.homogeneity_exponent
    )
    if family["same_symbol"]:
        score *= proposal.same_symbol_weight
    if family["modules"] > 1 and not family["same_symbol"]:
        score *= proposal.multi_module_non_same_weight
    if family["witness"] == "connected":
        score *= proposal.connected_weight
    if family["witness"] == "exact":
        score *= proposal.exact_weight
    if family["witness"] == "bounded-window":
        score *= proposal.bounded_window_weight
    if family["implementation_type"]:
        score *= proposal.implementation_type_weight
    if not math.isfinite(score) or score < 0:
        raise ValueError(f"invalid score for {family['id']}: {score}")
    return score


def empty_counts() -> dict[str, int]:
    return {"hits": 0, "matched": 0, "reported": 0}


def add_counts(left: dict[str, int], right: dict[str, int]) -> None:
    for key in left:
        left[key] += right[key]


def rank_repository(row: dict[str, Any], proposal: Proposal) -> RepositoryRanking:
    top = order_families(row["families"], proposal)[:10]
    return RepositoryRanking(
        hits=sum(family["truth"] is True for family in top),
        matched=sum(family["truth"] is not None for family in top),
        reported=len(top),
        top_keys=tuple(family["key"] for family in top),
    )


def precompute_rankings(
    dataset: dict[str, Any],
) -> dict[str, dict[str, RepositoryRanking]]:
    """Rank each repository/proposal pair once for full and fold aggregation."""
    return {
        proposal.id: {
            repo: rank_repository(row, proposal)
            for repo, row in sorted(dataset["repositories"].items())
        }
        for proposal in PROPOSALS
    }


def metrics_for(
    dataset: dict[str, Any],
    proposal: Proposal,
    repositories: Iterable[str] | None = None,
    *,
    rankings: dict[str, dict[str, RepositoryRanking]] | None = None,
) -> dict[str, Any]:
    selected = set(dataset["repositories"]) if repositories is None else set(repositories)
    aggregate = empty_counts()
    languages = {language: empty_counts() for language in LANGUAGES}
    per_repository: dict[str, dict[str, Any]] = {}
    for repo in sorted(selected):
        row = dataset["repositories"][repo]
        ranked = (
            rank_repository(row, proposal)
            if rankings is None
            else rankings[proposal.id][repo]
        )
        counts = {
            "hits": ranked.hits,
            "matched": ranked.matched,
            "reported": ranked.reported,
        }
        add_counts(aggregate, counts)
        add_counts(languages[row["language"]], counts)
        per_repository[repo] = {"counts": counts, "top_keys": list(ranked.top_keys)}
    return {
        "overall": metric_record(aggregate),
        "languages": {
            language: metric_record(counts)
            for language, counts in languages.items()
            if counts["reported"]
        },
        "repositories": per_repository,
    }


def order_families(
    families: Iterable[dict[str, Any]], proposal: Proposal
) -> list[dict[str, Any]]:
    """Reproduce current exactly and give every experimental score a strict order."""
    if proposal.id == BASELINE.id:
        return sorted(families, key=lambda family: family["current_rank"])
    return sorted(
        families,
        key=lambda family: (
            -proposal_score(family, proposal),
            -family["value"],
            family["anchor"][0],
            family["anchor"][1],
            family["current_rank"],
            family["key"],
        ),
    )


def metric_record(counts: dict[str, int]) -> dict[str, Any]:
    hits, matched, reported = counts["hits"], counts["matched"], counts["reported"]
    return {
        **counts,
        "precision_pct": round(100.0 * hits / matched if matched else 0.0, 4),
        "coverage_pct": round(100.0 * matched / reported if reported else 0.0, 4),
        "slot_yield_pct": round(100.0 * hits / reported if reported else 0.0, 4),
        "best_case_slot_precision_pct": round(
            100.0 * (hits + reported - matched) / reported if reported else 0.0, 4
        ),
    }


def candidate_eligible(result: dict[str, Any], baseline: dict[str, Any]) -> bool:
    if not coverage_regression_eligible(result, baseline):
        return False
    for record in result["languages"].values():
        if (
            record["reported"] >= CONTRACT["language_floor_min_positions"]
            and not ratio_at_least(
                record["hits"],
                record["matched"],
                int(CONTRACT["language_precision_floor_pct"]),
            )
        ):
            return False
    return True


def coverage_regression_eligible(result: dict[str, Any], baseline: dict[str, Any]) -> bool:
    if result["overall"]["reported"] != baseline["overall"]["reported"]:
        return False
    if set(result["languages"]) != set(baseline["languages"]):
        return False
    if not ratio_at_least(
        result["overall"]["matched"],
        result["overall"]["reported"],
        int(CONTRACT["label_coverage_min_pct"]),
    ):
        return False
    for language, record in result["languages"].items():
        baseline_record = baseline["languages"][language]
        if record["reported"] != baseline_record["reported"]:
            return False
        if record["reported"] < CONTRACT["language_floor_min_positions"]:
            continue
        if not regression_within_limit(
            record["hits"],
            record["matched"],
            baseline_record["hits"],
            baseline_record["matched"],
            int(CONTRACT["max_language_regression_pp"]),
        ):
            return False
    return True


def ratio_at_least(hits: int, count: int, threshold_pct: int) -> bool:
    return count > 0 and 100 * hits >= threshold_pct * count


def regression_within_limit(
    hits: int, count: int, baseline_hits: int, baseline_count: int, limit_pp: int
) -> bool:
    if count <= 0 or baseline_count <= 0:
        return False
    return 100 * (hits * baseline_count - baseline_hits * count) >= (
        -limit_pp * count * baseline_count
    )


def public_metrics(result: dict[str, Any]) -> dict[str, Any]:
    return {"overall": result["overall"], "languages": result["languages"]}


def result_order(item: tuple[Proposal, dict[str, Any]]) -> tuple[float, float, float, str]:
    proposal, result = item
    return (
        -result["overall"]["precision_pct"],
        -result["overall"]["slot_yield_pct"],
        -result["overall"]["coverage_pct"],
        proposal.id,
    )


def fold_assignment(dataset: dict[str, Any]) -> dict[str, int]:
    assignments: dict[str, int] = {}
    for language in LANGUAGES:
        repos = sorted(
            repo
            for repo, row in dataset["repositories"].items()
            if row["language"] == language
        )
        for index, repo in enumerate(repos):
            assignments[repo] = index % FOLD_COUNT
    return assignments


def cross_validate(
    dataset: dict[str, Any],
    rankings: dict[str, dict[str, RepositoryRanking]] | None = None,
) -> dict[str, Any]:
    if rankings is None:
        rankings = precompute_rankings(dataset)
    assignments = fold_assignment(dataset)
    all_repos = set(dataset["repositories"])
    folds = []
    oof = empty_counts()
    oof_languages = {language: empty_counts() for language in LANGUAGES}
    selections: dict[str, int] = {}
    for fold in range(FOLD_COUNT):
        validation = sorted(repo for repo, value in assignments.items() if value == fold)
        training = sorted(all_repos - set(validation))
        baseline_train = metrics_for(dataset, BASELINE, training, rankings=rankings)
        candidates = []
        for proposal in PROPOSALS:
            result = metrics_for(dataset, proposal, training, rankings=rankings)
            if candidate_eligible(result, baseline_train):
                candidates.append((proposal, result))
        if not candidates:
            selected = BASELINE
        else:
            selected = sorted(candidates, key=result_order)[0][0]
        selections[selected.id] = selections.get(selected.id, 0) + 1
        measured = metrics_for(dataset, selected, validation, rankings=rankings)
        fold_counts = {
            key: measured["overall"][key] for key in ("hits", "matched", "reported")
        }
        add_counts(oof, fold_counts)
        for language, record in measured["languages"].items():
            add_counts(
                oof_languages[language],
                {key: record[key] for key in ("hits", "matched", "reported")},
            )
        folds.append(
            {
                "fold": fold,
                "selected_proposal": selected.id,
                "training_repositories": training,
                "validation_repositories": validation,
                "validation": public_metrics(measured),
            }
        )
    return {
        "assignment": assignments,
        "folds": folds,
        "oof": {
            "overall": metric_record(oof),
            "languages": {
                language: metric_record(counts)
                for language, counts in oof_languages.items()
                if counts["reported"]
            },
        },
        "selection_frequency": dict(sorted(selections.items())),
    }


def evaluate_dataset(dataset: dict[str, Any]) -> dict[str, Any]:
    by_id = {proposal.id: proposal for proposal in PROPOSALS}
    rankings = precompute_rankings(dataset)
    baseline = metrics_for(dataset, BASELINE, rankings=rankings)
    results = {
        proposal.id: metrics_for(dataset, proposal, rankings=rankings)
        for proposal in PROPOSALS
    }
    eligible = [
        (by_id[proposal_id], result)
        for proposal_id, result in results.items()
        if candidate_eligible(result, baseline)
    ]
    eligible.sort(key=result_order)
    coverage_guarded = [
        (by_id[proposal_id], result)
        for proposal_id, result in results.items()
        if coverage_regression_eligible(result, baseline)
    ]
    coverage_guarded.sort(key=result_order)
    all_results = sorted(
        [(by_id[proposal_id], result) for proposal_id, result in results.items()],
        key=result_order,
    )
    successes = [
        (proposal, result)
        for proposal, result in eligible
        if ratio_at_least(
            result["overall"]["hits"],
            result["overall"]["matched"],
            int(CONTRACT["precision_at_10_min_pct"]),
        )
    ]
    optimistically_possible = [
        (proposal, result)
        for proposal, result in all_results
        if ratio_at_least(
            result["overall"]["hits"]
            + result["overall"]["reported"]
            - result["overall"]["matched"],
            result["overall"]["reported"],
            int(CONTRACT["precision_at_10_min_pct"]),
        )
    ]
    formulas = {
        canonical_sha256({key: value for key, value in asdict(proposal).items() if key != "id"})
        for proposal in PROPOSALS
    }
    return {
        "baseline": public_metrics(baseline),
        "best_any": {
            "proposal": all_results[0][0].id,
            "result": public_metrics(all_results[0][1]),
        },
        "best_coverage_guarded": (
            {
                "proposal": coverage_guarded[0][0].id,
                "result": public_metrics(coverage_guarded[0][1]),
            }
            if coverage_guarded
            else None
        ),
        "best_eligible": (
            {"proposal": eligible[0][0].id, "result": public_metrics(eligible[0][1])}
            if eligible
            else None
        ),
        "decision": (
            "go"
            if successes
            else "evidence-incomplete"
            if optimistically_possible
            else "no-go"
        ),
        "optimistically_possible_proposals": [
            proposal.id for proposal, _ in optimistically_possible
        ],
        "proposal_formula_count": len(formulas),
        "proposal_definitions": [asdict(proposal) for proposal in PROPOSALS],
        "proposal_results": {
            proposal_id: public_metrics(result) for proposal_id, result in results.items()
        },
        "successful_proposals": [proposal.id for proposal, _ in successes],
        "cross_validation": cross_validate(dataset, rankings),
    }


def compact_dataset(collection: dict[str, Any]) -> dict[str, Any]:
    if collection.get("schema") != "nose.residual_ranking_collection.v1":
        raise ValueError("unexpected collection schema")
    if collection.get("split") != "dev" or collection.get("heldout_policy") != HELDOUT_POLICY:
        raise ValueError("collection is not dev-only with held-out closed")
    labels = load_dev_labels()
    corpus = dev_repositories()
    if set(collection["repositories"]) != set(labels) or set(labels) != set(corpus):
        raise ValueError("collection repository set mismatch")
    repositories = {}
    family_count = 0
    for repo, row in sorted(collection["repositories"].items()):
        if row["commit"] != corpus[repo]["commit"]:
            raise ValueError(f"{repo}: corpus commit mismatch")
        families = [
            compact_family(family, labels[repo], rank)
            for rank, family in enumerate(row["families"], start=1)
        ]
        if len({family["key"] for family in families}) != len(families):
            raise ValueError(f"{repo}: duplicate family ID/anchor key")
        family_count += len(families)
        repositories[repo] = {
            "commit": row["commit"],
            "language": row["language"],
            "query_stdout_sha256": row["stdout_sha256"],
            "families": families,
        }
    return {
        "family_count": family_count,
        "repository_count": len(repositories),
        "repositories": repositories,
    }


def reviewer_records() -> list[dict[str, Any]]:
    return [
        {
            "reviewer": "default-surface",
            "baseline_reproduced": "382/647",
            "recommendation": "dev-cv-then-no-go-if-70-missed",
            "heldout_opened": False,
            "primary_signal": "spread-reward-relaxation",
        },
        {
            "reviewer": "zero-noise-gate",
            "baseline_reproduced": "382/647",
            "recommendation": "bounded-module-interaction-or-no-go",
            "heldout_opened": False,
            "primary_signal": "module-spread-x-same-symbol",
        },
        {
            "reviewer": "soundness-lab",
            "baseline_reproduced": "382/647",
            "recommendation": "full-universe-cv-and-no-missingness-win",
            "heldout_opened": False,
            "primary_signal": "spread-reward-relaxation",
        },
    ]


def freeze(args: argparse.Namespace) -> None:
    collection = read_json(args.input)
    dataset = compact_dataset(collection)
    dataset_sha = canonical_sha256(dataset)
    evaluation = evaluate_dataset(dataset)
    if evaluation["baseline"]["overall"] != {
        "hits": 380,
        "matched": 645,
        "reported": 658,
        "precision_pct": 58.9147,
        "coverage_pct": 98.0243,
        "slot_yield_pct": 57.7508,
        "best_case_slot_precision_pct": 59.7264,
    }:
        raise ValueError("current baseline did not reproduce exactly")
    artifact = {
        "schema": "nose.residual_ranking_calibration.v1",
        "issue": 845,
        "split": "dev",
        "decision": evaluation["decision"],
        "heldout_policy": HELDOUT_POLICY,
        "contract": CONTRACT,
        "provenance": {
            "base_commit": EXPECTED_BASE_COMMIT,
            "base_tree": EXPECTED_BASE_TREE,
            "binary_sha256": collection["nose"]["sha256"],
            "binary_version": collection["nose"]["version"],
            "corpus": {"path": str(CORPUS.relative_to(ROOT)), "sha256": sha256_file(CORPUS)},
            "dev_labels": [
                {"path": str(path.relative_to(ROOT)), "sha256": sha256_file(path)}
                for path in DEV_LABELS
            ],
            "parent_quality": {
                "path": str(PARENT_QUALITY.relative_to(ROOT)),
                "sha256": sha256_file(PARENT_QUALITY),
            },
            "collector": frozen_collector_record(),
        },
        "dataset_sha256": dataset_sha,
        "dataset": dataset,
        "evaluation": evaluation,
        "independent_preimplementation_reviews": reviewer_records(),
        "preservation": {
            "product_code_changed": False,
            "ranking_changed": False,
            "surface_changed": False,
            "full_universe_worthy_recall": "2716/2849",
            "worthy_recall_delta": 0,
        },
        "next_step": EXPECTED_NEXT_STEP,
    }
    args.output.write_text(
        json.dumps(artifact, sort_keys=True, separators=(",", ":")) + "\n",
        encoding="utf-8",
    )
    print(f"dataset_sha256={dataset_sha}")
    print(json.dumps(evaluation["best_eligible"], indent=2, sort_keys=True))


def require_equal(actual: object, expected: object, label: str) -> None:
    if actual != expected:
        raise ValueError(f"{label}: mismatch")


def require_exact_keys(value: object, expected: set[str], label: str) -> dict[str, Any]:
    if not isinstance(value, dict) or set(value) != expected:
        raise ValueError(f"{label}: expected exact keys {sorted(expected)}")
    return value


def expected_path_record(path: Path) -> dict[str, str]:
    return {"path": path.relative_to(ROOT).as_posix(), "sha256": sha256_file(path)}


def frozen_collector_record() -> dict[str, str]:
    path = Path(__file__).relative_to(ROOT).as_posix()
    frozen = subprocess.run(
        ["git", "show", f"{EXPECTED_COLLECTOR_COMMIT}:{path}"],
        cwd=ROOT,
        check=True,
        capture_output=True,
    ).stdout
    require_equal(
        sha256_bytes(frozen), EXPECTED_COLLECTOR_SHA256, "frozen collector blob"
    )
    return {"path": path, "sha256": EXPECTED_COLLECTOR_SHA256}


def validate_current_order(dataset: dict[str, Any]) -> None:
    for repo, row in dataset["repositories"].items():
        families = row["families"]
        ranks = [family["current_rank"] for family in families]
        if ranks != list(range(1, len(families) + 1)):
            raise ValueError(f"{repo}: current ranks are not the collected total order")
        for family in families:
            anchor_path = Path(family["anchor"][0])
            expected_prefix = Path("bench/repos") / repo
            if anchor_path.is_absolute() or ".." in anchor_path.parts or not anchor_path.is_relative_to(expected_prefix):
                raise ValueError(f"{repo}: family anchor escapes its pinned dev checkout")
            expected_key = (
                f"{family['id']}@{family['anchor'][0]}:{family['anchor'][1]}"
                f"#rank-{family['current_rank']}"
            )
            require_equal(family["key"], expected_key, f"{repo}: family key")


def validate_payload(
    artifact: dict[str, Any], *, expected_evaluation: dict[str, Any] | None = None
) -> None:
    require_exact_keys(
        artifact,
        {
            "schema",
            "issue",
            "split",
            "decision",
            "heldout_policy",
            "contract",
            "provenance",
            "dataset_sha256",
            "dataset",
            "evaluation",
            "independent_preimplementation_reviews",
            "preservation",
            "next_step",
        },
        "artifact",
    )
    require_equal(artifact.get("schema"), "nose.residual_ranking_calibration.v1", "schema")
    require_equal(artifact.get("issue"), 845, "issue")
    require_equal(artifact.get("split"), "dev", "split")
    require_equal(artifact.get("heldout_policy"), HELDOUT_POLICY, "heldout_policy")
    require_equal(artifact.get("contract"), CONTRACT, "contract")
    provenance = require_exact_keys(
        artifact["provenance"],
        {
            "base_commit",
            "base_tree",
            "binary_sha256",
            "binary_version",
            "corpus",
            "dev_labels",
            "parent_quality",
            "collector",
        },
        "provenance",
    )
    require_equal(provenance["base_commit"], EXPECTED_BASE_COMMIT, "base commit")
    require_equal(provenance["base_tree"], EXPECTED_BASE_TREE, "base tree")
    require_equal(provenance["binary_sha256"], EXPECTED_BINARY_SHA256, "binary")
    require_equal(provenance["binary_version"], EXPECTED_BINARY_VERSION, "binary version")
    expected_corpus = expected_path_record(CORPUS)
    expected_parent = expected_path_record(PARENT_QUALITY)
    expected_labels = [expected_path_record(path) for path in DEV_LABELS]
    expected_collector = frozen_collector_record()
    require_equal(provenance["corpus"], expected_corpus, "corpus provenance")
    require_equal(provenance["parent_quality"], expected_parent, "parent provenance")
    require_equal(provenance["dev_labels"], expected_labels, "dev-label provenance")
    require_equal(provenance["collector"], expected_collector, "collector provenance")
    for label, record in (
        ("corpus", expected_corpus),
        ("parent quality", expected_parent),
        *[(f"dev label {index}", record) for index, record in enumerate(expected_labels)],
    ):
        require_exact_keys(record, {"path", "sha256"}, label)
        require_equal(sha256_file(ROOT / record["path"]), record["sha256"], label)
    dataset_sha = canonical_sha256(artifact["dataset"])
    require_equal(dataset_sha, artifact["dataset_sha256"], "dataset digest")
    if EXPECTED_DATASET_SHA256:
        require_equal(dataset_sha, EXPECTED_DATASET_SHA256, "frozen dataset digest")
    require_equal(
        artifact["dataset"]["repository_count"], len(artifact["dataset"]["repositories"]), "repository count"
    )
    require_equal(
        artifact["dataset"]["family_count"],
        sum(len(row["families"]) for row in artifact["dataset"]["repositories"].values()),
        "family count",
    )
    validate_current_order(artifact["dataset"])
    reproduced = (
        evaluate_dataset(artifact["dataset"])
        if expected_evaluation is None
        else expected_evaluation
    )
    require_equal(reproduced, artifact["evaluation"], "evaluation")
    require_equal(artifact["decision"], "evidence-incomplete", "decision")
    require_equal(
        artifact["evaluation"]["decision"], "evidence-incomplete", "evaluated decision"
    )
    require_equal(artifact["evaluation"]["successful_proposals"], [], "successful proposals")
    reviews = artifact["independent_preimplementation_reviews"]
    require_equal(reviews, reviewer_records(), "independent reviews")
    if any(review["heldout_opened"] for review in reviews):
        raise ValueError("a review opened held-out evidence")
    require_equal(
        artifact["preservation"],
        {
            "product_code_changed": False,
            "ranking_changed": False,
            "surface_changed": False,
            "full_universe_worthy_recall": "2716/2849",
            "worthy_recall_delta": 0,
        },
        "preservation",
    )
    require_equal(artifact["next_step"], EXPECTED_NEXT_STEP, "next step")


def validate(args: argparse.Namespace) -> None:
    validate_payload(read_json(args.artifact))
    print(f"validated {args.artifact}")


def self_test(args: argparse.Namespace) -> None:
    artifact = read_json(args.artifact)
    validate_payload(artifact)
    expected_evaluation = artifact["evaluation"]
    mutations = []
    changed = copy.deepcopy(artifact)
    changed["heldout_policy"]["labels_opened"] = True
    mutations.append(changed)
    changed = copy.deepcopy(artifact)
    first_repo = next(iter(changed["dataset"]["repositories"].values()))
    first_repo["families"][0]["base_core"] += 1.0
    mutations.append(changed)
    changed = copy.deepcopy(artifact)
    changed["evaluation"]["decision"] = "go"
    mutations.append(changed)
    changed = copy.deepcopy(artifact)
    changed["independent_preimplementation_reviews"][0]["heldout_opened"] = True
    mutations.append(changed)
    changed = copy.deepcopy(artifact)
    changed["heldout_evaluation"] = {"precision_at_10": 99}
    mutations.append(changed)
    changed = copy.deepcopy(artifact)
    changed["provenance"]["heldout_result"] = {"path": "secret-heldout.json"}
    mutations.append(changed)
    changed = copy.deepcopy(artifact)
    changed["provenance"]["dev_labels"] = []
    mutations.append(changed)
    changed = copy.deepcopy(artifact)
    changed["provenance"]["binary_version"] = "nose 999.0.0"
    mutations.append(changed)
    changed = copy.deepcopy(artifact)
    changed["next_step"] = "Open held-out now"
    mutations.append(changed)
    for mutation in mutations:
        try:
            validate_payload(mutation, expected_evaluation=expected_evaluation)
        except ValueError:
            pass
        else:
            raise AssertionError("invalid mutation was accepted")
    assert not candidate_eligible(
        {
            "overall": metric_record({"hits": 400, "matched": 600, "reported": 658}),
            "languages": {
                "C": metric_record({"hits": 0, "matched": 29, "reported": 90})
            },
        },
        {
            "overall": metric_record({"hits": 380, "matched": 645, "reported": 658}),
            "languages": {
                "C": metric_record({"hits": 40, "matched": 90, "reported": 90})
            },
        },
    ), "reported-position language floor must reject missingness evasion"
    synthetic = [
        {
            "current_rank": rank,
            "key": key,
            "id": key,
            "value": 1.0,
            "anchor": ["same.rs", 1],
            "base_core": 1.0,
            "file_bonus": 0.0,
            "module_bonus": 0.0,
            "language_bonus": 0.0,
            "params": 0,
            "tightness": 1.0,
            "homogeneity": 1.0,
            "same_symbol": False,
            "modules": 1,
            "witness": None,
            "implementation_type": False,
            "truth": rank == 1,
        }
        for rank, key in ((2, "b"), (1, "a"))
    ]
    experimental = next(proposal for proposal in PROPOSALS if proposal.id != "current")
    assert [family["key"] for family in order_families(synthetic, experimental)] == ["a", "b"]
    assert [family["key"] for family in order_families(reversed(synthetic), experimental)] == ["a", "b"]
    synthetic_dataset = {
        "repositories": {
            "repo": {"language": "Rust", "families": synthetic},
        }
    }
    rankings = precompute_rankings(synthetic_dataset)
    for proposal in (BASELINE, experimental):
        assert metrics_for(synthetic_dataset, proposal) == metrics_for(
            synthetic_dataset, proposal, rankings=rankings
        )
    print("residual-ranking self-test passed")


def inspect(args: argparse.Namespace) -> None:
    artifact = read_json(args.artifact)
    evaluation = artifact["evaluation"]
    print(json.dumps({
        "decision": evaluation["decision"],
        "baseline": evaluation["baseline"]["overall"],
        "best_eligible": evaluation["best_eligible"],
        "oof": evaluation["cross_validation"]["oof"],
        "selection_frequency": evaluation["cross_validation"]["selection_frequency"],
    }, indent=2, sort_keys=True))


def parser() -> argparse.ArgumentParser:
    root = argparse.ArgumentParser()
    commands = root.add_subparsers(dest="command", required=True)
    collect_parser = commands.add_parser("collect")
    collect_parser.add_argument("--nose", type=Path, required=True)
    collect_parser.add_argument("--repos-root", type=Path, default=ROOT / "bench/repos")
    collect_parser.add_argument("--jobs", type=int, default=4)
    collect_parser.add_argument("--output", type=Path, required=True)
    collect_parser.set_defaults(run=collect)
    freeze_parser = commands.add_parser("freeze")
    freeze_parser.add_argument("--input", type=Path, required=True)
    freeze_parser.add_argument("--output", type=Path, default=DEFAULT_ARTIFACT)
    freeze_parser.set_defaults(run=freeze)
    validate_parser = commands.add_parser("validate")
    validate_parser.add_argument("artifact", type=Path, nargs="?", default=DEFAULT_ARTIFACT)
    validate_parser.set_defaults(run=validate)
    self_test_parser = commands.add_parser("self-test")
    self_test_parser.add_argument("--artifact", type=Path, default=DEFAULT_ARTIFACT)
    self_test_parser.set_defaults(run=self_test)
    inspect_parser = commands.add_parser("inspect")
    inspect_parser.add_argument("--artifact", type=Path, default=DEFAULT_ARTIFACT)
    inspect_parser.set_defaults(run=inspect)
    return root


def main() -> None:
    args = parser().parse_args()
    args.run(args)


if __name__ == "__main__":
    main()
