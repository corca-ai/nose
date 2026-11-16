use super::*;

fn included_dashboard(project: &Path) -> serde_json::Value {
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
            "--include-inline-suppressed",
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
fn inline_suppression_watch_keeps_policy_on_leaf_refresh_and_restart() {
    let project = project("watch_inline_suppression");
    project.write("a.py", &format!("# nose-ignore\n{FIRST}"));
    project.write("b.py", &format!("# nose-ignore\n{SECOND}"));
    let cache = make_temp_dir("watch_inline_suppression_cache");
    // Prime the other policy in the very same cache directory.
    {
        let mut default = WatchProcess::start(project.path(), &cache);
        let initial = default.next("default snapshot");
        assert_same_analysis(&initial["snapshot"], &clean_dashboard(project.path()));
    }
    {
        let mut watch =
            WatchProcess::start_with_args(project.path(), &cache, &["--include-inline-suppressed"]);
        let initial = watch.next("included snapshot");
        assert_same_analysis(&initial["snapshot"], &included_dashboard(project.path()));
        assert_ne!(
            initial["snapshot"]["summary"],
            clean_dashboard(project.path())["summary"]
        );
        write_leaf_edit(&project, "b.py", &format!("# nose-ignore\n{CHANGED}"));
        let revision = watch.next("included leaf revision");
        assert_eq!(revision["reconciliation"], "incremental-leaf");
        assert_same_analysis(&revision["snapshot"], &included_dashboard(project.path()));
    }
    let mut watch =
        WatchProcess::start_with_args(project.path(), &cache, &["--include-inline-suppressed"]);
    let restart = watch.next("included restart");
    assert_same_analysis(&restart["snapshot"], &included_dashboard(project.path()));
    drop(watch);
    let _ = fs::remove_dir_all(cache);
}
