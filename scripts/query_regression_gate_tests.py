"""Behavioral checks for prospective elapsed-time release decisions."""

import copy


def run_self_test(checker) -> None:
    def report(*, elapsed=-10.0, stages=None, iterations=5, changed=False):
        result = checker.sample_v3(checker.sample_report(
            delta=elapsed, iterations=iterations,
            hash_current=checker.SAMPLE_CHANGED_HASH if changed else checker.SAMPLE_OUTPUT_HASH,
        ))
        deltas = stages if stages is not None else [10.0] * iterations
        for run in result["runs"]:
            run["stages_ms"]["lower"] = 50.0 + (
                deltas[run["iteration"] - 1] if run["label"] == "current" else 0.0
            )
        result["summary"] = checker.summarize_runs(result["runs"], result["repos"])
        return result

    def check_failure(primary, message, **kwargs):
        try:
            checker.evaluate_gate(primary, runtime_gate="elapsed-v1", **kwargs)
        except checker.CheckFailed as error:
            assert message in str(error), str(error)
            return error
        raise AssertionError(f"expected failure: {message}")

    # A real stage regression remains visible while faster complete queries pass.
    primary = report()
    original = copy.deepcopy(primary)
    status = checker.evaluate_gate(primary, runtime_gate="elapsed-v1")
    assert status["status"] == "pass" and status["focused_repos"] == []
    assert status["primary"]["runtime"]["blocking"] == []
    warning = status["primary"]["runtime"]["warnings"][0]
    assert warning["stage"] == "lower" and warning["triggered"]
    assert warning["adjusted_delta_ms"] == 10.0
    assert "warning (primary; triggered)" in checker.markdown_summary(status, primary)
    assert primary == original

    # Historical callers keep the previous fail/focus decision and output shape.
    try:
        checker.evaluate_gate(primary)
    except checker.CheckFailed as error:
        assert error.exit_code == 3
        assert "runtime_gate" not in error.status["thresholds"]
        assert "warnings" not in error.status["primary"]["runtime"]
    else:
        raise AssertionError("historical all-metric gate must request stage focus")

    uncertain = report(iterations=6, stages=[10.0, 10.0, 10.0, 10.0, 0.0, 0.0])
    status = checker.evaluate_gate(uncertain, runtime_gate="elapsed-v1")
    assert status["status"] == "pass"
    assert status["primary"]["runtime"]["warnings"][0]["inconclusive"]
    assert "warning (primary; inconclusive)" in checker.markdown_summary(status, uncertain)

    # Whole-query regressions and insufficient focused evidence still block.
    slower = report(elapsed=10.0)
    error = check_failure(slower, "focused rerun")
    assert error.exit_code == 3 and error.status["focused_repos"] == ["repo-a"]
    focused = report(elapsed=10.0, iterations=6)
    error = check_failure(slower, "confirmed material runtime regression", focused_report=focused)
    assert error.status["status"] == "fail"
    for run in focused["runs"]:
        if run["label"] == "current" and run["iteration"] in (5, 6):
            run["elapsed_ms"] = 100.0
    focused["summary"] = checker.summarize_runs(focused["runs"], focused["repos"])
    check_failure(slower, "remains insufficient", focused_report=focused)

    # Faster peers cannot mask one repository; stage-only peers do not expand focus.
    mixed = copy.deepcopy(slower)
    peer = report(elapsed=-20.0)
    for run in peer["runs"]:
        run["repo"] = "repo-b"
    mixed["repos"].append("repo-b")
    mixed["runs"].extend(peer["runs"])
    mixed["runs"].sort(key=lambda run: (run["iteration"], run["repo"], run["pair_position"]))
    mixed["corpus"]["repositories"].append({"repo": "repo-b", "commit": "8" * 40})
    mixed["summary"] = checker.summarize_runs(mixed["runs"], mixed["repos"])
    error = check_failure(mixed, "focused rerun")
    assert error.status["focused_repos"] == ["repo-a"]
    status = checker.evaluate_gate(
        mixed, runtime_gate="elapsed-v1", focused_report=report(iterations=6),
    )
    assert status["status"] == "pass"
    rendered = checker.markdown_summary(status, mixed)
    assert "`repo-b:lower`" in rendered and "warning (primary; triggered)" in rendered
    assert "warning (focused; triggered)" in rendered
    cleared = checker.evaluate_gate(
        mixed, runtime_gate="elapsed-v1",
        focused_report=report(iterations=6, stages=[0.0] * 6),
    )
    rendered = checker.markdown_summary(cleared, mixed)
    assert "`repo-a:lower`" not in rendered and "`repo-b:lower`" in rendered

    # Output, provenance and raw timing validation precede the scope decision.
    check_failure(report(changed=True), "unexpected product output drift")
    check_failure(slower, "focused rerun output drift", focused_report=report(iterations=6, changed=True))
    check_failure(primary, "same-binary control is required", require_same_binary_control=True)
    malformed = report()
    malformed["runs"][0]["stages_ms"]["lower"] = float("nan")
    check_failure(malformed, "finite")
    insufficient = report(iterations=2)
    check_failure(insufficient, "focused rerun")
    check_failure(insufficient, "at least", focused_report=report(iterations=2))
    try:
        checker.evaluate_gate(primary, runtime_gate="unknown")
    except checker.CheckFailed as error:
        assert "runtime_gate" in str(error)
    else:
        raise AssertionError("unknown release policy must fail")
