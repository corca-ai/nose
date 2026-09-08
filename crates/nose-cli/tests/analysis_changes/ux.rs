use super::*;

fn action<'a>(report: &'a Value, kind: &str) -> &'a Value {
    &report["actions"]
        .as_array()
        .unwrap()
        .iter()
        .find(|a| a["kind"] == kind)
        .unwrap()["command"]
}

#[test]
fn incomplete_capture_preserves_diagnostics_for_offline_exploration() {
    let p = Project::new();
    p.write("legacy.h", "namespace engine { class Thing {}; }");
    let out = p.run(&[
        "query",
        ".",
        "--mode",
        "semantic",
        "--min-size",
        "1",
        "--min-lines",
        "1",
        "--save-analysis",
        "before.json",
    ]);
    assert!(out.status.success());
    let text = String::from_utf8_lossy(&out.stdout);
    assert!(
        text.contains("INCOMPLETE") && text.contains("legacy.h"),
        "{text}"
    );
    let artifact: Value =
        serde_json::from_slice(&std::fs::read(p.0.join("before.json")).unwrap()).unwrap();
    assert_eq!(artifact["skipped_sources"], 1);
    assert!(!artifact["source_diagnostics"][0]["reason"]
        .as_str()
        .unwrap()
        .is_empty());
    std::fs::copy(p.0.join("before.json"), p.0.join("after.json")).unwrap();
    for file in ["a.py", "b.py", "legacy.h"] {
        std::fs::remove_file(p.0.join(file)).unwrap();
    }
    p.write("nose.toml", "invalid [[[ config");
    let report = p.compare(&[]);
    assert_eq!(
        report["coverage"]["before"]["diagnostics"],
        artifact["source_diagnostics"]
    );
    assert_eq!(report["candidate_search_complete"], true);
    assert_eq!(report["complete"], false);
    assert!(!report["actions"]
        .as_array()
        .unwrap()
        .iter()
        .any(|a| a["kind"] == "increase-budget"));
    let mut legacy = artifact;
    legacy.as_object_mut().unwrap().remove("source_diagnostics");
    p.write("after.json", &legacy.to_string());
    let old = p.compare(&[]);
    assert_eq!(
        old["coverage"]["after"]["diagnostics_status"],
        "not-recorded"
    );
    assert_eq!(old["summary"]["retained"], 0);
}

#[test]
fn next_actions_preserve_format_group_and_offer_explicit_budget_recovery() {
    let p = Project::new();
    p.capture("before.json", &[]);
    p.capture("after.json", &[]);
    let grouped = p.compare(&["scope=prod", "group=reason", "top=1"]);
    let expanded = p.follow(action(&grouped, "expand-view"));
    assert_eq!(expanded["view"], "group");
    assert_eq!(expanded["group_field"], "reason");
    assert_eq!(expanded["inputs"], grouped["inputs"]);
    assert_eq!(
        expanded["summary"]["groups_shown"],
        expanded["summary"]["groups_total"]
    );
    let capped = p.compare(&["scope=prod", "group=reason", "--max-candidates", "0"]);
    let recovered = p.follow(action(&capped, "increase-budget"));
    assert_eq!(recovered["complete"], true);
    assert_eq!(recovered["view"], "group");
    assert_eq!(recovered["inputs"], capped["inputs"]);
    assert!(action(&capped, "increase-budget")
        .as_str()
        .unwrap()
        .contains("scope=prod"));
    let human = p.run(&[
        "query",
        "--before",
        "before.json",
        "--after",
        "after.json",
        "group=reason",
    ]);
    let output = String::from_utf8_lossy(&human.stdout);
    assert!(output.contains("--format human"));
    assert!(output.contains("Show all entries in this view"));
}

#[test]
fn landing_prioritizes_recheck_and_separates_global_and_selected_counts() {
    let p = Project::new();
    for n in 0..10 {
        let source = SOURCE.replace("+ 7", &format!("+ {}", 100 + n));
        for suffix in ["a", "b"] {
            p.write(&format!("{n}_{suffix}.py"), &source);
        }
    }
    p.capture("before.json", &[]);
    p.write("c.py", SOURCE);
    p.capture("after.json", &[]);
    let report = p.compare(&[]);
    assert!(report["summary"]["total"].as_u64().unwrap() > 5);
    assert!(report["summary"]["retained"].as_u64().unwrap() > 0);
    assert_eq!(report["items"][0]["unchanged_evidence"], false);
    let recheck = p.follow(action(&report, "recheck"));
    assert_eq!(recheck["summary"]["selected_retained"], 0);
    assert_eq!(
        recheck["summary"]["selected"],
        recheck["summary"]["selected_recheck"]
    );
    assert_eq!(
        recheck["summary"]["retained"],
        report["summary"]["retained"]
    );
    let empty = p.compare(&["scope=test"]);
    assert!(empty["empty_message"].as_str().unwrap().contains("filters"));
    assert_eq!(
        p.follow(action(&empty, "reset-filters"))["summary"]["selected"],
        report["summary"]["total"]
    );
}

#[test]
fn full_summarizes_location_and_membership_without_inventing_policy_changes() {
    let p = Project::new();
    p.capture("before.json", &[]);
    for file in ["a.py", "b.py"] {
        p.write(file, &format!("# header\n{SOURCE}"));
    }
    p.write("c.py", SOURCE);
    p.capture("after.json", &[]);
    let report = p.compare(&["full", "top=0"]);
    let row = report["items"]
        .as_array()
        .unwrap()
        .iter()
        .find(|r| !r["before"].is_null())
        .unwrap();
    for reason in [
        "scope-changed",
        "packs-changed",
        "analysis-changed",
        "member-content-changed",
    ] {
        assert!(
            !row["reasons"].as_array().unwrap().contains(&json!(reason)),
            "{row}"
        );
    }
    assert_eq!(row["member_changes"]["before_members"], 2);
    assert_eq!(row["member_changes"]["after_member_counts"], json!([3]));
    let members = row["member_changes"]["members"].as_array().unwrap();
    assert_eq!(
        members
            .iter()
            .filter(|m| m["status"] == "same-content-new-location")
            .count(),
        2
    );
    assert!(members
        .iter()
        .any(|m| m["status"] == "unmatched-current" && m["after"][0]["file"] == "c.py"));
    assert_eq!(row["unchanged_evidence"], false);
}

#[test]
fn missing_input_errors_name_the_side_and_expected_capture() {
    let p = Project::new();
    p.capture("before.json", &[]);
    for (before, after, side) in [
        ("absent-before.json", "before.json", "--before"),
        ("before.json", "absent-after.json", "--after"),
    ] {
        let out = p.run(&["query", "--before", before, "--after", after]);
        assert!(!out.status.success());
        let error = String::from_utf8_lossy(&out.stderr);
        assert!(
            error.contains(side) && error.contains("absent-") && error.contains("--save-analysis"),
            "{error}"
        );
    }
}

#[test]
fn human_comparison_keeps_context_in_full_and_counts_readable() {
    let p = Project::new();
    p.capture("before.json", &[]);
    p.write("c.py", SOURCE);
    p.capture("after.json", &[]);
    let args = ["query", "--before", "before.json", "--after", "after.json"];
    let output = p.run(&args);
    let compact = String::from_utf8(output.stdout).unwrap();
    assert!(compact.starts_with("Analysis comparison:"));
    assert!(compact.contains("observations") && compact.contains("Add `full`"));
    assert!(!compact.contains("Path bases:") && !compact.contains("Roots:"));
    assert!(!compact.contains("reason=review-evidence-retained ·"));
    let mut full = args.to_vec();
    full.push("full");
    let output = p.run(&full);
    let detail = String::from_utf8(output.stdout).unwrap();
    assert!(detail.contains("Path bases:") && detail.contains("Roots:"));
    assert!(detail.contains("Member counts: 2 → 3"), "{detail}");
    assert!(!detail.contains("Member counts: null") && !detail.contains("Member counts: 2 → ["));
}

#[test]
fn human_group_counts_do_not_report_zero_observations_shown() {
    let p = Project::new();
    p.capture("before.json", &[]);
    p.capture("after.json", &[]);
    let out = p.run(&[
        "query",
        "--before",
        "before.json",
        "--after",
        "after.json",
        "group=reason",
    ]);
    let text = String::from_utf8(out.stdout).unwrap();
    assert!(text.contains("group view below") && text.contains("Showing 1 / 1 groups"));
    assert!(!text.contains("0 shown") && !text.contains("1 observations"));
}

#[test]
fn saved_exploration_prints_identical_verified_source_once() {
    let p = Project::new();
    p.capture("before.json", &[]);
    let listing = p.json(&[
        "query",
        "--before",
        "before.json",
        "--after",
        "before.json",
        "--format",
        "json",
    ]);
    let change = format!("change={}", listing["items"][0]["id"].as_str().unwrap());
    let args = [
        "query",
        "--before",
        "before.json",
        "--after",
        "before.json",
        &change,
        "full",
        "--before-source",
        ".",
        "--after-source",
        ".",
    ];
    let output = p.run(&args);
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    let text = String::from_utf8(output.stdout).unwrap();
    assert!(text.starts_with("Saved analysis exploration:"), "{text}");
    assert_eq!(text.matches("b = a + 7").count(), 1, "{text}");
    assert!(text.contains("verified source unchanged"), "{text}");
    let mut json_args = args.to_vec();
    json_args.extend(["--format", "json"]);
    let report = p.json(&json_args);
    let row = &report["items"][0];
    let diffs = row["source_diffs"].as_array().unwrap();
    assert_eq!(diffs.len(), 2);
    assert!(diffs.iter().all(|diff| diff["same_content"] == true));
    for observation in [&row["before_observation"], &row["after_observations"][0]] {
        for member in observation["members"].as_array().unwrap() {
            assert!(member["source_body"]["text"]
                .as_str()
                .unwrap()
                .contains("b = a + 7"));
        }
    }
    let resumed = p.follow(action(&report, "resume-selection"));
    assert_eq!(resumed["inputs"], report["inputs"]);
    assert_eq!(resumed["items"], report["items"]);
}

#[test]
fn captured_external_root_sources_reopen_and_remap_to_a_historical_checkout() {
    let caller = Project::new();
    let target = Project::new();
    let historical = Project::new();
    caller.json(&[
        "query",
        target.0.to_str().unwrap(),
        "--mode",
        "semantic",
        "--min-size",
        "1",
        "--min-lines",
        "1",
        "--save-analysis",
        "external.json",
        "--format",
        "json",
    ]);
    let args = [
        "query",
        "--before",
        "external.json",
        "--after",
        "external.json",
        "full",
        "--format",
        "json",
    ];
    let detail = caller.json(&args);
    let verified = caller.follow(&detail["items"][0]["actions"][0]["command"]);
    assert_eq!(
        verified["items"][0]["source_lookup"],
        json!({"verified":4,"unavailable":0})
    );
    for side in ["before_observation", "after_observations"] {
        let observation = if side == "after_observations" {
            &verified["items"][0][side][0]
        } else {
            &verified["items"][0][side]
        };
        for member in observation["members"].as_array().unwrap() {
            assert_eq!(member["source_body"]["status"], "verified", "{member}");
        }
    }
    let change = format!("change={}", detail["items"][0]["id"].as_str().unwrap());
    target.write("a.py", "# changed since capture\n");
    let old = caller.json(&[
        "query",
        "--before",
        "external.json",
        "--after",
        "external.json",
        &change,
        "--before-source",
        historical.0.to_str().unwrap(),
        "--format",
        "json",
    ]);
    assert!(old["items"][0]["before_observation"]["members"]
        .as_array()
        .unwrap()
        .iter()
        .all(|m| m["source_body"]["status"] == "verified"));
    target.write("b.py", "# also changed since capture\n");
    let unavailable = caller.follow(&detail["items"][0]["actions"][0]["command"]);
    assert_eq!(
        unavailable["items"][0]["source_lookup"],
        json!({"verified":0,"unavailable":4})
    );
}

#[test]
fn source_actions_cover_external_file_and_multiple_roots() {
    let caller = Project::new();
    let a = Project::new();
    let b = Project::new();
    a.write(
        "a.py",
        &format!("{SOURCE}{}", SOURCE.replace("compute", "other")),
    );
    for (name, roots) in [
        ("file.json", vec![a.0.join("a.py")]),
        ("multi.json", vec![a.0.clone(), b.0.clone()]),
    ] {
        let mut args = vec![
            "query",
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
        for root in &roots {
            args.extend(["--root", root.to_str().unwrap()]);
        }
        caller.json(&args);
        let detail = caller.json(&[
            "query", "--before", name, "--after", name, "full", "--format", "json",
        ]);
        let verified = caller.follow(&detail["items"][0]["actions"][0]["command"]);
        assert_eq!(
            verified["items"][0]["source_lookup"]["unavailable"], 0,
            "{verified}"
        );
        assert!(
            verified["items"][0]["source_lookup"]["verified"]
                .as_u64()
                .unwrap()
                > 0
        );
    }
}
