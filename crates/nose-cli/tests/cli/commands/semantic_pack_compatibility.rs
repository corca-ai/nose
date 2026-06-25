use super::*;

#[test]
fn semantic_pack_compatibility_json_reports_fail_closed_policy() {
    let out = run(&["semantic-pack", "compatibility", "--format", "json"]);
    let json: serde_json::Value =
        serde_json::from_str(&out).expect("compatibility must emit valid JSON");

    assert_eq!(json["schema_version"], 1);
    assert_eq!(json["status"], "ok");
    assert_eq!(json["current_nose_version"], env!("CARGO_PKG_VERSION"));
    assert_eq!(
        json_array_strings(&json["supported"], "manifest_api_versions"),
        vec!["nose.semantic-pack.v0"]
    );
    assert_eq!(
        json_array_strings(&json["supported"], "report_sources"),
        vec!["policy"]
    );
    assert_eq!(
        json["policy"]["manifest_nose_version"],
        "must-include-installed-version"
    );
    assert_eq!(json["policy"]["external_pack_influence"], "metadata-only");
    assert_eq!(json["policy"]["external_pack_execution"], "none");
    assert_eq!(json["policy"]["external_packs_enabled_by_default"], false);
    assert!(json_array_strings(&json["requirements"], "manifest")
        .contains(&"nose-version-range-includes-installed-version"));
    assert!(json_array_strings(&json["requirements"], "kernel")
        .contains(&"unsupported-capability-fail-closed"));
    assert!(json_array_strings(&json["requirements"], "kernel")
        .contains(&"metadata-only-external-rows-do-not-influence-analysis"));
    assert_eq!(json["checks"]["builtin_inventory_status"], "ok");
    assert_eq!(json["checks"]["builtin_packs"], 48);
    assert_eq!(json["checks"]["external_metadata_only"], true);
    assert_eq!(
        json_array_strings(&json["checks"], "external_influence_blockers"),
        vec![
            "data-only-registration",
            "dependency-backed-evidence-unavailable",
            "explicit-influence-trust-gate-missing",
            "executable-conformance-unavailable",
            "row-conflict"
        ]
    );

    let failure_modes = json["failure_modes"]
        .as_array()
        .expect("failure modes should be an array");
    assert!(failure_modes
        .iter()
        .any(|mode| mode["code"] == "unsupported-api-version"
            && mode["action"] == "reject-before-analysis"));
    assert!(failure_modes
        .iter()
        .any(|mode| mode["code"] == "unsupported-nose-version"
            && mode["action"] == "reject-before-analysis"));
    assert!(failure_modes
        .iter()
        .any(|mode| mode["code"] == "unsupported-influence"
            && mode["action"] == "block-external-influence"));
}
