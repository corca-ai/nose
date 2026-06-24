use super::*;

#[test]
fn semantic_pack_adoption_gates_json_reports_builtin_gate_status() {
    let out = run(&["semantic-pack", "adoption-gates", "--format", "json"]);
    let json: serde_json::Value =
        serde_json::from_str(&out).expect("adoption gates must emit valid JSON");

    assert_eq!(json["schema_version"], 1);
    assert_eq!(json["status"], "ok");
    assert_eq!(json["policy"]["scope"], "compiled-builtin");
    assert_eq!(json["policy"]["external_influence"], "metadata-only");
    assert_eq!(json["totals"]["builtin_packs"], 44);
    assert_eq!(json["totals"]["builtin_default_packs"], 44);
    assert_eq!(json["totals"]["builtin_optional_packs"], 0);
    assert_eq!(json["totals"]["packs_needing_coverage"], 0);
    assert_eq!(json["totals"]["blocked_packs"], 0);
    assert!(json_array_strings(&json["checklist"], "builtin_optional")
        .contains(&"stable-pack-id-owner-version-policy"));
    assert!(json_array_strings(&json["checklist"], "builtin_default")
        .contains(&"query-regression-summary"));
    assert!(json_array_strings(&json["checklist"], "rollback")
        .contains(&"demote-pack-to-builtin-optional"));

    let packs = json["packs"]
        .as_array()
        .expect("adoption gate packs should be an array");
    let c_language = packs
        .iter()
        .find(|pack| pack["id"] == "nose.lang.c")
        .expect("C language pack should be reported");
    assert_eq!(c_language["trust"], "builtin-default");
    assert_eq!(c_language["enabled_by_default"], true);
    assert_eq!(c_language["adoption_status"], "default-gated");
    assert_eq!(c_language["coverage_status"], "covered");
    assert!(json_array_strings(c_language, "blockers").is_empty());
    assert!(
        json_array_strings(c_language, "required_evidence").contains(&"runtime-drift-measurement")
    );
}
