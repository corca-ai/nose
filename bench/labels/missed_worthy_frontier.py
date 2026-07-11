#!/usr/bin/env python3
"""Validation and split-safe dev selection for the missed-worthy frontier.

The collection command lives in ``recall_ceiling_probe.py``.  This module keeps
the deterministic schema, selection policy, validators, and source-context
renderer separate from the comparatively expensive detector invocations.
"""

from __future__ import annotations

from collections import Counter, defaultdict
import hashlib
import json
from pathlib import Path
import re
import tempfile
from typing import Any, Iterable

from labelset import RECALL_METRIC, load_labelset, metric_eligible, sha256_file
from query_schema import QUERY_SCHEMA_VERSION


ROOT = Path(__file__).resolve().parents[2]
ARTIFACT_SCHEMA = "nose.missed_worthy_frontier.v2"
DECISIONS_SCHEMA = "nose.missed_worthy_dev_audit.v1"
STAGE_AUDIT_SCHEMA = "nose.missed_worthy_stage_audit.dev.v1"
CLOSEOUT_SCHEMA = "nose.missed_worthy_frontier_closeout.v1"
SOURCE_BOUNDS_SCHEMA = "nose.missed_worthy_source_bounds.dev.v1"
SELECTION_SEED = "nose-issue-816-dev-audit-v1"
SELECTION_PER_LANGUAGE = 5
SUBDAG_FLOORS = (8, 12, 20)
INLINE_FLOOR = 20
CHECKED_RECALL_PROFILES = {
    "2664d2935eaf8e86243dcf3592225c9f4884154ac7757c1307fd2a4281688e2c": {
        "dev": {"hits": 2626, "n": 2849},
        "heldout": {"hits": 1949, "n": 2091},
    },
}
REQUIRED_RESIDUAL_LANES = (
    "inline-ceiling",
    "same-unit-window",
    "unrecovered",
    "extraction-other",
)
VALID_PROBE_CLASSES = {
    "subdag-ceiling",
    "inline-ceiling",
    "same-unit-window",
    "unrecovered",
    "no-overlapping-unit",
    "member-file-missing",
    "features-failed",
}
VALID_BLOCKER_CLASSIFICATIONS = {
    "unit-extraction-or-missing-unit-kind",
    "candidate-generation",
    "connected-anchor-or-subdag-construction",
    "family-folding-or-overlap-matching",
    "one-step-pure-helper-composition",
    "same-unit-actionable-fragment",
    "no-coherent-general-mechanism",
}
SHA256_RE = re.compile(r"^[0-9a-f]{64}$")
GIT_SHA_RE = re.compile(r"^[0-9a-f]{40}$")


def canonical_json(value: object) -> str:
    return json.dumps(value, ensure_ascii=False, separators=(",", ":"), sort_keys=True)


def canonical_sha256(value: object) -> str:
    return hashlib.sha256(canonical_json(value).encode("utf-8")).hexdigest()


def registered_recall_profile(digest: object) -> dict[str, dict[str, int]]:
    """Return the recall contract registered for an evaluation digest."""
    _validate_hash(digest, "evaluation input sha256: invalid")
    registered = CHECKED_RECALL_PROFILES.get(digest)
    _require(registered is not None, f"evaluation report {digest} is unregistered")
    return {split: dict(counts) for split, counts in registered.items()}


def checked_recall_profile(evaluation_input: object) -> dict[str, dict[str, int]]:
    """Validate and return the recall contract carried by an artifact input."""
    _require(isinstance(evaluation_input, dict), "evaluation input: expected an object")
    registered = registered_recall_profile(evaluation_input.get("sha256"))
    _require(
        evaluation_input.get("expected_worthy_recall") == registered,
        "evaluation report recall differs from its registered profile",
    )
    return registered


def validate_evaluation_recall(
    evaluation: object,
    expected_recall: dict[str, dict[str, int]],
) -> None:
    _require(isinstance(evaluation, dict), "checked evaluation: expected an object")
    metrics = evaluation.get("metrics")
    _require(isinstance(metrics, dict), "checked evaluation metrics missing")
    for split, expected in expected_recall.items():
        split_metrics = metrics.get(split)
        _require(isinstance(split_metrics, dict), f"checked evaluation {split} missing")
        overall = split_metrics.get("OVERALL")
        _require(isinstance(overall, dict), f"checked evaluation {split} overall missing")
        actual = overall.get("worthy_recall")
        _require(isinstance(actual, dict), f"checked evaluation {split} recall missing")
        _require(
            {"hits": actual.get("hits"), "n": actual.get("n")} == expected,
            f"checked evaluation {split} worthy recall drifted",
        )


def project_path(path: str) -> Path:
    candidate = Path(path)
    return candidate if candidate.is_absolute() else ROOT / candidate


def relative_path(path: Path) -> str:
    try:
        return path.resolve().relative_to(ROOT).as_posix()
    except ValueError:
        return path.resolve().as_posix()


def candidate_payload(record: dict[str, Any]) -> dict[str, Any]:
    return {key: value for key, value in record.items() if key != "candidate_sha256"}


def candidate_sha256(record: dict[str, Any]) -> str:
    return canonical_sha256(candidate_payload(record))


def audit_lane(record: dict[str, Any]) -> str:
    if record.get("subdag_ge_20") is True:
        return "subdag-ge20"
    probe_class = record.get("class")
    if probe_class == "subdag-ceiling":
        return "subdag-below-20"
    if probe_class in {
        "inline-ceiling",
        "same-unit-window",
        "unrecovered",
    }:
        return str(probe_class)
    return "extraction-other"


def selection_rank(record: dict[str, Any], phase: str, seed: str) -> str:
    material = (
        f"{seed}\0{phase}\0{record['candidate_key']}\0"
        f"{record['candidate_sha256']}"
    )
    return hashlib.sha256(material.encode("utf-8")).hexdigest()


def select_dev_audit(
    records: Iterable[dict[str, Any]],
    *,
    per_language: int = SELECTION_PER_LANGUAGE,
    required_lanes: tuple[str, ...] = REQUIRED_RESIDUAL_LANES,
    seed: str = SELECTION_SEED,
) -> list[dict[str, Any]]:
    """Select a language-balanced dev audit without consulting source text.

    One example from every required residual lane is reserved first.  Each
    language is then filled to the same quota, preferring the shipped weight-20
    sub-DAG ceiling and distinct repositories.  Hash ranks make ties stable and
    insensitive to artifact input order.
    """

    dev = sorted(
        (record for record in records if record.get("split") == "dev"),
        key=lambda record: record["candidate_key"],
    )
    languages = sorted({record["language"] for record in dev})
    if not languages:
        raise ValueError("dev audit selection has no candidate languages")
    by_language: dict[str, list[dict[str, Any]]] = defaultdict(list)
    for record in dev:
        by_language[record["language"]].append(record)
    undersized = {
        language: len(rows)
        for language, rows in by_language.items()
        if len(rows) < per_language
    }
    if undersized:
        raise ValueError(f"dev audit language quotas cannot be filled: {undersized}")

    selected: list[dict[str, Any]] = []
    selected_keys: set[str] = set()
    language_counts: Counter[str] = Counter()
    repository_counts: Counter[str] = Counter()

    def add(record: dict[str, Any], phase: str) -> None:
        selected_keys.add(record["candidate_key"])
        language_counts[record["language"]] += 1
        repository_counts[record["repo"]] += 1
        selected.append(
            {
                "candidate_key": record["candidate_key"],
                "candidate_sha256": record["candidate_sha256"],
                "language": record["language"],
                "repo": record["repo"],
                "lane": audit_lane(record),
                "phase": phase,
            }
        )

    for lane in required_lanes:
        eligible = [
            record
            for record in dev
            if audit_lane(record) == lane
            and language_counts[record["language"]] < per_language
        ]
        if not eligible:
            raise ValueError(f"dev audit selection cannot represent lane {lane}")
        choice = min(
            eligible,
            key=lambda record: (
                language_counts[record["language"]],
                repository_counts[record["repo"]] > 0,
                selection_rank(record, f"residual:{lane}", seed),
            ),
        )
        add(choice, f"required-residual:{lane}")

    for language in languages:
        while language_counts[language] < per_language:
            eligible = [
                record
                for record in by_language[language]
                if record["candidate_key"] not in selected_keys
            ]
            if not eligible:
                raise ValueError(f"dev audit exhausted candidates for {language}")
            choice = min(
                eligible,
                key=lambda record: (
                    audit_lane(record) != "subdag-ge20",
                    repository_counts[record["repo"]] > 0,
                    audit_lane(record) == "subdag-below-20",
                    selection_rank(record, f"fill:{language}", seed),
                ),
            )
            add(choice, "language-fill")

    return [dict(record, order=index + 1) for index, record in enumerate(selected)]


def aggregate_metrics(
    query_runs: dict[str, dict[str, Any]],
    candidates: list[dict[str, Any]],
) -> tuple[dict[str, Any], dict[str, Any]]:
    candidate_counts: dict[tuple[str, str], Counter[str]] = defaultdict(Counter)
    for record in candidates:
        key = (record["split"], record["language"])
        candidate_counts[key][record["class"]] += 1
        for floor in SUBDAG_FLOORS:
            candidate_counts[key][f"subdag_ge_{floor}"] += bool(
                record.get(f"subdag_ge_{floor}")
            )

    by_language: dict[tuple[str, str], Counter[str]] = defaultdict(Counter)
    for run in query_runs.values():
        key = (run["split"], run["language"])
        by_language[key]["repositories"] += 1
        for field in ("worthy", "hit_arm0", "hit_arm1"):
            by_language[key][field] += run[field]
    for key, counts in candidate_counts.items():
        by_language[key].update(counts)

    rendered_by_language: dict[str, dict[str, Any]] = defaultdict(dict)
    by_split: dict[str, Counter[str]] = defaultdict(Counter)
    for (split, language), counts in sorted(by_language.items()):
        counts["missed_arm1"] = counts["worthy"] - counts["hit_arm1"]
        rendered_by_language[split][language] = dict(sorted(counts.items()))
        by_split[split].update(counts)
    rendered_by_split = {
        split: dict(sorted(counts.items())) for split, counts in sorted(by_split.items())
    }
    return dict(rendered_by_language), rendered_by_split


def _require(condition: bool, message: str) -> None:
    if not condition:
        raise ValueError(message)


def _validate_hash(value: object, label: str) -> str:
    _require(isinstance(value, str) and SHA256_RE.fullmatch(value) is not None, label)
    return str(value)


def _validate_tracked_input(record: object, label: str) -> Path:
    _require(isinstance(record, dict), f"{label}: expected an object")
    path = record.get("path")
    _require(isinstance(path, str) and path, f"{label}.path: expected a path")
    expected = _validate_hash(record.get("sha256"), f"{label}.sha256: invalid SHA-256")
    resolved = project_path(path)
    _require(resolved.is_file(), f"{label}: missing {resolved}")
    actual = sha256_file(resolved)
    _require(actual == expected, f"{label}: hash mismatch {actual} != {expected}")
    return resolved


def _validate_candidate_records(
    candidates: object,
    source_files: object,
    *,
    check_sources: bool,
) -> list[dict[str, Any]]:
    _require(isinstance(candidates, list), "missed_worthy: expected an array")
    _require(isinstance(source_files, dict), "source_files: expected an object")
    rows: list[dict[str, Any]] = candidates
    expected_order = sorted(
        rows, key=lambda record: (record.get("repo", ""), record.get("family_id", ""))
    )
    _require(rows == expected_order, "missed_worthy: records are not canonically sorted")
    keys: set[str] = set()
    for index, record in enumerate(rows):
        label = f"missed_worthy[{index}]"
        _require(isinstance(record, dict), f"{label}: expected an object")
        required_strings = (
            "candidate_key",
            "candidate_sha256",
            "family_id",
            "repo",
            "split",
            "language",
            "reason",
            "channel",
            "scope",
            "class",
        )
        for field in required_strings:
            _require(
                isinstance(record.get(field), str) and bool(record[field]),
                f"{label}.{field}: expected a non-empty string",
            )
        _require(record["split"] in {"dev", "heldout"}, f"{label}.split: invalid")
        _require(record["class"] in VALID_PROBE_CLASSES, f"{label}.class: invalid")
        _validate_hash(record["candidate_sha256"], f"{label}.candidate_sha256: invalid")
        _require(
            record["candidate_sha256"] == candidate_sha256(record),
            f"{label}: candidate hash mismatch",
        )
        _require(record["candidate_key"] not in keys, f"{label}: duplicate candidate key")
        keys.add(record["candidate_key"])
        members = record.get("members")
        _require(isinstance(members, list) and len(members) == 2, f"{label}.members")
        file_keys = record.get("source_files")
        _require(isinstance(file_keys, list) and file_keys, f"{label}.source_files")
        member_files = {member.get("file") for member in members}
        _require(set(file_keys) == member_files, f"{label}: source files differ from members")
        for file_key in file_keys:
            _require(file_key in source_files, f"{label}: unknown source file {file_key}")
        probe_class = record["class"]
        if probe_class == "subdag-ceiling":
            mass = record.get("intersection_mass")
            _require(isinstance(mass, int) and mass >= SUBDAG_FLOORS[0], f"{label}: bad mass")
            for floor in SUBDAG_FLOORS:
                _require(
                    record.get(f"subdag_ge_{floor}") == (mass >= floor),
                    f"{label}: threshold flag {floor} drifted",
                )
        elif probe_class in {"inline-ceiling", "unrecovered"}:
            mass = record.get("intersection_mass")
            augmented = record.get("inline_aug_mass")
            _require(
                isinstance(mass, int) and 0 <= mass < SUBDAG_FLOORS[0],
                f"{label}: inline base mass is invalid",
            )
            _require(
                isinstance(augmented, int) and augmented >= mass,
                f"{label}: inline augmented mass is invalid",
            )
            _require(
                (augmented >= INLINE_FLOOR) == (probe_class == "inline-ceiling"),
                f"{label}: inline class disagrees with its ceiling",
            )

    for path, record in source_files.items():
        label = f"source_files[{path!r}]"
        _require(isinstance(record, dict), f"{label}: expected an object")
        _validate_hash(record.get("sha256"), f"{label}.sha256: invalid")
        _require(
            isinstance(record.get("size_bytes"), int) and record["size_bytes"] >= 0,
            f"{label}.size_bytes: invalid",
        )
        if check_sources:
            resolved = project_path(path)
            _require(resolved.is_file(), f"{label}: source checkout is missing")
            _require(sha256_file(resolved) == record["sha256"], f"{label}: hash mismatch")
            _require(resolved.stat().st_size == record["size_bytes"], f"{label}: size mismatch")
    return rows


def validate_artifact(
    payload: object,
    *,
    check_inputs: bool = True,
    check_sources: bool = False,
) -> None:
    _require(isinstance(payload, dict), "artifact: expected an object")
    _require(payload.get("schema") == ARTIFACT_SCHEMA, "artifact: unsupported schema")
    config = payload.get("configuration")
    _require(isinstance(config, dict), "configuration: expected an object")
    _require(config.get("subdag_floors") == list(SUBDAG_FLOORS), "sub-DAG floors drifted")
    _require(config.get("inline_floor") == INLINE_FLOOR, "inline floor drifted")
    _require(config.get("limit_repositories") is None, "official artifact cannot be limited")

    provenance = payload.get("provenance")
    _require(isinstance(provenance, dict), "provenance: expected an object")
    _require(
        isinstance(provenance.get("command"), str) and provenance["command"],
        "provenance.command: missing",
    )
    _require(
        isinstance(provenance.get("git_sha"), str)
        and GIT_SHA_RE.fullmatch(provenance["git_sha"]) is not None,
        "provenance.git_sha: invalid",
    )
    _require(
        provenance.get("working_tree_status_before_measurement") == "",
        "measurement did not start from a clean working tree",
    )
    nose = provenance.get("nose")
    _require(isinstance(nose, dict), "provenance.nose: expected an object")
    _require(nose.get("version") == "nose 0.18.0", "artifact must use nose 0.18.0")
    _validate_hash(nose.get("sha256"), "provenance.nose.sha256: invalid")
    _require(
        provenance.get("query_schema_version") == QUERY_SCHEMA_VERSION,
        "query schema version drifted",
    )
    failures = payload.get("failures")
    _require(failures == [], f"official artifact contains failures: {failures}")

    inputs = provenance.get("inputs")
    _require(isinstance(inputs, dict), "provenance.inputs: expected an object")
    required_inputs = {
        "recall_labelset",
        "precision_labelset",
        "evaluation_report",
        "corpus_manifest",
        "prune_manifest",
        "query_schema",
    }
    _require(set(inputs) == required_inputs, "provenance.inputs: incomplete input set")
    expected_recall = checked_recall_profile(inputs.get("evaluation_report"))
    resolved_inputs: dict[str, Path] = {}
    recall_labels = None
    corpus_payload = None
    prune_payload = None
    if check_inputs:
        resolved_inputs = {
            name: _validate_tracked_input(record, f"provenance.inputs.{name}")
            for name, record in inputs.items()
        }
        _require(
            inputs["recall_labelset"].get("role") == "only-worthy-recall-pool",
            "recall labelset role drifted",
        )
        _require(
            inputs["precision_labelset"].get("role")
            == "precision-only-current-output-overlay",
            "precision labelset role drifted",
        )
        recall_labels = load_labelset(resolved_inputs["recall_labelset"])
        _require(recall_labels.version == "v5", "worthy recall input must remain v5")
        precision_labels = load_labelset(resolved_inputs["precision_labelset"])
        _require(precision_labels.version == "v6", "precision overlay must remain v6")
        base_ids = {
            (family["repo"], family["family_id"]) for family in recall_labels.families
        }
        for family in precision_labels.families:
            if (family["repo"], family["family_id"]) not in base_ids:
                _require(
                    not metric_eligible(family, RECALL_METRIC),
                    "v6 current-output overlay entered worthy recall",
                )
        evaluation = json.loads(resolved_inputs["evaluation_report"].read_text())
        validate_evaluation_recall(evaluation, expected_recall)
        corpus_payload = json.loads(resolved_inputs["corpus_manifest"].read_text())
        repositories = corpus_payload.get("repositories")
        _require(isinstance(repositories, list), "corpus manifest repositories missing")
        digest = hashlib.sha256()
        for repository in sorted(repositories, key=lambda row: row["id"]):
            digest.update(
                (
                    f"{repository['id']}\t{repository['split']}\t"
                    f"{repository['primary_language']}\t{repository['commit']}\n"
                ).encode("utf-8")
            )
        _require(
            provenance.get("corpus_commit_digest") == digest.hexdigest(),
            "corpus commit digest drifted",
        )
        prune_payload = json.loads(resolved_inputs["prune_manifest"].read_text())
        _require(
            provenance.get("post_prune_corpus_digest")
            == prune_payload.get("corpus_digest_after_prune"),
            "post-prune corpus digest drifted",
        )

    repository_commits = provenance.get("repository_commits")
    _require(isinstance(repository_commits, dict) and repository_commits, "repository commits missing")
    for repo, record in repository_commits.items():
        _require(isinstance(record, dict), f"repository_commits.{repo}: expected object")
        expected = record.get("expected")
        observed = record.get("observed")
        _require(
            isinstance(expected, str) and GIT_SHA_RE.fullmatch(expected) is not None,
            f"repository_commits.{repo}.expected: invalid",
        )
        _require(observed == expected, f"repository_commits.{repo}: pin mismatch")
    if corpus_payload is not None:
        expected_commits = {
            repository["id"]: repository["commit"]
            for repository in corpus_payload["repositories"]
        }
        _require(set(repository_commits) == set(expected_commits), "repository pin set drifted")
        _require(
            {
                repo: record["expected"]
                for repo, record in repository_commits.items()
            }
            == expected_commits,
            "repository pins differ from the corpus manifest",
        )

    query_runs = payload.get("query_runs")
    _require(isinstance(query_runs, dict) and query_runs, "query_runs: expected an object")
    for repo, run in query_runs.items():
        label = f"query_runs.{repo}"
        _require(isinstance(run, dict), f"{label}: expected an object")
        for field in ("worthy", "hit_arm0", "hit_arm1"):
            _require(
                isinstance(run.get(field), int) and run[field] >= 0,
                f"{label}.{field}: invalid",
            )
        _require(run["hit_arm0"] <= run["worthy"], f"{label}: arm0 over-count")
        _require(run["hit_arm1"] <= run["worthy"], f"{label}: arm1 over-count")
        for arm in ("arm0", "arm1"):
            arm_run = run.get(arm)
            _require(isinstance(arm_run, dict), f"{label}.{arm}: expected an object")
            _require(arm_run.get("returncode") == 0, f"{label}.{arm}: query failed")
            _validate_hash(arm_run.get("stdout_sha256"), f"{label}.{arm}: bad stdout hash")
            _validate_hash(arm_run.get("stderr_sha256"), f"{label}.{arm}: bad stderr hash")
    if recall_labels is not None:
        expected_query_repos = {
            family["repo"]
            for family in recall_labels.families
            if family.get("worthy") is True and metric_eligible(family, RECALL_METRIC)
        }
        _require(set(query_runs) == expected_query_repos, "measured repository set drifted")

    candidates = _validate_candidate_records(
        payload.get("missed_worthy"),
        payload.get("source_files"),
        check_sources=check_sources,
    )
    if recall_labels is not None and corpus_payload is not None:
        eligible = {
            (family["repo"], family["family_id"]): family
            for family in recall_labels.families
            if family.get("worthy") is True and metric_eligible(family, RECALL_METRIC)
        }
        _require(
            len(eligible)
            == sum(
                family.get("worthy") is True and metric_eligible(family, RECALL_METRIC)
                for family in recall_labels.families
            ),
            "eligible recall family identity is not unique",
        )
        corpus_by_repo = {
            repository["id"]: repository
            for repository in corpus_payload["repositories"]
        }
        for record in candidates:
            identity = (record["repo"], record["family_id"])
            _require(identity in eligible, f"{record['candidate_key']}: not an eligible v5 family")
            family = eligible[identity]
            repository = corpus_by_repo.get(record["repo"])
            _require(repository is not None, f"{record['candidate_key']}: repo missing from corpus")
            _require(
                record["candidate_key"] == f"{record['repo']}:{record['family_id']}",
                f"{record['candidate_key']}: candidate identity drifted",
            )
            expected_members = [
                {
                    key: member[key]
                    for key in ("file", "start_line", "end_line")
                }
                for member in family["members"][:2]
            ]
            expected_metadata = {
                "split": family["split"],
                "language": repository["primary_language"],
                "reason": family["reason"],
                "channel": family["channel"],
                "scope": family["scope"],
                "confidence": family.get("confidence"),
                "members": expected_members,
                "source_files": sorted({member["file"] for member in expected_members}),
            }
            for field, expected_value in expected_metadata.items():
                _require(
                    record.get(field) == expected_value,
                    f"{record['candidate_key']}: {field} differs from frozen inputs",
                )
            _require(
                family["split"] == repository["split"],
                f"{record['candidate_key']}: label/corpus split mismatch",
            )
        eligible_by_repo = Counter(repo for repo, _ in eligible)
        for repo, run in query_runs.items():
            repository = corpus_by_repo[repo]
            _require(run.get("split") == repository["split"], f"query_runs.{repo}: split drifted")
            _require(
                run.get("language") == repository["primary_language"],
                f"query_runs.{repo}: language drifted",
            )
            _require(
                run["worthy"] == eligible_by_repo[repo],
                f"query_runs.{repo}: worthy denominator differs from v5",
            )
    by_repo = Counter(record["repo"] for record in candidates)
    for repo, run in query_runs.items():
        _require(
            by_repo[repo] == run["worthy"] - run["hit_arm1"],
            f"query_runs.{repo}: miss count differs from candidates",
        )
    _require(set(by_repo) <= set(query_runs), "candidate references an unmeasured repository")

    feature_runs = payload.get("feature_runs")
    _require(isinstance(feature_runs, dict), "feature_runs: expected an object")
    referenced_feature_runs = {
        record["feature_run"] for record in candidates if "feature_run" in record
    }
    _require(
        referenced_feature_runs == set(feature_runs),
        "feature run set differs from candidate references",
    )
    for run_key, run in feature_runs.items():
        label = f"feature_runs.{run_key}"
        _require(isinstance(run, dict), f"{label}: expected an object")
        files = run.get("files")
        _require(isinstance(files, list) and files == sorted(files), f"{label}.files: invalid")
        _require(run_key == canonical_sha256(files)[:20], f"{label}: key drifted")
        _require(run.get("returncode") == 0, f"{label}: command failed")
        _require("parse_error" not in run, f"{label}: output did not parse")
        _require(isinstance(run.get("units"), int), f"{label}.units: invalid")
        _validate_hash(run.get("stdout_sha256"), f"{label}: bad stdout hash")
        _validate_hash(run.get("stderr_sha256"), f"{label}: bad stderr hash")

    if check_sources:
        binary_path = project_path(nose["path"])
        _require(binary_path.is_file(), "recorded nose binary is missing")
        _require(sha256_file(binary_path) == nose["sha256"], "nose binary hash mismatch")

    by_language, by_split = aggregate_metrics(query_runs, candidates)
    _require(payload.get("metrics_by_language") == by_language, "language metrics drifted")
    _require(payload.get("metrics") == by_split, "split metrics drifted")
    for split, expected in expected_recall.items():
        actual = by_split.get(split, {})
        _require(
            {"hits": actual.get("hit_arm1"), "n": actual.get("worthy")} == expected,
            f"arm1 did not reproduce checked {split} recall: {actual}",
        )

    selection = payload.get("dev_audit_selection")
    _require(isinstance(selection, dict), "dev_audit_selection: expected an object")
    _require(selection.get("seed") == SELECTION_SEED, "dev selection seed drifted")
    _require(
        selection.get("per_language") == SELECTION_PER_LANGUAGE,
        "dev selection language quota drifted",
    )
    _require(
        selection.get("required_residual_lanes") == list(REQUIRED_RESIDUAL_LANES),
        "dev selection residual lanes drifted",
    )
    expected_selection = select_dev_audit(candidates)
    _require(selection.get("candidates") == expected_selection, "dev selection is not reproducible")
    _require(
        selection.get("sha256") == canonical_sha256(expected_selection),
        "dev selection hash mismatch",
    )


def load_and_validate_artifact(
    path: Path,
    *,
    check_sources: bool = False,
) -> dict[str, Any]:
    try:
        payload = json.loads(path.read_text(encoding="utf-8"))
    except (OSError, json.JSONDecodeError) as error:
        raise ValueError(f"cannot read artifact {path}: {error}") from error
    validate_artifact(payload, check_inputs=True, check_sources=check_sources)
    return payload


def decision_summary(decisions: list[dict[str, Any]]) -> dict[str, int]:
    return dict(
        sorted(Counter(decision["blocker_classification"] for decision in decisions).items())
    )


def validate_decisions(
    payload: object,
    artifact: dict[str, Any],
    stage_artifact: dict[str, Any],
) -> None:
    _require(isinstance(payload, dict), "decisions: expected an object")
    _require(payload.get("schema") == DECISIONS_SCHEMA, "decisions: unsupported schema")
    _require(payload.get("split") == "dev", "decisions must be dev-only")
    source = payload.get("source_artifact")
    _require(isinstance(source, dict), "source_artifact: expected an object")
    _validate_hash(source.get("sha256"), "source_artifact.sha256: invalid")
    selection = artifact["dev_audit_selection"]
    _require(
        payload.get("selection_sha256") == selection["sha256"],
        "decisions were not made against the frozen selection",
    )
    _require(
        stage_artifact.get("schema") == STAGE_AUDIT_SCHEMA,
        "decisions use an unsupported stage artifact",
    )
    _require(stage_artifact.get("split") == "dev", "stage artifact must be dev-only")
    _require(
        stage_artifact.get("provenance", {}).get("selection_sha256") == selection["sha256"],
        "stage evidence uses a different dev selection",
    )
    stage_by_key = {
        record["candidate_key"]: record for record in stage_artifact["candidates"]
    }
    selected = {record["candidate_key"]: record for record in selection["candidates"]}
    candidates = {record["candidate_key"]: record for record in artifact["missed_worthy"]}
    decisions = payload.get("decisions")
    _require(isinstance(decisions, list), "decisions: expected an array")
    _require(
        [decision.get("candidate_key") for decision in decisions]
        == [record["candidate_key"] for record in selection["candidates"]],
        "decisions must exactly follow frozen selection order",
    )
    _require(len(decisions) == len(selected), "decisions do not cover the selection")
    for index, decision in enumerate(decisions):
        label = f"decisions[{index}]"
        _require(isinstance(decision, dict), f"{label}: expected an object")
        key = decision.get("candidate_key")
        _require(key in selected, f"{label}: candidate was not selected")
        _require(
            decision.get("candidate_sha256") == selected[key]["candidate_sha256"],
            f"{label}: candidate hash mismatch",
        )
        _require(key in stage_by_key, f"{label}: stage evidence is missing")
        _require(
            decision.get("observed_stage") == stage_by_key[key]["stage"],
            f"{label}: observed stage drifted",
        )
        _require(
            decision.get("blocker_classification") in VALID_BLOCKER_CLASSIFICATIONS,
            f"{label}: invalid blocker classification",
        )
        for field in ("rationale", "smallest_sound_invariant"):
            _require(
                isinstance(decision.get(field), str) and bool(decision[field].strip()),
                f"{label}.{field}: expected a non-empty string",
            )
        evidence = decision.get("source_evidence")
        _require(isinstance(evidence, list) and evidence, f"{label}.source_evidence: empty")
        allowed_files = set(candidates[key]["source_files"])
        for evidence_index, item in enumerate(evidence):
            item_label = f"{label}.source_evidence[{evidence_index}]"
            _require(isinstance(item, dict), f"{item_label}: expected an object")
            _require(item.get("file") in allowed_files, f"{item_label}.file: not a member file")
            start = item.get("start_line")
            end = item.get("end_line")
            _require(
                isinstance(start, int) and isinstance(end, int) and 0 < start <= end,
                f"{item_label}: invalid line interval",
            )
            _require(
                isinstance(item.get("observation"), str) and bool(item["observation"].strip()),
                f"{item_label}.observation: expected text",
            )
    _require(
        payload.get("classification_summary") == decision_summary(decisions),
        "decision classification summary drifted",
    )
    proposal = payload.get("dev_proposal")
    _require(isinstance(proposal, dict), "dev_proposal: expected an object")
    _require(
        proposal.get("status") == "frozen-before-heldout-confirmation",
        "dev proposal was not frozen before held-out confirmation",
    )
    _require(proposal.get("route") in {"A", "B", "C", "D", "E"}, "invalid proposed route")
    _require(
        proposal.get("heldout_source_confirmation") == "not-run",
        "dev decision artifact must precede held-out confirmation",
    )
    for field in ("mechanism", "smallest_sound_invariant"):
        _require(
            isinstance(proposal.get(field), str) and bool(proposal[field].strip()),
            f"dev_proposal.{field}: expected text",
        )
    hard_negatives = proposal.get("hard_negatives")
    _require(
        isinstance(hard_negatives, list)
        and len(hard_negatives) >= 2
        and all(isinstance(item, str) and item.strip() for item in hard_negatives),
        "dev_proposal.hard_negatives: expected at least two obligations",
    )
    if proposal["route"] == "A":
        accepted = stage_artifact["summary"]["states"]["accepted-pair"]
        estimate = proposal.get("estimated_affected_dev_misses")
        _require(isinstance(estimate, dict), "route A needs a dev estimate")
        _require(
            estimate.get("lower_bound") == accepted,
            "route A lower bound must equal accepted raw dev pairs",
        )
    _require(
        proposal.get("hof_route_d_status") == "deferred-no-direct-pure-callback-evidence",
        "#806 may move only on direct pure-callback evidence",
    )


def load_and_validate_decisions(path: Path, artifact_path: Path) -> dict[str, Any]:
    artifact = load_and_validate_artifact(artifact_path)
    try:
        payload = json.loads(path.read_text(encoding="utf-8"))
    except (OSError, json.JSONDecodeError) as error:
        raise ValueError(f"cannot read decisions {path}: {error}") from error
    source = payload.get("source_artifact") if isinstance(payload, dict) else None
    _require(isinstance(source, dict), "source_artifact: expected an object")
    _require(relative_path(artifact_path) == source.get("path"), "source artifact path mismatch")
    _require(sha256_file(artifact_path) == source.get("sha256"), "source artifact hash mismatch")
    stage_source = payload.get("stage_artifact")
    _require(isinstance(stage_source, dict), "stage_artifact: expected an object")
    stage_path_raw = stage_source.get("path")
    _require(isinstance(stage_path_raw, str) and stage_path_raw, "stage artifact path missing")
    stage_path = project_path(stage_path_raw)
    _require(stage_path.is_file(), "stage artifact file is missing")
    _require(sha256_file(stage_path) == stage_source.get("sha256"), "stage artifact hash mismatch")
    stage_artifact = json.loads(stage_path.read_text(encoding="utf-8"))
    validate_decisions(payload, artifact, stage_artifact)
    return payload


def source_line_count(path: Path) -> int:
    return len(path.read_bytes().splitlines())


def build_source_bounds(artifact_path: Path, decisions_path: Path) -> dict[str, Any]:
    artifact = load_and_validate_artifact(artifact_path)
    decisions = load_and_validate_decisions(decisions_path, artifact_path)
    evidence_files = sorted(
        {
            evidence["file"]
            for decision in decisions["decisions"]
            for evidence in decision["source_evidence"]
        }
    )
    files = {}
    for path in evidence_files:
        frozen = artifact["source_files"][path]
        resolved = project_path(path)
        _require(resolved.is_file(), f"source bounds {path}: checkout missing")
        _require(sha256_file(resolved) == frozen["sha256"], f"source bounds {path}: hash mismatch")
        _require(resolved.stat().st_size == frozen["size_bytes"], f"source bounds {path}: size mismatch")
        files[path] = {
            "sha256": frozen["sha256"],
            "size_bytes": frozen["size_bytes"],
            "line_count": source_line_count(resolved),
        }
    return {
        "schema": SOURCE_BOUNDS_SCHEMA,
        "split": "dev",
        "method": "line bounds derived from the hash-checked frozen source bytes",
        "source_artifact": {
            "path": relative_path(artifact_path),
            "sha256": sha256_file(artifact_path),
        },
        "decisions": {
            "path": relative_path(decisions_path),
            "sha256": sha256_file(decisions_path),
        },
        "files": files,
    }


def validate_source_bounds(
    payload: object,
    artifact_path: Path,
    decisions_path: Path,
    *,
    check_sources: bool = False,
) -> None:
    _require(isinstance(payload, dict), "source bounds: expected an object")
    _require(payload.get("schema") == SOURCE_BOUNDS_SCHEMA, "source bounds: unsupported schema")
    _require(payload.get("split") == "dev", "source bounds must be dev-only")
    source_record = payload.get("source_artifact")
    _require(isinstance(source_record, dict), "source bounds: source artifact missing")
    _require(source_record.get("path") == relative_path(artifact_path), "source bounds path drifted")
    _require(source_record.get("sha256") == sha256_file(artifact_path), "source bounds hash drifted")
    decision_record = payload.get("decisions")
    _require(isinstance(decision_record, dict), "source bounds: decisions missing")
    _require(decision_record.get("path") == relative_path(decisions_path), "decision path drifted")
    _require(decision_record.get("sha256") == sha256_file(decisions_path), "decision hash drifted")
    artifact = load_and_validate_artifact(artifact_path)
    decisions = load_and_validate_decisions(decisions_path, artifact_path)
    evidence_files = {
        evidence["file"]
        for decision in decisions["decisions"]
        for evidence in decision["source_evidence"]
    }
    files = payload.get("files")
    _require(isinstance(files, dict) and set(files) == evidence_files, "source bound set drifted")
    for path, record in files.items():
        _require(isinstance(record, dict), f"source bounds {path}: expected an object")
        frozen = artifact["source_files"][path]
        _require(record.get("sha256") == frozen["sha256"], f"source bounds {path}: hash drifted")
        _require(
            record.get("size_bytes") == frozen["size_bytes"],
            f"source bounds {path}: size drifted",
        )
        line_count = record.get("line_count")
        _require(
            isinstance(line_count, int) and not isinstance(line_count, bool) and line_count > 0,
            f"source bounds {path}: invalid line count",
        )
        if check_sources:
            resolved = project_path(path)
            _require(resolved.is_file(), f"source bounds {path}: checkout missing")
            _require(sha256_file(resolved) == frozen["sha256"], f"source bounds {path}: source hash mismatch")
            _require(source_line_count(resolved) == line_count, f"source bounds {path}: line count mismatch")
    for decision in decisions["decisions"]:
        for evidence in decision["source_evidence"]:
            _require(
                evidence["end_line"] <= files[evidence["file"]]["line_count"],
                f"{decision['candidate_key']}: source evidence exceeds frozen file bounds",
            )


def load_and_validate_source_bounds(
    path: Path,
    artifact_path: Path,
    decisions_path: Path,
    *,
    check_sources: bool = False,
) -> dict[str, Any]:
    try:
        payload = json.loads(path.read_text(encoding="utf-8"))
    except (OSError, json.JSONDecodeError) as error:
        raise ValueError(f"cannot read source bounds {path}: {error}") from error
    validate_source_bounds(
        payload,
        artifact_path,
        decisions_path,
        check_sources=check_sources,
    )
    return payload


def _load_checked_record(record: object, label: str) -> tuple[Path, dict[str, Any]]:
    _require(isinstance(record, dict), f"{label}: expected an object")
    raw_path = record.get("path")
    _require(isinstance(raw_path, str) and raw_path, f"{label}.path: missing")
    path = project_path(raw_path)
    _require(path.is_file(), f"{label}: missing {path}")
    _require(sha256_file(path) == record.get("sha256"), f"{label}: hash mismatch")
    try:
        payload = json.loads(path.read_text(encoding="utf-8"))
    except json.JSONDecodeError as error:
        raise ValueError(f"{label}: invalid JSON: {error}") from error
    _require(isinstance(payload, dict), f"{label}: expected JSON object")
    return path, payload


def validate_closeout(payload: object) -> None:
    _require(isinstance(payload, dict), "closeout: expected an object")
    _require(payload.get("schema") == CLOSEOUT_SCHEMA, "closeout: unsupported schema")
    _require(payload.get("issue") == 816, "closeout: expected issue 816")
    artifacts = payload.get("artifacts")
    _require(isinstance(artifacts, dict), "closeout.artifacts: expected an object")
    expected_names = {
        "frontier",
        "dev_stage",
        "dev_decisions",
        "dev_source_bounds",
        "heldout_confirmation",
        "pricing_primary",
        "pricing_control",
        "pricing_status",
    }
    _require(set(artifacts) == expected_names, "closeout artifact set is incomplete")
    checked = {
        name: _load_checked_record(record, f"closeout.artifacts.{name}")
        for name, record in artifacts.items()
    }
    frontier_path, frontier = checked["frontier"]
    validate_artifact(frontier, check_inputs=True, check_sources=False)
    decisions_path, decisions = checked["dev_decisions"]
    load_and_validate_decisions(decisions_path, frontier_path)
    source_bounds_path, _ = checked["dev_source_bounds"]
    load_and_validate_source_bounds(
        source_bounds_path,
        frontier_path,
        decisions_path,
    )
    _, dev_stage = checked["dev_stage"]
    _require(dev_stage.get("schema") == STAGE_AUDIT_SCHEMA, "dev stage schema drifted")
    dev_summary = dev_stage.get("summary", {})
    _require(dev_summary.get("total") == 223, "dev stage total drifted")
    _require(
        dev_summary.get("states")
        == {
            "accepted-pair": 51,
            "candidate-only": 41,
            "extracted-no-candidate": 96,
            "missing-unit": 35,
        },
        "dev stage counts drifted",
    )
    _, confirmation = checked["heldout_confirmation"]
    _require(
        confirmation.get("schema") == "nose.missed_worthy_stage_confirmation.heldout.v1",
        "held-out confirmation schema drifted",
    )
    _require(
        confirmation.get("confirmation_gate", {}).get("passed") is True,
        "held-out confirmation did not pass",
    )
    heldout_summary = confirmation.get("summary", {})
    heldout_gate = confirmation.get("confirmation_gate", {})
    _require(heldout_summary.get("total") == 142, "held-out stage total drifted")
    _require(
        heldout_summary.get("states", {}).get("accepted-pair") == 42,
        "held-out accepted-pair count drifted",
    )
    _require(
        confirmation.get("provenance", {}).get("frozen_dev_decisions", {}).get("sha256")
        == sha256_file(decisions_path),
        "held-out confirmation did not use the frozen dev decisions",
    )
    _, pricing_primary = checked["pricing_primary"]
    _, pricing_control = checked["pricing_control"]
    _, pricing_status = checked["pricing_status"]
    _require(
        pricing_primary.get("schema") == "nose.query_regression_harness.v2"
        and pricing_control.get("schema") == "nose.query_regression_harness.v2",
        "pricing harness schema drifted",
    )
    _require(
        pricing_status.get("schema") == "nose.semantic_regression_check.v1"
        and pricing_status.get("status") == "pass",
        "pricing status did not pass",
    )
    _require(
        all(pricing_primary["summary"]["hashes_identical_by_repo"].values()),
        "pricing primary contains output drift",
    )
    _require(pricing_primary.get("repos") == pricing_control.get("repos"), "pricing repo set drifted")
    primary_output = pricing_status.get("primary", {}).get("output", {})
    _require(primary_output.get("unexpected_drifts") == [], "pricing status contains output drift")
    aggregate_signals = [
        signal
        for signal in pricing_status.get("primary", {}).get("runtime", {}).get("signals", [])
        if signal.get("scope") == "aggregate"
    ]
    _require(len(aggregate_signals) == 1, "pricing aggregate runtime signal missing")

    selected = payload.get("selected_route")
    _require(isinstance(selected, dict), "selected_route: expected an object")
    _require(selected.get("id") == "A", "closeout must select exactly one A-E route")
    _require(
        selected.get("status") == "selected-with-protocol-deviation",
        "Route A omission must be recorded as a protocol deviation",
    )
    follow_up = selected.get("follow_up_issue")
    _require(isinstance(follow_up, dict), "selected route follow-up missing")
    _require(
        follow_up.get("number") == 817
        and follow_up.get("url") == "https://github.com/corca-ai/nose/issues/817",
        "Route A must link follow-up issue 817",
    )
    for field in ("rationale", "smallest_sound_invariant", "protocol_deviation"):
        _require(
            isinstance(selected.get(field), str) and selected[field].strip(),
            f"selected_route.{field}: expected text",
        )
    rejected = payload.get("rejected_routes")
    _require(
        isinstance(rejected, dict) and set(rejected) == {"B", "C", "D", "E"},
        "rejected route set drifted",
    )
    _require(
        all(isinstance(reason, str) and reason.strip() for reason in rejected.values()),
        "every rejected route needs a rationale",
    )
    _require(
        payload.get("hof_roadmap_status") == "deferred-no-direct-pure-callback-evidence",
        "#806 moved without direct HOF evidence",
    )
    pricing = payload.get("pricing")
    _require(isinstance(pricing, dict), "closeout.pricing: expected an object")
    pricing_repos = pricing.get("repositories")
    _require(
        isinstance(pricing_repos, list)
        and len(pricing_repos) == 7
        and set(pricing_repos) == set(pricing_primary["repos"]),
        "closeout pricing repositories drifted",
    )
    baseline_by_repo = pricing_primary["summary"]["by_repo"]
    family_totals: Counter[str] = Counter()
    for repo in pricing_primary["repos"]:
        baseline = baseline_by_repo[repo]["baseline"]
        families = baseline.get("families")
        surfaces = baseline.get("surface_counts")
        _require(
            isinstance(families, list) and len(set(families)) == 1,
            f"pricing {repo} family count is unstable",
        )
        _require(
            isinstance(surfaces, list)
            and surfaces
            and all(surface == surfaces[0] for surface in surfaces),
            f"pricing {repo} surface count is unstable",
        )
        family_totals["raw"] += families[0]
        family_totals.update(surfaces[0])
    _require(pricing.get("raw_families") == family_totals["raw"] == 1263, "pricing raw count drifted")
    _require(pricing.get("default_families") == family_totals["default"] == 415, "pricing default count drifted")
    _require(pricing.get("hidden_families") == family_totals["hidden"] == 795, "pricing hidden count drifted")
    _require(
        pricing.get("divergence_families") == family_totals["divergence"] == 53,
        "pricing divergence count drifted",
    )
    _require(
        pricing.get("aggregate_baseline_median_ms")
        == round(pricing_primary["summary"]["aggregate_baseline_median_ms"], 2),
        "pricing aggregate baseline drifted",
    )
    aggregate_signal = aggregate_signals[0]
    _require(
        pricing.get("same_binary_output_drift") == len(primary_output["unexpected_drifts"]),
        "pricing output-drift summary drifted",
    )
    _require(
        pricing.get("control_adjusted_runtime_delta_pct")
        == aggregate_signal.get("adjusted_delta_pct"),
        "pricing adjusted runtime summary drifted",
    )
    _require(pricing.get("intervention_head_measured") is False, "#816 did not measure a head")
    _require(
        pricing.get("slice_dev_labeled_recovery_scenario_rows") == 25,
        "pricing labeled scenario drifted",
    )
    _require(pricing.get("raw_growth_scenario_pct") == 1.98, "pricing raw scenario drifted")
    _require(
        pricing.get("default_growth_scenario_pct") == 6.02,
        "pricing default scenario drifted",
    )
    closeout_confirmation = payload.get("confirmation")
    _require(isinstance(closeout_confirmation, dict), "closeout confirmation missing")
    accepted_keys = {
        decision["candidate_key"]
        for decision in decisions["decisions"]
        if decision.get("observed_stage") == "accepted-pair"
    }
    coherent_keys = {
        decision["candidate_key"]
        for decision in decisions["decisions"]
        if decision.get("blocker_classification")
        == "family-folding-or-overlap-matching"
    }
    _require(
        closeout_confirmation
        == {
            "dev_direct_accepted": dev_summary["states"]["accepted-pair"],
            "dev_misses": dev_summary["total"],
            "selected_direct_accepted": len(accepted_keys),
            "selected_source_coherent": len(coherent_keys),
            "heldout_direct_accepted": heldout_summary["states"]["accepted-pair"],
            "heldout_misses": heldout_summary["total"],
            "heldout_languages": heldout_gate["observed_accepted_languages"],
            "heldout_gate": "passed",
            "heldout_source_review": "none",
        },
        "closeout confirmation does not match checked evidence",
    )
    _require(
        accepted_keys == coherent_keys and len(accepted_keys) == 18,
        "selected accepted-pair and source-coherent cohorts differ",
    )
    docs = payload.get("documentation")
    expected_docs = {
        "docs/missed-worthy-frontier-816.md",
        "docs/benchmark.md",
        "docs/design.md",
        "docs/experiments.md",
        "docs/home.md",
        "bench/labels/README.md",
        "CHANGELOG.md",
    }
    _require(isinstance(docs, list) and set(docs) == expected_docs, "closeout documentation drifted")
    _require(all(project_path(path).is_file() for path in docs), "closeout documentation path missing")
    acceptance = payload.get("acceptance")
    expected_acceptance = {
        "checked_nose_0_18_artifact_complete_provenance",
        "arm1_exact_dev_and_heldout_counts",
        "deterministic_language_stratified_dev_selection_frozen",
        "every_selected_family_has_source_classification",
        "dev_proposal_committed_before_heldout_confirmation",
        "current_product_baseline_and_noise_priced_with_809_conventions",
        "actual_intervention_cost_deferred_to_817",
        "exactly_one_route_and_followup_issue",
        "route_tree_omission_recorded_as_protocol_deviation",
        "hof_roadmap_deferred_without_direct_evidence",
        "selected_and_rejected_routes_documented",
    }
    _require(
        isinstance(acceptance, dict)
        and set(acceptance) == expected_acceptance
        and all(value is True for value in acceptance.values()),
        "closeout acceptance is incomplete",
    )
    deviations = payload.get("protocol_deviations")
    _require(
        isinstance(deviations, list)
        and all(isinstance(item, dict) for item in deviations)
        and {item.get("id") for item in deviations if isinstance(item, dict)}
        == {"route-tree-omission", "intervention-cost-non-goal-conflict"}
        and all(
            isinstance(item.get("disposition"), str) and item["disposition"].strip()
            for item in deviations
        ),
        "closeout protocol deviations are incomplete",
    )
    next_actions = payload.get("result_dependent_next_action")
    _require(
        isinstance(next_actions, list)
        and len(next_actions) == 3
        and all(isinstance(item, str) and item.strip() for item in next_actions),
        "result-dependent next actions are incomplete",
    )


def load_and_validate_closeout(path: Path) -> dict[str, Any]:
    try:
        payload = json.loads(path.read_text(encoding="utf-8"))
    except (OSError, json.JSONDecodeError) as error:
        raise ValueError(f"cannot read closeout {path}: {error}") from error
    validate_closeout(payload)
    return payload


def render_dev_context(artifact: dict[str, Any], output: Path, context_lines: int = 8) -> None:
    candidates = {record["candidate_key"]: record for record in artifact["missed_worthy"]}
    lines = [
        "# Frozen #816 dev audit context",
        "",
        f"Selection SHA-256: `{artifact['dev_audit_selection']['sha256']}`",
        "",
        "This file renders only the pre-registered dev selection. It contains no held-out source.",
        "",
    ]
    for selection in artifact["dev_audit_selection"]["candidates"]:
        record = candidates[selection["candidate_key"]]
        lines.extend(
            [
                f"## {selection['order']:02d}. {record['candidate_key']}",
                "",
                f"- Language/repository: {record['language']} / {record['repo']}",
                f"- Selection lane: {selection['lane']} ({selection['phase']})",
                f"- Probe class: {record['class']}",
                f"- Label reason/channel/scope: {record['reason']} / {record['channel']} / {record['scope']}",
                f"- Candidate SHA-256: `{record['candidate_sha256']}`",
                "",
            ]
        )
        for member in record["members"]:
            path = project_path(member["file"])
            source = path.read_text(encoding="utf-8", errors="replace").splitlines()
            start = max(1, member["start_line"] - context_lines)
            end = min(len(source), member["end_line"] + context_lines)
            suffix = path.suffix.removeprefix(".")
            lines.extend(
                [
                    f"### {member['file']}:{member['start_line']}-{member['end_line']}",
                    "",
                    f"```{suffix}",
                ]
            )
            lines.extend(f"{line_number:>6}  {source[line_number - 1]}" for line_number in range(start, end + 1))
            lines.extend(["```", ""])
    output.write_text("\n".join(lines) + "\n", encoding="utf-8")


def run_self_test() -> None:
    pre_817_input = {
        "sha256": "2664d2935eaf8e86243dcf3592225c9f4884154ac7757c1307fd2a4281688e2c",
        "expected_worthy_recall": {
            "dev": {"hits": 2626, "n": 2849},
            "heldout": {"hits": 1949, "n": 2091},
        },
    }
    _require(
        checked_recall_profile(pre_817_input)
        == pre_817_input["expected_worthy_recall"],
        "registered recall profile did not round-trip",
    )
    unregistered = json.loads(json.dumps(pre_817_input))
    unregistered["sha256"] = "f" * 64
    try:
        checked_recall_profile(unregistered)
    except ValueError as error:
        _require("unregistered" in str(error), "unregistered profile failed unclearly")
    else:
        raise AssertionError("an unregistered evaluation report was accepted")
    substituted = json.loads(json.dumps(pre_817_input))
    substituted["expected_worthy_recall"]["dev"]["hits"] += 1
    try:
        checked_recall_profile(substituted)
    except ValueError as error:
        _require("recall" in str(error), "recall substitution failed unclearly")
    else:
        raise AssertionError("a registered evaluation recall substitution was accepted")
    evaluation = {
        "metrics": {
            split: {"OVERALL": {"worthy_recall": dict(counts)}}
            for split, counts in pre_817_input["expected_worthy_recall"].items()
        }
    }
    validate_evaluation_recall(evaluation, checked_recall_profile(pre_817_input))
    evaluation["metrics"]["heldout"]["OVERALL"]["worthy_recall"]["hits"] -= 1
    try:
        validate_evaluation_recall(evaluation, checked_recall_profile(pre_817_input))
    except ValueError as error:
        _require("heldout worthy recall drifted" in str(error), "metric drift failed unclearly")
    else:
        raise AssertionError("a checked evaluation metric substitution was accepted")

    def row(
        key: str,
        language: str,
        repo: str,
        probe_class: str,
        *,
        ge20: bool = False,
    ) -> dict[str, Any]:
        record: dict[str, Any] = {
            "candidate_key": key,
            "family_id": key,
            "repo": repo,
            "split": "dev",
            "language": language,
            "reason": "extract-helper",
            "channel": "structural",
            "scope": "production",
            "class": probe_class,
            "members": [
                {"file": f"{repo}/a", "start_line": 1, "end_line": 2},
                {"file": f"{repo}/b", "start_line": 3, "end_line": 4},
            ],
            "source_files": [f"{repo}/a", f"{repo}/b"],
        }
        if probe_class == "subdag-ceiling":
            record.update(
                intersection_mass=20 if ge20 else 12,
                unit_value_sizes=[20, 20],
                subdag_ge_8=True,
                subdag_ge_12=True,
                subdag_ge_20=ge20,
            )
        record["candidate_sha256"] = candidate_sha256(record)
        return record

    rows = [
        row("a-inline", "A", "r1", "inline-ceiling"),
        row("a-window", "A", "r2", "same-unit-window"),
        row("a-high", "A", "r3", "subdag-ceiling", ge20=True),
        row("a-unrec", "A", "r4", "unrecovered"),
        row("b-other", "B", "r5", "no-overlapping-unit"),
        row("b-high-1", "B", "r6", "subdag-ceiling", ge20=True),
        row("b-high-2", "B", "r7", "subdag-ceiling", ge20=True),
        row("b-low", "B", "r8", "subdag-ceiling"),
    ]
    selected = select_dev_audit(rows, per_language=3)
    reversed_selection = select_dev_audit(list(reversed(rows)), per_language=3)
    _require(selected == reversed_selection, "selection depends on input order")
    _require(Counter(item["language"] for item in selected) == {"A": 3, "B": 3}, "quota drift")
    _require(
        set(REQUIRED_RESIDUAL_LANES)
        <= {item["lane"] for item in selected},
        "required lanes were not retained",
    )

    tampered = dict(rows[0], reason="parameterize")
    try:
        _validate_candidate_records(
            [tampered],
            {
                "r1/a": {"sha256": "0" * 64, "size_bytes": 0},
                "r1/b": {"sha256": "0" * 64, "size_bytes": 0},
            },
            check_sources=False,
        )
    except ValueError as error:
        _require("hash mismatch" in str(error), "tamper test failed for wrong reason")
    else:
        raise AssertionError("candidate tampering was accepted")

    audited = rows[0]
    selection_record = {
        "candidate_key": audited["candidate_key"],
        "candidate_sha256": audited["candidate_sha256"],
        "language": audited["language"],
        "repo": audited["repo"],
        "lane": audit_lane(audited),
        "phase": "self-test",
        "order": 1,
    }
    fake_artifact = {
        "dev_audit_selection": {
            "candidates": [selection_record],
            "sha256": canonical_sha256([selection_record]),
        },
        "missed_worthy": [audited],
    }
    valid_decisions = {
        "schema": DECISIONS_SCHEMA,
        "split": "dev",
        "source_artifact": {"path": "artifact.json", "sha256": "0" * 64},
        "stage_artifact": {"path": "stage.json", "sha256": "1" * 64},
        "selection_sha256": fake_artifact["dev_audit_selection"]["sha256"],
        "decisions": [
            {
                "candidate_key": audited["candidate_key"],
                "candidate_sha256": audited["candidate_sha256"],
                "observed_stage": "candidate-only",
                "blocker_classification": "one-step-pure-helper-composition",
                "rationale": "The two members differ only by one local pure helper.",
                "smallest_sound_invariant": "Inline one proven-pure single-return helper.",
                "source_evidence": [
                    {
                        "file": audited["members"][0]["file"],
                        "start_line": 1,
                        "end_line": 2,
                        "observation": "The call site supplies the repeated computation.",
                    }
                ],
            }
        ],
        "classification_summary": {"one-step-pure-helper-composition": 1},
        "dev_proposal": {
            "status": "frozen-before-heldout-confirmation",
            "route": "A",
            "heldout_source_confirmation": "not-run",
            "mechanism": "Keep accepted pair endpoints represented through grouping.",
            "smallest_sound_invariant": "Every accepted pair retains endpoint coverage.",
            "hard_negatives": ["Do not infer A-C from A-B and B-C.", "Do not emit nested duplicates."],
            "estimated_affected_dev_misses": {"lower_bound": 1},
            "hof_route_d_status": "deferred-no-direct-pure-callback-evidence",
        },
    }
    fake_stage_artifact = {
        "schema": STAGE_AUDIT_SCHEMA,
        "split": "dev",
        "provenance": {
            "selection_sha256": fake_artifact["dev_audit_selection"]["sha256"]
        },
        "summary": {"states": {"accepted-pair": 1}},
        "candidates": [
            {"candidate_key": audited["candidate_key"], "stage": "candidate-only"}
        ],
    }
    # Use an accepted count of one only to exercise the route-A estimate check;
    # the selected record's observed stage independently exercises the exact join.
    validate_decisions(valid_decisions, fake_artifact, fake_stage_artifact)
    invalid_decisions = json.loads(json.dumps(valid_decisions))
    invalid_decisions["decisions"][0]["source_evidence"] = []
    try:
        validate_decisions(invalid_decisions, fake_artifact, fake_stage_artifact)
    except ValueError as error:
        _require("source_evidence" in str(error), "decision test failed for wrong reason")
    else:
        raise AssertionError("a decision without source evidence was accepted")

    with tempfile.TemporaryDirectory(prefix="nose-frontier-self-test-") as directory:
        output = Path(directory) / "canonical.json"
        output.write_text(canonical_json(selected), encoding="utf-8")
        _require(sha256_file(output) == canonical_sha256(selected), "canonical hash drift")
    print("missed-worthy frontier self-test passed")
