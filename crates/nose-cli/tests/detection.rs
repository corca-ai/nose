//! Detection-pipeline property tests: determinism (a release-critical invariant we
//! must not regress when optimizing) and end-to-end Rust clone grouping.

use nose_detect::{detect, rank_families, DetectOptions, StructuralDetector};
use nose_il::{Corpus, FileId, Interner, Lang};

fn build(files: &[(&str, &str, Lang)]) -> Corpus {
    let interner = Interner::new();
    let ils = files
        .iter()
        .enumerate()
        .map(|(i, (p, s, l))| {
            nose_frontend::lower_source(FileId(i as u32), p, s.as_bytes(), *l, &interner).unwrap()
        })
        .collect();
    Corpus::new(interner, ils)
}

/// Sorted (file,start,file,start) keys of accepted pairs — a stable fingerprint of
/// a detection run's output.
fn pair_keys(report: &nose_detect::Report) -> Vec<(String, u32, String, u32)> {
    let mut v: Vec<_> = report
        .duplicates
        .iter()
        .map(|d| {
            let (l, r) = (&d.left, &d.right);
            (l.file.clone(), l.start_line, r.file.clone(), r.start_line)
        })
        .collect();
    v.sort();
    v
}

#[test]
fn detection_is_deterministic() {
    let f = "def sum_list(items):\n    total = 0\n    for x in items:\n        if x > 0:\n            total = total + x\n    return total\n";
    let g = "function total(xs){ let acc = 0; for (const v of xs){ if (v > 0){ acc = acc + v; } } return acc; }";
    let files = &[("a.py", f, Lang::Python), ("b.js", g, Lang::JavaScript)];
    let opts = DetectOptions {
        min_tokens: 12,
        ..Default::default()
    };

    let r1 = pair_keys(&detect(
        &build(files),
        &opts,
        &StructuralDetector::strict(opts.jaccard_weight),
    ));
    let r2 = pair_keys(&detect(
        &build(files),
        &opts,
        &StructuralDetector::strict(opts.jaccard_weight),
    ));
    assert_eq!(r1, r2, "two runs must produce byte-identical output");
}

#[test]
fn rust_clones_group_decoy_excluded() {
    // Two structurally-identical Rust functions (renamed) + an unrelated decoy.
    let a = "fn run(items: &[i32]) -> i32 {\n    let mut total = 0;\n    for x in items {\n        if *x > 0 {\n            total += x;\n        }\n    }\n    total\n}\n";
    let b = "fn compute(values: &[i32]) -> i32 {\n    let mut acc = 0;\n    for v in values {\n        if *v > 0 {\n            acc += v;\n        }\n    }\n    acc\n}\n";
    let decoy = "fn greet(name: &str) -> String {\n    let mut s = String::new();\n    s.push_str(\"hi \");\n    s.push_str(name);\n    s\n}\n";
    let corpus = build(&[
        ("a.rs", a, Lang::Rust),
        ("b.rs", b, Lang::Rust),
        ("c.rs", decoy, Lang::Rust),
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

    assert!(
        report.duplicates.iter().any(|d| {
            (d.left.file == "a.rs" && d.right.file == "b.rs")
                || (d.left.file == "b.rs" && d.right.file == "a.rs")
        }),
        "the two Rust clones must be detected as a pair"
    );
    assert!(
        report
            .duplicates
            .iter()
            .all(|d| d.left.file != "c.rs" && d.right.file != "c.rs"),
        "the decoy must not match"
    );
}

#[test]
fn sub_dag_family_annotates_each_site_with_shared_computation_lines() {
    // Two functions that share a heavy computation but differ elsewhere — a partial / sub-DAG
    // clone caught by the anchor (near) channel. Each site must report ITS OWN source range for
    // the shared computation, so the report can point at where the shared logic lives in each copy.
    let a = "function reportA(items) {\n  const totals = items.map(i => i.price * i.qty).reduce((s, x) => s + x, 0);\n  const tax = totals * 0.1;\n  const grand = totals + tax;\n  logA(grand);\n  return grand;\n}\n";
    let b = "function reportB(items) {\n  warmup();\n  warmup2();\n  const totals = items.map(i => i.price * i.qty).reduce((s, x) => s + x, 0);\n  const tax = totals * 0.1;\n  const grand = totals + tax;\n  saveB(grand);\n  notifyB(grand);\n}\n";
    let corpus = build(&[("a.ts", a, Lang::TypeScript), ("b.ts", b, Lang::TypeScript)]);
    let opts = DetectOptions {
        threshold: 0.70,
        min_tokens: 1,
        ..Default::default()
    };
    let report = detect(
        &corpus,
        &opts,
        &StructuralDetector::candidates(opts.jaccard_weight),
    );
    let families = rank_families(&report);
    let fam = families
        .iter()
        .find(|f| f.locations.iter().any(|l| l.shared_subdag.is_some()))
        .expect("a sub-DAG family annotated with shared-computation lines must exist");
    let site = |file: &str| {
        fam.locations
            .iter()
            .find(|l| l.file == file)
            .and_then(|l| l.shared_subdag)
    };
    let sa = site("a.ts").expect("a.ts site is annotated");
    let sb = site("b.ts").expect("b.ts site is annotated");
    assert_eq!(
        sb.0,
        sa.0 + 2,
        "each site reports the shared computation at its OWN location (b is two lines lower)",
    );
}

#[test]
fn cross_language_family_groups_proven_foreach_languages() {
    // The same guarded-accumulator loop, written idiomatically in languages with proven foreach
    // semantics, must land in ONE cross-language family. Ruby `.each` is present as a hard
    // negative: it stays closed until a pack supplies receiver/protocol proof for `items`.
    let py = "def f(items):\n    total = 0\n    for x in items:\n        if x > 0:\n            total = total + x\n    return total\n";
    let ts = "function g(xs: number[]): number {\n    let acc = 0;\n    for (const v of xs) {\n        if (v > 0) {\n            acc = acc + v;\n        }\n    }\n    return acc;\n}\n";
    let go = "package m\nfunc H(items []int) int {\n\tsum := 0\n\tfor _, e := range items {\n\t\tif e > 0 {\n\t\t\tsum = sum + e\n\t\t}\n\t}\n\treturn sum\n}\n";
    let rs = "fn k(items: &[i32]) -> i32 {\n    let mut total = 0;\n    for x in items {\n        if *x > 0 {\n            total = total + x;\n        }\n    }\n    total\n}\n";
    let java = "class C {\n    int m(int[] items) {\n        int total = 0;\n        for (int x : items) {\n            if (x > 0) {\n                total = total + x;\n            }\n        }\n        return total;\n    }\n}\n";
    let ruby = "def f(items)\n  total = 0\n  items.each do |x|\n    if x > 0\n      total = total + x\n    end\n  end\n  total\nend\n";
    let decoy = "def greet(name):\n    msg = 'hi ' + name\n    print(msg)\n    return msg\n";
    let corpus = build(&[
        ("acc.py", py, Lang::Python),
        ("acc.ts", ts, Lang::TypeScript),
        ("acc.go", go, Lang::Go),
        ("acc.rs", rs, Lang::Rust),
        ("acc.java", java, Lang::Java),
        ("acc.rb", ruby, Lang::Ruby),
        ("decoy.py", decoy, Lang::Python),
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
    let families = rank_families(&report);
    let xlang = families
        .iter()
        .find(|f| f.languages >= 2)
        .expect("a cross-language family must exist");
    assert!(
        xlang.languages == 5 && xlang.members == 5,
        "py/ts/go/rust/java accumulators form one 5-language, 5-site family (got {} langs, {} sites)",
        xlang.languages,
        xlang.members
    );
    assert!(
        xlang
            .locations
            .iter()
            .all(|l| l.file != "decoy.py" && l.file != "acc.rb"),
        "ruby each and the decoy must not join the proven foreach family"
    );
}

#[test]
fn contiguous_distinguishes_different_data_tables() {
    // The contiguous copy-paste channel is VALUE-sensitive: two arrays of *different*
    // string constants must not cluster as a clone. They did when literal values were
    // folded to the abstract `Str` class — every data table (HTML-entity maps, locale
    // tables) looked like one long identical token run, exploding into mega-families.
    // Identical tables still match (genuine copy-paste).
    let tbl = |name: &str, words: &[&str]| -> String {
        let body: Vec<String> = words.iter().map(|w| format!("  \"{w}\",")).collect();
        format!("const {name} = [\n{}\n];\n", body.join("\n"))
    };
    let a = tbl(
        "A",
        &[
            "alpha", "beta", "gamma", "delta", "epsilon", "zeta", "eta", "theta", "iota", "kappa",
        ],
    );
    let b = tbl(
        "B",
        &[
            "one", "two", "three", "four", "five", "six", "seven", "eight", "nine", "ten",
        ],
    );
    let opts = DetectOptions {
        min_tokens: 4,
        min_lines: 1,
        contiguous_min_tokens: 4,
        contiguous_min_lines: 1,
        contiguous: true,
        ..Default::default()
    };
    let det = StructuralDetector::candidates(opts.jaccard_weight);
    let cross_file_clone = |files: &[(&str, &str, Lang)]| -> bool {
        let rep = detect(&build(files), &opts, &det);
        rep.groups.iter().any(|g| {
            g.members
                .iter()
                .map(|m| m.file.as_str())
                .collect::<std::collections::HashSet<_>>()
                .len()
                >= 2
        })
    };
    assert!(
        !cross_file_clone(&[
            ("a.js", &a, Lang::JavaScript),
            ("b.js", &b, Lang::JavaScript)
        ]),
        "different string data tables must not form a contiguous clone"
    );
    assert!(
        cross_file_clone(&[
            ("a.js", &a, Lang::JavaScript),
            ("c.js", &a, Lang::JavaScript)
        ]),
        "identical string data tables ARE a copy-paste clone"
    );
}
