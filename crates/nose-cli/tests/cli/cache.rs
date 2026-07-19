use super::*;
use std::process::Output;

#[derive(Debug, Eq, PartialEq)]
struct CacheStats {
    files: usize,
    hits: usize,
    misses: usize,
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
