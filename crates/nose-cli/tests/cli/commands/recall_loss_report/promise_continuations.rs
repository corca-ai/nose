use super::*;

#[test]
fn recall_loss_report_surfaces_promise_continuation_rows() {
    let project = TempProject::new("recall_loss_promise_continuation_rows");
    project.write(
        "promise.js",
        "function thenIt(p, f, r) { return p.then(f, r); }\n\
function catchIt(p, h) { return p.catch(h); }\n\
function finallyIt(p, h) { return p.finally(h); }\n\
async function load() { return 1; }\n\
function thenAsync(f) { return load().then(f); }\n\
function makeResolved() { return Promise.resolve(1); }\n\
function thenLocal(f) { return makeResolved().then(f); }\n\
function finallyLocal() { return makeResolved().finally(() => 9); }\n\
function makeBranched(flag) { if (flag) return Promise.resolve(1); return Promise.resolve(2); }\n\
function thenBranched(flag, f) { return makeBranched(flag).then(f); }\n\
function thenConstruct(executor, f) { return new Promise(executor).then(f); }\n\
function thenCall(db, id, f) { return db.get(id).then(f); }\n",
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
    // Twelve whole functions plus twelve independently audited product-fragment candidates.
    assert_eq!(report["summary"]["total_units"], 24);
    assert_eq!(report["summary"]["interpretable_units"], 21);
    assert_eq!(report["summary"]["excluded_units"], 3);
    assert_eq!(report["summary"]["admission_rejections"], 9);

    let obligations = report["by_obligation"]
        .as_array()
        .expect("by_obligation should be an array");
    for (family, subreason, count) in [
        (
            "ambiguous-selector-boundary",
            "promise-then-promise-like-receiver-proof-missing",
            1,
        ),
        (
            "rejection-channel",
            "promise-catch-rejection-continuation-contract-missing",
            1,
        ),
        (
            "rejection-channel",
            "promise-finally-settlement-continuation-contract-missing",
            1,
        ),
        (
            "success-error-result-channel",
            "promise-then-fulfillment-continuation-contract-missing",
            3,
        ),
        (
            "success-error-result-channel",
            "promise-constructor-receiver-producer-proof-missing",
            1,
        ),
        (
            "ambiguous-selector-boundary",
            "promise-call-return-member-callee-proof-missing",
            1,
        ),
    ] {
        assert!(
            obligations
                .iter()
                .any(|item| item["obligation_family"] == family
                    && item["obligation_subreason"] == subreason
                    && item["count"] == count),
            "expected Promise continuation obligation {family}/{subreason}: {report}"
        );
    }

    let rejections = report["admission_rejections"]
        .as_array()
        .expect("admission_rejections should be an array");
    assert_promise_continuation_missing_labels(rejections, &report);
}

#[test]
fn recall_loss_report_splits_imported_promise_call_return_receivers() {
    let project = TempProject::new("recall_loss_imported_promise_call_return_rows");
    project.write(
        "promise.js",
        "import { load, service } from './api';\n\
import * as client from './client';\n\
function thenImportedFunction(f) { return load().then(f); }\n\
function thenImportedBindingMember(f) { return service.load().then(f); }\n\
function thenImportedNamespaceMember(f) { return client.load().then(f); }\n",
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
    let rejections = report["admission_rejections"]
        .as_array()
        .expect("admission_rejections should be an array");

    assert!(
        rejections.iter().any(|item| missing_evidence_contains(
            item,
            "promise-call-return-imported-function-settled-value-contract"
        )),
        "imported function Promise receivers need settled-value proof, not just target identity: {report}"
    );
    assert!(
        rejections.iter().any(|item| missing_evidence_contains(
            item,
            "promise-call-return-imported-member-settled-value-contract"
        )),
        "imported member Promise receivers need settled-value proof, not just target identity: {report}"
    );
    assert!(
        !rejections.iter().any(|item| missing_evidence_contains(
            item,
            "promise-call-return-imported-member-callee-proof"
        )),
        "imported member call-target proof should be present for import-backed receivers: {report}"
    );

    let obligations = report["by_obligation"]
        .as_array()
        .expect("by_obligation should be an array");
    assert!(
        obligations.iter().any(|item| item["obligation_family"] == "success-error-result-channel"
            && item["obligation_subreason"]
                == "promise-call-return-imported-member-settled-value-contract-missing"),
        "imported member Promise receiver gaps should roll up under success/error result channel: {report}"
    );
}

fn assert_promise_continuation_missing_labels(
    rejections: &[serde_json::Value],
    report: &serde_json::Value,
) {
    for expected in [
        "promise-then-promise-like-receiver-proof",
        "promise-then-fulfillment-continuation-contract",
        "promise-then-rejection-continuation-contract",
        "promise-then-callback-demand-effect-contract",
        "promise-catch-rejection-continuation-contract",
        "promise-catch-callback-demand-effect-contract",
        "promise-finally-settlement-continuation-contract",
        "promise-finally-callback-demand-effect-contract",
        "promise-constructor-receiver-producer-proof",
        "promise-call-return-receiver-producer-proof",
        "promise-call-return-member-callee-proof",
    ] {
        assert!(
            rejections
                .iter()
                .any(|item| item["reason"] == "unsupported-runtime-boundary"
                    && item["missing_evidence"]
                        .as_array()
                        .is_some_and(|items| items.iter().any(|value| value == expected))),
            "expected Promise continuation missing evidence label {expected}: {report}"
        );
    }
    assert!(
        !rejections
            .iter()
            .any(|item| item["reason"] == "unsupported-runtime-boundary"
                && item["missing_evidence"]
                    .as_array()
                    .is_some_and(|items| items
                        .iter()
                        .any(|value| value == "promise-async-function-return-producer-proof"))),
        "same-file async function receiver proof should not stay reported as missing: {report}"
    );
    assert!(
        !rejections
            .iter()
            .any(|item| item["reason"] == "unsupported-runtime-boundary"
                && item["missing_evidence"].as_array().is_some_and(|items| items
                    .iter()
                    .any(|value| value
                        == "promise-call-return-direct-function-return-domain-proof"))),
        "same-file direct Promise-returning function proof should not stay reported as missing: {report}"
    );
    assert!(
        !rejections
            .iter()
            .filter(|item| item["unit"] == "finallyLocal")
            .any(|item| item["reason"] == "unsupported-runtime-boundary"),
        "safe local Promise.finally recovery should not add a runtime-boundary rejection: {report}"
    );
}

fn missing_evidence_contains(item: &serde_json::Value, label: &str) -> bool {
    item["reason"] == "unsupported-runtime-boundary"
        && item["missing_evidence"]
            .as_array()
            .is_some_and(|items| items.iter().any(|value| value == label))
}
