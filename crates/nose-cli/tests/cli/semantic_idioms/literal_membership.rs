use super::*;

#[path = "literal_membership/fixture.rs"]
mod fixture;

// Broad fixture matrix for literal collection membership contracts. The size is
// intentional until the fixture setup has a clearer table-builder abstraction.
#[allow(clippy::too_many_lines)]
#[test]
fn query_mode_semantic_proves_literal_collection_membership() {
    let dir = std::env::temp_dir().join(format!("nose_literal_membership_{}", std::process::id()));
    let _ = fs::remove_dir_all(&dir);
    fs::create_dir_all(&dir).unwrap();
    fixture::write_literal_membership_fixtures(&dir);

    let semantic = query_min_json(&dir, "semantic");
    let semantic_json = query_json(&semantic);
    let semantic_families = query_families(&semantic_json);
    assert!(
        !semantic_families.is_empty(),
        "semantic mode should report literal membership families: {semantic}"
    );
    let semantic_text = semantic_json.to_string();
    let positive_family = semantic_families
        .iter()
        .map(serde_json::Value::to_string)
        .find(|family| {
            [
                "membership.py",
                "membership.rb",
                "python_set_factory.py",
                "python_deque_import.py",
                "python_deque_alias.py",
                "python_deque_namespace.py",
                "ruby_set_new_include.rb",
                "ruby_set_new_member.rb",
                "ruby_set_local.rb",
                "rust_std_hashset.rs",
            ]
            .iter()
            .all(|expected| family.contains(expected))
        })
        .unwrap_or_else(|| {
            panic!("semantic mode should include positive literal membership family: {semantic}")
        });
    for expected in [
        "membership.py",
        "membership.js",
        "membership.ts",
        "membership.go",
        "membership.rs",
        "membership.swift",
        "membership.rb",
        "ruby_member.rb",
        "ruby_set_new_include.rb",
        "ruby_set_new_member.rb",
        "ruby_set_local.rb",
        "python_set_factory.py",
        "python_tuple_factory.py",
        "python_frozenset_factory.py",
        "python_deque_import.py",
        "python_deque_alias.py",
        "python_deque_namespace.py",
        "python_module_set.py",
        "array_some.js",
        "array_some.ts",
        "array_indexof.js",
        "array_indexof.ts",
        "array_findindex.js",
        "array_findindex.ts",
        "array_filter_length.js",
        "array_filter_length.ts",
        "not_membership.py",
        "not_includes.js",
        "array_every.js",
        "array_every.ts",
        "array_filter_length_absence.js",
        "array_filter_length_absence.ts",
        "go_slices_package.go",
        "go_slices_alias.go",
        "go_slices_const.go",
        "go_slices_local.go",
        "module_set.js",
        "module_set.ts",
        "module_list.java",
        "java_local_list.java",
        "rust_local_array.rs",
        "rust_local_typed_array.rs",
        "rust_local_slice_ref.rs",
        "rust_local_vec.rs",
        "swift_local_array.swift",
        "rust_std_hashset.rs",
        "rust_std_btreeset.rs",
        "rust_std_vecdeque.rs",
    ] {
        assert!(
            semantic_text.contains(expected),
            "semantic mode should include {expected}: {semantic}"
        );
    }
    for unexpected in [
        "wrong_element.py",
        "wrong_collection.js",
        "swift_wrong_element.swift",
        "swift_wrong_collection.swift",
        "js_in_array_a.js",
        "js_in_array_b.js",
        "array_some_wrong_element.js",
        "array_some_wrong_collection.ts",
        "array_some_loose.js",
        "array_indexof_wrong_element.js",
        "array_indexof_wrong_collection.ts",
        "array_indexof_value.js",
        "array_indexof_ne_zero.js",
        "array_indexof_reversed_gt_zero.js",
        "array_findindex_wrong_element.js",
        "array_findindex_wrong_collection.ts",
        "array_findindex_loose.js",
        "array_findindex_value.js",
        "array_findindex_ne_zero.js",
        "array_filter_length_wrong_element.js",
        "array_filter_length_wrong_collection.ts",
        "array_filter_length_loose.js",
        "array_filter_length_value.js",
        "array_filter_length_absence_wrong_element.js",
        "array_filter_length_absence_wrong_collection.ts",
        "array_every_wrong_element.js",
        "array_every_wrong_collection.ts",
        "array_every_loose.js",
        "python_module_tuple.py",
        "substring.rs",
        "module_set_mutated.js",
        "python_module_mutated.py",
        "module_set_shadowed.ts",
        "module_list_shadowed.java",
        "python_factory_shadowed.py",
        "python_deque_wrong_element.py",
        "python_deque_wrong_collection.py",
        "python_deque_missing_import.py",
        "python_deque_shadowed.py",
        "python_deque_mutated.py",
        "go_slices_mutated.go",
        "go_slices_local_mutated.go",
        "go_slices_unimported.go",
        "java_local_list_mutated.java",
        "rust_local_mutated.rs",
        "rust_local_custom_receiver.rs",
        "swift_local_mutated.swift",
        "swift_custom_receiver.swift",
        "rust_std_wrong_element.rs",
        "rust_std_wrong_collection.rs",
        "rust_std_mutated.rs",
        "ruby_set_wrong_element.rb",
        "ruby_set_wrong_collection.rb",
        "ruby_set_missing_require.rb",
        "ruby_set_shadowed.rb",
        "ruby_set_mutated.rb",
    ] {
        assert!(
            !positive_family.contains(unexpected),
            "semantic mode must preserve literal membership boundaries: {semantic}"
        );
    }
    assert!(
        !semantic_text.contains("js_in_array_a.js") && !semantic_text.contains("js_in_array_b.js"),
        "semantic mode must not treat JavaScript `in` as collection membership: {semantic}"
    );
    let absence_family = semantic_families
        .iter()
        .map(serde_json::Value::to_string)
        .find(|family| family.contains("not_membership.py"))
        .unwrap_or_else(|| {
            panic!("semantic mode should include negated membership family: {semantic}")
        });
    for expected in [
        "not_includes.js",
        "array_every.js",
        "array_every.ts",
        "array_filter_length_absence.js",
        "array_filter_length_absence.ts",
    ] {
        assert!(
            absence_family.contains(expected),
            "semantic mode should include negated membership {expected}: {semantic}"
        );
    }
    for unexpected in [
        "array_every_wrong_element.js",
        "array_every_wrong_collection.ts",
        "array_filter_length_absence_wrong_element.js",
        "array_filter_length_absence_wrong_collection.ts",
    ] {
        assert!(
            !absence_family.contains(unexpected),
            "semantic mode must preserve negated membership boundaries: {semantic}"
        );
    }

    let _ = fs::remove_dir_all(&dir);
}
