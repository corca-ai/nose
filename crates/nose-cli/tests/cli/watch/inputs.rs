use super::*;

const PROSE: &str = "# Install\n\nDownload the binary from the releases page and place it on your PATH. Then run the version command to confirm the installation succeeded correctly.\n";

#[test]
fn markdown_only_change_refreshes_dashboard() {
    let project = project("watch_markdown");
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
