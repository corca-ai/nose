use super::support::{fam, loc};

#[test]
fn extractability_ranks_tight_over_bloated() {
    // A: a big block whose copies share little (a dispatch skeleton over divergent
    // bodies) — high raw `value`, few shared lines, many params. B: a small tight
    // pair. Extractability must rank B above A even though A's `value` is larger.
    let bloated = fam(
        5000.0,
        200,
        9,
        22,
        vec![loc("a.rs", 1, 200, "rs"), loc("b.rs", 1, 200, "rs")],
    );
    let tight = fam(
        200.0,
        22,
        18,
        1,
        vec![loc("c.rs", 1, 22, "rs"), loc("d.rs", 1, 22, "rs")],
    );
    assert!(
        bloated.value > tight.value,
        "old ranking favors the bloated block"
    );
    assert!(
        tight.extractability() > bloated.extractability(),
        "extractability favors the cleanly-extractable pair"
    );
}

#[test]
fn hazard_inverts_extractability_on_text_similarity() {
    // The defining contract: same size and copy count, differing only in how much
    // text the copies share. `tight` is near-identical; `divergent` is the same
    // behavior with little shared text (an invisible sibling). Hazard must rank the
    // divergent one higher (it's the dangerous one), and extractability the tight one
    // — the two axes are *opposed* on the text-similarity dimension.
    let tight = fam(
        0.0,
        30,
        27,
        0,
        vec![loc("a.rs", 1, 30, "rs"), loc("b.rs", 1, 30, "rs")],
    );
    let divergent = fam(
        0.0,
        30,
        3,
        0,
        vec![loc("c.rs", 1, 30, "rs"), loc("d.rs", 1, 30, "rs")],
    );
    assert!(
        divergent.hazard() > tight.hazard(),
        "hazard ranks the syntactically-divergent (invisible) family higher"
    );
    assert!(
        tight.extractability() > divergent.extractability(),
        "extractability ranks the tight family higher — the axes are opposed"
    );
}

#[test]
fn hazard_surfaces_cross_language() {
    // Cross-language families have no shared source lines (shared_weight 0), so
    // invisibility maxes out — the sibling is truly invisible. Hazard surfaces them,
    // where extractability can barely rank them.
    let xlang = fam(
        0.0,
        30,
        0,
        0,
        vec![loc("a.py", 1, 30, "py"), loc("b.ts", 1, 30, "ts")],
    );
    let tight_same = fam(
        0.0,
        30,
        27,
        0,
        vec![loc("a.rs", 1, 30, "rs"), loc("b.rs", 1, 30, "rs")],
    );
    assert!(
        xlang.hazard() > tight_same.hazard(),
        "an invisible cross-language sibling outranks a tight same-language pair"
    );
}

#[test]
fn hazard_demotes_test_scope() {
    let prod = fam(
        0.0,
        30,
        3,
        0,
        vec![loc("a.rs", 1, 30, "rs"), loc("b.rs", 1, 30, "rs")],
    );
    let mut test = fam(
        0.0,
        30,
        3,
        0,
        vec![loc("a.rs", 1, 30, "rs"), loc("b.rs", 1, 30, "rs")],
    );
    test.scope = "test";
    assert!(
        prod.hazard() > test.hazard(),
        "a divergence in prod outranks the same divergence in tests"
    );
}

#[test]
fn extractability_falls_back_without_shared() {
    // Cross-language families have no shared *source* lines (shared_lines = 0); the
    // fallback keeps them ranked on structural similarity × volume, not at zero.
    let xlang = fam(
        0.0,
        40,
        0,
        0,
        vec![
            loc("a.py", 1, 40, "python"),
            loc("a.ts", 1, 40, "typescript"),
        ],
    );
    assert!(xlang.extractability() > 0.0);
}

#[test]
fn no_source_invariants_cannot_rank_as_an_extractable_helper() {
    let mut family = fam(
        80.0,
        8,
        0,
        1,
        vec![loc("a.rs", 1, 8, "rust"), loc("b.rs", 1, 8, "rust")],
    );
    family.shared_weight = 6.0;
    assert_eq!(family.extractability(), 0.0);
}
