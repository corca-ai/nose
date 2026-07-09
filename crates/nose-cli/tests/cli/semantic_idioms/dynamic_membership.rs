use super::*;

#[test]
fn query_mode_semantic_keeps_unproven_contains_calls_distinct() {
    let dir = std::env::temp_dir().join(format!("nose_unproven_contains_{}", std::process::id()));
    let _ = fs::remove_dir_all(&dir);
    fs::create_dir_all(&dir).unwrap();
    fs::write(
        dir.join("java_list.java"),
        "import java.util.List;\n\nclass C { static boolean f(List<String> values, String value) { return values.contains(value); } }\n",
    )
    .unwrap();
    fs::write(
        dir.join("java_string_negative.java"),
        "class C { static boolean f(String values, String value) { return values.contains(value); } }\n",
    )
    .unwrap();
    fs::write(
        dir.join("rust_slice.rs"),
        "pub fn f(values: &[&str], value: &str) -> bool {\n    values.contains(&value)\n}\n",
    )
    .unwrap();

    let semantic = query_min_json(&dir, "semantic");
    let semantic_json: serde_json::Value =
        serde_json::from_str(&semantic).expect("semantic query should emit JSON");
    let semantic_text = semantic_json.to_string();
    assert!(
        !semantic_text.contains("java_string_negative.java"),
        "semantic mode must not merge unproven collection membership with substring contains: {semantic}"
    );

    let _ = fs::remove_dir_all(&dir);
}

// Broad fixture matrix for typed dynamic collection membership contracts. The size is
// intentional until the fixture setup has a clearer table-builder abstraction.
#[allow(clippy::too_many_lines)]
#[test]
fn query_mode_semantic_proves_typed_dynamic_collection_membership() {
    let dir = std::env::temp_dir().join(format!(
        "nose_typed_dynamic_membership_{}",
        std::process::id()
    ));
    let _ = fs::remove_dir_all(&dir);
    fs::create_dir_all(&dir).unwrap();
    fs::write(
        dir.join("membership.py"),
        "def f(values: list[str], value: str, other: str) -> bool:\n    return value in values\n",
    )
    .unwrap();
    fs::write(
        dir.join("membership.ts"),
        "function f(values: string[], value: string, other: string): boolean {\n  return values.includes(value);\n}\n",
    )
    .unwrap();
    fs::write(
        dir.join("membership.go"),
        "package p\n\nimport \"slices\"\n\nfunc F(values []string, value string, other string) bool {\n    return slices.Contains(values, value)\n}\n",
    )
    .unwrap();
    fs::write(
        dir.join("membership.rs"),
        "pub fn f(values: &[&str], value: &str, other: &str) -> bool {\n    values.contains(&value)\n}\n",
    )
    .unwrap();
    fs::write(
        dir.join("membership.java"),
        "import java.util.List;\n\nclass C { static boolean f(List<String> values, String value, String other) { return values.contains(value); } }\n",
    )
    .unwrap();
    fs::write(
        dir.join("membership.swift"),
        "func f(_ values: [String], _ value: String, _ other: String) -> Bool {\n    return values.contains(value)\n}\n",
    )
    .unwrap();
    fs::write(
        dir.join("membership_tuple.py"),
        "def f(values: tuple[str, ...], value: str, other: str) -> bool:\n    return value in values\n",
    )
    .unwrap();
    fs::write(
        dir.join("membership_alias_sequence.py"),
        "from typing import Sequence as Values\n\ndef f(values: Values[str], value: str, other: str, other_values: Values[str]) -> bool:\n    return value in values\n",
    )
    .unwrap();
    fs::write(
        dir.join("membership_alias_container.py"),
        "from collections.abc import Container as Values\n\ndef f(values: Values[str], value: str, other: str, other_values: Values[str]) -> bool:\n    return value in values\n",
    )
    .unwrap();
    fs::write(
        dir.join("membership_alias_set.py"),
        "from typing import Set as Values\n\ndef f(values: Values[str], value: str, other: str, other_values: Values[str]) -> bool:\n    return value in values\n",
    )
    .unwrap();
    fs::write(
        dir.join("membership_queue.java"),
        "import java.util.Queue;\n\nclass C { static boolean f(Queue<String> values, String value, String other) { return values.contains(value); } }\n",
    )
    .unwrap();
    fs::write(
        dir.join("membership_vecdeque.rs"),
        "use std::collections::VecDeque;\n\npub fn f(values: &VecDeque<&str>, value: &str, other: &str) -> bool {\n    values.contains(&value)\n}\n",
    )
    .unwrap();
    fs::write(
        dir.join("wrong_element.ts"),
        "function f(values: string[], value: string, other: string): boolean {\n  return values.includes(other);\n}\n",
    )
    .unwrap();
    fs::write(
        dir.join("wrong_element.swift"),
        "func f(_ values: [String], _ value: String, _ other: String) -> Bool {\n    return values.contains(other)\n}\n",
    )
    .unwrap();
    fs::write(
        dir.join("wrong_receiver.swift"),
        "func f(_ values: [String], _ value: String, _ other: String, _ otherValues: [String]) -> Bool {\n    return otherValues.contains(value)\n}\n",
    )
    .unwrap();
    fs::write(
        dir.join("custom_receiver.swift"),
        "struct Values {\n    func contains(_ value: String) -> Bool {\n        return false\n    }\n}\n\nfunc f(_ values: Values, _ value: String, _ other: String) -> Bool {\n    return values.contains(value)\n}\n",
    )
    .unwrap();
    fs::write(
        dir.join("mutated_receiver.swift"),
        "func f(_ value: String, _ other: String) -> Bool {\n    var values = [\"red\", \"blue\"]\n    values.append(\"green\")\n    return values.contains(value)\n}\n",
    )
    .unwrap();
    fs::write(
        dir.join("membership_alias_wrong_element.py"),
        "from typing import Sequence as Values\n\ndef f(values: Values[str], value: str, other: str, other_values: Values[str]) -> bool:\n    return other in values\n",
    )
    .unwrap();
    fs::write(
        dir.join("membership_alias_wrong_receiver.py"),
        "from typing import Sequence as Values\n\ndef f(values: Values[str], value: str, other: str, other_values: Values[str]) -> bool:\n    return value in other_values\n",
    )
    .unwrap();
    fs::write(
        dir.join("membership_alias_unresolved.py"),
        "def f(values: Values[str], value: str, other: str, other_values: Values[str]) -> bool:\n    return value in values\n",
    )
    .unwrap();
    fs::write(
        dir.join("membership_alias_shadowed.py"),
        "from typing import Sequence as Values\nValues = str\n\ndef f(values: Values[str], value: str, other: str, other_values: Values[str]) -> bool:\n    return value in values\n",
    )
    .unwrap();
    fs::write(
        dir.join("string_negative.java"),
        "class C { static boolean f(String values, String value, String other) { return values.contains(value); } }\n",
    )
    .unwrap();

    let semantic = query_min_json(&dir, "semantic");
    let semantic_json = query_json(&semantic);
    let semantic_families = query_families(&semantic_json);
    let expected = [
        "membership.py",
        "membership.ts",
        "membership.go",
        "membership.rs",
        "membership.java",
        "membership.swift",
        "membership_tuple.py",
        "membership_alias_sequence.py",
        "membership_alias_container.py",
        "membership_alias_set.py",
        "membership_queue.java",
        "membership_vecdeque.rs",
    ];
    let positive_family = semantic_families
        .iter()
        .find(|family| {
            let family_text = family.to_string();
            expected
                .iter()
                .all(|expected| family_text.contains(expected))
        })
        .unwrap_or_else(|| {
            panic!("semantic mode should report one typed dynamic membership family: {semantic}")
        });
    let positive_text = positive_family.to_string();
    for expected in expected {
        assert!(
            positive_text.contains(expected),
            "semantic mode should include typed dynamic membership {expected}: {semantic}"
        );
    }
    for unexpected in [
        "wrong_element.ts",
        "wrong_element.swift",
        "wrong_receiver.swift",
        "custom_receiver.swift",
        "mutated_receiver.swift",
        "membership_alias_wrong_element.py",
        "membership_alias_wrong_receiver.py",
        "membership_alias_unresolved.py",
        "membership_alias_shadowed.py",
        "string_negative.java",
    ] {
        assert!(
            !positive_text.contains(unexpected),
            "semantic mode must preserve typed dynamic membership boundaries: {semantic}"
        );
    }

    let _ = fs::remove_dir_all(&dir);
}

// Broad fixture matrix for proven Set membership receiver contracts. The size is
// intentional until the fixture setup has a clearer table-builder abstraction.
#[allow(clippy::too_many_lines)]
#[test]
fn query_mode_semantic_proves_set_membership_when_receiver_is_proven() {
    let dir = std::env::temp_dir().join(format!("nose_set_membership_{}", std::process::id()));
    let _ = fs::remove_dir_all(&dir);
    fs::create_dir_all(&dir).unwrap();
    fs::write(
        dir.join("literal.py"),
        "def f(value, other):\n    return value in [\"red\", \"blue\"]\n",
    )
    .unwrap();
    fs::write(
        dir.join("set_inline.js"),
        "function f(value, other) {\n  return new Set([\"red\", \"blue\"]).has(value);\n}\n",
    )
    .unwrap();
    fs::write(
        dir.join("set_local.js"),
        "function f(value, other) {\n  const values = new Set([\"red\", \"blue\"]);\n  return values.has(value);\n}\n",
    )
    .unwrap();
    fs::write(
        dir.join("set_call.js"),
        "function f(value, other) {\n  return Set([\"red\", \"blue\"]).has(value);\n}\n",
    )
    .unwrap();
    fs::write(
        dir.join("shadowed_set_global.js"),
        "function Set(values) {\n  return { has: function() { return false; } };\n}\nfunction f(value, other) {\n  return new Set([\"red\", \"blue\"]).has(value);\n}\n",
    )
    .unwrap();
    fs::write(
        dir.join("typed_array.ts"),
        "function f(values: string[], value: string, other: string): boolean {\n  return values.includes(value);\n}\n",
    )
    .unwrap();
    fs::write(
        dir.join("typed_set.ts"),
        "function f(values: Set<string>, value: string, other: string): boolean {\n  return values.has(value);\n}\n",
    )
    .unwrap();
    fs::write(
        dir.join("java_list_of.java"),
        "import java.util.List;\n\nclass JavaListOf { static boolean f(String value, String other) { return List.of(\"red\", \"blue\").contains(value); } }\n",
    )
    .unwrap();
    fs::write(
        dir.join("java_set_of.java"),
        "import java.util.Set;\n\nclass JavaSetOf { static boolean f(String value, String other) { return Set.of(\"red\", \"blue\").contains(value); } }\n",
    )
    .unwrap();
    fs::write(
        dir.join("java_arrays_aslist.java"),
        "import java.util.Arrays;\n\nclass JavaArraysAsList { static boolean f(String value, String other) { return Arrays.asList(\"red\", \"blue\").contains(value); } }\n",
    )
    .unwrap();
    fs::write(
        dir.join("wrong_element.js"),
        "function f(value, other) {\n  return new Set([\"red\", \"blue\"]).has(other);\n}\n",
    )
    .unwrap();
    fs::write(
        dir.join("wrong_collection.js"),
        "function f(value, other) {\n  return new Set([\"green\", \"blue\"]).has(value);\n}\n",
    )
    .unwrap();
    fs::write(
        dir.join("untyped_receiver.ts"),
        "function f(values, value, other) {\n  return values.has(value);\n}\n",
    )
    .unwrap();
    fs::write(
        dir.join("shadowed_set.js"),
        "function f(Set, value, other) {\n  return new Set([\"red\", \"blue\"]).has(value);\n}\n",
    )
    .unwrap();
    fs::write(
        dir.join("java_wrong_element.java"),
        "import java.util.List;\n\nclass JavaWrongElement { static boolean f(String value, String other) { return List.of(\"red\", \"blue\").contains(other); } }\n",
    )
    .unwrap();
    fs::write(
        dir.join("java_shadowed_list.java"),
        "class JavaShadowedList { static boolean f(Object List, String value, String other) { return List.of(\"red\", \"blue\").contains(value); } }\n",
    )
    .unwrap();
    fs::write(
        dir.join("java_local_list.java"),
        "class JavaLocalList { static boolean f(String value, String other) { return List.of(\"red\", \"blue\").contains(value); } }\nclass List { static Box of(String a, String b) { return new Box(); } }\nclass Box { boolean contains(String value) { return false; } }\n",
    )
    .unwrap();

    let semantic = query_min_json(&dir, "semantic");
    let semantic_json = query_json(&semantic);
    let semantic_families = query_families(&semantic_json);
    let expected_positive = [
        "literal.py",
        "set_inline.js",
        "set_local.js",
        "java_list_of.java",
        "java_set_of.java",
        "java_arrays_aslist.java",
    ];
    let positive_family = semantic_families
        .iter()
        .find(|family| {
            let family_text = family.to_string();
            expected_positive
                .iter()
                .all(|expected| family_text.contains(expected))
        })
        .unwrap_or_else(|| {
            panic!("semantic mode should include proven Set membership family: {semantic}")
        });
    let positive_text = positive_family.to_string();
    let typed_family = semantic_families
        .iter()
        .find(|family| {
            let family_text = family.to_string();
            family_text.contains("typed_array.ts") && family_text.contains("typed_set.ts")
        })
        .unwrap_or_else(|| {
            panic!("semantic mode should include typed Set membership family: {semantic}")
        });
    let typed_text = typed_family.to_string();
    for unexpected in [
        "set_call.js",
        "shadowed_set_global.js",
        "wrong_element.js",
        "wrong_collection.js",
        "untyped_receiver.ts",
        "shadowed_set.js",
        "java_wrong_element.java",
        "java_shadowed_list.java",
        "java_local_list.java",
    ] {
        assert!(
            !positive_text.contains(unexpected),
            "semantic mode must preserve Set membership boundaries: {semantic}"
        );
        assert!(
            !typed_text.contains(unexpected),
            "semantic mode must preserve typed Set membership boundaries: {semantic}"
        );
    }

    let _ = fs::remove_dir_all(&dir);
}

#[test]
fn query_mode_semantic_keeps_aslist_single_unproven_receiver_distinct() {
    // `Arrays.asList(x).contains(value)` with a single argument is ambiguous: when `x`
    // is an array it is spread into the element list (membership in the elements), but
    // when `x` is a single object it is the sole element (`value.equals(x)`). Without an
    // array proof these two readings must not converge, otherwise an array-typed field
    // and a list-typed field of the same name would false-merge.
    let dir = std::env::temp_dir().join(format!("nose_aslist_unproven_{}", std::process::id()));
    let _ = fs::remove_dir_all(&dir);
    fs::create_dir_all(&dir).unwrap();
    fs::write(
        dir.join("array_field.java"),
        "import java.util.Arrays;\n\nclass ArrayField { String[] items; boolean f(String value) { return Arrays.asList(items).contains(value); } }\n",
    )
    .unwrap();
    fs::write(
        dir.join("list_field.java"),
        "import java.util.Arrays;\nimport java.util.List;\n\nclass ListField { List<String> items; boolean f(String value) { return Arrays.asList(items).contains(value); } }\n",
    )
    .unwrap();

    let semantic = query_min_json(&dir, "semantic");
    let semantic_json = query_json(&semantic);
    let semantic_families = query_families(&semantic_json);
    for family in semantic_families {
        let family_text = family.to_string();
        assert!(
            !(family_text.contains("array_field.java") && family_text.contains("list_field.java")),
            "single-argument asList over an unproven receiver must not merge array and list provenance: {semantic}"
        );
    }

    let _ = fs::remove_dir_all(&dir);
}

#[test]
fn query_mode_semantic_handles_shadowed_callback_collection_name() {
    let dir = std::env::temp_dir().join(format!(
        "nose_shadowed_callback_collection_{}",
        std::process::id()
    ));
    let _ = fs::remove_dir_all(&dir);
    fs::create_dir_all(&dir).unwrap();
    fs::write(
        dir.join("shadowed_callback.js"),
        r#"function clean(stdout, original) {
  const words = stdout
    ? stdout
        .split("\n")
        .filter((word, _, words) => {
          const lowerCased = word.toLowerCase();
          return lowerCased === word || !words.includes(lowerCased);
        })
        .sort((a, b) => a.toLowerCase().localeCompare(b.toLowerCase()))
    : [];
  const removed = original.filter((word) => !words.includes(word));
  return removed.length;
}
"#,
    )
    .unwrap();

    let semantic = run(&[
        "query",
        dir.to_str().unwrap(),
        "--mode",
        "semantic",
        "--format",
        "json",
        "top=0",
    ]);
    let _: serde_json::Value =
        serde_json::from_str(&semantic).expect("semantic query should emit JSON");

    let _ = fs::remove_dir_all(&dir);
}
