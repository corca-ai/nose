use super::{Project, SOURCE};
use serde_json::{json, Value};

fn save_review(p: &Project, decision: &str, file: &str) {
    let view = p.json(&[
        "query",
        "--before",
        "before.json",
        "--after",
        "before.json",
        "--format",
        "json",
    ]);
    let change = format!("change={}", view["items"][0]["id"].as_str().unwrap());
    let output = p.run(&[
        "query",
        "--before",
        "before.json",
        "--after",
        "before.json",
        &change,
        "--write-review",
        file,
        "--decision",
        decision,
        "--reason",
        "Different owners retain independent policy",
    ]);
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
}
fn reviewed(p: &Project, extra: &[&str]) -> Value {
    let mut terms = vec!["--reviews", "review ' $.json", "full", "top=0"];
    terms.extend(extra);
    p.compare(&terms)
}

#[test]
fn review_survives_unique_move_but_rechecks_an_added_copy() {
    let p = Project::new();
    p.capture("before.json", &[]);
    save_review(&p, "keep-separate", "review ' $.json");
    std::fs::rename(p.0.join("b.py"), p.0.join("moved.py")).unwrap();
    p.capture("after.json", &[]);
    let view = reviewed(&p, &["review=applicable"]);
    assert_eq!(view["summary"]["selected"], 1, "{view}");
    assert_eq!(view["items"][0]["reviews"][0]["decision"], "keep-separate");
    let followed = p.follow(&view["items"][0]["next"][0]);
    assert_eq!(followed["items"][0]["review_status"], "applicable");
    p.write("c.py", SOURCE);
    p.capture("copies.json", &[]);
    let copies = p.json(&[
        "query",
        "--before",
        "before.json",
        "--after",
        "copies.json",
        "--reviews",
        "review ' $.json",
        "--format",
        "json",
        "top=0",
    ]);
    assert!(
        copies["items"]
            .as_array()
            .unwrap()
            .iter()
            .any(|r| r["review_status"] == "recheck"),
        "{copies}"
    );
    assert!(!copies["items"]
        .as_array()
        .unwrap()
        .iter()
        .any(|r| r["review_status"] == "applicable"));
}

#[test]
fn changed_source_and_incomplete_search_do_not_reuse_decisions() {
    let p = Project::new();
    p.capture("before.json", &[]);
    save_review(&p, "defer", "review ' $.json");
    p.write("b.py", &SOURCE.replace("+ 7", "+ 11"));
    p.capture("after.json", &[]);
    let changed = reviewed(&p, &[]);
    assert!(
        !changed["items"]
            .as_array()
            .unwrap()
            .iter()
            .any(|r| r["review_status"] == "applicable"),
        "{changed}"
    );
    let budget = reviewed(&p, &["--max-candidates", "0"]);
    assert!(!budget["items"]
        .as_array()
        .unwrap()
        .iter()
        .any(|r| r["review_status"] == "applicable"));
}

#[test]
fn conflicting_records_recheck_and_existing_files_are_never_overwritten() {
    let p = Project::new();
    p.capture("before.json", &[]);
    p.capture("after.json", &[]);
    save_review(&p, "keep-separate", "review ' $.json");
    save_review(&p, "refactor", "conflict.json");
    let conflict = reviewed(&p, &["--reviews", "conflict.json"]);
    assert_eq!(
        conflict["items"][0]["review_status"], "recheck",
        "{conflict}"
    );
    assert!(conflict["items"][0]["reviews"][0]["basis"]
        .as_str()
        .unwrap()
        .contains("Conflicting"));
    let before = std::fs::read(p.0.join("review ' $.json")).unwrap();
    let change = format!("change={}", conflict["items"][0]["id"].as_str().unwrap());
    let out = p.run(&[
        "query",
        "--before",
        "before.json",
        "--after",
        "after.json",
        &change,
        "--write-review",
        "review ' $.json",
        "--decision",
        "defer",
        "--reason",
        "overwrite",
    ]);
    assert!(!out.status.success());
    assert_eq!(std::fs::read(p.0.join("review ' $.json")).unwrap(), before);
}

#[test]
fn sources_are_explicit_verified_and_historical_text_is_not_replaced_by_current_text() {
    let p = Project::new();
    p.capture("before.json", &[]);
    std::fs::create_dir(p.0.join("historical")).unwrap();
    for file in ["a.py", "b.py"] {
        std::fs::copy(p.0.join(file), p.0.join("historical").join(file)).unwrap();
    }
    p.write("b.py", &SOURCE.replace("+ 7", "+ 11"));
    p.capture("after.json", &["--exclude", "historical/**"]);
    let view = p.compare(&["full", "top=0"]);
    let row = view["items"]
        .as_array()
        .unwrap()
        .iter()
        .find(|r| !r["before_observation"].is_null())
        .unwrap();
    let change = format!("change={}", row["id"].as_str().unwrap());
    assert_eq!(row["source_body_status"], "not-stored");
    let wrong = p.compare(&[&change, "--before-source", ".", "--after-source", "."]);
    let old = &wrong["items"][0]["before_observation"]["members"];
    let stale = old
        .as_array()
        .unwrap()
        .iter()
        .find(|m| m["file"] == "b.py")
        .unwrap();
    assert_eq!(stale["source_body"]["status"], "unavailable");
    assert_eq!(
        wrong["items"][0]["source_lookup"],
        json!({"verified":1,"unavailable":1})
    );
    assert!(stale["source_body"].get("text").is_none());
    let verified = p.compare(&[
        &change,
        "--before-source",
        "historical",
        "--after-source",
        ".",
    ]);
    let old = &verified["items"][0]["before_observation"]["members"];
    assert!(old
        .as_array()
        .unwrap()
        .iter()
        .all(|m| m["source_body"]["status"] == "verified"));
    assert!(old
        .as_array()
        .unwrap()
        .iter()
        .all(|m| m["source_body"]["text"].as_str().unwrap().contains("+ 7")));
    assert!(verified["items"][0]["source_diffs"].is_array());
    let followed = p.follow(&verified["items"][0]["actions"][0]["command"]);
    assert_eq!(
        followed["items"][0]["source_body_status"],
        "explicit-verified-lookup"
    );
}

#[test]
fn ambiguous_moves_changed_scope_and_other_captures_do_not_inherit_a_review() {
    let p = Project::new();
    p.capture("before.json", &[]);
    save_review(&p, "keep-separate", "review ' $.json");
    std::fs::rename(p.0.join("a.py"), p.0.join("first.py")).unwrap();
    std::fs::rename(p.0.join("b.py"), p.0.join("second.py")).unwrap();
    p.capture("after.json", &[]);
    let moved = reviewed(&p, &[]);
    assert!(
        !moved["items"]
            .as_array()
            .unwrap()
            .iter()
            .any(|r| r["review_status"] == "applicable"),
        "{moved}"
    );
    std::fs::rename(p.0.join("first.py"), p.0.join("a.py")).unwrap();
    std::fs::rename(p.0.join("second.py"), p.0.join("test_b.py")).unwrap();
    p.capture("scope.json", &[]);
    let scope = p.json(&[
        "query",
        "--before",
        "before.json",
        "--after",
        "scope.json",
        "--reviews",
        "review ' $.json",
        "--format",
        "json",
        "top=0",
    ]);
    assert!(
        !scope["items"]
            .as_array()
            .unwrap()
            .iter()
            .any(|r| r["review_status"] == "applicable"),
        "{scope}"
    );
    let unrelated = p.json(&[
        "query",
        "--before",
        "scope.json",
        "--after",
        "scope.json",
        "--reviews",
        "review ' $.json",
        "--format",
        "json",
    ]);
    assert_eq!(
        unrelated["reviews"]["unrelated"].as_array().unwrap().len(),
        1
    );
    assert!(unrelated["items"]
        .as_array()
        .unwrap()
        .iter()
        .all(|r| r["review_status"] == "unreviewed"));
}

#[test]
fn verified_source_alignment_exposes_common_and_changed_lines() {
    let p = Project::new();
    p.capture("before.json", &["--exclude", "old/**"]);
    std::fs::create_dir(p.0.join("old")).unwrap();
    for file in ["a.py", "b.py"] {
        std::fs::copy(p.0.join(file), p.0.join("old").join(file)).unwrap();
        p.write(
            file,
            &SOURCE.replace("    b =", "    # explain the constant\n    b ="),
        );
    }
    p.capture("after.json", &["--exclude", "old/**"]);
    let view = p.compare(&["full", "top=0"]);
    let change = format!("change={}", view["items"][0]["id"].as_str().unwrap());
    let detail = p.compare(&[&change, "--before-source", "old", "--after-source", "."]);
    let diffs = detail["items"][0]["source_diffs"].as_array().unwrap();
    assert!(!diffs.is_empty(), "{detail}");
    assert!(
        diffs
            .iter()
            .flat_map(|d| d["lines"].as_array().unwrap())
            .any(
                |l| l["tag"] == "+" && l["text"].as_str().unwrap().contains("explain the constant")
            ),
        "{detail}"
    );
    assert!(diffs
        .iter()
        .flat_map(|d| d["lines"].as_array().unwrap())
        .any(|l| l["tag"] == " "));
}

#[cfg(unix)]
#[test]
fn source_lookup_rejects_symlinks_outside_the_explicit_base() {
    let p = Project::new();
    let outside = Project::new();
    p.capture("before.json", &[]);
    p.capture("after.json", &[]);
    let view = p.compare(&[]);
    let change = format!("change={}", view["items"][0]["id"].as_str().unwrap());
    std::fs::remove_file(p.0.join("a.py")).unwrap();
    std::os::unix::fs::symlink(outside.0.join("a.py"), p.0.join("a.py")).unwrap();
    let detail = p.compare(&[&change, "--before-source", "."]);
    let escaped = detail["items"][0]["before_observation"]["members"]
        .as_array()
        .unwrap()
        .iter()
        .find(|m| m["file"] == "a.py")
        .unwrap();
    assert_eq!(escaped["source_body"]["status"], "unavailable");
    assert!(escaped["source_body"]["reason"]
        .as_str()
        .unwrap()
        .contains("escapes"));
    assert!(escaped["source_body"].get("text").is_none());
}

#[test]
fn incomplete_population_allows_current_intent_but_not_transfer() {
    let p = Project::new();
    p.write("legacy.h", "namespace engine { class Thing {}; }");
    p.capture("before.json", &[]);
    save_review(&p, "defer", "review ' $.json");
    let current = p.json(&[
        "query",
        "--before",
        "before.json",
        "--after",
        "before.json",
        "--reviews",
        "review ' $.json",
        "review=applicable",
        "--format",
        "json",
    ]);
    assert_eq!(current["complete"], false);
    assert_eq!(current["summary"]["selected"], 1, "{current}");
    assert_eq!(current["items"][0]["reviews"][0]["decision"], "defer");
    p.write("a.py", &format!("# new containing buffer\n{SOURCE}"));
    p.capture("after.json", &[]);
    let transferred = reviewed(&p, &[]);
    assert!(
        !transferred["items"]
            .as_array()
            .unwrap()
            .iter()
            .any(|item| item["review_status"] == "applicable"),
        "{transferred}"
    );
}
