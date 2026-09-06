use super::*;

#[test]
fn live_family_handle_reopens_the_captured_observation_offline() {
    let p = Project::new();
    let live = p.json(&[
        "query",
        ".",
        "--mode",
        "semantic",
        "--min-size",
        "1",
        "--min-lines",
        "1",
        "--format",
        "json",
        "all",
        "top=0",
    ]);
    let family = &live["families"][0];
    let handle = family["id"].as_str().unwrap();
    let capture = p.capture("before.json", &[]);
    std::fs::copy(p.0.join("before.json"), p.0.join("after.json")).unwrap();
    let complete = p.compare(&["full", "top=0"]);
    for file in ["a.py", "b.py"] {
        std::fs::remove_file(p.0.join(file)).unwrap();
    }
    let selected = p.compare(&[&format!("id={}", &handle[..10]), "full"]);
    assert_eq!(selected["summary"]["selected"], 1);
    let item = &selected["items"][0];
    assert_eq!(item["id"], complete["items"][0]["id"]);
    assert_eq!(item["after"], complete["items"][0]["after"]);
    let detail = p.follow(&item["next"][0]);
    assert_eq!(detail["view"], "change");
    assert_eq!(detail["items"][0]["id"], item["id"]);
    for file in ["a.py", "b.py"] {
        p.write(file, SOURCE);
    }
    let source = &item["actions"][0]["command"];
    let verified = p.follow(source);
    assert_eq!(
        verified["items"][0]["source_lookup"],
        json!({"verified":4,"unavailable":0})
    );
    let recorded = p.follow(&json!(format!(
        "{} --write-review review.json --decision defer --reason 'Check caller contracts'",
        source.as_str().unwrap()
    )));
    let inspect = recorded["actions"]
        .as_array()
        .unwrap()
        .iter()
        .find(|a| a["kind"] == "inspect-review")
        .unwrap();
    let reviewed = p.follow(&inspect["command"]);
    assert_eq!(reviewed["items"][0]["review_status"], "applicable");
    // Legacy captures keep their evidence and observation addresses, but cannot
    // invent an old live handle that was never recorded.
    let mut legacy = capture;
    legacy.as_object_mut().unwrap().remove("family_handles");
    p.write("after.json", &legacy.to_string());
    let transferred = p.compare(&["--reviews", "review.json", "full"]);
    assert_eq!(transferred["items"][0]["review_status"], "applicable");
    for name in ["before.json", "after.json"] {
        p.write(name, &legacy.to_string());
    }
    let legacy_view = p.compare(&["full", "top=0"]);
    assert_eq!(legacy_view["items"], complete["items"]);
    let missing = p.run(&[
        "query",
        "--before",
        "before.json",
        "--after",
        "after.json",
        &format!("id={handle}"),
    ]);
    assert!(!missing.status.success());
    let error = String::from_utf8_lossy(&missing.stderr);
    assert!(
        error.contains("not recorded") && error.contains("path~"),
        "{error}"
    );
}

#[test]
fn ambiguous_handles_and_dangling_capture_references_fail_explicitly() {
    let p = Project::new();
    for file in ["c.py", "d.py"] {
        p.write(file, &SOURCE.replace("+ 7", "+ 19"));
    }
    let mut capture = p.capture("before.json", &[]);
    assert!(capture["families"].as_array().unwrap().len() > 1);
    let target = capture["families"][0]["id"].clone();
    capture["family_handles"] = json!({
        "aaaa000000000000": [target], "aaaa111111111111": [target]
    });
    let run = |value: &Value, handle: &str| {
        p.write("after.json", &value.to_string());
        p.run(&[
            "query",
            "--before",
            "after.json",
            "--after",
            "after.json",
            &format!("id={handle}"),
            "--format",
            "json",
        ])
    };
    let ambiguous = run(&capture, "aaaa");
    assert!(!ambiguous.status.success());
    assert!(String::from_utf8_lossy(&ambiguous.stderr).contains("ambiguous live family id"));
    let precise = run(&capture, "aaaa000000000000");
    assert!(precise.status.success());
    let selected: Value = serde_json::from_slice(&precise.stdout).unwrap();
    assert_eq!(selected["summary"]["selected"], 1);
    let mut targets: Vec<_> = capture["families"]
        .as_array()
        .unwrap()
        .iter()
        .map(|f| f["id"].as_str().unwrap().to_owned())
        .collect();
    targets.sort();
    capture["family_handles"]["aaaa000000000000"] = json!(targets);
    let collision = run(&capture, "aaaa000000000000");
    assert!(!collision.status.success());
    assert!(String::from_utf8_lossy(&collision.stderr).contains("multiple captured observations"));
    capture["family_handles"]["aaaa000000000000"] = json!(["0".repeat(64)]);
    let dangling = run(&capture, "aaaa000000000000");
    assert!(!dangling.status.success());
    assert!(String::from_utf8_lossy(&dangling.stderr).contains("observation reference"));
}
