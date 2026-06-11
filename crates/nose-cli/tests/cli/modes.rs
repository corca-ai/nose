use super::*;

#[test]
fn default_mode_runs_syntax_semantic_and_near() {
    let dir = make_mode_project("default_modes");
    let p = dir.to_str().unwrap();
    let out = run(&["scan", p, "--min-size", "12", "--format", "json"]);
    assert!(
        out.contains("copy_a.py") && out.contains("copy_b.py"),
        "default mode includes syntax: {out}"
    );
    assert!(
        out.contains("renamed_a.py") && out.contains("renamed_b.py"),
        "default mode includes semantic: {out}"
    );
    // §BM / #241: the default surface includes unthresholded `near`.
    assert!(
        out.contains("near_a.py") && out.contains("near_b.py"),
        "default mode includes near: {out}"
    );
    // An explicit --mode replaces the default, so the near pair drops out.
    let repeated = run(&[
        "scan",
        p,
        "--mode",
        "syntax",
        "--mode",
        "semantic",
        "--min-size",
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
    assert!(
        !repeated.contains("near_a.py"),
        "explicit --mode replaces the default; near must not run: {repeated}"
    );
    let _ = fs::remove_dir_all(&dir);
}

#[test]
fn non_near_scan_modes_reject_similarity_thresholds() {
    let dir = make_mode_project("exact_threshold");
    // Exact channels carry no inline threshold; `syntax:0.5` / `semantic:0.5` are invalid.
    for mode in ["syntax:0.5", "semantic:0.5"] {
        let out = Command::new(bin())
            .args(["scan", dir.to_str().unwrap(), "--mode", mode])
            .output()
            .expect("run nose");
        assert!(
            !out.status.success(),
            "{mode} must not accept a fuzzy similarity threshold"
        );
        let stderr = String::from_utf8(out.stderr).unwrap();
        assert!(
            stderr.contains("unknown mode"),
            "specific error explains the invalid threshold for {mode}: {stderr}"
        );
        assert!(
            stderr.contains("abstraction:T"),
            "unknown-mode help should include the hidden abstraction threshold spelling: {stderr}"
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
            "near:0.5",
            "--min-size",
            "12",
            "--format",
            "json",
        ])
        .output()
        .expect("run nose");
    assert!(
        out.status.success(),
        "near mode should accept inline near:T thresholds: {}",
        String::from_utf8_lossy(&out.stderr)
    );
    let _ = fs::remove_dir_all(&dir);
}

#[test]
fn abstraction_scan_mode_accepts_similarity_threshold() {
    let dir = make_mode_project("abstraction_threshold");
    let out = Command::new(bin())
        .args([
            "scan",
            dir.to_str().unwrap(),
            "--mode",
            "abstraction:0.5",
            "--min-size",
            "12",
            "--format",
            "json",
        ])
        .output()
        .expect("run nose");
    assert!(
        out.status.success(),
        "abstraction mode should accept inline abstraction:T thresholds: {}",
        String::from_utf8_lossy(&out.stderr)
    );
    let _ = fs::remove_dir_all(&dir);
}

#[test]
fn combined_fuzzy_modes_accept_one_shared_threshold() {
    let dir = make_mode_project("shared_threshold");
    let out = Command::new(bin())
        .args([
            "scan",
            dir.to_str().unwrap(),
            "--mode",
            "near:0.5,abstraction:0.5",
            "--min-size",
            "12",
            "--format",
            "json",
        ])
        .output()
        .expect("run nose");
    assert!(
        out.status.success(),
        "near and abstraction should accept the same shared threshold: {}",
        String::from_utf8_lossy(&out.stderr)
    );
    let _ = fs::remove_dir_all(&dir);
}

#[test]
fn combined_fuzzy_modes_reject_conflicting_thresholds() {
    let dir = make_mode_project("conflicting_thresholds");
    let out = Command::new(bin())
        .args([
            "scan",
            dir.to_str().unwrap(),
            "--mode",
            "near:0.8,abstraction:0.5",
        ])
        .output()
        .expect("run nose");
    assert!(
        !out.status.success(),
        "conflicting near/abstraction thresholds should not silently overwrite each other"
    );
    let stderr = String::from_utf8(out.stderr).unwrap();
    assert!(
        stderr.contains("conflicting --mode thresholds")
            && stderr.contains("share one acceptance threshold"),
        "error should explain the shared fuzzy threshold: {stderr}"
    );
    let _ = fs::remove_dir_all(&dir);
}

#[test]
fn detect_threshold_rejects_nan() {
    let dir = make_mode_project("detect_nan_threshold");
    let out = Command::new(bin())
        .args([
            "detect",
            dir.to_str().unwrap(),
            "--threshold",
            "NaN",
            "--candidates",
        ])
        .output()
        .expect("run nose");
    assert!(
        !out.status.success(),
        "detect --threshold NaN must be rejected instead of disabling every comparison"
    );
    let stderr = String::from_utf8(out.stderr).unwrap();
    assert!(
        stderr.contains("threshold must be a finite number in [0,1]"),
        "stderr should explain the invalid threshold: {stderr}"
    );
    let _ = fs::remove_dir_all(&dir);
}

#[test]
fn detect_rejects_zero_lsh_parameters() {
    let dir = make_mode_project("detect_zero_lsh");
    for (flag, message) in [
        ("--minhash-k", "minhash-k must be positive"),
        ("--bands", "bands must be positive"),
    ] {
        let out = Command::new(bin())
            .args(["detect", dir.to_str().unwrap(), "--candidates", flag, "0"])
            .output()
            .expect("run nose");
        assert!(
            !out.status.success(),
            "{flag} 0 must be rejected instead of disabling LSH candidates"
        );
        let stderr = String::from_utf8(out.stderr).unwrap();
        assert!(
            stderr.contains(message),
            "stderr should explain the invalid {flag}: {stderr}"
        );
    }
    let _ = fs::remove_dir_all(&dir);
}

#[test]
fn abstraction_mode_reports_numeric_literal_witness() {
    let dir = std::env::temp_dir().join(format!("nose_abstraction_{}", std::process::id()));
    let _ = fs::remove_dir_all(&dir);
    fs::create_dir_all(dir.join("a")).unwrap();
    fs::create_dir_all(dir.join("b")).unwrap();
    fs::write(
        dir.join("a/sum.py"),
        "def sum_int(xs):\n    total = 0\n    for x in xs:\n        total = total + x\n    return total\n",
    )
    .unwrap();
    fs::write(
        dir.join("b/sum.py"),
        "def sum_float(xs):\n    total = 0.0\n    for x in xs:\n        total = total + x\n    return total\n",
    )
    .unwrap();

    let semantic = scan_json(&run(&[
        "scan",
        dir.to_str().unwrap(),
        "--mode",
        "semantic",
        "--min-size",
        "8",
        "--format",
        "json",
        "--top",
        "0",
    ]));
    assert!(
        !family_contains_all(&semantic, &["a/sum.py", "b/sum.py"]),
        "int/float literal seeds must not become exact semantic clones: {semantic}"
    );

    let abstraction = scan_json(&run(&[
        "scan",
        dir.to_str().unwrap(),
        "--mode",
        "abstraction",
        "--min-size",
        "8",
        "--format",
        "json",
        "--top",
        "0",
    ]));
    let family = family_with_all(&abstraction, &["a/sum.py", "b/sum.py"])
        .unwrap_or_else(|| panic!("abstraction mode should report the pair: {abstraction}"));
    let witness = &family["abstraction_witness"];
    assert_eq!(witness["claim"], "weak-refactoring-template");
    assert_eq!(witness["basis"], "family");
    assert_eq!(witness["members_checked"], 2);
    assert_eq!(witness["reason_code"], "type-parametric");
    assert_eq!(witness["template_format"], "normalized-il-preorder");
    assert_eq!(witness["holes"][0]["kind"], "literal");
    assert_eq!(witness["holes"][0]["role"], "leaf");
    assert_eq!(
        witness["holes"][0]["observed"]
            .as_array()
            .map(|values| values.len()),
        Some(2)
    );
    assert!(
        witness["holes"][0]["template_index"].as_u64().is_some(),
        "hole should point back into the normalized template: {witness}"
    );
    assert_eq!(witness["holes"][0]["left"], "int-literal");
    assert_eq!(witness["holes"][0]["right"], "float-literal");
    assert!(
        witness["caveats"]
            .as_array()
            .is_some_and(|caveats| caveats.iter().any(|c| c == "numeric-domain-sensitive")),
        "numeric caveat should be explicit: {witness}"
    );
    assert!(
        witness["template"]
            .as_array()
            .is_some_and(|template| template.iter().any(|t| t == "<hole 1: literal>")),
        "template should mark the typed hole: {witness}"
    );
    let _ = fs::remove_dir_all(&dir);
}

#[test]
fn abstraction_mode_reports_same_class_literal_without_numeric_caveat() {
    let dir = std::env::temp_dir().join(format!("nose_abstraction_lit_{}", std::process::id()));
    let _ = fs::remove_dir_all(&dir);
    fs::create_dir_all(dir.join("a")).unwrap();
    fs::create_dir_all(dir.join("b")).unwrap();
    fs::write(
        dir.join("a/scale.py"),
        "def scale(xs):\n    total = 0\n    for x in xs:\n        total = total + x * 2\n    return total\n",
    )
    .unwrap();
    fs::write(
        dir.join("b/scale.py"),
        "def scale_more(xs):\n    total = 0\n    for x in xs:\n        total = total + x * 3\n    return total\n",
    )
    .unwrap();

    let abstraction = scan_json(&run(&[
        "scan",
        dir.to_str().unwrap(),
        "--mode",
        "abstraction",
        "--min-size",
        "8",
        "--format",
        "json",
        "--top",
        "0",
    ]));
    let family =
        family_with_all(&abstraction, &["a/scale.py", "b/scale.py"]).unwrap_or_else(|| {
            panic!("abstraction mode should report the literal pair: {abstraction}")
        });
    let witness = &family["abstraction_witness"];
    assert_eq!(witness["basis"], "family");
    assert_eq!(witness["members_checked"], 2);
    assert_eq!(witness["reason_code"], "literal-abstracted");
    assert_eq!(witness["holes"][0]["role"], "leaf");
    assert_eq!(witness["holes"][0]["left"], "int-literal");
    assert_eq!(witness["holes"][0]["right"], "int-literal");
    assert_eq!(witness["holes"][0]["observed"][0], "int-literal");
    assert!(
        witness["caveats"].as_array().is_some_and(|c| c.is_empty()),
        "same-class literal abstraction should not invent a numeric-domain caveat: {witness}"
    );
    let _ = fs::remove_dir_all(&dir);
}

#[test]
fn abstraction_mode_rejects_operator_swap_witnesses() {
    let dir = std::env::temp_dir().join(format!("nose_abstraction_op_{}", std::process::id()));
    let _ = fs::remove_dir_all(&dir);
    fs::create_dir_all(dir.join("a")).unwrap();
    fs::create_dir_all(dir.join("b")).unwrap();
    let mk = |op: &str| {
        format!("def fold(xs):\n    total = 1\n    for x in xs:\n        total = total {op} x\n    return total\n")
    };
    fs::write(dir.join("a/fold.py"), mk("+")).unwrap();
    fs::write(dir.join("b/fold.py"), mk("*")).unwrap();

    let abstraction = scan_json(&run(&[
        "scan",
        dir.to_str().unwrap(),
        "--mode",
        "abstraction",
        "--min-size",
        "8",
        "--format",
        "json",
        "--top",
        "0",
    ]));
    assert!(
        !family_contains_all(&abstraction, &["a/fold.py", "b/fold.py"]),
        "operator swaps are behavioral diffs, not abstraction witnesses: {abstraction}"
    );
    let _ = fs::remove_dir_all(&dir);
}
