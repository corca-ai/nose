use super::*;

#[test]
fn query_jazzy_provenance_is_reason_coded_and_recoverable() {
    let dir = make_jazzy_generated_project("jazzy_generated");
    let args = [
        "query",
        dir.to_str().unwrap(),
        "--mode",
        "semantic",
        "--min-size",
        "12",
    ];
    let default = run(&args);
    assert!(
        default.contains("generated-code") && !default.contains("a/index.html"),
        "the default human surface explains but does not list Jazzy output: {default}"
    );

    let mut all_args = args.to_vec();
    all_args.extend(["all", "top=0", "--format", "json"]);
    let all = query_json(&run(&all_args));
    let families = query_families(&all);
    assert!(
        !families.is_empty(),
        "full JSON retains Jazzy families: {all}"
    );
    assert!(
        families
            .iter()
            .all(|family| family["surface"] == "generated"),
        "every retained source-coherent Jazzy family is reason-coded generated: {all}"
    );
    assert!(families.iter().all(|family| {
        family["locations"]
            .as_array()
            .is_some_and(|locations| locations.len() >= 2)
    }));
    let _ = fs::remove_dir_all(&dir);
}

#[test]
fn query_declared_generator_provenance_is_reason_coded_and_recoverable() {
    let dir = make_declared_generator_project("declared_generator");
    let args = [
        "query",
        dir.to_str().unwrap(),
        "--mode",
        "semantic",
        "--min-size",
        "12",
    ];
    let default = run(&args);
    assert!(
        default.contains("generated-code") && !default.contains("a/index.html"),
        "the default human surface explains but does not list declared generated output: {default}"
    );

    let mut all_args = args.to_vec();
    all_args.extend(["all", "top=0", "--format", "json"]);
    let all = query_json(&run(&all_args));
    let families = query_families(&all);
    assert!(
        !families.is_empty(),
        "full JSON retains declared generated families: {all}"
    );
    assert!(
        families
            .iter()
            .all(|family| family["surface"] == "generated"),
        "every source-coherent declared generated family is reason-coded generated: {all}"
    );
    assert!(families.iter().all(|family| {
        family["generated_provenance"]
            == serde_json::json!({"basis": "all-members", "sources": ["nose-inferred"]})
    }));
    let _ = fs::remove_dir_all(&dir);
}

#[test]
fn caller_generated_paths_are_additive_recoverable_and_surface_only() {
    let dir = make_project("caller_generated_paths");
    let root = dir.to_str().unwrap();
    let base = query_json(&run(&[
        "query", root, "--mode", "semantic", "all", "top=0", "--format", "json",
    ]));
    let base_family = project_clone_family(&base).clone();

    let partial = query_json(&run(&[
        "query",
        root,
        "--mode",
        "semantic",
        "--generated-path",
        "a/**",
        "all",
        "top=0",
        "--format",
        "json",
    ]));
    let partial_family = project_clone_family(&partial);
    assert_eq!(partial_family["surface"], base_family["surface"]);
    assert!(partial_family.get("generated_provenance").is_none());

    let config = dir.join("caller.toml");
    fs::write(&config, "[query]\ngenerated-paths = [\"a/**\"]\n").unwrap();
    let asserted = query_json(&run(&[
        "query",
        root,
        "--mode",
        "semantic",
        "--config",
        config.to_str().unwrap(),
        "--generated-path",
        "b/**",
        "--generated-path",
        "tests/**",
        "all",
        "top=0",
        "--format",
        "json",
    ]));
    let asserted_family = project_clone_family(&asserted);
    assert_eq!(asserted_family["surface"], "generated");
    assert_eq!(
        asserted_family["generated_provenance"],
        serde_json::json!({"basis": "all-members", "sources": ["caller-path"]})
    );

    let mut before = base_family;
    let mut after = asserted_family.clone();
    before.as_object_mut().unwrap().remove("surface");
    before
        .as_object_mut()
        .unwrap()
        .remove("recommended_surface");
    after.as_object_mut().unwrap().remove("surface");
    after.as_object_mut().unwrap().remove("recommended_surface");
    after
        .as_object_mut()
        .unwrap()
        .remove("generated_provenance");
    assert_eq!(
        after, before,
        "caller assertions change only surface metadata"
    );

    let human = run(&[
        "query",
        root,
        "--mode",
        "semantic",
        "--config",
        config.to_str().unwrap(),
        "--generated-path",
        "b/**",
        "--generated-path",
        "tests/**",
    ]);
    assert!(human.contains("generated-code"));
    assert!(!human.contains("a/f.py"));
    let _ = fs::remove_dir_all(&dir);
}

#[test]
fn caller_generated_paths_reject_invalid_and_base_view_inputs() {
    let dir = make_project("caller_generated_path_errors");
    let root = dir.to_str().unwrap();
    let invalid = run_fail(&["query", root, "--generated-path", "../outside/**", "top=0"]);
    assert!(invalid.contains("invalid generated-path glob"), "{invalid}");

    let invalid_config = dir.join("invalid.toml");
    fs::write(
        &invalid_config,
        "[query]\ngenerated-paths = [\"!generated/**\"]\n",
    )
    .unwrap();
    let invalid = run_fail(&[
        "query",
        root,
        "--config",
        invalid_config.to_str().unwrap(),
        "top=0",
    ]);
    assert!(invalid.contains("invalid generated-path glob"), "{invalid}");

    let base = run_fail(&["query", root, "--generated-path", "a/**", "base=HEAD"]);
    assert!(base.contains("does not support --generated-path"), "{base}");

    let base_config = dir.join("base.toml");
    fs::write(
        &base_config,
        "[query]\ngenerated-paths = [\"generated/**\"]\n",
    )
    .unwrap();
    let base = run_fail(&[
        "query",
        root,
        "--config",
        base_config.to_str().unwrap(),
        "base=HEAD",
    ]);
    assert!(
        base.contains("does not support generated-paths config"),
        "{base}"
    );
    let _ = fs::remove_dir_all(&dir);
}

#[test]
fn caller_generated_paths_apply_independently_to_every_query_root() {
    let first = make_project("caller_generated_multi_first");
    let second = make_project("caller_generated_multi_second");
    let report = query_json(&run(&[
        "query",
        "--root",
        first.to_str().unwrap(),
        "--root",
        second.to_str().unwrap(),
        "--mode",
        "semantic",
        "--generated-path",
        "a/**",
        "--generated-path",
        "b/**",
        "--generated-path",
        "tests/**",
        "all",
        "top=0",
        "--format",
        "json",
    ]));
    let family = query_families(&report)
        .iter()
        .find(|family| {
            family["members"]
                .as_u64()
                .is_some_and(|members| members >= 6)
        })
        .expect("the cross-root clone family");
    assert_eq!(family["surface"], "generated");
    assert_eq!(
        family["generated_provenance"],
        serde_json::json!({"basis": "all-members", "sources": ["caller-path"]})
    );
    let _ = fs::remove_dir_all(first);
    let _ = fs::remove_dir_all(second);
}

fn project_clone_family(report: &serde_json::Value) -> &serde_json::Value {
    query_families(report)
        .iter()
        .find(|family| {
            let files = family["locations"]
                .as_array()
                .into_iter()
                .flatten()
                .filter_map(|location| location["file"].as_str())
                .collect::<Vec<_>>();
            ["a/f.py", "b/f.py", "tests/f.py"]
                .iter()
                .all(|suffix| files.iter().any(|file| file.ends_with(suffix)))
        })
        .expect("the three-copy fixture family")
}
