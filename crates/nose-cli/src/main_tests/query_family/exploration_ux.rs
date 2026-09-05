use super::*;

#[test]
fn cross_language_relation_does_not_promise_a_callable_helper() {
    let mut family = fam(2, 2, &[None, None]);
    let mut helper = loc_at("math.py", 1, 15, nose_il::UnitKind::Function);
    helper.name = Some("clamp".into());
    helper.lang = "python".into();
    family.locations = vec![helper, loc_at("app.go", 1, 15, nose_il::UnitKind::Block)];
    assert_eq!(family.extraction_shape(), "consolidate-cross-language");
    let hint = family_hint(&family);
    assert!(
        hint.contains("across languages") && hint.contains("not established"),
        "{hint}"
    );
    assert!(family_existing_helper(&family).is_none());
}

#[test]
fn measured_zero_invariants_do_not_recommend_extraction() {
    let mut family = fam_at(&[("a.rs", 1, 8), ("b.rs", 1, 8)]);
    family.shared_lines = 0;
    family.shared_weight = 6.0;
    family.display_params = Some(1);
    assert_eq!(family.extractability(), 0.0);
    assert!(family_hint(&family).contains("no invariant source lines"));
}

#[test]
fn common_punctuation_does_not_support_helper_advice() {
    let mut family = fam_at(&[("a.rs", 1, 8), ("b.rs", 1, 8)]);
    family.shared_lines = 1;
    family.shared_weight = 0.0;
    family.display_params = Some(2);
    assert!(family_hint(&family).contains("no substantive ranking weight"));
}

#[test]
fn source_comparison_reports_sample_missing_and_truncated_members() {
    let dir = std::env::temp_dir().join(format!("nose-source-coverage-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let file = dir.join("source.rs");
    std::fs::write(&file, "work();\n".repeat(121)).unwrap();
    let path = file.to_str().unwrap();
    let mut family = fam_at(&[(path, 1, 121); 9]);
    family.locations[1].file = dir.join("missing.rs").to_string_lossy().into_owned();
    let evidence = crate::query_source_evidence::collect(&family, true);
    assert_eq!(evidence["status"], "partial");
    assert_eq!(evidence["coverage"]["attempted_members"], 8);
    assert_eq!(evidence["coverage"]["available_members"], 7);
    assert_eq!(evidence["coverage"]["omitted_members"], 1);
    assert_eq!(evidence["coverage"]["complete"], false);
    assert_eq!(evidence["members"][1]["status"], "unavailable");
    assert_eq!(evidence["members"][0]["truncated"], true);
    assert_eq!(evidence["diffs"][0]["truncated"], true);
    assert_eq!(evidence["shared_lines"], 120);
    std::fs::remove_dir_all(dir).unwrap();
}

#[test]
fn source_diff_preserves_pair_identity_and_absolute_line_coordinates() {
    let dir = std::env::temp_dir().join(format!("nose-source-pair-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let a = dir.join("a.rs");
    let b = dir.join("b.rs");
    std::fs::write(&a, "// context\nstart();\nfinish();\n").unwrap();
    std::fs::write(&b, "// context\nstart();\nadditional();\nfinish();\n").unwrap();
    let family = fam_at(&[(a.to_str().unwrap(), 2, 3), (b.to_str().unwrap(), 2, 4)]);
    let evidence = crate::query_source_evidence::collect(&family, true);
    let diff = &evidence["diffs"][0];
    assert_eq!(evidence["status"], "complete");
    assert_eq!(
        diff["a"]["member_id"],
        baseline::member_id(&family.locations[0])
    );
    assert_eq!(
        diff["b"]["member_id"],
        baseline::member_id(&family.locations[1])
    );
    assert_eq!(diff["lines"][1]["tag"], "+");
    assert_eq!(diff["lines"][1]["a_line"], serde_json::Value::Null);
    assert_eq!(diff["lines"][1]["b_line"], 3);
    assert_eq!(diff["lines"][2]["a_line"], 3);
    assert_eq!(diff["lines"][2]["b_line"], 4);
    assert_eq!(diff["scope"], "pair-only");
    let mut missing_anchor = family.clone();
    missing_anchor.locations[0].end_line = 99;
    let evidence = crate::query_source_evidence::collect(&missing_anchor, true);
    assert_eq!(evidence["status"], "unavailable");
    assert_eq!(evidence["members"][0]["reason"], "range-unavailable");
    assert!(evidence.get("skeleton").is_none());
    std::fs::remove_dir_all(dir).unwrap();
}
