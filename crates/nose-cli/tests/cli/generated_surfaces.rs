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
    let _ = fs::remove_dir_all(&dir);
}
