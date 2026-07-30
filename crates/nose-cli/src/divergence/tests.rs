mod targets;

use super::detect::{divergence_priority, ranges_touch, to_site};
use super::git::{
    parse_name_status, parse_new_side_ranges, parse_old_side_ranges, parse_patch_entries,
    DiffStatus,
};
use super::output::{divergence_items_json, divergence_sarif, fragment_context};
use super::{
    Divergence, DivergenceLane, DivergenceTier, Site, DIVERGENCE_LANE_VALUES,
    DIVERGENCE_SUPPRESSION_KIND_VALUES, DIVERGENCE_TAXONOMY_HINT_VALUES,
    DIVERGENCE_TIER_REASON_VALUES, DIVERGENCE_TIER_VALUES, DIVERGENT_EDIT_V2_POLICY,
};

use nose_detect::{EnclosingUnit, FragmentKind, LineSpan, Loc, LocInit, RefactorFamily};

// `git diff --unified=0` for: base "keep1\n-- marker\nkeep2a\nkeep2b\nzzz\n"
// → new "KEEP1\nkeep2a\nkeep2b\nZZZ\n". The deleted "-- marker" line shows in the body
// as "--- marker", which must NOT be parsed as a "--- a/path" file header.
const DIFF_WITH_DASHDASH_CONTENT: &str = "\
diff --git a/f.txt b/f.txt
index 1111111..2222222 100644
--- a/f.txt
+++ b/f.txt
@@ -1,2 +1 @@
-keep1
--- marker
+KEEP1
@@ -5 +4 @@
-zzz
+ZZZ
";

#[test]
fn parse_ignores_deleted_content_lines_that_look_like_headers() {
    let ranges = parse_old_side_ranges(DIFF_WITH_DASHDASH_CONTENT);
    let f = ranges.get("f.txt").expect("f.txt has changed ranges");
    assert!(f.contains(&(1, 2)), "first hunk: {f:?}");
    assert!(
        f.contains(&(5, 5)),
        "second hunk must survive the `--- marker` body line: {f:?}"
    );
    assert_eq!(
        ranges.len(),
        1,
        "no phantom file key from a content line: {ranges:?}"
    );
}

#[test]
fn pure_insertion_does_not_touch_a_member_ending_at_the_insertion_point() {
    // Insert a line after base line 1: `@@ -1,0 +2 @@`. The insertion sits *between*
    // base lines 1 and 2, so a member occupying only line 1 was not edited.
    let diff = "diff --git a/g.txt b/g.txt\n--- a/g.txt\n+++ b/g.txt\n@@ -1,0 +2 @@\n+inserted\n";
    let r = parse_old_side_ranges(diff);
    let ranges = r.get("g.txt").expect("g.txt range");
    assert!(
        !ranges_touch(ranges, 1, 1),
        "a member ending at the insertion point is not touched: {ranges:?}"
    );
    assert!(
        ranges_touch(ranges, 1, 3),
        "a member straddling the insertion gap IS touched: {ranges:?}"
    );
}

#[test]
fn new_side_ranges_include_added_file_lines() {
    let diff = "diff --git a/new.py b/new.py\nnew file mode 100644\n--- /dev/null\n+++ b/new.py\n@@ -0,0 +1,3 @@\n+def f():\n+    return 1\n+\n";
    let old = parse_old_side_ranges(diff);
    assert!(
        old.is_empty(),
        "added files have no base-side changed range: {old:?}"
    );
    let new = parse_new_side_ranges(diff);
    assert_eq!(
        new.get("new.py"),
        Some(&vec![(1, 3)]),
        "current-side ranges cover the added member: {new:?}"
    );
}

#[test]
fn side_ranges_trim_git_path_metadata_for_paths_with_spaces() {
    let diff = "\
diff --git a/src dir/old name.py b/src dir/old name.py
index b859599..ea74361 100644
--- a/src dir/old name.py\t
+++ b/src dir/old name.py\t
@@ -2 +2 @@ def f():
-    return 1
+    return 2
";
    let old = parse_old_side_ranges(diff);
    assert_eq!(
        old.get("src dir/old name.py"),
        Some(&vec![(2, 2)]),
        "base-side path should not retain git's tab separator: {old:?}"
    );
    let new = parse_new_side_ranges(diff);
    assert_eq!(
        new.get("src dir/old name.py"),
        Some(&vec![(2, 2)]),
        "current-side path should not retain git's tab separator: {new:?}"
    );
}

#[test]
fn name_status_tracks_current_paths_for_adds_and_renames() {
    let entries = parse_name_status("A\tnew.py\nR087\told.py\tmoved.py\nM\tsame.py\n");
    assert_eq!(entries[0].status, DiffStatus::Added);
    assert_eq!(entries[0].new_path.as_deref(), Some("new.py"));
    assert_eq!(entries[1].status, DiffStatus::Renamed);
    assert_eq!(entries[1].old_path.as_deref(), Some("old.py"));
    assert_eq!(entries[1].new_path.as_deref(), Some("moved.py"));
    assert_eq!(entries[2].status, DiffStatus::Modified);
}

#[test]
fn patch_entries_track_added_and_renamed_current_paths() {
    let diff = "\
diff --git a/new.py b/new.py
new file mode 100644
--- /dev/null
+++ b/new.py
@@ -0,0 +1 @@
+print('new')
diff --git a/old.py b/moved.py
similarity index 91%
rename from old.py
rename to moved.py
--- a/old.py
+++ b/moved.py
@@ -1 +1 @@
-print('old')
+print('moved')
";
    let entries = parse_patch_entries(diff);
    assert_eq!(entries[0].status, DiffStatus::Added);
    assert_eq!(entries[0].old_path, None);
    assert_eq!(entries[0].new_path.as_deref(), Some("new.py"));
    assert_eq!(entries[1].status, DiffStatus::Renamed);
    assert_eq!(entries[1].old_path.as_deref(), Some("old.py"));
    assert_eq!(entries[1].new_path.as_deref(), Some("moved.py"));
}

#[test]
fn patch_entries_track_copied_and_deleted_paths() {
    let diff = "\
diff --git a/template.py b/copied.py
similarity index 100%
copy from template.py
copy to copied.py
--- a/template.py
+++ b/copied.py
diff --git a/old.py b/old.py
deleted file mode 100644
--- a/old.py
+++ /dev/null
@@ -1 +0,0 @@
-print('old')
";
    let entries = parse_patch_entries(diff);
    assert_eq!(entries[0].status, DiffStatus::Copied);
    assert_eq!(entries[0].old_path.as_deref(), Some("template.py"));
    assert_eq!(entries[0].new_path.as_deref(), Some("copied.py"));
    assert_eq!(entries[1].status, DiffStatus::Deleted);
    assert_eq!(entries[1].old_path.as_deref(), Some("old.py"));
    assert_eq!(entries[1].new_path, None);
}

#[test]
fn patch_entries_track_added_paths_with_spaces() {
    let diff = "\
diff --git a/src dir/new copy.py b/src dir/new copy.py
new file mode 100644
index 0000000..b859599
--- /dev/null
+++ b/src dir/new copy.py\t
@@ -0,0 +1,2 @@
+def f():
+    return 1
";
    let entries = parse_patch_entries(diff);
    assert_eq!(entries[0].status, DiffStatus::Added);
    assert_eq!(entries[0].old_path, None);
    assert_eq!(entries[0].new_path.as_deref(), Some("src dir/new copy.py"));
}

fn fragment_loc(file: &str, start: u32, end: u32) -> Loc {
    let mut loc = Loc::new(LocInit {
        file: file.into(),
        source_span: LineSpan::new(start, end),
        lang: "rust".into(),
        kind: nose_il::UnitKind::Block,
        origin: Default::default(),
        name: None,
        sem: 4,
        span_tokens: 8,
    });
    loc.is_fragment = true;
    loc.fragment_kind = Some(FragmentKind::ConditionalGuard);
    loc.reason_code = Some(FragmentKind::ConditionalGuard.reason_code());
    loc.enclosing_unit = Some(EnclosingUnit {
        file: file.into(),
        start_line: 1,
        end_line: 20,
        kind: nose_il::UnitKind::Function,
        name: Some("owner".into()),
        unit_key: String::new(),
    });
    loc.enclosing_unit.as_mut().unwrap().refresh_unit_key();
    loc
}

fn divergence_family(locs: Vec<Loc>) -> RefactorFamily {
    RefactorFamily {
        value: 1.0,
        members: locs.len(),
        files: locs.len(),
        modules: 1,
        languages: 1,
        mean_score: 1.0,
        mean_lines: 4,
        dup_lines: 4,
        shared_lines: 4,
        params: 0,
        shared_weight: 4.0,
        locations: locs,
        direct_edges: Vec::new(),
        accepted_coverage: Vec::new(),
        display_params: None,
        mean_sem: 4.0,
        scope: "prod",
        discount: 1.0,
        abstraction_witness: None,
        witness: None,
        varying_spots: Vec::new(),
        semantic_laws: Vec::new(),
        semantic_pack_near: Vec::new(),
        semantic_pack_external_exact: Vec::new(),
    }
}

#[test]
fn fragment_context_names_enclosing_unit() {
    let site = to_site(&fragment_loc("src/a.rs", 8, 9));
    let context = fragment_context(&site).expect("fragment context");
    assert!(context.contains("conditional-guard fragment"));
    assert!(context.contains("`owner`"));
    assert!(context.contains("src/a.rs:1-20"));
}

#[test]
fn divergence_priority_promotes_fragment_surface() {
    let changed = fragment_loc("src/a.rs", 8, 11);
    let sibling = fragment_loc("src/b.rs", 8, 11);
    let family = divergence_family(vec![changed.clone(), sibling.clone()]);
    assert_eq!(family.recommended_surface(), "divergence");
    assert_eq!(
        divergence_priority(&family, &[&changed], &[&sibling]),
        3,
        "divergence-surface fragment hazards should rank before generic clone divergences"
    );
}

fn tier_site(file: &str, touches_shared: Option<bool>) -> Site {
    Site {
        file: file.into(),
        name: Some("f".into()),
        start_line: 1,
        end_line: 8,
        lang: "python".into(),
        kind: nose_il::UnitKind::Function,
        span_lines: 8,
        span_tokens: 24,
        is_fragment: false,
        fragment_kind: None,
        reason_code: None,
        enclosing_unit: None,
        touches_shared,
        semantic_change: None,
    }
}

fn tier_divergence(scope: &'static str, fire_eligible: bool, touch: Option<bool>) -> Divergence {
    Divergence {
        lane: DivergenceLane::BaseDivergence,
        family_id: "fam".into(),
        similarity: 1.0,
        hazard: 0.0,
        divergence_priority: 0,
        complexity: 24,
        scope,
        witness_kind: Some("copy-paste-run"),
        fire_eligible,
        graded: None,
        changed: vec![tier_site("a.py", touch)],
        not_updated: vec![tier_site("b.py", None)],
        targets: Vec::new(),
    }
}

#[test]
fn policy_adapter_normalizes_cli_scope_and_site_evidence() {
    let unproven = tier_divergence("prod", false, None);
    let not_touched = tier_divergence("prod", false, Some(false));
    let mut touched = tier_divergence("prod", true, Some(false));
    touched.changed.push(tier_site("c.py", Some(true)));
    let mixed = tier_divergence("mixed", true, Some(true));

    for (divergence, tier, taxonomy, reasons, fail_default) in [
        (
            unproven,
            DivergenceTier::Review,
            "unclear",
            vec!["shared_logic_unproven", "non_test_scope"],
            false,
        ),
        (
            not_touched,
            DivergenceTier::Review,
            "no_propagation_needed",
            vec!["shared_logic_not_touched", "non_test_scope"],
            false,
        ),
        (
            touched,
            DivergenceTier::Strict,
            "missed_propagation",
            vec!["shared_logic_touched", "non_test_scope"],
            true,
        ),
        (
            mixed,
            DivergenceTier::ReportOnly,
            "test_scaffolding",
            vec!["shared_logic_touched", "test_scope", "test_scaffolding"],
            false,
        ),
    ] {
        let decision = divergence.policy_decision();
        assert_eq!(decision.tier, tier);
        assert_eq!(decision.taxonomy_hint, taxonomy);
        assert_eq!(decision.tier_reasons, reasons);
        assert_eq!(decision.gate.fail_default, fail_default);
    }
}

#[test]
fn v8_contract_closed_enum_values_are_pinned() {
    assert_eq!(
        DIVERGENCE_LANE_VALUES,
        &["base-divergence", "new-copy"],
        "lane is a closed schema v8 enum; changing it requires a schema bump"
    );
    assert_eq!(
        DIVERGENCE_TIER_VALUES,
        &["strict", "review", "report-only", "suppressed"],
        "tier is a closed schema v8 enum; changing it requires a schema bump"
    );
    assert_eq!(
        DIVERGENCE_TIER_REASON_VALUES,
        &[
            "shared_logic_touched",
            "shared_logic_not_touched",
            "shared_logic_unproven",
            "non_test_scope",
            "test_scope",
            "variant_signal",
            "test_scaffolding",
            "grouping_artifact",
            "new_copy_no_base_member",
            "structured_ignore",
            "unclassified"
        ],
        "tier_reasons is a closed schema v8 enum; changing it requires a schema bump"
    );
    assert_eq!(
        DIVERGENCE_TAXONOMY_HINT_VALUES,
        &[
            "missed_propagation",
            "no_propagation_needed",
            "intentional_variant",
            "test_scaffolding",
            "grouping_artifact",
            "unclear"
        ],
        "taxonomy_hint is a closed schema v8 enum; changing it requires a schema bump"
    );
    assert_eq!(
        DIVERGENCE_SUPPRESSION_KIND_VALUES,
        &["structured-ignore"],
        "suppression.kind is a closed schema v8 enum; changing it requires a schema bump"
    );
}

#[test]
fn v8_json_and_sarif_share_policy_fields() {
    let mut new_copy = tier_divergence("prod", false, None);
    new_copy.lane = DivergenceLane::NewCopy;
    let cases = vec![
        tier_divergence("prod", true, Some(true)),
        tier_divergence("prod", false, Some(false)),
        tier_divergence("mixed", true, Some(true)),
        new_copy,
    ];
    let json_items = divergence_items_json(&cases);
    let sarif_doc: serde_json::Value =
        serde_json::from_str(&divergence_sarif(&cases, Some(0), "top=0").unwrap())
            .expect("divergence SARIF");
    let sarif_results = sarif_doc["runs"][0]["results"]
        .as_array()
        .expect("SARIF results");
    assert_eq!(json_items.len(), sarif_results.len());

    for (item, result) in json_items.iter().zip(sarif_results) {
        assert!(
            DIVERGENCE_LANE_VALUES.contains(&item["lane"].as_str().unwrap()),
            "JSON lane is in the v8 closed enum: {item}"
        );
        assert!(
            DIVERGENCE_TIER_VALUES.contains(&item["tier"].as_str().unwrap()),
            "JSON tier is in the v8 closed enum: {item}"
        );
        assert!(
            DIVERGENCE_TAXONOMY_HINT_VALUES.contains(&item["taxonomy_hint"].as_str().unwrap()),
            "JSON taxonomy_hint is in the v8 closed enum: {item}"
        );
        for reason in item["tier_reasons"].as_array().unwrap() {
            assert!(
                DIVERGENCE_TIER_REASON_VALUES.contains(&reason.as_str().unwrap()),
                "JSON tier_reason is in the v8 closed enum: {item}"
            );
        }

        assert_eq!(item["lane"], result["properties"]["lane"]);
        assert_eq!(item["tier"], result["properties"]["tier"]);
        assert_eq!(item["tier_reasons"], result["properties"]["tier_reasons"]);
        assert_eq!(item["taxonomy_hint"], result["properties"]["taxonomy_hint"]);
        assert_eq!(item["gate"], result["properties"]["gate"]);
        assert_eq!(item["fire_eligible"], result["properties"]["fire_eligible"]);
        assert_eq!(item["targets"], result["properties"]["targets"]);
        assert_eq!(item["gate"]["policy"], DIVERGENT_EDIT_V2_POLICY);
        assert_eq!(result["properties"]["policy"], DIVERGENT_EDIT_V2_POLICY);
        assert!(
            item["suppression"].is_null(),
            "active v8 output omits suppressed rows by default: {item}"
        );
    }
}
