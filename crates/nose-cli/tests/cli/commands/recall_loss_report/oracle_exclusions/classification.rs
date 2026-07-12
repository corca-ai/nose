#[test]
fn recall_loss_report_classifies_oracle_exclusions_by_actionable_bucket() {
    let report =
        super::await_oracle_exclusion_report("recall_loss_oracle_exclusion_classification");
    let classifications = report["oracle_exclusions"]["by_classification"]
        .as_array()
        .expect("oracle_exclusions.by_classification should be an array");
    let units = report["oracle_exclusions"]["units"]
        .as_array()
        .expect("oracle_exclusions.units should be an array");
    let classified: u64 = classifications
        .iter()
        .map(|item| item["count"].as_u64().unwrap_or(0))
        .sum();
    assert_eq!(
        classified,
        units.len() as u64,
        "classification rollups should cover every excluded unit: {report}"
    );
    assert!(
        classifications
            .iter()
            .any(|item| item["exclusion_reason"] == "uninterpretable"
                && item["classification"] == "semantic-boundary-attributed"
                && item["oracle_excluded"].as_u64().unwrap_or(0) >= 5
                && item["attributed_units"].as_u64().unwrap_or(0) >= 5
                && item["unattributed_units"].as_u64().unwrap_or(0) == 0),
        "expected semantic-boundary-attributed oracle exclusions: {report}"
    );
}
