use super::{Project, Value};

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
