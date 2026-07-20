use super::*;
use serde::Deserialize;
use std::process::Output;

#[path = "cache/incremental.rs"]
mod incremental;

#[derive(Debug, Eq, PartialEq)]
struct CacheStats {
    files: usize,
    hits: usize,
    misses: usize,
}

#[derive(Debug, Deserialize)]
struct DetectionStats {
    schema: String,
    state_hit: bool,
    units_reused: usize,
    units_added: usize,
    units_removed: usize,
    buckets_rebuilt: usize,
    scores_reused: usize,
    scores_evaluated: usize,
    connected_evaluations_reused: usize,
    connected_evaluations_evaluated: usize,
    contiguous_streams_reused: usize,
    contiguous_streams_rebuilt: usize,
}

#[derive(Debug, Deserialize)]
struct InvalidationReport {
    schema: String,
    global_invalidations: Vec<String>,
    source_snapshots: LayerStats,
    raw_il: LayerStats,
    resolved_il: LayerStats,
    invalidated: Vec<InvalidatedRegion>,
    over_invalidated: Vec<String>,
    source_identities: SourceIdentityCounts,
}

#[derive(Debug, Deserialize)]
struct SourceIdentityCounts {
    git_blob: usize,
    content_sha256: usize,
}

#[derive(Debug, Deserialize)]
struct LayerStats {
    hits: usize,
    misses: usize,
}

#[derive(Debug, Deserialize)]
struct InvalidatedRegion {
    path: String,
    reasons: Vec<String>,
}

fn query(project: &Path, cache: Option<&Path>) -> Output {
    let mut command = Command::new(bin());
    command.args([
        "query",
        project.to_str().unwrap(),
        "all",
        "top=0",
        "--mode",
        "semantic",
        "--format",
        "json",
    ]);
    if let Some(cache) = cache {
        command.args(["--cache-dir", cache.to_str().unwrap()]);
        command.env("NOSE_CACHE_STATS", "1");
    }
    let output = command.output().expect("run cache equivalence query");
    assert!(
        output.status.success(),
        "query failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    output
}

fn cache_stats(output: &Output) -> CacheStats {
    let stderr = String::from_utf8_lossy(&output.stderr);
    let line = stderr
        .lines()
        .find(|line| line.trim_start().starts_with("[cache]"))
        .unwrap_or_else(|| panic!("missing cache stats in stderr: {stderr}"));
    let values = line
        .split_whitespace()
        .skip(1)
        .map(|field| {
            let (name, value) = field.split_once('=').expect("name=value cache statistic");
            (
                name,
                value.parse::<usize>().expect("numeric cache statistic"),
            )
        })
        .collect::<std::collections::HashMap<_, _>>();
    CacheStats {
        files: values["files"],
        hits: values["hits"],
        misses: values["misses"],
    }
}

fn invalidation_report(output: &Output) -> InvalidationReport {
    let stderr = String::from_utf8_lossy(&output.stderr);
    let line = stderr
        .lines()
        .find(|line| line.trim_start().starts_with("[invalidation]"))
        .unwrap_or_else(|| panic!("missing invalidation report in stderr: {stderr}"));
    let json = line
        .trim_start()
        .strip_prefix("[invalidation] ")
        .expect("invalidation report prefix");
    serde_json::from_str(json).expect("valid invalidation JSON")
}

fn detection_stats(output: &Output) -> DetectionStats {
    let stderr = String::from_utf8_lossy(&output.stderr);
    let line = stderr
        .lines()
        .find(|line| line.trim_start().starts_with("[detection]"))
        .unwrap_or_else(|| panic!("missing detection stats in stderr: {stderr}"));
    let json = line
        .trim_start()
        .strip_prefix("[detection] ")
        .expect("detection stats prefix");
    serde_json::from_str(json).expect("valid incremental detection JSON")
}

fn assert_warm_detection_reused(output: &Output) {
    let stats = detection_stats(output);
    assert_eq!(stats.schema, "nose.detection-incremental/v1");
    assert!(stats.state_hit);
    assert!(stats.units_reused > 0);
    assert_eq!(stats.units_added, 0);
    assert_eq!(stats.units_removed, 0);
    assert_eq!(stats.buckets_rebuilt, 0);
    assert_eq!(stats.scores_evaluated, 0);
    assert!(stats.scores_reused > 0);
}

#[test]
fn clone_shaped_files_keep_their_own_names_on_a_warm_hit() {
    let project = TempProject::new("cache_reporting_identity");
    let source = |name: &str| {
        format!(
            "def {name}(items):\n    total = 0\n    for item in items:\n        if item > 0:\n            total = total + item * item\n    return total\n"
        )
    };
    project.write("a/one.py", &source("one"));
    project.write("b/two.py", &source("two"));
    project.write("c/three.py", &source("three"));
    let cache = project.path().join(".cache");

    let clean = query(project.path(), None);
    let cold = query(project.path(), Some(&cache));
    let warm = query(project.path(), Some(&cache));

    assert_eq!(cold.stdout, clean.stdout);
    assert_eq!(warm.stdout, clean.stdout);
    assert_warm_detection_reused(&warm);
    let warm_invalidation = invalidation_report(&warm);
    assert_eq!(
        (
            warm_invalidation.resolved_il.hits,
            warm_invalidation.resolved_il.misses
        ),
        (3, 0)
    );
    assert!(warm_invalidation.invalidated.is_empty());
    assert!(warm_invalidation.global_invalidations.is_empty());
    assert_eq!(
        cache_stats(&warm),
        CacheStats {
            files: 3,
            hits: 3,
            misses: 0
        }
    );

    project.write("b/two.py", &format!("\n{}", source("second")));
    let clean_after = query(project.path(), None);
    let cached_after = query(project.path(), Some(&cache));
    assert_ne!(clean_after.stdout, clean.stdout);
    assert_eq!(cached_after.stdout, clean_after.stdout);
    let changed_detection = detection_stats(&cached_after);
    assert!(changed_detection.state_hit);
    assert!(changed_detection.units_added > 0);
    assert!(changed_detection.units_removed > 0);
    assert!(changed_detection.buckets_rebuilt > 0);
    assert_eq!(
        cache_stats(&cached_after),
        CacheStats {
            files: 3,
            hits: 2,
            misses: 1
        },
        "a unit-name and source-span-only edit must not reuse stale report metadata"
    );
}

#[test]
fn provider_edit_keeps_issue_275_output_equal_and_invalidates_the_importer() {
    let project = TempProject::new("cache_issue_275");
    project.write(
        "local.py",
        "def lookup(key, other):\n    return {\"red\": 1, \"blue\": 2}.get(key, 0)\n",
    );
    project.write("tables.py", "LOOKUP = {\"red\": 1, \"blue\": 2}\n");
    project.write(
        "imported.py",
        "from tables import LOOKUP\n\ndef lookup(key, other):\n    return LOOKUP.get(key, 0)\n",
    );
    let cache = project.path().join(".cache");

    let clean_seed = query(project.path(), None);
    let cached_seed = query(project.path(), Some(&cache));
    assert_eq!(cached_seed.stdout, clean_seed.stdout);
    assert_eq!(
        cache_stats(&cached_seed),
        CacheStats {
            files: 3,
            hits: 0,
            misses: 3
        }
    );

    project.write("tables.py", "LOOKUP = {\"red\": 9, \"blue\": 2}\n");
    let clean_after = query(project.path(), None);
    let cached_after = query(project.path(), Some(&cache));
    assert_ne!(
        clean_after.stdout, clean_seed.stdout,
        "the mutation must affect the result"
    );
    assert_eq!(cached_after.stdout, clean_after.stdout);
    assert_eq!(
        cache_stats(&cached_after),
        CacheStats {
            files: 3,
            hits: 1,
            misses: 2
        },
        "the unchanged local file should hit while the provider and resolved importer miss"
    );

    let warm_after = query(project.path(), Some(&cache));
    assert_eq!(warm_after.stdout, clean_after.stdout);
    assert_eq!(
        cache_stats(&warm_after),
        CacheStats {
            files: 3,
            hits: 3,
            misses: 0
        }
    );
}

#[test]
fn provider_internal_edit_does_not_invalidate_importers() {
    let project = TempProject::new("cache_export_surface");
    project.write(
        "tables.py",
        "LOOKUP = {\"red\": 1}\n\ndef helper():\n    return 1\n",
    );
    project.write(
        "imported.py",
        "from tables import LOOKUP\n\ndef lookup(key):\n    return LOOKUP.get(key, 0)\n",
    );
    let cache = project.path().join(".cache");
    query(project.path(), Some(&cache));

    project.write(
        "tables.py",
        "def helper():\n    return 9\n\nLOOKUP = {\"red\": 1}\n",
    );
    let output = query(project.path(), Some(&cache));
    let report = invalidation_report(&output);
    assert_eq!(report.schema, "nose.invalidation/v1");
    assert_eq!((report.raw_il.hits, report.raw_il.misses), (1, 1));
    assert_eq!((report.resolved_il.hits, report.resolved_il.misses), (1, 1));
    assert_eq!(report.invalidated.len(), 1);
    assert!(report.invalidated[0].path.ends_with("tables.py"));
    assert_eq!(report.invalidated[0].reasons, ["source-content"]);
}

#[test]
fn adding_an_earlier_path_does_not_invalidate_shifted_file_ids() {
    let project = TempProject::new("cache_file_id_shift");
    project.write("b.py", "VALUE = {\"answer\": 1}\n");
    project.write(
        "c.py",
        "from b import VALUE\n\ndef c():\n    return VALUE.get(\"answer\", 0)\n",
    );
    project.write("d.py", "def d(x):\n    return x + 3\n");
    let cache = project.path().join(".cache");
    query(project.path(), Some(&cache));

    project.write("a.py", "def a(x):\n    return x + 4\n");
    let output = query(project.path(), Some(&cache));
    let report = invalidation_report(&output);
    assert_eq!(
        (report.source_snapshots.hits, report.source_snapshots.misses),
        (3, 1)
    );
    assert_eq!((report.raw_il.hits, report.raw_il.misses), (3, 1));
    assert_eq!((report.resolved_il.hits, report.resolved_il.misses), (4, 0));
    assert_eq!(report.invalidated.len(), 1);
    assert!(report.invalidated[0].path.ends_with("a.py"));
    assert!(report
        .global_invalidations
        .iter()
        .any(|reason| reason == "discovery-membership"));
}

#[test]
fn swift_global_barrier_invalidates_every_swift_consumer() {
    let project = TempProject::new("cache_swift_barrier");
    project.write(
        "User.swift",
        "func positive(_ values: [Int]) -> Bool {\n  values.allSatisfy { $0 >= 0 }\n}\n",
    );
    project.write(
        "Twin.swift",
        "func alsoPositive(_ values: [Int]) -> Bool {\n  values.allSatisfy { $0 >= 0 }\n}\n",
    );
    let cache = project.path().join(".cache");
    query(project.path(), Some(&cache));

    project.write(
        "Overload.swift",
        "extension Array where Element == Int {\n  func allSatisfy(_ predicate: (Int) -> Bool) -> Bool { false }\n}\n",
    );
    let output = query(project.path(), Some(&cache));
    let report = invalidation_report(&output);
    assert_eq!((report.raw_il.hits, report.raw_il.misses), (2, 1));
    assert_eq!((report.resolved_il.hits, report.resolved_il.misses), (0, 3));
    assert_eq!(report.invalidated.len(), 3);
    assert!(report.invalidated.iter().any(|region| region
        .reasons
        .iter()
        .any(|reason| reason == "swift-global-sentinel")));
}

#[test]
fn unknown_imports_fail_safe_and_report_over_invalidation() {
    let project = TempProject::new("cache_unknown_dependency");
    project.write(
        "consumer.py",
        "from absent import VALUE\n\ndef value():\n    return VALUE\n",
    );
    project.write("other.py", "EXPORTED = 1\n");
    let cache = project.path().join(".cache");
    query(project.path(), Some(&cache));

    project.write("other.py", "EXPORTED = 2\n");
    let output = query(project.path(), Some(&cache));
    let report = invalidation_report(&output);
    assert_eq!((report.raw_il.hits, report.raw_il.misses), (1, 1));
    assert_eq!((report.resolved_il.hits, report.resolved_il.misses), (1, 1));
    assert_eq!(
        cache_stats(&output),
        CacheStats {
            files: 2,
            hits: 0,
            misses: 2,
        }
    );
    assert!(report
        .over_invalidated
        .iter()
        .any(|path| path.ends_with("consumer.py")));
    assert!(report.invalidated.iter().any(|region| {
        region.path.ends_with("consumer.py")
            && region
                .reasons
                .iter()
                .any(|reason| reason == "unknown-dependency-over-invalidation")
    }));
}

#[test]
fn clean_tracked_sources_use_git_blob_identity_but_dirty_sources_use_content() {
    let project = TempProject::new("cache_git_identity");
    project.write("tracked.py", "def value(x):\n    return x + 1\n");
    for args in [
        &["init", "-q"][..],
        &["config", "user.email", "cache@example.invalid"][..],
        &["config", "user.name", "Cache Test"][..],
        &["add", "tracked.py"][..],
        &["commit", "-qm", "fixture"][..],
    ] {
        let status = Command::new("git")
            .arg("-C")
            .arg(project.path())
            .args(args)
            .status()
            .expect("run git fixture command");
        assert!(status.success());
    }
    let cache = project.path().join(".cache");
    let clean = query(project.path(), Some(&cache));
    let clean_report = invalidation_report(&clean);
    assert_eq!(clean_report.source_identities.git_blob, 1);
    assert_eq!(clean_report.source_identities.content_sha256, 0);

    project.write("tracked.py", "def value(x):\n    return x - 1\n");
    let dirty = query(project.path(), Some(&cache));
    let dirty_report = invalidation_report(&dirty);
    assert_eq!(dirty_report.source_identities.git_blob, 0);
    assert_eq!(dirty_report.source_identities.content_sha256, 1);
}

#[test]
fn a_new_ambiguous_provider_invalidates_the_import_consumer_fail_safe() {
    let project = TempProject::new("cache_provider_ambiguity");
    project.write("tables.py", "VALUE = {\"answer\": 1}\n");
    project.write(
        "consumer.py",
        "from tables import VALUE\n\ndef answer():\n    return VALUE.get(\"answer\", 0)\n",
    );
    let cache = project.path().join(".cache");
    query(project.path(), Some(&cache));

    project.write("duplicate/tables.py", "VALUE = {\"answer\": 1}\n");
    let clean = query(project.path(), None);
    let cached = query(project.path(), Some(&cache));
    assert_eq!(cached.stdout, clean.stdout);
    let report = invalidation_report(&cached);
    assert!(report.invalidated.iter().any(|region| {
        region.path.ends_with("consumer.py")
            && region
                .reasons
                .iter()
                .any(|reason| reason == "unknown-dependency-over-invalidation")
    }));
    assert!(report
        .over_invalidated
        .iter()
        .any(|path| path.ends_with("consumer.py")));
}

#[test]
fn a_new_go_namespace_export_invalidates_a_partially_resolved_consumer() {
    let project = TempProject::new("cache_go_partial_namespace");
    project.write(
        "tables.go",
        "package tables\nvar Lookup = map[string]int{\"red\": 1}\n",
    );
    project.write(
        "consumer.go",
        "package consumer\nimport \"tables\"\nfunc value(key string) int { return tables.Lookup[key] + tables.Missing[key] }\n",
    );
    let cache = project.path().join(".cache");
    query(project.path(), Some(&cache));

    project.write(
        "tables.go",
        "package tables\nvar Lookup = map[string]int{\"red\": 1}\nvar Missing = map[string]int{\"red\": 2}\n",
    );
    let clean = query(project.path(), None);
    let cached = query(project.path(), Some(&cache));
    assert_eq!(cached.stdout, clean.stdout);
    let report = invalidation_report(&cached);
    assert!(report.invalidated.iter().any(|region| {
        region.path.ends_with("consumer.go")
            && region
                .reasons
                .iter()
                .any(|reason| reason == "dependency-export")
    }));
    assert_eq!(
        cache_stats(&cached),
        CacheStats {
            files: 2,
            hits: 0,
            misses: 2,
        }
    );
}
