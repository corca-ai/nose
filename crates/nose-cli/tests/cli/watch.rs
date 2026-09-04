use super::*;
use std::io::{BufRead, BufReader};
use std::process::{Child, ChildStdout};
use std::sync::mpsc;
use std::thread;
use std::time::Duration;

#[path = "watch/inputs.rs"]
mod inputs;

const FIRST: &str = "def first(items):\n    total = 0\n    for item in items:\n        if item > 0:\n            total = total + item * item\n    return total\n";
const SECOND: &str = "def second(values):\n    total = 0\n    for value in values:\n        if value > 0:\n            total = total + value * value\n    return total\n";
const CHANGED: &str = "def second(values):\n    total = 0\n    for value in values:\n        if value > 0:\n            total = total - value * value\n    return total\n";
const CHANGED_FIRST: &str = "def first(items):\n    total = 1\n    for item in items:\n        if item > 0:\n            total = total - item * item\n    return total\n";

struct WatchProcess {
    child: Child,
    lines: mpsc::Receiver<String>,
    reader: Option<thread::JoinHandle<()>>,
}

impl WatchProcess {
    fn start(project: &Path, cache: &Path) -> Self {
        Self::start_with_args(project, cache, &[])
    }

    fn start_with_args(project: &Path, cache: &Path, extra: &[&str]) -> Self {
        let mut child = Command::new(bin())
            .args([
                "query",
                project.to_str().unwrap(),
                "--watch",
                "--format",
                "jsonl",
                "--mode",
                "semantic",
                "--min-size",
                "1",
                "--min-lines",
                "1",
                "--cache-dir",
                cache.to_str().unwrap(),
            ])
            .args(extra)
            .stdout(Stdio::piped())
            .stderr(Stdio::inherit())
            .spawn()
            .unwrap();
        let stdout = child.stdout.take().unwrap();
        let (sender, lines) = mpsc::channel();
        let reader = thread::spawn(move || read_lines(stdout, sender));
        Self {
            child,
            lines,
            reader: Some(reader),
        }
    }

    fn next(&mut self, expected: &str) -> serde_json::Value {
        let line = self
            .lines
            .recv_timeout(Duration::from_secs(30))
            .unwrap_or_else(|error| {
                let status = self.child.try_wait().expect("watch process status");
                panic!("watch emitted {expected}: {error}; process status: {status:?}")
            });
        serde_json::from_str(&line).unwrap()
    }
}

impl Drop for WatchProcess {
    fn drop(&mut self) {
        let _ = self.child.kill();
        let _ = self.child.wait();
        if let Some(reader) = self.reader.take() {
            let _ = reader.join();
        }
    }
}

fn read_lines(stdout: ChildStdout, sender: mpsc::Sender<String>) {
    for line in BufReader::new(stdout).lines().map_while(Result::ok) {
        if sender.send(line).is_err() {
            break;
        }
    }
}

fn project(tag: &str) -> TempProject {
    let project = TempProject::new(tag);
    project.write("a.py", FIRST);
    project.write("b.py", SECOND);
    project
}

fn clean_dashboard(project: &Path) -> serde_json::Value {
    let output = Command::new(bin())
        .args([
            "query",
            project.to_str().unwrap(),
            "--format",
            "json",
            "--mode",
            "semantic",
            "--min-size",
            "1",
            "--min-lines",
            "1",
        ])
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    serde_json::from_slice(&output.stdout).unwrap()
}

#[test]
fn leaf_revision_matches_a_clean_query_and_restart() {
    let project = project("watch_leaf");
    let cache = make_temp_dir("watch_leaf_cache");
    let first_digest;
    {
        let mut watch = WatchProcess::start(project.path(), &cache);
        let initial = watch.next("initial snapshot");
        assert_eq!(initial["schema"], "nose.query-watch/v1");
        assert_eq!(initial["sequence"], 0);
        assert_eq!(initial["snapshot"], clean_dashboard(project.path()));
        first_digest = initial["source_set_digest"].as_str().unwrap().to_owned();

        project.write("b.py", CHANGED);
        let revision = watch.next("leaf revision");
        assert_eq!(revision["sequence"], 1);
        assert_eq!(revision["reconciliation"], "incremental-leaf");
        assert_ne!(revision["source_set_digest"], first_digest);
        assert_eq!(revision["snapshot"], clean_dashboard(project.path()));
        assert!(revision["changed_paths"]
            .as_array()
            .unwrap()
            .iter()
            .any(|path| path.as_str().unwrap().ends_with("b.py")));
    }

    let mut restarted = WatchProcess::start(project.path(), &cache);
    let initial = restarted.next("restart snapshot");
    assert_eq!(initial["sequence"], 0);
    assert_eq!(initial["snapshot"], clean_dashboard(project.path()));
    assert_ne!(initial["source_set_digest"], first_digest);
    drop(restarted);
    let _ = fs::remove_dir_all(cache);
}

#[test]
fn atomic_replace_reconciles_to_the_final_filesystem() {
    let project = project("watch_atomic");
    let cache = make_temp_dir("watch_atomic_cache");
    let mut watch = WatchProcess::start(project.path(), &cache);
    let _initial = watch.next("initial snapshot");
    let replacement = project.path().join("b.py.nose-tmp");
    fs::write(&replacement, CHANGED).unwrap();
    if fs::rename(&replacement, project.path().join("b.py")).is_err() {
        fs::remove_file(project.path().join("b.py")).unwrap();
        fs::rename(&replacement, project.path().join("b.py")).unwrap();
    }
    let revision = watch.next("atomic-replace revision");
    assert_eq!(revision["sequence"], 1);
    assert_eq!(revision["snapshot"], clean_dashboard(project.path()));
    drop(watch);
    let _ = fs::remove_dir_all(cache);
}

#[test]
fn burst_across_sources_emits_the_final_clean_snapshot() {
    let project = project("watch_burst");
    let cache = make_temp_dir("watch_burst_cache");
    let mut watch = WatchProcess::start(project.path(), &cache);
    let _initial = watch.next("initial snapshot");
    project.write("a.py", CHANGED_FIRST);
    project.write("b.py", CHANGED);
    let revision = watch.next("burst revision");
    assert_eq!(revision["sequence"], 1);
    assert_eq!(revision["snapshot"], clean_dashboard(project.path()));
    drop(watch);
    let _ = fs::remove_dir_all(cache);
}

#[test]
fn delete_recreate_burst_cannot_leave_stale_state() {
    let project = project("watch_recreate");
    let cache = make_temp_dir("watch_recreate_cache");
    let mut watch = WatchProcess::start(project.path(), &cache);
    let _initial = watch.next("initial snapshot");
    fs::remove_file(project.path().join("b.py")).unwrap();
    project.write("b.py", CHANGED);
    let revision = watch.next("delete-recreate revision");
    assert_eq!(revision["sequence"], 1);
    assert_eq!(revision["snapshot"], clean_dashboard(project.path()));
    drop(watch);
    let _ = fs::remove_dir_all(cache);
}

#[test]
fn watch_and_jsonl_must_be_selected_together() {
    let project = project("watch_args");
    let error = run_fail(&["query", project.path().to_str().unwrap(), "--watch"]);
    assert!(error.contains("--watch requires --format jsonl"));
    let error = run_fail(&[
        "query",
        project.path().to_str().unwrap(),
        "--format",
        "jsonl",
    ]);
    assert!(error.contains("--format jsonl requires --watch"));
}

#[test]
fn invalid_config_emits_error_and_recovers_in_same_process() {
    let project = project("watch_config_recovery");
    let cache = make_temp_dir("watch_config_recovery_cache");
    project.write("nose.toml", "[query]\n");
    let config = project.path().join("nose.toml");
    let mut watch = WatchProcess::start_with_args(
        project.path(),
        &cache,
        &["--config", config.to_str().unwrap()],
    );
    let initial = watch.next("initial");
    project.write("nose.toml", "[query]\nmin-size = [\n");
    let error = watch.next("error");
    assert_eq!(error["kind"], "error");
    assert_eq!(error["snapshot_valid"], false);
    project.write("nose.toml", "[query]\n");
    let recovered = watch.next("recovery");
    assert_eq!(recovered["kind"], "snapshot");
    assert_eq!(recovered["snapshot"], initial["snapshot"]);
    assert!(recovered["sequence"].as_u64().unwrap() > error["sequence"].as_u64().unwrap());
}
