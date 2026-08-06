use crate::*;

#[test]
fn query_mode_semantic_proves_regex_literal_predicate_matches() {
    let dir = std::env::temp_dir().join(format!("nose_regex_semantic_{}", std::process::id()));
    let _ = fs::remove_dir_all(&dir);
    fs::create_dir_all(&dir).unwrap();
    fs::write(
        dir.join("dot-only.ts"),
        "export function isDotOnlyPathSegment(value: string) {\n    return /^\\.+$/u.test(value);\n}\n",
    )
    .unwrap();
    fs::write(
        dir.join("markdown-link.ts"),
        "export function isWorkspaceMarkdownLink(markdown: string) {\n    return /^\\[\\[[^\\]]+\\]\\]$/u.test(markdown);\n}\n",
    )
    .unwrap();
    fs::write(
        dir.join("dot-only-copy.ts"),
        "export function matchesDotOnly(segment: string) {\n    return /^\\.+$/u.test(segment);\n}\n",
    )
    .unwrap();
    fs::write(
        dir.join("string-test-a.ts"),
        "export function stringTestA(value: string) {\n    return \"^\\\\.+$\".test(value);\n}\n",
    )
    .unwrap();
    fs::write(
        dir.join("string-test-b.ts"),
        "export function stringTestB(value: string) {\n    return \"^\\\\.+$\".test(value);\n}\n",
    )
    .unwrap();

    let semantic = query_min_json(&dir, "semantic");
    let semantic_json = query_json(&semantic);
    let semantic_families = query_families(&semantic_json);
    assert_eq!(
        semantic_families.len(),
        1,
        "semantic mode should report only the matching regex-literal predicate: {semantic}"
    );
    let semantic_text = semantic_json.to_string();
    assert!(
        semantic_text.contains("dot-only.ts")
            && semantic_text.contains("dot-only-copy.ts")
            && !semantic_text.contains("markdown-link.ts")
            && !semantic_text.contains("string-test-a.ts")
            && !semantic_text.contains("string-test-b.ts"),
        "semantic mode must consume regex literal provenance without merging different patterns: {semantic}"
    );

    let near = query_min_json(&dir, "near:0.5");
    assert!(
        near.contains("dot-only.ts") && near.contains("markdown-link.ts"),
        "near mode may still surface the divergent-edit candidate: {near}"
    );

    let _ = fs::remove_dir_all(&dir);
}

#[test]
fn query_mode_semantic_allows_proved_js_static_builtins() {
    assert_single_semantic_family(
        "static_builtin",
        &[
            (
                "array_a.ts",
                "export function isList(value: unknown) {\n    return Array.isArray(value);\n}\n",
            ),
            (
                "array_b.ts",
                "export function acceptsArray(input: unknown) {\n    return Array.isArray(input);\n}\n",
            ),
            (
                "typeof_negative.ts",
                "export function acceptsObject(value: unknown) {\n    return typeof value === \"object\";\n}\n",
            ),
        ],
        &["array_a.ts", "array_b.ts"],
        &["typeof_negative.ts"],
    );
}

#[test]
fn query_mode_semantic_rejects_unproved_typeof_function_name() {
    let dir = std::env::temp_dir().join(format!(
        "nose_unproved_typeof_function_{}",
        std::process::id()
    ));
    let _ = fs::remove_dir_all(&dir);
    fs::create_dir_all(&dir).unwrap();
    fs::write(
        dir.join("provider_a.py"),
        "def typeof(value):\n    return 1\n",
    )
    .unwrap();
    fs::write(
        dir.join("provider_b.py"),
        "def typeof(value):\n    return 2\n",
    )
    .unwrap();
    fs::write(
        dir.join("raw_typeof_a.py"),
        "from provider_a import *\n\ndef classify(value):\n    return typeof(value)\n",
    )
    .unwrap();
    fs::write(
        dir.join("raw_typeof_b.py"),
        "from provider_b import *\n\ndef classify(value):\n    return typeof(value)\n",
    )
    .unwrap();

    let semantic = query_min_json(&dir, "semantic");
    let semantic_json = query_json(&semantic);
    assert!(
        !family_contains_all(
            &semantic_json,
            &["raw_typeof_a.py", "raw_typeof_b.py"]
        ),
        "semantic mode must not treat an arbitrary typeof function as the JS typeof operator: {semantic}"
    );

    let _ = fs::remove_dir_all(&dir);
}

#[test]
fn query_mode_semantic_rejects_unproved_array_isarray_name() {
    let dir = std::env::temp_dir().join(format!(
        "nose_unproved_array_isarray_{}",
        std::process::id()
    ));
    let _ = fs::remove_dir_all(&dir);
    fs::create_dir_all(&dir).unwrap();
    fs::write(
        dir.join("array_a.py"),
        "class Array:\n    @staticmethod\n    def isArray(value):\n        return True\n\ndef check(value):\n    return Array.isArray(value)\n",
    )
    .unwrap();
    fs::write(
        dir.join("array_b.py"),
        "class Array:\n    @staticmethod\n    def isArray(value):\n        return False\n\ndef check(value):\n    return Array.isArray(value)\n",
    )
    .unwrap();

    let semantic = query_min_json(&dir, "semantic");
    let semantic_json = query_json(&semantic);
    assert!(
        !family_contains_all(&semantic_json, &["array_a.py", "array_b.py"]),
        "semantic mode must not treat an arbitrary Array.isArray method as the JS static builtin: {semantic}"
    );

    let _ = fs::remove_dir_all(&dir);
}

#[test]
fn query_mode_semantic_rejects_unproved_literal_test_method_name() {
    let dir =
        std::env::temp_dir().join(format!("nose_unproved_literal_test_{}", std::process::id()));
    let _ = fs::remove_dir_all(&dir);
    fs::create_dir_all(&dir).unwrap();
    fs::write(
        dir.join("literal_test_a.rb"),
        "class String\n  def test(value)\n    true\n  end\nend\n\ndef accepts(value)\n  \"rule\".test(value)\nend\n",
    )
    .unwrap();
    fs::write(
        dir.join("literal_test_b.rb"),
        "class String\n  def test(value)\n    false\n  end\nend\n\ndef accepts(value)\n  \"rule\".test(value)\nend\n",
    )
    .unwrap();

    let semantic = query_min_json(&dir, "semantic");
    let semantic_json = query_json(&semantic);
    assert!(
        !family_contains_all(
            &semantic_json,
            &["literal_test_a.rb", "literal_test_b.rb"]
        ),
        "semantic mode must not treat an arbitrary literal .test method as JS regex semantics: {semantic}"
    );

    let _ = fs::remove_dir_all(&dir);
}
