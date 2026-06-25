use super::*;

#[test]
fn query_mode_semantic_proves_js_object_keys_key_view_boundaries() {
    let dir = std::env::temp_dir().join(format!("nose_js_object_keys_{}", std::process::id()));
    let _ = fs::remove_dir_all(&dir);
    fs::create_dir_all(&dir).unwrap();
    write_object_keys_fixtures(&dir);

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
        "object_proto_key.js",
        "object_escaped_proto_key.js",
        "object_numeric_key.js",
        "object_mutated.js",
        "object_delete_mutation.js",
        "object_alias_mutated.js",
        "object_receiver_call.js",
        "object_hoisted_mutator.js",
        "object_direct_eval.js",
        "object_with_scope_delete.js",
        "object_enclosing_with_scope.js",
        "object_for_in_target_mutation.js",
        "object_for_of_target_mutation.js",
        "object_conditional_initializer.js",
        "object_parameter_shadow.js",
    ] {
        assert!(
            !positive_family.contains(unexpected),
            "semantic mode must preserve Object.keys boundary {unexpected}: {semantic}"
        );
    }

    let _ = fs::remove_dir_all(&dir);
}

fn write_object_keys_fixtures(dir: &Path) {
    for (name, src) in [
        (
            "object_keys_local.js",
            "function f(key, other) {\n  const values = { red: 1, blue: 2 };\n  return Object.keys(values).includes(key);\n}\n",
        ),
        (
            "object_keys_inline.js",
            "function f(key, other) {\n  return Object.keys({ red: 1, blue: 2 }).includes(key);\n}\n",
        ),
        (
            "object_keys_wrong_key.js",
            "function f(key, other) {\n  const values = { red: 1, blue: 2 };\n  return Object.keys(values).includes(other);\n}\n",
        ),
        (
            "object_values.js",
            "function f(key, other) {\n  const values = { red: 1, blue: 2 };\n  return Object.values(values).includes(key);\n}\n",
        ),
        (
            "object_entries.js",
            "function f(key, other) {\n  const values = { red: 1, blue: 2 };\n  return Object.entries(values).includes(key);\n}\n",
        ),
        (
            "object_shadowed.js",
            "function f(Object, key, other) {\n  const values = { red: 1, blue: 2 };\n  return Object.keys(values).includes(key);\n}\n",
        ),
        (
            "object_proto_key.js",
            "function f(key, other) {\n  const values = { __proto__: null, red: 1 };\n  return Object.keys(values).includes(key);\n}\n",
        ),
        (
            "object_escaped_proto_key.js",
            "function f(key, other) {\n  const values = { \\u005f\\u005fproto__: null, red: 1 };\n  return Object.keys(values).includes(key);\n}\n",
        ),
        (
            "object_numeric_key.js",
            "function f(key, other) {\n  const values = { 1.0: true, red: 1 };\n  return Object.keys(values).includes(key);\n}\n",
        ),
        (
            "object_mutated.js",
            "function f(key, other) {\n  const values = { red: 1, blue: 2 };\n  values.green = 3;\n  return Object.keys(values).includes(key);\n}\n",
        ),
        (
            "object_delete_mutation.js",
            "function f(key, other) {\n  const values = { red: 1, blue: 2 };\n  delete values.red;\n  return Object.keys(values).includes(key);\n}\n",
        ),
        (
            "object_alias_mutated.js",
            "function f(key, other) {\n  const values = { red: 1, blue: 2 };\n  const alias = values;\n  alias.green = 3;\n  return Object.keys(values).includes(key);\n}\n",
        ),
        (
            "object_receiver_call.js",
            "function f(key, other) {\n  const values = { red: 1, blue: 2 };\n  values.clear();\n  return Object.keys(values).includes(key);\n}\n",
        ),
        (
            "object_hoisted_mutator.js",
            "function f(key, other) {\n  const values = { red: 1, blue: 2 };\n  mutate();\n  return Object.keys(values).includes(key);\n  function mutate() { values.green = 3; }\n}\n",
        ),
        (
            "object_direct_eval.js",
            "function f(key, other) {\n  const values = { red: 1, blue: 2 };\n  eval(\"values.green = 3\");\n  return Object.keys(values).includes(key);\n}\n",
        ),
        (
            "object_with_scope_delete.js",
            "function f(key, other) {\n  const values = { red: 1, blue: 2 };\n  with (values) { delete red; }\n  return Object.keys(values).includes(key);\n}\n",
        ),
        (
            "object_enclosing_with_scope.js",
            "function f(key, other) {\n  const values = { values: { red: 1 }, blue: 2 };\n  with (values) { return Object.keys(values).includes(key); }\n}\n",
        ),
        (
            "object_for_in_target_mutation.js",
            "function f(key, other) {\n  const values = { red: 1, blue: 2 };\n  for (values.green in { green: 1 }) {}\n  return Object.keys(values).includes(key);\n}\n",
        ),
        (
            "object_for_of_target_mutation.js",
            "function f(key, other) {\n  const values = { red: 1, blue: 2 };\n  for (values.green of [\"green\"]) {}\n  return Object.keys(values).includes(key);\n}\n",
        ),
        (
            "object_conditional_initializer.js",
            "function f(flag, key, other) {\n  if (flag) { var values = { red: 1, blue: 2 }; }\n  return Object.keys(values).includes(key);\n}\n",
        ),
        (
            "object_parameter_shadow.js",
            "const values = { red: 1, blue: 2 };\nfunction f(values, key, other) {\n  return Object.keys(values).includes(key);\n}\n",
        ),
    ] {
        fs::write(dir.join(name), src).unwrap();
    }
}
