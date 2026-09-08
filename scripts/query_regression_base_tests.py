"""Pinned base-workload provenance must work without ordinary prune-state fields."""

import copy
import hashlib
import json


def run_self_test(checker) -> None:
    def reseal(report):
        corpus = report["corpus"]
        bases = {row["repo"]: row["base"] for row in corpus["base_revisions"]}
        rows = [{**row, "base": bases[row["repo"]]} for row in corpus["repositories"]]
        corpus["selection_sha256"] = hashlib.sha256(
            json.dumps(rows, sort_keys=True, separators=(",", ":")).encode()
        ).hexdigest()

    def report(*, iterations=5, control=False):
        factory = checker.sample_control if control else checker.sample_report
        result = checker.sample_v3(factory(delta=2.0, iterations=iterations))
        result["command"] = "nose query <repo> base=<base> top=0 --format json"
        corpus = result["corpus"]
        for key in ("corpus_state", "corpus_state_sha256", "expected_corpus_state",
                    "expected_corpus_state_sha256", "subset_digest_after_prune"):
            del corpus[key]
        corpus["base_revisions"] = [{"repo": "repo-a", "base": "8" * 40}]
        corpus["selection"] = "first eligible source finding per repository"
        result["provenance"].update(
            base_workload_manifest=corpus["corpus_manifest"],
            base_workload_manifest_sha256=corpus["corpus_manifest_sha256"],
            harness_sha256="a" * 64, worktree_helper_sha256="b" * 64,
            worktrees_root="/stable/base-worktrees",
        )
        reseal(result)
        return result

    primary = report()
    before = copy.deepcopy(primary)
    assert checker.evaluate_gate(
        primary, same_binary_control=report(control=True),
        require_corpus_provenance=True, require_same_binary_control=True,
        runtime_gate="elapsed-v1",
    )["status"] == "pass"
    assert primary == before

    def reject(value, expected):
        try:
            checker.validate_structured_report(value, "base fixture", require_corpus_provenance=True)
        except checker.CheckFailed as error:
            assert expected in str(error), str(error)
        else:
            raise AssertionError(f"must reject {expected}")

    for key in ("base_workload_manifest", "base_workload_manifest_sha256",
                "harness_sha256", "worktree_helper_sha256", "worktrees_root"):
        broken = report()
        del broken["provenance"][key]
        reject(broken, key)
    for key in ("base_revisions", "selection"):
        broken = report()
        del broken["corpus"][key]
        reject(broken, key)
    broken = report()
    broken["corpus"]["base_revisions"].append(broken["corpus"]["base_revisions"][0])
    reject(broken, "selection")
    broken = report()
    broken["corpus"]["base_revisions"][0]["base"] = "not-a-commit"
    reject(broken, "base")
    broken = report()
    broken["corpus"]["base_revisions"][0]["repo"] = "unknown"
    reject(broken, "selection")
    broken = report()
    broken["corpus"]["selection_sha256"] = "f" * 64
    reject(broken, "selection_sha256")
    for key, value in (("base_workload_manifest", "/other/manifest.json"),
                       ("base_workload_manifest_sha256", "f" * 64)):
        broken = report()
        broken["provenance"][key] = value
        reject(broken, key)
    broken = report()
    broken["command"] = "nose query <repo> all top=0 --format json"
    reject(broken, "base=<base>")

    # A valid focused subset must preserve both checked-out and ancestor commits.
    focus = report(iterations=6)
    checker.validate_focused_report(primary, focus, ["repo-a"], 6, require_corpus_provenance=True)
    for collection, key in (("base_revisions", "base"), ("repositories", "commit")):
        changed = copy.deepcopy(focus)
        changed["corpus"][collection][0][key] = "9" * 40
        reseal(changed)
        try:
            checker.validate_focused_report(primary, changed, ["repo-a"], 6, require_corpus_provenance=True)
        except checker.CheckFailed as error:
            assert "revision" in str(error)
        else:
            raise AssertionError("focus cannot substitute a different base workload")
    projected = checker.project_report_repos(primary, ["repo-a"])
    checker.validate_structured_report(projected, "projected", require_corpus_provenance=True)
