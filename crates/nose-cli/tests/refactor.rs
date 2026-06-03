//! End-to-end tests for the refactoring-candidate report: detection → family
//! ranking. Guards the candidate-mode pipeline (so later perf work can't regress
//! it) and the design-level value ordering.

use nose_detect::{detect, rank_families, DetectOptions, StructuralDetector};
use nose_il::{Corpus, FileId, Interner, Lang};

/// A non-trivial function (passes the size gate) that sums a list, parameterized
/// by identifier names so we can plant near-duplicates.
fn sum_fn(acc: &str, item: &str) -> String {
    format!(
        "def f(items):\n    {acc} = 0\n    for {item} in items:\n        if {item} > 0:\n            {acc} = {acc} + {item} * {item}\n    return {acc}\n"
    )
}

fn corpus_from(files: &[(&str, &str, Lang)]) -> Corpus {
    let interner = Interner::new();
    let ils = files
        .iter()
        .enumerate()
        .map(|(i, (path, src, lang))| {
            nose_frontend::lower_source(FileId(i as u32), path, src.as_bytes(), *lang, &interner)
                .unwrap()
        })
        .collect();
    Corpus::new(interner, ils)
}

#[test]
fn ranks_a_planted_cross_module_family_first() {
    // Three near-identical copies across three modules + an unrelated decoy.
    let a = sum_fn("total", "x");
    let b = sum_fn("acc", "v");
    let c = sum_fn("s", "n");
    let decoy = "def greet(name):\n    msg = 'hi ' + name\n    print(msg)\n    print(name)\n    return msg\n";
    let corpus = corpus_from(&[
        ("mod_a/x.py", &a, Lang::Python),
        ("mod_b/y.py", &b, Lang::Python),
        ("mod_c/z.py", &c, Lang::Python),
        ("mod_d/d.py", decoy, Lang::Python),
    ]);

    let opts = DetectOptions {
        threshold: 0.70,
        min_tokens: 12,
        ..Default::default()
    };
    let report = detect(
        &corpus,
        &opts,
        &StructuralDetector::candidates(opts.jaccard_weight),
    );
    let fams = rank_families(&report);

    assert!(!fams.is_empty(), "should find at least the planted family");
    let top = &fams[0];
    assert_eq!(top.members, 3, "the three sum copies form one family");
    assert_eq!(top.files, 3);
    assert_eq!(top.modules, 3, "family spans three modules (design-level)");
    // the decoy must not be pulled into the family
    assert!(
        top.locations.iter().all(|l| !l.file.contains("mod_d")),
        "unrelated decoy excluded from the family"
    );
}

#[test]
fn family_value_rewards_more_sites() {
    // Same per-site code; a family with more copies should rank higher.
    let mk = |n: usize| -> Corpus {
        let body = sum_fn("total", "x");
        let files: Vec<(String, String)> = (0..n)
            .map(|i| (format!("m{i}/f.py"), body.clone()))
            .collect();
        let refs: Vec<(&str, &str, Lang)> = files
            .iter()
            .map(|(p, s)| (p.as_str(), s.as_str(), Lang::Python))
            .collect();
        corpus_from(&refs)
    };
    let opts = DetectOptions {
        threshold: 0.70,
        min_tokens: 12,
        ..Default::default()
    };
    let val = |n| {
        let r = detect(
            &mk(n),
            &opts,
            &StructuralDetector::candidates(opts.jaccard_weight),
        );
        rank_families(&r).first().map(|f| f.value).unwrap_or(0.0)
    };
    assert!(val(6) > val(3), "a 6-site family outranks a 3-site one");
}

#[test]
fn large_family_stays_one_connected_family() {
    // 60 identical copies — above the LSH bucket all-pairs cap (48). The O(k)
    // connectivity skeleton must still union them into ONE family, not fragments.
    let body = sum_fn("total", "x");
    let files: Vec<(String, String)> = (0..60)
        .map(|i| (format!("m{i}/f.py"), body.clone()))
        .collect();
    let refs: Vec<(&str, &str, Lang)> = files
        .iter()
        .map(|(p, s)| (p.as_str(), s.as_str(), Lang::Python))
        .collect();
    let corpus = corpus_from(&refs);
    let opts = DetectOptions {
        threshold: 0.70,
        min_tokens: 12,
        ..Default::default()
    };
    let report = detect(
        &corpus,
        &opts,
        &StructuralDetector::candidates(opts.jaccard_weight),
    );
    let fams = rank_families(&report);
    assert_eq!(
        fams[0].members, 60,
        "all 60 copies stay in one family despite the bucket cap"
    );
}

#[test]
fn empty_corpus_yields_no_families() {
    let corpus = corpus_from(&[("a.py", "x = 1\n", Lang::Python)]);
    let opts = DetectOptions::default();
    let report = detect(
        &corpus,
        &opts,
        &StructuralDetector::candidates(opts.jaccard_weight),
    );
    assert!(rank_families(&report).is_empty());
}
