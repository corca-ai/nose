//! Saved analysis comparison is an offline, navigable query surface.
use serde_json::{json, Value};
use std::path::{Path, PathBuf};
use std::process::{Command, Output};
use std::sync::atomic::{AtomicUsize, Ordering};

struct Project(PathBuf);
impl Project {
    fn new() -> Self {
        static NEXT: AtomicUsize = AtomicUsize::new(0);
        let path = std::env::temp_dir().join(format!(
            "nose changes ' $ {}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        std::fs::create_dir_all(&path).unwrap();
        let p = Self(path);
        p.write("a.py", SOURCE);
        p.write("b.py", SOURCE);
        p
    }
    fn write(&self, name: &str, source: &str) {
        std::fs::write(self.0.join(name), source).unwrap();
    }
    fn run(&self, args: &[&str]) -> Output {
        Command::new(env!("CARGO_BIN_EXE_nose"))
            .current_dir(&self.0)
            .env("RAYON_NUM_THREADS", "2")
            .args(args)
            .output()
            .unwrap()
    }
    fn json(&self, args: &[&str]) -> Value {
        let out = self.run(args);
        assert!(
            out.status.success(),
            "{}",
            String::from_utf8_lossy(&out.stderr)
        );
        serde_json::from_slice(&out.stdout).unwrap()
    }
    fn capture(&self, name: &str, extra: &[&str]) -> Value {
        let mut args = vec![
            "query",
            ".",
            "--mode",
            "semantic",
            "--min-size",
            "1",
            "--min-lines",
            "1",
            "--save-analysis",
            name,
            "--format",
            "json",
        ];
        args.extend_from_slice(extra);
        self.json(&args);
        serde_json::from_slice(&std::fs::read(self.0.join(name)).unwrap()).unwrap()
    }
    fn compare(&self, extra: &[&str]) -> Value {
        let mut args = vec![
            "query",
            "--before",
            "before.json",
            "--after",
            "after.json",
            "--format",
            "json",
        ];
        args.extend_from_slice(extra);
        self.json(&args)
    }
    fn follow(&self, next: &Value) -> Value {
        let binary = Path::new(env!("CARGO_BIN_EXE_nose"));
        let path = std::env::join_paths(
            std::iter::once(binary.parent().unwrap().to_path_buf()).chain(std::env::split_paths(
                &std::env::var_os("PATH").unwrap_or_default(),
            )),
        )
        .unwrap();
        let out = Command::new("sh")
            .current_dir(&self.0)
            .env("PATH", path)
            .args(["-c", &format!("{} --format json", next.as_str().unwrap())])
            .output()
            .unwrap();
        assert!(
            out.status.success(),
            "{}",
            String::from_utf8_lossy(&out.stderr)
        );
        serde_json::from_slice(&out.stdout).unwrap()
    }
}
impl Drop for Project {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}
const SOURCE: &str =
    "def compute(x):\n    a = x * x\n    b = a + 7\n    c = b // 3\n    return c\n";

#[test]
fn movement_keeps_evidence_and_generated_navigation_survives_missing_source() {
    let p = Project::new();
    let before = p.capture("before.json", &[]);
    assert!(!before["families"].as_array().unwrap().is_empty());
    std::fs::rename(p.0.join("b.py"), p.0.join("renamed.py")).unwrap();
    p.capture("after.json", &[]);
    for name in ["a.py", "renamed.py"] {
        std::fs::remove_file(p.0.join(name)).unwrap();
    }
    // Comparison does not load ambient configuration or require any source.
    p.write("nose.toml", "not valid toml [[[ ");
    let report = p.compare(&[]);
    assert_eq!(report["complete"], true);
    assert!(report["items"]
        .as_array()
        .unwrap()
        .iter()
        .all(|r| r["unchanged_evidence"] == true));
    let groups = p.follow(&report["next"][0]);
    assert_eq!(groups["view"], "group");
    let slice = p.follow(&groups["groups"][0]["next"][0]);
    let detail = p.follow(&slice["items"][0]["next"][0]);
    assert_eq!(detail["view"], "change");
    assert_eq!(detail["items"][0]["source_body_status"], "not-stored");
    assert!(detail["items"][0]["before_observation"].is_object());
    assert_eq!(detail["inputs"], report["inputs"]);
    let human = p.run(&["query", "--before", "before.json", "--after", "after.json"]);
    assert!(human.status.success());
    assert!(String::from_utf8_lossy(&human.stdout).contains("review-evidence-retained"));
}

#[test]
fn a_new_copy_changes_membership_and_never_inherits_retained_evidence() {
    let p = Project::new();
    p.capture("before.json", &[]);
    p.write("c.py", SOURCE);
    p.capture("after.json", &[]);
    let result = p.compare(&["top=0"]);
    assert!(result["items"]
        .as_array()
        .unwrap()
        .iter()
        .all(|r| r["unchanged_evidence"] == false));
    assert!(result["items"]
        .as_array()
        .unwrap()
        .iter()
        .any(|r| r["reasons"]
            .as_array()
            .unwrap()
            .contains(&json!("membership-changed"))));
}

#[test]
fn profile_missing_coverage_and_candidate_budget_preclude_retention() {
    let p = Project::new();
    p.capture("before.json", &[]);
    let mut after = p.capture("after.json", &[]);
    let full = p.compare(&["top=0"]);
    assert!(full["summary"]["retained"].as_u64().unwrap() > 0);
    let capped = p.compare(&["--max-candidates", "0"]);
    assert_eq!(capped["complete"], false);
    assert_eq!(capped["summary"]["retained"], 0);
    after["profile"]["engine"] = json!("different engine");
    p.write("after.json", &after.to_string());
    assert_eq!(p.compare(&[])["summary"]["retained"], 0);
    after["profile"] =
        serde_json::from_slice::<Value>(&std::fs::read(p.0.join("before.json")).unwrap()).unwrap()
            ["profile"]
            .clone();
    after["complete"] = json!(false);
    p.write("after.json", &after.to_string());
    assert_eq!(p.compare(&[])["summary"]["retained"], 0);
}

#[test]
fn display_limits_filters_and_group_navigation_preserve_comparison_context() {
    let p = Project::new();
    p.capture("before.json", &[]);
    p.capture("after.json", &[]);
    let full = p.compare(&["top=0"]);
    let short = p.compare(&["top=1"]);
    assert_eq!(full["summary"]["total"], short["summary"]["total"]);
    assert_eq!(full["summary"]["retained"], short["summary"]["retained"]);
    let grouped = p.compare(&["scope=prod", "group=reason", "--max-candidates", "500"]);
    let list = p.follow(&grouped["groups"][0]["next"][0]);
    assert_eq!(list["max_candidates"], 500);
    assert!(grouped["groups"][0]["next"][0]
        .as_str()
        .unwrap()
        .contains("scope=prod"));
    let empty = p.compare(&["scope=test"]);
    assert_eq!(empty["summary"]["selected"], 0);
    assert!(
        p.follow(&empty["next"][0])["summary"]["selected"]
            .as_u64()
            .unwrap()
            > 0
    );
}

#[test]
fn partial_query_json_and_invalid_combinations_fail_loudly() {
    let p = Project::new();
    p.capture("before.json", &[]);
    p.capture("after.json", &[]);
    for extra in [
        vec!["reason=typo"],
        vec!["since=before.json"],
        vec!["--mode", "near"],
        vec!["--fail-on", "any"],
        vec!["group=typo"],
    ] {
        let mut args = vec!["query", "--before", "before.json", "--after", "after.json"];
        args.extend(extra);
        assert!(!p.run(&args).status.success(), "{args:?}");
    }
    for extra in [
        vec!["top=1"],
        vec!["--watch"],
        vec!["--baseline", "base.json"],
    ] {
        let mut args = vec!["query", ".", "--save-analysis", "new.json"];
        args.extend(extra);
        assert!(!p.run(&args).status.success(), "{args:?}");
    }
    let original = std::fs::read(p.0.join("before.json")).unwrap();
    assert!(!p
        .run(&["query", ".", "--save-analysis", "before.json"])
        .status
        .success());
    assert_eq!(original, std::fs::read(p.0.join("before.json")).unwrap());
    p.write("after.json", "{\"view\":\"dashboard\",\"families\":[]}");
    assert!(!p
        .run(&["query", "--before", "before.json", "--after", "after.json"])
        .status
        .success());
}

#[test]
fn clean_cold_warm_captures_and_worker_counts_are_identical() {
    let p = Project::new();
    let clean = p.capture("clean.json", &[]);
    assert_eq!(clean, p.capture("cold.json", &["--cache-dir", "cache"]));
    assert_eq!(clean, p.capture("warm.json", &["--cache-dir", "cache"]));
    let out = Command::new(env!("CARGO_BIN_EXE_nose"))
        .current_dir(&p.0)
        .env("RAYON_NUM_THREADS", "4")
        .args([
            "query",
            ".",
            "--mode",
            "semantic",
            "--min-size",
            "1",
            "--min-lines",
            "1",
            "--save-analysis",
            "threads.json",
        ])
        .output()
        .unwrap();
    assert!(
        out.status.success(),
        "{}",
        String::from_utf8_lossy(&out.stderr)
    );
    assert_eq!(
        clean,
        serde_json::from_slice::<Value>(&std::fs::read(p.0.join("threads.json")).unwrap()).unwrap()
    );
}

#[test]
fn path_facets_handle_literal_commas_operators_and_multi_root_capture() {
    let p = Project::new();
    std::fs::create_dir(p.0.join("left root")).unwrap();
    std::fs::create_dir(p.0.join("right root")).unwrap();
    std::fs::rename(p.0.join("a.py"), p.0.join("left root/a,!=b.py")).unwrap();
    std::fs::rename(p.0.join("b.py"), p.0.join("right root/b.py")).unwrap();
    p.write(
        "config.toml",
        "[query]\nmode = [\"semantic\"]\nmin-size = 1\nmin-lines = 1\n",
    );
    for name in ["before.json", "after.json"] {
        p.json(&[
            "query",
            "--root",
            "left root",
            "--root",
            "right root",
            "--config",
            "config.toml",
            "--save-analysis",
            name,
            "--format",
            "json",
        ]);
    }
    std::fs::remove_file(p.0.join("config.toml")).unwrap();
    let grouped = p.compare(&["group=path"]);
    assert_eq!(grouped["groups"].as_array().unwrap().len(), 2);
    for group in grouped["groups"].as_array().unwrap() {
        let selected = p.follow(&group["next"][0]);
        assert_eq!(selected["summary"]["selected"], group["count"]);
        assert_eq!(selected["roots"], grouped["roots"]);
    }
    assert_eq!(p.compare(&["path~a,!=b"])["summary"]["selected"], 1);
}
