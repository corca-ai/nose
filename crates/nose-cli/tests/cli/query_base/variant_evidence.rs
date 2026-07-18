use super::*;

fn target_with_files<'a>(
    json: &'a serde_json::Value,
    changed: &str,
    skipped: &str,
) -> &'a serde_json::Value {
    query_base_items(json)
        .iter()
        .flat_map(|item| item["targets"].as_array().into_iter().flatten())
        .find(|target| target["changed"]["file"] == changed && target["skipped"]["file"] == skipped)
        .unwrap_or_else(|| panic!("expected target {changed} -> {skipped}: {json}"))
}

fn signal_codes(target: &serde_json::Value) -> Vec<&str> {
    target["variant_evidence"]["signals"]
        .as_array()
        .into_iter()
        .flatten()
        .filter_map(|signal| signal["code"].as_str())
        .collect()
}

#[test]
fn query_base_names_definition_decorator_mismatch_without_changing_v2_gate() {
    let project = GitProject::new("query_base_variant_decorator");
    let body = |decorator: &str, name: &str| {
        format!(
            "@route(\"{decorator}\")\ndef {name}(values):\n    total = 0\n    for value in values:\n        if value > 0:\n            total = total + value * 2\n    return total\n"
        )
    };
    project.write("a.py", &body("alpha", "first"));
    project.write("b.py", &body("beta", "second"));
    project.init();
    project.write(
        "a.py",
        &body("alpha", "first").replace("    return total", "    return total + 1"),
    );

    let json = query_base_json_value(project.path(), &[]);
    let target = target_with_files(&json, "a.py", "b.py");
    assert_eq!(target["variant_evidence"]["status"], "disqualifying");
    assert!(signal_codes(target).contains(&"decorator-mismatch"));
    let item = query_base_items(&json)
        .iter()
        .find(|item| {
            item["targets"]
                .as_array()
                .is_some_and(|targets| targets.contains(target))
        })
        .expect("owning divergence");
    assert_eq!(
        item["gate"]["fail_default"], true,
        "#851 records evidence but leaves the v2 policy unchanged: {json}"
    );
}

#[test]
fn query_base_names_pair_local_referent_mismatch() {
    let project = GitProject::new("query_base_variant_referent");
    let body = |delta: i32, name: &str| {
        format!(
            "def handler(value):\n    return value + {delta}\n\ndef {name}(values):\n    total = 0\n    for value in values:\n        if value > 0:\n            total = total + handler(value)\n    return total\n"
        )
    };
    project.write("a.py", &body(1, "first"));
    project.write("b.py", &body(2, "second"));
    project.init();
    project.write(
        "a.py",
        &body(1, "first").replace("    return total", "    return total + 1"),
    );

    let json = query_base_json_value(project.path(), &[]);
    let target = target_with_files(&json, "a.py", "b.py");
    assert_eq!(target["variant_evidence"]["status"], "disqualifying");
    let referent = target["variant_evidence"]["signals"]
        .as_array()
        .unwrap()
        .iter()
        .find(|signal| signal["code"] == "referent-mismatch")
        .unwrap_or_else(|| panic!("referent mismatch should be inspectable: {json}"));
    assert_eq!(referent["strength"], "strong");
    assert!(
        referent["changed"]
            .as_array()
            .is_some_and(|names| names.iter().any(|name| name == "handler")),
        "the mismatched referent is named: {json}"
    );
}
