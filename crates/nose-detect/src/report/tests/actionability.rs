use super::support::{fam, loc, witnessed};

#[test]
fn shallow_extraction_is_demoted_off_default() {
    // An unproven match whose helper would be mostly parameters (params ≥ a third of
    // the shared lines) is decidable non-action: demoted to the `shallow` surface,
    // reason-coded, but never deleted. shared=9, params=4 → ratio 0.44 ≥ 0.33.
    let shallow = witnessed(
        fam(
            500.0,
            30,
            9,
            4,
            vec![loc("a.go", 1, 30, "go"), loc("b.go", 1, 30, "go")],
        ),
        "copy-paste-run",
    );
    assert_eq!(shallow.actionability_reason(), Some("shallow-extraction"));
    assert_eq!(shallow.recommended_surface(), "shallow");
}

#[test]
fn clean_low_param_match_stays_default() {
    // shared=18, params=1 → ratio 0.06 < 0.33: a clean extract, stays on default.
    let clean = witnessed(
        fam(
            500.0,
            30,
            18,
            1,
            vec![loc("a.go", 1, 30, "go"), loc("b.go", 1, 30, "go")],
        ),
        "copy-paste-run",
    );
    assert_eq!(clean.actionability_reason(), None);
    assert_eq!(clean.recommended_surface(), "default");
}

#[test]
fn proven_channels_are_never_shallow() {
    // Same boundary-shallow shape (shared=9, params=3), but proof strength is not an
    // actionability verdict. #844's source-backed dev audit found worthy parameterized
    // extractions at this boundary, so both proven channels fail open to default.
    for kind in ["exact-value-graph", "shared-sub-dag"] {
        let proven = witnessed(
            fam(
                500.0,
                30,
                9,
                3,
                vec![loc("a.go", 1, 30, "go"), loc("b.go", 1, 30, "go")],
            ),
            kind,
        );
        assert_eq!(proven.actionability_reason(), None, "{kind}");
        assert_eq!(proven.recommended_surface(), "default", "{kind}");
    }
}

#[test]
fn cross_language_no_shared_lines_is_never_shallow() {
    // shared_lines == 0 (cross-language / unreadable) → ratio undefined → not shallow.
    let cross = witnessed(
        fam(
            500.0,
            30,
            0,
            4,
            vec![
                loc("a.py", 1, 30, "python"),
                loc("b.ts", 1, 30, "typescript"),
            ],
        ),
        "structural-similarity",
    );
    assert_eq!(cross.actionability_reason(), None);
    assert_ne!(cross.recommended_surface(), "shallow");
}

#[test]
fn trivial_family_is_demoted_to_hidden() {
    // mean_lines ≤ 4, unproven → too small to extract: reason `trivial`, surface hidden.
    let tiny = witnessed(
        fam(
            500.0,
            3,
            3,
            0,
            vec![loc("a.go", 1, 3, "go"), loc("b.go", 1, 3, "go")],
        ),
        "copy-paste-run",
    );
    assert_eq!(tiny.actionability_reason(), Some("trivial"));
    assert_eq!(tiny.recommended_surface(), "hidden");
}

#[test]
fn proven_tiny_families_are_not_trivial() {
    // #844 froze a worthy four-line helper as the size-floor hard negative. Protect
    // both proven channels: proof of equal behavior does not make a small extraction
    // worthless, and size alone did not clear the measured non-action precision gate.
    for kind in ["exact-value-graph", "shared-sub-dag"] {
        let proven = witnessed(
            fam(
                500.0,
                4,
                4,
                0,
                vec![loc("a.go", 1, 4, "go"), loc("b.go", 1, 4, "go")],
            ),
            kind,
        );
        assert_eq!(proven.actionability_reason(), None, "{kind}");
        assert_eq!(proven.recommended_surface(), "default", "{kind}");
    }
}

#[test]
fn trivial_takes_precedence_over_shallow() {
    // Tiny AND high-param: size is the more fundamental reason.
    let f = witnessed(
        fam(
            500.0,
            4,
            3,
            2,
            vec![loc("a.go", 1, 4, "go"), loc("b.go", 1, 4, "go")],
        ),
        "copy-paste-run",
    );
    assert_eq!(f.actionability_reason(), Some("trivial"));
}

#[test]
fn extraction_shape_classifies_structurally() {
    use crate::Loc;
    // default: two same-language whole functions → extract-helper.
    let helper = fam(
        500.0,
        20,
        15,
        1,
        vec![loc("a.go", 1, 20, "go"), loc("b.go", 1, 20, "go")],
    );
    assert_eq!(helper.extraction_shape(), "extract-helper");
    // cross-language.
    let cross = fam(
        500.0,
        20,
        0,
        1,
        vec![
            loc("a.py", 1, 20, "python"),
            loc("b.ts", 1, 20, "typescript"),
        ],
    );
    assert_eq!(cross.extraction_shape(), "consolidate-cross-language");
    // all blocks → extract a method from the repeated block.
    let block = |file: &str| Loc {
        kind: nose_il::UnitKind::Block,
        ..loc(file, 1, 20, "go")
    };
    let blocks = fam(500.0, 20, 15, 1, vec![block("a.go"), block("b.go")]);
    assert_eq!(blocks.extraction_shape(), "extract-method-from-block");
}
