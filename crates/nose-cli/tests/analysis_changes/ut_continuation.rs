use super::Project;

#[test]
fn json_baseline_acceptance_has_a_receipt_for_the_persisted_population() {
    let p = Project::new();
    let common = [
        "query",
        ".",
        "--mode",
        "semantic",
        "--min-size",
        "1",
        "--min-lines",
        "1",
        "--baseline",
        "accepted.json",
        "--format",
        "json",
    ];
    let mut write = common.to_vec();
    write.push("--write-baseline");
    let receipt = p.json(&write);
    assert_eq!(receipt["schema"], "nose.baseline-write/v1");
    assert_eq!(receipt["file"], "accepted.json");
    let baseline: serde_json::Value =
        serde_json::from_slice(&std::fs::read(p.0.join("accepted.json")).unwrap()).unwrap();
    assert!(receipt["families"].as_u64().unwrap() > 0);
    assert_eq!(
        receipt["families"].as_u64().unwrap() as usize,
        baseline["families"].as_array().unwrap().len()
    );
    let mut gate = common.to_vec();
    gate.extend(["--fail-on", "new"]);
    let unchanged = p.json(&gate);
    assert!(unchanged["families"].as_array().unwrap().is_empty());
}

#[test]
fn higher_work_retry_preserves_the_original_gate_and_selection() {
    let p = Project::new();
    p.write("c.py", super::SOURCE);
    let failed = p.run(&[
        "query",
        ".",
        "--mode",
        "semantic",
        "--min-size",
        "1",
        "--min-lines",
        "1",
        "--max-candidate-pairs",
        "1",
        "scope=prod",
        "--fail-on",
        "any",
        "--format",
        "json",
    ]);
    assert!(!failed.status.success());
    assert!(failed.stdout.is_empty());
    let error = String::from_utf8(failed.stderr).unwrap();
    assert!(error.contains("Analysis incomplete"));
    let retry = error
        .lines()
        .map(str::trim)
        .rfind(|line| line.starts_with("nose query "))
        .unwrap();
    assert!(retry.contains("'--fail-on' 'any'"), "{retry}");
    assert!(retry.contains("'scope=prod'"), "{retry}");
    // Authorize ample work in this small fixture; the test concerns gate recovery, not doubling policy.
    let budget = retry
        .split("'--max-candidate-pairs' '")
        .nth(1)
        .unwrap()
        .split("'")
        .next()
        .unwrap();
    let retry = retry.replace(
        &format!("'--max-candidate-pairs' '{budget}'"),
        "'--max-candidate-pairs' '1000000'",
    );
    let retry = retry.replacen("nose query ", "\"$NOSE_UT_BINARY\" query ", 1);
    let result = std::process::Command::new("sh")
        .current_dir(&p.0)
        .env("NOSE_UT_BINARY", env!("CARGO_BIN_EXE_nose"))
        .args(["-c", &retry])
        .output()
        .unwrap();
    assert_eq!(result.status.code(), Some(1));
    let output: serde_json::Value = serde_json::from_slice(&result.stdout).unwrap();
    assert_eq!(output["analysis"]["scanned_files"], 3);
    assert!(!output["families"].as_array().unwrap().is_empty());
}
