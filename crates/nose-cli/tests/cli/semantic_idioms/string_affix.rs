use super::*;

#[test]
fn query_mode_semantic_hardens_js_ts_string_affix_receivers() {
    let dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/string_affix_550");

    let semantic = query_min_json(&dir, "semantic");
    let semantic_json = query_json(&semantic);
    assert_proved_string_affix_families(&semantic_json, &semantic);
    assert_string_affix_hard_negatives(&semantic_json, &semantic);
}

#[test]
fn query_mode_semantic_admits_only_proven_ruby_string_affix_receivers() {
    let dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/string_affix_551");

    let semantic = query_min_json(&dir, "semantic");
    let semantic_json = query_json(&semantic);
    assert_proved_ruby_string_affix_families(&semantic_json, &semantic);
    assert_ruby_string_affix_hard_negatives(&semantic_json, &semantic);
}

fn assert_proved_string_affix_families(semantic_json: &serde_json::Value, semantic: &str) {
    assert!(
        family_contains_all(
            semantic_json,
            &[
                "prefix.py",
                "prefix.ts",
                "prefix.go",
                "prefix.rs",
                "prefix.java",
                "shadowed_constructor_patch.ts",
            ],
        ),
        "semantic mode should report the proved prefix affix family: {semantic}"
    );
    assert!(
        family_contains_all(semantic_json, &["suffix.py", "suffix.ts"]),
        "semantic mode should report the proved suffix affix family: {semantic}"
    );
    assert!(
        !family_contains_all(semantic_json, &["prefix.py", "suffix.ts"]),
        "prefix and suffix coordinates must stay distinct: {semantic}"
    );
}

fn assert_proved_ruby_string_affix_families(semantic_json: &serde_json::Value, semantic: &str) {
    assert!(
        family_contains_all(semantic_json, &["prefix.rb", "prefix_same.rb"]),
        "semantic mode should report the proved Ruby prefix affix family: {semantic}"
    );
    assert!(
        family_contains_all(semantic_json, &["suffix.rb", "suffix_same.rb"]),
        "semantic mode should report the proved Ruby suffix affix family: {semantic}"
    );
    assert!(
        !family_contains_all(semantic_json, &["prefix.rb", "suffix.rb"]),
        "Ruby prefix and suffix coordinates must stay distinct: {semantic}"
    );
}

fn assert_string_affix_hard_negatives(semantic_json: &serde_json::Value, semantic: &str) {
    for unexpected in [
        "prefix.js",
        "borrowed_prototype.js",
        "custom_same_name.js",
        "offset.ts",
        "string_object_wrapper.ts",
        "nullable.ts",
        "optional.ts",
        "patched.ts",
        "patched_after.ts",
        "conditional_patch.ts",
        "define_property_patch.ts",
        "nested_param_string_patch.ts",
        "nested_param_object_define_property_patch.ts",
        "block_scoped_string_then_global_patch.ts",
        "block_scoped_object_then_define_property.ts",
        "affix_negative.py",
        "receiver_negative.rs",
    ] {
        assert!(
            !family_contains_all(semantic_json, &["prefix.py", unexpected])
                && !family_contains_all(semantic_json, &["prefix.ts", unexpected]),
            "semantic mode must keep {unexpected} out of the proved affix family: {semantic}"
        );
    }
}

fn assert_ruby_string_affix_hard_negatives(semantic_json: &serde_json::Value, semantic: &str) {
    for unexpected in [
        "untyped_receiver.rb",
        "custom_same_name.rb",
        "multi_affix.rb",
        "wrong_receiver.rb",
        "direction_mismatch.rb",
        "monkey_patch.rb",
        "class_eval_patch.rb",
        "define_method_patch.rb",
    ] {
        assert!(
            !family_contains_all(semantic_json, &["prefix.rb", unexpected])
                && !family_contains_all(semantic_json, &["prefix_same.rb", unexpected]),
            "semantic mode must keep {unexpected} out of the proved Ruby prefix family: {semantic}"
        );
    }
}
