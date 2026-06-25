use super::*;

#[test]
fn query_mode_semantic_proves_js_object_keys_key_view_boundaries() {
    let dir = std::env::temp_dir().join(format!("nose_js_object_keys_{}", std::process::id()));
    let _ = fs::remove_dir_all(&dir);
    fs::create_dir_all(&dir).unwrap();
    fs::write(
        dir.join("object_keys_local.js"),
        "function f(key, other) {\n  const values = { red: 1, blue: 2 };\n  return Object.keys(values).includes(key);\n}\n",
    )
    .unwrap();
    fs::write(
        dir.join("object_keys_inline.js"),
        "function f(key, other) {\n  return Object.keys({ red: 1, blue: 2 }).includes(key);\n}\n",
    )
    .unwrap();
    fs::write(
        dir.join("object_keys_wrong_key.js"),
        "function f(key, other) {\n  const values = { red: 1, blue: 2 };\n  return Object.keys(values).includes(other);\n}\n",
    )
    .unwrap();
    fs::write(
        dir.join("object_values.js"),
        "function f(key, other) {\n  const values = { red: 1, blue: 2 };\n  return Object.values(values).includes(key);\n}\n",
    )
    .unwrap();
    fs::write(
        dir.join("object_entries.js"),
        "function f(key, other) {\n  const values = { red: 1, blue: 2 };\n  return Object.entries(values).includes(key);\n}\n",
    )
    .unwrap();
    fs::write(
        dir.join("object_shadowed.js"),
        "function f(Object, key, other) {\n  const values = { red: 1, blue: 2 };\n  return Object.keys(values).includes(key);\n}\n",
    )
    .unwrap();
    fs::write(
        dir.join("object_mutated.js"),
        "function f(key, other) {\n  const values = { red: 1, blue: 2 };\n  values.green = 3;\n  return Object.keys(values).includes(key);\n}\n",
    )
    .unwrap();

    let semantic = query_min_json(&dir, "semantic");
    let semantic_json = query_json(&semantic);
    let semantic_families = query_families(&semantic_json);
    let positive_family = semantic_families
        .iter()
        .map(serde_json::Value::to_string)
        .find(|family| {
            ["object_keys_local.js", "object_keys_inline.js"]
                .iter()
                .all(|expected| family.contains(expected))
        })
        .unwrap_or_else(|| {
            panic!("semantic mode should include Object.keys key-view family: {semantic}")
        });

    for expected in ["object_keys_local.js", "object_keys_inline.js"] {
        assert!(
            positive_family.contains(expected),
            "semantic mode should include Object.keys positive {expected}: {semantic}"
        );
    }
    for unexpected in [
        "object_keys_wrong_key.js",
        "object_values.js",
        "object_entries.js",
        "object_shadowed.js",
        "object_mutated.js",
    ] {
        assert!(
            !positive_family.contains(unexpected),
            "semantic mode must preserve Object.keys boundary {unexpected}: {semantic}"
        );
    }

    let _ = fs::remove_dir_all(&dir);
}
