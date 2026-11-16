use super::*;

const INCLUDE: &str = "--include-inline-suppressed";

fn marked(mark: bool) -> String {
    format!(
        "# {}\n{SOURCE}",
        if mark { "nose-ignore" } else { "reviewed   " }
    )
}

// Source evidence deliberately differs when the marker differs. Compare the
// detected membership and witness, retaining source coordinates and analysis keys.
fn population(snapshot: &Value) -> Vec<String> {
    let mut rows = snapshot["families"]
        .as_array()
        .unwrap()
        .iter()
        .map(|family| {
            let mut members = family["members"]
                .as_array()
                .unwrap()
                .iter()
                .map(|member| {
                    json!([
                        member["file"],
                        member["lang"],
                        member["kind"],
                        member["start_line"],
                        member["end_line"],
                        member["analysis_key"]
                    ])
                    .to_string()
                })
                .collect::<Vec<_>>();
            members.sort();
            json!([family["witness"], members]).to_string()
        })
        .collect::<Vec<_>>();
    rows.sort();
    rows
}

#[test]
fn inline_suppression_saved_new_copy_matches_unmarked_controls_cold_and_warm() {
    let mut control = None;
    for (marked_base, marked_new) in [(false, false), (true, false), (false, true), (true, true)] {
        let p = Project::new();
        std::fs::remove_file(p.0.join("b.py")).unwrap();
        p.write("a.py", &format!("{}# base member\n", marked(marked_base)));
        let before = p.capture("before.json", &[INCLUDE, "--cache-dir", "cache"]);
        assert!(population(&before).is_empty());
        assert_eq!(before["profile"]["inline-suppression"], "include");
        let warm = p.capture("before-warm.json", &[INCLUDE, "--cache-dir", "cache"]);
        assert_eq!(before, warm);
        p.write("b.py", &format!("{}# new member\n", marked(marked_new)));
        let clean = p.capture("after-clean.json", &[INCLUDE]);
        let cold = p.capture("after.json", &[INCLUDE, "--cache-dir", "after-cache"]);
        let warm = p.capture("after-warm.json", &[INCLUDE, "--cache-dir", "after-cache"]);
        let incremental = p.capture("after-incremental.json", &[INCLUDE, "--cache-dir", "cache"]);
        assert_eq!(clean, cold);
        assert_eq!(cold, warm);
        assert_eq!(warm, incremental);
        let actual = population(&clean);
        assert!(!actual.is_empty());
        if let Some(expected) = &control {
            assert_eq!(&actual, expected);
        } else {
            control = Some(actual);
        }
        for (before_file, after_file) in [
            ("before.json", "after.json"),
            ("before-warm.json", "after-warm.json"),
        ] {
            let comparison = p.json(&[
                "query",
                "--before",
                before_file,
                "--after",
                after_file,
                "--format",
                "json",
                "top=0",
            ]);
            assert_eq!(comparison["profile_matches"], true);
            assert_eq!(comparison["complete"], true);
            assert!(comparison["items"]
                .as_array()
                .unwrap()
                .iter()
                .any(|item| item["correspondence"] == "unmatched-current"));
        }
        // The default remains suppression-aware, including a cache warmed by include.
        let respected = p.capture("respect.json", &["--cache-dir", "cache"]);
        assert_eq!(population(&respected).is_empty(), marked_base || marked_new);
        let included_again = p.capture("include-again.json", &[INCLUDE, "--cache-dir", "cache"]);
        assert_eq!(clean, included_again);
    }
}

#[test]
fn inline_suppression_policy_mismatch_and_legacy_capture_do_not_retain_evidence() {
    let p = Project::new();
    let before = p.capture("before.json", &[]);
    p.capture("after.json", &[INCLUDE]);
    let comparison = p.compare(&[]);
    assert_eq!(comparison["profile_matches"], false);
    assert!(comparison["items"]
        .as_array()
        .unwrap()
        .iter()
        .all(|item| item["unchanged_evidence"] == false));
    let mut legacy = before;
    legacy["profile"]
        .as_object_mut()
        .unwrap()
        .remove("inline-suppression");
    p.write("before.json", &legacy.to_string());
    assert_eq!(p.compare(&[])["profile_matches"], false);
    let output = p.run(&[
        "query",
        "--before",
        "before.json",
        "--after",
        "after.json",
        INCLUDE,
    ]);
    assert!(
        !output.status.success(),
        "offline comparison must use captured policy"
    );
}

fn live_ids(report: &Value) -> Vec<String> {
    let mut ids = report["families"]
        .as_array()
        .unwrap()
        .iter()
        .map(|family| family["id"].as_str().unwrap().to_owned())
        .collect::<Vec<_>>();
    ids.sort();
    ids
}

#[test]
fn inline_suppression_live_channels_navigation_and_leaf_cache_use_the_policy() {
    let p = Project::new();
    p.write("a.py", &marked(true));
    p.write("b.py", &marked(false));
    for mode in ["syntax", "semantic", "near"] {
        let query = |extra: &[&str]| {
            let mut args = vec![
                "query",
                ".",
                "--mode",
                mode,
                "--min-size",
                "1",
                "--min-lines",
                "1",
                "--format",
                "json",
                "all",
                "top=0",
            ];
            args.extend_from_slice(extra);
            let output = Command::new(env!("CARGO_BIN_EXE_nose"))
                .current_dir(&p.0)
                .env("NOSE_CACHE_STATS", "1")
                .env("RAYON_NUM_THREADS", "2")
                .args(&args)
                .output()
                .unwrap();
            assert!(
                output.status.success(),
                "{}",
                String::from_utf8_lossy(&output.stderr)
            );
            (
                serde_json::from_slice::<Value>(&output.stdout).unwrap(),
                String::from_utf8(output.stderr).unwrap(),
            )
        };
        let (clean, _) = query(&[INCLUDE]);
        // Exercise generated navigation, not just flag recognition.
        assert!(!clean["families"].as_array().unwrap().is_empty(), "{clean}");
        let family = &clean["families"][0];
        let action = clean["actions"]
            .as_array()
            .unwrap()
            .iter()
            .find(|action| action["kind"] == "open-family" && action["id"] == family["id"])
            .unwrap();
        let detail = p.follow(&action["command"]);
        assert_eq!(detail["view"], "family");
        assert!(detail["review_capture"]["command_prefix"]
            .as_str()
            .unwrap()
            .contains(INCLUDE));
        for attempt in 0..2 {
            let (cached, stats) = query(&[INCLUDE, "--cache-dir", "cache"]);
            assert_eq!(live_ids(&clean), live_ids(&cached));
            if attempt == 1 {
                assert!(stats.contains("hits=2 misses=0"), "{stats}");
            }
        }
        let (respected, _) = query(&["--cache-dir", "cache"]);
        assert!(respected["families"].as_array().unwrap().is_empty());
        assert_eq!(
            live_ids(&query(&[INCLUDE, "--cache-dir", "cache"]).0),
            live_ids(&clean)
        );
    }
    let before = p.capture("before.json", &[INCLUDE, "--cache-dir", "leaf-cache"]);
    p.write("a.py", &marked(true).replace("a + 7", "a - 7"));
    let after = p.capture("after.json", &[INCLUDE, "--cache-dir", "leaf-cache"]);
    assert_eq!(after, p.capture("clean.json", &[INCLUDE]));
    assert_ne!(population(&before), population(&after));
}

#[test]
fn inline_suppression_include_preserves_source_admission() {
    let p = Project::new();
    p.write("a.py", &marked(true));
    p.write("b.py", &marked(false));
    p.write("excluded.py", &marked(true));
    p.write("binary.py", &format!("{}\n# \0", marked(true)));
    let clean = p.capture("clean.json", &[INCLUDE, "--exclude", "excluded.py"]);
    assert_eq!(clean["complete"], false);
    assert_eq!(clean["skipped_sources"], 1);
    assert_eq!(
        clean["source_diagnostics"][0]["reason"],
        "binary-source-artifact"
    );
    assert!(clean["families"]
        .as_array()
        .unwrap()
        .iter()
        .flat_map(|f| f["members"].as_array().unwrap())
        .all(|m| m["file"] == "a.py" || m["file"] == "b.py"));
    assert_eq!(
        clean,
        p.capture(
            "cold.json",
            &[INCLUDE, "--exclude", "excluded.py", "--cache-dir", "cache"]
        )
    );
    assert_eq!(
        clean,
        p.capture(
            "warm.json",
            &[INCLUDE, "--exclude", "excluded.py", "--cache-dir", "cache"]
        )
    );
}

#[test]
fn inline_suppression_embedded_capture_keeps_original_source_identity() {
    let p = Project::new();
    for name in ["a.py", "b.py"] {
        std::fs::remove_file(p.0.join(name)).unwrap();
    }
    let component = "<script>\n// nose-ignore\nfunction compute(x) { const a = x * x; const b = a + 7; return b / 3; }\n</script>\n<style>\n/* nose-ignore */\n.a { color: red; padding: 4px; margin: 2px; }\n</style>\n<template><div>hello</div></template>\n";
    p.write("a.vue", component);
    p.write("b.vue", &component.replace("nose-ignore", "reviewed   "));
    let clean = p.capture("clean.json", &[INCLUDE]);
    let members = clean["families"]
        .as_array()
        .unwrap()
        .iter()
        .flat_map(|family| family["members"].as_array().unwrap())
        .collect::<Vec<_>>();
    for lang in ["javascript", "css"] {
        assert!(
            members
                .iter()
                .any(|m| m["file"] == "a.vue" && m["lang"] == lang),
            "missing {lang}: {clean}"
        );
    }
    let expected = nose_il::ContentDigest::sha256(component.as_bytes()).hex();
    for member in members.iter().filter(|m| m["file"] == "a.vue") {
        assert_eq!(member["source"]["source_digest"], expected);
    }
    assert_eq!(
        clean,
        p.capture("cold.json", &[INCLUDE, "--cache-dir", "cache"])
    );
    assert_eq!(
        clean,
        p.capture("warm.json", &[INCLUDE, "--cache-dir", "cache"])
    );
}

#[test]
fn inline_suppression_included_family_can_be_graded() {
    let p = Project::new();
    let fixtures = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../bench/type4/fixtures/async_sync/async_sync");
    for (output, fixture) in [
        ("a.py", "t1_handle_async.py"),
        ("b.py", "t1_handle_sync.py"),
    ] {
        let source = std::fs::read_to_string(fixtures.join(fixture)).unwrap();
        p.write(output, &format!("# nose-ignore\n{source}"));
    }
    let report = p.json(&[
        "query",
        ".",
        "all",
        "top=0",
        "spotclass=structural",
        "--mode",
        "near",
        "--min-size",
        "1",
        "--min-lines",
        "1",
        "--format",
        "json",
        INCLUDE,
    ]);
    assert!(
        report["families"].as_array().unwrap().iter().any(|family| {
            family["graded"]["patterns"]
                .as_array()
                .is_some_and(|patterns| patterns.iter().any(|p| p == "async-mirror"))
        }),
        "marked representatives must remain available to witness enrichment: {report}"
    );
}
