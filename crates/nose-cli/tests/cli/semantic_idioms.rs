use super::*;

#[test]
fn scan_mode_semantic_proves_regex_literal_predicate_matches() {
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

    let semantic = run(&[
        "scan",
        dir.to_str().unwrap(),
        "--mode",
        "semantic",
        "--min-lines",
        "1",
        "--min-size",
        "1",
        "--format",
        "json",
        "--top",
        "0",
    ]);
    let semantic_json = scan_json(&semantic);
    let semantic_families = scan_families(&semantic_json);
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

    let near = run(&[
        "scan",
        dir.to_str().unwrap(),
        "--mode",
        "near:0.5",
        "--min-lines",
        "1",
        "--min-size",
        "1",
        "--format",
        "json",
        "--top",
        "0",
    ]);
    assert!(
        near.contains("dot-only.ts") && near.contains("markdown-link.ts"),
        "near mode may still surface the review candidate: {near}"
    );

    let _ = fs::remove_dir_all(&dir);
}

#[test]
fn scan_mode_semantic_allows_proved_js_static_builtins() {
    let dir = std::env::temp_dir().join(format!("nose_static_builtin_{}", std::process::id()));
    let _ = fs::remove_dir_all(&dir);
    fs::create_dir_all(&dir).unwrap();
    fs::write(
        dir.join("array_a.ts"),
        "export function isList(value: unknown) {\n    return Array.isArray(value);\n}\n",
    )
    .unwrap();
    fs::write(
        dir.join("array_b.ts"),
        "export function acceptsArray(input: unknown) {\n    return Array.isArray(input);\n}\n",
    )
    .unwrap();
    fs::write(
        dir.join("typeof_negative.ts"),
        "export function acceptsObject(value: unknown) {\n    return typeof value === \"object\";\n}\n",
    )
    .unwrap();

    let semantic = run(&[
        "scan",
        dir.to_str().unwrap(),
        "--mode",
        "semantic",
        "--min-lines",
        "1",
        "--min-size",
        "1",
        "--format",
        "json",
        "--top",
        "0",
    ]);
    let semantic_json = scan_json(&semantic);
    let semantic_families = scan_families(&semantic_json);
    assert_eq!(
        semantic_families.len(),
        1,
        "semantic mode should report only the identical Array.isArray guard: {semantic}"
    );
    let semantic_text = semantic_json.to_string();
    assert!(
        semantic_text.contains("array_a.ts")
            && semantic_text.contains("array_b.ts")
            && !semantic_text.contains("typeof_negative.ts"),
        "semantic mode must keep static builtin calls exact: {semantic}"
    );

    let _ = fs::remove_dir_all(&dir);
}

#[test]
fn scan_mode_semantic_rejects_unproved_typeof_function_name() {
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

    let semantic = run(&[
        "scan",
        dir.to_str().unwrap(),
        "--mode",
        "semantic",
        "--min-lines",
        "1",
        "--min-size",
        "1",
        "--format",
        "json",
        "--top",
        "0",
    ]);
    let semantic_json = scan_json(&semantic);
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
fn scan_mode_semantic_rejects_unproved_array_isarray_name() {
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

    let semantic = run(&[
        "scan",
        dir.to_str().unwrap(),
        "--mode",
        "semantic",
        "--min-lines",
        "1",
        "--min-size",
        "1",
        "--format",
        "json",
        "--top",
        "0",
    ]);
    let semantic_json = scan_json(&semantic);
    assert!(
        !family_contains_all(&semantic_json, &["array_a.py", "array_b.py"]),
        "semantic mode must not treat an arbitrary Array.isArray method as the JS static builtin: {semantic}"
    );

    let _ = fs::remove_dir_all(&dir);
}

#[test]
fn scan_mode_semantic_rejects_unproved_literal_test_method_name() {
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

    let semantic = run(&[
        "scan",
        dir.to_str().unwrap(),
        "--mode",
        "semantic",
        "--min-lines",
        "1",
        "--min-size",
        "1",
        "--format",
        "json",
        "--top",
        "0",
    ]);
    let semantic_json = scan_json(&semantic);
    assert!(
        !family_contains_all(
            &semantic_json,
            &["literal_test_a.rb", "literal_test_b.rb"]
        ),
        "semantic mode must not treat an arbitrary literal .test method as JS regex semantics: {semantic}"
    );

    let _ = fs::remove_dir_all(&dir);
}

#[test]
fn scan_mode_semantic_proves_extreme_type4_idioms() {
    let dir = std::env::temp_dir().join(format!("nose_extreme_type4_{}", std::process::id()));
    let _ = fs::remove_dir_all(&dir);
    fs::create_dir_all(&dir).unwrap();
    fs::write(
        dir.join("record.ts"),
        "export function recordA(value: unknown) { return value !== null && typeof value === 'object' && Array.isArray(value) === false; }\n\
         export function recordB(input: unknown) { return typeof input === 'object' && input !== null && !Array.isArray(input); }\n\
         export function recordMissingArray(value: unknown) { return typeof value === 'object' && value !== null; }\n",
    )
    .unwrap();
    fs::write(
        dir.join("early.ts"),
        "export function anyLoop(xs: number[]) { let found = false; for (const x of xs) { if (x > 0) { found = true; break; } } return found; }\n\
         export function anySome(xs: number[]) { return xs.some(x => x > 0); }\n\
         export function anyWrongPredicate(xs: number[]) { let found = false; for (const x of xs) { if (x < 0) { found = true; break; } } return found; }\n",
    )
    .unwrap();
    fs::write(
        dir.join("membership.ts"),
        "export function colorOr(value: string) { return value === 'red' || value === 'blue'; }\n\
         export function colorIncludes(value: string) { return ['blue', 'red'].includes(value); }\n\
         export function colorWrongLiteral(value: string) { return value === 'red' || value === 'green'; }\n",
    )
    .unwrap();
    fs::write(
        dir.join("builder.py"),
        concat!(
            "def concat_loop(xs):\n",
            "    out = \"\"\n",
            "    for x in xs:\n",
            "        out += x\n",
            "    return out\n\n",
            "def concat_join(xs):\n",
            "    return \"\".join(xs)\n\n",
            "def concat_prepend(xs):\n",
            "    out = \"\"\n",
            "    for x in xs:\n",
            "        out = x + out\n",
            "    return out\n",
        ),
    )
    .unwrap();

    let semantic = run(&[
        "scan",
        dir.to_str().unwrap(),
        "--mode",
        "semantic",
        "--min-lines",
        "1",
        "--min-size",
        "1",
        "--format",
        "json",
        "--top",
        "0",
    ]);
    let semantic_json = scan_json(&semantic);
    let semantic_families = scan_families(&semantic_json);
    let family = |positives: &[&str], negatives: &[&str]| {
        semantic_families
            .iter()
            .map(serde_json::Value::to_string)
            .find(|text| {
                positives.iter().all(|name| text.contains(name))
                    && negatives.iter().all(|name| !text.contains(name))
            })
            .unwrap_or_else(|| {
                panic!(
                    "semantic mode should report {positives:?} without {negatives:?}: {semantic}"
                )
            })
    };

    family(&["recordA", "recordB"], &["recordMissingArray"]);
    family(&["anyLoop", "anySome"], &["anyWrongPredicate"]);
    family(&["colorOr", "colorIncludes"], &["colorWrongLiteral"]);
    family(&["concat_loop", "concat_join"], &["concat_prepend"]);

    let _ = fs::remove_dir_all(&dir);
}

#[test]
fn scan_mode_semantic_proves_collection_empty_checks() {
    let dir = std::env::temp_dir().join(format!("nose_collection_empty_{}", std::process::id()));
    let _ = fs::remove_dir_all(&dir);
    fs::create_dir_all(&dir).unwrap();
    fs::write(
        dir.join("rust_len.rs"),
        "pub fn empty_len(items: &[i32], other: &[i32]) -> bool {\n    items.len() == 0\n}\n",
    )
    .unwrap();
    fs::write(
        dir.join("rust_named.rs"),
        "pub fn empty_named(values: &[i32], other: &[i32]) -> bool {\n    values.is_empty()\n}\n",
    )
    .unwrap();
    fs::write(
        dir.join("rust_threshold_negative.rs"),
        "pub fn one_item(items: &[i32], other: &[i32]) -> bool {\n    items.len() == 1\n}\n",
    )
    .unwrap();
    fs::write(
        dir.join("rust_receiver_negative.rs"),
        "pub fn other_empty(items: &[i32], other: &[i32]) -> bool {\n    other.is_empty()\n}\n",
    )
    .unwrap();
    fs::write(
        dir.join("java_size.java"),
        "class JavaSize { static boolean emptySize(java.util.List<Integer> items, java.util.List<Integer> other) { return items.size() == 0; } }\n",
    )
    .unwrap();
    fs::write(
        dir.join("java_named.java"),
        "class JavaNamed { static boolean emptyNamed(java.util.List<Integer> values, java.util.List<Integer> other) { return values.isEmpty(); } }\n",
    )
    .unwrap();
    fs::write(
        dir.join("ruby_length.rb"),
        "def empty_length(items, other)\n  items.length == 0\nend\n",
    )
    .unwrap();
    fs::write(
        dir.join("ruby_named.rb"),
        "def empty_named(values, other)\n  values.empty?\nend\n",
    )
    .unwrap();

    let semantic = run(&[
        "scan",
        dir.to_str().unwrap(),
        "--mode",
        "semantic",
        "--min-lines",
        "1",
        "--min-size",
        "1",
        "--format",
        "json",
        "--top",
        "0",
    ]);
    let semantic_json = scan_json(&semantic);
    let semantic_families = scan_families(&semantic_json);
    assert!(
        !semantic_families.is_empty(),
        "semantic mode should report collection emptiness families: {semantic}"
    );
    let semantic_text = semantic_json.to_string();
    assert!(
        semantic_text.contains("rust_len.rs")
            && semantic_text.contains("rust_named.rs")
            && semantic_text.contains("java_size.java")
            && semantic_text.contains("java_named.java")
            && !semantic_text.contains("ruby_length.rb")
            && !semantic_text.contains("ruby_named.rb")
            && !semantic_text.contains("rust_threshold_negative.rs")
            && !semantic_text.contains("rust_receiver_negative.rs"),
        "semantic mode must prove collection-empty checks without merging boundaries: {semantic}"
    );

    let _ = fs::remove_dir_all(&dir);
}

#[test]
fn scan_mode_semantic_proves_string_prefix_checks() {
    let dir = std::env::temp_dir().join(format!("nose_string_prefix_{}", std::process::id()));
    let _ = fs::remove_dir_all(&dir);
    fs::create_dir_all(&dir).unwrap();
    fs::write(
        dir.join("prefix.py"),
        "def prefix(value: str, other: str) -> bool:\n    return value.startswith(\"pre\")\n",
    )
    .unwrap();
    fs::write(
        dir.join("prefix.js"),
        "function prefix(value, other) {\n  return value.startsWith(\"pre\");\n}\n",
    )
    .unwrap();
    fs::write(
        dir.join("prefix.ts"),
        "function prefix(value: string, other: string): boolean {\n  return value.startsWith(\"pre\");\n}\n",
    )
    .unwrap();
    fs::write(
        dir.join("prefix.go"),
        "package p\n\nimport \"strings\"\n\nfunc Prefix(value string, other string) bool {\n    return strings.HasPrefix(value, \"pre\")\n}\n",
    )
    .unwrap();
    fs::write(
        dir.join("prefix.rs"),
        "pub fn prefix(value: &str, other: &str) -> bool {\n    value.starts_with(\"pre\")\n}\n",
    )
    .unwrap();
    fs::write(
        dir.join("prefix.java"),
        "class Prefix { static boolean prefix(String value, String other) { return value.startsWith(\"pre\"); } }\n",
    )
    .unwrap();
    fs::write(
        dir.join("prefix.rb"),
        "def prefix(value, other)\n  value.start_with?(\"pre\")\nend\n",
    )
    .unwrap();
    fs::write(
        dir.join("affix_negative.py"),
        "def prefix_alt(value, other):\n    return value.startswith(\"alt\")\n",
    )
    .unwrap();
    fs::write(
        dir.join("direction_negative.js"),
        "function suffix(value, other) {\n  return value.endsWith(\"pre\");\n}\n",
    )
    .unwrap();
    fs::write(
        dir.join("receiver_negative.rs"),
        "pub fn prefix_other(value: &str, other: &str) -> bool {\n    other.starts_with(\"pre\")\n}\n",
    )
    .unwrap();

    let semantic = run(&[
        "scan",
        dir.to_str().unwrap(),
        "--mode",
        "semantic",
        "--min-lines",
        "1",
        "--min-size",
        "1",
        "--format",
        "json",
        "--top",
        "0",
    ]);
    let semantic_json = scan_json(&semantic);
    let semantic_families = scan_families(&semantic_json);
    assert!(
        !semantic_families.is_empty(),
        "semantic mode should report string prefix families: {semantic}"
    );
    let semantic_text = semantic_json.to_string();
    for expected in [
        "prefix.py",
        "prefix.ts",
        "prefix.go",
        "prefix.rs",
        "prefix.java",
    ] {
        assert!(
            semantic_text.contains(expected),
            "semantic mode should include {expected}: {semantic}"
        );
    }
    for unexpected in [
        "prefix.js",
        "prefix.rb",
        "affix_negative.py",
        "direction_negative.js",
        "receiver_negative.rs",
    ] {
        assert!(
            !semantic_text.contains(unexpected),
            "semantic mode must preserve string prefix boundaries: {semantic}"
        );
    }

    let _ = fs::remove_dir_all(&dir);
}

#[test]
fn scan_mode_semantic_proves_rust_integer_methods() {
    let dir =
        std::env::temp_dir().join(format!("nose_rust_numeric_methods_{}", std::process::id()));
    let _ = fs::remove_dir_all(&dir);
    fs::create_dir_all(&dir).unwrap();
    fs::write(
        dir.join("abs_conditional.py"),
        "def magnitude(value, other):\n    magnitude = value if value >= 0 else -value\n    return magnitude + other\n",
    )
    .unwrap();
    fs::write(
        dir.join("abs_method.rs"),
        "pub fn magnitude(value: i64, other: i64) -> i64 {\n    let magnitude = value.abs();\n    magnitude + other\n}\n",
    )
    .unwrap();
    fs::write(
        dir.join("min_conditional.py"),
        "def select(left, right, other):\n    selected = left if left <= right else right\n    return selected + other\n",
    )
    .unwrap();
    fs::write(
        dir.join("min_method.rs"),
        "pub fn select(left: i64, right: i64, other: i64) -> i64 {\n    let selected = left.min(right);\n    selected + other\n}\n",
    )
    .unwrap();
    fs::write(
        dir.join("max_conditional.py"),
        "def select(left, right, other):\n    selected = left if left >= right else right\n    return selected + other\n",
    )
    .unwrap();
    fs::write(
        dir.join("max_method.rs"),
        "pub fn select(left: i64, right: i64, other: i64) -> i64 {\n    let selected = left.max(right);\n    selected + other\n}\n",
    )
    .unwrap();
    fs::write(
        dir.join("abs_custom_negative.rs"),
        "struct Wrap(i64);\nimpl Wrap { fn abs(&self) -> i64 { 0 } }\npub fn magnitude(value: Wrap) -> i64 {\n    let magnitude = value.abs();\n    magnitude + 1\n}\n",
    )
    .unwrap();
    fs::write(
        dir.join("min_custom_negative.rs"),
        "struct Wrap(i64);\nimpl Wrap { fn min(&self, _right: i64) -> i64 { 0 } }\npub fn select(left: Wrap, right: i64, other: i64) -> i64 {\n    let selected = left.min(right);\n    selected + other\n}\n",
    )
    .unwrap();
    fs::write(
        dir.join("max_custom_negative.rs"),
        "struct Wrap(i64);\nimpl Wrap { fn max(&self, _right: i64) -> i64 { 0 } }\npub fn select(left: Wrap, right: i64, other: i64) -> i64 {\n    let selected = left.max(right);\n    selected + other\n}\n",
    )
    .unwrap();
    fs::write(
        dir.join("min_wrong_value.rs"),
        "pub fn select(left: i64, right: i64, other: i64) -> i64 {\n    let selected = left.min(other);\n    selected + other\n}\n",
    )
    .unwrap();

    let semantic = run(&[
        "scan",
        dir.to_str().unwrap(),
        "--mode",
        "semantic",
        "--min-lines",
        "1",
        "--min-size",
        "1",
        "--format",
        "json",
        "--top",
        "0",
    ]);
    let semantic_json = scan_json(&semantic);
    let semantic_families = scan_families(&semantic_json);
    for expected_pair in [
        ["abs_conditional.py", "abs_method.rs"],
        ["min_conditional.py", "min_method.rs"],
        ["max_conditional.py", "max_method.rs"],
    ] {
        let family = semantic_families
            .iter()
            .find(|family| {
                let text = family.to_string();
                expected_pair.iter().all(|expected| text.contains(expected))
            })
            .unwrap_or_else(|| {
                panic!("semantic mode should report integer method family: {semantic}")
            });
        let text = family.to_string();
        for unexpected in [
            "abs_custom_negative.rs",
            "min_custom_negative.rs",
            "max_custom_negative.rs",
            "min_wrong_value.rs",
        ] {
            assert!(
                !text.contains(unexpected),
                "semantic mode must preserve Rust integer method boundaries: {semantic}"
            );
        }
    }

    let _ = fs::remove_dir_all(&dir);
}

// Broad fixture matrix for literal collection membership contracts. The size is
// intentional until the fixture setup has a clearer table-builder abstraction.
#[allow(clippy::too_many_lines)]
#[test]
fn scan_mode_semantic_proves_literal_collection_membership() {
    let dir = std::env::temp_dir().join(format!("nose_literal_membership_{}", std::process::id()));
    let _ = fs::remove_dir_all(&dir);
    fs::create_dir_all(&dir).unwrap();
    fs::write(
        dir.join("membership.py"),
        "def membership(value, other):\n    return value in [\"red\", \"blue\"]\n",
    )
    .unwrap();
    fs::write(
        dir.join("python_set_factory.py"),
        "def python_set_factory(value, other):\n    return set([\"red\", \"blue\"]).__contains__(value)\n",
    )
    .unwrap();
    fs::write(
        dir.join("python_tuple_factory.py"),
        "def python_tuple_factory(value, other):\n    return tuple([\"red\", \"blue\"]).__contains__(value)\n",
    )
    .unwrap();
    fs::write(
        dir.join("python_frozenset_factory.py"),
        "def python_frozenset_factory(value, other):\n    return frozenset([\"red\", \"blue\"]).__contains__(value)\n",
    )
    .unwrap();
    fs::write(
        dir.join("python_deque_import.py"),
        "from collections import deque\n\n\ndef python_deque_import(value, other):\n    return deque([\"red\", \"blue\"]).__contains__(value)\n",
    )
    .unwrap();
    fs::write(
        dir.join("python_deque_alias.py"),
        "from collections import deque as Values\n\n\ndef python_deque_alias(value, other):\n    return Values([\"red\", \"blue\"]).__contains__(value)\n",
    )
    .unwrap();
    fs::write(
        dir.join("python_deque_namespace.py"),
        "import collections\n\n\ndef python_deque_namespace(value, other):\n    return collections.deque([\"red\", \"blue\"]).__contains__(value)\n",
    )
    .unwrap();
    fs::write(
        dir.join("python_module_tuple.py"),
        "VALUES = (\"red\", \"blue\")\n\n\ndef python_module_tuple(value, other):\n    return value in VALUES\n",
    )
    .unwrap();
    fs::write(
        dir.join("python_module_set.py"),
        "VALUES = {\"red\", \"blue\"}\n\n\ndef python_module_set(value, other):\n    return value in VALUES\n",
    )
    .unwrap();
    fs::write(
        dir.join("membership.js"),
        "function membership(value, other) {\n  return [\"red\", \"blue\"].includes(value);\n}\n",
    )
    .unwrap();
    fs::write(
        dir.join("membership.ts"),
        "function membership(value: string, other: string): boolean {\n  return [\"red\", \"blue\"].includes(value);\n}\n",
    )
    .unwrap();
    fs::write(
        dir.join("js_in_array_a.js"),
        "function jsInArrayA(value, other) {\n  return value in [\"red\", \"blue\"];\n}\n",
    )
    .unwrap();
    fs::write(
        dir.join("js_in_array_b.js"),
        "function jsInArrayB(value, other) {\n  return value in [\"red\", \"blue\"];\n}\n",
    )
    .unwrap();
    fs::write(
        dir.join("membership.go"),
        "package p\n\nimport \"slices\"\n\nfunc Membership(value string, other string) bool {\n    return slices.Contains([]string{\"red\", \"blue\"}, value)\n}\n",
    )
    .unwrap();
    fs::write(
        dir.join("membership.rs"),
        "pub fn membership(value: &str, other: &str) -> bool {\n    [\"red\", \"blue\"].contains(value)\n}\n",
    )
    .unwrap();
    fs::write(
        dir.join("membership.rb"),
        "def membership(value, other)\n  [\"red\", \"blue\"].include?(value)\nend\n",
    )
    .unwrap();
    fs::write(
        dir.join("ruby_member.rb"),
        "def ruby_member(value, other)\n  [\"red\", \"blue\"].member?(value)\nend\n",
    )
    .unwrap();
    fs::write(
        dir.join("ruby_set_new_include.rb"),
        "require \"set\"\n\ndef ruby_set_new_include(value, other)\n  Set.new([\"red\", \"blue\"]).include?(value)\nend\n",
    )
    .unwrap();
    fs::write(
        dir.join("ruby_set_new_member.rb"),
        "require \"set\"\n\ndef ruby_set_new_member(value, other)\n  Set.new([\"red\", \"blue\"]).member?(value)\nend\n",
    )
    .unwrap();
    fs::write(
        dir.join("ruby_set_local.rb"),
        "require \"set\"\n\ndef ruby_set_local(value, other)\n  values = Set.new([\"red\", \"blue\"])\n  values.include?(value)\nend\n",
    )
    .unwrap();
    fs::write(
        dir.join("module_set.js"),
        "const VALUES = new Set([\"red\", \"blue\"]);\n\nfunction moduleSet(value, other) {\n  return VALUES.has(value);\n}\n",
    )
    .unwrap();
    fs::write(
        dir.join("module_set.ts"),
        "const VALUES = new Set<string>([\"red\", \"blue\"]);\n\nfunction moduleSet(value: string, other: string): boolean {\n  return VALUES.has(value);\n}\n",
    )
    .unwrap();
    fs::write(
        dir.join("array_some.js"),
        "function arraySome(value, other) {\n  return [\"red\", \"blue\"].some((item) => item === value);\n}\n",
    )
    .unwrap();
    fs::write(
        dir.join("array_some.ts"),
        "function arraySome(value: string, other: string): boolean {\n  return [\"red\", \"blue\"].some((item: string) => item === value);\n}\n",
    )
    .unwrap();
    fs::write(
        dir.join("array_indexof.js"),
        "function arrayIndexOf(value, other) {\n  return [\"red\", \"blue\"].indexOf(value) !== -1;\n}\n",
    )
    .unwrap();
    fs::write(
        dir.join("array_indexof.ts"),
        "function arrayIndexOf(value: string, other: string): boolean {\n  return [\"red\", \"blue\"].indexOf(value) >= 0;\n}\n",
    )
    .unwrap();
    fs::write(
        dir.join("array_findindex.js"),
        "function arrayFindIndex(value, other) {\n  return [\"red\", \"blue\"].findIndex((item) => item === value) !== -1;\n}\n",
    )
    .unwrap();
    fs::write(
        dir.join("array_findindex.ts"),
        "function arrayFindIndex(value: string, other: string): boolean {\n  return [\"red\", \"blue\"].findIndex((item: string) => item === value) >= 0;\n}\n",
    )
    .unwrap();
    fs::write(
        dir.join("array_some_loose.js"),
        "function arraySomeLoose(value, other) {\n  return [\"red\", \"blue\"].some((item) => item == value);\n}\n",
    )
    .unwrap();
    fs::write(
        dir.join("array_findindex_loose.js"),
        "function arrayFindIndexLoose(value, other) {\n  return [\"red\", \"blue\"].findIndex((item) => item == value) !== -1;\n}\n",
    )
    .unwrap();
    fs::write(
        dir.join("array_filter_length.js"),
        "function arrayFilterLength(value, other) {\n  return [\"red\", \"blue\"].filter((item) => item === value).length !== 0;\n}\n",
    )
    .unwrap();
    fs::write(
        dir.join("array_filter_length.ts"),
        "function arrayFilterLength(value: string, other: string): boolean {\n  return [\"red\", \"blue\"].filter((item: string) => item === value).length >= 1;\n}\n",
    )
    .unwrap();
    fs::write(
        dir.join("not_membership.py"),
        "def not_membership(value, other):\n    return value not in [\"red\", \"blue\"]\n",
    )
    .unwrap();
    fs::write(
        dir.join("not_includes.js"),
        "function notIncludes(value, other) {\n  return ![\"red\", \"blue\"].includes(value);\n}\n",
    )
    .unwrap();
    fs::write(
        dir.join("array_every.js"),
        "function arrayEvery(value, other) {\n  return [\"red\", \"blue\"].every((item) => item !== value);\n}\n",
    )
    .unwrap();
    fs::write(
        dir.join("array_every.ts"),
        "function arrayEvery(value: string, other: string): boolean {\n  return [\"red\", \"blue\"].every((item: string) => item !== value);\n}\n",
    )
    .unwrap();
    fs::write(
        dir.join("array_every_loose.js"),
        "function arrayEveryLoose(value, other) {\n  return [\"red\", \"blue\"].every((item) => item != value);\n}\n",
    )
    .unwrap();
    fs::write(
        dir.join("array_filter_length_absence.js"),
        "function arrayFilterLengthAbsence(value, other) {\n  return [\"red\", \"blue\"].filter((item) => item === value).length === 0;\n}\n",
    )
    .unwrap();
    fs::write(
        dir.join("array_filter_length_loose.js"),
        "function arrayFilterLengthLoose(value, other) {\n  return [\"red\", \"blue\"].filter((item) => item == value).length !== 0;\n}\n",
    )
    .unwrap();
    fs::write(
        dir.join("array_filter_length_absence.ts"),
        "function arrayFilterLengthAbsence(value: string, other: string): boolean {\n  return [\"red\", \"blue\"].filter((item: string) => item === value).length <= 0;\n}\n",
    )
    .unwrap();
    fs::write(
        dir.join("module_list.java"),
        "import java.util.List;\n\nclass ModuleList {\n    static final List<String> VALUES = List.of(\"red\", \"blue\");\n\n    static boolean moduleList(String value, String other) {\n        return VALUES.contains(value);\n    }\n}\n",
    )
    .unwrap();
    fs::write(
        dir.join("go_slices_package.go"),
        "package p\n\nimport \"slices\"\n\nvar values = []string{\"red\", \"blue\"}\n\nfunc SlicesPackage(value string, other string) bool {\n    return slices.Contains(values, value)\n}\n",
    )
    .unwrap();
    fs::write(
        dir.join("go_slices_alias.go"),
        "package p\n\nimport sl \"slices\"\n\nvar values = []string{\"red\", \"blue\"}\n\nfunc SlicesAlias(value string, other string) bool {\n    return sl.Contains(values, value)\n}\n",
    )
    .unwrap();
    fs::write(
        dir.join("go_slices_const.go"),
        "package p\n\nimport \"slices\"\n\nconst first = \"red\"\nvar values = []string{first, \"blue\"}\n\nfunc SlicesConst(value string, other string) bool {\n    return slices.Contains(values, value)\n}\n",
    )
    .unwrap();
    fs::write(
        dir.join("go_slices_local.go"),
        "package p\n\nimport \"slices\"\n\nfunc SlicesLocal(value string, other string) bool {\n    values := []string{\"red\", \"blue\"}\n    return slices.Contains(values, value)\n}\n",
    )
    .unwrap();
    fs::write(
        dir.join("java_local_list.java"),
        "import java.util.List;\n\nclass JavaLocalList {\n    static boolean javaLocalList(String value, String other) {\n        var values = List.of(\"red\", \"blue\");\n        return values.contains(value);\n    }\n}\n",
    )
    .unwrap();
    fs::write(
        dir.join("rust_local_array.rs"),
        "pub fn rust_local_array(value: &str, other: &str) -> bool {\n    let values = [\"red\", \"blue\"];\n    values.contains(&value)\n}\n",
    )
    .unwrap();
    fs::write(
        dir.join("rust_local_typed_array.rs"),
        "pub fn rust_local_typed_array(value: &str, other: &str) -> bool {\n    let values: [&str; 2] = [\"red\", \"blue\"];\n    values.contains(&value)\n}\n",
    )
    .unwrap();
    fs::write(
        dir.join("rust_local_slice_ref.rs"),
        "pub fn rust_local_slice_ref(value: &str, other: &str) -> bool {\n    let values: &[&str] = &[\"red\", \"blue\"];\n    values.contains(&value)\n}\n",
    )
    .unwrap();
    fs::write(
        dir.join("rust_local_vec.rs"),
        "pub fn rust_local_vec(value: &str, other: &str) -> bool {\n    let values = vec![\"red\", \"blue\"];\n    values.contains(&value)\n}\n",
    )
    .unwrap();
    fs::write(
        dir.join("rust_std_hashset.rs"),
        "pub fn rust_std_hashset(value: &str, other: &str) -> bool {\n    let values = std::collections::HashSet::from([\"red\", \"blue\"]);\n    values.contains(&value)\n}\n",
    )
    .unwrap();
    fs::write(
        dir.join("rust_std_btreeset.rs"),
        "pub fn rust_std_btreeset(value: &str, other: &str) -> bool {\n    let values = std::collections::BTreeSet::from([\"red\", \"blue\"]);\n    values.contains(&value)\n}\n",
    )
    .unwrap();
    fs::write(
        dir.join("rust_std_vecdeque.rs"),
        "pub fn rust_std_vecdeque(value: &str, other: &str) -> bool {\n    let values = std::collections::VecDeque::from([\"red\", \"blue\"]);\n    values.contains(&value)\n}\n",
    )
    .unwrap();
    fs::write(
        dir.join("wrong_element.py"),
        "def wrong_element(value, other):\n    return other in [\"red\", \"blue\"]\n",
    )
    .unwrap();
    fs::write(
        dir.join("wrong_collection.js"),
        "function wrongCollection(value, other) {\n  return [\"green\", \"blue\"].includes(value);\n}\n",
    )
    .unwrap();
    fs::write(
        dir.join("array_some_wrong_element.js"),
        "function arraySomeWrongElement(value, other, third) {\n  return [\"red\", \"blue\"].some((item) => item === third);\n}\n",
    )
    .unwrap();
    fs::write(
        dir.join("array_some_wrong_collection.ts"),
        "function arraySomeWrongCollection(value: string, other: string): boolean {\n  return [\"purple\", \"orange\"].some((item: string) => item === value);\n}\n",
    )
    .unwrap();
    fs::write(
        dir.join("array_indexof_wrong_element.js"),
        "function arrayIndexOfWrongElement(value, other, third) {\n  return [\"red\", \"blue\"].indexOf(value + third) !== -1;\n}\n",
    )
    .unwrap();
    fs::write(
        dir.join("array_indexof_wrong_collection.ts"),
        "function arrayIndexOfWrongCollection(value: string, other: string): boolean {\n  return [\"yellow\", \"orange\"].indexOf(value) >= 0;\n}\n",
    )
    .unwrap();
    fs::write(
        dir.join("array_indexof_value.js"),
        "function arrayIndexOfValue(value, other) {\n  return [\"red\", \"blue\"].indexOf(value);\n}\n",
    )
    .unwrap();
    fs::write(
        dir.join("array_indexof_ne_zero.js"),
        "function arrayIndexOfNeZero(value, other) {\n  return [\"red\", \"blue\"].indexOf(value) !== 0;\n}\n",
    )
    .unwrap();
    fs::write(
        dir.join("array_indexof_reversed_gt_zero.js"),
        "function arrayIndexOfReversedGtZero(value, other) {\n  return 0 < [\"red\", \"blue\"].indexOf(value);\n}\n",
    )
    .unwrap();
    fs::write(
        dir.join("array_findindex_wrong_element.js"),
        "function arrayFindIndexWrongElement(value, other, third) {\n  return [\"red\", \"blue\"].findIndex((item) => item === value + third + other) !== -1;\n}\n",
    )
    .unwrap();
    fs::write(
        dir.join("array_findindex_wrong_collection.ts"),
        "function arrayFindIndexWrongCollection(value: string, other: string): boolean {\n  return [\"cyan\", \"magenta\"].findIndex((item: string) => item === value) >= 0;\n}\n",
    )
    .unwrap();
    fs::write(
        dir.join("array_findindex_value.js"),
        "function arrayFindIndexValue(value, other) {\n  return [\"red\", \"blue\"].findIndex((item) => item === value);\n}\n",
    )
    .unwrap();
    fs::write(
        dir.join("array_findindex_ne_zero.js"),
        "function arrayFindIndexNeZero(value, other) {\n  return [\"red\", \"blue\"].findIndex((item) => item === value) !== 0;\n}\n",
    )
    .unwrap();
    fs::write(
        dir.join("array_filter_length_wrong_element.js"),
        "function arrayFilterLengthWrongElement(value, other, third) {\n  return [\"red\", \"blue\"].filter((item) => item === other + third).length !== 0;\n}\n",
    )
    .unwrap();
    fs::write(
        dir.join("array_filter_length_wrong_collection.ts"),
        "function arrayFilterLengthWrongCollection(value: string, other: string): boolean {\n  return [\"black\", \"white\"].filter((item: string) => item === value).length >= 1;\n}\n",
    )
    .unwrap();
    fs::write(
        dir.join("array_filter_length_value.js"),
        "function arrayFilterLengthValue(value, other) {\n  return [\"red\", \"blue\"].filter((item) => item === value).length;\n}\n",
    )
    .unwrap();
    fs::write(
        dir.join("array_filter_length_absence_wrong_element.js"),
        "function arrayFilterLengthAbsenceWrongElement(value, other, third) {\n  return [\"red\", \"blue\"].filter((item) => item === other + third).length === 0;\n}\n",
    )
    .unwrap();
    fs::write(
        dir.join("array_filter_length_absence_wrong_collection.ts"),
        "function arrayFilterLengthAbsenceWrongCollection(value: string, other: string): boolean {\n  return [\"black\", \"white\"].filter((item: string) => item === value).length <= 0;\n}\n",
    )
    .unwrap();
    fs::write(
        dir.join("array_every_wrong_element.js"),
        "function arrayEveryWrongElement(value, other, third) {\n  return [\"red\", \"blue\"].every((item) => item !== third);\n}\n",
    )
    .unwrap();
    fs::write(
        dir.join("array_every_wrong_collection.ts"),
        "function arrayEveryWrongCollection(value: string, other: string): boolean {\n  return [\"purple\", \"orange\"].every((item: string) => item !== value);\n}\n",
    )
    .unwrap();
    fs::write(
        dir.join("substring.rs"),
        "pub fn substring(value: &str, other: &str) -> bool {\n    value.contains(\"red\")\n}\n",
    )
    .unwrap();
    fs::write(
        dir.join("module_set_mutated.js"),
        "const VALUES = new Set([\"red\", \"blue\"]);\nVALUES.add(\"green\");\n\nfunction moduleSetMutated(value, other) {\n  return VALUES.has(value);\n}\n",
    )
    .unwrap();
    fs::write(
        dir.join("python_module_mutated.py"),
        "VALUES = [\"red\", \"blue\"]\nVALUES.append(\"green\")\n\n\ndef python_module_mutated(value, other):\n    return value in VALUES\n",
    )
    .unwrap();
    fs::write(
        dir.join("module_set_shadowed.ts"),
        "const Set: any = function(_values: any) {\n  return { has: function() { return false; } };\n};\nconst VALUES = new Set([\"red\", \"blue\"]);\n\nfunction moduleSetShadowed(value: string, other: string): boolean {\n  return VALUES.has(value);\n}\n",
    )
    .unwrap();
    fs::write(
        dir.join("module_list_shadowed.java"),
        "class ModuleListShadowed {\n    static final List<String> VALUES = List.of(\"red\", \"blue\");\n\n    static boolean moduleListShadowed(String value, String other) {\n        return VALUES.contains(value);\n    }\n}\n\nclass List<T> {\n    static java.util.List<String> of(String left, String right) {\n        return java.util.List.of(\"green\", right);\n    }\n}\n",
    )
    .unwrap();
    fs::write(
        dir.join("python_factory_shadowed.py"),
        "def python_factory_shadowed(value, other):\n    def set(_values):\n        class Box:\n            def __contains__(self, _value):\n                return False\n        return Box()\n    return set([\"red\", \"blue\"]).__contains__(value)\n",
    )
    .unwrap();
    fs::write(
        dir.join("python_deque_wrong_element.py"),
        "from collections import deque\n\n\ndef python_deque_wrong_element(value, other):\n    return deque([\"red\", \"blue\"]).__contains__(other)\n",
    )
    .unwrap();
    fs::write(
        dir.join("python_deque_wrong_collection.py"),
        "from collections import deque\n\n\ndef python_deque_wrong_collection(value, other):\n    return deque([\"green\", \"blue\"]).__contains__(value)\n",
    )
    .unwrap();
    fs::write(
        dir.join("python_deque_missing_import.py"),
        "def python_deque_missing_import(value, other):\n    return deque([\"red\", \"blue\"]).__contains__(value)\n",
    )
    .unwrap();
    fs::write(
        dir.join("python_deque_shadowed.py"),
        "from collections import deque\n\n\ndef deque(_values):\n    class Box:\n        def __contains__(self, _value):\n            return False\n    return Box()\n\n\ndef python_deque_shadowed(value, other):\n    return deque([\"red\", \"blue\"]).__contains__(value)\n",
    )
    .unwrap();
    fs::write(
        dir.join("python_deque_mutated.py"),
        "from collections import deque\n\n\ndef python_deque_mutated(value, other):\n    values = deque([\"red\", \"blue\"])\n    values.append(\"green\")\n    return values.__contains__(value)\n",
    )
    .unwrap();
    fs::write(
        dir.join("go_slices_mutated.go"),
        "package p\n\nimport \"slices\"\n\nvar values = append([]string{\"red\", \"blue\"}, \"green\")\n\nfunc SlicesMutated(value string, other string) bool {\n    return slices.Contains(values, value)\n}\n",
    )
    .unwrap();
    fs::write(
        dir.join("go_slices_local_mutated.go"),
        "package p\n\nimport \"slices\"\n\nfunc SlicesLocalMutated(value string, other string) bool {\n    values := []string{\"red\", \"blue\"}\n    values = append(values, \"green\")\n    return slices.Contains(values, value)\n}\n",
    )
    .unwrap();
    fs::write(
        dir.join("go_slices_unimported.go"),
        "package p\n\ntype fakeSlices struct{}\n\nfunc (fakeSlices) Contains(values []string, value string) bool {\n    return false\n}\n\nvar slices fakeSlices\nvar values = []string{\"red\", \"blue\"}\n\nfunc SlicesUnimported(value string, other string) bool {\n    return slices.Contains(values, value)\n}\n",
    )
    .unwrap();
    fs::write(
        dir.join("java_local_list_mutated.java"),
        "import java.util.ArrayList;\nimport java.util.List;\n\nclass JavaLocalListMutated {\n    static boolean javaLocalListMutated(String value, String other) {\n        var values = new ArrayList<String>(List.of(\"red\", \"blue\"));\n        values.add(\"green\");\n        return values.contains(value);\n    }\n}\n",
    )
    .unwrap();
    fs::write(
        dir.join("rust_local_mutated.rs"),
        "pub fn rust_local_mutated(value: &str, other: &str) -> bool {\n    let mut values = vec![\"red\", \"blue\"];\n    values.push(\"green\");\n    values.contains(&value)\n}\n",
    )
    .unwrap();
    fs::write(
        dir.join("rust_local_custom_receiver.rs"),
        "struct Values;\n\nimpl Values {\n    fn contains(&self, _value: &&str) -> bool {\n        false\n    }\n}\n\npub fn rust_local_custom_receiver(value: &str, other: &str) -> bool {\n    let values = Values;\n    values.contains(&value)\n}\n",
    )
    .unwrap();
    fs::write(
        dir.join("rust_std_wrong_element.rs"),
        "pub fn rust_std_wrong_element(value: &str, other: &str) -> bool {\n    let values = std::collections::HashSet::from([\"red\", \"blue\"]);\n    values.contains(&(value.to_owned() + other))\n}\n",
    )
    .unwrap();
    fs::write(
        dir.join("rust_std_wrong_collection.rs"),
        "pub fn rust_std_wrong_collection(value: &str, other: &str) -> bool {\n    let values = std::collections::BTreeSet::from([\"silver\", \"gold\"]);\n    values.contains(&value)\n}\n",
    )
    .unwrap();
    fs::write(
        dir.join("rust_std_mutated.rs"),
        "pub fn rust_std_mutated(value: &str, other: &str) -> bool {\n    let mut values = std::collections::HashSet::from([\"red\", \"blue\"]);\n    values.insert(\"green\");\n    values.contains(&value)\n}\n",
    )
    .unwrap();
    fs::write(
        dir.join("ruby_set_wrong_element.rb"),
        "require \"set\"\n\ndef ruby_set_wrong_element(value, other)\n  Set.new([\"red\", \"blue\"]).include?(other)\nend\n",
    )
    .unwrap();
    fs::write(
        dir.join("ruby_set_wrong_collection.rb"),
        "require \"set\"\n\ndef ruby_set_wrong_collection(value, other)\n  Set.new([\"green\", \"blue\"]).include?(value)\nend\n",
    )
    .unwrap();
    fs::write(
        dir.join("ruby_set_missing_require.rb"),
        "def ruby_set_missing_require(value, other)\n  Set.new([\"red\", \"blue\"]).include?(value)\nend\n",
    )
    .unwrap();
    fs::write(
        dir.join("ruby_set_shadowed.rb"),
        "require \"set\"\n\nclass Set\n  def self.new(_values)\n    Box.new\n  end\nend\n\nclass Box\n  def include?(_value)\n    false\n  end\nend\n\ndef ruby_set_shadowed(value, other)\n  Set.new([\"red\", \"blue\"]).include?(value)\nend\n",
    )
    .unwrap();
    fs::write(
        dir.join("ruby_set_mutated.rb"),
        "require \"set\"\n\ndef ruby_set_mutated(value, other)\n  values = Set.new([\"red\", \"blue\"])\n  values.add(\"green\")\n  values.include?(value)\nend\n",
    )
    .unwrap();

    let semantic = run(&[
        "scan",
        dir.to_str().unwrap(),
        "--mode",
        "semantic",
        "--min-lines",
        "1",
        "--min-size",
        "1",
        "--format",
        "json",
        "--top",
        "0",
    ]);
    let semantic_json = scan_json(&semantic);
    let semantic_families = scan_families(&semantic_json);
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

#[test]
fn scan_mode_semantic_keeps_unproven_contains_calls_distinct() {
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

    let semantic = run(&[
        "scan",
        dir.to_str().unwrap(),
        "--mode",
        "semantic",
        "--min-lines",
        "1",
        "--min-size",
        "1",
        "--format",
        "json",
        "--top",
        "0",
    ]);
    let semantic_json: serde_json::Value =
        serde_json::from_str(&semantic).expect("semantic scan should emit JSON");
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
fn scan_mode_semantic_proves_typed_dynamic_collection_membership() {
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

    let semantic = run(&[
        "scan",
        dir.to_str().unwrap(),
        "--mode",
        "semantic",
        "--min-lines",
        "1",
        "--min-size",
        "1",
        "--format",
        "json",
        "--top",
        "0",
    ]);
    let semantic_json = scan_json(&semantic);
    let semantic_families = scan_families(&semantic_json);
    let expected = [
        "membership.py",
        "membership.ts",
        "membership.go",
        "membership.rs",
        "membership.java",
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
fn scan_mode_semantic_proves_set_membership_when_receiver_is_proven() {
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

    let semantic = run(&[
        "scan",
        dir.to_str().unwrap(),
        "--mode",
        "semantic",
        "--min-lines",
        "1",
        "--min-size",
        "1",
        "--format",
        "json",
        "--top",
        "0",
    ]);
    let semantic_json = scan_json(&semantic);
    let semantic_families = scan_families(&semantic_json);
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
fn scan_mode_semantic_keeps_aslist_single_unproven_receiver_distinct() {
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

    let semantic = run(&[
        "scan",
        dir.to_str().unwrap(),
        "--mode",
        "semantic",
        "--min-lines",
        "1",
        "--min-size",
        "1",
        "--format",
        "json",
        "--top",
        "0",
    ]);
    let semantic_json = scan_json(&semantic);
    let semantic_families = scan_families(&semantic_json);
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
fn scan_mode_semantic_handles_shadowed_callback_collection_name() {
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
        "scan",
        dir.to_str().unwrap(),
        "--mode",
        "semantic",
        "--format",
        "json",
        "--top",
        "0",
    ]);
    let _: serde_json::Value =
        serde_json::from_str(&semantic).expect("semantic scan should emit JSON");

    let _ = fs::remove_dir_all(&dir);
}

// Broad fixture matrix for typed TypeScript map-key membership contracts. The size is
// intentional until the fixture setup has a clearer table-builder abstraction.
#[allow(clippy::too_many_lines)]
#[test]
fn scan_mode_semantic_proves_typed_typescript_map_key_membership() {
    let dir = std::env::temp_dir().join(format!("nose_typed_ts_map_key_{}", std::process::id()));
    let _ = fs::remove_dir_all(&dir);
    fs::create_dir_all(&dir).unwrap();
    fs::write(
        dir.join("map_key.py"),
        "def f(lookup: dict[str, str], other_lookup: dict[str, str], key: str, other: str) -> bool:\n    return key in lookup\n",
    )
    .unwrap();
    fs::write(
        dir.join("map_key.java"),
        "import java.util.Map;\n\nclass C { static boolean f(Map<String, String> lookup, Map<String, String> other_lookup, String key, String other) { return lookup.containsKey(key); } }\n",
    )
    .unwrap();
    fs::write(
        dir.join("map_key.ts"),
        "function f(lookup: Map<string, string>, other_lookup: Map<string, string>, key: string, other: string): boolean {\n  return lookup.has(key);\n}\n",
    )
    .unwrap();
    fs::write(
        dir.join("map_key_python_keys_in.py"),
        "def f(lookup: dict[str, str], other_lookup: dict[str, str], key: str, other: str) -> bool:\n    return key in lookup.keys()\n",
    )
    .unwrap();
    fs::write(
        dir.join("map_key_python_keys_contains.py"),
        "def f(lookup: dict[str, str], other_lookup: dict[str, str], key: str, other: str) -> bool:\n    return lookup.keys().__contains__(key)\n",
    )
    .unwrap();
    fs::write(
        dir.join("map_key_ts_array_from_keys.ts"),
        "function f(lookup: Map<string, string>, other_lookup: Map<string, string>, key: string, other: string): boolean {\n  return Array.from(lookup.keys()).includes(key);\n}\n",
    )
    .unwrap();
    fs::write(
        dir.join("wrong_key.ts"),
        "function f(lookup: Map<string, string>, other_lookup: Map<string, string>, key: string, other: string): boolean {\n  return lookup.has(other);\n}\n",
    )
    .unwrap();
    fs::write(
        dir.join("python_keys_wrong_key.py"),
        "def f(lookup: dict[str, str], other_lookup: dict[str, str], key: str, other: str) -> bool:\n    return other in lookup.keys()\n",
    )
    .unwrap();
    fs::write(
        dir.join("python_keys_wrong_map.py"),
        "def f(lookup: dict[str, str], other_lookup: dict[str, str], key: str, other: str) -> bool:\n    return key in other_lookup.keys()\n",
    )
    .unwrap();
    fs::write(
        dir.join("python_values_view.py"),
        "def f(lookup: dict[str, str], other_lookup: dict[str, str], key: str, other: str) -> bool:\n    return key in lookup.values()\n",
    )
    .unwrap();
    fs::write(
        dir.join("ts_array_from_keys_wrong_key.ts"),
        "function f(lookup: Map<string, string>, other_lookup: Map<string, string>, key: string, other: string): boolean {\n  return Array.from(lookup.keys()).includes(other);\n}\n",
    )
    .unwrap();
    fs::write(
        dir.join("ts_array_from_keys_wrong_map.ts"),
        "function f(lookup: Map<string, string>, other_lookup: Map<string, string>, key: string, other: string): boolean {\n  return Array.from(other_lookup.keys()).includes(key);\n}\n",
    )
    .unwrap();
    fs::write(
        dir.join("ts_array_from_values.ts"),
        "function f(lookup: Map<string, string>, other_lookup: Map<string, string>, key: string, other: string): boolean {\n  return Array.from(lookup.values()).includes(key);\n}\n",
    )
    .unwrap();
    fs::write(
        dir.join("set_negative.ts"),
        "function f(lookup: Set<string>, other_lookup: Set<string>, key: string, other: string): boolean {\n  return lookup.has(key);\n}\n",
    )
    .unwrap();

    let semantic = run(&[
        "scan",
        dir.to_str().unwrap(),
        "--mode",
        "semantic",
        "--min-lines",
        "1",
        "--min-size",
        "1",
        "--format",
        "json",
        "--top",
        "0",
    ]);
    let semantic_json = scan_json(&semantic);
    let semantic_families = scan_families(&semantic_json);
    let positive_family = semantic_families
        .iter()
        .map(serde_json::Value::to_string)
        .find(|family| {
            [
                "map_key.py",
                "map_key.java",
                "map_key.ts",
                "map_key_python_keys_in.py",
                "map_key_python_keys_contains.py",
                "map_key_ts_array_from_keys.ts",
            ]
            .iter()
            .all(|expected| family.contains(expected))
        })
        .unwrap_or_else(|| panic!("semantic mode should include typed map-key family: {semantic}"));
    for expected in [
        "map_key.py",
        "map_key.java",
        "map_key.ts",
        "map_key_python_keys_in.py",
        "map_key_python_keys_contains.py",
        "map_key_ts_array_from_keys.ts",
    ] {
        assert!(
            positive_family.contains(expected),
            "semantic mode should include typed TS Map.has map-key membership {expected}: {semantic}"
        );
    }
    for unexpected in [
        "wrong_key.ts",
        "set_negative.ts",
        "python_keys_wrong_key.py",
        "python_keys_wrong_map.py",
        "python_values_view.py",
        "ts_array_from_keys_wrong_key.ts",
        "ts_array_from_keys_wrong_map.ts",
        "ts_array_from_values.ts",
    ] {
        assert!(
            !positive_family.contains(unexpected),
            "semantic mode must preserve typed TS Map.has boundaries: {semantic}"
        );
    }

    let _ = fs::remove_dir_all(&dir);
}

// Broad fixture matrix for typed TypeScript map default lookup contracts. The size is
// intentional until the fixture setup has a clearer table-builder abstraction.
#[allow(clippy::too_many_lines)]
#[test]
fn scan_mode_semantic_proves_typed_typescript_map_default_lookup() {
    let dir =
        std::env::temp_dir().join(format!("nose_typed_ts_map_default_{}", std::process::id()));
    let _ = fs::remove_dir_all(&dir);
    fs::create_dir_all(&dir).unwrap();
    fs::write(
        dir.join("map_default.go"),
        "package p\n\nfunc F(lookup map[string]int, otherLookup map[string]int, key string, otherKey string, fallback int, otherDefault int) int { value, ok := lookup[key]; if !ok { value = fallback }; return value }\n",
    )
    .unwrap();
    fs::write(
        dir.join("ts_nullish.ts"),
        "function f(lookup: Map<string, number>, other_lookup: Map<string, number>, key: string, other_key: string, fallback: number, other_default: number): number {\n  return lookup.get(key) ?? fallback;\n}\n",
    )
    .unwrap();
    fs::write(
        dir.join("ts_has_get.ts"),
        "function f(lookup: Map<string, number>, other_lookup: Map<string, number>, key: string, other_key: string, fallback: number, other_default: number): number {\n  return lookup.has(key) ? lookup.get(key) : fallback;\n}\n",
    )
    .unwrap();
    fs::write(
        dir.join("ts_temp_guard.ts"),
        "function f(lookup: Map<string, number>, other_lookup: Map<string, number>, key: string, other_key: string, fallback: number, other_default: number): number {\n  const selected = lookup.get(key);\n  return selected === undefined ? fallback : selected;\n}\n",
    )
    .unwrap();
    fs::write(
        dir.join("ts_guard_return.ts"),
        "function f(lookup: Map<string, number>, other_lookup: Map<string, number>, key: string, other_key: string, fallback: number, other_default: number): number {\n  if (lookup.has(key)) {\n    return lookup.get(key)!;\n  }\n  return fallback;\n}\n",
    )
    .unwrap();
    fs::write(
        dir.join("java_guard_return.java"),
        "import java.util.Map;\n\nclass C { static int f(Map<String, Integer> lookup, Map<String, Integer> other_lookup, String key, String other_key, int fallback, int other_default) { if (lookup.containsKey(key)) { return lookup.get(key); } return fallback; } }\n",
    )
    .unwrap();
    fs::write(
        dir.join("ts_wrong_key.ts"),
        "function f(lookup: Map<string, number>, other_lookup: Map<string, number>, key: string, other_key: string, fallback: number, other_default: number): number {\n  return lookup.get(other_key) ?? fallback;\n}\n",
    )
    .unwrap();
    fs::write(
        dir.join("ts_wrong_default.ts"),
        "function f(lookup: Map<string, number>, other_lookup: Map<string, number>, key: string, other_key: string, fallback: number, other_default: number): number {\n  return lookup.get(key) ?? other_default;\n}\n",
    )
    .unwrap();
    fs::write(
        dir.join("ts_wrong_map.ts"),
        "function f(lookup: Map<string, number>, other_lookup: Map<string, number>, key: string, other_key: string, fallback: number, other_default: number): number {\n  return other_lookup.get(key) ?? fallback;\n}\n",
    )
    .unwrap();
    fs::write(
        dir.join("ts_untyped.ts"),
        "function f(lookup, other_lookup, key, other_key, fallback, other_default) {\n  return lookup.get(key) ?? fallback;\n}\n",
    )
    .unwrap();
    fs::write(
        dir.join("ts_guard_wrong_key.ts"),
        "function f(lookup: Map<string, number>, other_lookup: Map<string, number>, key: string, other_key: string, fallback: number, other_default: number): number {\n  if (lookup.has(other_key)) {\n    return lookup.get(other_key)!;\n  }\n  return fallback;\n}\n",
    )
    .unwrap();
    fs::write(
        dir.join("java_guard_wrong_default.java"),
        "import java.util.Map;\n\nclass C { static int f(Map<String, Integer> lookup, Map<String, Integer> other_lookup, String key, String other_key, int fallback, int other_default) { if (lookup.containsKey(key)) { return lookup.get(key); } return other_default; } }\n",
    )
    .unwrap();
    fs::write(
        dir.join("py_dict.py"),
        "def f(lookup: dict[str, int], other_lookup: dict[str, int], key: str, other_key: str, fallback: int, other_default: int) -> int:\n    return lookup.get(key, fallback)\n",
    )
    .unwrap();
    fs::write(
        dir.join("py_guard_return.py"),
        "def f(lookup: dict[str, int], other_lookup: dict[str, int], key: str, other_key: str, fallback: int, other_default: int) -> int:\n    if key in lookup:\n        return lookup[key]\n    return fallback\n",
    )
    .unwrap();
    fs::write(
        dir.join("py_mapping.py"),
        "from collections.abc import Mapping\n\ndef f(lookup: Mapping[str, int], other_lookup: Mapping[str, int], key: str, other_key: str, fallback: int, other_default: int) -> int:\n    return lookup.get(key, fallback)\n",
    )
    .unwrap();
    fs::write(
        dir.join("py_mutable_mapping.py"),
        "from collections.abc import MutableMapping\n\ndef f(lookup: MutableMapping[str, int], other_lookup: MutableMapping[str, int], key: str, other_key: str, fallback: int, other_default: int) -> int:\n    return lookup.get(key, fallback)\n",
    )
    .unwrap();
    fs::write(
        dir.join("py_alias_mapping.py"),
        "from collections.abc import Mapping as MapLike\n\ndef f(lookup: MapLike[str, int], other_lookup: MapLike[str, int], key: str, other_key: str, fallback: int, other_default: int) -> int:\n    return lookup.get(key, fallback)\n",
    )
    .unwrap();
    fs::write(
        dir.join("py_alias_mutable_mapping.py"),
        "from collections.abc import MutableMapping as MapLike\n\ndef f(lookup: MapLike[str, int], other_lookup: MapLike[str, int], key: str, other_key: str, fallback: int, other_default: int) -> int:\n    return lookup.get(key, fallback)\n",
    )
    .unwrap();
    fs::write(
        dir.join("py_alias_dict.py"),
        "from typing import Dict as MapLike\n\ndef f(lookup: MapLike[str, int], other_lookup: MapLike[str, int], key: str, other_key: str, fallback: int, other_default: int) -> int:\n    return lookup.get(key, fallback)\n",
    )
    .unwrap();
    fs::write(
        dir.join("py_wrong_key.py"),
        "def f(lookup: dict[str, int], other_lookup: dict[str, int], key: str, other_key: str, fallback: int, other_default: int) -> int:\n    return lookup.get(other_key, fallback)\n",
    )
    .unwrap();
    fs::write(
        dir.join("py_wrong_default.py"),
        "def f(lookup: dict[str, int], other_lookup: dict[str, int], key: str, other_key: str, fallback: int, other_default: int) -> int:\n    return lookup.get(key, other_default)\n",
    )
    .unwrap();
    fs::write(
        dir.join("py_wrong_map.py"),
        "def f(lookup: dict[str, int], other_lookup: dict[str, int], key: str, other_key: str, fallback: int, other_default: int) -> int:\n    return other_lookup.get(key, fallback)\n",
    )
    .unwrap();
    fs::write(
        dir.join("py_guard_wrong_map.py"),
        "def f(lookup: dict[str, int], other_lookup: dict[str, int], key: str, other_key: str, fallback: int, other_default: int) -> int:\n    if key in other_lookup:\n        return other_lookup[key]\n    return fallback\n",
    )
    .unwrap();
    fs::write(
        dir.join("py_untyped.py"),
        "def f(lookup, other_lookup, key, other_key, fallback, other_default):\n    return lookup.get(key, fallback)\n",
    )
    .unwrap();
    fs::write(
        dir.join("py_alias_wrong_key.py"),
        "from collections.abc import Mapping as MapLike\n\ndef f(lookup: MapLike[str, int], other_lookup: MapLike[str, int], key: str, other_key: str, fallback: int, other_default: int) -> int:\n    return lookup.get(other_key, fallback)\n",
    )
    .unwrap();
    fs::write(
        dir.join("py_alias_wrong_default.py"),
        "from collections.abc import Mapping as MapLike\n\ndef f(lookup: MapLike[str, int], other_lookup: MapLike[str, int], key: str, other_key: str, fallback: int, other_default: int) -> int:\n    return lookup.get(key, other_default)\n",
    )
    .unwrap();
    fs::write(
        dir.join("py_alias_wrong_map.py"),
        "from collections.abc import Mapping as MapLike\n\ndef f(lookup: MapLike[str, int], other_lookup: MapLike[str, int], key: str, other_key: str, fallback: int, other_default: int) -> int:\n    return other_lookup.get(key, fallback)\n",
    )
    .unwrap();
    fs::write(
        dir.join("py_alias_unresolved.py"),
        "def f(lookup: MapLike[str, int], other_lookup: MapLike[str, int], key: str, other_key: str, fallback: int, other_default: int) -> int:\n    return lookup.get(key, fallback)\n",
    )
    .unwrap();
    fs::write(
        dir.join("py_alias_shadowed.py"),
        "from collections.abc import Mapping as MapLike\nMapLike = list\n\ndef f(lookup: MapLike[str, int], other_lookup: MapLike[str, int], key: str, other_key: str, fallback: int, other_default: int) -> int:\n    return lookup.get(key, fallback)\n",
    )
    .unwrap();

    let semantic = run(&[
        "scan",
        dir.to_str().unwrap(),
        "--mode",
        "semantic",
        "--min-lines",
        "1",
        "--min-size",
        "1",
        "--format",
        "json",
        "--top",
        "0",
    ]);
    let semantic_json = scan_json(&semantic);
    let semantic_families = scan_families(&semantic_json);
    let expected = [
        "map_default.go",
        "ts_nullish.ts",
        "ts_has_get.ts",
        "ts_temp_guard.ts",
        "ts_guard_return.ts",
        "java_guard_return.java",
        "py_dict.py",
        "py_guard_return.py",
        "py_mapping.py",
        "py_mutable_mapping.py",
        "py_alias_mapping.py",
        "py_alias_mutable_mapping.py",
        "py_alias_dict.py",
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
            panic!("semantic mode should report one typed map default family: {semantic}")
        });
    let positive_text = positive_family.to_string();
    for expected in expected {
        assert!(
            positive_text.contains(expected),
            "semantic mode should include typed map default lookup {expected}: {semantic}"
        );
    }
    for unexpected in [
        "ts_wrong_key.ts",
        "ts_wrong_default.ts",
        "ts_wrong_map.ts",
        "ts_untyped.ts",
        "ts_guard_wrong_key.ts",
        "java_guard_wrong_default.java",
        "py_wrong_key.py",
        "py_wrong_default.py",
        "py_wrong_map.py",
        "py_guard_wrong_map.py",
        "py_untyped.py",
        "py_alias_wrong_key.py",
        "py_alias_wrong_default.py",
        "py_alias_wrong_map.py",
        "py_alias_unresolved.py",
        "py_alias_shadowed.py",
    ] {
        assert!(
            !positive_text.contains(unexpected),
            "semantic mode must preserve typed map default boundaries: {semantic}"
        );
    }

    let _ = fs::remove_dir_all(&dir);
}

// Broad fixture matrix for literal map default contracts. The size is
// intentional until the fixture setup has a clearer table-builder abstraction.
#[allow(clippy::too_many_lines)]
#[test]
fn scan_mode_semantic_proves_literal_map_default_lookup() {
    let dir = std::env::temp_dir().join(format!("nose_map_default_{}", std::process::id()));
    let _ = fs::remove_dir_all(&dir);
    fs::create_dir_all(&dir).unwrap();
    fs::write(
        dir.join("map_default.py"),
        "def lookup(key, other):\n    return {\"red\": 1, \"blue\": 2}.get(key, 0)\n",
    )
    .unwrap();
    fs::write(
        dir.join("map_default.rb"),
        "def lookup(key, other)\n  {\"red\" => 1, \"blue\" => 2}.fetch(key, 0)\nend\n",
    )
    .unwrap();
    fs::write(
        dir.join("map_default_block.rb"),
        "def lookup(key, other)\n  {\"red\" => 1, \"blue\" => 2}.fetch(key) { 0 }\nend\n",
    )
    .unwrap();
    fs::write(
        dir.join("map_default_inline.js"),
        "function lookup(key, other) {\n  return new Map([[\"red\", 1], [\"blue\", 2]]).get(key) ?? 0;\n}\n",
    )
    .unwrap();
    fs::write(
        dir.join("map_default_call.js"),
        "function lookup(key, other) {\n  return Map([[\"red\", 1], [\"blue\", 2]]).get(key) ?? 0;\n}\n",
    )
    .unwrap();
    fs::write(
        dir.join("shadowed_map_global.js"),
        "function Map(entries) {\n  return { get: function() { return 99; }, has: function() { return true; } };\n}\nfunction lookup(key, other) {\n  return new Map([[\"red\", 1], [\"blue\", 2]]).get(key) ?? 0;\n}\n",
    )
    .unwrap();
    fs::write(
        dir.join("map_default_local.js"),
        "function lookup(key, other) {\n  const values = new Map([[\"red\", 1], [\"blue\", 2]]);\n  return values.get(key) ?? 0;\n}\n",
    )
    .unwrap();
    fs::write(
        dir.join("map_default_has_get.js"),
        "function lookup(key, other) {\n  const values = new Map([[\"red\", 1], [\"blue\", 2]]);\n  return values.has(key) ? values.get(key) : 0;\n}\n",
    )
    .unwrap();
    fs::write(
        dir.join("map_default_inline.ts"),
        "function lookup(key: string, other: string): number {\n  return new Map<string, number>([[\"red\", 1], [\"blue\", 2]]).get(key) ?? 0;\n}\n",
    )
    .unwrap();
    fs::write(
        dir.join("map_default_java_of.java"),
        "import java.util.Map;\n\nclass JavaMapOf {\n  static int lookup(String key, String other) {\n    return Map.of(\"red\", 1, \"blue\", 2).getOrDefault(key, 0);\n  }\n}\n",
    )
    .unwrap();
    fs::write(
        dir.join("map_default_java_entries.java"),
        "import java.util.Map;\n\nclass JavaMapEntries {\n  static int lookup(String key, String other) {\n    return Map.ofEntries(Map.entry(\"red\", 1), Map.entry(\"blue\", 2)).getOrDefault(key, 0);\n  }\n}\n",
    )
    .unwrap();
    fs::write(
        dir.join("map_default_java_local.java"),
        "import java.util.Map;\n\nclass JavaMapLocal {\n  static int lookup(String key, String other) {\n    Map<String, Integer> values = Map.of(\"red\", 1, \"blue\", 2);\n    return values.getOrDefault(key, 0);\n  }\n}\n",
    )
    .unwrap();
    fs::write(
        dir.join("map_default_module.js"),
        "const LOOKUP = new Map([[\"red\", 1], [\"blue\", 2]]);\n\nfunction lookup(key, other) {\n  return LOOKUP.get(key) ?? 0;\n}\n",
    )
    .unwrap();
    fs::write(
        dir.join("map_default_module.ts"),
        "const LOOKUP = new Map<string, number>([[\"red\", 1], [\"blue\", 2]]);\n\nfunction lookup(key: string, other: string): number {\n  return LOOKUP.get(key) ?? 0;\n}\n",
    )
    .unwrap();
    fs::write(
        dir.join("map_default_module.java"),
        "import java.util.Map;\n\nclass JavaModuleMap {\n  static final Map<String, Integer> LOOKUP = Map.of(\"red\", 1, \"blue\", 2);\n\n  static int lookup(String key, String other) {\n    return LOOKUP.getOrDefault(key, 0);\n  }\n}\n",
    )
    .unwrap();
    fs::write(
        dir.join("map_default_rust_hashmap.rs"),
        "pub fn lookup(key: &str, other: &str) -> i32 {\n    *std::collections::HashMap::from([(\"red\", 1), (\"blue\", 2)]).get(key).unwrap_or(&0)\n}\n",
    )
    .unwrap();
    fs::write(
        dir.join("map_default_rust_btreemap.rs"),
        "pub fn lookup(key: &str, other: &str) -> i32 {\n    *std::collections::BTreeMap::from([(\"red\", 1), (\"blue\", 2)]).get(key).unwrap_or(&0)\n}\n",
    )
    .unwrap();
    fs::write(
        dir.join("map_default_rust_local.rs"),
        "pub fn lookup(key: &str, other: &str) -> i32 {\n    let values = std::collections::HashMap::from([(\"red\", 1), (\"blue\", 2)]);\n    *values.get(key).unwrap_or(&0)\n}\n",
    )
    .unwrap();
    fs::write(
        dir.join("map_default_go_inline.go"),
        "package p\n\nfunc Lookup(key string, other string) int {\n    return map[string]int{\"red\": 1, \"blue\": 2}[key]\n}\n",
    )
    .unwrap();
    fs::write(
        dir.join("map_default_go_local.go"),
        "package p\n\nfunc Lookup(key string, other string) int {\n    lookup := map[string]int{\"red\": 1, \"blue\": 2}\n    return lookup[key]\n}\n",
    )
    .unwrap();
    fs::write(
        dir.join("map_default_go_var.go"),
        "package p\n\nfunc Lookup(key string, other string) int {\n    var lookup = map[string]int{\"red\": 1, \"blue\": 2}\n    return lookup[key]\n}\n",
    )
    .unwrap();
    fs::write(
        dir.join("map_default_string.py"),
        "def lookup(key, other):\n    return {\"red\": \"apple\", \"blue\": \"berry\"}.get(key, \"\")\n",
    )
    .unwrap();
    fs::write(
        dir.join("map_default_string.rb"),
        "def lookup(key, other)\n  {\"red\" => \"apple\", \"blue\" => \"berry\"}.fetch(key, \"\")\nend\n",
    )
    .unwrap();
    fs::write(
        dir.join("map_default_string_block.rb"),
        "def lookup(key, other)\n  {\"red\" => \"apple\", \"blue\" => \"berry\"}.fetch(key) { \"\" }\nend\n",
    )
    .unwrap();
    fs::write(
        dir.join("map_default_go_string_inline.go"),
        "package p\n\nfunc Lookup(key string, other string) string {\n    return map[string]string{\"red\": \"apple\", \"blue\": \"berry\"}[key]\n}\n",
    )
    .unwrap();
    fs::write(
        dir.join("map_default_go_string_local.go"),
        "package p\n\nfunc Lookup(key string, other string) string {\n    lookup := map[string]string{\"red\": \"apple\", \"blue\": \"berry\"}\n    return lookup[key]\n}\n",
    )
    .unwrap();
    fs::write(
        dir.join("map_default_bool.py"),
        "def lookup(key, other):\n    return {\"red\": True, \"blue\": False}.get(key, False)\n",
    )
    .unwrap();
    fs::write(
        dir.join("map_default_bool.rb"),
        "def lookup(key, other)\n  {\"red\" => true, \"blue\" => false}.fetch(key, false)\nend\n",
    )
    .unwrap();
    fs::write(
        dir.join("map_default_bool_block.rb"),
        "def lookup(key, other)\n  {\"red\" => true, \"blue\" => false}.fetch(key) { false }\nend\n",
    )
    .unwrap();
    fs::write(
        dir.join("map_default_go_bool_inline.go"),
        "package p\n\nfunc Lookup(key string, other string) bool {\n    return map[string]bool{\"red\": true, \"blue\": false}[key]\n}\n",
    )
    .unwrap();
    fs::write(
        dir.join("map_default_float.py"),
        "def lookup(key, other):\n    return {\"red\": 1.5, \"blue\": 2.5}.get(key, 0.0)\n",
    )
    .unwrap();
    fs::write(
        dir.join("map_default_float.rb"),
        "def lookup(key, other)\n  {\"red\" => 1.5, \"blue\" => 2.5}.fetch(key, 0.0)\nend\n",
    )
    .unwrap();
    fs::write(
        dir.join("map_default_go_float_inline.go"),
        "package p\n\nfunc Lookup(key string, other string) float64 {\n    return map[string]float64{\"red\": 1.5, \"blue\": 2.5}[key]\n}\n",
    )
    .unwrap();
    fs::write(
        dir.join("map_default_go_float_local.go"),
        "package p\n\nfunc Lookup(key string, other string) float64 {\n    lookup := map[string]float64{\"red\": 1.5, \"blue\": 2.5}\n    return lookup[key]\n}\n",
    )
    .unwrap();
    fs::write(
        dir.join("map_default_nil.py"),
        "def lookup(key, other):\n    return {\"red\": None, \"blue\": None}.get(key, None)\n",
    )
    .unwrap();
    fs::write(
        dir.join("map_default_nil.rb"),
        "def lookup(key, other)\n  {\"red\" => nil, \"blue\" => nil}.fetch(key, nil)\nend\n",
    )
    .unwrap();
    fs::write(
        dir.join("map_default_nil_block.rb"),
        "def lookup(key, other)\n  {\"red\" => nil, \"blue\" => nil}.fetch(key) { nil }\nend\n",
    )
    .unwrap();
    fs::write(
        dir.join("map_default_go_nil_inline.go"),
        "package p\n\ntype Item struct{}\n\nfunc Lookup(key string, other string) *Item {\n    return map[string]*Item{\"red\": nil, \"blue\": nil}[key]\n}\n",
    )
    .unwrap();
    fs::write(
        dir.join("object_hasown.js"),
        "function lookup(key, other) {\n  const values = { \"red\": 1, \"blue\": 2 };\n  return Object.hasOwn(values, key) ? values[key] : 0;\n}\n",
    )
    .unwrap();
    fs::write(
        dir.join("object_hasown_call.js"),
        "function lookup(key, other) {\n  const values = { \"red\": 1, \"blue\": 2 };\n  return Object.prototype.hasOwnProperty.call(values, key) ? values[key] : 0;\n}\n",
    )
    .unwrap();
    fs::write(
        dir.join("object_negated.ts"),
        "function lookup(key: string, other: string): number {\n  const values: Record<string, number> = { \"red\": 1, \"blue\": 2 };\n  return !Object.hasOwn(values, key) ? 0 : values[key];\n}\n",
    )
    .unwrap();
    fs::write(
        dir.join("wrong_key.py"),
        "def wrong_key(key, other):\n    return {\"red\": 1, \"blue\": 2}.get(other, 0)\n",
    )
    .unwrap();
    fs::write(
        dir.join("wrong_default.rb"),
        "def wrong_default(key, other)\n  {\"red\" => 1, \"blue\" => 2}.fetch(key, 9)\nend\n",
    )
    .unwrap();
    fs::write(
        dir.join("wrong_default_block.rb"),
        "def wrong_default(key, other)\n  {\"red\" => 1, \"blue\" => 2}.fetch(key) { 9 }\nend\n",
    )
    .unwrap();
    fs::write(
        dir.join("wrong_map.py"),
        "def wrong_map(key, other):\n    return {\"red\": 9, \"blue\": 2}.get(key, 0)\n",
    )
    .unwrap();
    fs::write(
        dir.join("ruby_fetch_block_param.rb"),
        "def wrong(key, other)\n  {\"red\" => 1, \"blue\" => 2}.fetch(key) { |missing| missing.to_s }\nend\n",
    )
    .unwrap();
    fs::write(
        dir.join("ruby_fetch_raise_block.rb"),
        "def wrong(key, other)\n  {\"red\" => 1, \"blue\" => 2}.fetch(key) { raise KeyError }\nend\n",
    )
    .unwrap();
    fs::write(
        dir.join("wrong_js_key.js"),
        "function wrong_key(key, other) {\n  return new Map([[\"red\", 1], [\"blue\", 2]]).get(other) ?? 0;\n}\n",
    )
    .unwrap();
    fs::write(
        dir.join("wrong_js_default.js"),
        "function wrong_default(key, other) {\n  return new Map([[\"red\", 1], [\"blue\", 2]]).get(key) ?? 9;\n}\n",
    )
    .unwrap();
    fs::write(
        dir.join("wrong_js_map.js"),
        "function wrong_map(key, other) {\n  return new Map([[\"red\", 9], [\"blue\", 2]]).get(key) ?? 0;\n}\n",
    )
    .unwrap();
    fs::write(
        dir.join("untyped_receiver.js"),
        "function untyped_receiver(values, key, other) {\n  return values.get(key) ?? 0;\n}\n",
    )
    .unwrap();
    fs::write(
        dir.join("shadowed_map.js"),
        "function shadowed_map(key, other, Map) {\n  return new Map([[\"red\", 1], [\"blue\", 2]]).get(key) ?? 0;\n}\n",
    )
    .unwrap();
    fs::write(
        dir.join("wrong_java_key.java"),
        "import java.util.Map;\n\nclass WrongJavaKey {\n  static int wrong(String key, String other) {\n    return Map.of(\"red\", 1, \"blue\", 2).getOrDefault(other, 0);\n  }\n}\n",
    )
    .unwrap();
    fs::write(
        dir.join("wrong_java_default.java"),
        "import java.util.Map;\n\nclass WrongJavaDefault {\n  static int wrong(String key, String other) {\n    return Map.of(\"red\", 1, \"blue\", 2).getOrDefault(key, 9);\n  }\n}\n",
    )
    .unwrap();
    fs::write(
        dir.join("wrong_java_map.java"),
        "import java.util.Map;\n\nclass WrongJavaMap {\n  static int wrong(String key, String other) {\n    return Map.of(\"red\", 9, \"blue\", 2).getOrDefault(key, 0);\n  }\n}\n",
    )
    .unwrap();
    fs::write(
        dir.join("shadowed_java_map.java"),
        "class ShadowedJavaMap {\n  static class MapFactory {\n    java.util.Map<String, Integer> of(Object... values) { return java.util.Map.of(); }\n  }\n  static int wrong(String key, String other, MapFactory Map) {\n    return Map.of(\"red\", 1, \"blue\", 2).getOrDefault(key, 0);\n  }\n}\n",
    )
    .unwrap();
    fs::write(
        dir.join("local_java_map_type.java"),
        "class LocalJavaMapType {\n  static int wrong(String key, String other) {\n    return Map.of(\"red\", 1, \"blue\", 2).getOrDefault(key, 0);\n  }\n}\nclass Map {\n  static java.util.Map<String, Integer> of(Object... values) { return java.util.Map.of(); }\n}\n",
    )
    .unwrap();
    fs::write(
        dir.join("module_map_missing_java_import.java"),
        "class JavaModuleMapMissingImport {\n  static final Map<String, Integer> LOOKUP = Map.of(\"red\", 1, \"blue\", 2);\n\n  static int wrong(String key, String other) {\n    return LOOKUP.getOrDefault(key, 0);\n  }\n}\n",
    )
    .unwrap();
    fs::write(
        dir.join("module_map_mutated.js"),
        "const LOOKUP = new Map([[\"red\", 1], [\"blue\", 2]]);\nLOOKUP.set(\"red\", 9);\n\nfunction wrong(key, other) {\n  return LOOKUP.get(key) ?? 0;\n}\n",
    )
    .unwrap();
    fs::write(
        dir.join("module_map_shadowed.ts"),
        "const Map: any = function(_entries: any) {\n  return { get: function() { return 9; } };\n};\nconst LOOKUP = new Map([[\"red\", 1], [\"blue\", 2]]);\n\nfunction wrong(key: string, other: string): number {\n  return LOOKUP.get(key) ?? 0;\n}\n",
    )
    .unwrap();
    fs::write(
        dir.join("module_map_shadowed.java"),
        "class JavaShadowedModuleMap {\n  static final Map<String, Integer> LOOKUP = Map.of(\"red\", 1, \"blue\", 2);\n\n  static int wrong(String key, String other) {\n    return LOOKUP.getOrDefault(key, 0);\n  }\n}\nclass Map {\n  static java.util.Map<String, Integer> of(Object... values) { return java.util.Map.of(); }\n}\n",
    )
    .unwrap();
    fs::write(
        dir.join("wrong_rust_key.rs"),
        "pub fn wrong(key: &str, other: &str) -> i32 {\n    *std::collections::HashMap::from([(\"red\", 1), (\"blue\", 2)]).get(other).unwrap_or(&0)\n}\n",
    )
    .unwrap();
    fs::write(
        dir.join("wrong_rust_default.rs"),
        "pub fn wrong(key: &str, other: &str) -> i32 {\n    *std::collections::HashMap::from([(\"red\", 1), (\"blue\", 2)]).get(key).unwrap_or(&9)\n}\n",
    )
    .unwrap();
    fs::write(
        dir.join("wrong_rust_map.rs"),
        "pub fn wrong(key: &str, other: &str) -> i32 {\n    *std::collections::HashMap::from([(\"red\", 9), (\"blue\", 2)]).get(key).unwrap_or(&0)\n}\n",
    )
    .unwrap();
    fs::write(
        dir.join("rust_map_mutated.rs"),
        "pub fn wrong(key: &str, other: &str) -> i32 {\n    let mut values = std::collections::HashMap::from([(\"red\", 1), (\"blue\", 2)]);\n    values.insert(\"red\", 9);\n    *values.get(key).unwrap_or(&0)\n}\n",
    )
    .unwrap();
    fs::write(
        dir.join("wrong_go_key.go"),
        "package p\n\nfunc Wrong(key string, other string) int {\n    return map[string]int{\"red\": 1, \"blue\": 2}[other]\n}\n",
    )
    .unwrap();
    fs::write(
        dir.join("wrong_go_map.go"),
        "package p\n\nfunc Wrong(key string, other string) int {\n    return map[string]int{\"red\": 9, \"blue\": 2}[key]\n}\n",
    )
    .unwrap();
    fs::write(
        dir.join("go_keyed_slice.go"),
        "package p\n\nfunc Wrong(key int, other int) int {\n    return []int{0: 1, 1: 2}[key]\n}\n",
    )
    .unwrap();
    fs::write(
        dir.join("wrong_go_string_key.go"),
        "package p\n\nfunc Wrong(key string, other string) string {\n    return map[string]string{\"red\": \"apple\", \"blue\": \"berry\"}[other]\n}\n",
    )
    .unwrap();
    fs::write(
        dir.join("wrong_go_bool_map.go"),
        "package p\n\nfunc Wrong(key string, other string) bool {\n    return map[string]bool{\"red\": false, \"blue\": false}[key]\n}\n",
    )
    .unwrap();
    fs::write(
        dir.join("wrong_go_float_key.go"),
        "package p\n\nfunc Wrong(key string, other string) float64 {\n    return map[string]float64{\"red\": 1.5, \"blue\": 2.5}[other]\n}\n",
    )
    .unwrap();
    fs::write(
        dir.join("wrong_go_nil_map.go"),
        "package p\n\nfunc Wrong(key string, other string) string {\n    return map[string]string{\"red\": \"apricot\", \"blue\": \"berry\"}[key]\n}\n",
    )
    .unwrap();
    fs::write(
        dir.join("go_mixed_value_map.go"),
        "package p\n\nfunc Wrong(key string, other string) interface{} {\n    return map[string]interface{}{\"red\": \"apple\", \"blue\": false}[key]\n}\n",
    )
    .unwrap();
    fs::write(
        dir.join("go_string_keyed_slice.go"),
        "package p\n\nfunc Wrong(key int, other int) string {\n    return []string{0: \"apple\", 1: \"berry\"}[key]\n}\n",
    )
    .unwrap();
    fs::write(
        dir.join("object_wrong_key.js"),
        "function wrong_key(key, other) {\n  const values = { \"red\": 1, \"blue\": 2 };\n  return Object.hasOwn(values, other) ? values[other] : 0;\n}\n",
    )
    .unwrap();
    fs::write(
        dir.join("object_wrong_default.js"),
        "function wrong_default(key, other) {\n  const values = { \"red\": 1, \"blue\": 2 };\n  return Object.hasOwn(values, key) ? values[key] : 9;\n}\n",
    )
    .unwrap();
    fs::write(
        dir.join("object_wrong_map.js"),
        "function wrong_map(key, other) {\n  const values = { \"red\": 9, \"blue\": 2 };\n  return Object.hasOwn(values, key) ? values[key] : 0;\n}\n",
    )
    .unwrap();
    fs::write(
        dir.join("object_unguarded.js"),
        "function unguarded(key, other) {\n  const values = { \"red\": 1, \"blue\": 2 };\n  return values[key] ?? 0;\n}\n",
    )
    .unwrap();
    fs::write(
        dir.join("object_in.js"),
        "function object_in(key, other) {\n  const values = { \"red\": 1, \"blue\": 2 };\n  return key in values ? values[key] : 0;\n}\n",
    )
    .unwrap();
    fs::write(
        dir.join("object_method.js"),
        "function object_method(key, other) {\n  const values = { \"red\": 1, \"blue\": 2 };\n  return values.hasOwnProperty(key) ? values[key] : 0;\n}\n",
    )
    .unwrap();
    fs::write(
        dir.join("object_shadowed.js"),
        "function object_shadowed(key, other, Object) {\n  const values = { \"red\": 1, \"blue\": 2 };\n  return Object.hasOwn(values, key) ? values[key] : 0;\n}\n",
    )
    .unwrap();

    let semantic = run(&[
        "scan",
        dir.to_str().unwrap(),
        "--mode",
        "semantic",
        "--min-lines",
        "1",
        "--min-size",
        "1",
        "--format",
        "json",
        "--top",
        "0",
    ]);
    let semantic_json = scan_json(&semantic);
    let semantic_families = scan_families(&semantic_json);
    let expected = [
        "map_default.py",
        "map_default.rb",
        "map_default_block.rb",
        "map_default_java_of.java",
        "map_default_java_entries.java",
        "map_default_java_local.java",
        "map_default_module.java",
        "map_default_inline.js",
        "map_default_local.js",
        "map_default_has_get.js",
        "map_default_inline.ts",
        "map_default_module.js",
        "map_default_module.ts",
        "map_default_rust_hashmap.rs",
        "map_default_rust_btreemap.rs",
        "map_default_go_inline.go",
        "map_default_go_local.go",
        "map_default_go_var.go",
        "object_hasown.js",
        "object_hasown_call.js",
        "object_negated.ts",
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
            panic!("semantic mode should report one literal map-default family: {semantic}")
        });
    let positive_text = positive_family.to_string();
    for expected in expected {
        assert!(
            positive_text.contains(expected),
            "semantic mode should include {expected}: {semantic}"
        );
    }
    let string_expected = [
        "map_default_string.py",
        "map_default_string.rb",
        "map_default_string_block.rb",
        "map_default_go_string_inline.go",
        "map_default_go_string_local.go",
    ];
    let string_family = semantic_families
        .iter()
        .find(|family| {
            let family_text = family.to_string();
            string_expected
                .iter()
                .all(|expected| family_text.contains(expected))
        })
        .unwrap_or_else(|| {
            panic!("semantic mode should report one string map-default family: {semantic}")
        });
    let string_text = string_family.to_string();
    for expected in string_expected {
        assert!(
            string_text.contains(expected),
            "semantic mode should include string map-default {expected}: {semantic}"
        );
    }

    let bool_expected = [
        "map_default_bool.py",
        "map_default_bool.rb",
        "map_default_bool_block.rb",
        "map_default_go_bool_inline.go",
    ];
    let bool_family = semantic_families
        .iter()
        .find(|family| {
            let family_text = family.to_string();
            bool_expected
                .iter()
                .all(|expected| family_text.contains(expected))
        })
        .unwrap_or_else(|| {
            panic!("semantic mode should report one bool map-default family: {semantic}")
        });
    let bool_text = bool_family.to_string();
    for expected in bool_expected {
        assert!(
            bool_text.contains(expected),
            "semantic mode should include bool map-default {expected}: {semantic}"
        );
    }

    let float_expected = [
        "map_default_float.py",
        "map_default_float.rb",
        "map_default_go_float_inline.go",
        "map_default_go_float_local.go",
    ];
    let float_family = semantic_families
        .iter()
        .find(|family| {
            let family_text = family.to_string();
            float_expected
                .iter()
                .all(|expected| family_text.contains(expected))
        })
        .unwrap_or_else(|| {
            panic!("semantic mode should report one float map-default family: {semantic}")
        });
    let float_text = float_family.to_string();
    for expected in float_expected {
        assert!(
            float_text.contains(expected),
            "semantic mode should include float map-default {expected}: {semantic}"
        );
    }

    let nil_expected = [
        "map_default_nil.py",
        "map_default_nil.rb",
        "map_default_nil_block.rb",
        "map_default_go_nil_inline.go",
    ];
    let nil_family = semantic_families
        .iter()
        .find(|family| {
            let family_text = family.to_string();
            nil_expected
                .iter()
                .all(|expected| family_text.contains(expected))
        })
        .unwrap_or_else(|| {
            panic!("semantic mode should report one nil map-default family: {semantic}")
        });
    let nil_text = nil_family.to_string();
    for expected in nil_expected {
        assert!(
            nil_text.contains(expected),
            "semantic mode should include nil map-default {expected}: {semantic}"
        );
    }

    let boundary_files = [
        "wrong_key.py",
        "wrong_default.rb",
        "wrong_default_block.rb",
        "wrong_map.py",
        "ruby_fetch_block_param.rb",
        "ruby_fetch_raise_block.rb",
        "map_default_call.js",
        "shadowed_map_global.js",
        "map_default_rust_local.rs",
        "wrong_js_key.js",
        "wrong_js_default.js",
        "wrong_js_map.js",
        "untyped_receiver.js",
        "shadowed_map.js",
        "wrong_java_key.java",
        "wrong_java_default.java",
        "wrong_java_map.java",
        "shadowed_java_map.java",
        "local_java_map_type.java",
        "module_map_missing_java_import.java",
        "module_map_mutated.js",
        "module_map_shadowed.ts",
        "module_map_shadowed.java",
        "wrong_rust_key.rs",
        "wrong_rust_default.rs",
        "wrong_rust_map.rs",
        "rust_map_mutated.rs",
        "wrong_go_key.go",
        "wrong_go_map.go",
        "go_keyed_slice.go",
        "wrong_go_string_key.go",
        "wrong_go_bool_map.go",
        "wrong_go_float_key.go",
        "wrong_go_nil_map.go",
        "go_mixed_value_map.go",
        "go_string_keyed_slice.go",
        "object_wrong_key.js",
        "object_wrong_default.js",
        "object_wrong_map.js",
        "object_unguarded.js",
        "object_in.js",
        "object_method.js",
        "object_shadowed.js",
    ];
    for family_text in [
        &positive_text,
        &string_text,
        &bool_text,
        &float_text,
        &nil_text,
    ] {
        for unexpected in &boundary_files {
            assert!(
                !family_text.contains(*unexpected),
                "semantic mode must preserve literal map-default boundaries: {semantic}"
            );
        }
    }

    let _ = fs::remove_dir_all(&dir);
}

#[test]
fn scan_mode_semantic_proves_null_presence_predicates() {
    let dir = std::env::temp_dir().join(format!("nose_null_presence_{}", std::process::id()));
    let _ = fs::remove_dir_all(&dir);
    fs::create_dir_all(&dir).unwrap();
    fs::write(
        dir.join("none_compare.py"),
        "def is_missing(value, other):\n    return value is None\n",
    )
    .unwrap();
    fs::write(
        dir.join("null_compare.c"),
        "#include <stddef.h>\n\nint is_missing(void *value, void *other) {\n    return value == NULL;\n}\n",
    )
    .unwrap();
    fs::write(
        dir.join("nil_method.rb"),
        "def is_missing(value, other)\n  value.nil?\nend\n",
    )
    .unwrap();
    fs::write(
        dir.join("none_method.rs"),
        "pub fn is_missing(value: Option<i32>, other: Option<i32>) -> bool {\n    value.is_none()\n}\n",
    )
    .unwrap();
    fs::write(
        dir.join("iflet_none.rs"),
        "pub fn is_missing(value: Option<i32>, other: Option<i32>) -> bool {\n    if let None = value { true } else { false }\n}\n",
    )
    .unwrap();
    fs::write(
        dir.join("some_method.rs"),
        "pub fn is_present(value: Option<i32>, other: Option<i32>) -> bool {\n    value.is_some()\n}\n",
    )
    .unwrap();
    fs::write(
        dir.join("wrong_value.py"),
        "def wrong_value(value, other):\n    return other is None\n",
    )
    .unwrap();

    let semantic = run(&[
        "scan",
        dir.to_str().unwrap(),
        "--mode",
        "semantic",
        "--min-lines",
        "1",
        "--min-size",
        "1",
        "--format",
        "json",
        "--top",
        "0",
    ]);
    let semantic_json = scan_json(&semantic);
    let semantic_families = scan_families(&semantic_json);
    assert_eq!(
        semantic_families.len(),
        1,
        "semantic mode should report one null-presence family: {semantic}"
    );
    let semantic_text = semantic_json.to_string();
    for expected in [
        "none_compare.py",
        "null_compare.c",
        "none_method.rs",
        "iflet_none.rs",
    ] {
        assert!(
            semantic_text.contains(expected),
            "semantic mode should include {expected}: {semantic}"
        );
    }
    for unexpected in ["nil_method.rb", "some_method.rs", "wrong_value.py"] {
        assert!(
            !semantic_text.contains(unexpected),
            "semantic mode must preserve null-presence boundaries: {semantic}"
        );
    }

    let _ = fs::remove_dir_all(&dir);
}

#[test]
fn scan_mode_semantic_reports_flattened_guard_span_only() {
    let dir = std::env::temp_dir().join(format!("nose_guard_span_semantic_{}", std::process::id()));
    let _ = fs::remove_dir_all(&dir);
    fs::create_dir_all(&dir).unwrap();

    fs::write(
        dir.join("large_branch.ts"),
        "export function table(rows: string[], errors: string[]) {\n  if (rows.length === 0) {\n    return null;\n  } else {\n    let title = \"Potential problems\";\n    if (errors.length > 0) {\n      title = title.toUpperCase();\n    } else {\n      title = title.toLowerCase();\n    }\n    return title + rows.join(\",\");\n  }\n}\n",
    )
    .unwrap();
    fs::write(
        dir.join("small_guard.ts"),
        "export function payload(data: string[]) {\n  if (data.length === 0) return null;\n  return data;\n}\n",
    )
    .unwrap();

    let semantic = run(&[
        "scan",
        dir.to_str().unwrap(),
        "--mode",
        "semantic",
        "--min-lines",
        "1",
        "--min-size",
        "1",
        "--format",
        "json",
        "--top",
        "0",
    ]);
    let semantic_json = scan_json(&semantic);
    let families = scan_families(&semantic_json);
    let text = semantic_json.to_string();
    assert!(
        text.contains("large_branch.ts") && text.contains("small_guard.ts"),
        "semantic mode should still report the strict guard-clause clone: {semantic}"
    );
    assert!(
        families.iter().all(|family| {
            let Some(locations) = family["locations"].as_array() else {
                return true;
            };
            locations.iter().all(|loc| {
                let Some(file) = loc["file"].as_str() else {
                    return true;
                };
                !file.ends_with("large_branch.ts")
                    || (loc["start_line"] == 2 && loc["end_line"] == 4)
            })
        }),
        "semantic mode must not report the flattened guard as the whole if/else span: {semantic}"
    );

    let _ = fs::remove_dir_all(&dir);
}

#[test]
fn scan_mode_semantic_preserves_js_typeof_operator() {
    let dir = std::env::temp_dir().join(format!("nose_typeof_semantic_{}", std::process::id()));
    let _ = fs::remove_dir_all(&dir);
    fs::create_dir_all(&dir).unwrap();
    fs::write(
        dir.join("typeof_a.ts"),
        "export function isString(value: unknown) {\n    return typeof value === \"string\";\n}\n",
    )
    .unwrap();
    fs::write(
        dir.join("typeof_b.ts"),
        "export function acceptsString(input: unknown) {\n    return typeof input === \"string\";\n}\n",
    )
    .unwrap();
    fs::write(
        dir.join("plain_equality_negative.ts"),
        "export function equalsString(value: unknown) {\n    return value === \"string\";\n}\n",
    )
    .unwrap();
    fs::write(
        dir.join("void_a.ts"),
        "export function eraseValue(value: unknown) {\n    return void value;\n}\n",
    )
    .unwrap();
    fs::write(
        dir.join("void_b.ts"),
        "export function eraseInput(input: unknown) {\n    return void input;\n}\n",
    )
    .unwrap();

    let semantic = run(&[
        "scan",
        dir.to_str().unwrap(),
        "--mode",
        "semantic",
        "--min-lines",
        "1",
        "--min-size",
        "1",
        "--format",
        "json",
        "--top",
        "0",
    ]);
    let semantic_json = scan_json(&semantic);
    let semantic_families = scan_families(&semantic_json);
    assert_eq!(
        semantic_families.len(),
        1,
        "semantic mode should report only the identical typeof guard: {semantic}"
    );
    let semantic_text = semantic_json.to_string();
    assert!(
        semantic_text.contains("typeof_a.ts")
            && semantic_text.contains("typeof_b.ts")
            && !semantic_text.contains("plain_equality_negative.ts")
            && !semantic_text.contains("void_a.ts")
            && !semantic_text.contains("void_b.ts"),
        "semantic mode must preserve typeof and reject unproved unary JS operators: {semantic}"
    );

    let _ = fs::remove_dir_all(&dir);
}

#[test]
fn scan_mode_semantic_allows_safe_uninterpreted_calls() {
    let dir = std::env::temp_dir().join(format!("nose_uninterpreted_call_{}", std::process::id()));
    let _ = fs::remove_dir_all(&dir);
    fs::create_dir_all(&dir).unwrap();
    fs::write(
        dir.join("call_a.py"),
        "def report(helper, value):\n    adjusted = value + 1\n    return helper(adjusted)\n",
    )
    .unwrap();
    fs::write(
        dir.join("call_b.py"),
        "def build(callback, input):\n    adjusted = input + 1\n    return callback(adjusted)\n",
    )
    .unwrap();
    fs::write(
        dir.join("call_negative.py"),
        "def other(value, callback):\n    adjusted = value + 1\n    return callback(adjusted)\n",
    )
    .unwrap();

    let semantic = run(&[
        "scan",
        dir.to_str().unwrap(),
        "--mode",
        "semantic",
        "--min-lines",
        "3",
        "--min-size",
        "1",
        "--format",
        "json",
        "--top",
        "0",
    ]);
    let semantic_json = scan_json(&semantic);
    let semantic_families = scan_families(&semantic_json);
    assert_eq!(
        semantic_families.len(),
        1,
        "semantic mode should report only same helper-call identity: {semantic}"
    );
    let semantic_text = semantic_json.to_string();
    assert!(
        semantic_text.contains("call_a.py")
            && semantic_text.contains("call_b.py")
            && !semantic_text.contains("call_negative.py"),
        "semantic mode must preserve uninterpreted callee identity: {semantic}"
    );

    let _ = fs::remove_dir_all(&dir);
}

#[test]
fn scan_mode_semantic_allows_safe_uninterpreted_method_calls() {
    let dir =
        std::env::temp_dir().join(format!("nose_uninterpreted_method_{}", std::process::id()));
    let _ = fs::remove_dir_all(&dir);
    fs::create_dir_all(&dir).unwrap();
    fs::write(
        dir.join("method_a.js"),
        "function normalize(value) { return value; }\nexport function assertLine(t, stderr) {\n    const line = normalize(stderr);\n    t.is(line, \"ok\");\n}\n",
    )
    .unwrap();
    fs::write(
        dir.join("method_b.js"),
        "function normalize(value) { return value; }\nexport function checkLine(assertion, output) {\n    const line = normalize(output);\n    assertion.is(line, \"ok\");\n}\n",
    )
    .unwrap();
    fs::write(
        dir.join("method_negative.js"),
        "function normalize(value) { return value; }\nexport function assertDeep(t, stderr) {\n    const line = normalize(stderr);\n    t.deepEqual(line, \"ok\");\n}\n",
    )
    .unwrap();

    let semantic = run(&[
        "scan",
        dir.to_str().unwrap(),
        "--mode",
        "semantic",
        "--min-lines",
        "3",
        "--min-size",
        "1",
        "--format",
        "json",
        "--top",
        "0",
    ]);
    let semantic_json = scan_json(&semantic);
    let semantic_families = scan_families(&semantic_json);
    assert_eq!(
        semantic_families.len(),
        1,
        "semantic mode should report only same method-call identity: {semantic}"
    );
    let semantic_text = semantic_json.to_string();
    assert!(
        semantic_text.contains("method_a.js")
            && semantic_text.contains("method_b.js")
            && !semantic_text.contains("method_negative.js"),
        "semantic mode must preserve uninterpreted method identity: {semantic}"
    );

    let _ = fs::remove_dir_all(&dir);
}

#[test]
fn scan_mode_semantic_distinguishes_sequence_kinds() {
    let dir = std::env::temp_dir().join(format!("nose_seq_semantic_{}", std::process::id()));
    let _ = fs::remove_dir_all(&dir);
    fs::create_dir_all(&dir).unwrap();
    fs::write(
        dir.join("list_a.py"),
        "def pair(a, b):\n    return [a, b]\n",
    )
    .unwrap();
    fs::write(
        dir.join("list_b.py"),
        "def make_pair(x, y):\n    return [x, y]\n",
    )
    .unwrap();
    fs::write(
        dir.join("tuple.py"),
        "def tuple_pair(a, b):\n    return (a, b)\n",
    )
    .unwrap();

    let semantic = run(&[
        "scan",
        dir.to_str().unwrap(),
        "--mode",
        "semantic",
        "--min-lines",
        "1",
        "--min-size",
        "1",
        "--format",
        "json",
        "--top",
        "0",
    ]);
    let semantic_json = scan_json(&semantic);
    let semantic_families = scan_families(&semantic_json);
    assert_eq!(
        semantic_families.len(),
        1,
        "semantic mode should report only the same list-construction family: {semantic}"
    );
    let semantic_text = semantic_json.to_string();
    assert!(
        semantic_text.contains("list_a.py")
            && semantic_text.contains("list_b.py")
            && !semantic_text.contains("tuple.py"),
        "semantic mode must preserve list-vs-tuple sequence kind: {semantic}"
    );

    let _ = fs::remove_dir_all(&dir);
}

#[test]
fn scan_mode_semantic_allows_static_import_identity() {
    let dir = std::env::temp_dir().join(format!("nose_import_identity_{}", std::process::id()));
    let _ = fs::remove_dir_all(&dir);
    fs::create_dir_all(&dir).unwrap();
    fs::write(
        dir.join("import_a.py"),
        "from shared_math import unused_helper, helper\n\ndef report(value):\n    shifted = value + 1\n    return helper(shifted)\n",
    )
    .unwrap();
    fs::write(
        dir.join("import_b.py"),
        "from shared_math import unused_helper, helper as calc\n\ndef build(input):\n    shifted = input + 1\n    return calc(shifted)\n",
    )
    .unwrap();
    fs::write(
        dir.join("import_negative.py"),
        "from shared_math import unused_helper, other_helper as calc\n\ndef other(input):\n    shifted = input + 1\n    return calc(shifted)\n",
    )
    .unwrap();

    let semantic = run(&[
        "scan",
        dir.to_str().unwrap(),
        "--mode",
        "semantic",
        "--min-lines",
        "1",
        "--min-size",
        "1",
        "--format",
        "json",
        "--top",
        "0",
    ]);
    let semantic_json = scan_json(&semantic);
    let semantic_families = scan_families(&semantic_json);
    assert_eq!(
        semantic_families.len(),
        1,
        "semantic mode should report only same import coordinate: {semantic}"
    );
    let semantic_text = semantic_json.to_string();
    assert!(
        semantic_text.contains("import_a.py")
            && semantic_text.contains("import_b.py")
            && !semantic_text.contains("import_negative.py"),
        "semantic mode must preserve static import coordinates: {semantic}"
    );

    let _ = fs::remove_dir_all(&dir);
}

#[test]
fn scan_mode_semantic_allows_named_namespace_import_identity() {
    let dir = std::env::temp_dir().join(format!(
        "nose_import_namespace_member_{}",
        std::process::id()
    ));
    let _ = fs::remove_dir_all(&dir);
    fs::create_dir_all(&dir).unwrap();
    fs::write(
        dir.join("named.js"),
        "import { helper } from \"./shared-math\";\n\nfunction report(value) {\n  return helper(value + 1);\n}\n",
    )
    .unwrap();
    fs::write(
        dir.join("namespace.js"),
        "import * as mathOps from \"./shared-math\";\n\nfunction build(input) {\n  return mathOps.helper(input + 1);\n}\n",
    )
    .unwrap();
    fs::write(
        dir.join("typed_namespace.ts"),
        "import * as mathOps from \"./shared-math\";\n\nfunction build(input: number): number {\n  return mathOps.helper(input + 1);\n}\n",
    )
    .unwrap();
    fs::write(
        dir.join("wrong_member.js"),
        "import * as mathOps from \"./shared-math\";\n\nfunction other(input) {\n  return mathOps.otherHelper(input + 1);\n}\n",
    )
    .unwrap();

    let semantic = run(&[
        "scan",
        dir.to_str().unwrap(),
        "--mode",
        "semantic",
        "--min-lines",
        "1",
        "--min-size",
        "1",
        "--format",
        "json",
        "--top",
        "0",
    ]);
    let semantic_json: serde_json::Value =
        serde_json::from_str(&semantic).expect("semantic scan should emit JSON");
    let semantic_text = semantic_json.to_string();
    for expected in ["named.js", "namespace.js", "typed_namespace.ts"] {
        assert!(
            semantic_text.contains(expected),
            "semantic mode should include static import member positive {expected}: {semantic}"
        );
    }
    assert!(
        !semantic_text.contains("wrong_member.js"),
        "semantic mode must preserve imported member coordinate boundaries: {semantic}"
    );

    let _ = fs::remove_dir_all(&dir);
}

#[test]
fn scan_mode_semantic_allows_static_projection_identity() {
    let dir = std::env::temp_dir().join(format!("nose_projection_identity_{}", std::process::id()));
    let _ = fs::remove_dir_all(&dir);
    fs::create_dir_all(&dir).unwrap();
    fs::write(
        dir.join("projection_direct.js"),
        "function direct(record, value) {\n  return value + record.today;\n}\n",
    )
    .unwrap();
    fs::write(
        dir.join("projection_keyed.js"),
        "function keyed(row, amount) {\n  return amount + row['today'];\n}\n",
    )
    .unwrap();
    fs::write(
        dir.join("projection_destructured.js"),
        "function destructured(row, amount) {\n  const { today: selected } = row;\n  return amount + selected;\n}\n",
    )
    .unwrap();
    fs::write(
        dir.join("projection_rust_direct.rs"),
        "pub fn rust_direct(record: Reading, value: i32) -> i32 {\n    value + record.today\n}\n",
    )
    .unwrap();
    fs::write(
        dir.join("projection_rust_shorthand.rs"),
        "pub fn rust_shorthand(row: Reading, amount: i32) -> i32 {\n    let Reading { today, .. } = row;\n    amount + today\n}\n",
    )
    .unwrap();
    fs::write(
        dir.join("projection_negative.js"),
        "function negative(row, amount, key) {\n  return amount + row[key];\n}\n",
    )
    .unwrap();

    let semantic = run(&[
        "scan",
        dir.to_str().unwrap(),
        "--mode",
        "semantic",
        "--min-lines",
        "1",
        "--min-size",
        "1",
        "--format",
        "json",
        "--top",
        "0",
    ]);
    let semantic_json = scan_json(&semantic);
    let semantic_families = scan_families(&semantic_json);
    assert!(
        !semantic_families.is_empty(),
        "semantic mode should report static projection identities: {semantic}"
    );
    let semantic_text = semantic_json.to_string();
    for expected in [
        "projection_direct.js",
        "projection_keyed.js",
        "projection_destructured.js",
        "projection_rust_direct.rs",
        "projection_rust_shorthand.rs",
    ] {
        assert!(
            semantic_text.contains(expected),
            "semantic mode should include {expected}: {semantic}"
        );
    }
    assert!(
        !semantic_text.contains("projection_negative.js"),
        "semantic mode must not treat dynamic keys as fixed projections: {semantic}"
    );

    let _ = fs::remove_dir_all(&dir);
}

#[test]
fn scan_mode_semantic_distinguishes_nullish_from_truthy_defaults() {
    let dir = std::env::temp_dir().join(format!("nose_nullish_default_{}", std::process::id()));
    let _ = fs::remove_dir_all(&dir);
    fs::create_dir_all(&dir).unwrap();
    fs::write(
        dir.join("nullish_coalesce.js"),
        "function coalesce(value, fallback) {\n  return value ?? fallback;\n}\n",
    )
    .unwrap();
    fs::write(
        dir.join("nullish_ternary.js"),
        "function ternary(value, fallback) {\n  return value == null ? fallback : value;\n}\n",
    )
    .unwrap();
    fs::write(
        dir.join("nullish_guard.js"),
        "function guard(value, fallback) {\n  if (value == null) {\n    return fallback;\n  }\n  return value;\n}\n",
    )
    .unwrap();
    fs::write(
        dir.join("truthy_negative.js"),
        "function truthy(value, fallback) {\n  return value || fallback;\n}\n",
    )
    .unwrap();

    let semantic = run(&[
        "scan",
        dir.to_str().unwrap(),
        "--mode",
        "semantic",
        "--min-lines",
        "1",
        "--min-size",
        "1",
        "--format",
        "json",
        "--top",
        "0",
    ]);
    let semantic_json = scan_json(&semantic);
    let semantic_families = scan_families(&semantic_json);
    assert_eq!(
        semantic_families.len(),
        1,
        "semantic mode should report one nullish-default family: {semantic}"
    );
    let semantic_text = semantic_json.to_string();
    for expected in [
        "nullish_coalesce.js",
        "nullish_ternary.js",
        "nullish_guard.js",
    ] {
        assert!(
            semantic_text.contains(expected),
            "semantic mode should include {expected}: {semantic}"
        );
    }
    assert!(
        !semantic_text.contains("truthy_negative.js"),
        "semantic mode must not merge nullish and truthy defaults: {semantic}"
    );

    let _ = fs::remove_dir_all(&dir);
}

#[test]
fn scan_mode_semantic_proves_js_record_shape_guards() {
    let dir = std::env::temp_dir().join(format!("nose_record_guard_{}", std::process::id()));
    let _ = fs::remove_dir_all(&dir);
    fs::create_dir_all(&dir).unwrap();
    fs::write(
        dir.join("guard_direct.js"),
        "function direct(value) {\n  return typeof value === \"object\" && value !== null && !Array.isArray(value);\n}\n",
    )
    .unwrap();
    fs::write(
        dir.join("guard_reordered.js"),
        "function reordered(candidate) {\n  return !Array.isArray(candidate) && candidate !== null && typeof candidate === \"object\";\n}\n",
    )
    .unwrap();
    fs::write(
        dir.join("guard_truthy.js"),
        "function truthy(input) {\n  return Boolean(input) && typeof input === \"object\" && !Array.isArray(input);\n}\n",
    )
    .unwrap();
    fs::write(
        dir.join("boolean_shadowed_negative.js"),
        "function shadowed(Boolean, input) {\n  return Boolean(input) && typeof input === \"object\" && !Array.isArray(input);\n}\n",
    )
    .unwrap();
    fs::write(
        dir.join("array_shadowed_negative.js"),
        "function shadowed(Array, input) {\n  return typeof input === \"object\" && input !== null && !Array.isArray(input);\n}\n",
    )
    .unwrap();
    fs::write(
        dir.join("array_destructured_shadow_negative.js"),
        "function shadowed(scope, input) {\n  const { Array } = scope;\n  return typeof input === \"object\" && input !== null && !Array.isArray(input);\n}\n",
    )
    .unwrap();
    fs::write(
        dir.join("array_allowed_negative.js"),
        "function arrayAllowed(value) {\n  return typeof value === \"object\" && value !== null;\n}\n",
    )
    .unwrap();
    fs::write(
        dir.join("null_allowed_negative.js"),
        "function nullAllowed(value) {\n  return typeof value === \"object\" && !Array.isArray(value);\n}\n",
    )
    .unwrap();
    fs::write(
        dir.join("wrong_typeof_literal_negative.js"),
        "function wrongLiteral(value) {\n  return typeof value === \"ob ject\" && value !== null && !Array.isArray(value);\n}\n",
    )
    .unwrap();
    fs::write(
        dir.join("typeof_identifier_negative.js"),
        "function wrongIdentifier(value) {\n  return typeofvalue === \"object\" && value !== null && !Array.isArray(value);\n}\n",
    )
    .unwrap();

    let semantic = run(&[
        "scan",
        dir.to_str().unwrap(),
        "--mode",
        "semantic",
        "--min-lines",
        "1",
        "--min-size",
        "1",
        "--format",
        "json",
        "--top",
        "0",
    ]);
    let semantic_json = scan_json(&semantic);
    let semantic_families = scan_families(&semantic_json);
    assert_eq!(
        semantic_families.len(),
        1,
        "semantic mode should report one proved record-shape guard family: {semantic}"
    );
    let semantic_text = semantic_json.to_string();
    for expected in ["guard_direct.js", "guard_reordered.js", "guard_truthy.js"] {
        assert!(
            semantic_text.contains(expected),
            "semantic mode should include {expected}: {semantic}"
        );
    }
    for unexpected in [
        "array_allowed_negative.js",
        "array_destructured_shadow_negative.js",
        "array_shadowed_negative.js",
        "boolean_shadowed_negative.js",
        "null_allowed_negative.js",
        "typeof_identifier_negative.js",
        "wrong_typeof_literal_negative.js",
    ] {
        assert!(
            !semantic_text.contains(unexpected),
            "semantic mode must reject invalid or incomplete record-shape guards: {semantic}"
        );
    }

    let _ = fs::remove_dir_all(&dir);
}

#[test]
fn scan_mode_semantic_proves_js_own_property_guards() {
    let dir = std::env::temp_dir().join(format!("nose_own_property_{}", std::process::id()));
    let _ = fs::remove_dir_all(&dir);
    fs::create_dir_all(&dir).unwrap();
    fs::write(
        dir.join("has_own.js"),
        "function hasOwn(value) {\n  return Object.hasOwn(value, \"ready\");\n}\n",
    )
    .unwrap();
    fs::write(
        dir.join("has_own_call.js"),
        "function hasOwnCall(candidate) {\n  return Object.prototype.hasOwnProperty.call(candidate, \"ready\");\n}\n",
    )
    .unwrap();
    fs::write(
        dir.join("in_operator_negative.js"),
        "function inOperator(value) {\n  return \"ready\" in value;\n}\n",
    )
    .unwrap();
    fs::write(
        dir.join("direct_method_negative.js"),
        "function directMethod(value) {\n  return value.hasOwnProperty(\"ready\");\n}\n",
    )
    .unwrap();
    fs::write(
        dir.join("different_key_negative.js"),
        "function differentKey(value) {\n  return Object.hasOwn(value, \"enabled\");\n}\n",
    )
    .unwrap();
    fs::write(
        dir.join("shadowed_object_negative.js"),
        "function shadowedObject(Object, value) {\n  return Object.hasOwn(value, \"ready\");\n}\n",
    )
    .unwrap();
    fs::write(
        dir.join("shadowed_global_object_negative.js"),
        "const Object = { hasOwn() { return false; } };\nfunction shadowedGlobal(value) {\n  return Object.hasOwn(value, \"ready\");\n}\n",
    )
    .unwrap();
    fs::write(
        dir.join("shadowed_object_call_negative.js"),
        "function shadowedObjectCall(Object, value) {\n  return Object.prototype.hasOwnProperty.call(value, \"ready\");\n}\n",
    )
    .unwrap();

    let semantic = run(&[
        "scan",
        dir.to_str().unwrap(),
        "--mode",
        "semantic",
        "--min-lines",
        "1",
        "--min-size",
        "1",
        "--format",
        "json",
        "--top",
        "0",
    ]);
    let semantic_json = scan_json(&semantic);
    let semantic_families = scan_families(&semantic_json);
    assert_eq!(
        semantic_families.len(),
        1,
        "semantic mode should report one proved own-property guard family: {semantic}"
    );
    let semantic_text = semantic_json.to_string();
    for expected in ["has_own.js", "has_own_call.js"] {
        assert!(
            semantic_text.contains(expected),
            "semantic mode should include {expected}: {semantic}"
        );
    }
    for unexpected in [
        "in_operator_negative.js",
        "direct_method_negative.js",
        "different_key_negative.js",
        "shadowed_object_negative.js",
        "shadowed_object_call_negative.js",
        "shadowed_global_object_negative.js",
    ] {
        assert!(
            !semantic_text.contains(unexpected),
            "semantic mode must reject non-own or different-key property guards: {semantic}"
        );
    }

    let _ = fs::remove_dir_all(&dir);
}

#[test]
fn scan_mode_semantic_converges_cross_language_list_literals() {
    let dir = std::env::temp_dir().join(format!("nose_list_cross_{}", std::process::id()));
    let _ = fs::remove_dir_all(&dir);
    fs::create_dir_all(&dir).unwrap();
    fs::write(
        dir.join("pair.js"),
        "export function pair(a, b) {\n    return [a, b];\n}\n",
    )
    .unwrap();
    fs::write(
        dir.join("pair.py"),
        "def make_pair(x, y):\n    return [x, y]\n",
    )
    .unwrap();
    fs::write(
        dir.join("pair.rb"),
        "def build_pair(first, second)\n  [first, second]\nend\n",
    )
    .unwrap();
    fs::write(
        dir.join("tuple_negative.py"),
        "def tuple_pair(a, b):\n    return (a, b)\n",
    )
    .unwrap();

    let semantic = run(&[
        "scan",
        dir.to_str().unwrap(),
        "--mode",
        "semantic",
        "--min-lines",
        "1",
        "--min-size",
        "1",
        "--format",
        "json",
        "--top",
        "0",
    ]);
    let semantic_json = scan_json(&semantic);
    let semantic_families = scan_families(&semantic_json);
    assert_eq!(
        semantic_families.len(),
        1,
        "semantic mode should report one cross-language list literal family: {semantic}"
    );
    let semantic_text = semantic_json.to_string();
    assert!(
        semantic_text.contains("pair.js")
            && semantic_text.contains("pair.py")
            && semantic_text.contains("pair.rb")
            && !semantic_text.contains("tuple_negative.py"),
        "semantic mode must converge list-like literals without merging tuples: {semantic}"
    );

    let _ = fs::remove_dir_all(&dir);
}

#[test]
fn scan_mode_semantic_preserves_js_object_keys() {
    let dir = std::env::temp_dir().join(format!("nose_object_semantic_{}", std::process::id()));
    let _ = fs::remove_dir_all(&dir);
    fs::create_dir_all(&dir).unwrap();
    fs::write(
        dir.join("object_a.ts"),
        "export function example(command: string, description: string) {\n    return { command, description };\n}\n",
    )
    .unwrap();
    fs::write(
        dir.join("object_b.ts"),
        "export function makeExample(cmd: string, desc: string) {\n    return { command: cmd, description: desc };\n}\n",
    )
    .unwrap();
    fs::write(
        dir.join("object_key_negative.ts"),
        "export function makeParam(name: string, description: string) {\n    return { name, description };\n}\n",
    )
    .unwrap();
    fs::write(
        dir.join("object_computed_a.ts"),
        "const KEY = \"command\";\nexport function computed(command: string, description: string) {\n    return { [KEY]: command, description };\n}\n",
    )
    .unwrap();
    fs::write(
        dir.join("object_computed_b.ts"),
        "const FIELD = \"command\";\nexport function computedOther(cmd: string, desc: string) {\n    return { [FIELD]: cmd, description: desc };\n}\n",
    )
    .unwrap();

    let semantic = run(&[
        "scan",
        dir.to_str().unwrap(),
        "--mode",
        "semantic",
        "--min-lines",
        "1",
        "--min-size",
        "1",
        "--format",
        "json",
        "--top",
        "0",
    ]);
    let semantic_json = scan_json(&semantic);
    let semantic_families = scan_families(&semantic_json);
    assert_eq!(
        semantic_families.len(),
        1,
        "semantic mode should report only same-key object construction: {semantic}"
    );
    let semantic_text = semantic_json.to_string();
    assert!(
        semantic_text.contains("object_a.ts")
            && semantic_text.contains("object_b.ts")
            && !semantic_text.contains("object_key_negative.ts")
            && !semantic_text.contains("object_computed_a.ts")
            && !semantic_text.contains("object_computed_b.ts"),
        "semantic mode must preserve static object keys and reject computed-key object contracts: {semantic}"
    );

    let _ = fs::remove_dir_all(&dir);
}

#[test]
fn scan_mode_semantic_converges_cross_language_map_literals() {
    let dir = std::env::temp_dir().join(format!("nose_map_cross_{}", std::process::id()));
    let _ = fs::remove_dir_all(&dir);
    fs::create_dir_all(&dir).unwrap();
    fs::write(
        dir.join("map.ts"),
        "export function example(command: string, description: string) {\n    return { command, description };\n}\n",
    )
    .unwrap();
    fs::write(
        dir.join("map.py"),
        "def make_example(cmd, desc):\n    return {\"command\": cmd, \"description\": desc}\n",
    )
    .unwrap();
    fs::write(
        dir.join("map.rb"),
        "def build_example(command, description)\n  { command: command, description: description }\nend\n",
    )
    .unwrap();
    fs::write(
        dir.join("map_key_negative.ts"),
        "export function makeParam(name: string, description: string) {\n    return { name, description };\n}\n",
    )
    .unwrap();

    let semantic = run(&[
        "scan",
        dir.to_str().unwrap(),
        "--mode",
        "semantic",
        "--min-lines",
        "1",
        "--min-size",
        "1",
        "--format",
        "json",
        "--top",
        "0",
    ]);
    let semantic_json = scan_json(&semantic);
    let semantic_families = scan_families(&semantic_json);
    assert_eq!(
        semantic_families.len(),
        1,
        "semantic mode should report one cross-language map literal family: {semantic}"
    );
    let semantic_text = semantic_json.to_string();
    assert!(
        semantic_text.contains("map.ts")
            && semantic_text.contains("map.py")
            && semantic_text.contains("map.rb")
            && !semantic_text.contains("map_key_negative.ts"),
        "semantic mode must converge map-like literals without dropping key identity: {semantic}"
    );

    let _ = fs::remove_dir_all(&dir);
}

#[test]
fn scan_mode_semantic_captures_module_literal_bindings() {
    let dir = std::env::temp_dir().join(format!("nose_module_const_{}", std::process::id()));
    let _ = fs::remove_dir_all(&dir);
    fs::create_dir_all(&dir).unwrap();
    fs::write(
        dir.join("locale_a.ts"),
        "const labels = { today: \"today\", tomorrow: \"tomorrow\" };\nexport function label(token: string) {\n    return labels[token];\n}\n",
    )
    .unwrap();
    fs::write(
        dir.join("locale_b.ts"),
        "const labels = { today: \"heute\", tomorrow: \"morgen\" };\nexport function label(token: string) {\n    return labels[token];\n}\n",
    )
    .unwrap();
    fs::write(
        dir.join("locale_a_copy.ts"),
        "const labels = { today: \"today\", tomorrow: \"tomorrow\" };\nexport function relativeLabel(key: string) {\n    return labels[key];\n}\n",
    )
    .unwrap();
    fs::write(
        dir.join("locale_mutated.ts"),
        "let labels = { today: \"today\", tomorrow: \"tomorrow\" };\nlabels = { today: \"heute\", tomorrow: \"morgen\" };\nexport function mutatedLabel(key: string) {\n    return labels[key];\n}\n",
    )
    .unwrap();

    let semantic = run(&[
        "scan",
        dir.to_str().unwrap(),
        "--mode",
        "semantic",
        "--min-lines",
        "1",
        "--min-size",
        "1",
        "--format",
        "json",
        "--top",
        "0",
    ]);
    let semantic_json = scan_json(&semantic);
    let semantic_families = scan_families(&semantic_json);
    assert_eq!(
        semantic_families.len(),
        1,
        "semantic mode should report only same module-literal binding behavior: {semantic}"
    );
    let semantic_text = semantic_json.to_string();
    assert!(
        semantic_text.contains("locale_a.ts")
            && semantic_text.contains("locale_a_copy.ts")
            && !semantic_text.contains("locale_b.ts")
            && !semantic_text.contains("locale_mutated.ts"),
        "semantic mode must include captured module literal values: {semantic}"
    );

    let _ = fs::remove_dir_all(&dir);
}

#[test]
fn scan_mode_semantic_preserves_python_dict_keys() {
    let dir = std::env::temp_dir().join(format!("nose_dict_semantic_{}", std::process::id()));
    let _ = fs::remove_dir_all(&dir);
    fs::create_dir_all(&dir).unwrap();
    fs::write(
        dir.join("dict_a.py"),
        "def example(command, description):\n    return {\"command\": command, \"description\": description}\n",
    )
    .unwrap();
    fs::write(
        dir.join("dict_b.py"),
        "def make_example(cmd, desc):\n    return {\"command\": cmd, \"description\": desc}\n",
    )
    .unwrap();
    fs::write(
        dir.join("dict_key_negative.py"),
        "def make_param(name, description):\n    return {\"name\": name, \"description\": description}\n",
    )
    .unwrap();
    fs::write(
        dir.join("dict_spread_a.py"),
        "def with_spread(base, command):\n    return {**base, \"command\": command}\n",
    )
    .unwrap();
    fs::write(
        dir.join("dict_spread_b.py"),
        "def copy_spread(other, cmd):\n    return {**other, \"command\": cmd}\n",
    )
    .unwrap();

    let semantic = run(&[
        "scan",
        dir.to_str().unwrap(),
        "--mode",
        "semantic",
        "--min-lines",
        "1",
        "--min-size",
        "1",
        "--format",
        "json",
        "--top",
        "0",
    ]);
    let semantic_json = scan_json(&semantic);
    let semantic_families = scan_families(&semantic_json);
    assert_eq!(
        semantic_families.len(),
        1,
        "semantic mode should report only same-key dict construction: {semantic}"
    );
    let semantic_text = semantic_json.to_string();
    assert!(
        semantic_text.contains("dict_a.py")
            && semantic_text.contains("dict_b.py")
            && !semantic_text.contains("dict_key_negative.py")
            && !semantic_text.contains("dict_spread_a.py")
            && !semantic_text.contains("dict_spread_b.py"),
        "semantic mode must preserve dict keys and reject unproved unpacking: {semantic}"
    );

    let _ = fs::remove_dir_all(&dir);
}

#[test]
fn scan_mode_semantic_preserves_ruby_hash_keys() {
    let dir = std::env::temp_dir().join(format!("nose_hash_semantic_{}", std::process::id()));
    let _ = fs::remove_dir_all(&dir);
    fs::create_dir_all(&dir).unwrap();
    fs::write(
        dir.join("hash_a.rb"),
        "def example(command, description)\n  { command: command, description: description }\nend\n",
    )
    .unwrap();
    fs::write(
        dir.join("hash_b.rb"),
        "def make_example(cmd, desc)\n  { command: cmd, description: desc }\nend\n",
    )
    .unwrap();
    fs::write(
        dir.join("hash_key_negative.rb"),
        "def make_param(name, description)\n  { name: name, description: description }\nend\n",
    )
    .unwrap();
    fs::write(
        dir.join("hash_splat_a.rb"),
        "def with_splat(base, command)\n  { **base, command: command }\nend\n",
    )
    .unwrap();
    fs::write(
        dir.join("hash_splat_b.rb"),
        "def copy_splat(other, cmd)\n  { **other, command: cmd }\nend\n",
    )
    .unwrap();

    let semantic = run(&[
        "scan",
        dir.to_str().unwrap(),
        "--mode",
        "semantic",
        "--min-lines",
        "1",
        "--min-size",
        "1",
        "--format",
        "json",
        "--top",
        "0",
    ]);
    let semantic_json = scan_json(&semantic);
    let semantic_families = scan_families(&semantic_json);
    assert_eq!(
        semantic_families.len(),
        1,
        "semantic mode should report only same-key hash construction: {semantic}"
    );
    let semantic_text = semantic_json.to_string();
    assert!(
        semantic_text.contains("hash_a.rb")
            && semantic_text.contains("hash_b.rb")
            && !semantic_text.contains("hash_key_negative.rb")
            && !semantic_text.contains("hash_splat_a.rb")
            && !semantic_text.contains("hash_splat_b.rb"),
        "semantic mode must preserve hash keys and reject unproved splats: {semantic}"
    );

    let _ = fs::remove_dir_all(&dir);
}
