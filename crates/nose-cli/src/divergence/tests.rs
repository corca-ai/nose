use super::detect::{divergence_priority, ranges_touch, to_site};
use super::git::parse_old_side_ranges;
use super::output::fragment_context;
use super::{Divergence, DivergenceTier, Site};

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
        mean_sem: 4.0,
        scope: "prod",
        discount: 1.0,
        abstraction_witness: None,
        witness: None,
        varying_spots: Vec::new(),
        semantic_laws: Vec::new(),
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
    }
}

fn tier_divergence(scope: &'static str, fire_eligible: bool, touch: Option<bool>) -> Divergence {
    Divergence {
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
    }
}

#[test]
fn v2_tier_routes_unknown_shared_evidence_to_review() {
    let d = tier_divergence("prod", false, None);
    assert_eq!(d.tier(), DivergenceTier::Review);
    assert!(!d.gate_fail_default(), "unknown evidence must fail closed");
    assert_eq!(d.taxonomy_hint(), "unclear");
    assert_eq!(
        d.tier_reasons(),
        vec!["shared_logic_unproven", "non_test_scope"]
    );
}

#[test]
fn v2_tier_routes_mixed_scope_to_report_only_even_when_legacy_fire_eligible() {
    let d = tier_divergence("mixed", true, Some(true));
    assert_eq!(d.tier(), DivergenceTier::ReportOnly);
    assert!(!d.gate_fail_default(), "report-only never fails default CI");
    assert_eq!(d.taxonomy_hint(), "test_scaffolding");
    assert_eq!(
        d.tier_reasons(),
        vec!["shared_logic_touched", "test_scope", "test_scaffolding"]
    );
}
