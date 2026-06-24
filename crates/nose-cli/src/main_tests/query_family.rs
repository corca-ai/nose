use super::surface_hints::{fam, fam_kind};
use super::*;

fn loc_at(file: &str, start: u32, end: u32, kind: nose_il::UnitKind) -> Loc {
    Loc::new(LocInit {
        file: file.to_string(),
        source_span: LineSpan::new(start, end),
        lang: "go".into(),
        kind,
        origin: Default::default(),
        name: None,
        sem: 50,
        span_tokens: 50,
    })
}

fn fam_at(spans: &[(&str, u32, u32)]) -> RefactorFamily {
    let mut f = fam_kind(1, 1, &vec![None; spans.len()], nose_il::UnitKind::Block);
    f.locations = spans
        .iter()
        .map(|(file, s, e)| loc_at(file, *s, *e, nose_il::UnitKind::Block))
        .collect();
    f
}

#[test]
fn compiled_css_pipeline_demotes_source_plus_outputs_but_not_cross_source() {
    let gen: std::collections::HashSet<String> = [
        "css/bundle.css".to_string(),
        "css/bundle.min.css".to_string(),
    ]
    .into_iter()
    .collect();
    // 1 source partial + its compiled + minified outputs → build pipeline (demote).
    let pipe = fam_at(&[
        ("src/_a.css", 1, 9),
        ("css/bundle.css", 100, 108),
        ("css/bundle.min.css", 1, 1),
    ]);
    assert!(family_is_compiled_css_pipeline(&pipe, &gen));
    let ov = SurfaceOverrides {
        generated_sources: gen.clone(),
        declaration_run_ids: std::collections::HashSet::new(),
    };
    assert_eq!(effective_surface(&pipe, &ov), "generated");
    assert!(
        !is_default_report_family(&pipe, &ov),
        "CSS build-pipeline families stay off query's default surface"
    );
    assert_eq!(
        family_actionability_reason(&pipe, &ov),
        Some("generated-source")
    );
    assert_eq!(
        surface_omission_note(std::slice::from_ref(&pipe), &ov).as_deref(),
        Some("omitted 1 family from default output (1 generated-code)")
    );
    // 2 distinct hand-written sources sharing a block (+ a compiled copy) → keep.
    let dedup = fam_at(&[
        ("src/_a.css", 1, 9),
        ("src/_b.css", 1, 9),
        ("css/bundle.css", 100, 108),
    ]);
    assert!(!family_is_compiled_css_pipeline(&dedup, &gen));
    // all-compiled also matches (subsumes the all-generated case for CSS).
    let allc = fam_at(&[("css/bundle.css", 1, 9), ("css/bundle.min.css", 1, 1)]);
    assert!(family_is_compiled_css_pipeline(&allc, &gen));
    // a non-CSS member disqualifies — this rule is CSS-only.
    let mixed = fam_at(&[("src/_a.css", 1, 9), ("app.js", 1, 9)]);
    assert!(!family_is_compiled_css_pipeline(&mixed, &gen));
}

#[test]
fn overlapping_slices_fold_under_their_primary() {
    // B's members are both shifted slices of A's regions → one opportunity.
    // C shares only ONE region with A (its other member lives elsewhere) —
    // a single shared region can be coincidence, so C stays its own entry.
    let a = fam_at(&[("t/a.go", 100, 130), ("t/b.go", 50, 70)]);
    let b = fam_at(&[("t/a.go", 105, 128), ("t/b.go", 52, 66)]);
    let c = fam_at(&[("t/a.go", 100, 130), ("t/z.go", 5, 25)]);
    let ranked = [&a, &b, &c];
    let groups = OpportunityGroups::from_ranked(&ranked);
    assert!(groups.is_slice(&b), "b is a slice of a");
    assert!(
        !groups.is_slice(&a),
        "the best-ranked family is the primary"
    );
    assert!(!groups.is_slice(&c), "one shared region must not group");
    assert_eq!(
        groups.slices(&a),
        Some(&[baseline::family_id(&b)][..]),
        "a lists exactly b as its folded slice"
    );
}

#[test]
fn query_family_json_carries_fold_navigation() {
    // a subsumes b (b's two members are shifted slices of a's regions).
    let a = fam_at(&[("t/a.go", 100, 130), ("t/b.go", 50, 70)]);
    let b = fam_at(&[("t/a.go", 105, 128), ("t/b.go", 52, 66)]);
    let ranked = [&a, &b];
    let opp = OpportunityGroups::from_ranked(&ranked);
    let ov = SurfaceOverrides {
        generated_sources: std::collections::HashSet::new(),
        declaration_run_ids: std::collections::HashSet::new(),
    };
    // The primary lists the slice ids it subsumes (navigable id= handles).
    let ja = query_family_json(&a, &ov, &opp, false, None, None);
    assert_eq!(
        ja["subsumes"],
        serde_json::json!([short_id(&baseline::family_id(&b))]),
        "primary names the slices it subsumes: {ja}"
    );
    assert!(ja.get("subsumed_by").is_none(), "a primary is not subsumed");
    // The slice points back at its primary.
    let jb = query_family_json(&b, &ov, &opp, false, None, None);
    assert_eq!(
        jb["subsumed_by"],
        serde_json::Value::from(short_id(&baseline::family_id(&a))),
        "slice points at its primary: {jb}"
    );
}

#[test]
fn classify_param_hints_value_class() {
    assert_eq!(classify_param(&["  42"]), "literal");
    assert_eq!(classify_param(&["\"hello\""]), "literal");
    assert_eq!(classify_param(&["foo.bar"]), "name");
    assert_eq!(classify_param(&["compute(x, y)"]), "call");
    assert_eq!(classify_param(&["a + b * c"]), "expr");
    assert_eq!(classify_param(&["line one", "line two"]), "block");
    assert_eq!(classify_param(&[]), "expr");
}

#[test]
fn query_family_json_carries_proof_depth() {
    let ov = SurfaceOverrides {
        generated_sources: std::collections::HashSet::new(),
        declaration_run_ids: std::collections::HashSet::new(),
    };
    let empty = OpportunityGroups::default();
    // Exact channel: how much is proven identical (the shared value-multiset size).
    let mut exact = fam(1, 2, &[Some("a"), Some("b")]);
    exact.witness = Some(nose_detect::EquivalenceWitness {
        kind: "exact-value-graph",
        value_nodes: Some(12),
        mean_value_jaccard: None,
        mean_shape_jaccard: None,
        graded: None,
        graded_pair: None,
    });
    let je = query_family_json(&exact, &ov, &empty, false, None, None);
    assert_eq!(
        je["value_nodes"], 12,
        "exact family carries value_nodes: {je}"
    );
    // Sub-dag channel: the proven shared-computation span per location.
    let mut sub = fam(1, 2, &[Some("c"), Some("d")]);
    sub.locations[0].shared_subdag = Some((10, 14));
    let js = query_family_json(&sub, &ov, &empty, false, None, None);
    assert_eq!(
        js["locations"][0]["shared_subdag"],
        serde_json::json!([10, 14]),
        "location carries the proven shared-subdag span: {js}"
    );
}

#[test]
fn query_family_json_carries_raw_detector_metrics() {
    let ov = SurfaceOverrides {
        generated_sources: std::collections::HashSet::new(),
        declaration_run_ids: std::collections::HashSet::new(),
    };
    let empty = OpportunityGroups::default();
    let mut f = fam(2, 3, &[Some("a"), Some("b"), Some("c")]);
    f.mean_sem = 123.5;
    f.mean_score = 0.82;
    f.mean_lines = 17;
    f.shared_weight = 11.25;
    f.dup_lines = 34;
    f.shared_lines = 9;
    f.params = 4;
    f.value = 456.0;
    f.scope = "mixed";

    let j = query_family_json(&f, &ov, &empty, false, None, None);
    assert_eq!(
        j["metrics"],
        serde_json::json!({
            "mean_sem": 123.5,
            "members": 3,
            "modules": 3,
            "files": 3,
            "languages": 2,
            "mean_score": 0.82,
            "mean_lines": 17,
            "shared_weight": 11.25,
            "params": 4,
            "scope": "mixed",
            "value": 456.0,
            "dup_lines": 34,
            "shared_lines": 9,
        }),
        "query-json family metrics preserve detector features for evaluation tooling: {j}"
    );
}

#[test]
fn hint_prefers_calling_the_existing_helper() {
    let mut f = fam(1, 2, &[None, None, None]);
    f.locations = vec![
        {
            let mut l = loc_at("core/math.ts", 10, 14, nose_il::UnitKind::Function);
            l.name = Some("clamp".to_string());
            l
        },
        loc_at("ui/model.ts", 80, 84, nose_il::UnitKind::Block),
        loc_at("worker/job.ts", 33, 37, nose_il::UnitKind::Block),
    ];
    assert_eq!(
        family_hint(&f),
        "2 sites reimplement `clamp` — call the existing helper (core/math.ts)"
    );
}

#[test]
fn existing_helper_names_the_call_target_member() {
    // A call-existing-helper family: one named function + inline copies that recompute it.
    let mut f = fam(1, 2, &[None, None, None]);
    f.locations = vec![
        {
            let mut l = loc_at("core/math.ts", 10, 14, nose_il::UnitKind::Function);
            l.name = Some("clamp".to_string());
            l
        },
        loc_at("ui/model.ts", 80, 84, nose_il::UnitKind::Block),
        loc_at("worker/job.ts", 33, 37, nose_il::UnitKind::Block),
    ];
    let helper = family_existing_helper(&f).expect("call-existing-helper has a helper member");
    assert_eq!(helper.name.as_deref(), Some("clamp"));
    assert_eq!(helper.file, "core/math.ts");
    // A plain multi-function family is a fresh extraction — there is no member to call.
    assert!(family_existing_helper(&fam(1, 2, &[Some("a"), Some("b")])).is_none());
}

#[test]
fn spotclass_grades_near_family_holes() {
    use nose_detect::{EquivalenceWitness, GradedWitness, WitnessHole};
    let hole = |class: &'static str| WitnessHole {
        class,
        a_lines: None,
        b_lines: None,
        effect: false,
        a_text: String::new(),
        b_text: String::new(),
    };
    let graded = |spots: Vec<WitnessHole>, referent: Vec<String>| {
        let mut f = fam(1, 2, &[Some("x"), Some("y")]);
        f.witness = Some(EquivalenceWitness {
            kind: "structural-similarity",
            value_nodes: None,
            mean_value_jaccard: None,
            mean_shape_jaccard: None,
            graded: Some(GradedWitness {
                holes: spots.len(),
                spots,
                patterns: Vec::new(),
                referent_mismatches: referent,
                caveat_names: Vec::new(),
                equal_modulo_holes: true,
                modeled_caveat: false,
            }),
            graded_pair: None,
        });
        f
    };
    // Only value-leaf holes → a clean parameterize/extract candidate.
    assert_eq!(
        family_spotclass(&graded(vec![hole("literal"), hole("call")], vec![])),
        Some("leaf-only")
    );
    // A shape/arity hole → genuine logic divergence, not just a parameter.
    assert_eq!(
        family_spotclass(&graded(vec![hole("literal"), hole("shape")], vec![])),
        Some("structural")
    );
    // A referent mismatch (same name, behaviorally distinct) → structural even with leaf holes.
    assert_eq!(
        family_spotclass(&graded(vec![hole("literal")], vec!["equals".into()])),
        Some("structural")
    );
    // A transformation twin may have leaf-shaped holes, but if the witness is demoted
    // it is not a clean parameterize/extract candidate.
    let mut demoted = graded(vec![hole("call")], vec![]);
    let g = demoted
        .witness
        .as_mut()
        .and_then(|w| w.graded.as_mut())
        .unwrap();
    g.equal_modulo_holes = false;
    g.patterns.push("async-mirror");
    assert_eq!(family_spotclass(&demoted), Some("structural"));
    // No graded witness (not enriched / not a near family) → no class.
    assert!(family_spotclass(&fam(1, 1, &[Some("a"), Some("b")])).is_none());
}

#[test]
fn helper_hint_never_points_prod_at_a_test_helper() {
    // Coevo C2: the named function lives in test code while the inline
    // copies are production — "call the existing helper" would be wrong-
    // direction advice, so the hint falls back to plain extraction.
    let mut f = fam(1, 2, &[None, None, None]);
    f.scope = "mixed";
    f.locations = vec![
        {
            let mut l = loc_at("tests/helpers.ts", 10, 14, nose_il::UnitKind::Function);
            l.name = Some("clamp".to_string());
            l
        },
        loc_at("ui/model.ts", 80, 84, nose_il::UnitKind::Block),
        loc_at("worker/job.ts", 33, 37, nose_il::UnitKind::Block),
    ];
    let hint = family_hint(&f);
    assert!(
        !hint.contains("call the existing helper"),
        "a test-code helper must not be recommended to prod copies: {hint}"
    );
    // All-test families may keep the recommendation: tests calling a test
    // helper is exactly the refactor.
    f.scope = "test";
    assert!(
        family_hint(&f).contains("call the existing helper"),
        "an all-test family may still consolidate on its test helper"
    );
}

#[test]
fn helper_hint_allows_test_copies_to_call_a_prod_helper() {
    // C5 boundary: the inverse direction is fine — tests calling a
    // production helper is exactly the refactor.
    let mut f = fam(1, 2, &[None, None]);
    f.scope = "mixed";
    f.locations = vec![
        {
            let mut l = loc_at("core/math.ts", 10, 14, nose_il::UnitKind::Function);
            l.name = Some("clamp".to_string());
            l
        },
        loc_at("tests/model.spec.ts", 80, 84, nose_il::UnitKind::Block),
    ];
    assert!(
        family_hint(&f).contains("call the existing helper"),
        "prod helper recommended to test copies is the right direction"
    );
}

#[test]
fn high_parameter_caution_boundary_is_six() {
    // S3-C5 gap: the >= boundary itself was untested.
    let mut f = fam(1, 1, &[None, None]);
    f.shared_lines = 30;
    f.params = 5;
    assert!(
        !family_hint(&f).contains("high-parameter"),
        "five spots is below the caution boundary"
    );
    f.params = 6;
    assert!(
        family_hint(&f).contains("high-parameter (6 varying spots)"),
        "six spots is the boundary and must carry the caution"
    );
}

#[test]
fn helper_hint_carries_the_high_parameter_caution() {
    // S2-C2: the early return must not bypass the params caution — six
    // varying spots mean the inline copies diverge from the helper.
    let mut f = fam(1, 2, &[None, None, None]);
    f.params = 8;
    f.shared_lines = 12;
    f.locations = vec![
        {
            let mut l = loc_at("core/math.ts", 10, 14, nose_il::UnitKind::Function);
            l.name = Some("clamp".to_string());
            l
        },
        loc_at("ui/model.ts", 80, 84, nose_il::UnitKind::Block),
        loc_at("worker/job.ts", 33, 37, nose_il::UnitKind::Block),
    ];
    let hint = family_hint(&f);
    assert!(
        hint.contains("call the existing helper") && hint.contains("high-parameter (8"),
        "helper advice at 8 varying spots must carry the caution: {hint}"
    );
}

#[test]
fn helper_hint_never_points_at_generated_code() {
    let mut f = fam(1, 2, &[None, None]);
    f.locations = vec![
        {
            let mut l = loc_at("gen/api.ts", 10, 14, nose_il::UnitKind::Function);
            l.name = Some("encode".to_string());
            l.looks_generated = true;
            l
        },
        loc_at("ui/model.ts", 80, 84, nose_il::UnitKind::Block),
    ];
    let hint = family_hint(&f);
    assert!(
        !hint.contains("call the existing helper"),
        "a generated-file helper is not the maintainer's API: {hint}"
    );
}

#[test]
fn hint_flags_high_parameter_extractions() {
    let mut f = fam(1, 1, &[None, None]);
    f.params = 8;
    f.shared_lines = 12;
    let hint = family_hint(&f);
    assert!(
        hint.contains("high-parameter (8 varying spots)"),
        "an 8-spot extraction must carry the readability caution: {hint}"
    );
}

#[test]
fn summary_names_the_equivalence_evidence() {
    let mut f = fam(1, 1, &[None, None]);
    f.witness = Some(nose_detect::EquivalenceWitness {
        kind: "exact-value-graph",
        value_nodes: Some(12),
        mean_value_jaccard: None,
        mean_shape_jaccard: None,
        graded: None,
        graded_pair: None,
    });
    assert!(
        family_summary(&f).contains("· exact behavior match"),
        "the human line names WHY the members merged: {}",
        family_summary(&f)
    );
}
