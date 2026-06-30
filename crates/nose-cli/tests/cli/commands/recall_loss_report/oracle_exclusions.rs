use super::*;

#[test]
fn recall_loss_report_attributes_await_oracle_exclusions_to_shared_scheduling_obligation() {
    let project = TempProject::new("recall_loss_cross_language_await_exclusions");
    project.write(
        "await.js",
        "async function idAsync(x) {\n  return await x + 1;\n}\n",
    );
    project.write(
        "await.ts",
        "async function idAsync(x: Promise<number>) {\n  return await x + 1;\n}\n",
    );
    project.write(
        "await.py",
        "async def id_async(x):\n    return await x + 1\n",
    );
    project.write(
        "await.rs",
        "async fn id_async(x: i32) -> i32 { async move { x + 1 }.await }\n",
    );
    project.write(
        "await.swift",
        "func idAsync(_ x: Int) async -> Int { return await x + 1 }\n",
    );
    let report_path = project.path().join("recall-loss.json");
    let out = run_raw(&[
        "verify",
        project.path().to_str().unwrap(),
        "--max-violations",
        "0",
        "--recall-loss-report",
        report_path.to_str().unwrap(),
    ]);
    assert!(out.contains("GATE: 0"));

    let report_text = fs::read_to_string(&report_path).expect("recall-loss report");
    let report: serde_json::Value =
        serde_json::from_str(&report_text).expect("recall-loss report JSON");
    let exclusion_obligations = report["oracle_exclusions"]["by_obligation"]
        .as_array()
        .expect("oracle_exclusions.by_obligation should be an array");
    assert!(
        exclusion_obligations.iter().any(|item| item["exclusion_reason"] == "uninterpretable"
            && item["attribution_reason"] == "unsupported-runtime-boundary"
            && item["obligation_family"] == "scheduling-boundary"
            && item["obligation_subreason"] == "async-await-scheduling-contract-missing"
            && item["oracle_excluded"].as_u64().unwrap_or(0) >= 5),
        "expected cross-language await exclusions to roll up under the shared scheduling obligation: {report}"
    );

    let excluded_units = report["oracle_exclusions"]["units"]
        .as_array()
        .expect("oracle_exclusions.units should be an array");
    for language in ["javascript", "typescript", "python", "rust", "swift"] {
        assert!(
            excluded_units
                .iter()
                .any(|item| item["reason"] == "uninterpretable"
                    && item["loc"]["language"] == language
                    && item["attribution"]["oracle_status"] == "excluded"
                    && item["attribution"]["capability_id"] == "runtime-boundary-model"
                    && item["attribution"]["missing_evidence"]
                        .as_array()
                        .is_some_and(|items| items
                            .iter()
                            .any(|value| value == "async-await-scheduling-contract"))),
            "expected {language} await exclusion attribution: {report}"
        );
    }
    assert!(
        report["by_obligation"]
            .as_array()
            .expect("by_obligation should be an array")
            .iter()
            .all(|item| item["obligation_subreason"]
                != "async-await-scheduling-contract-missing"),
        "oracle-excluded await rows should not be mixed into interpretable admission rollups: {report}"
    );
}

#[test]
fn recall_loss_report_attributes_async_function_and_block_oracle_exclusions() {
    let project = TempProject::new("recall_loss_cross_language_async_function_exclusions");
    project.write(
        "function.js",
        "async function idAsync(x) {\n  return x + 1;\n}\n",
    );
    project.write(
        "function.ts",
        "async function idAsync(x: number): Promise<number> {\n  return x + 1;\n}\n",
    );
    project.write("function.py", "async def id_async(x):\n    return x + 1\n");
    project.write(
        "function.rs",
        "async fn id_async(x: i32) -> i32 { x + 1 }\n",
    );
    project.write(
        "block.rs",
        "fn make_future(x: i32) -> impl Future<Output = i32> {\n    return async move { x + 1 };\n}\n",
    );
    project.write(
        "function.swift",
        "func idAsync(_ x: Int) async -> Int { return x + 1 }\n",
    );
    let report_path = project.path().join("recall-loss.json");
    let out = run_raw(&[
        "verify",
        project.path().to_str().unwrap(),
        "--max-violations",
        "0",
        "--recall-loss-report",
        report_path.to_str().unwrap(),
    ]);
    assert!(out.contains("GATE: 0"));

    let report_text = fs::read_to_string(&report_path).expect("recall-loss report");
    let report: serde_json::Value =
        serde_json::from_str(&report_text).expect("recall-loss report JSON");
    let exclusion_obligations = report["oracle_exclusions"]["by_obligation"]
        .as_array()
        .expect("oracle_exclusions.by_obligation should be an array");
    assert!(
        exclusion_obligations.iter().any(|item| item["exclusion_reason"] == "uninterpretable"
            && item["attribution_reason"] == "unsupported-runtime-boundary"
            && item["obligation_family"] == "scheduling-boundary"
            && item["obligation_subreason"] == "async-function-scheduling-contract-missing"
            && item["oracle_excluded"].as_u64().unwrap_or(0) >= 5),
        "expected cross-language async function exclusions under the shared scheduling obligation: {report}"
    );
    assert!(
        exclusion_obligations
            .iter()
            .any(|item| item["exclusion_reason"] == "uninterpretable"
                && item["attribution_reason"] == "unsupported-runtime-boundary"
                && item["obligation_family"] == "scheduling-boundary"
                && item["obligation_subreason"] == "async-block-scheduling-contract-missing"
                && item["oracle_excluded"].as_u64().unwrap_or(0) >= 1),
        "expected Rust async block exclusions under the shared async-block obligation: {report}"
    );

    let excluded_units = report["oracle_exclusions"]["units"]
        .as_array()
        .expect("oracle_exclusions.units should be an array");
    for language in ["javascript", "typescript", "python", "rust", "swift"] {
        assert!(
            excluded_units
                .iter()
                .any(|item| item["reason"] == "uninterpretable"
                    && item["loc"]["language"] == language
                    && item["attribution"]["oracle_status"] == "excluded"
                    && item["attribution"]["capability_id"] == "runtime-boundary-model"
                    && item["attribution"]["missing_evidence"]
                        .as_array()
                        .is_some_and(|items| items
                            .iter()
                            .any(|value| value == "async-function-scheduling-contract"))),
            "expected {language} async function exclusion attribution: {report}"
        );
    }
    assert!(
        excluded_units
            .iter()
            .any(|item| item["reason"] == "uninterpretable"
                && item["loc"]["language"] == "rust"
                && item["attribution"]["missing_evidence"]
                    .as_array()
                    .is_some_and(|items| items
                        .iter()
                        .any(|value| value == "async-block-scheduling-contract"))),
        "expected Rust async block exclusion attribution: {report}"
    );
    assert!(
        report["by_obligation"]
            .as_array()
            .expect("by_obligation should be an array")
            .iter()
            .all(|item| item["obligation_subreason"]
                != "async-function-scheduling-contract-missing"
                && item["obligation_subreason"] != "async-block-scheduling-contract-missing"),
        "oracle-excluded async rows should not be mixed into interpretable admission rollups: {report}"
    );
}

#[test]
fn recall_loss_report_attributes_non_js_async_runtime_api_exclusions() {
    let project = TempProject::new("recall_loss_non_js_async_runtime_api_exclusions");
    write_non_js_async_runtime_api_fixture(&project);
    let report_path = project.path().join("recall-loss.json");
    let out = run_raw(&[
        "verify",
        project.path().to_str().unwrap(),
        "--max-violations",
        "0",
        "--recall-loss-report",
        report_path.to_str().unwrap(),
    ]);
    assert!(out.contains("GATE: 0"));

    let report_text = fs::read_to_string(&report_path).expect("recall-loss report");
    let report: serde_json::Value =
        serde_json::from_str(&report_text).expect("recall-loss report JSON");
    assert_admission_obligation(
        &report,
        "scheduling-boundary",
        "task-spawn-scheduling-contract-missing",
        2,
    );
    assert_admission_obligation(
        &report,
        "scheduling-boundary",
        "task-yield-scheduling-contract-missing",
        1,
    );
    assert_admission_obligation(
        &report,
        "scheduling-boundary",
        "timer-scheduling-contract-missing",
        1,
    );
    assert_admission_obligation(
        &report,
        "scheduling-boundary",
        "future-drive-scheduling-contract-missing",
        2,
    );
    for (family, subreason, minimum) in [
        (
            "scheduling-boundary",
            "task-spawn-scheduling-contract-missing",
            2,
        ),
        (
            "success-error-result-channel",
            "async-aggregate-all-completion-contract-missing",
            1,
        ),
        (
            "cancellation-liveness-boundary",
            "async-aggregate-first-completion-contract-missing",
            1,
        ),
    ] {
        assert_exclusion_obligation(&report, family, subreason, minimum);
    }

    for (language, evidence) in [
        ("swift", "task-spawn-scheduling-contract"),
        ("swift", "async-aggregate-all-completion-contract"),
        ("swift", "exception-channel-contract"),
        ("rust", "async-aggregate-all-completion-contract"),
        ("rust", "async-aggregate-first-completion-contract"),
    ] {
        assert_excluded_runtime_unit(&report, language, evidence);
    }
    for language in ["python", "rust"] {
        assert_admission_rejection(&report, language, "task-spawn-scheduling-contract");
    }
    assert_admission_rejection(&report, "rust", "future-drive-scheduling-contract");
    for evidence in [
        "task-yield-scheduling-contract",
        "timer-scheduling-contract",
    ] {
        assert_admission_rejection(&report, "swift", evidence);
    }
    for subreason in [
        "async-aggregate-all-completion-contract-missing",
        "async-aggregate-first-completion-contract-missing",
        "async-aggregate-completion-contract-missing",
    ] {
        assert_no_admission_obligation(&report, subreason);
    }
}

fn write_non_js_async_runtime_api_fixture(project: &TempProject) {
    project.write(
        "asyncio_api.py",
        "import asyncio\n\
         def schedule():\n    return asyncio.create_task(work())\n\
         def sleep_timer():\n    return asyncio.sleep(1)\n\
         def gather_all(task):\n    return asyncio.gather(task)\n\
         def wait_some(task):\n    return asyncio.wait([task])\n",
    );
    project.write(
        "rust_runtime.rs",
        "use tokio::runtime::Handle;\n\
         use tokio_test::block_on as test_block_on;\n\
         fn spawn_it() { tokio::spawn(work()); }\n\
         fn join_it() { tokio::join!(work(), other()); }\n\
         fn select_it() { futures::select!(a = work() => a); }\n\
         fn drive_handle() { Handle::current().block_on(work()); }\n\
         fn drive_test() { test_block_on(work()); }\n",
    );
    project.write(
        "swift_task.swift",
        "func schedule() {\n  Task { work() }\n}\n\
         func detached() {\n  Task.detached { work() }\n}\n\
         func sleepTimer() {\n  Task.sleep(nanoseconds: 1)\n}\n\
         func yieldNow() {\n  Task.yield()\n}\n\
         func groupAll() {\n  withTaskGroup(of: Int.self) { group in\n    group.addTask { 1 }\n  }\n}\n\
         func groupThrowing() {\n  withThrowingTaskGroup(of: Int.self) { group in\n    group.addTask { 1 }\n  }\n}\n",
    );
}

#[test]
fn recall_loss_report_keeps_python_asyncio_namespace_enclosing_shadows_closed() {
    let project = TempProject::new("recall_loss_python_asyncio_namespace_enclosing_shadows");
    project.write(
        "shadowed_asyncio.py",
        "import asyncio\n\
         import asyncio as aio\n\
         def bare_shadow(asyncio):\n    async def inner():\n        return await asyncio.sleep(1)\n    return inner\n\
         def alias_shadow(aio):\n    async def inner():\n        return await aio.sleep(1)\n    return inner\n",
    );
    let report_path = project.path().join("recall-loss.json");
    let out = run_raw(&[
        "verify",
        project.path().to_str().unwrap(),
        "--max-violations",
        "0",
        "--recall-loss-report",
        report_path.to_str().unwrap(),
    ]);
    assert!(out.contains("GATE: 0"));

    let report_text = fs::read_to_string(&report_path).expect("recall-loss report");
    let report: serde_json::Value =
        serde_json::from_str(&report_text).expect("recall-loss report JSON");
    assert_no_admission_rejection(&report, "python", "timer-scheduling-contract");
    assert_no_excluded_runtime_unit(&report, "python", "timer-scheduling-contract");
}

fn report_array<'a>(
    report: &'a serde_json::Value,
    path: &[&str],
    description: &str,
) -> &'a [serde_json::Value] {
    let mut cursor = report;
    for key in path {
        cursor = &cursor[*key];
    }
    cursor
        .as_array()
        .unwrap_or_else(|| panic!("{description} should be an array"))
}

fn has_obligation_rollup(
    items: &[serde_json::Value],
    family: &str,
    subreason: &str,
    count_field: &str,
    minimum: u64,
) -> bool {
    items.iter().any(|item| {
        item["obligation_family"] == family
            && item["obligation_subreason"] == subreason
            && item[count_field].as_u64().unwrap_or(0) >= minimum
    })
}

fn assert_admission_obligation(
    report: &serde_json::Value,
    family: &str,
    subreason: &str,
    minimum: u64,
) {
    let obligations = report_array(report, &["by_obligation"], "by_obligation");
    assert!(
        has_obligation_rollup(
            obligations,
            family,
            subreason,
            "oracle_interpretable",
            minimum
        ),
        "expected interpretable {family}/{subreason} admission rollup: {report}"
    );
}

fn assert_no_admission_obligation(report: &serde_json::Value, subreason: &str) {
    let obligations = report_array(report, &["by_obligation"], "by_obligation");
    assert!(
        obligations
            .iter()
            .all(|item| item["obligation_subreason"] != subreason),
        "oracle-excluded rows for {subreason} should not be mixed into interpretable admission rollups: {report}"
    );
}

fn assert_exclusion_obligation(
    report: &serde_json::Value,
    family: &str,
    subreason: &str,
    minimum: u64,
) {
    let obligations = report_array(
        report,
        &["oracle_exclusions", "by_obligation"],
        "oracle_exclusions.by_obligation",
    );
    assert!(
        obligations
            .iter()
            .any(|item| item["exclusion_reason"] == "uninterpretable"
                && item["attribution_reason"] == "unsupported-runtime-boundary"
                && has_obligation_rollup(
                    std::slice::from_ref(item),
                    family,
                    subreason,
                    "oracle_excluded",
                    minimum
                )),
        "expected {family}/{subreason} oracle exclusion rollup: {report}"
    );
}

fn missing_evidence_contains(item: &serde_json::Value, evidence: &str) -> bool {
    item["missing_evidence"]
        .as_array()
        .is_some_and(|items| items.iter().any(|value| value == evidence))
}

fn attribution_missing_evidence_contains(item: &serde_json::Value, evidence: &str) -> bool {
    item["attribution"]["missing_evidence"]
        .as_array()
        .is_some_and(|items| items.iter().any(|value| value == evidence))
}

fn assert_excluded_runtime_unit(report: &serde_json::Value, language: &str, evidence: &str) {
    let units = report_array(
        report,
        &["oracle_exclusions", "units"],
        "oracle_exclusions.units",
    );
    assert!(
        units.iter().any(|item| item["reason"] == "uninterpretable"
            && item["loc"]["language"] == language
            && item["attribution"]["oracle_status"] == "excluded"
            && item["attribution"]["capability_id"] == "runtime-boundary-model"
            && attribution_missing_evidence_contains(item, evidence)),
        "expected {language} exclusion attribution for {evidence}: {report}"
    );
}

fn assert_admission_rejection(report: &serde_json::Value, language: &str, evidence: &str) {
    let rejections = report_array(report, &["admission_rejections"], "admission_rejections");
    assert!(
        rejections
            .iter()
            .any(|item| item["loc"]["language"] == language
                && item["capability_id"] == "runtime-boundary-model"
                && missing_evidence_contains(item, evidence)),
        "expected interpretable {language} attribution for {evidence}: {report}"
    );
}

fn assert_no_admission_rejection(report: &serde_json::Value, language: &str, evidence: &str) {
    let rejections = report_array(report, &["admission_rejections"], "admission_rejections");
    assert!(
        rejections
            .iter()
            .all(|item| item["loc"]["language"] != language
                || item["capability_id"] != "runtime-boundary-model"
                || !missing_evidence_contains(item, evidence)),
        "unexpected interpretable {language} attribution for {evidence}: {report}"
    );
}

fn assert_no_excluded_runtime_unit(report: &serde_json::Value, language: &str, evidence: &str) {
    let units = report_array(
        report,
        &["oracle_exclusions", "units"],
        "oracle_exclusions.units",
    );
    assert!(
        units.iter().all(|item| item["reason"] != "uninterpretable"
            || item["loc"]["language"] != language
            || item["attribution"]["oracle_status"] != "excluded"
            || item["attribution"]["capability_id"] != "runtime-boundary-model"
            || !attribution_missing_evidence_contains(item, evidence)),
        "unexpected {language} exclusion attribution for {evidence}: {report}"
    );
}
