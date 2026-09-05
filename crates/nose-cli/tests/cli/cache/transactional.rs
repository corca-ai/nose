use super::*;

fn clone_project(tag: &str) -> TempProject {
    let project = TempProject::new(tag);
    for (path, name) in [
        ("a/one.py", "one"),
        ("b/two.py", "two"),
        ("c/three.py", "three"),
    ] {
        project.write(
            path,
            &format!(
                "def {name}(items):\n    total = 0\n    for item in items:\n        if item > 0:\n            total = total + item * item\n    return total\n"
            ),
        );
    }
    project
}

fn query_with_limit(project: &Path, cache: &Path, limit: Option<&str>) -> Output {
    let mut command = Command::new(bin());
    command.args([
        "query",
        project.to_str().unwrap(),
        "all",
        "top=0",
        "--format",
        "json",
        "--cache-dir",
        cache.to_str().unwrap(),
    ]);
    if let Some(limit) = limit {
        command.args(["--cache-max-bytes", limit]);
    }
    let output = command.output().expect("run transactional cache query");
    assert!(
        output.status.success(),
        "query failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    output
}

fn managed_files(root: &Path) -> Vec<PathBuf> {
    let mut files = Vec::new();
    let mut pending = vec![root.join("cas-v2"), root.join("state-v2")];
    while let Some(path) = pending.pop() {
        let Ok(metadata) = fs::symlink_metadata(&path) else {
            continue;
        };
        if metadata.is_file() {
            files.push(path);
        } else if let Ok(entries) = fs::read_dir(path) {
            pending.extend(entries.flatten().map(|entry| entry.path()));
        }
    }
    files
}

#[test]
fn concurrent_processes_publish_one_readable_store() {
    let project = clone_project("cache_concurrent");
    let cache = project.path().join(".cache");
    let clean = query(project.path(), None);
    let command = || {
        let mut command = Command::new(bin());
        command.args([
            "query",
            project.path().to_str().unwrap(),
            "all",
            "top=0",
            "--format",
            "json",
            "--cache-dir",
            cache.to_str().unwrap(),
        ]);
        command.stdout(Stdio::piped()).stderr(Stdio::piped());
        command
    };
    let first = command().spawn().expect("spawn first cache writer");
    let second = command().spawn().expect("spawn second cache writer");
    let first = first.wait_with_output().expect("join first cache writer");
    let second = second.wait_with_output().expect("join second cache writer");
    assert!(first.status.success());
    assert!(second.status.success());
    assert_same_analysis_output(&first, &clean);
    assert_same_analysis_output(&second, &clean);
    assert_same_analysis_output(&query_with_limit(project.path(), &cache, None), &clean);
}

#[test]
fn corruption_and_truncation_recompute_without_changing_output() {
    let project = clone_project("cache_corruption_v2");
    let cache = project.path().join(".cache");
    let clean = query(project.path(), None);
    assert_same_analysis_output(&query_with_limit(project.path(), &cache, None), &clean);
    let files = managed_files(&cache);
    assert!(files
        .iter()
        .any(|path| path.extension().is_some_and(|ext| ext == "artifact")));
    assert!(files
        .iter()
        .any(|path| path.file_name().is_some_and(|name| name == "CURRENT")));
    for (index, path) in files.into_iter().enumerate() {
        if path.file_name().is_some_and(|name| name == "LOCK") {
            continue;
        }
        let mut bytes = fs::read(&path).unwrap();
        if bytes.is_empty() {
            continue;
        }
        if index % 2 == 0 {
            bytes.truncate(bytes.len() / 2);
        } else {
            let last = bytes.len() - 1;
            bytes[last] ^= 0xff;
        }
        fs::write(path, bytes).unwrap();
    }
    let repaired = query_with_limit(project.path(), &cache, None);
    assert_same_analysis_output(&repaired, &clean);
    assert!(
        String::from_utf8_lossy(&repaired.stderr).contains("ignoring corrupt cache"),
        "corrupt entries should produce actionable warnings"
    );
}

#[test]
fn eviction_and_cache_commands_affect_performance_only() {
    let project = clone_project("cache_prune_cli");
    let cache = project.path().join(".cache");
    let clean = query(project.path(), None);
    let bounded = query_with_limit(project.path(), &cache, Some("1KiB"));
    assert_same_analysis_output(&bounded, &clean);

    let status = Command::new(bin())
        .args([
            "cache",
            "status",
            "--dir",
            cache.to_str().unwrap(),
            "--format",
            "json",
            "--max-bytes",
            "1KiB",
        ])
        .output()
        .unwrap();
    assert!(status.status.success());
    let status: serde_json::Value = serde_json::from_slice(&status.stdout).unwrap();
    assert_eq!(status["schema"], "nose.cache-status/v1");
    assert!(status["bytes"].as_u64().unwrap() <= 1024);

    let cleared = Command::new(bin())
        .args([
            "cache",
            "clear",
            "--dir",
            cache.to_str().unwrap(),
            "--format",
            "json",
        ])
        .output()
        .unwrap();
    assert!(cleared.status.success());
    assert_same_analysis_output(&query_with_limit(project.path(), &cache, None), &clean);
}

#[cfg(unix)]
#[test]
fn read_only_store_hits_and_computes_misses() {
    let project = clone_project("cache_read_only");
    let cache = project.path().join(".cache");
    let clean = query(project.path(), None);
    assert_same_analysis_output(&query_with_limit(project.path(), &cache, None), &clean);
    let protected = Command::new("chmod")
        .args(["-R", "a-w", cache.to_str().unwrap()])
        .status()
        .unwrap();
    assert!(protected.success());
    let warm = query_with_limit(project.path(), &cache, None);
    assert_same_analysis_output(&warm, &clean);
    project.write(
        "c/three.py",
        "def three(items):\n    return sum(item * item for item in items if item > 0)\n",
    );
    let clean_changed = query(project.path(), None);
    let read_only_miss = query_with_limit(project.path(), &cache, None);
    assert_same_analysis_output(&read_only_miss, &clean_changed);
    let _ = Command::new("chmod")
        .args(["-R", "u+w", cache.to_str().unwrap()])
        .status();
}
