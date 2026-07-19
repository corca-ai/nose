use super::*;

#[test]
fn compiled_css_is_detected_but_hand_written_is_not() {
    // Distributed / compiled stylesheets carry build markers → treated as generated.
    assert!(looks_compiled_css(
        "dist/app.css",
        "/*! App v1.2.3 | MIT */\n.a{x:1}"
    ));
    // minified bundle: banner collapsed behind a leading @charset on one line
    assert!(looks_compiled_css(
        "css/pico.amber.css",
        "@charset \"UTF-8\";/*! Pico CSS v2.1.1 */\n.x{y:1}"
    ));
    assert!(looks_compiled_css(
        "css/sakura.css",
        "/* Sakura.css v1.5.1 */\nhtml{x:1}"
    ));
    assert!(looks_compiled_css("a/b.min.css", ".x{y:1}"));
    assert!(looks_compiled_css(
        "css/bulma.css",
        "@charset \"UTF-8\";\n.x{y:1}\n/*# sourceMappingURL=bulma.css.map */"
    ));
    // Hand-written application/source CSS has none of these markers → NOT generated.
    assert!(!looks_compiled_css(
        "src/styles/app.css",
        "/* app styles */\n.card { padding: 1rem; }\n.btn { color: red; }"
    ));
    assert!(!looks_compiled_css(
        "src/parts/_range.css",
        "input[type=range]{ width:100% }"
    ));
    // A preprocessor SOURCE file is the input, not compiled output.
    assert!(!looks_compiled_css(
        "scss/_buttons.css",
        "/*! v1.0 */\n.b{x:1}"
    ));
    // Non-CSS is never matched here.
    assert!(!looks_compiled_css("app.js", "/*! lib v1.2.3 */"));
    assert!(has_version_tag("Sakura.css v1.5.1"));
    assert!(has_version_tag("Pico v2.1"));
    assert!(!has_version_tag("version two point oh"));
    assert!(!has_version_tag("v2 final")); // no dotted minor → not a release tag
}

#[test]
fn jazzy_provenance_requires_both_bounded_html_signal_classes() {
    let beyond_header_lines = format!(
        "{}<link href=\"css/JAZZY.CSS\"><a class=\"dashAnchor\"></a>",
        "ordinary documentation\n".repeat(12)
    );
    assert!(head_has_jazzy_generated_provenance(
        "docs/Types/Foo.HTML",
        beyond_header_lines.as_bytes(),
    ));
    assert!(head_has_jazzy_generated_provenance(
        "docs/Foo.html",
        b"<script src=\"jazzy.js\"></script><!-- //apple_ref/swift/cl/Foo -->",
    ));
    assert!(!head_has_jazzy_generated_provenance(
        "docs/Foo.html",
        b"<link href=\"jazzy.css\"><main>hand written</main>",
    ));
    assert!(!head_has_jazzy_generated_provenance(
        "docs/Foo.html",
        b"<a class=\"dashanchor\">hand written</a>",
    ));
    assert!(!head_has_jazzy_generated_provenance(
        "docs/Foo.md",
        b"jazzy.css class=\"dashanchor\"",
    ));

    let mut outside_bound = vec![b' '; 64 * 1024];
    outside_bound.extend_from_slice(b"jazzy.css class=\"dashanchor\"");
    assert!(!head_has_jazzy_generated_provenance(
        "docs/Foo.html",
        &outside_bound[..64 * 1024],
    ));
}

#[test]
fn decorator_prefix_is_language_aware() {
    // `@` is a decorator in these languages...
    assert_eq!(decorator_prefix("python"), Some("@"));
    assert_eq!(decorator_prefix("typescript"), Some("@"));
    assert_eq!(decorator_prefix("java"), Some("@"));
    assert_eq!(decorator_prefix("rust"), Some("#["));
    // ...but in Ruby a leading `@` is an INSTANCE VARIABLE, not a decorator, and
    // Go/C have no such syntax — these must report none, or `@token = …` would be
    // misread as a decorator and falsely split equal families.
    assert_eq!(decorator_prefix("ruby"), None);
    assert_eq!(decorator_prefix("go"), None);
    assert_eq!(decorator_prefix("c"), None);
}

#[test]
fn decorator_difference_detects_arg_changes() {
    let a = vec![r#"@click.argument("arg")"#.to_string()];
    let b = vec![r#"@click.argument("arg", metavar="m")"#.to_string()];
    let (a_only, b_only) = decorator_difference(&a, &b).expect("differs");
    assert_eq!(a_only, vec![r#"@click.argument("arg")"#.to_string()]);
    assert_eq!(
        b_only,
        vec![r#"@click.argument("arg", metavar="m")"#.to_string()]
    );
    // Identical decorator sets do not differ (the legit equal-modulo-holes case).
    assert!(decorator_difference(&a, &a).is_none());
    // Extra decorator on one side only.
    let c = vec!["@a".to_string(), "@b".to_string()];
    let d = vec!["@a".to_string()];
    let (c_only, d_only) = decorator_difference(&c, &d).expect("differs");
    assert_eq!(c_only, vec!["@b".to_string()]);
    assert!(d_only.is_empty());
}

pub(super) fn fam(langs: usize, modules: usize, names: &[Option<&str>]) -> RefactorFamily {
    fam_kind(langs, modules, names, nose_il::UnitKind::Function)
}

pub(super) fn fam_kind(
    langs: usize,
    modules: usize,
    names: &[Option<&str>],
    kind: nose_il::UnitKind,
) -> RefactorFamily {
    let locations = names
        .iter()
        .enumerate()
        .map(|(i, n)| {
            Loc::new(LocInit {
                file: format!("m{i}/f.rs"),
                source_span: LineSpan::new(1, 10),
                lang: "rust".into(),
                kind,
                origin: Default::default(),
                name: n.map(|s| s.to_string()),
                sem: 50,
                span_tokens: 50,
            })
        })
        .collect();
    RefactorFamily {
        value: 1.0,
        members: names.len(),
        files: names.len(),
        modules,
        languages: langs,
        mean_score: 0.9,
        mean_lines: 10,
        dup_lines: 10,
        shared_lines: 0,
        params: 0,
        shared_weight: 0.0,
        locations,
        direct_edges: Vec::new(),
        accepted_coverage: Vec::new(),
        display_params: None,
        mean_sem: 50.0,
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
fn verify_battery_budget_is_node_row_bounded() {
    assert!(
        !verify_battery_over_budget(2_000, 192),
        "the documented 2k x 192-row boundary stays inside the verify budget"
    );
    assert!(
        verify_battery_over_budget(2_001, 192),
        "one node beyond the boundary fails closed as battery-bail"
    );
    assert!(
        !verify_battery_over_budget(6_000, 1),
        "large units are allowed when the battery is tiny"
    );
}

#[test]
fn shared_lines_params_come_from_first_successful_pair() {
    use std::io::Write;
    // The representative pair can be unreadable while a *later* pair reads fine
    // (e.g. a deleted/edited file among the family members). The parameter count
    // must then come from the first pair that actually reads — not be dropped
    // just because the readable pair wasn't iteration 0.
    let dir = std::env::temp_dir().join(format!("nose_slo_{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let write = |name: &str, body: &str| {
        let p = dir.join(name);
        std::fs::File::create(&p)
            .unwrap()
            .write_all(body.as_bytes())
            .unwrap();
        p.to_string_lossy().to_string()
    };
    let f0 = write("a.rs", "AAA\nshared1\nshared2\n");
    let f2 = write("c.rs", "BBB\nshared1\nshared2\n");
    let missing = dir.join("missing.rs").to_string_lossy().to_string();

    let mk = |file: String| {
        Loc::new(LocInit {
            file,
            source_span: LineSpan::new(1, 3),
            lang: "rust".into(),
            kind: nose_il::UnitKind::Function,
            origin: Default::default(),
            name: None,
            sem: 50,
            span_tokens: 50,
        })
    };
    // locs[1] (the first compared pair) is unreadable; locs[2] reads and differs
    // from the representative by one parameter line.
    let locs = vec![mk(f0), mk(missing), mk(f2)];
    let mut cache = FileLineCache::default();
    let s = shared_lines_of(&locs, &mut cache).expect("a later pair reads");

    assert!(
        s.rank_lines.contains(&"shared1".to_string()),
        "shared lines extracted: {:?}",
        s.rank_lines
    );
    assert_eq!(
        s.params, 1,
        "params must come from the first successful pair, not iteration 0"
    );
    assert_eq!(s.display, 2, "two lines survive across every readable copy");
    assert_eq!(
        s.display_params, 1,
        "the all-copies display has one leading parameter hole"
    );

    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn hint_shared_name_consolidates() {
    let f = fam(1, 3, &[Some("series"), Some("series"), Some("series")]);
    assert_eq!(family_hint(&f), "consolidate `series` — 3 copies");
}

#[test]
fn hint_cross_language_is_flagged() {
    let f = fam(2, 2, &[Some("parse"), Some("parse")]);
    assert!(family_hint(&f).ends_with("(cross-language)"));
}

#[test]
fn hint_mixed_names_falls_back_to_spread() {
    let f = fam(1, 3, &[Some("replace"), Some("replaceOrAppend"), None]);
    assert_eq!(
        family_hint(&f),
        "repeated across 3 directories — extract a shared abstraction"
    );
}

#[test]
fn hint_test_scope_flags_scaffolding_caveat() {
    let mut f = fam(1, 2, &[None, None]);
    f.scope = "test";
    let h = family_hint(&f);
    assert!(h.contains("extract a helper"), "{h}");
    assert!(h.ends_with("not per-scenario setup"), "{h}");
}

#[test]
fn hint_prod_scope_has_no_test_caveat() {
    let f = fam(1, 2, &[None, None]); // scope defaults to prod
    assert!(!family_hint(&f).contains("test scaffolding"));
}

#[test]
fn hint_high_param_caution_wins_over_test_caveat() {
    let mut f = fam(1, 2, &[None, None]);
    f.scope = "test";
    f.params = 8; // >= HIGH_PARAM_SPOTS
    let h = family_hint(&f);
    assert!(h.contains("high-parameter"), "{h}");
    assert!(
        !h.contains("test scaffolding"),
        "high-param branch wins: {h}"
    );
}

#[test]
fn hint_local_duplication() {
    let f = fam(1, 1, &[None, None]);
    assert_eq!(family_hint(&f), "local duplication — extract a helper");
}

#[test]
fn hint_class_family_suggests_base_class() {
    let f = fam_kind(1, 3, &[None, None, None], nose_il::UnitKind::Class);
    assert_eq!(
        family_hint(&f),
        "repeated across 3 directories — extract a shared base class / mixin"
    );
}

#[test]
fn hint_origin_protocol_contract_avoids_base_class() {
    use nose_il::{
        RegionKind, SourceGranularity, UnitBodyKind, UnitDomain, UnitDomains, UnitSubkind,
    };
    let mut f = fam_kind(
        1,
        2,
        &[Some("TraceReadable"), Some("TraceWritable")],
        nose_il::UnitKind::Class,
    );
    for loc in &mut f.locations {
        loc.lang = "swift".into();
        loc.origin = nose_il::UnitOrigin::new(
            UnitDomains::of(UnitDomain::TypeContract),
            UnitSubkind::InterfaceTraitProtocol,
            UnitBodyKind::DeclarationOnly,
            SourceGranularity::WholeUnit,
            RegionKind::Code,
        );
    }
    assert_eq!(
        family_hint(&f),
        "duplicated across 2 directories — consolidate one shared interface/protocol contract"
    );
    assert!(hint_reasons(&f)
        .iter()
        .any(|reason| reason == "no implementation body was found"));
}

#[test]
fn hint_origin_behavior_class_keeps_base_class() {
    use nose_il::{
        RegionKind, SourceGranularity, UnitBodyKind, UnitDomain, UnitDomains, UnitSubkind,
    };
    let mut f = fam_kind(1, 2, &[None, None], nose_il::UnitKind::Class);
    for loc in &mut f.locations {
        loc.origin = nose_il::UnitOrigin::new(
            UnitDomains::of(UnitDomain::ImplementationType),
            UnitSubkind::Class,
            UnitBodyKind::Implementation,
            SourceGranularity::WholeUnit,
            RegionKind::Code,
        );
    }
    assert_eq!(
        family_hint(&f),
        "duplicated across 2 directories — extract a shared base class / mixin"
    );
}

#[test]
fn hint_origin_data_record_is_type_contract_not_base_class() {
    // A data record (TypeContract + Data, no implementation facet) must render as a
    // type/API contract, never "extract a shared base class / mixin" (#453).
    use nose_il::{
        RegionKind, SourceGranularity, UnitBodyKind, UnitDomain, UnitDomains, UnitSubkind,
    };
    let mut f = fam_kind(
        1,
        2,
        &[Some("Point"), Some("Coord")],
        nose_il::UnitKind::Class,
    );
    for loc in &mut f.locations {
        loc.lang = "java".into();
        loc.origin = nose_il::UnitOrigin::new(
            UnitDomains::of(UnitDomain::TypeContract).with(UnitDomain::Data),
            UnitSubkind::StructRecord,
            UnitBodyKind::DeclarativeDenotation,
            SourceGranularity::WholeUnit,
            RegionKind::Code,
        );
    }
    assert_eq!(
        family_hint(&f),
        "duplicated across 2 directories — consolidate one shared type/API contract"
    );
}

#[test]
fn hint_origin_style_is_declarative() {
    use nose_il::{
        RegionKind, SourceGranularity, UnitBodyKind, UnitDomain, UnitDomains, UnitSubkind,
    };
    let mut f = fam_kind(1, 1, &[None, None], nose_il::UnitKind::Block);
    for loc in &mut f.locations {
        loc.lang = "css".into();
        loc.origin = nose_il::UnitOrigin::new(
            UnitDomains::of(UnitDomain::Style),
            UnitSubkind::CssRule,
            UnitBodyKind::DeclarativeDenotation,
            SourceGranularity::Rule,
            RegionKind::Style,
        );
    }
    assert_eq!(
        family_hint(&f),
        "local duplication — merge selectors or move the declarations to a shared class/token if these elements should be coupled"
    );
}

#[test]
fn hint_block_family_suggests_method() {
    let f = fam_kind(1, 1, &[None, None], nose_il::UnitKind::Block);
    assert_eq!(
        family_hint(&f),
        "local duplication — extract a method from the repeated block"
    );
}
