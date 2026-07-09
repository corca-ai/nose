use super::*;

#[path = "guards/nullish_and_object.rs"]
mod nullish_and_object;

#[test]
fn query_mode_semantic_proves_null_presence_predicates() {
    let dir = make_temp_dir("null_presence");
    write_files(
        &dir,
        &[
            (
                "none_compare.py",
                "def is_missing(value, other):\n    return value is None\n",
            ),
            (
                "null_compare.c",
                "#include <stddef.h>\n\nint is_missing(void *value, void *other) {\n    return value == NULL;\n}\n",
            ),
            (
                "nil_method.rb",
                "def is_missing(value, other)\n  value.nil?\nend\n",
            ),
            (
                "swift_nil.swift",
                "func isMissing(_ value: Int?, _ other: Int?) -> Bool {\n    return value == nil\n}\n",
            ),
            (
                "none_method.rs",
                "pub fn is_missing(value: Option<i32>, other: Option<i32>) -> bool {\n    value.is_none()\n}\n",
            ),
            (
                "iflet_none.rs",
                "pub fn is_missing(value: Option<i32>, other: Option<i32>) -> bool {\n    if let None = value { true } else { false }\n}\n",
            ),
            (
                "some_method.rs",
                "pub fn is_present(value: Option<i32>, other: Option<i32>) -> bool {\n    value.is_some()\n}\n",
            ),
            (
                "wrong_value.py",
                "def wrong_value(value, other):\n    return other is None\n",
            ),
        ],
    );

    let semantic = query_min_json(&dir, "semantic");
    let semantic_json = query_json(&semantic);
    let semantic_families = query_families(&semantic_json);
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
        "swift_nil.swift",
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
fn query_mode_semantic_reports_flattened_guard_span_only() {
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

    let semantic = query_min_json(&dir, "semantic");
    let semantic_json = query_json(&semantic);
    let families = query_families(&semantic_json);
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
fn query_mode_semantic_preserves_js_typeof_operator() {
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

    let semantic = query_min_json(&dir, "semantic");
    let semantic_json = query_json(&semantic);
    let semantic_families = query_families(&semantic_json);
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
fn query_mode_semantic_allows_safe_uninterpreted_calls() {
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
        "query",
        dir.to_str().unwrap(),
        "--mode",
        "semantic",
        "--min-lines",
        "3",
        "--min-size",
        "1",
        "--format",
        "json",
        "top=0",
    ]);
    let semantic_json = query_json(&semantic);
    let semantic_families = query_families(&semantic_json);
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
fn query_mode_semantic_allows_safe_uninterpreted_method_calls() {
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
        "query",
        dir.to_str().unwrap(),
        "--mode",
        "semantic",
        "--min-lines",
        "3",
        "--min-size",
        "1",
        "--format",
        "json",
        "top=0",
    ]);
    let semantic_json = query_json(&semantic);
    let semantic_families = query_families(&semantic_json);
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
fn query_mode_semantic_distinguishes_sequence_kinds() {
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

    let semantic = query_min_json(&dir, "semantic");
    let semantic_json = query_json(&semantic);
    let semantic_families = query_families(&semantic_json);
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
fn query_mode_semantic_allows_static_import_identity() {
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

    let semantic = query_min_json(&dir, "semantic");
    let semantic_json = query_json(&semantic);
    let semantic_families = query_families(&semantic_json);
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
fn query_mode_semantic_allows_named_namespace_import_identity() {
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
        dir.join("typed_named.ts"),
        "import { helper } from \"./shared-math\";\n\nfunction report(input: number): number {\n  return helper(input + 1);\n}\n",
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

    let semantic = query_min_json(&dir, "semantic");
    let semantic_json: serde_json::Value =
        serde_json::from_str(&semantic).expect("semantic query should emit JSON");
    let semantic_text = semantic_json.to_string();
    assert!(
        family_contains_all(&semantic_json, &["named.js", "namespace.js"]),
        "semantic mode should include untyped JS static import member positives: {semantic}"
    );
    assert!(
        family_contains_all(&semantic_json, &["typed_named.ts", "typed_namespace.ts"]),
        "semantic mode should include typed TS static import member positives: {semantic}"
    );
    assert!(
        !semantic_text.contains("wrong_member.js"),
        "semantic mode must preserve imported member coordinate boundaries: {semantic}"
    );

    let _ = fs::remove_dir_all(&dir);
}

#[test]
fn query_mode_semantic_allows_static_projection_identity() {
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

    let semantic = query_min_json(&dir, "semantic");
    let semantic_json = query_json(&semantic);
    let semantic_families = query_families(&semantic_json);
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
