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
    assert!(family_hint(&family).contains("only common syntax"));
}
