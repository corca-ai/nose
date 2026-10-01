use super::{Project, Value};

#[test]
fn first_review_capture_replays_detection_without_reporting_suppressions() {
    let p = Project::new();
    p.write("accepted.json", r#"{"schema_version":2,"tool":"nose","baseline_kind":"accepted-duplication","families":[]}"#);
    p.write("ignored.json", r#"{"ignores":[]}"#);
    let options = [
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
        "--ignore-file",
        "ignored.json",
    ];
    let mut list_args = options.to_vec();
    list_args.extend(["top=1", "--format", "json"]);
    let list = p.json(&list_args);
    let id = format!("id={}", list["families"][0]["id"].as_str().unwrap());
    let mut open_args = options.to_vec();
    open_args.extend([&id, "full"]);
    let opened = p.run(&open_args);
    assert!(opened.status.success());
    let text = String::from_utf8(opened.stdout).unwrap();
    let command = text
        .lines()
        .find_map(|line| {
            line.trim()
                .strip_prefix("Start a caller review (save once; choose a new filename): ")
        })
        .unwrap();
    let binary = std::path::Path::new(env!("CARGO_BIN_EXE_nose"));
    let captured = std::process::Command::new("sh")
        .current_dir(&p.0)
        .env(
            "PATH",
            format!(
                "{}:{}",
                binary.parent().unwrap().display(),
                std::env::var("PATH").unwrap()
            ),
        )
        .args(["-c", command])
        .output()
        .unwrap();
    assert!(
        captured.status.success(),
        "{}",
        String::from_utf8_lossy(&captured.stderr)
    );
    let capture: Value =
        serde_json::from_slice(&std::fs::read(p.0.join("nose-analysis.json")).unwrap()).unwrap();
    assert!(capture["family_handles"]
        .get(list["families"][0]["id"].as_str().unwrap())
        .is_some());
}

#[test]
fn filtered_baseline_write_cannot_silently_accept_the_whole_population() {
    let p = Project::new();
    let out = p.run(&[
        "query",
        ".",
        "path~a.py",
        "--baseline",
        "accepted.json",
        "--write-baseline",
    ]);
    assert!(!out.status.success());
    assert!(!p.0.join("accepted.json").exists());
    assert!(String::from_utf8_lossy(&out.stderr).contains("structured ignore"));
    let whole = p.run(&[
        "query",
        ".",
        "--baseline",
        "accepted.json",
        "--write-baseline",
    ]);
    assert!(whole.status.success());
    assert!(p.0.join("accepted.json").exists());
}

#[test]
fn selected_change_explains_and_executes_caller_review_recording() {
    let p = Project::new();
    p.capture("before.json", &[]);
    p.capture("after.json", &[]);
    let list = p.compare(&[]);
    let change = format!("change={}", list["items"][0]["id"].as_str().unwrap());
    let detail = p.compare(&[&change]);
    let recording = &detail["review_recording"];
    assert_eq!(recording["available"], true);
    assert_eq!(
        recording["required_arguments"]["--decision"][0],
        "keep-separate"
    );
    assert!(!p.0.join("decision.json").exists());
    let command = format!(
        "{} --write-review decision.json --decision keep-separate --reason 'Independent ownership'",
        recording["command_prefix"].as_str().unwrap()
    );
    let written = p.follow(&Value::String(command));
    assert_eq!(written["reviews"]["written"], "decision.json");
    let reopened = p.compare(&[&change, "--reviews", "decision.json"]);
    assert_eq!(reopened["items"][0]["reviews"][0]["status"], "applicable");
}
