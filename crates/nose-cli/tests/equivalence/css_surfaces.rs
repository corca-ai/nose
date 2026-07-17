use super::*;

// ----- CSS (declarative) exact-channel semantics -----
//
// CSS units are NOT lowered through the imperative value graph; their exact fingerprint
// is the canonical multiset of their declarations (see `nose-normalize::css`). These
// lock in the soundness-relevant behavior: a duplicated declaration block is a clone
// regardless of selector or declaration order, but a cascade-affecting or value
// difference must NOT merge.

/// The declarative value fingerprint of the first `CssRule` unit in `src`.
pub(super) fn css_fp(interner: &Interner, src: &str) -> Vec<u64> {
    let il = nose_frontend::lower_source(FileId(0), "t.css", src.as_bytes(), Lang::Css, interner)
        .unwrap();
    let n = normalize(&il, interner, &NormalizeOptions::default());
    let root = n
        .units
        .iter()
        .find(|u| n.node(u.root).kind == nose_il::NodeKind::CssRule)
        .map(|u| u.root)
        .expect("expected a CssRule unit");
    nose_normalize::value_fingerprint(&n, root, interner)
}

#[test]
fn css_declaration_order_independent_and_selector_independent() {
    // A duplicated declaration block is the canonical CSS clone — same declarations,
    // different selector AND different source order, must converge.
    let i = Interner::new();
    let a = ".btn { display: flex; align-items: center; gap: 8px; padding: 12px; }";
    let b = ".cta { padding: 12px; gap: 8px; align-items: center; display: flex; }";
    assert_eq!(
        css_fp(&i, a),
        css_fp(&i, b),
        "same declarations across selectors/orders must converge",
    );
}

#[test]
fn css_differing_value_does_not_converge() {
    // One value differs (12px vs 16px) → different computed style → must NOT merge.
    let i = Interner::new();
    let a = ".btn { display: flex; align-items: center; gap: 8px; padding: 12px; }";
    let c = ".btn { display: flex; align-items: center; gap: 8px; padding: 16px; }";
    assert_ne!(
        css_fp(&i, a),
        css_fp(&i, c),
        "a differing value must not merge"
    );
}

#[test]
fn css_cascade_last_wins_is_order_sensitive_for_repeated_properties() {
    // Declaration order is irrelevant EXCEPT for a repeated property, where the cascade
    // keeps the last — so these two compute different colors and must NOT merge.
    let i = Interner::new();
    let red_then_blue = ".x { color: red; color: blue; }";
    let blue_then_red = ".x { color: blue; color: red; }";
    assert_ne!(
        css_fp(&i, red_then_blue),
        css_fp(&i, blue_then_red),
        "repeated-property cascade (last-wins) must stay order-sensitive",
    );
}

#[test]
fn css_value_order_within_a_declaration_is_significant() {
    // Across declarations order is free, but WITHIN a declaration token order matters:
    // `margin: 1px 2px` (top/bottom 1px, left/right 2px) ≠ `margin: 2px 1px`.
    let i = Interner::new();
    let a = ".a { margin: 1px 2px; color: red; display: flex; gap: 4px; }";
    let b = ".b { margin: 2px 1px; color: red; display: flex; gap: 4px; }";
    assert_ne!(
        css_fp(&i, a),
        css_fp(&i, b),
        "value-token order within a declaration must be significant",
    );
}

#[test]
fn css_at_rule_context_blocks_merge_with_unconditional_rule() {
    // A `@media`-scoped rule and an identical unconditional rule compute different
    // styles (one is conditional) — the at-rule context must keep them apart.
    let i = Interner::new();
    let scoped =
        "@media (max-width: 600px) { .btn { display: flex; align-items: center; gap: 8px; padding: 12px; } }";
    let plain = ".btn { display: flex; align-items: center; gap: 8px; padding: 12px; }";
    assert_ne!(
        css_fp(&i, scoped),
        css_fp(&i, plain),
        "@media-scoped rule must not merge with an unconditional one",
    );
}

#[test]
fn css_computed_equivalence_converges_color_shorthand_unit() {
    // The deep CSS equivalence: textually different but computed-identical rules merge.
    // #ffffff≡white, margin:0 0 0 0≡margin:0, #ff0000≡red, padding 4-val≡2-val collapse.
    let i = Interner::new();
    let a = ".btn { color: #ffffff; margin: 0 0 0 0; background: #ff0000; padding: 10px 20px 10px 20px; }";
    let b = ".cta { color: white; margin: 0; background: red; padding: 10px 20px; }";
    assert_eq!(
        css_fp(&i, a),
        css_fp(&i, b),
        "computed-equivalent rules (color/shorthand/unit) must converge",
    );
}

#[test]
fn css_distinct_colors_do_not_converge() {
    // Soundness hard negative: different colors must never merge, however close.
    let i = Interner::new();
    let red = ".a { color: #ff0000; display: block; padding: 1px; }";
    let blue = ".b { color: #0000ff; display: block; padding: 1px; }";
    assert_ne!(
        css_fp(&i, red),
        css_fp(&i, blue),
        "#f00 vs #00f must stay distinct"
    );
}

#[test]
fn css_custom_property_token_spelling_stays_distinct() {
    let i = Interner::new();
    for (left, right) in [
        (".a { --x: white; }", ".b { --x: #fff; }"),
        (".a { --x: 0px; }", ".b { --x: 0; }"),
        (".a { --x: 1PX; }", ".b { --x: 1px; }"),
    ] {
        assert_ne!(
            css_fp(&i, left),
            css_fp(&i, right),
            "custom-property token spelling is behavior-bearing",
        );
    }
}

#[test]
fn css_shorthand_longhand_cascade_is_order_sensitive() {
    // Soundness: a shorthand and one of its longhands in the same rule cascade by
    // ORDER — `margin: 0; margin-top: 5px` (top=5px) ≠ `margin-top: 5px; margin: 0`
    // (top=0). The order-independent multiset would false-merge these; the
    // cascade-ambiguity guard must keep them apart.
    let i = Interner::new();
    let a = ".a { margin: 0; margin-top: 5px; display: block; color: red; }";
    let b = ".b { margin-top: 5px; margin: 0; display: block; color: red; }";
    assert_ne!(
        css_fp(&i, a),
        css_fp(&i, b),
        "shorthand/longhand cascade must stay order-sensitive",
    );
}

#[test]
fn css_at_rule_full_condition_distinguishes_blocks() {
    // Soundness (regression: a bulma `@container` false merge). The WHOLE at-rule head
    // is the context — two blocks differing only in their condition (and selectors,
    // which the fingerprint excludes) compute different styles and must NOT merge.
    let i = Interner::new();
    let a = "@container foo (max-width: 768px) { .a > .grid { --c: 1; } .a2 > .grid { --c: 2; } }";
    let b = "@container foo (min-width: 769px) { .b > .grid { --c: 1; } .b2 > .grid { --c: 2; } }";
    assert_ne!(
        css_fp(&i, a),
        css_fp(&i, b),
        "at-rule blocks with different conditions must not merge",
    );
}

#[test]
fn css_independent_non_overlapping_properties_stay_order_free() {
    // The guard must NOT over-fire: non-overlapping properties (no shorthand/longhand
    // relation) remain order-independent.
    let i = Interner::new();
    let a = ".a { color: red; display: block; padding: 4px; gap: 2px; }";
    let b = ".b { gap: 2px; padding: 4px; display: block; color: red; }";
    assert_eq!(
        css_fp(&i, a),
        css_fp(&i, b),
        "independent properties must remain order-free",
    );
}

#[test]
fn css_fingerprint_is_domain_disjoint_from_imperative() {
    // Cross-domain false-merge guard: a CSS fingerprint must never equal an imperative
    // one, so the (language-blind) exact channel can never merge CSS with code.
    let i = Interner::new();
    let css = css_fp(
        &i,
        ".a { display: flex; align-items: center; gap: 8px; padding: 12px; }",
    );
    let py = value_fp(&i, "def f(x):\n    return x + 5\n", Lang::Python);
    assert_ne!(css, py, "CSS and imperative fingerprints must be disjoint");
}
