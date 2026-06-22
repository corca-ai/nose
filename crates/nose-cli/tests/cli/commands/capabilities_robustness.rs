use super::*;

#[test]
fn version_flag_works() {
    let out = run(&["--version"]);
    assert!(out.starts_with("nose "), "version line: {out}");
}

#[test]
fn capabilities_command_emits_machine_readable_contract() {
    let out = run(&["capabilities"]);
    let json: serde_json::Value =
        serde_json::from_str(&out).expect("capabilities must emit valid JSON");

    assert_eq!(json["schema_version"], 4);
    assert_eq!(json["tool"]["name"], "nose");
    assert_eq!(json["tool"]["version"], env!("CARGO_PKG_VERSION"));
    assert!(
        json["platform"]["os"]
            .as_str()
            .is_some_and(|s| !s.is_empty()),
        "platform.os should be non-empty: {out}"
    );
    assert!(
        json["platform"]["arch"]
            .as_str()
            .is_some_and(|s| !s.is_empty()),
        "platform.arch should be non-empty: {out}"
    );
    assert_eq!(json["interfaces"]["capabilities_json"], true);
    assert_eq!(json["interfaces"]["version_json"], false);
    assert_eq!(json["interfaces"]["doctor_json"], false);
}

#[test]
fn capabilities_command_lists_stable_commands_and_schemas() {
    let out = run(&["capabilities"]);
    let json: serde_json::Value =
        serde_json::from_str(&out).expect("capabilities must emit valid JSON");

    assert_eq!(
        json_array_strings(&json["commands"], "stable"),
        vec!["capabilities", "il", "query", "semantic-pack", "stats"]
    );
    assert!(json_array_strings(&json["commands"], "deprecated").is_empty());
    assert_eq!(json["schemas"]["capabilities"][0], 4);
    assert_eq!(json["schemas"]["query_json"][0], 6);
    assert_eq!(
        json["schemas"]["semantic_packs"][0],
        "nose.semantic-pack.v0"
    );
    assert_eq!(json["schemas"]["semantic_pack_conformance"][0], 2);
    assert_eq!(json["schemas"]["semantic_pack_inventory"][0], 1);
    assert_eq!(json["schemas"]["semantic_pack_adoption_gates"][0], 1);
    assert_eq!(json["schemas"]["semantic_pack_compatibility"][0], 1);
}

#[test]
fn capabilities_command_reports_query_surface() {
    let out = run(&["capabilities"]);
    let json: serde_json::Value =
        serde_json::from_str(&out).expect("capabilities must emit valid JSON");

    assert_eq!(
        json_array_strings(&json["query"], "modes"),
        vec!["syntax", "semantic", "near"]
    );
    assert_eq!(
        json_array_strings(&json["query"], "default_modes"),
        vec!["syntax", "semantic", "near"]
    );
    assert_eq!(
        json_array_strings(&json["query"], "output_formats"),
        vec!["human", "json", "markdown", "sarif"]
    );
    assert_eq!(
        json_array_strings(&json["query"], "sort_keys"),
        vec!["extractability", "value", "sites", "hazard"]
    );
    assert_eq!(json["query"]["capabilities"]["baseline"], true);
    assert_eq!(
        json["query"]["capabilities"]["baseline_member_digest"],
        true
    );
    assert_eq!(json["query"]["capabilities"]["multi_root"], true);
    assert_eq!(json["query"]["capabilities"]["semantic_pack_loading"], true);
    assert_eq!(json["query"]["capabilities"]["structured_ignores"], true);
}

#[test]
fn capabilities_command_reports_semantic_pack_il_and_stats_surfaces() {
    let out = run(&["capabilities"]);
    let json: serde_json::Value =
        serde_json::from_str(&out).expect("capabilities must emit valid JSON");

    assert_eq!(
        json["semantic_packs"]["api_versions"][0],
        "nose.semantic-pack.v0"
    );
    assert_eq!(
        json["semantic_packs"]["external_pack_influence"],
        "metadata-only"
    );
    assert_eq!(
        json_array_strings(&json["semantic_packs"], "external_influence_blockers"),
        vec![
            "data-only-registration",
            "dependency-backed-evidence-unavailable",
            "explicit-influence-trust-gate-missing",
            "executable-conformance-unavailable",
            "row-conflict"
        ]
    );
    assert_eq!(json["semantic_packs"]["external_pack_execution"], "none");
    assert_eq!(
        json_array_strings(&json["semantic_packs"], "conformance"),
        vec!["local-manifest-file", "local-manifest-directory"]
    );
    assert_eq!(
        json_array_strings(&json["semantic_packs"], "conformance_output_formats"),
        vec!["human", "json"]
    );
    assert_eq!(
        json_array_strings(&json["semantic_packs"], "inventory"),
        vec!["compiled-builtin"]
    );
    assert_eq!(
        json_array_strings(&json["semantic_packs"], "inventory_output_formats"),
        vec!["human", "json"]
    );
    assert_eq!(
        json_array_strings(&json["semantic_packs"], "adoption_gates"),
        vec!["compiled-builtin"]
    );
    assert_eq!(
        json_array_strings(&json["semantic_packs"], "adoption_gate_output_formats"),
        vec!["human", "json"]
    );
    assert_eq!(
        json_array_strings(&json["semantic_packs"], "compatibility"),
        vec!["policy"]
    );
    assert_eq!(
        json_array_strings(&json["semantic_packs"], "compatibility_output_formats"),
        vec!["human", "json"]
    );
    assert_eq!(
        json_array_strings(&json["semantic_packs"], "loading"),
        vec![
            "compiled-builtin",
            "local-manifest-file",
            "local-manifest-directory"
        ]
    );
    assert_eq!(
        json_array_strings(&json["semantic_packs"], "trust"),
        vec!["builtin-default", "builtin-optional", "external-opt-in"]
    );
    assert_eq!(
        json["semantic_packs"]["external_packs_enabled_by_default"],
        false
    );
    assert_eq!(
        json_array_strings(&json["il"], "output_formats"),
        vec!["sexpr", "json"]
    );
    assert_eq!(
        json_array_strings(&json["stats"], "output_formats"),
        vec!["human", "json"]
    );
}

#[test]
fn broken_pipe_exits_cleanly() {
    // `nose query … | head` (or quitting a pager) closes the read end of stdout while
    // nose is still writing. That write then fails with `BrokenPipe`, which `println!`
    // turns into a panic — the `failed printing to stdout` crash users hit. The Unix
    // convention is to stop quietly; main()'s panic hook must catch this and exit 0.
    // We reproduce it by closing the pipe's read end outright, so every write to the
    // now-readerless pipe gets EPIPE.
    let dir = std::env::temp_dir().join(format!("nose_pipe_{}", std::process::id()));
    let _ = fs::remove_dir_all(&dir);
    // A handful of large, distinct clone families -> plenty of markdown output, so the child
    // keeps writing past any buffering and reliably hits the dead pipe.
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
            "query",
            dir.to_str().unwrap(),
            "all",
            "top=40",
            "--mode",
            "near:0.5",
            "--format",
            "markdown",
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
    let _ = run(&["query", dir.to_str().unwrap()]);
    let _ = fs::remove_dir_all(&dir);
}

#[test]
fn recursive_hof_callback_fragment_does_not_overflow() {
    // Regression for the rxjs parser abort: when extracting sub-function units inside a
    // recursive helper, the value graph must not register the enclosing function as a pure inline
    // target and inline the helper through its own reduce callback forever.
    let dir = std::env::temp_dir().join(format!("nose_recursive_hof_{}", std::process::id()));
    let _ = fs::remove_dir_all(&dir);
    fs::create_dir_all(&dir).unwrap();
    fs::write(
        dir.join("recursive.ts"),
        "export function recInLambda(xs: any[]): number {\n  return xs.reduce((acc, x) => recInLambda(x), 0);\n}\n",
    )
    .unwrap();
    let out = run(&[
        "query",
        dir.to_str().unwrap(),
        "--mode",
        "semantic",
        "--format",
        "json",
        "top=0",
    ]);
    let json = query_json(&out);
    assert_eq!(json["schema_version"], 6);
    let _ = fs::remove_dir_all(&dir);
}
