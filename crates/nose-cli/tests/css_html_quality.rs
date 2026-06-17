//! Declarative (CSS / HTML) clone-detection quality benchmark.
//!
//! The imperative languages have a measured precision/recall gold set
//! ([benchmark](../../../docs/benchmark.md), bench/type4). This is the analogue for the
//! DECLARATIVE channels: a labeled set of
//! - POSITIVE groups — snippets that are computed-equivalent (CSS: same computed style;
//!   HTML: same rendered DOM) and so MUST share a fingerprint, and
//! - HARD-NEGATIVE pairs — computed-DISTINCT snippets that MUST NOT share a fingerprint.
//!
//! It reports recall (% of positive groups fully converged) and soundness (% of hard
//! negatives kept distinct), printed with `--nocapture`. Soundness is a hard gate (must
//! be 100% — a single false merge is a moat violation); recall has a floor that rises as
//! the modeled equivalence set grows.

use nose_il::{FileId, Interner, Lang, NodeKind};
use nose_normalize::{normalize, value_fingerprint, NormalizeOptions};

fn css_fp(interner: &Interner, src: &str) -> Vec<u64> {
    let il = nose_frontend::lower_source(FileId(0), "t.css", src.as_bytes(), Lang::Css, interner)
        .unwrap();
    let n = normalize(&il, interner, &NormalizeOptions::default());
    let root = n
        .units
        .iter()
        .find(|u| n.node(u.root).kind == NodeKind::CssRule)
        .map(|u| u.root)
        .expect("a CssRule unit");
    value_fingerprint(&n, root, interner)
}

fn html_fp(interner: &Interner, src: &str) -> Vec<u64> {
    let ils = nose_frontend::lower_source_regions(
        FileId(0),
        "t.html",
        src.as_bytes(),
        Lang::Html,
        interner,
    );
    let markup = ils
        .iter()
        .find(|il| il.meta.lang == Lang::Html)
        .expect("markup region");
    let n = normalize(markup, interner, &NormalizeOptions::default());
    let root = n
        .children(n.root)
        .iter()
        .copied()
        .find(|&c| n.node(c).kind == NodeKind::HtmlElement)
        .expect("a top-level HtmlElement");
    value_fingerprint(&n, root, interner)
}

type Fp = fn(&Interner, &str) -> Vec<u64>;

/// (axis, fingerprint fn, snippets that must all converge)
const POSITIVES: &[(&str, Fp, &[&str])] = &[
    ("css/color-hex-name-rgb", css_fp, &[
        ".x { color: #fff; display: block; padding: 1px }",
        ".y { color: #ffffff; display: block; padding: 1px }",
        ".z { color: #FFF; display: block; padding: 1px }",
        ".w { color: white; display: block; padding: 1px }",
        ".v { color: rgb(255, 255, 255); display: block; padding: 1px }",
        ".u { color: rgb(255 255 255); display: block; padding: 1px }",
        ".t { color: #ffffffff; display: block; padding: 1px }",
    ]),
    ("css/color-extended-names", css_fp, &[
        ".a { color: darkgray; border-color: rebeccapurple }",
        ".b { color: #a9a9a9; border-color: #663399 }",
        ".c { color: #A9A9A9; border-color: #639 }",
    ]),
    ("css/hsl-spelling", css_fp, &[
        ".a { color: hsl(0, 100%, 50%); padding: 1px }",
        ".b { color: hsl(0 100% 50%); padding: 1px }",
        ".c { color: hsl(0,100%,50%); padding: 1px }",
    ]),
    ("css/url-quotes", css_fp, &[
        ".a { background: url(\"x.png\"); padding: 1px }",
        ".b { background: url('x.png'); padding: 1px }",
        ".c { background: url(x.png); padding: 1px }",
    ]),
    ("css/zero-units", css_fp, &[
        ".a { margin: 0; padding: 0px; top: 0em }",
        ".b { margin: 0px; padding: 0; top: 0rem }",
    ]),
    ("css/number-canon", css_fp, &[
        ".a { opacity: 0.50; width: 1.0px; left: +2px }",
        ".b { opacity: .5; width: 1px; left: 2px }",
    ]),
    ("css/box-shorthand-collapse", css_fp, &[
        ".a { margin: 0 0 0 0; padding: 1px 2px 1px 2px }",
        ".b { margin: 0; padding: 1px 2px }",
        ".c { margin: 0px 0 0em 0; padding: 1px 2px 1px 2px }",
    ]),
    ("css/order-and-selector-independence", css_fp, &[
        ".btn { display: flex; align-items: center; gap: 8px; padding: 12px }",
        ".cta { padding: 12px; gap: 8px; align-items: center; display: flex }",
        "button.primary { gap: 8px; display: flex; padding: 12px; align-items: center }",
    ]),
    ("css/media-query-canon", css_fp, &[
        "@media screen and (max-width: 600px) { .a { color: red; padding: 1px } }",
        "@media screen and (max-width:600px) { .b { color: red; padding: 1px } }",
        "@media (max-width: 600px) and screen { .c { color: red; padding: 1px } }",
        "@media ( max-width : 600px ) and screen { .d { color: red; padding: 1px } }",
    ]),
    ("css/media-query-value-canon", css_fp, &[
        "@media (min-width: 0px) { .a { color: red; padding: 1px } }",
        "@media (min-width: 0) { .b { color: red; padding: 1px } }",
    ]),
    ("html/dom-normalize", html_fp, &[
        r#"<div class="card x"><img src="a.png" alt="p"><button type="button" disabled>Go</button></div>"#,
        "<div class=\"x card\">\n <img alt=\"p\"  src=\"a.png\">\n <button disabled=\"\" type=\"button\">Go</button>\n</div>",
    ]),
    ("html/structure-with-inline-style", html_fp, &[
        r#"<section style="margin: 0 0 0 0; color: #ffffff"><p>hi</p></section>"#,
        r#"<section style="color: white; margin: 0"><p>hi</p></section>"#,
    ]),
    ("html/vue-directive-shorthand", html_fp, &[
        r#"<button :class="c" @click="f"><i class="ico"></i>Go</button>"#,
        r#"<button v-bind:class="c" v-on:click="f"><i class="ico"></i>Go</button>"#,
    ]),
];

/// (axis, fingerprint fn, two snippets that MUST stay distinct)
const NEGATIVES: &[(&str, Fp, &str, &str)] = &[
    (
        "css/distinct-colors",
        css_fp,
        ".a { color: #f00; padding: 1px }",
        ".b { color: #00f; padding: 1px }",
    ),
    (
        "css/distinct-values",
        css_fp,
        ".a { padding: 12px; display: block }",
        ".b { padding: 16px; display: block }",
    ),
    (
        "css/cascade-last-wins",
        css_fp,
        ".a { color: red; color: blue }",
        ".a { color: blue; color: red }",
    ),
    (
        "css/shorthand-longhand-cascade",
        css_fp,
        ".a { margin: 0; margin-top: 5px; color: red }",
        ".a { margin-top: 5px; margin: 0; color: red }",
    ),
    (
        "css/box-not-all-equal",
        css_fp,
        ".a { margin: 0 1px 0 1px }",
        ".a { margin: 1px }",
    ),
    (
        "css/value-order-in-decl",
        css_fp,
        ".a { margin: 1px 2px; color: red; display: flex }",
        ".b { margin: 2px 1px; color: red; display: flex }",
    ),
    (
        "css/at-rule-condition",
        css_fp,
        "@container foo (max-width: 768px) { .a > .g { --c: 1 } .a2 > .g { --c: 2 } }",
        "@container foo (min-width: 769px) { .b > .g { --c: 1 } .b2 > .g { --c: 2 } }",
    ),
    (
        "css/hsl-distinct",
        css_fp,
        ".a { color: hsl(0, 100%, 50%); padding: 1px }",
        ".b { color: hsl(120, 100%, 50%); padding: 1px }",
    ),
    (
        // @media (X) and @supports (X) share a condition string but mean different
        // things — must not merge (the at-rule type prefix keeps them apart).
        "css/media-vs-supports",
        css_fp,
        "@media (min-width: 600px) { .a { color: red; padding: 1px } }",
        "@supports (min-width: 600px) { .b { color: red; padding: 1px } }",
    ),
    (
        "css/media-distinct-condition",
        css_fp,
        "@media (max-width: 600px) { .a { color: red; padding: 1px } }",
        "@media (max-width: 900px) { .b { color: red; padding: 1px } }",
    ),
    (
        "html/distinct-text",
        html_fp,
        r#"<div class="c"><h3>Title</h3><p>body</p></div>"#,
        r#"<div class="c"><h3>Other</h3><p>body</p></div>"#,
    ),
    (
        "html/distinct-attr-value",
        html_fp,
        r#"<a href="/a" class="c"><span>x</span></a>"#,
        r#"<a href="/b" class="c"><span>x</span></a>"#,
    ),
    (
        "html/child-order",
        html_fp,
        r#"<ul class="m"><li>one</li><li>two</li></ul>"#,
        r#"<ul class="m"><li>two</li><li>one</li></ul>"#,
    ),
    (
        "html/pre-whitespace",
        html_fp,
        "<pre>fn f() {\n    a;\n    b;\n}</pre>",
        "<pre>fn f() {\n  a;\n  b;\n}</pre>",
    ),
];

#[test]
fn declarative_quality_benchmark() {
    let i = Interner::new();

    let mut pos_ok = 0;
    let mut pos_fail: Vec<&str> = Vec::new();
    for (axis, fp, snippets) in POSITIVES {
        let first = fp(&i, snippets[0]);
        if snippets.iter().all(|s| fp(&i, s) == first) {
            pos_ok += 1;
        } else {
            pos_fail.push(axis);
        }
    }

    let mut neg_ok = 0;
    let mut neg_fail: Vec<&str> = Vec::new();
    for (axis, fp, a, b) in NEGATIVES {
        if fp(&i, a) != fp(&i, b) {
            neg_ok += 1;
        } else {
            neg_fail.push(axis);
        }
    }

    // Cross-domain disjointness: no CSS positive may collide with any HTML positive or
    // with an imperative fingerprint (the language-blind exact channel must not cross).
    let css_sample = css_fp(
        &i,
        ".btn { display: flex; align-items: center; gap: 8px; padding: 12px }",
    );
    let html_sample = html_fp(
        &i,
        r#"<div class="card"><h3>T</h3><p>body text here</p></div>"#,
    );
    let py = {
        let il = nose_frontend::lower_source(
            FileId(0),
            "t.py",
            b"def f(x):\n    return x + 5\n",
            Lang::Python,
            &i,
        )
        .unwrap();
        let n = normalize(&il, &i, &NormalizeOptions::default());
        let f = n
            .units
            .iter()
            .find(|u| matches!(u.kind, nose_il::UnitKind::Function))
            .unwrap()
            .root;
        value_fingerprint(&n, f, &i)
    };
    let disjoint = css_sample != html_sample && css_sample != py && html_sample != py;

    let recall = pos_ok as f64 / POSITIVES.len() as f64;
    let soundness = neg_ok as f64 / NEGATIVES.len() as f64;
    eprintln!("\n=== declarative (CSS/HTML) quality benchmark ===");
    eprintln!(
        "  recall    : {pos_ok}/{} positive groups converged ({:.1}%)",
        POSITIVES.len(),
        recall * 100.0
    );
    if !pos_fail.is_empty() {
        eprintln!("    MISSED  : {pos_fail:?}");
    }
    eprintln!(
        "  soundness : {neg_ok}/{} hard negatives kept distinct ({:.1}%)",
        NEGATIVES.len(),
        soundness * 100.0
    );
    if !neg_fail.is_empty() {
        eprintln!("    FALSE-MERGED: {neg_fail:?}");
    }
    eprintln!("  cross-domain disjoint (css/html/imperative): {disjoint}");

    // Hard gates.
    assert!(
        neg_fail.is_empty(),
        "SOUNDNESS: hard negatives false-merged: {neg_fail:?}"
    );
    assert!(disjoint, "cross-domain fingerprints must be disjoint");
    assert!(
        recall >= 1.0,
        "recall {:.1}% below floor; missed: {pos_fail:?}",
        recall * 100.0,
    );
}
