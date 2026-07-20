use super::*;

#[derive(Debug, serde::Deserialize)]
struct LineIndexStats {
    schema: String,
    files_reused: usize,
    files_rebuilt: usize,
    files_removed: usize,
    changed_document_frequencies: usize,
}

#[derive(Debug, serde::Deserialize)]
struct FamilyLineStats {
    schema: String,
    families_reused: usize,
    families_reweighted: usize,
    families_rebuilt: usize,
}

fn query_default(project: &Path, cache: Option<&Path>) -> Output {
    let mut command = Command::new(bin());
    command.args([
        "query",
        project.to_str().unwrap(),
        "all",
        "top=0",
        "--format",
        "json",
    ]);
    if let Some(cache) = cache {
        command.args(["--cache-dir", cache.to_str().unwrap()]);
        command.env("NOSE_CACHE_STATS", "1");
    }
    let output = command.output().expect("run cached default query");
    assert!(
        output.status.success(),
        "query failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    output
}

fn query_incremental_mode(
    project: &Path,
    cache: Option<&Path>,
    mode: Option<&str>,
    threads: &str,
) -> Output {
    let mut command = Command::new(bin());
    command.args([
        "query",
        project.to_str().unwrap(),
        "all",
        "top=0",
        "--min-size",
        "12",
        "--min-lines",
        "3",
        "--format",
        "json",
    ]);
    if let Some(mode) = mode {
        command.args(["--mode", mode]);
    }
    if let Some(cache) = cache {
        command.args(["--cache-dir", cache.to_str().unwrap()]);
    }
    let output = command
        .env("RAYON_NUM_THREADS", threads)
        .output()
        .expect("run mutation-matrix query");
    assert!(
        output.status.success(),
        "query failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    output
}

fn assert_incremental_mode_matches_clean(
    project: &TempProject,
    cache: &Path,
    mode: Option<&str>,
    threads: &str,
) {
    let clean = query_incremental_mode(project.path(), None, mode, "2");
    let cached = query_incremental_mode(project.path(), Some(cache), mode, threads);
    assert_eq!(cached.stdout, clean.stdout, "mode {mode:?} diverged");
}

fn clone_source(name: &str, operator: &str) -> String {
    format!(
        "def {name}(items):\n    total = 0\n    for item in items:\n        if item > 0:\n            total = total {operator} item * item\n    return total\n"
    )
}

fn prefixed_json<T: serde::de::DeserializeOwned>(output: &Output, prefix: &str) -> T {
    let stderr = String::from_utf8_lossy(&output.stderr);
    let line = stderr
        .lines()
        .find(|line| line.trim_start().starts_with(prefix))
        .unwrap_or_else(|| panic!("missing {prefix} stats in stderr: {stderr}"));
    let json = line
        .trim_start()
        .strip_prefix(prefix)
        .expect("checked prefix")
        .trim_start();
    serde_json::from_str(json).expect("valid prefixed stats JSON")
}

fn assert_warm_default_layers_reused(output: &Output) {
    let detection = detection_stats(output);
    assert!(detection.state_hit);
    assert_eq!(detection.scores_evaluated, 0);
    assert!(detection.scores_reused > 0);
    assert_eq!(detection.connected_evaluations_evaluated, 0);
    assert!(detection.connected_evaluations_reused > 0);
    assert!(detection.contiguous_streams_reused > 0);
    assert_eq!(detection.contiguous_streams_rebuilt, 0);

    let lines: LineIndexStats = prefixed_json(output, "[line-index]");
    assert_eq!(lines.schema, "nose.line-index/v1");
    assert_eq!(lines.files_reused, 3);
    assert_eq!(lines.files_rebuilt, 0);
    assert_eq!(lines.files_removed, 0);
    assert_eq!(lines.changed_document_frequencies, 0);

    let families: FamilyLineStats = prefixed_json(output, "[family-lines]");
    assert_eq!(families.schema, "nose.family-line-state/v1");
    assert!(families.families_reused > 0);
    assert_eq!(families.families_reweighted, 0);
    assert_eq!(families.families_rebuilt, 0);
}

#[test]
fn default_detection_state_matches_clean_across_warm_and_leaf_edit() {
    let project = TempProject::new("cache_incremental_default");
    let source = |name: &str, operator: &str| {
        format!(
            "def {name}(items):\n    total = 0\n    for item in items:\n        if item > 0:\n            total = total {operator} item * item\n        else:\n            total = total {operator} 1\n    return total\n"
        )
    };
    project.write("a/one.py", &source("one", "+"));
    project.write("b/two.py", &source("two", "+"));
    project.write("c/three.py", &source("three", "-"));
    let cache = project.path().join(".cache");

    let clean = query_default(project.path(), None);
    let cold = query_default(project.path(), Some(&cache));
    let warm = query_default(project.path(), Some(&cache));
    assert_eq!(cold.stdout, clean.stdout);
    assert_eq!(warm.stdout, clean.stdout);
    assert_warm_default_layers_reused(&warm);

    project.write("c/three.py", &format!("\n{}", source("third", "+")));
    let clean_changed = query_default(project.path(), None);
    let cached_changed = query_default(project.path(), Some(&cache));
    assert_eq!(cached_changed.stdout, clean_changed.stdout);
    let changed_stats = detection_stats(&cached_changed);
    assert!(changed_stats.state_hit);
    assert!(changed_stats.units_reused > 0);
    assert!(changed_stats.scores_reused > 0);
    assert!(changed_stats.scores_evaluated > 0);
}

#[test]
fn add_edit_delete_and_rename_match_clean_in_every_detection_mode() {
    for (label, mode) in [
        ("syntax", Some("syntax")),
        ("semantic", Some("semantic")),
        ("near", Some("near:0.70")),
        ("default", None),
    ] {
        let project = TempProject::new(&format!("cache_mutations_{label}"));
        project.write("a/one.py", &clone_source("one", "+"));
        project.write("b/two.py", &clone_source("two", "+"));
        let cache = project.path().join(".cache");
        assert_incremental_mode_matches_clean(&project, &cache, mode, "1");

        project.write("b/two.py", &clone_source("two", "-"));
        assert_incremental_mode_matches_clean(&project, &cache, mode, "4");
        project.write("c/three.py", &clone_source("three", "+"));
        assert_incremental_mode_matches_clean(&project, &cache, mode, "1");
        fs::rename(
            project.path().join("c/three.py"),
            project.path().join("c/renamed.py"),
        )
        .unwrap();
        assert_incremental_mode_matches_clean(&project, &cache, mode, "4");
        fs::remove_file(project.path().join("b/two.py")).unwrap();
        assert_incremental_mode_matches_clean(&project, &cache, mode, "1");
    }
}

#[test]
fn final_output_is_independent_of_edit_order_and_thread_count() {
    let project = TempProject::new("cache_edit_order");
    let cache_a = project.path().join(".cache-a");
    let cache_b = project.path().join(".cache-b");
    let initial_a = clone_source("one", "-");
    let initial_b = clone_source("two", "-");
    let final_a = clone_source("one", "+");
    let final_b = clone_source("two", "+");
    project.write("a/one.py", &initial_a);
    project.write("b/two.py", &initial_b);
    project.write("c/three.py", &clone_source("three", "+"));
    project.write("d/four.py", &clone_source("four", "+"));

    query_incremental_mode(project.path(), Some(&cache_a), None, "1");
    project.write("a/one.py", &final_a);
    query_incremental_mode(project.path(), Some(&cache_a), None, "4");
    project.write("b/two.py", &final_b);
    let order_ab = query_incremental_mode(project.path(), Some(&cache_a), None, "1");

    project.write("a/one.py", &initial_a);
    project.write("b/two.py", &initial_b);
    query_incremental_mode(project.path(), Some(&cache_b), None, "4");
    project.write("b/two.py", &final_b);
    query_incremental_mode(project.path(), Some(&cache_b), None, "1");
    project.write("a/one.py", &final_a);
    let order_ba = query_incremental_mode(project.path(), Some(&cache_b), None, "4");
    let clean = query_incremental_mode(project.path(), None, None, "2");

    assert_eq!(order_ab.stdout, clean.stdout);
    assert_eq!(order_ba.stdout, clean.stdout);
    assert_eq!(order_ab.stdout, order_ba.stdout);
}
