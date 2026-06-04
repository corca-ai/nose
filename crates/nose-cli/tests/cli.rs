//! End-to-end CLI tests: run the built `nose` binary against a temp project and
//! check the user-visible behavior (discovery, `scan` report, `--exclude`).

use std::fs;
use std::path::PathBuf;
use std::process::{Command, Stdio};

fn bin() -> &'static str {
    env!("CARGO_BIN_EXE_nose")
}

/// Write a small project (a 3-copy clone family + a decoy) into a unique temp dir.
fn make_project(tag: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("nose_cli_{tag}_{}", std::process::id()));
    let _ = fs::remove_dir_all(&dir);
    let body = |acc: &str, it: &str| {
        format!("def f(items):\n    {acc} = 0\n    for {it} in items:\n        if {it} > 0:\n            {acc} = {acc} + {it} * {it}\n    return {acc}\n")
    };
    for (sub, src) in [
        ("a", body("total", "x")),
        ("b", body("acc", "v")),
        ("tests", body("s", "n")),
    ] {
        let d = dir.join(sub);
        fs::create_dir_all(&d).unwrap();
        fs::write(d.join("f.py"), src).unwrap();
    }
    let d = dir.join("c");
    fs::create_dir_all(&d).unwrap();
    fs::write(
        d.join("decoy.py"),
        "def greet(n):\n    m = 'hi ' + n\n    print(m)\n    print(n)\n    return m\n",
    )
    .unwrap();
    dir
}

fn make_mode_project(tag: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("nose_modes_{tag}_{}", std::process::id()));
    let _ = fs::remove_dir_all(&dir);
    fs::create_dir_all(&dir).unwrap();

    fs::write(
        dir.join("renamed_a.py"),
        "def total(items):\n    total = 0\n    for item in items:\n        if item > 0:\n            total = total + item * item\n    return total\n",
    )
    .unwrap();
    fs::write(
        dir.join("renamed_b.py"),
        "def score(values):\n    acc = 0\n    for value in values:\n        if value > 0:\n            acc = acc + value * value\n    return acc\n",
    )
    .unwrap();

    let copied = "def copied(events):\n    out = []\n    for e in events:\n        if e.kind == 1:\n            out.append(e.payload)\n            record(e.id, e.kind)\n    return out\n";
    fs::write(dir.join("copy_a.py"), copied).unwrap();
    fs::write(dir.join("copy_b.py"), copied).unwrap();

    dir
}

fn run(args: &[&str]) -> String {
    let out = Command::new(bin()).args(args).output().expect("run nose");
    assert!(
        out.status.success(),
        "nose exited non-zero: {:?}",
        out.status
    );
    String::from_utf8(out.stdout).unwrap()
}

#[test]
fn scan_mode_syntax_reports_copy_paste_only() {
    let dir = make_mode_project("syntax");
    let p = dir.to_str().unwrap();
    let out = run(&[
        "scan",
        p,
        "--mode",
        "syntax",
        "--min-tokens",
        "12",
        "--format",
        "json",
    ]);
    assert!(
        out.contains("copy_a.py"),
        "syntax reports exact copies: {out}"
    );
    assert!(
        out.contains("copy_b.py"),
        "syntax reports exact copies: {out}"
    );
    assert!(
        !out.contains("renamed_a.py") && !out.contains("renamed_b.py"),
        "syntax must not report semantic renamed clones: {out}"
    );
    let _ = fs::remove_dir_all(&dir);
}

#[test]
fn scan_mode_syntax_min_tokens_controls_copy_paste_floor() {
    let dir = make_mode_project("syntax_floor");
    let p = dir.to_str().unwrap();
    let out = run(&[
        "scan",
        p,
        "--mode",
        "syntax",
        "--min-tokens",
        "80",
        "--format",
        "json",
    ]);
    assert_eq!(
        out.trim(),
        "[]",
        "a high syntax token floor suppresses the short copy-paste run: {out}"
    );
    let _ = fs::remove_dir_all(&dir);
}

#[test]
fn scan_mode_semantic_keeps_renamed_exact_clone_candidates() {
    let dir = make_mode_project("semantic_mode");
    let p = dir.to_str().unwrap();
    let out = run(&[
        "scan",
        p,
        "--mode",
        "semantic",
        "--min-tokens",
        "12",
        "--format",
        "json",
    ]);
    assert!(
        out.contains("renamed_a.py") && out.contains("renamed_b.py"),
        "semantic mode keeps exact value-fingerprint candidates: {out}"
    );
    let _ = fs::remove_dir_all(&dir);
}

#[test]
fn scan_mode_semantic_rejects_unproved_regex_predicate_matches() {
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

    let semantic = run(&[
        "scan",
        dir.to_str().unwrap(),
        "--mode",
        "semantic",
        "--min-lines",
        "1",
        "--min-tokens",
        "1",
        "--format",
        "json",
        "--top",
        "0",
    ]);
    let semantic_json: serde_json::Value =
        serde_json::from_str(&semantic).expect("semantic scan should emit JSON");
    let semantic_families = semantic_json.as_array().expect("semantic JSON array");
    assert_eq!(
        semantic_families.len(),
        1,
        "semantic mode should report only the same-regex exact family: {semantic}"
    );
    let semantic_text = semantic_json.to_string();
    assert!(
        semantic_text.contains("dot-only.ts")
            && semantic_text.contains("dot-only-copy.ts")
            && !semantic_text.contains("markdown-link.ts"),
        "semantic mode must distinguish regex pattern semantics: {semantic}"
    );

    let near = run(&[
        "scan",
        dir.to_str().unwrap(),
        "--mode",
        "near",
        "--threshold",
        "0.5",
        "--min-lines",
        "1",
        "--min-tokens",
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
        "--min-tokens",
        "1",
        "--format",
        "json",
        "--top",
        "0",
    ]);
    let semantic_json: serde_json::Value =
        serde_json::from_str(&semantic).expect("semantic scan should emit JSON");
    let semantic_families = semantic_json.as_array().expect("semantic JSON array");
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
        "--min-tokens",
        "1",
        "--format",
        "json",
        "--top",
        "0",
    ]);
    let semantic_json: serde_json::Value =
        serde_json::from_str(&semantic).expect("semantic scan should emit JSON");
    let semantic_families = semantic_json.as_array().expect("semantic JSON array");
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
            && semantic_text.contains("ruby_length.rb")
            && semantic_text.contains("ruby_named.rb")
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
        "def prefix(value, other):\n    return value.startswith(\"pre\")\n",
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
        "--min-tokens",
        "1",
        "--format",
        "json",
        "--top",
        "0",
    ]);
    let semantic_json: serde_json::Value =
        serde_json::from_str(&semantic).expect("semantic scan should emit JSON");
    let semantic_families = semantic_json.as_array().expect("semantic JSON array");
    assert!(
        !semantic_families.is_empty(),
        "semantic mode should report string prefix families: {semantic}"
    );
    let semantic_text = semantic_json.to_string();
    for expected in [
        "prefix.py",
        "prefix.js",
        "prefix.ts",
        "prefix.go",
        "prefix.rs",
        "prefix.java",
        "prefix.rb",
    ] {
        assert!(
            semantic_text.contains(expected),
            "semantic mode should include {expected}: {semantic}"
        );
    }
    for unexpected in [
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
fn scan_mode_semantic_proves_rust_numeric_methods() {
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
        "--min-tokens",
        "1",
        "--format",
        "json",
        "--top",
        "0",
    ]);
    let semantic_json: serde_json::Value =
        serde_json::from_str(&semantic).expect("semantic scan should emit JSON");
    let semantic_families = semantic_json.as_array().expect("semantic JSON array");
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
                panic!("semantic mode should report numeric method family: {semantic}")
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
                "semantic mode must preserve Rust numeric method boundaries: {semantic}"
            );
        }
    }

    let _ = fs::remove_dir_all(&dir);
}

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
        dir.join("array_filter_length_absence.js"),
        "function arrayFilterLengthAbsence(value, other) {\n  return [\"red\", \"blue\"].filter((item) => item === value).length === 0;\n}\n",
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
        dir.join("go_slices_mutated.go"),
        "package p\n\nimport \"slices\"\n\nvar values = append([]string{\"red\", \"blue\"}, \"green\")\n\nfunc SlicesMutated(value string, other string) bool {\n    return slices.Contains(values, value)\n}\n",
    )
    .unwrap();
    fs::write(
        dir.join("go_slices_unimported.go"),
        "package p\n\ntype fakeSlices struct{}\n\nfunc (fakeSlices) Contains(values []string, value string) bool {\n    return false\n}\n\nvar slices fakeSlices\nvar values = []string{\"red\", \"blue\"}\n\nfunc SlicesUnimported(value string, other string) bool {\n    return slices.Contains(values, value)\n}\n",
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

    let semantic = run(&[
        "scan",
        dir.to_str().unwrap(),
        "--mode",
        "semantic",
        "--min-lines",
        "1",
        "--min-tokens",
        "1",
        "--format",
        "json",
        "--top",
        "0",
    ]);
    let semantic_json: serde_json::Value =
        serde_json::from_str(&semantic).expect("semantic scan should emit JSON");
    let semantic_families = semantic_json.as_array().expect("semantic JSON array");
    assert!(
        !semantic_families.is_empty(),
        "semantic mode should report literal membership families: {semantic}"
    );
    let semantic_text = semantic_json.to_string();
    for expected in [
        "membership.py",
        "membership.js",
        "membership.ts",
        "membership.go",
        "membership.rs",
        "membership.rb",
        "module_set.js",
        "module_set.ts",
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
        "module_list.java",
        "go_slices_package.go",
        "go_slices_alias.go",
        "go_slices_const.go",
        "rust_local_array.rs",
        "rust_local_typed_array.rs",
        "rust_local_slice_ref.rs",
    ] {
        assert!(
            semantic_text.contains(expected),
            "semantic mode should include {expected}: {semantic}"
        );
    }
    for unexpected in [
        "wrong_element.py",
        "wrong_collection.js",
        "array_some_wrong_element.js",
        "array_some_wrong_collection.ts",
        "array_indexof_wrong_element.js",
        "array_indexof_wrong_collection.ts",
        "array_indexof_value.js",
        "array_findindex_wrong_element.js",
        "array_findindex_wrong_collection.ts",
        "array_findindex_value.js",
        "array_filter_length_wrong_element.js",
        "array_filter_length_wrong_collection.ts",
        "array_filter_length_value.js",
        "array_filter_length_absence_wrong_element.js",
        "array_filter_length_absence_wrong_collection.ts",
        "array_every_wrong_element.js",
        "array_every_wrong_collection.ts",
        "substring.rs",
        "module_set_mutated.js",
        "module_set_shadowed.ts",
        "module_list_shadowed.java",
        "go_slices_mutated.go",
        "go_slices_unimported.go",
        "rust_local_mutated.rs",
        "rust_local_custom_receiver.rs",
    ] {
        assert!(
            !semantic_text.contains(unexpected),
            "semantic mode must preserve literal membership boundaries: {semantic}"
        );
    }
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
        "--min-tokens",
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
        dir.join("wrong_element.ts"),
        "function f(values: string[], value: string, other: string): boolean {\n  return values.includes(other);\n}\n",
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
        "--min-tokens",
        "1",
        "--format",
        "json",
        "--top",
        "0",
    ]);
    let semantic_json: serde_json::Value =
        serde_json::from_str(&semantic).expect("semantic scan should emit JSON");
    let semantic_text = semantic_json.to_string();
    for expected in [
        "membership.py",
        "membership.ts",
        "membership.go",
        "membership.rs",
        "membership.java",
    ] {
        assert!(
            semantic_text.contains(expected),
            "semantic mode should include typed dynamic membership {expected}: {semantic}"
        );
    }
    for unexpected in ["wrong_element.ts", "string_negative.java"] {
        assert!(
            !semantic_text.contains(unexpected),
            "semantic mode must preserve typed dynamic membership boundaries: {semantic}"
        );
    }

    let _ = fs::remove_dir_all(&dir);
}

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
        "--min-tokens",
        "1",
        "--format",
        "json",
        "--top",
        "0",
    ]);
    let semantic_json: serde_json::Value =
        serde_json::from_str(&semantic).expect("semantic scan should emit JSON");
    let semantic_families = semantic_json.as_array().expect("semantic JSON array");
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
fn scan_mode_semantic_proves_typed_typescript_map_key_membership() {
    let dir = std::env::temp_dir().join(format!("nose_typed_ts_map_key_{}", std::process::id()));
    let _ = fs::remove_dir_all(&dir);
    fs::create_dir_all(&dir).unwrap();
    fs::write(
        dir.join("map_key.py"),
        "def f(lookup, other_lookup, key, other):\n    return key in lookup\n",
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
        dir.join("wrong_key.ts"),
        "function f(lookup: Map<string, string>, other_lookup: Map<string, string>, key: string, other: string): boolean {\n  return lookup.has(other);\n}\n",
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
        "--min-tokens",
        "1",
        "--format",
        "json",
        "--top",
        "0",
    ]);
    let semantic_json: serde_json::Value =
        serde_json::from_str(&semantic).expect("semantic scan should emit JSON");
    let semantic_text = semantic_json.to_string();
    for expected in ["map_key.py", "map_key.java", "map_key.ts"] {
        assert!(
            semantic_text.contains(expected),
            "semantic mode should include typed TS Map.has map-key membership {expected}: {semantic}"
        );
    }
    for unexpected in ["wrong_key.ts", "set_negative.ts"] {
        assert!(
            !semantic_text.contains(unexpected),
            "semantic mode must preserve typed TS Map.has boundaries: {semantic}"
        );
    }

    let _ = fs::remove_dir_all(&dir);
}

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

    let semantic = run(&[
        "scan",
        dir.to_str().unwrap(),
        "--mode",
        "semantic",
        "--min-lines",
        "1",
        "--min-tokens",
        "1",
        "--format",
        "json",
        "--top",
        "0",
    ]);
    let semantic_json: serde_json::Value =
        serde_json::from_str(&semantic).expect("semantic scan should emit JSON");
    let semantic_text = semantic_json.to_string();
    for expected in [
        "map_default.go",
        "ts_nullish.ts",
        "ts_has_get.ts",
        "ts_temp_guard.ts",
    ] {
        assert!(
            semantic_text.contains(expected),
            "semantic mode should include typed TS Map default lookup {expected}: {semantic}"
        );
    }
    for unexpected in [
        "ts_wrong_key.ts",
        "ts_wrong_default.ts",
        "ts_wrong_map.ts",
        "ts_untyped.ts",
    ] {
        assert!(
            !semantic_text.contains(unexpected),
            "semantic mode must preserve typed TS Map default boundaries: {semantic}"
        );
    }

    let _ = fs::remove_dir_all(&dir);
}

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
        dir.join("map_default_inline.js"),
        "function lookup(key, other) {\n  return new Map([[\"red\", 1], [\"blue\", 2]]).get(key) ?? 0;\n}\n",
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
        dir.join("wrong_map.py"),
        "def wrong_map(key, other):\n    return {\"red\": 9, \"blue\": 2}.get(key, 0)\n",
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
        "--min-tokens",
        "1",
        "--format",
        "json",
        "--top",
        "0",
    ]);
    let semantic_json: serde_json::Value =
        serde_json::from_str(&semantic).expect("semantic scan should emit JSON");
    let semantic_families = semantic_json.as_array().expect("semantic JSON array");
    let expected = [
        "map_default.py",
        "map_default.rb",
        "map_default_inline.js",
        "map_default_local.js",
        "map_default_has_get.js",
        "map_default_inline.ts",
        "map_default_java_of.java",
        "map_default_java_entries.java",
        "map_default_java_local.java",
        "map_default_module.js",
        "map_default_module.ts",
        "map_default_module.java",
        "map_default_rust_hashmap.rs",
        "map_default_rust_btreemap.rs",
        "map_default_rust_local.rs",
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
    for unexpected in [
        "wrong_key.py",
        "wrong_default.rb",
        "wrong_map.py",
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
        "object_wrong_key.js",
        "object_wrong_default.js",
        "object_wrong_map.js",
        "object_unguarded.js",
        "object_in.js",
        "object_method.js",
        "object_shadowed.js",
    ] {
        assert!(
            !positive_text.contains(unexpected),
            "semantic mode must preserve literal map-default boundaries: {semantic}"
        );
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
        "--min-tokens",
        "1",
        "--format",
        "json",
        "--top",
        "0",
    ]);
    let semantic_json: serde_json::Value =
        serde_json::from_str(&semantic).expect("semantic scan should emit JSON");
    let semantic_families = semantic_json.as_array().expect("semantic JSON array");
    assert_eq!(
        semantic_families.len(),
        1,
        "semantic mode should report one null-presence family: {semantic}"
    );
    let semantic_text = semantic_json.to_string();
    for expected in [
        "none_compare.py",
        "null_compare.c",
        "nil_method.rb",
        "none_method.rs",
        "iflet_none.rs",
    ] {
        assert!(
            semantic_text.contains(expected),
            "semantic mode should include {expected}: {semantic}"
        );
    }
    for unexpected in ["some_method.rs", "wrong_value.py"] {
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
        dir.join("large_branch.rb"),
        "def table(rows, errors)\n  if rows.length == 0\n    return nil\n  else\n    title = \"Potential problems\"\n    if errors.length > 0\n      title = title.red\n    else\n      title = title.yellow\n    end\n    return Terminal::Table.new(title: title, rows: rows).to_s\n  end\nend\n",
    )
    .unwrap();
    fs::write(
        dir.join("small_guard.rb"),
        "def payload(data)\n  return nil if data.empty?\n  data\nend\n",
    )
    .unwrap();

    let semantic = run(&[
        "scan",
        dir.to_str().unwrap(),
        "--mode",
        "semantic",
        "--min-lines",
        "1",
        "--min-tokens",
        "1",
        "--format",
        "json",
        "--top",
        "0",
    ]);
    let semantic_json: serde_json::Value =
        serde_json::from_str(&semantic).expect("semantic scan should emit JSON");
    let families = semantic_json.as_array().expect("semantic JSON array");
    let text = semantic_json.to_string();
    assert!(
        text.contains("large_branch.rb") && text.contains("small_guard.rb"),
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
                !file.ends_with("large_branch.rb")
                    || (loc["start_line"] == 2 && loc["end_line"] == 3)
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
        "--min-tokens",
        "1",
        "--format",
        "json",
        "--top",
        "0",
    ]);
    let semantic_json: serde_json::Value =
        serde_json::from_str(&semantic).expect("semantic scan should emit JSON");
    let semantic_families = semantic_json.as_array().expect("semantic JSON array");
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
        "--min-tokens",
        "1",
        "--format",
        "json",
        "--top",
        "0",
    ]);
    let semantic_json: serde_json::Value =
        serde_json::from_str(&semantic).expect("semantic scan should emit JSON");
    let semantic_families = semantic_json.as_array().expect("semantic JSON array");
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
        "--min-tokens",
        "1",
        "--format",
        "json",
        "--top",
        "0",
    ]);
    let semantic_json: serde_json::Value =
        serde_json::from_str(&semantic).expect("semantic scan should emit JSON");
    let semantic_families = semantic_json.as_array().expect("semantic JSON array");
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
        "--min-tokens",
        "1",
        "--format",
        "json",
        "--top",
        "0",
    ]);
    let semantic_json: serde_json::Value =
        serde_json::from_str(&semantic).expect("semantic scan should emit JSON");
    let semantic_families = semantic_json.as_array().expect("semantic JSON array");
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
        "--min-tokens",
        "1",
        "--format",
        "json",
        "--top",
        "0",
    ]);
    let semantic_json: serde_json::Value =
        serde_json::from_str(&semantic).expect("semantic scan should emit JSON");
    let semantic_families = semantic_json.as_array().expect("semantic JSON array");
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
        "--min-tokens",
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
        "--min-tokens",
        "1",
        "--format",
        "json",
        "--top",
        "0",
    ]);
    let semantic_json: serde_json::Value =
        serde_json::from_str(&semantic).expect("semantic scan should emit JSON");
    let semantic_families = semantic_json.as_array().expect("semantic JSON array");
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
        "--min-tokens",
        "1",
        "--format",
        "json",
        "--top",
        "0",
    ]);
    let semantic_json: serde_json::Value =
        serde_json::from_str(&semantic).expect("semantic scan should emit JSON");
    let semantic_families = semantic_json.as_array().expect("semantic JSON array");
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

    let semantic = run(&[
        "scan",
        dir.to_str().unwrap(),
        "--mode",
        "semantic",
        "--min-lines",
        "1",
        "--min-tokens",
        "1",
        "--format",
        "json",
        "--top",
        "0",
    ]);
    let semantic_json: serde_json::Value =
        serde_json::from_str(&semantic).expect("semantic scan should emit JSON");
    let semantic_families = semantic_json.as_array().expect("semantic JSON array");
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
        "null_allowed_negative.js",
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

    let semantic = run(&[
        "scan",
        dir.to_str().unwrap(),
        "--mode",
        "semantic",
        "--min-lines",
        "1",
        "--min-tokens",
        "1",
        "--format",
        "json",
        "--top",
        "0",
    ]);
    let semantic_json: serde_json::Value =
        serde_json::from_str(&semantic).expect("semantic scan should emit JSON");
    let semantic_families = semantic_json.as_array().expect("semantic JSON array");
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
        "--min-tokens",
        "1",
        "--format",
        "json",
        "--top",
        "0",
    ]);
    let semantic_json: serde_json::Value =
        serde_json::from_str(&semantic).expect("semantic scan should emit JSON");
    let semantic_families = semantic_json.as_array().expect("semantic JSON array");
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

    let semantic = run(&[
        "scan",
        dir.to_str().unwrap(),
        "--mode",
        "semantic",
        "--min-lines",
        "1",
        "--min-tokens",
        "1",
        "--format",
        "json",
        "--top",
        "0",
    ]);
    let semantic_json: serde_json::Value =
        serde_json::from_str(&semantic).expect("semantic scan should emit JSON");
    let semantic_families = semantic_json.as_array().expect("semantic JSON array");
    assert_eq!(
        semantic_families.len(),
        1,
        "semantic mode should report only same-key object construction: {semantic}"
    );
    let semantic_text = semantic_json.to_string();
    assert!(
        semantic_text.contains("object_a.ts")
            && semantic_text.contains("object_b.ts")
            && !semantic_text.contains("object_key_negative.ts"),
        "semantic mode must preserve object keys: {semantic}"
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
        "--min-tokens",
        "1",
        "--format",
        "json",
        "--top",
        "0",
    ]);
    let semantic_json: serde_json::Value =
        serde_json::from_str(&semantic).expect("semantic scan should emit JSON");
    let semantic_families = semantic_json.as_array().expect("semantic JSON array");
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
        "--min-tokens",
        "1",
        "--format",
        "json",
        "--top",
        "0",
    ]);
    let semantic_json: serde_json::Value =
        serde_json::from_str(&semantic).expect("semantic scan should emit JSON");
    let semantic_families = semantic_json.as_array().expect("semantic JSON array");
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
        "--min-tokens",
        "1",
        "--format",
        "json",
        "--top",
        "0",
    ]);
    let semantic_json: serde_json::Value =
        serde_json::from_str(&semantic).expect("semantic scan should emit JSON");
    let semantic_families = semantic_json.as_array().expect("semantic JSON array");
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
        "--min-tokens",
        "1",
        "--format",
        "json",
        "--top",
        "0",
    ]);
    let semantic_json: serde_json::Value =
        serde_json::from_str(&semantic).expect("semantic scan should emit JSON");
    let semantic_families = semantic_json.as_array().expect("semantic JSON array");
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

#[test]
fn default_mode_runs_syntax_and_semantic() {
    let dir = make_mode_project("default_modes");
    let p = dir.to_str().unwrap();
    let out = run(&["scan", p, "--min-tokens", "12", "--format", "json"]);
    assert!(
        out.contains("copy_a.py") && out.contains("copy_b.py"),
        "default mode includes syntax: {out}"
    );
    assert!(
        out.contains("renamed_a.py") && out.contains("renamed_b.py"),
        "default mode includes semantic: {out}"
    );
    let repeated = run(&[
        "scan",
        p,
        "--mode",
        "syntax",
        "--mode",
        "semantic",
        "--min-tokens",
        "12",
        "--format",
        "json",
    ]);
    assert!(
        repeated.contains("copy_a.py") && repeated.contains("copy_b.py"),
        "repeated --mode includes syntax: {repeated}"
    );
    assert!(
        repeated.contains("renamed_a.py") && repeated.contains("renamed_b.py"),
        "repeated --mode includes semantic: {repeated}"
    );
    let _ = fs::remove_dir_all(&dir);
}

#[test]
fn non_near_scan_modes_reject_similarity_thresholds() {
    let dir = make_mode_project("exact_threshold");
    for mode in ["syntax", "semantic", "syntax,semantic"] {
        let out = Command::new(bin())
            .args([
                "scan",
                dir.to_str().unwrap(),
                "--mode",
                mode,
                "--threshold",
                "0.5",
            ])
            .output()
            .expect("run nose");
        assert!(
            !out.status.success(),
            "{mode} must not accept a fuzzy similarity threshold"
        );
        let stderr = String::from_utf8(out.stderr).unwrap();
        assert!(
            stderr.contains("--threshold is only valid when --mode includes near"),
            "specific error explains the invalid threshold for {mode}: {stderr}"
        );
    }
    let _ = fs::remove_dir_all(&dir);
}

#[test]
fn near_scan_mode_accepts_similarity_threshold() {
    let dir = make_mode_project("near_threshold");
    let out = Command::new(bin())
        .args([
            "scan",
            dir.to_str().unwrap(),
            "--mode",
            "near",
            "--threshold",
            "0.5",
            "--min-tokens",
            "12",
            "--format",
            "json",
        ])
        .output()
        .expect("run nose");
    assert!(
        out.status.success(),
        "near mode should accept --threshold: {}",
        String::from_utf8_lossy(&out.stderr)
    );
    let _ = fs::remove_dir_all(&dir);
}

#[test]
fn scan_reports_the_clone_family() {
    let dir = make_project("fam");
    let out = run(&[
        "scan",
        dir.to_str().unwrap(),
        "--min-tokens",
        "12",
        "--top",
        "10",
    ]);
    assert!(out.contains("clone"), "has a header: {out}");
    assert!(out.contains("copies"), "lists a family: {out}");
    let _ = fs::remove_dir_all(&dir);
}

#[test]
fn exclude_drops_matching_paths() {
    let dir = make_project("excl");
    let p = dir.to_str().unwrap();
    // The family spans a/, b/, tests/. Excluding `tests` must shrink it.
    let with = run(&[
        "scan",
        p,
        "--min-tokens",
        "12",
        "--format",
        "json",
        "--top",
        "1",
    ]);
    let without = run(&[
        "scan",
        p,
        "--min-tokens",
        "12",
        "--format",
        "json",
        "--top",
        "1",
        "--exclude",
        "tests",
    ]);
    let count = |s: &str| s.matches("\"file\"").count();
    assert!(
        count(&with) > count(&without),
        "excluding tests removes a site: {} vs {}",
        count(&with),
        count(&without)
    );
    assert!(
        !without.contains("/tests/"),
        "no tests/ path in excluded output"
    );
    let _ = fs::remove_dir_all(&dir);
}

#[test]
fn exclude_supports_globs() {
    // `--exclude` takes gitignore-style globs, not just substrings: `a/**` excludes
    // the a/ directory specifically, leaving the family in b/ and tests/.
    let dir = make_project("glob");
    let p = dir.to_str().unwrap();
    let out = run(&[
        "scan",
        p,
        "--min-tokens",
        "12",
        "--format",
        "json",
        "--exclude",
        "a/**",
    ]);
    assert!(
        !out.contains("/a/f.py"),
        "glob `a/**` must exclude a/f.py: {out}"
    );
    assert!(out.contains("/b/f.py"), "b/ must remain: {out}");
    let _ = fs::remove_dir_all(&dir);
}

#[test]
fn min_value_filters_low_value_families() {
    let dir = make_project("minval");
    let p = dir.to_str().unwrap();
    // A value floor above any family's value hides them all; zero keeps them.
    let all = run(&["scan", p, "--min-tokens", "12", "--min-value", "0"]);
    let none = run(&["scan", p, "--min-tokens", "12", "--min-value", "100000"]);
    assert!(all.contains("copies"), "unfiltered shows a family: {all}");
    assert!(
        !none.contains("copies"),
        "a high value floor hides every family: {none}"
    );
    let _ = fs::remove_dir_all(&dir);
}

#[test]
fn hotspots_lists_modules() {
    let dir = make_project("hot");
    let out = run(&[
        "scan",
        dir.to_str().unwrap(),
        "--min-tokens",
        "12",
        "--hotspots",
    ]);
    assert!(
        out.contains("duplication hotspots"),
        "hotspots section present: {out}"
    );
    assert!(out.contains("dup lines"), "hotspot rows present: {out}");
    let _ = fs::remove_dir_all(&dir);
}

#[test]
fn empty_corpus_warns_on_stderr() {
    // A path with no supported source must not silently look like "no duplication".
    let dir = std::env::temp_dir().join(format!("nose_empty_{}", std::process::id()));
    let _ = fs::remove_dir_all(&dir);
    fs::create_dir_all(&dir).unwrap();
    fs::write(dir.join("README.txt"), "no code here\n").unwrap();
    let out = Command::new(bin())
        .args(["scan", dir.to_str().unwrap()])
        .output()
        .expect("run nose");
    let stderr = String::from_utf8(out.stderr).unwrap();
    assert!(
        stderr.contains("no supported source files found"),
        "expected an empty-corpus warning, got: {stderr}"
    );
    let _ = fs::remove_dir_all(&dir);
}

#[test]
fn output_is_independent_of_thread_count() {
    // Parallel lowering/detection must not affect output: a single-threaded run and
    // a multi-threaded run must produce byte-identical JSON. A release invariant.
    let dir = make_project("threads");
    let p = dir.to_str().unwrap();
    let run_with_threads = |n: &str| {
        let out = Command::new(bin())
            .args(["scan", p, "--min-tokens", "12", "--format", "json"])
            .env("RAYON_NUM_THREADS", n)
            .output()
            .expect("run nose");
        assert!(out.status.success());
        String::from_utf8(out.stdout).unwrap()
    };
    assert_eq!(
        run_with_threads("1"),
        run_with_threads("8"),
        "1-thread and 8-thread runs must produce identical output"
    );
    let _ = fs::remove_dir_all(&dir);
}

#[test]
fn cache_output_matches_uncached_cold_and_warm() {
    // --cache-dir must not change results: cold (populating) and warm (reusing)
    // runs must both equal the non-cached output. This is the cache's correctness
    // contract — a stale/incorrect entry would diverge here.
    let dir = make_project("cache");
    // make_project's clones are *renamed* copies — matched only by the value-graph
    // channel. Add files with a *verbatim* duplicated block so the contiguous
    // (copy-paste) channel also produces a family: the cache once stored only units and
    // silently dropped that channel, so without the streams cached this run would diverge
    // here. (Identical variable names → identical raw token stream → a contiguous match,
    // unlike the renamed value-graph clones.)
    let block = "def handle(events):\n    out = []\n    for e in events:\n        if e.kind == 1:\n            out.append(e.payload)\n            record(e.id, e.kind)\n    return out\n";
    for name in ["dup_a", "dup_b", "dup_c"] {
        fs::write(dir.join(format!("{name}.py")), block).unwrap();
    }
    let p = dir.to_str().unwrap();
    let cache = std::env::temp_dir().join(format!("nose_cachedir_{}", std::process::id()));
    let _ = fs::remove_dir_all(&cache);
    let cd = cache.to_str().unwrap();

    let uncached = run(&["scan", p, "--min-tokens", "12", "--format", "json"]);
    let cold = run(&[
        "scan",
        p,
        "--min-tokens",
        "12",
        "--format",
        "json",
        "--cache-dir",
        cd,
    ]);
    let warm = run(&[
        "scan",
        p,
        "--min-tokens",
        "12",
        "--format",
        "json",
        "--cache-dir",
        cd,
    ]);

    assert_eq!(
        uncached, cold,
        "cold cache run must match the uncached output"
    );
    assert_eq!(cold, warm, "warm cache run must match the cold run");
    let _ = fs::remove_dir_all(&cache);
    let _ = fs::remove_dir_all(&dir);
}

#[test]
fn fail_flag_sets_exit_code_as_ci_gate() {
    let dir = make_project("fail");
    let p = dir.to_str().unwrap();
    // A family exists → --fail must exit non-zero.
    let found = Command::new(bin())
        .args(["scan", p, "--min-tokens", "12", "--fail"])
        .output()
        .expect("run");
    assert!(
        !found.status.success(),
        "--fail must fail when a family is found"
    );
    // Filter everything out → nothing to fail on → success.
    let clean = Command::new(bin())
        .args([
            "scan",
            p,
            "--min-tokens",
            "12",
            "--min-value",
            "1e9",
            "--fail",
        ])
        .output()
        .expect("run");
    assert!(
        clean.status.success(),
        "--fail must pass when no family survives filters"
    );
    let _ = fs::remove_dir_all(&dir);
}

#[test]
fn output_paths_are_relative_to_cwd() {
    // Even when scanned via an absolute path, reported paths should be relative to
    // the working directory (readable in CI logs / reviews).
    let dir = make_project("relpath");
    let abs = dir.canonicalize().unwrap();
    let out = Command::new(bin())
        .args([
            "scan",
            abs.to_str().unwrap(),
            "--min-tokens",
            "12",
            "--format",
            "json",
        ])
        .current_dir(&abs)
        .output()
        .expect("run");
    let stdout = String::from_utf8(out.stdout).unwrap();
    assert!(
        stdout.contains("\"a/f.py\"") || stdout.contains("\"b/f.py\""),
        "paths should be relative to cwd, got: {stdout}"
    );
    assert!(
        !stdout.contains(abs.to_str().unwrap()),
        "no absolute path should leak: {stdout}"
    );
    let _ = fs::remove_dir_all(&dir);
}

#[test]
fn baseline_hides_accepted_families() {
    // Write a baseline accepting today's families, then a re-run reports nothing new.
    let dir = make_project("baseline");
    let p = dir.to_str().unwrap();
    let bl = std::env::temp_dir().join(format!("nose_bl_{}.json", std::process::id()));
    let bls = bl.to_str().unwrap();

    // Sanity: without a baseline there IS a family.
    assert!(run(&["scan", p, "--min-tokens", "12"]).contains("copies"));

    // Accept current state…
    let _ = Command::new(bin())
        .args([
            "scan",
            p,
            "--min-tokens",
            "12",
            "--baseline",
            bls,
            "--write-baseline",
        ])
        .output()
        .expect("run");
    assert!(bl.exists(), "baseline file should be written");

    // …then a re-run shows no *new* families.
    let after = run(&["scan", p, "--min-tokens", "12", "--baseline", bls]);
    assert!(
        !after.contains("sites"),
        "baselined families must be hidden, got: {after}"
    );
    let _ = fs::remove_file(&bl);
    let _ = fs::remove_dir_all(&dir);
}

#[test]
fn config_file_supplies_defaults() {
    // A nose.toml in the working dir provides defaults (here: an exclude glob and
    // min-tokens); a CLI flag still overrides.
    let dir = make_project("cfg");
    fs::write(
        dir.join("nose.toml"),
        "[scan]\nexclude = [\"a/**\"]\nmin-tokens = 12\n",
    )
    .unwrap();
    let out = Command::new(bin())
        .args(["scan", ".", "--format", "json"])
        .current_dir(&dir)
        .output()
        .expect("run");
    let stdout = String::from_utf8(out.stdout).unwrap();
    assert!(
        !stdout.contains("a/f.py"),
        "config exclude a/** should drop a/f.py: {stdout}"
    );
    assert!(stdout.contains("b/f.py"), "b/ remains: {stdout}");
    let _ = fs::remove_dir_all(&dir);
}

#[test]
fn inline_nose_ignore_suppresses_a_site() {
    // A `nose-ignore` marker above a function drops that site; with only one copy
    // left there's no family.
    let dir = std::env::temp_dir().join(format!("nose_sup_{}", std::process::id()));
    let _ = fs::remove_dir_all(&dir);
    let body = "def f(items):\n    t = 0\n    for x in items:\n        if x > 0:\n            t = t + x * x\n    return t\n";
    fs::create_dir_all(dir.join("a")).unwrap();
    fs::create_dir_all(dir.join("b")).unwrap();
    fs::write(dir.join("a/f.py"), body).unwrap();
    fs::write(dir.join("b/f.py"), format!("# nose-ignore\n{body}")).unwrap();
    let p = dir.to_str().unwrap();
    assert!(
        run(&["scan", p, "--min-tokens", "12"]).contains("0 clone"),
        "the marked copy must be suppressed, leaving no family"
    );
    let _ = fs::remove_dir_all(&dir);
}

#[test]
fn sarif_output_is_well_formed() {
    let dir = make_project("sarif");
    let out = run(&[
        "scan",
        dir.to_str().unwrap(),
        "--min-tokens",
        "12",
        "--format",
        "sarif",
    ]);
    let v: serde_json::Value = serde_json::from_str(&out).expect("SARIF must be valid JSON");
    assert_eq!(v["version"], "2.1.0");
    assert!(v["runs"][0]["tool"]["driver"]["name"] == "nose");
    assert!(
        !v["runs"][0]["results"].as_array().unwrap().is_empty(),
        "should have at least one result: {out}"
    );
    let _ = fs::remove_dir_all(&dir);
}

#[test]
fn diff_shows_the_differing_line() {
    // Two near-identical functions differing in one line → --diff marks it +/-.
    let dir = std::env::temp_dir().join(format!("nose_diff_{}", std::process::id()));
    let _ = fs::remove_dir_all(&dir);
    fs::create_dir_all(dir.join("a")).unwrap();
    fs::create_dir_all(dir.join("b")).unwrap();
    // The two sum-loops differ only in a literal (the per-element weight), so they
    // are a clear near-duplicate family. (A `+`-loop vs a `*`-loop is deliberately
    // *not* used here: the value graph now treats those as distinct reductions — see
    // §AH — so they are no longer a single family.)
    fs::write(
        dir.join("a/f.py"),
        "def f(items):\n    t = 0\n    for x in items:\n        t = t + x * 2\n    return t\n",
    )
    .unwrap();
    fs::write(
        dir.join("b/f.py"),
        "def g(items):\n    t = 0\n    for x in items:\n        t = t + x * 3\n    return t\n",
    )
    .unwrap();
    let out = run(&[
        "scan",
        dir.to_str().unwrap(),
        "--mode",
        "near",
        "--threshold",
        "0.5",
        "--min-tokens",
        "10",
        "--diff",
    ]);
    assert!(out.contains("diff  "), "should print a diff header: {out}");
    assert!(
        out.contains("-         t = t + x * 2") && out.contains("+         t = t + x * 3"),
        "diff should mark the changed line: {out}"
    );
    let _ = fs::remove_dir_all(&dir);
}

#[test]
fn version_flag_works() {
    let out = run(&["--version"]);
    assert!(out.starts_with("nose "), "version line: {out}");
}

#[test]
fn broken_pipe_exits_cleanly() {
    // `nose scan … | head` (or quitting a pager) closes the read end of stdout while
    // nose is still writing. That write then fails with `BrokenPipe`, which `println!`
    // turns into a panic — the `failed printing to stdout` crash users hit. The Unix
    // convention is to stop quietly; main()'s panic hook must catch this and exit 0.
    // We reproduce it by closing the pipe's read end outright, so every write to the
    // now-readerless pipe gets EPIPE.
    let dir = std::env::temp_dir().join(format!("nose_pipe_{}", std::process::id()));
    let _ = fs::remove_dir_all(&dir);
    // A handful of large, distinct clone families → plenty of `--diff` output, so the
    // child keeps writing past any buffering and reliably hits the dead pipe.
    for i in 0..40 {
        let body = |delta: &str| {
            let mut s = format!("def fam{i}(xs):\n    acc{i} = 0\n");
            for k in 0..120 {
                s.push_str(&format!("    acc{i} = acc{i} + xs[{k}] * {i}\n"));
            }
            s.push_str(&format!("    acc{i} = acc{i} {delta}\n    return acc{i}\n"));
            s
        };
        for (sub, delta) in [("a", "+ 1"), ("b", "- 1")] {
            let d = dir.join(sub);
            fs::create_dir_all(&d).unwrap();
            fs::write(d.join(format!("f{i}.py")), body(delta)).unwrap();
        }
    }

    let mut child = Command::new(bin())
        .args([
            "scan",
            dir.to_str().unwrap(),
            "--mode",
            "near",
            "--threshold",
            "0.5",
            "--diff",
            "--top",
            "40",
        ])
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("spawn nose");
    // Close the read end immediately — the moral equivalent of `| head` exiting early.
    drop(child.stdout.take());
    let out = child.wait_with_output().expect("wait for nose");
    let _ = fs::remove_dir_all(&dir);

    let stderr = String::from_utf8_lossy(&out.stderr);
    assert!(
        !stderr.contains("panicked") && !stderr.contains("Broken pipe"),
        "broken pipe must not panic, got stderr:\n{stderr}"
    );
    assert!(
        out.status.success(),
        "broken pipe should exit cleanly (0), got {:?}\nstderr:\n{stderr}",
        out.status
    );
}

#[test]
fn deeply_nested_file_does_not_overflow() {
    // A pathologically deep expression (minified bundle / generated code) must not
    // crash the recursive lowering on rayon's small worker stack. Regression for the
    // stack-overflow on real repos (prettier test fixtures); main() sizes the pool's
    // stack so this completes instead of aborting (`run` asserts a clean exit).
    let dir = std::env::temp_dir().join(format!("nose_deep_{}", std::process::id()));
    let _ = fs::remove_dir_all(&dir);
    fs::create_dir_all(&dir).unwrap();
    let depth = 40_000;
    let body = format!("const x = {}1{};\n", "[".repeat(depth), "]".repeat(depth));
    fs::write(dir.join("deep.js"), body).unwrap();
    let _ = run(&["scan", dir.to_str().unwrap()]);
    let _ = fs::remove_dir_all(&dir);
}

#[test]
fn proposal_shows_shared_skeleton_and_parameters() {
    // Two near-identical functions differing in one line → an extraction proposal:
    // the shared skeleton plus a ⟨param⟩ for the varying spot.
    let dir = std::env::temp_dir().join(format!("nose_prop_{}", std::process::id()));
    let _ = fs::remove_dir_all(&dir);
    fs::create_dir_all(dir.join("a")).unwrap();
    fs::create_dir_all(dir.join("b")).unwrap();
    let mk = |op: &str| {
        format!("def f(xs):\n    t = 0\n    for x in xs:\n        if x > 0:\n            t = t {op} x\n    return t\n")
    };
    fs::write(dir.join("a/m.py"), mk("+")).unwrap();
    fs::write(dir.join("b/m.py"), mk("*")).unwrap();
    let out = run(&[
        "scan",
        dir.to_str().unwrap(),
        "--mode",
        "near",
        "--threshold",
        "0.5",
        "--proposal",
        "--min-tokens",
        "12",
    ]);
    assert!(out.contains("proposal"), "should print a proposal: {out}");
    assert!(
        out.contains("parameter(s) vary"),
        "should report parameters: {out}"
    );
    assert!(
        out.contains("⟨param 1⟩"),
        "should show a parameter placeholder: {out}"
    );
    let _ = fs::remove_dir_all(&dir);
}

#[test]
fn sort_keys_label_the_ranking_and_reject_garbage() {
    let dir = make_project("sort");
    let p = dir.to_str().unwrap();
    // Default ranking is extractability — the header says so, in plain language.
    let def = run(&["scan", p, "--min-tokens", "12"]);
    assert!(
        def.contains("ranked by extractability"),
        "default header names the ranking: {def}"
    );
    // Families are described in plain language (copies + removable lines), not a
    // wall of internal metrics.
    assert!(
        def.contains("copies") && def.contains("lines removable"),
        "family summary is plain-language: {def}"
    );
    // --sort value switches the header.
    let byval = run(&["scan", p, "--min-tokens", "12", "--sort", "value"]);
    assert!(
        byval.contains("raw duplicated volume"),
        "--sort value names the ranking: {byval}"
    );
    // An unknown sort key is rejected by clap.
    let bad = Command::new(bin())
        .args(["scan", p, "--sort", "bogus"])
        .output()
        .expect("run nose");
    assert!(!bad.status.success(), "unknown --sort value is rejected");
    let _ = fs::remove_dir_all(&dir);
}

#[test]
fn scan_reports_what_it_scanned() {
    let dir = make_project("scope");
    let p = dir.to_str().unwrap();
    // The scope line states the file count and per-language breakdown (make_project
    // writes four Python files), so a `.gitignore`-pruned scope is visible, not silent.
    let out = run(&["scan", p, "--min-tokens", "12"]);
    assert!(
        out.contains("scanned 4 files") && out.contains("python 4"),
        "header reports scanned count and languages: {out}"
    );
    // The scope line must not corrupt machine-readable output.
    let json = run(&["scan", p, "--min-tokens", "12", "--format", "json"]);
    assert!(
        json.trim_start().starts_with('[') && !json.contains("scanned"),
        "json output stays pure: {json}"
    );
    let _ = fs::remove_dir_all(&dir);
}

/// A determinism guard with teeth: the toy `make_project` is too small to exercise the
/// order-sensitive paths (shared-line IDF summation, RANSAC offset ties, cross-family
/// ordering) where byte-identical output has actually broken before. Generate a project
/// with many same-language near-duplicate functions — several clone families whose copies
/// differ in a few lines — and assert the JSON report is identical across a range of
/// thread counts (each a distinct process, so this also covers `HashMap` seed variation).
#[test]
fn output_is_byte_identical_across_thread_counts_on_a_rich_project() {
    let dir = std::env::temp_dir().join(format!("nose_determinism_{}", std::process::id()));
    let _ = fs::remove_dir_all(&dir);
    fs::create_dir_all(&dir).unwrap();
    // 8 families × 5 near-duplicate copies. Each copy renames variables and tweaks a
    // couple of lines, so families form, RANSAC alignment runs, and the honest
    // shared-line counter compares several members.
    for fam in 0..8 {
        for copy in 0..5 {
            let v = format!("v{copy}");
            let src = format!(
                "def family{fam}_{copy}(items):\n    \
                 {v}_total = {copy}\n    \
                 {v}_seen = []\n    \
                 for {v}_x in items:\n        \
                 if {v}_x > {fam}:\n            \
                 {v}_total = {v}_total + {v}_x * {fam}\n            \
                 {v}_seen.append({v}_x)\n        \
                 else:\n            \
                 {v}_total = {v}_total - {copy}\n    \
                 return ({v}_total, {v}_seen)\n"
            );
            fs::write(dir.join(format!("f{fam}_{copy}.py")), src).unwrap();
        }
    }
    let p = dir.to_str().unwrap();
    let run_threads = |n: &str| {
        let out = Command::new(bin())
            .args([
                "scan",
                p,
                "--mode",
                "near",
                "--threshold",
                "0.5",
                "--min-tokens",
                "12",
                "--format",
                "json",
                "--top",
                "1000",
            ])
            .env("RAYON_NUM_THREADS", n)
            .output()
            .expect("run nose");
        assert!(out.status.success());
        String::from_utf8(out.stdout).unwrap()
    };
    let baseline = run_threads("1");
    assert!(
        baseline.contains("\"members\""),
        "the fixture forms families: {baseline}"
    );
    for n in ["2", "3", "4", "8"] {
        assert_eq!(
            run_threads(n),
            baseline,
            "{n}-thread output must be byte-identical to the 1-thread run"
        );
    }
    let _ = fs::remove_dir_all(&dir);
}

#[test]
fn top_zero_shows_all_families() {
    // `--top 0` is documented as "no limit" (docs/usage.md). It must return every
    // family, not an empty set. Regression: `.take(0)` used to silently drop all.
    let dir = std::env::temp_dir().join(format!("nose_top0_{}", std::process::id()));
    let _ = fs::remove_dir_all(&dir);
    // Build several *structurally distinct* clone families so "all" is clearly more
    // than "top 1". nose finds semantic (Type-4) clones, so each family must have a
    // different shape (statement count / control flow) or they collapse into one.
    let bodies = [
        "def f0(items):\n    acc = 0\n    for x in items:\n        acc = acc + x\n    return acc\n",
        "def f1(items):\n    acc = 1\n    for x in items:\n        if x > 0:\n            acc = acc * x\n    return acc\n",
        "def f2(s):\n    out = []\n    for c in s:\n        out.append(c)\n        out.append(c)\n    return out\n",
        "def f3(a, b):\n    r = 0\n    while a < b:\n        r = r + a\n        a = a + 1\n    return r\n",
        "def f4(d):\n    keys = []\n    for k in d:\n        if k is not None:\n            keys.append(k)\n    keys.sort()\n    return keys\n",
        "def f5(n):\n    total = 0\n    i = 0\n    while i < n:\n        total = total + i * i\n        i = i + 1\n    return total\n",
    ];
    for (i, body) in bodies.iter().enumerate() {
        for sub in ["a", "b"] {
            let d = dir.join(sub);
            fs::create_dir_all(&d).unwrap();
            fs::write(d.join(format!("f{i}.py")), body).unwrap();
        }
    }
    let p = dir.to_str().unwrap();
    let count = |args: &[&str]| -> usize {
        let out = run(args);
        let v: serde_json::Value = serde_json::from_str(&out).expect("valid JSON");
        v.as_array().expect("JSON array").len()
    };

    let all = count(&[
        "scan",
        p,
        "--min-tokens",
        "12",
        "--format",
        "json",
        "--top",
        "0",
    ]);
    let one = count(&[
        "scan",
        p,
        "--min-tokens",
        "12",
        "--format",
        "json",
        "--top",
        "1",
    ]);
    assert!(all > 1, "--top 0 must show all families, got {all}");
    assert_eq!(one, 1, "--top 1 must still cap at one family, got {one}");
    assert!(
        all >= 6,
        "--top 0 should include every distinct family (expected >=6, got {all})"
    );
    let _ = fs::remove_dir_all(&dir);
}
