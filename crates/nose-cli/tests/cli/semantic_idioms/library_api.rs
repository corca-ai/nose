use super::*;

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

    let semantic = query_min_json(&dir, "semantic");
    let semantic_json = query_json(&semantic);
    let semantic_families = query_families(&semantic_json);
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

#[test]
fn query_mode_semantic_proves_extreme_type4_idioms() {
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

    let semantic = query_min_json(&dir, "semantic");
    let semantic_json = query_json(&semantic);
    let semantic_families = query_families(&semantic_json);
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
fn query_mode_semantic_proves_collection_empty_checks() {
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

    let semantic = query_min_json(&dir, "semantic");
    let semantic_json = query_json(&semantic);
    let semantic_families = query_families(&semantic_json);
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
fn query_mode_semantic_proves_string_prefix_checks() {
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

    let semantic = query_min_json(&dir, "semantic");
    let semantic_json = query_json(&semantic);
    let semantic_families = query_families(&semantic_json);
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
fn cli_normalized_il_proves_go_strings_contains_namespace_calls() {
    let dir = std::env::temp_dir().join(format!("nose_go_strings_contains_{}", std::process::id()));
    let _ = fs::remove_dir_all(&dir);
    fs::create_dir_all(&dir).unwrap();
    fs::write(
        dir.join("contains_std.go"),
        "package p\n\nimport \"strings\"\n\nfunc ContainsStd(value string) bool {\n    return strings.Contains(value, \"pre\")\n}\n",
    )
    .unwrap();
    fs::write(
        dir.join("contains_alias.go"),
        "package p\n\nimport str \"strings\"\n\nfunc ContainsAlias(value string) bool {\n    return str.Contains(value, \"pre\")\n}\n",
    )
    .unwrap();
    fs::write(
        dir.join("contains_other_needle.go"),
        "package p\n\nimport \"strings\"\n\nfunc ContainsOtherNeedle(value string) bool {\n    return strings.Contains(value, \"alt\")\n}\n",
    )
    .unwrap();
    fs::write(
        dir.join("contains_slice.go"),
        "package p\n\nimport \"slices\"\n\nfunc ContainsSlice(xs []string) bool {\n    return slices.Contains(xs, \"pre\")\n}\n",
    )
    .unwrap();
    fs::write(
        dir.join("contains_shadow.go"),
        "package p\n\ntype matcher struct{}\n\nfunc (m matcher) Contains(value string, needle string) bool { return true }\n\nfunc ContainsShadow(strings matcher, value string) bool {\n    return strings.Contains(value, \"pre\")\n}\n",
    )
    .unwrap();

    let normalized = |name: &str| {
        run_raw(&[
            "il",
            dir.join(name).to_str().unwrap(),
            "--normalized",
            "--format",
            "sexpr",
        ])
    };
    let std = normalized("contains_std.go");
    let alias = normalized("contains_alias.go");
    assert_eq!(
        std, alias,
        "Go strings.Contains should canonicalize import aliases through namespace evidence"
    );
    assert!(
        std.contains("@StringContains"),
        "strings.Contains should lower to substring membership, not collection membership: {std}"
    );

    let other_needle = normalized("contains_other_needle.go");
    assert_ne!(
        std, other_needle,
        "different substring needles must remain distinct"
    );
    let slice = normalized("contains_slice.go");
    assert!(
        slice.contains("@Contains") && !slice.contains("@StringContains"),
        "slices.Contains should stay collection membership: {slice}"
    );
    let shadow = normalized("contains_shadow.go");
    assert!(
        !shadow.contains("@StringContains") && !shadow.contains("@Contains"),
        "a local value named strings must not prove any stdlib Contains semantic: {shadow}"
    );

    let _ = fs::remove_dir_all(&dir);
}

#[test]
fn query_mode_semantic_proves_rust_integer_methods() {
    let dir =
        std::env::temp_dir().join(format!("nose_rust_numeric_methods_{}", std::process::id()));
    let _ = fs::remove_dir_all(&dir);
    fs::create_dir_all(&dir).unwrap();
    fs::write(
        dir.join("abs_conditional.py"),
        "def magnitude(value: int, other: int):\n    magnitude = value if value >= 0 else -value\n    return magnitude + other\n",
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

    let semantic = query_min_json(&dir, "semantic");
    let semantic_json = query_json(&semantic);
    let semantic_families = query_families(&semantic_json);
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
