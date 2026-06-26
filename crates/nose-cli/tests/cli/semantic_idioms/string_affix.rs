use super::*;

#[test]
fn query_mode_semantic_hardens_js_ts_string_affix_receivers() {
    let dir = std::env::temp_dir().join(format!("nose_string_affix_{}", std::process::id()));
    let _ = fs::remove_dir_all(&dir);
    fs::create_dir_all(&dir).unwrap();

    fs::write(
        dir.join("prefix.py"),
        "def prefix(value: str, other: str) -> bool:\n    return value.startswith(\"pre\")\n",
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
        dir.join("suffix.py"),
        "def suffix(value: str) -> bool:\n    return value.endswith(\"pre\")\n",
    )
    .unwrap();
    fs::write(
        dir.join("suffix.ts"),
        "function suffix(value: string): boolean {\n  return value.endsWith(\"pre\");\n}\n",
    )
    .unwrap();

    fs::write(
        dir.join("prefix.js"),
        "function prefix(value, other) {\n  return value.startsWith(\"pre\");\n}\n",
    )
    .unwrap();
    fs::write(
        dir.join("borrowed_prototype.js"),
        "function borrowed(value) {\n  return String.prototype.startsWith.call(value, \"pre\");\n}\n",
    )
    .unwrap();
    fs::write(
        dir.join("custom_same_name.js"),
        "function custom(value) {\n  const box = { startsWith(prefix) { return prefix.length > 0; } };\n  return box.startsWith(\"pre\");\n}\n",
    )
    .unwrap();
    fs::write(
        dir.join("offset.ts"),
        "function offset(value: string): boolean {\n  return value.startsWith(\"pre\", 1);\n}\n",
    )
    .unwrap();
    fs::write(
        dir.join("string_object_wrapper.ts"),
        "function wrapper(value: String): boolean {\n  return value.startsWith(\"pre\");\n}\n",
    )
    .unwrap();
    fs::write(
        dir.join("nullable.ts"),
        "function nullable(value: string | null): boolean {\n  return value.startsWith(\"pre\");\n}\n",
    )
    .unwrap();
    fs::write(
        dir.join("patched.ts"),
        "String.prototype.startsWith = function() { return true; };\nfunction patched(value: string): boolean {\n  return value.startsWith(\"pre\");\n}\n",
    )
    .unwrap();
    fs::write(
        dir.join("patched_after.ts"),
        "function patchedAfter(value: string): boolean {\n  return value.startsWith(\"pre\");\n}\nString.prototype.startsWith = function() { return true; };\n",
    )
    .unwrap();
    fs::write(
        dir.join("affix_negative.py"),
        "def prefix_alt(value, other):\n    return value.startswith(\"alt\")\n",
    )
    .unwrap();
    fs::write(
        dir.join("receiver_negative.rs"),
        "pub fn prefix_other(value: &str, other: &str) -> bool {\n    other.starts_with(\"pre\")\n}\n",
    )
    .unwrap();

    let semantic = query_min_json(&dir, "semantic");
    let semantic_json = query_json(&semantic);
    assert!(
        family_contains_all(
            &semantic_json,
            &[
                "prefix.py",
                "prefix.ts",
                "prefix.go",
                "prefix.rs",
                "prefix.java",
            ],
        ),
        "semantic mode should report the proved prefix affix family: {semantic}"
    );
    assert!(
        family_contains_all(&semantic_json, &["suffix.py", "suffix.ts"]),
        "semantic mode should report the proved suffix affix family: {semantic}"
    );
    assert!(
        !family_contains_all(&semantic_json, &["prefix.py", "suffix.ts"]),
        "prefix and suffix coordinates must stay distinct: {semantic}"
    );

    for unexpected in [
        "prefix.js",
        "borrowed_prototype.js",
        "custom_same_name.js",
        "offset.ts",
        "string_object_wrapper.ts",
        "nullable.ts",
        "patched.ts",
        "patched_after.ts",
        "affix_negative.py",
        "receiver_negative.rs",
    ] {
        assert!(
            !family_contains_all(&semantic_json, &["prefix.py", unexpected])
                && !family_contains_all(&semantic_json, &["prefix.ts", unexpected]),
            "semantic mode must keep {unexpected} out of the proved affix family: {semantic}"
        );
    }

    let _ = fs::remove_dir_all(&dir);
}
