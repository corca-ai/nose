use super::*;

const PROSE: &str = "# Install\n\nDownload the binary from the releases page and place it on your PATH. Then run the version command to confirm the installation succeeded correctly.\n";

#[test]
fn watch_survives_tiny_cache_and_idle_eviction() {
    let project = project("watch_tiny_budget");
    let cache = make_temp_dir("watch_tiny_budget_cache");
    let mut watch =
        WatchProcess::start_with_args(project.path(), &cache, &["--cache-max-bytes", "1KiB"]);
    let initial = watch.next("initial tiny-budget snapshot");
    assert_same_analysis(&initial["snapshot"], &clean_dashboard(project.path()));
    for revision in 1..=3 {
        let mut clear = Command::new(bin())
            .args([
                "cache",
                "clear",
                "--dir",
                cache.to_str().unwrap(),
                "--format",
                "json",
            ])
            .stdout(Stdio::null())
            .spawn()
            .unwrap();
        let deadline = std::time::Instant::now() + Duration::from_secs(5);
        loop {
            if let Some(status) = clear.try_wait().unwrap() {
                assert!(status.success());
                break;
            }
            if std::time::Instant::now() >= deadline {
                clear.kill().unwrap();
                clear.wait().unwrap();
                panic!("cache clear blocked while the watch session was idle");
            }
            thread::sleep(Duration::from_millis(10));
        }
        project.write(
            "b.py",
            &SECOND.replace("total = 0", &format!("total = {revision}")),
        );
        let changed = watch.next("tiny-budget edit");
        assert_same_analysis(&changed["snapshot"], &clean_dashboard(project.path()));
    }
    drop(watch);
    fs::remove_dir_all(cache).unwrap();
}

#[test]
fn leaf_watch_enforces_storage_budget_issue_1001() {
    let project = project("watch_budget");
    let cache = make_temp_dir("watch_budget_cache");
    let budget = 40_000u64;
    let mut watch = WatchProcess::start_with_args(
        project.path(),
        &cache,
        &["--cache-max-bytes", &budget.to_string()],
    );
    watch.next("initial bounded snapshot");
    for revision in 1..=12 {
        project.write(
            "b.py",
            &SECOND.replace("total = 0", &format!("total = {revision}")),
        );
        let snapshot = watch.next("bounded leaf revision");
        assert_eq!(snapshot["reconciliation"], "incremental-leaf");
        assert_same_analysis(&snapshot["snapshot"], &clean_dashboard(project.path()));
        let output = Command::new(bin())
            .args([
                "cache",
                "status",
                "--dir",
                cache.to_str().unwrap(),
                "--format",
                "json",
            ])
            .output()
            .unwrap();
        assert!(
            output.status.success(),
            "{}",
            String::from_utf8_lossy(&output.stderr)
        );
        let status: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();
        assert!(
            status["bytes"].as_u64().unwrap() <= budget,
            "revision {revision}: {status}"
        );
    }
    drop(watch);
    fs::remove_dir_all(cache).unwrap();
}

#[test]
fn watch_empty_code_facet_transitions_issue_1000() {
    let project = TempProject::new("watch_empty");
    let cache = make_temp_dir("watch_empty_cache");
    let mut watch = WatchProcess::start(project.path(), &cache);
    let initial = watch.next("empty root");
    assert_same_analysis(&initial["snapshot"], &clean_dashboard(project.path()));
    project.write("a.py", FIRST);
    let added = watch.next("first code file");
    assert_same_analysis(&added["snapshot"], &clean_dashboard(project.path()));
    fs::remove_file(project.path().join("a.py")).unwrap();
    let removed = watch.next("last code file removed");
    assert_same_analysis(&removed["snapshot"], &clean_dashboard(project.path()));
    project.write("a.md", PROSE);
    project.write("b.md", PROSE);
    let markdown = watch.next("Markdown after empty code");
    assert_same_analysis(&markdown["snapshot"], &clean_dashboard(project.path()));
    drop(watch);
    fs::remove_dir_all(cache).unwrap();
}

#[test]
fn markdown_only_change_refreshes_dashboard() {
    let project = TempProject::new("watch_markdown");
    project.write("a.md", PROSE);
    project.write("b.md", PROSE);
    let cache = make_temp_dir("watch_markdown_cache");
    let mut watch = WatchProcess::start(project.path(), &cache);
    let initial = watch.next("initial Markdown snapshot");
    assert!(!initial["snapshot"]["markdown"]
        .as_array()
        .unwrap()
        .is_empty());
    project.write(
        "b.md",
        "# Unrelated\n\nQuantum particles occupy a separate experimental domain.\n",
    );
    let revision = watch.next("Markdown change");
    assert_eq!(revision["sequence"], 1);
    assert_same_analysis(&revision["snapshot"], &clean_dashboard(project.path()));
    assert!(revision["snapshot"]["markdown"]
        .as_array()
        .unwrap()
        .is_empty());
    drop(watch);
    fs::remove_dir_all(cache).unwrap();
}

#[test]
fn ignore_only_change_refreshes_dashboard_without_source_edit() {
    let project = project("watch_ignore");
    let ignore = project.path().join("nose.ignore.json");
    project.write("nose.ignore.json", r#"{"ignores":[]}"#);
    project.write("nose.toml", "[query]\nignore-file = \"nose.ignore.json\"\n");
    let cache = make_temp_dir("watch_ignore_cache");
    let config = project.path().join("nose.toml");
    let mut watch = WatchProcess::start_with_args(
        project.path(),
        &cache,
        &["--config", config.to_str().unwrap()],
    );
    let initial = watch.next("initial ignore snapshot");
    fs::write(
        &ignore,
        r#"{"ignores":[{"paths":["**/*.py"],"reason":"accepted"}]}"#,
    )
    .unwrap();
    let revision = watch.next("ignore change");
    assert_eq!(revision["sequence"], 1);
    assert_ne!(initial["snapshot"], revision["snapshot"]);
    let clean = Command::new(bin())
        .args([
            "query",
            project.path().to_str().unwrap(),
            "--format",
            "json",
            "--mode",
            "semantic",
            "--min-size",
            "1",
            "--min-lines",
            "1",
            "--config",
            config.to_str().unwrap(),
        ])
        .output()
        .unwrap();
    assert!(clean.status.success());
    assert_same_analysis(
        &revision["snapshot"],
        &serde_json::from_slice::<serde_json::Value>(&clean.stdout).unwrap(),
    );
    drop(watch);
    fs::remove_dir_all(cache).unwrap();
}

#[test]
fn external_ignore_file_without_an_extension_is_watched() {
    let project = project("watch_external_ignore");
    let cache = make_temp_dir("watch_external_ignore_cache");
    let config_dir = make_temp_dir("watch_external_config");
    let ignore = config_dir.join("rules");
    fs::write(&ignore, r#"{"ignores":[]}"#).unwrap();
    let mut watch = WatchProcess::start_with_args(
        project.path(),
        &cache,
        &["--ignore-file", ignore.to_str().unwrap()],
    );
    let initial = watch.next("initial external-ignore snapshot");
    fs::write(
        &ignore,
        r#"{"ignores":[{"paths":["**/*.py"],"reason":"accepted"}]}"#,
    )
    .unwrap();
    let revision = watch.next("external ignore change");
    assert_eq!(revision["sequence"], 1);
    assert_ne!(initial["snapshot"], revision["snapshot"]);
    assert_eq!(initial["source_set_digest"], revision["source_set_digest"]);
    drop(watch);
    fs::remove_dir_all(config_dir).unwrap();
    fs::remove_dir_all(cache).unwrap();
}
