//! Robustness: nose runs on arbitrary real-world trees, so degenerate inputs —
//! empty files, comment-only files, and syntactically broken code — must lower
//! without panicking and must not crash the end-to-end detection pipeline.

use nose_detect::{detect, rank_families, DetectOptions, StructuralDetector};
use nose_il::{Corpus, FileId, Interner, Lang};

const LANGS: &[(Lang, &str)] = &[
    (Lang::Python, "py"),
    (Lang::JavaScript, "js"),
    (Lang::TypeScript, "ts"),
    (Lang::Go, "go"),
    (Lang::Rust, "rs"),
    (Lang::Java, "java"),
    (Lang::C, "c"),
    (Lang::Ruby, "rb"),
    (Lang::Vue, "vue"),
    (Lang::Svelte, "svelte"),
    (Lang::Html, "html"),
];

/// Lowering any of these snippets in any language must return `Ok` (never panic).
const DEGENERATE: &[&str] = &[
    "",                                  // empty
    "\n\n   \n\t\n",                     // whitespace only
    "// a comment\n# another\n/* c */",  // comments only (mixed styles)
    "function (((( ] return >>> 1 2 3",  // garbage / unbalanced
    "\u{0}\u{1}\u{2}not valid anything", // control bytes
    " \u{4e2d}\u{6587} = \u{1f600}",     // non-ASCII identifiers/emoji
];

#[test]
fn degenerate_inputs_never_panic() {
    let interner = Interner::new();
    for (lang, ext) in LANGS {
        for (i, src) in DEGENERATE.iter().enumerate() {
            let path = format!("deg_{i}.{ext}");
            // The contract is "no panic, returns a result we can keep". tree-sitter
            // produces ERROR nodes for garbage; lowering must tolerate them.
            let r = nose_frontend::lower_source(
                FileId(i as u32),
                &path,
                src.as_bytes(),
                *lang,
                &interner,
            );
            assert!(r.is_ok(), "{lang:?} must lower {path:?} without error");
        }
    }
}

#[test]
fn pipeline_handles_a_corpus_of_junk() {
    // A whole corpus of degenerate files must run detect + rank_families cleanly
    // and (correctly) surface nothing — no units large enough to be a clone.
    let interner = Interner::new();
    let ils = DEGENERATE
        .iter()
        .enumerate()
        .map(|(i, src)| {
            nose_frontend::lower_source(
                FileId(i as u32),
                &format!("j{i}.py"),
                src.as_bytes(),
                Lang::Python,
                &interner,
            )
            .unwrap()
        })
        .collect();
    let corpus = Corpus::new(interner, ils);
    let opts = DetectOptions::default();
    let report = detect(
        &corpus,
        &opts,
        &StructuralDetector::candidates(opts.jaccard_weight),
    );
    let families = rank_families(&report);
    assert!(
        families.is_empty(),
        "degenerate files should yield no refactoring families, got {}",
        families.len()
    );
}

#[test]
fn single_tiny_file_is_not_a_self_clone() {
    // One real function on its own must not match itself or sub-blocks.
    let interner = Interner::new();
    let src = "def f(x):\n    if x > 0:\n        return x + 1\n    return 0\n";
    let il =
        nose_frontend::lower_source(FileId(0), "a.py", src.as_bytes(), Lang::Python, &interner)
            .unwrap();
    let corpus = Corpus::new(interner, vec![il]);
    let opts = DetectOptions::default();
    let report = detect(
        &corpus,
        &opts,
        &StructuralDetector::candidates(opts.jaccard_weight),
    );
    assert!(
        report.duplicates.is_empty(),
        "a lone function must not be reported as a clone of itself"
    );
}
