use super::*;

#[test]
fn semantic_pack_adoption_gates_json_reports_builtin_gate_status() {
    let out = run(&["semantic-pack", "adoption-gates", "--format", "json"]);
    let json: serde_json::Value =
        serde_json::from_str(&out).expect("adoption gates must emit valid JSON");

    assert_adoption_gate_totals(&json);
    let packs = json["packs"]
        .as_array()
        .expect("adoption gate packs should be an array");
    assert_default_gated_c_language_pack(packs);
    assert_default_gated_guava_pack(packs);
    assert_default_gated_swift_collection_factory_pack(packs);
}

fn assert_adoption_gate_totals(json: &serde_json::Value) {
    assert_eq!(json["schema_version"], 1);
    assert_eq!(json["status"], "ok");
    assert_eq!(json["policy"]["scope"], "compiled-builtin");
    assert_eq!(json["policy"]["external_influence"], "metadata-only");
    assert_eq!(json["totals"]["builtin_packs"], 46);
    assert_eq!(json["totals"]["builtin_default_packs"], 46);
    assert_eq!(json["totals"]["builtin_optional_packs"], 0);
    assert_eq!(json["totals"]["packs_needing_coverage"], 0);
    assert_eq!(json["totals"]["blocked_packs"], 0);
    assert!(json_array_strings(&json["checklist"], "builtin_optional")
        .contains(&"stable-pack-id-owner-version-policy"));
    assert!(json_array_strings(&json["checklist"], "builtin_default")
        .contains(&"query-regression-summary"));
    assert!(json_array_strings(&json["checklist"], "rollback")
        .contains(&"demote-pack-to-builtin-optional"));
}

fn assert_default_gated_c_language_pack(packs: &[serde_json::Value]) {
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

fn assert_default_gated_guava_pack(packs: &[serde_json::Value]) {
    let guava = packs
        .iter()
        .find(|pack| pack["id"] == "nose.java.ecosystem.guava.immutable_collection_factories")
        .expect("Guava immutable collection factory pack should be reported");
    assert_eq!(guava["trust"], "builtin-default");
    assert_eq!(guava["enabled_by_default"], true);
    assert_eq!(guava["adoption_status"], "default-gated");
    assert_eq!(guava["coverage_status"], "covered");
    assert!(json_array_strings(guava, "blockers").is_empty());
    assert!(json_array_strings(guava, "required_evidence").contains(&"query-regression-summary"));
    assert!(
        json_array_strings(guava, "required_evidence").contains(&"default-surface-noise-review")
    );
    assert!(
        json_array_strings(guava, "rollback_actions").contains(&"demote-pack-to-builtin-optional")
    );
}

fn assert_default_gated_swift_collection_factory_pack(packs: &[serde_json::Value]) {
    let swift = packs
        .iter()
        .find(|pack| pack["id"] == "nose.swift.stdlib.collection_factories")
        .expect("Swift stdlib collection factory pack should be reported");
    assert_eq!(swift["trust"], "builtin-default");
    assert_eq!(swift["enabled_by_default"], true);
    assert_eq!(swift["adoption_status"], "default-gated");
    assert_eq!(swift["coverage_status"], "covered");
    assert!(json_array_strings(swift, "blockers").is_empty());
    assert!(json_array_strings(swift, "required_evidence").contains(&"query-regression-summary"));
    assert!(
        json_array_strings(swift, "required_evidence").contains(&"default-surface-noise-review")
    );
}
