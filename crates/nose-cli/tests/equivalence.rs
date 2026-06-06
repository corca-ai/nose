//! IL-equivalence tests — the heart of correctness. Semantically-equivalent
//! snippets must normalize to the same structural hash; genuinely different code
//! must not. Also covers provenance and an end-to-end detection smoke test.

use nose_detect::{detect, DetectOptions, StructuralDetector};
use nose_il::{Corpus, FileId, Interner, Lang, NodeId, UnitKind};
use nose_normalize::{normalize, subtree_hashes, NormalizeOptions};

/// Normalize `src` and return the structural hash of its first function/method
/// unit. A shared `interner` keeps field-name symbols comparable across calls.
fn unit_hash(interner: &Interner, src: &str, lang: Lang) -> u64 {
    let il = nose_frontend::lower_source(FileId(0), "t", src.as_bytes(), lang, interner).unwrap();
    let n = normalize(&il, interner, &NormalizeOptions::default());
    let hashes = subtree_hashes(&n, interner);
    let root = first_func(&n);
    hashes[root.0 as usize]
}

fn first_func(il: &nose_il::Il) -> NodeId {
    il.units
        .iter()
        .find(|u| matches!(u.kind, UnitKind::Function | UnitKind::Method))
        .map(|u| u.root)
        .expect("expected a function unit")
}

#[test]
fn loop_unification_cfor_equals_while() {
    let i = Interner::new();
    let cfor = "function f(xs){ let t=0; for(let k=0;k<xs.length;k++){ t+=xs[k]; } return t; }";
    let whilev =
        "function g(ys){ let s=0; let j=0; while(j<ys.length){ s+=ys[j]; j=j+1; } return s; }";
    assert_eq!(
        unit_hash(&i, cfor, Lang::JavaScript),
        unit_hash(&i, whilev, Lang::JavaScript),
        "C-style for and while summation should converge"
    );
}

#[test]
fn alpha_equivalence_rename() {
    let i = Interner::new();
    let a = "def f(items):\n    total = 0\n    for x in items:\n        total = total + x\n    return total\n";
    let b = "def g(seq):\n    acc = 0\n    for e in seq:\n        acc = acc + e\n    return acc\n";
    assert_eq!(
        unit_hash(&i, a, Lang::Python),
        unit_hash(&i, b, Lang::Python)
    );
}

#[test]
fn compound_assignment_desugars() {
    let i = Interner::new();
    let a = "def f(n):\n    x = 1\n    x += n\n    return x\n";
    let b = "def g(m):\n    y = 1\n    y = y + m\n    return y\n";
    assert_eq!(
        unit_hash(&i, a, Lang::Python),
        unit_hash(&i, b, Lang::Python)
    );
}

#[test]
fn reduction_normal_form_converges_across_accumulator() {
    // Two sum-of-squares loops differing in accumulator/element names must produce
    // the SAME value fingerprint — the loop-recurrence normal form (§AI) canonicalizes
    // `acc = acc + x*x` to `Reduce(Add, 0, Elem*Elem)` regardless of naming/grouping.
    let i = Interner::new();
    let a = "def f(xs):\n    total = 0\n    for x in xs:\n        total = total + x * x\n    return total\n";
    let b =
        "def g(ys):\n    acc = 0\n    for e in ys:\n        acc = acc + e * e\n    return acc\n";
    assert_eq!(
        value_fp(&i, a, Lang::Python),
        value_fp(&i, b, Lang::Python),
        "sum-of-squares loops should converge to one Reduce value"
    );
}

#[test]
fn indexed_while_converges_with_foreach() {
    // An indexed `while i < len(xs) { … xs[i] …; i += 1 }` and the equivalent
    // `for x in xs` must produce the SAME value fingerprint — induction-variable
    // recognition rewrites `xs[i]` → `Elem(xs)` and drops the index bookkeeping (§AI).
    let i = Interner::new();
    let foreach = "def f(xs):\n    t = 0\n    for x in xs:\n        t = t + x * x\n    return t\n";
    let indexed = "def g(xs):\n    t = 0\n    i = 0\n    while i < len(xs):\n        t = t + xs[i] * xs[i]\n        i = i + 1\n    return t\n";
    assert_eq!(
        value_fp(&i, foreach, Lang::Python),
        value_fp(&i, indexed, Lang::Python),
        "indexed while and for-each over the same iterable should converge"
    );
}

#[test]
fn loop_converges_with_reduce_and_comprehension() {
    // The HoF→Reduce unification (§AI): an explicit accumulator loop, `functools.reduce`,
    // and a `sum(generator)` over the same per-element computation must all produce the
    // SAME value fingerprint — they are the same fold.
    let i = Interner::new();
    let prod_loop = "def p(xs):\n    r = 1\n    for x in xs:\n        r = r * x\n    return r\n";
    let prod_reduce = "def p(xs):\n    return reduce(lambda a, b: a * b, xs, 1)\n";
    assert_eq!(
        value_fp(&i, prod_loop, Lang::Python),
        value_fp(&i, prod_reduce, Lang::Python),
        "product loop should converge with reduce(λa,b. a*b, xs, 1)"
    );
    let sumsq_loop =
        "def f(xs):\n    t = 0\n    for x in xs:\n        t = t + x * x\n    return t\n";
    let sumsq_gen = "def f(xs):\n    return sum(x * x for x in xs)\n";
    assert_eq!(
        value_fp(&i, sumsq_loop, Lang::Python),
        value_fp(&i, sumsq_gen, Lang::Python),
        "sum-of-squares loop should converge with sum(x*x for x in xs)"
    );
}

#[test]
fn filtered_reduction_converges_for_and_while() {
    // A guarded (filtered) reduction `if cond: acc += contrib` is recognized as
    // `Reduce(+, 0, cond ? contrib : 0)` (§AI), so a filtered for-each loop and the
    // equivalent indexed while converge.
    let i = Interner::new();
    let foreach = "def f(xs):\n    t = 0\n    for x in xs:\n        if x % 2 == 0:\n            t = t + x * x\n    return t\n";
    let indexed = "def g(xs):\n    t = 0\n    i = 0\n    while i < len(xs):\n        if xs[i] % 2 == 0:\n            t = t + xs[i] * xs[i]\n        i = i + 1\n    return t\n";
    assert_eq!(
        value_fp(&i, foreach, Lang::Python),
        value_fp(&i, indexed, Lang::Python),
        "filtered sum-of-even-squares should converge across loop shapes"
    );
}

#[test]
fn coupled_loop_recurrence_stays_compact_and_distinct() {
    let i = Interner::new();
    let checksum_like = r#"
void f(int *a, int n, int *out) {
  int s1 = 0;
  int s2 = 0;
  int i = 0;
  while (i < n) {
    s1 = s1 + a[i] + s2;
    s2 = s2 + a[i] + s1;
    s1 = s1 + a[i] + s2;
    s2 = s2 + a[i] + s1;
    s1 = s1 + a[i] + s2;
    s2 = s2 + a[i] + s1;
    s1 = s1 + a[i] + s2;
    s2 = s2 + a[i] + s1;
    i = i + 1;
  }
  out[0] = s1;
  out[1] = s2;
}
"#;
    let changed_recurrence = r#"
void f(int *a, int n, int *out) {
  int s1 = 0;
  int s2 = 0;
  int i = 0;
  while (i < n) {
    s1 = s1 + a[i] + s2;
    s2 = s2 + a[i] + s1;
    s1 = s1 + a[i] + s2;
    s2 = s2 + a[i] + s1;
    s1 = s1 + a[i] + s2;
    s2 = s2 + a[i] + s1;
    s1 = s1 + a[i] + s2;
    s2 = s2 - a[i] + s1;
    i = i + 1;
  }
  out[0] = s1;
  out[1] = s2;
}
"#;

    let fp = value_fp(&i, checksum_like, Lang::C);
    assert!(
        fp.len() < 200,
        "coupled loop recurrence should not expand into a huge value DAG: {} atoms",
        fp.len()
    );
    assert_ne!(
        fp,
        value_fp(&i, changed_recurrence, Lang::C),
        "compacted recurrence must keep behavior-defining update differences"
    );
}

#[test]
fn large_generated_ac_formula_stays_compact_and_distinct() {
    let i = Interner::new();
    let params: Vec<String> = (0..80).map(|n| format!("x{n}")).collect();
    let forward = params.join(" + ");
    let reverse = params.iter().rev().cloned().collect::<Vec<_>>().join(" + ");
    let changed = format!("{} + x0 * x0", params[1..].join(" + "));
    let src = |expr: &str| format!("def f({}):\n    return {expr}\n", params.join(", "));

    let fp = value_fp(&i, &src(&forward), Lang::Python);
    assert_eq!(
        fp,
        value_fp(&i, &src(&reverse), Lang::Python),
        "large generated AC formulas should keep canonical operand ordering"
    );
    assert_ne!(
        fp,
        value_fp(&i, &src(&changed), Lang::Python),
        "large formula compaction must keep changed terms distinct"
    );
    assert!(
        fp.len() < 20,
        "large formula should fingerprint as a compact atom set: {} atoms",
        fp.len()
    );
}

#[test]
fn large_generated_add_sub_formula_stays_compact_and_distinct() {
    let i = Interner::new();
    let params: Vec<String> = (0..80).map(|n| format!("x{n}")).collect();
    let alternating = params
        .iter()
        .enumerate()
        .map(|(idx, param)| {
            if idx == 0 {
                param.clone()
            } else if idx % 2 == 0 {
                format!("+ {param}")
            } else {
                format!("- {param}")
            }
        })
        .collect::<Vec<_>>()
        .join(" ");
    let positives = (0..80)
        .rev()
        .filter(|idx| idx % 2 == 0)
        .map(|idx| params[idx].clone())
        .collect::<Vec<_>>()
        .join(" + ");
    let negatives = (0..80)
        .rev()
        .filter(|idx| idx % 2 == 1)
        .map(|idx| format!("- {}", params[idx]))
        .collect::<Vec<_>>()
        .join(" ");
    let regrouped = format!("{positives} {negatives}");
    let changed = alternating.replacen("- x1", "+ x1", 1);
    let src = |expr: &str| format!("def f({}):\n    return {expr}\n", params.join(", "));

    let fp = value_fp(&i, &src(&alternating), Lang::Python);
    assert_eq!(
        fp,
        value_fp(&i, &src(&regrouped), Lang::Python),
        "large generated add/sub formulas should canonicalize signed operands"
    );
    assert_ne!(
        fp,
        value_fp(&i, &src(&changed), Lang::Python),
        "large add/sub formula compaction must keep sign changes distinct"
    );
    assert!(
        fp.len() < 20,
        "large add/sub formula should fingerprint as a compact atom set: {} atoms",
        fp.len()
    );
}

#[test]
fn filtered_comprehension_matches_filtered_loop() {
    // `sum(x for x in xs if x>0)` and the guarded loop `if x>0: t += x` produce the
    // same guarded Reduce (§AI). The loop additionally records the guard as a
    // branch-condition sink, so the comprehension's fingerprint is *contained* in the
    // loop's — every comprehension value appears in the loop, with high overlap.
    let i = Interner::new();
    let loopv =
        "def f(xs):\n    t = 0\n    for x in xs:\n        if x > 0:\n            t = t + x\n    return t\n";
    let comp = "def f(xs):\n    return sum(x for x in xs if x > 0)\n";
    let lf = value_fp(&i, loopv, Lang::Python);
    let cf = value_fp(&i, comp, Lang::Python);
    assert!(
        cf.iter().all(|v| lf.contains(v)),
        "filtered comprehension fingerprint should be contained in the loop's"
    );
    assert!(
        cf.len() as f64 / lf.len() as f64 >= 0.8,
        "overlap should be high: comp {} / loop {}",
        cf.len(),
        lf.len()
    );
}

#[test]
fn filtered_method_reduce_converges_with_guarded_loop() {
    // `filter(p).reduce(⊕, init)` is the same guarded accumulator loop as
    // `if p(x) { acc = acc ⊕ x }`. The value graph must attach the filter predicate
    // to the reduce contribution, while the accumulator seed stays behavior-defining.
    let i = Interner::new();
    let loop_js = "function f(xs){ let total = 0; for (const x of xs) { if (x > 0) { total += x; } } return total; }";
    let reduce_js =
        "function f(xs){ return xs.filter(x => x > 0).reduce((total, x) => total + x, 0); }";
    let reduce_rs = "fn f(xs: &[i64]) -> i64 { xs.iter().copied().filter(|x| *x > 0).fold(0, |total, x| total + x) }";
    let bad_init =
        "function f(xs){ return xs.filter(x => x > 0).reduce((total, x) => total + x, 1); }";
    let loop_fp = value_fp(&i, loop_js, Lang::JavaScript);
    assert_eq!(
        loop_fp,
        value_fp(&i, reduce_js, Lang::JavaScript),
        "JS filter().reduce(sum) should converge with the guarded loop"
    );
    assert_eq!(
        loop_fp,
        value_fp(&i, reduce_rs, Lang::Rust),
        "Rust filter().fold(sum) should converge through the same value graph"
    );
    assert_ne!(
        loop_fp,
        value_fp(&i, bad_init, Lang::JavaScript),
        "changing the reduce seed is a hard negative"
    );
}

#[test]
fn filtered_count_aggregates_converge_with_count_loop() {
    // `filter(p).length`, `filter(p).count()`, and `count { p }` are all
    // count-filter reductions: add 1 when the predicate holds, otherwise add 0.
    let i = Interner::new();
    let loop_js = "function f(xs){ let count = 0; for (const x of xs) { if (x > 0) { count += 1; } } return count; }";
    let len_js = "function f(xs){ return xs.filter(x => x > 0).length; }";
    let count_rs =
        "fn f(xs: &[i64]) -> i64 { xs.iter().copied().filter(|x| *x > 0).count() as i64 }";
    let count_rb = "def f(xs)\n  xs.count { |x| x > 0 }\nend\n";
    let bad_predicate = "function f(xs){ return xs.filter(x => x >= 0).length; }";
    let loop_fp = value_fp(&i, loop_js, Lang::JavaScript);
    assert_eq!(
        loop_fp,
        value_fp(&i, len_js, Lang::JavaScript),
        "JS filter().length should converge with a guarded count loop"
    );
    assert_eq!(
        loop_fp,
        value_fp(&i, count_rs, Lang::Rust),
        "Rust filter().count() should converge through the same count reduce"
    );
    assert_eq!(
        loop_fp,
        value_fp(&i, count_rb, Lang::Ruby),
        "Ruby count block should converge through the same count reduce"
    );
    assert_ne!(
        loop_fp,
        value_fp(&i, bad_predicate, Lang::JavaScript),
        "changing the count predicate is a hard negative"
    );
}

#[test]
fn java_stream_aggregates_converge_with_loops() {
    // Java stream pipelines should lower into the same shared iteration/reduction
    // shapes as enhanced-for loops: `Arrays.stream(xs)` is just the source collection,
    // and `anyMatch`/`allMatch` are predicate reductions.
    let i = Interner::new();
    let sum_loop = "class C { static int f(int[] xs) { int total = 0; for (int x : xs) { if (x > 0) { total += x; } } return total; } }";
    let sum_stream = "import java.util.Arrays; class C { static int f(int[] xs) { return Arrays.stream(xs).filter(x -> x > 0).reduce(0, (total, x) -> total + x); } }";
    let count_loop = "class C { static int f(int[] xs) { int count = 0; for (int x : xs) { if (x > 0) { count += 1; } } return count; } }";
    let count_stream = "import java.util.Arrays; class C { static int f(int[] xs) { return (int) Arrays.stream(xs).filter(x -> x > 0).count(); } }";
    let any_loop = "class C { static boolean f(int[] xs) { for (int x : xs) { if (x > 0) { return true; } } return false; } }";
    let any_stream = "import java.util.Arrays; class C { static boolean f(int[] xs) { return Arrays.stream(xs).anyMatch(x -> x > 0); } }";
    let all_loop = "class C { static boolean f(int[] xs) { for (int x : xs) { if (!(x >= 0)) { return false; } } return true; } }";
    let all_stream = "import java.util.Arrays; class C { static boolean f(int[] xs) { return Arrays.stream(xs).allMatch(x -> x >= 0); } }";
    let bad_seed =
        "import java.util.Arrays; class C { static int f(int[] xs) { return Arrays.stream(xs).filter(x -> x > 0).reduce(1, (total, x) -> total + x); } }";
    let sum_fp = value_fp(&i, sum_loop, Lang::Java);
    assert_eq!(sum_fp, value_fp(&i, sum_stream, Lang::Java));
    assert_eq!(
        value_fp(&i, count_loop, Lang::Java),
        value_fp(&i, count_stream, Lang::Java)
    );
    assert_eq!(
        value_fp(&i, any_loop, Lang::Java),
        value_fp(&i, any_stream, Lang::Java)
    );
    assert_eq!(
        value_fp(&i, all_loop, Lang::Java),
        value_fp(&i, all_stream, Lang::Java)
    );
    assert_ne!(sum_fp, value_fp(&i, bad_seed, Lang::Java));
}

#[test]
fn ruby_select_reduce_converges_with_guarded_loop() {
    // Ruby `select { p }.reduce(init) { |a, x| ... }` is the same filtered fold as
    // a guarded `each` accumulator loop. The changed seed remains a hard negative.
    let i = Interner::new();
    let loop_rb = "def f(xs)\n  total = 0\n  xs.each do |x|\n    if x > 0\n      total += x\n    end\n  end\n  total\nend\n";
    let reduce_rb =
        "def f(xs)\n  xs.select { |x| x > 0 }.reduce(0) { |total, x| total + x }\nend\n";
    let product_loop =
        "def f(xs)\n  product = 1\n  xs.each do |x|\n    if x > 0\n      product *= x\n    end\n  end\n  product\nend\n";
    let product_reduce =
        "def f(xs)\n  xs.select { |x| x > 0 }.reduce(1) { |product, x| product * x }\nend\n";
    let bad_seed = "def f(xs)\n  xs.select { |x| x > 0 }.reduce(1) { |total, x| total + x }\nend\n";
    let sum_fp = value_fp(&i, loop_rb, Lang::Ruby);
    assert_eq!(sum_fp, value_fp(&i, reduce_rb, Lang::Ruby));
    assert_eq!(
        value_fp(&i, product_loop, Lang::Ruby),
        value_fp(&i, product_reduce, Lang::Ruby)
    );
    assert_ne!(sum_fp, value_fp(&i, bad_seed, Lang::Ruby));
}

#[test]
fn ruby_any_all_predicates_converge_with_early_return_loops() {
    let i = Interner::new();
    let any_loop = "def f(xs)\n  xs.each do |x|\n    if x > 0\n      return true\n    end\n  end\n  false\nend\n";
    let any_call = "def f(xs)\n  xs.any? { |x| x > 0 }\nend\n";
    let all_loop = "def f(xs)\n  xs.each do |x|\n    if !(x >= 0)\n      return false\n    end\n  end\n  true\nend\n";
    let all_call = "def f(xs)\n  xs.all? { |x| x >= 0 }\nend\n";
    let bad_predicate = "def f(xs)\n  xs.any? { |x| x >= 0 }\nend\n";
    let any_fp = value_fp(&i, any_loop, Lang::Ruby);
    assert_eq!(any_fp, value_fp(&i, any_call, Lang::Ruby));
    assert_eq!(
        value_fp(&i, all_loop, Lang::Ruby),
        value_fp(&i, all_call, Lang::Ruby)
    );
    assert_ne!(any_fp, value_fp(&i, bad_predicate, Lang::Ruby));
}

#[test]
fn rust_any_all_predicates_converge_with_early_return_loops() {
    let i = Interner::new();
    let any_loop = "fn f(xs: &[i64]) -> bool { for &x in xs { if x > 0 { return true; } } false }";
    let any_call = "fn f(xs: &[i64]) -> bool { xs.iter().copied().any(|x| x > 0) }";
    let all_loop =
        "fn f(xs: &[i64]) -> bool { for &x in xs { if !(x >= 0) { return false; } } true }";
    let all_call = "fn f(xs: &[i64]) -> bool { xs.iter().copied().all(|x| x >= 0) }";
    let bad_predicate = "fn f(xs: &[i64]) -> bool { xs.iter().copied().any(|x| x >= 0) }";
    let any_fp = value_fp(&i, any_loop, Lang::Rust);
    assert_eq!(any_fp, value_fp(&i, any_call, Lang::Rust));
    assert_eq!(
        value_fp(&i, all_loop, Lang::Rust),
        value_fp(&i, all_call, Lang::Rust)
    );
    assert_ne!(any_fp, value_fp(&i, bad_predicate, Lang::Rust));
}

#[test]
fn python_math_prod_converges_with_product_loop() {
    let i = Interner::new();
    let loop_py = "def f(xs):\n    product = 1\n    for x in xs:\n        if x > 0:\n            product *= x\n    return product\n";
    let prod_py =
        "import math\n\ndef f(xs):\n    return math.prod((x for x in xs if x > 0), start=1)\n";
    let bad_seed =
        "import math\n\ndef f(xs):\n    return math.prod((x for x in xs if x > 0), start=2)\n";
    let loop_fp = value_fp(&i, loop_py, Lang::Python);
    assert_eq!(loop_fp, value_fp(&i, prod_py, Lang::Python));
    assert_ne!(loop_fp, value_fp(&i, bad_seed, Lang::Python));
}

#[test]
fn c_pointer_length_indexed_for_while_converge() {
    // C pointer+length loops do not expose `len(xs)` syntactically. We still recognize
    // the exact local pattern `i < n` + unit stride + `xs[i]` so C `for`/`while`
    // spellings converge, while offset/stride/inclusive-bound variants stay distinct.
    let i = Interner::new();
    let for_c = "int f(int *xs, int n) { int total = 0; for (int i = 0; i < n; i++) { if (xs[i] > 0) { total += xs[i]; } } return total; }";
    let while_c = "int g(int *ys, int m) { int j = 0; int sum = 0; while (j < m) { if (ys[j] > 0) { sum = sum + ys[j]; } j++; } return sum; }";
    let reversed_cond = "int h(int *xs, int n) { int total = 0; int i = 0; while (n > i) { if (xs[i] > 0) { total += xs[i]; } i++; } return total; }";
    let start_one = "int h(int *xs, int n) { int total = 0; for (int i = 1; i < n; i++) { if (xs[i] > 0) { total += xs[i]; } } return total; }";
    let stride_two = "int h(int *xs, int n) { int total = 0; for (int i = 0; i < n; i += 2) { if (xs[i] > 0) { total += xs[i]; } } return total; }";
    let inclusive = "int h(int *xs, int n) { int total = 0; for (int i = 0; i <= n; i++) { if (xs[i] > 0) { total += xs[i]; } } return total; }";

    let fp = value_fp(&i, for_c, Lang::C);
    assert_eq!(fp, value_fp(&i, while_c, Lang::C));
    assert_eq!(fp, value_fp(&i, reversed_cond, Lang::C));
    assert_ne!(fp, value_fp(&i, start_one, Lang::C));
    assert_ne!(fp, value_fp(&i, stride_two, Lang::C));
    assert_ne!(fp, value_fp(&i, inclusive, Lang::C));
}

#[test]
fn c_pointer_length_contract_converges_cross_language() {
    // Under the benchmark's C contract, `(int *xs, int n)` denotes the sequence of
    // exactly `n` elements. Only that strict `(collection, length)` parameter shape is
    // allowed to converge with high-level full-collection loops.
    let i = Interner::new();
    let c = "int f(int *xs, int n) { int total = 0; for (int i = 0; i < n; i++) { if (xs[i] > 0) { total += xs[i]; } } return total; }";
    let java = "class C { static int f(int[] xs) { int total = 0; for (int x : xs) { if (x > 0) { total += x; } } return total; } }";
    let ruby = "def f(xs)\n  total = 0\n  xs.each do |x|\n    if x > 0\n      total += x\n    end\n  end\n  total\nend\n";
    let param_order_not_contract = "int f(int n, int *xs) { int total = 0; for (int i = 0; i < n; i++) { if (xs[i] > 0) { total += xs[i]; } } return total; }";
    let inclusive = "int f(int *xs, int n) { int total = 0; for (int i = 0; i <= n; i++) { if (xs[i] > 0) { total += xs[i]; } } return total; }";

    let c_fp = value_fp(&i, c, Lang::C);
    assert_eq!(c_fp, value_fp(&i, java, Lang::Java));
    assert_eq!(c_fp, value_fp(&i, ruby, Lang::Ruby));
    assert_ne!(c_fp, value_fp(&i, param_order_not_contract, Lang::C));
    assert_ne!(c_fp, value_fp(&i, inclusive, Lang::C));
}

#[test]
fn c_integer_boolean_any_all_converge_cross_language() {
    // C commonly represents boolean predicate reductions as int 1/0. Treat that as a
    // boolean only inside the guarded early-return any/all pattern; other int returns
    // remain distinct.
    let i = Interner::new();
    let c_any = "int f(int *xs, int n) { for (int i = 0; i < n; i++) { if (xs[i] == 0) { return 1; } } return 0; }";
    let ruby_any = "def f(xs)\n  xs.each do |x|\n    if x == 0\n      return true\n    end\n  end\n  false\nend\n";
    let java_any = "class C { static boolean f(int[] xs) { for (int x : xs) { if (x == 0) { return true; } } return false; } }";
    let c_all = "int f(int *xs, int n) { for (int i = 0; i < n; i++) { if (!(xs[i] != 0)) { return 0; } } return 1; }";
    let java_all = "class C { static boolean f(int[] xs) { for (int x : xs) { if (!(x != 0)) { return false; } } return true; } }";
    let non_bool_return = "int f(int *xs, int n) { for (int i = 0; i < n; i++) { if (xs[i] == 0) { return 2; } } return 0; }";

    let any_fp = value_fp(&i, c_any, Lang::C);
    assert_eq!(any_fp, value_fp(&i, ruby_any, Lang::Ruby));
    assert_eq!(any_fp, value_fp(&i, java_any, Lang::Java));
    assert_eq!(
        value_fp(&i, c_all, Lang::C),
        value_fp(&i, java_all, Lang::Java)
    );
    assert_ne!(any_fp, value_fp(&i, non_bool_return, Lang::C));
}

#[test]
fn selection_reduction_loops_converge_cross_language() {
    let i = Interner::new();
    let py_max = "def f(xs):\n    best = 0\n    for x in xs:\n        if x > best:\n            best = x\n    return best\n";
    let js_max =
        "function f(xs){ let best = 0; for (const x of xs) { if (x > best) { best = x; } } return best; }";
    let reduce_js = "function f(xs){ return xs.reduce((best, x) => x > best ? x : best, 0); }";
    let rust_max =
        "fn f(xs: &[i32]) -> i32 { let mut best = 0; for &x in xs { if x > best { best = x; } } best }";
    let rust_fold_max =
        "fn f(xs: &[i32]) -> i32 { xs.iter().copied().fold(0, |best, x| if x > best { x } else { best }) }";
    let py_min = "def f(xs):\n    best = 0\n    for x in xs:\n        if x < best:\n            best = x\n    return best\n";
    let reduce_py =
        "def f(xs):\n    return reduce(lambda best, x: x if x < best else best, xs, 0)\n";
    let rust_min =
        "fn f(xs: &[i32]) -> i32 { let mut best = 0; for &x in xs { if x < best { best = x; } } best }";
    let rust_fold_min =
        "fn f(xs: &[i32]) -> i32 { xs.iter().copied().fold(0, |best, x| if x < best { x } else { best }) }";
    let bad_min = "def f(xs):\n    best = 0\n    for x in xs:\n        if x < best:\n            best = x\n    return best\n";

    let max_fp = value_fp(&i, py_max, Lang::Python);
    assert_eq!(max_fp, value_fp(&i, js_max, Lang::JavaScript));
    assert_eq!(max_fp, value_fp(&i, reduce_js, Lang::JavaScript));
    assert_eq!(max_fp, value_fp(&i, rust_max, Lang::Rust));
    assert_eq!(max_fp, value_fp(&i, rust_fold_max, Lang::Rust));
    let min_fp = value_fp(&i, py_min, Lang::Python);
    assert_eq!(min_fp, value_fp(&i, reduce_py, Lang::Python));
    assert_eq!(min_fp, value_fp(&i, rust_min, Lang::Rust));
    assert_eq!(
        min_fp,
        value_fp(&i, rust_fold_min, Lang::Rust),
        "Rust fold if-expression selection should converge with loop selection"
    );
    assert_ne!(max_fp, value_fp(&i, bad_min, Lang::Python));
}

#[test]
fn indexed_iteration_converges_range_and_while_multicollection() {
    // `C[idx]` for any index variable is the element of `C` (§AI), so a `range(len)`
    // indexed loop and a `while i<len` indexed loop converge — including the
    // multi-collection `a[i]*b[i]` dot product (i indexes both a and b).
    let i = Interner::new();
    let rng =
        "def d(a, b):\n    s = 0\n    for i in range(len(a)):\n        s = s + a[i] * b[i]\n    return s\n";
    let wh = "def d(a, b):\n    s = 0\n    i = 0\n    while i < len(a):\n        s = s + a[i] * b[i]\n        i = i + 1\n    return s\n";
    assert_eq!(
        value_fp(&i, rng, Lang::Python),
        value_fp(&i, wh, Lang::Python),
        "range-indexed and while-indexed dot products should converge"
    );
}

#[test]
fn zip_comprehension_converges_with_indexed_loop() {
    // `sum(x*y for x,y in zip(a,b))` binds the tuple to Elem(a), Elem(b) and converges
    // with the indexed `a[i]*b[i]` dot-product loop (§AI).
    let i = Interner::new();
    let zipc = "def d(a, b):\n    return sum(x * y for x, y in zip(a, b))\n";
    let loopv =
        "def d(a, b):\n    s = 0\n    for i in range(len(a)):\n        s = s + a[i] * b[i]\n    return s\n";
    assert_eq!(
        value_fp(&i, zipc, Lang::Python),
        value_fp(&i, loopv, Lang::Python),
        "zip dot-product should converge with the indexed loop"
    );
}

#[test]
fn dot_product_converges_across_index_zip_and_enumerate() {
    let i = Interner::new();
    let py_loop =
        "def d(a, b):\n    s = 0\n    for i in range(len(a)):\n        s += a[i] * b[i]\n    return s\n";
    let py_zip = "def d(a, b):\n    return sum(x * y for x, y in zip(a, b))\n";
    let go_range = "package p\nfunc d(a []int, b []int) int {\n\ts := 0\n\tfor i, x := range a {\n\t\ts += x * b[i]\n\t}\n\treturn s\n}\n";
    let go_for = "package p\nfunc d(a []int, b []int) int {\n\ts := 0\n\tfor i := 0; i < len(a); i++ {\n\t\ts += a[i] * b[i]\n\t}\n\treturn s\n}\n";
    let rust_range = "fn d(a: &[i32], b: &[i32]) -> i32 { let mut s = 0; for i in 0..a.len() { s += a[i] * b[i]; } s }";
    let rust_zip = "fn d(a: &[i32], b: &[i32]) -> i32 { a.iter().zip(b.iter()).fold(0, |s, (x, y)| s + *x * *y) }";
    let ruby_each =
        "def d(a, b)\n  s = 0\n  a.each_with_index do |x, i|\n    s += x * b[i]\n  end\n  s\nend\n";
    let ruby_while =
        "def d(a, b)\n  s = 0\n  i = 0\n  while i < a.length\n    s += a[i] * b[i]\n    i += 1\n  end\n  s\nend\n";
    let java_for = "class C { static int d(int[] a, int[] b) { int s = 0; for (int i = 0; i < a.length; i++) { s += a[i] * b[i]; } return s; } }";
    let c_for = "int d(int *a, int *b, int n) { int s = 0; for (int i = 0; i < n; i++) { s += a[i] * b[i]; } return s; }";
    let bad_pair_sum = "def d(a, b):\n    return sum(x + y for x, y in zip(a, b))\n";

    let fp = value_fp(&i, py_loop, Lang::Python);
    assert_eq!(fp, value_fp(&i, py_zip, Lang::Python));
    assert_eq!(fp, value_fp(&i, go_range, Lang::Go));
    assert_eq!(fp, value_fp(&i, go_for, Lang::Go));
    assert_eq!(fp, value_fp(&i, rust_range, Lang::Rust));
    assert_eq!(fp, value_fp(&i, rust_zip, Lang::Rust));
    assert_eq!(fp, value_fp(&i, ruby_each, Lang::Ruby));
    assert_eq!(fp, value_fp(&i, ruby_while, Lang::Ruby));
    assert_eq!(fp, value_fp(&i, java_for, Lang::Java));
    assert_eq!(fp, value_fp(&i, c_for, Lang::C));
    assert_ne!(fp, value_fp(&i, bad_pair_sum, Lang::Python));
}

#[test]
fn enumerate_converges_with_range_index() {
    // `for i, x in enumerate(xs)` and `for i in range(len(xs))` bind `i` to the same
    // canonical iteration index and `x`/`xs[i]` to the same element, so a first-match
    // search converges across the two iteration idioms (§AI).
    let i = Interner::new();
    let enum_ = "def ff(xs, t):\n    for i, x in enumerate(xs):\n        if x > t:\n            return i\n    return -1\n";
    let rng = "def ff(xs, t):\n    for i in range(len(xs)):\n        if xs[i] > t:\n            return i\n    return -1\n";
    assert_eq!(
        value_fp(&i, enum_, Lang::Python),
        value_fp(&i, rng, Lang::Python),
        "enumerate and range-index first-match should converge"
    );
}

#[test]
fn abs_idiom_converges() {
    // `abs(x)` and the `x if x>=0 else -x` idiom canonicalize to one Abs value (§AI).
    let i = Interner::new();
    let call = "def f(x):\n    return abs(x)\n";
    let tern = "def g(x):\n    return x if x >= 0 else -x\n";
    assert_eq!(
        value_fp(&i, call, Lang::Python),
        value_fp(&i, tern, Lang::Python),
        "abs(x) should converge with the conditional-negate idiom"
    );
}

#[test]
fn scalar_abs_axis_converges_with_unused_alternate_param() {
    let i = Interner::new();
    let call = "def f(value, other):\n    return abs(value)\n";
    let tern = "def g(value, other):\n    return value if value >= 0 else -value\n";
    assert_eq!(
        value_fp(&i, call, Lang::Python),
        value_fp(&i, tern, Lang::Python)
    );
}

#[test]
fn scalar_abs_builtins_converge_cross_language_with_shadow_boundary() {
    let i = Interner::new();
    let py = "def f(value, other):\n    magnitude = value if value >= 0 else -value\n    return magnitude + other\n";
    let js =
        "function f(value, other) { const magnitude = Math.abs(value); return magnitude + other; }";
    let ts = "function f(value: number, other: number): number { const magnitude = Math.abs(value); return magnitude + other; }";
    let go = "package p\n\nimport \"math\"\n\nfunc F(value float64, other float64) float64 { magnitude := math.Abs(value); return magnitude + other }\n";
    let java = "class C { static int f(int value, int other) { int magnitude = Math.abs(value); return magnitude + other; } }\n";
    let ruby_abs = "def f(value, other)\n  magnitude = value.abs\n  magnitude + other\nend\n";
    let rust_abs =
        "pub fn f(value: i64, other: i64) -> i64 { let magnitude = value.abs(); magnitude + other }\n";
    let shadowed_js = "function f(Math, value, other) { const magnitude = Math.abs(value); return magnitude + other; }";
    let local_shadowed_js = "function f(value, other) { const Math = { abs: function(_value) { return 0; } }; const magnitude = Math.abs(value); return magnitude + other; }";
    let custom_rust_abs = "struct Wrap(i64);\nimpl Wrap { fn abs(&self) -> i64 { 0 } }\npub fn f(value: Wrap) -> i64 { let magnitude = value.abs(); magnitude + 1 }\n";
    let fp = value_fp(&i, py, Lang::Python);
    assert_eq!(fp, value_fp(&i, js, Lang::JavaScript));
    assert_eq!(fp, value_fp(&i, ts, Lang::TypeScript));
    assert_eq!(fp, value_fp(&i, go, Lang::Go));
    assert_eq!(fp, value_fp(&i, java, Lang::Java));
    assert_eq!(fp, value_fp(&i, ruby_abs, Lang::Ruby));
    assert_eq!(fp, value_fp(&i, rust_abs, Lang::Rust));
    assert_ne!(fp, value_fp(&i, shadowed_js, Lang::JavaScript));
    assert_ne!(fp, value_fp(&i, local_shadowed_js, Lang::JavaScript));
    assert_ne!(fp, value_fp(&i, custom_rust_abs, Lang::Rust));
}

#[test]
fn scalar_minmax_builtins_converge_cross_language_with_shadow_boundary() {
    let i = Interner::new();
    let py_min = "def f(left, right, other):\n    selected = left if left <= right else right\n    return selected + other\n";
    let py_min_call =
        "def f(left, right, other):\n    selected = min(left, right)\n    return selected + other\n";
    let js_min = "function f(left, right, other) { const selected = Math.min(left, right); return selected + other; }";
    let ts_min = "function f(left: number, right: number, other: number): number { const selected = Math.min(left, right); return selected + other; }";
    let go_min = "package p\n\nimport \"math\"\n\nfunc F(left float64, right float64, other float64) float64 { selected := math.Min(left, right); return selected + other }\n";
    let java_min = "class C { static int f(int left, int right, int other) { int selected = Math.min(left, right); return selected + other; } }\n";
    let c_min = "#include <math.h>\n\ndouble f(double left, double right, double other) { double selected = fmin(left, right); return selected + other; }\n";
    let ruby_min =
        "def f(left, right, other)\n  selected = [left, right].min\n  selected + other\nend\n";
    let rust_min = "pub fn f(left: i64, right: i64, other: i64) -> i64 { let selected = left.min(right); selected + other }\n";
    let py_max = "def f(left, right, other):\n    selected = left if left >= right else right\n    return selected + other\n";
    let ruby_max =
        "def f(left, right, other)\n  selected = [left, right].max\n  selected + other\nend\n";
    let rust_max = "pub fn f(left: i64, right: i64, other: i64) -> i64 { let selected = left.max(right); selected + other }\n";
    let py_wrong_value =
        "def f(left, right, other):\n    selected = min(left, other)\n    return selected + other\n";
    let shadowed_js = "function f(left, right, other) { const Math = { min: function(_left, _right) { return 0; } }; const selected = Math.min(left, right); return selected + other; }";
    let custom_rust_min = "struct Wrap(i64);\nimpl Wrap { fn min(&self, _right: i64) -> i64 { 0 } }\npub fn f(left: Wrap, right: i64, other: i64) -> i64 { let selected = left.min(right); selected + other }\n";
    let custom_rust_max = "struct Wrap(i64);\nimpl Wrap { fn max(&self, _right: i64) -> i64 { 0 } }\npub fn f(left: Wrap, right: i64, other: i64) -> i64 { let selected = left.max(right); selected + other }\n";

    let fp = value_fp(&i, py_min, Lang::Python);
    assert_eq!(fp, value_fp(&i, py_min_call, Lang::Python));
    assert_eq!(fp, value_fp(&i, js_min, Lang::JavaScript));
    assert_eq!(fp, value_fp(&i, ts_min, Lang::TypeScript));
    assert_eq!(fp, value_fp(&i, go_min, Lang::Go));
    assert_eq!(fp, value_fp(&i, java_min, Lang::Java));
    assert_eq!(fp, value_fp(&i, c_min, Lang::C));
    assert_eq!(fp, value_fp(&i, ruby_min, Lang::Ruby));
    assert_eq!(fp, value_fp(&i, rust_min, Lang::Rust));
    assert_ne!(fp, value_fp(&i, py_max, Lang::Python));
    assert_eq!(
        value_fp(&i, py_max, Lang::Python),
        value_fp(&i, ruby_max, Lang::Ruby)
    );
    assert_eq!(
        value_fp(&i, py_max, Lang::Python),
        value_fp(&i, rust_max, Lang::Rust)
    );
    assert_ne!(fp, value_fp(&i, py_wrong_value, Lang::Python));
    assert_ne!(fp, value_fp(&i, shadowed_js, Lang::JavaScript));
    assert_ne!(fp, value_fp(&i, custom_rust_min, Lang::Rust));
    assert_ne!(
        value_fp(&i, py_max, Lang::Python),
        value_fp(&i, custom_rust_max, Lang::Rust)
    );
}

#[test]
fn conditional_abs_reduction_converges_with_aggregate() {
    // A branch in the per-element contribution is still a single reduction:
    // `total += (x < 0 ? -x : x)` must converge with aggregate `sum(abs(x))`.
    let i = Interner::new();
    let py_loop = "def f(xs):\n    total = 0\n    for x in xs:\n        if x < 0:\n            total += -x\n        else:\n            total += x\n    return total\n";
    let py_sum = "def f(xs):\n    return sum(abs(x) for x in xs)\n";
    let js_reduce =
        "function f(xs){ return xs.reduce((total, x) => total + (x < 0 ? -x : x), 0); }";
    let rust_fold =
        "fn f(xs: &[i32]) -> i32 { xs.iter().copied().fold(0, |total, x| total + if x < 0 { -x } else { x }) }";
    let c_loop = "int f(int *xs, int n) { int total = 0; for (int i = 0; i < n; i++) { if (xs[i] < 0) { total += -xs[i]; } else { total += xs[i]; } } return total; }";
    let bad_sum =
        "def f(xs):\n    total = 0\n    for x in xs:\n        total += x\n    return total\n";
    let fp = value_fp(&i, py_loop, Lang::Python);
    assert_eq!(fp, value_fp(&i, py_sum, Lang::Python));
    assert_eq!(fp, value_fp(&i, js_reduce, Lang::JavaScript));
    assert_eq!(fp, value_fp(&i, rust_fold, Lang::Rust));
    assert_eq!(fp, value_fp(&i, c_loop, Lang::C));
    assert_ne!(fp, value_fp(&i, bad_sum, Lang::Python));
}

#[test]
fn min_and_max_reductions_stay_distinct() {
    // `max(gen)` and `min(gen)` are distinct selection reductions — they must not
    // collapse (behavior axis, §AH).
    let i = Interner::new();
    let mx = "def fmx(xs):\n    return max(abs(x) for x in xs)\n";
    let mn = "def fmn(xs):\n    return min(abs(x) for x in xs)\n";
    assert_ne!(
        value_fp(&i, mx, Lang::Python),
        value_fp(&i, mn, Lang::Python),
        "max and min reductions must stay distinct"
    );
}

#[test]
fn if_assign_converges_with_ternary() {
    // `if c { x = a }` ≡ `x = a if c else x` (§AK): a statement-if that conditionally
    // assigns converges with the ternary form — the condition lives in the resulting
    // Phi merge, not a standalone sink.
    let i = Interner::new();
    let ifa = "def f(a, b):\n    m = a\n    if b > a:\n        m = b\n    return m\n";
    let tern = "def g(a, b):\n    m = a\n    m = b if b > a else m\n    return m\n";
    assert_eq!(
        value_fp(&i, ifa, Lang::Python),
        value_fp(&i, tern, Lang::Python),
        "conditional assignment should converge with the ternary"
    );
}

#[test]
fn branch_swapped_returns_stay_distinct() {
    // SOUNDNESS (§AJ): `if c {return a} else {return b}` and the branch-swapped
    // `if c {return b} else {return a}` compute DIFFERENT functions — path-sensitive
    // returns must keep their fingerprints distinct (they used to collapse to the same
    // order-insensitive multiset of return sinks; the oracle caught it).
    let i = Interner::new();
    let a = "def f(x):\n    if x > 0:\n        return x\n    else:\n        return -x\n";
    let b = "def g(x):\n    if x > 0:\n        return -x\n    else:\n        return x\n";
    assert_ne!(
        value_fp(&i, a, Lang::Python),
        value_fp(&i, b, Lang::Python),
        "branch-swapped returns must not have the same fingerprint"
    );
}

#[test]
fn c_total_order_comparator_guard_order_converges() {
    let i = Interner::new();
    let less_first = r#"
int f(const void *pa, const void *pb) {
    const int a = *(const int *)pa;
    const int b = *(const int *)pb;
    if (a < b)
        return -1;
    if (a > b)
        return 1;
    return 0;
}
"#;
    let greater_first = r#"
int g(const void *pa, const void *pb) {
    const int a = *(const int *)pa;
    const int b = *(const int *)pb;
    if (a > b)
        return 1;
    if (a < b)
        return -1;
    return 0;
}
"#;
    let ternary = r#"
int h(const void *pa, const void *pb) {
    const int *a = pa;
    const int *b = pb;
    return (*a > *b ? 1 : *a < *b ? -1 : 0);
}
"#;
    let fp = value_fp(&i, less_first, Lang::C);
    assert_eq!(
        fp,
        value_fp(&i, greater_first, Lang::C),
        "strict comparator guard order should not affect the fingerprint"
    );
    assert_eq!(
        fp,
        value_fp(&i, ternary, Lang::C),
        "strict if-return comparator should converge with the ternary sign form"
    );
}

#[test]
fn c_total_order_comparator_boundaries_stay_distinct() {
    let i = Interner::new();
    let ascending = r#"
int f(const void *pa, const void *pb) {
    const int a = *(const int *)pa;
    const int b = *(const int *)pb;
    if (a < b)
        return -1;
    if (a > b)
        return 1;
    return 0;
}
"#;
    let descending = r#"
int g(const void *pa, const void *pb) {
    const int a = *(const int *)pa;
    const int b = *(const int *)pb;
    if (a < b)
        return 1;
    if (a > b)
        return -1;
    return 0;
}
"#;
    let equal_as_less = r#"
int h(const void *pa, const void *pb) {
    const int a = *(const int *)pa;
    const int b = *(const int *)pb;
    if (a <= b)
        return -1;
    if (a > b)
        return 1;
    return 0;
}
"#;
    let fp = value_fp(&i, ascending, Lang::C);
    assert_ne!(
        fp,
        value_fp(&i, descending, Lang::C),
        "descending comparator order is a hard negative"
    );
    assert_ne!(
        fp,
        value_fp(&i, equal_as_less, Lang::C),
        "changing the equal case must stay distinct"
    );
}

#[test]
fn overloadable_comparator_guard_order_stays_distinct() {
    let i = Interner::new();
    let less_first = r#"
def f(a, b):
    if a < b:
        return -1
    if a > b:
        return 1
    return 0
"#;
    let greater_first = r#"
def g(a, b):
    if a > b:
        return 1
    if a < b:
        return -1
    return 0
"#;
    assert_ne!(
        value_fp(&i, less_first, Lang::Python),
        value_fp(&i, greater_first, Lang::Python),
        "Python comparison methods can be receiver-overloaded or effectful"
    );
}

#[test]
fn reduction_keeps_behavior_distinct() {
    // The behavior axis (§AH): a sum-loop and a product-loop share a skeleton but are
    // NOT behaviorally equivalent — their reductions must stay distinct.
    let i = Interner::new();
    let sum = "def f(xs):\n    t = 0\n    for x in xs:\n        t = t + x\n    return t\n";
    let prod = "def g(xs):\n    t = 1\n    for x in xs:\n        t = t * x\n    return t\n";
    assert_ne!(
        value_fp(&i, sum, Lang::Python),
        value_fp(&i, prod, Lang::Python),
        "sum vs product reductions must not collapse"
    );
}

#[test]
fn commutative_reconcile() {
    let i = Interner::new();
    let a = "def f(a, b):\n    return a + b\n";
    let b = "def g(a, b):\n    return b + a\n";
    // Numeric `+` commutativity is reconciled by the value graph (gated on non-concat).
    assert_eq!(value_fp(&i, a, Lang::Python), value_fp(&i, b, Lang::Python));
}

#[test]
fn cross_language_summation_converges() {
    let i = Interner::new();
    let py = "def f(items):\n    total = 0\n    i = 0\n    while i < len(items):\n        total += items[i]\n        i = i + 1\n    return total\n";
    let ts = "function f(items){ let total=0; for(let i=0;i<items.length;i++){ total += items[i]; } return total; }";
    let go = "package m\nfunc F(items []int) int {\n\ttotal := 0\n\tfor i := 0; i < len(items); i++ {\n\t\ttotal += items[i]\n\t}\n\treturn total\n}\n";
    let hp = unit_hash(&i, py, Lang::Python);
    assert_eq!(hp, unit_hash(&i, ts, Lang::TypeScript), "py == ts");
    assert_eq!(hp, unit_hash(&i, go, Lang::Go), "py == go");
}

#[test]
fn rust_alpha_equivalence_rename() {
    let i = Interner::new();
    let a = "fn f(items: &[i32]) -> i32 {\n    let mut total = 0;\n    for x in items {\n        total += x;\n    }\n    total\n}\n";
    let b = "fn g(xs: &[i32]) -> i32 {\n    let mut acc = 0;\n    for v in xs {\n        acc += v;\n    }\n    acc\n}\n";
    assert_eq!(unit_hash(&i, a, Lang::Rust), unit_hash(&i, b, Lang::Rust));
}

#[test]
fn rust_compound_assignment_desugars() {
    let i = Interner::new();
    let a = "fn f(n: i32) -> i32 {\n    let mut t = n;\n    t += 1;\n    t\n}\n";
    let b = "fn f(n: i32) -> i32 {\n    let mut t = n;\n    t = t + 1;\n    t\n}\n";
    assert_eq!(unit_hash(&i, a, Lang::Rust), unit_hash(&i, b, Lang::Rust));
}

#[test]
fn rust_sum_loop_converges_with_python() {
    // a Rust accumulator loop and the equivalent Python loop share IL shape
    let i = Interner::new();
    let rs = "fn f(items: &[i32]) -> i32 {\n    let mut total = 0;\n    for x in items {\n        total += x;\n    }\n    total\n}\n";
    let py =
        "def f(items):\n    total = 0\n    for x in items:\n        total += x\n    return total\n";
    assert_eq!(
        unit_hash(&i, rs, Lang::Rust),
        unit_hash(&i, py, Lang::Python),
        "rust == python sum loop"
    );
}

#[test]
fn rust_non_equivalent_different_op_differ() {
    let i = Interner::new();
    let a = "fn f(a: i32, b: i32) -> i32 {\n    a + b\n}\n";
    let b = "fn g(a: i32, b: i32) -> i32 {\n    a - b\n}\n";
    assert_ne!(unit_hash(&i, a, Lang::Rust), unit_hash(&i, b, Lang::Rust));
}

#[test]
fn rust_deref_peels_to_operand() {
    // `*x` is reference-level; it must not survive as a UnOp. A guarded sum that
    // derefs the element converges with the same loop written without the deref.
    let i = Interner::new();
    let with_deref = "fn f(items: &[i32]) -> i32 {\n    let mut total = 0;\n    for x in items {\n        if *x > 0 {\n            total = total + x;\n        }\n    }\n    total\n}\n";
    let plain = "fn g(items: &[i32]) -> i32 {\n    let mut total = 0;\n    for x in items {\n        if x > 0 {\n            total = total + x;\n        }\n    }\n    total\n}\n";
    assert_eq!(
        unit_hash(&i, with_deref, Lang::Rust),
        unit_hash(&i, plain, Lang::Rust),
        "`*x` must peel so it matches a plain `x`"
    );
}

#[test]
fn foreach_summation_converges_all_languages() {
    // A value-iteration accumulator loop converges to one IL shape across all five
    // languages — including Go's idiomatic `for _, x := range xs` (value binding).
    let i = Interner::new();
    let py =
        "def f(items):\n    total = 0\n    for x in items:\n        total += x\n    return total\n";
    let ts = "function f(items){ let total=0; for(const x of items){ total += x; } return total; }";
    let go = "package m\nfunc F(items []int) int {\n\ttotal := 0\n\tfor _, x := range items {\n\t\ttotal += x\n\t}\n\treturn total\n}\n";
    let rs = "fn f(items: &[i32]) -> i32 {\n    let mut total = 0;\n    for x in items {\n        total += x;\n    }\n    total\n}\n";
    let h = unit_hash(&i, py, Lang::Python);
    assert_eq!(h, unit_hash(&i, ts, Lang::TypeScript), "py == ts");
    assert_eq!(
        h,
        unit_hash(&i, go, Lang::Go),
        "py == go (range value binding)"
    );
    assert_eq!(h, unit_hash(&i, rs, Lang::Rust), "py == rust");
}

#[test]
fn fstring_converges_with_js_template() {
    // A Python f-string lowers to a string-concat chain (base Str + Add of each
    // interpolation), converging with a JS template literal.
    let i = Interner::new();
    let py = "def f(name):\n    return f\"hi {name}\"\n";
    let js = "function g(name){\n  return `hi ${name}`;\n}\n";
    assert_eq!(
        unit_hash(&i, py, Lang::Python),
        unit_hash(&i, js, Lang::JavaScript),
        "f-string == template literal"
    );
}

#[test]
fn fstring_format_spec_is_ignored() {
    // A format spec (`:>5`) is presentational — the interpolated expression is what
    // matters, so `f"{x:>5}"` lowers like `f"{x}"`.
    let i = Interner::new();
    let spec = "def f(x):\n    return f\"{x:>5}\"\n";
    let plain = "def g(x):\n    return f\"{x}\"\n";
    assert_eq!(
        unit_hash(&i, spec, Lang::Python),
        unit_hash(&i, plain, Lang::Python),
        "format spec doesn't change the lowered shape"
    );
}

#[test]
fn embedded_scripts_converge_with_plain_js_ts() {
    // The `<script>` logic in Vue/Svelte/HTML lowers exactly like the same code in a
    // plain .ts/.js file — the markup is blanked out, so only the script matters.
    let i = Interner::new();
    let body = "function f(items){ let t = 0; for (const x of items){ if (x > 0){ t = t + x; } } return t; }";
    let ts = body;
    let vue =
        format!("<template><b>{{{{n}}}}</b></template>\n<script lang=\"ts\">\n{body}\n</script>\n");
    let svelte = format!("<script lang=\"ts\">\n{body}\n</script>\n<p>markup</p>\n");
    let html = format!("<html><body>\n<script>\n{body}\n</script>\n</body></html>\n");
    let h = unit_hash(&i, ts, Lang::TypeScript);
    assert_eq!(h, unit_hash(&i, &vue, Lang::Vue), "vue <script> == ts");
    assert_eq!(
        h,
        unit_hash(&i, &svelte, Lang::Svelte),
        "svelte <script> == ts"
    );
    assert_eq!(
        h,
        unit_hash(&i, &html, Lang::Html),
        "html <script> == js/ts"
    );
}

#[test]
fn template_multi_interpolation_preserves_static_fragments() {
    // Static template fragments are behavior-defining. A multi-interpolation
    // template should match explicit concatenation and stay distinct when the
    // middle fragment changes or disappears.
    let i = Interner::new();
    let template = "function f(a, b){\n  return `${a} and ${b}`;\n}\n";
    let concat = "function g(a, b){\n  return \"\" + a + \" and \" + b;\n}\n";
    let missing_fragment = "function h(a, b){\n  return `${a}${b}`;\n}\n";
    assert_eq!(
        value_fp(&i, template, Lang::JavaScript),
        value_fp(&i, concat, Lang::JavaScript),
    );
    assert_ne!(
        value_fp(&i, template, Lang::JavaScript),
        value_fp(&i, missing_fragment, Lang::JavaScript),
    );
}

#[test]
fn conjoined_guard_merges() {
    // `if a: if b: X` ≡ `if a and b: X` (cfg_norm conjoined-guard merge).
    let i = Interner::new();
    let nested = "def f(a, b):\n    if a:\n        if b:\n            return 1\n    return 0\n";
    let conj = "def g(a, b):\n    if a and b:\n        return 1\n    return 0\n";
    assert_eq!(
        unit_hash(&i, nested, Lang::Python),
        unit_hash(&i, conj, Lang::Python),
        "nested ifs ≡ conjoined and"
    );
}

#[test]
fn continue_guard_unwraps() {
    // `for x: if c: continue; body` ≡ `for x: if not c: body` (continue-guard unwrap).
    let i = Interner::new();
    let cont = "def f(xs):\n    for x in xs:\n        if x < 0:\n            continue\n        process(x)\n";
    let guard = "def g(xs):\n    for x in xs:\n        if x >= 0:\n            process(x)\n";
    assert_eq!(
        unit_hash(&i, cont, Lang::Python),
        unit_hash(&i, guard, Lang::Python),
        "continue-guard ≡ inverted nested body"
    );
}

#[test]
fn branch_orientation_inverts_comparison_canonically() {
    // `if a < b { X } else { Y }` ≡ `if a >= b { Y } else { X }`: branch orientation
    // must invert the comparison into the *canonical* (Lt/Le) operand order, else the
    // two forms never converge. Regression for the Ge/Le canonicalization bug.
    let i = Interner::new();
    let lt = "def f(a, b, x, y):\n    if a < b:\n        r = x\n    else:\n        r = y\n    return r\n";
    let ge = "def g(a, b, x, y):\n    if a >= b:\n        r = y\n    else:\n        r = x\n    return r\n";
    assert_eq!(
        unit_hash(&i, lt, Lang::Python),
        unit_hash(&i, ge, Lang::Python),
        "a<b/else ≡ a>=b/swapped"
    );

    let le = "def f(a, b, x, y):\n    if a <= b:\n        r = x\n    else:\n        r = y\n    return r\n";
    let gt = "def g(a, b, x, y):\n    if a > b:\n        r = y\n    else:\n        r = x\n    return r\n";
    assert_eq!(
        unit_hash(&i, le, Lang::Python),
        unit_hash(&i, gt, Lang::Python),
        "a<=b/else ≡ a>b/swapped"
    );
}

#[test]
fn switch_converges_with_if_elif_chain() {
    // A JS `switch` and a Python if/elif chain over the same value normalize to the
    // same nested-`If` shape — a core Type-4 control-flow convergence.
    let i = Interner::new();
    let py = "def f(x):\n    if x == 1:\n        return 10\n    elif x == 2:\n        return 20\n    else:\n        return 0\n";
    let js = "function g(x){\n  switch(x){\n    case 1: return 10;\n    case 2: return 20;\n    default: return 0;\n  }\n}\n";
    assert_eq!(
        unit_hash(&i, py, Lang::Python),
        unit_hash(&i, js, Lang::JavaScript),
        "switch == if/elif chain"
    );
}

#[test]
fn ruby_interpolation_converges_with_python_fstring() {
    // Ruby `"hi #{name}"` lowers to the same concat chain as a Python f-string.
    let i = Interner::new();
    let rb = "def f(name)\n  \"hi #{name}\"\nend\n";
    let py = "def f(name):\n    return f\"hi {name}\"\n";
    assert_eq!(
        unit_hash(&i, rb, Lang::Ruby),
        unit_hash(&i, py, Lang::Python),
        "ruby interpolation == python f-string"
    );
}

#[test]
fn sum_and_product_loops_stay_distinct() {
    // Precision guard: a sum loop (`+=`, init 0) and a product loop (`*=`, init 1)
    // have the same shape but a different operation — they must NOT converge, or the
    // normalization would be over-merging behaviorally different code.
    let i = Interner::new();
    let sum = "def f(items):\n    acc = 0\n    for x in items:\n        acc += x\n    return acc\n";
    let prod =
        "def g(items):\n    acc = 1\n    for x in items:\n        acc *= x\n    return acc\n";
    assert_ne!(
        unit_hash(&i, sum, Lang::Python),
        unit_hash(&i, prod, Lang::Python),
        "+ and * loops must stay distinct"
    );
}

#[test]
fn try_except_converges_with_try_catch() {
    // Python try/except and JS try/catch normalize to the same `Try` structure.
    let i = Interner::new();
    let py = "def f():\n    try:\n        risky()\n    except Exception:\n        handle()\n";
    let js = "function g(){\n  try {\n    risky();\n  } catch (e) {\n    handle();\n  }\n}\n";
    assert_eq!(
        unit_hash(&i, py, Lang::Python),
        unit_hash(&i, js, Lang::JavaScript),
        "try/except == try/catch"
    );
}

#[test]
fn lambda_converges_with_js_arrow() {
    // A Python `lambda x: e` and a JS arrow `x => e` are both single-expression
    // callables; both lower to `Lambda(Block(Return(e)))` and converge.
    let i = Interner::new();
    let py = "def f():\n    return lambda x: x + 1\n";
    let js = "function g(){\n  return x => x + 1;\n}\n";
    assert_eq!(
        unit_hash(&i, py, Lang::Python),
        unit_hash(&i, js, Lang::JavaScript),
        "lambda == arrow"
    );
}

#[test]
fn optional_chaining_converges_with_plain_access() {
    // `a?.b?.c` is null-safe sugar over `a.b.c`; for structural matching they're the
    // same field-access chain.
    let i = Interner::new();
    let opt = "function f(a){\n  return a?.b?.c;\n}\n";
    let plain = "function g(a){\n  return a.b.c;\n}\n";
    assert_eq!(
        unit_hash(&i, opt, Lang::JavaScript),
        unit_hash(&i, plain, Lang::JavaScript),
        "a?.b?.c == a.b.c"
    );
}

#[test]
fn comprehension_converges_with_js_map() {
    // A Python list comprehension and a JS `.map` both canonicalize to `HoF Map`,
    // so the same transform written either way converges cross-language.
    let i = Interner::new();
    let py = "def f(items):\n    return [x * 2 for x in items]\n";
    let js = "function g(items){\n  return items.map(x => x * 2);\n}\n";
    assert_eq!(
        unit_hash(&i, py, Lang::Python),
        unit_hash(&i, js, Lang::JavaScript),
        "comprehension == .map (HoF canonicalization)"
    );
}

#[test]
fn find_max_converges_py_rust_ruby() {
    // A second algorithm shape (index `items[0]`, compare, branch-assign) converges
    // across languages — guards against shape-specific convergence over-fitting.
    let i = Interner::new();
    let py = "def f(items):\n    best = items[0]\n    for x in items:\n        if x > best:\n            best = x\n    return best\n";
    let rs = "fn g(items: &[i32]) -> i32 {\n    let mut best = items[0];\n    for x in items {\n        if *x > best {\n            best = *x;\n        }\n    }\n    best\n}\n";
    let rb = "def f(items)\n  best = items[0]\n  items.each do |x|\n    if x > best\n      best = x\n    end\n  end\n  best\nend\n";
    let h = unit_hash(&i, py, Lang::Python);
    assert_eq!(
        h,
        unit_hash(&i, rs, Lang::Rust),
        "python == rust (find-max)"
    );
    assert_eq!(
        h,
        unit_hash(&i, rb, Lang::Ruby),
        "python == ruby (find-max)"
    );
}

#[test]
fn java_method_converges_with_python_and_rust() {
    // A Java method's body converges with the equivalent Python/Rust foreach loop.
    let i = Interner::new();
    let java = "class C {\n    int f(int[] items) {\n        int total = 0;\n        for (int x : items) {\n            total += x;\n        }\n        return total;\n    }\n}\n";
    let py =
        "def f(items):\n    total = 0\n    for x in items:\n        total += x\n    return total\n";
    let rs = "fn f(items: &[i32]) -> i32 {\n    let mut total = 0;\n    for x in items {\n        total += x;\n    }\n    total\n}\n";
    let h = unit_hash(&i, py, Lang::Python);
    assert_eq!(h, unit_hash(&i, java, Lang::Java), "python == java");
    assert_eq!(h, unit_hash(&i, rs, Lang::Rust), "python == rust");
}

#[test]
fn java_compound_assignment_desugars() {
    let i = Interner::new();
    let a = "class C { int f(int n) { int t = 1; t += n; return t; } }";
    let b = "class C { int g(int m) { int t = 1; t = t + m; return t; } }";
    assert_eq!(unit_hash(&i, a, Lang::Java), unit_hash(&i, b, Lang::Java));
}

#[test]
fn c_alpha_equivalence_and_compound_assign() {
    let i = Interner::new();
    let a = "int f(int* xs, int n) {\n    int total = 0;\n    for (int k = 0; k < n; k++) {\n        total += xs[k];\n    }\n    return total;\n}\n";
    let b = "int g(int* arr, int m) {\n    int acc = 0;\n    for (int j = 0; j < m; j++) {\n        acc = acc + arr[j];\n    }\n    return acc;\n}\n";
    assert_eq!(
        unit_hash(&i, a, Lang::C),
        unit_hash(&i, b, Lang::C),
        "rename + compound-assign converge in C"
    );
}

#[test]
fn c_non_equivalent_different_op_differ() {
    let i = Interner::new();
    let a = "int f(int a, int b) { return a + b; }";
    let b = "int g(int a, int b) { return a * b; }";
    assert_ne!(unit_hash(&i, a, Lang::C), unit_hash(&i, b, Lang::C));
}

#[test]
fn ruby_each_converges_with_python_foreach() {
    // Ruby `xs.each { |x| … }` and a Python `for x in xs` loop share IL shape.
    let i = Interner::new();
    let rb =
        "def f(items)\n  total = 0\n  items.each do |x|\n    total += x\n  end\n  total\nend\n";
    let py =
        "def f(items):\n    total = 0\n    for x in items:\n        total += x\n    return total\n";
    assert_eq!(
        unit_hash(&i, rb, Lang::Ruby),
        unit_hash(&i, py, Lang::Python),
        "ruby each == python for"
    );
}

#[test]
fn ruby_alpha_and_compound_assign() {
    let i = Interner::new();
    let a = "def f(items)\n  total = 0\n  items.each do |x|\n    total += x\n  end\n  total\nend\n";
    let b = "def g(seq)\n  acc = 0\n  seq.each do |v|\n    acc = acc + v\n  end\n  acc\nend\n";
    assert_eq!(unit_hash(&i, a, Lang::Ruby), unit_hash(&i, b, Lang::Ruby));
}

#[test]
fn ruby_guard_modifier_converges_with_block_if() {
    // `stmt if cond` (modifier) must lower to the same IL as the block `if`.
    let i = Interner::new();
    let modifier = "def f(x)\n  log(x) if x\n  done()\nend\n";
    let block = "def g(y)\n  if y\n    log(y)\n  end\n  done()\nend\n";
    assert_eq!(
        unit_hash(&i, modifier, Lang::Ruby),
        unit_hash(&i, block, Lang::Ruby),
        "ruby guard-clause modifier == block if"
    );
}

#[test]
fn rust_macro_args_captured_and_alpha() {
    // Macro arguments (atoms inside the token tree) are captured as call args and
    // alpha-renamed, so two structurally-identical macro uses converge.
    let i = Interner::new();
    let a = "fn f(x: i32) -> i32 { assert_eq!(x, 1); let v = vec![x, x]; x }";
    let b = "fn g(y: i32) -> i32 { assert_eq!(y, 1); let v = vec![y, y]; y }";
    assert_eq!(
        unit_hash(&i, a, Lang::Rust),
        unit_hash(&i, b, Lang::Rust),
        "rust macro args captured + alpha-renamed"
    );
}

#[test]
fn rust_commutative_reconcile() {
    let i = Interner::new();
    let a = "fn f(a: i32, b: i32) -> i32 {\n    a + b\n}\n";
    let b = "fn g(a: i32, b: i32) -> i32 {\n    b + a\n}\n";
    // `i32` operands are Num, so the value graph sorts the `+` operands — converged.
    assert_eq!(value_fp(&i, a, Lang::Rust), value_fp(&i, b, Lang::Rust));
}

#[test]
fn non_equivalent_swapped_params_differ() {
    // `a - b` with params (a,b) must NOT match `b - a` with params (a,b):
    // subtraction is non-commutative and the data flow differs.
    let i = Interner::new();
    let a = "def f(a, b):\n    return a - b\n";
    let b = "def g(a, b):\n    return b - a\n";
    assert_ne!(
        unit_hash(&i, a, Lang::Python),
        unit_hash(&i, b, Lang::Python)
    );
}

#[test]
fn comprehension_equals_js_map() {
    let i = Interner::new();
    let py = "def f(xs):\n    return [x * 2 for x in xs]\n";
    let ts = "function f(xs){ return xs.map(x => x * 2); }";
    assert_eq!(
        unit_hash(&i, py, Lang::Python),
        unit_hash(&i, ts, Lang::TypeScript)
    );
}

#[test]
fn template_literal_equals_concat() {
    let i = Interner::new();
    let concat = "function f(x){ return \"a\" + x; }";
    let template = "function g(x){ return `a${x}`; }";
    assert_eq!(
        unit_hash(&i, concat, Lang::TypeScript),
        unit_hash(&i, template, Lang::TypeScript)
    );
}

#[test]
fn print_builtin_converges_cross_language() {
    let i = Interner::new();
    let py = "def f(x):\n    print(x)\n";
    let js = "function f(x){ console.log(x); }";
    let go = "package m\nfunc F(x int) {\n\tfmt.Println(x)\n}\n";
    let hp = unit_hash(&i, py, Lang::Python);
    assert_eq!(
        hp,
        unit_hash(&i, js, Lang::JavaScript),
        "py print == js console.log"
    );
    assert_eq!(
        hp,
        unit_hash(&i, go, Lang::Go),
        "py print == go fmt.Println"
    );
}

#[test]
fn guard_clause_equals_nested_else() {
    // else-after-return flattening makes these converge.
    let i = Interner::new();
    let guard = "def f(x):\n    if x:\n        return 1\n    return 2\n";
    let nested = "def g(x):\n    if x:\n        return 1\n    else:\n        return 2\n";
    assert_eq!(
        unit_hash(&i, guard, Lang::Python),
        unit_hash(&i, nested, Lang::Python)
    );
}

#[test]
fn switch_equals_if_chain() {
    let i = Interner::new();
    let sw = "function f(x){ switch(x){ case 1: return 10; default: return 0; } }";
    let ifc = "function g(x){ if (x === 1) { return 10; } else { return 0; } }";
    assert_eq!(
        unit_hash(&i, sw, Lang::TypeScript),
        unit_hash(&i, ifc, Lang::TypeScript)
    );
}

#[test]
fn single_use_temp_inlines() {
    let i = Interner::new();
    let with_temp = "def f(a, b):\n    t = a + b\n    return t * 2\n";
    let inlined = "def g(a, b):\n    return (a + b) * 2\n";
    assert_eq!(
        unit_hash(&i, with_temp, Lang::Python),
        unit_hash(&i, inlined, Lang::Python)
    );
}

#[test]
fn temp_chain_folds() {
    let i = Interner::new();
    let chained = "def f(a):\n    x = a + 1\n    y = x * 3\n    return y - 2\n";
    let direct = "def g(a):\n    return ((a + 1) * 3) - 2\n";
    assert_eq!(
        unit_hash(&i, chained, Lang::Python),
        unit_hash(&i, direct, Lang::Python)
    );
}

#[test]
fn temp_inlining_crosses_languages() {
    let i = Interner::new();
    let py = "def f(a, b):\n    s = a + b\n    return s * s\n";
    // `s` is used twice → NOT inlined; this stays a fair structural match to a TS
    // version that also keeps the temp.
    let ts = "function g(a, b){ const s = a + b; return s * s; }";
    assert_eq!(
        unit_hash(&i, py, Lang::Python),
        unit_hash(&i, ts, Lang::TypeScript)
    );
}

#[test]
fn provenance_spans_survive_normalization() {
    let i = Interner::new();
    let src = "def alpha(x):\n    return x\n\ndef beta(y):\n    return y + 1\n";
    let il =
        nose_frontend::lower_source(FileId(0), "t.py", src.as_bytes(), Lang::Python, &i).unwrap();
    let n = normalize(&il, &i, &NormalizeOptions::default());
    // The first function unit should still point at line 1.
    let alpha = n
        .units
        .iter()
        .find(|u| u.name == Some(i.intern("alpha")))
        .unwrap();
    assert_eq!(n.node(alpha.root).span.start_line, 1);
    let beta = n
        .units
        .iter()
        .find(|u| u.name == Some(i.intern("beta")))
        .unwrap();
    assert_eq!(n.node(beta.root).span.start_line, 4);
}

#[test]
fn cfg_nested_guard_equals_conjunction() {
    let i = Interner::new();
    let nested = "def f(a, b):\n    if a:\n        if b:\n            return 1\n    return 0\n";
    let conj = "def g(a, b):\n    if a and b:\n        return 1\n    return 0\n";
    assert_eq!(
        unit_hash(&i, nested, Lang::Python),
        unit_hash(&i, conj, Lang::Python)
    );
}

#[test]
fn cfg_continue_guard_equals_nested() {
    let i = Interner::new();
    let cont = "def f(xs):\n    total = 0\n    for x in xs:\n        if x < 0:\n            continue\n        total = total + x\n    return total\n";
    let nested = "def g(ys):\n    total = 0\n    for y in ys:\n        if y >= 0:\n            total = total + y\n    return total\n";
    assert_eq!(
        unit_hash(&i, cont, Lang::Python),
        unit_hash(&i, nested, Lang::Python)
    );
}

#[test]
fn algebra_associativity() {
    let i = Interner::new();
    let left = "def f(a, b, c):\n    return (a + b) + c\n";
    let right = "def g(a, b, c):\n    return a + (b + c)\n";
    let mixed = "def h(a, b, c):\n    return c + (a + b)\n";
    // `+` commutativity/associativity is reconciled by the value graph (type-GATED on
    // concat), not the algebra IL pass — so check the fingerprint, not the IL hash.
    let hl = value_fp(&i, left, Lang::Python);
    assert_eq!(hl, value_fp(&i, right, Lang::Python));
    assert_eq!(hl, value_fp(&i, mixed, Lang::Python));
}

#[test]
fn algebra_comparison_direction() {
    let i = Interner::new();
    let gt = "def f(a, b):\n    return a > b\n";
    let lt = "def g(a, b):\n    return b < a\n";
    assert_eq!(
        unit_hash(&i, gt, Lang::Python),
        unit_hash(&i, lt, Lang::Python)
    );
}

#[test]
fn algebra_de_morgan() {
    let i = Interner::new();
    let a = "def f(a, b):\n    return not (a and b)\n";
    let b = "def g(a, b):\n    return (not a) or (not b)\n";
    assert_eq!(
        unit_hash(&i, a, Lang::Python),
        unit_hash(&i, b, Lang::Python)
    );
}

#[test]
fn algebra_double_negation() {
    // `!!x` is `bool(x)` (truthiness), NOT `x` — it equals `x` ONLY when x is already Bool.
    // So `not not (x>0)` ≡ `x>0` (bool), but `not not x` ≢ `x` for an untyped x (`!!5` =
    // true ≠ 5 — converging them was a latent false merge the independent oracle exposed).
    let i = Interner::new();
    let bool_a = "def f(x):\n    return not (not (x > 0))\n";
    let bool_b = "def g(x):\n    return x > 0\n";
    assert_eq!(
        value_fp(&i, bool_a, Lang::Python),
        value_fp(&i, bool_b, Lang::Python),
        "double-negation of a bool must cancel"
    );
    let any_a = "def f(x):\n    return not (not x)\n";
    let any_b = "def g(x):\n    return x\n";
    assert_ne!(
        value_fp(&i, any_a, Lang::Python),
        value_fp(&i, any_b, Lang::Python),
        "double-negation of an untyped value must NOT cancel (it coerces to bool)"
    );
}

/// Collection-building loops converge with comprehensions / `.map` / `.push`: a local list
/// built by appending per element IS `[f(x) for x in xs]`. Cross-language and with a filter.
#[test]
fn list_builder_loop_converges_with_comprehension() {
    let i = Interner::new();
    let comp = value_fp(
        &i,
        "def f(xs):\n    return [x*x for x in xs]\n",
        Lang::Python,
    );
    let loop_py = value_fp(
        &i,
        "def g(xs):\n    r=[]\n    for x in xs:\n        r.append(x*x)\n    return r\n",
        Lang::Python,
    );
    let loop_js = value_fp(
        &i,
        "function g(xs){ let r=[]; for(const x of xs){ r.push(x*x); } return r; }",
        Lang::JavaScript,
    );
    assert_eq!(comp, loop_py, "append-loop ≡ list comprehension");
    assert_eq!(
        comp, loop_js,
        "JS push-loop ≡ Python comprehension (cross-language)"
    );
    let fcomp = value_fp(
        &i,
        "def f(xs):\n    return [x*x for x in xs if x>0]\n",
        Lang::Python,
    );
    let floop = value_fp(
        &i,
        "def g(xs):\n    r=[]\n    for x in xs:\n        if x>0:\n            r.append(x*x)\n    return r\n",
        Lang::Python,
    );
    assert_eq!(
        fcomp, floop,
        "filtered append-loop ≡ filtered comprehension"
    );
    assert_ne!(comp, fcomp, "unfiltered and filtered must stay distinct");
}

/// Multi-clause comprehensions are flat maps, not nested maps. They should converge with
/// an equivalent nested append loop and JS `.flatMap(... .map(...))`, while staying
/// distinct from a nested list comprehension.
#[test]
fn multi_clause_comprehension_converges_as_flat_map() {
    let i = Interner::new();
    let comp = value_fp(
        &i,
        "def f(xs, ys):\n    return [x + y for x in xs for y in ys]\n",
        Lang::Python,
    );
    let loop_py = value_fp(
        &i,
        "def g(xs, ys):\n    r = []\n    for x in xs:\n        for y in ys:\n            r.append(x + y)\n    return r\n",
        Lang::Python,
    );
    let flat_map_js = value_fp(
        &i,
        "function h(xs, ys){ return xs.flatMap(x => ys.map(y => x + y)); }",
        Lang::JavaScript,
    );
    let nested_list = value_fp(
        &i,
        "def h(xs, ys):\n    return [[x + y for y in ys] for x in xs]\n",
        Lang::Python,
    );

    assert_eq!(
        comp, loop_py,
        "flat comprehension should match nested append loop"
    );
    assert_eq!(
        comp, flat_map_js,
        "flat comprehension should match JS flatMap/map"
    );
    assert_ne!(
        comp, nested_list,
        "flat-map and nested-list comprehensions differ"
    );
}

/// Cross-language `any`/`all` predicate reductions converge to one fingerprint: Python
/// `any(p(x) for x in xs)`, JS `xs.some(p)`, Rust `xs.iter().any(p)` — and likewise
/// `all`/`every`. `any` and `all` stay DISTINCT (different short-circuit behavior).
#[test]
fn cross_language_any_all_converges() {
    let i = Interner::new();
    let any_py = value_fp(
        &i,
        "def f(xs):\n    return any(x > 0 for x in xs)\n",
        Lang::Python,
    );
    let any_js = value_fp(
        &i,
        "function g(xs){ return xs.some(x => x > 0); }",
        Lang::JavaScript,
    );
    let any_rs = value_fp(
        &i,
        "fn h(xs: &[i64]) -> bool { xs.iter().any(|x| *x > 0) }",
        Lang::Rust,
    );
    let all_py = value_fp(
        &i,
        "def f(xs):\n    return all(x > 0 for x in xs)\n",
        Lang::Python,
    );
    let all_js = value_fp(
        &i,
        "function g(xs){ return xs.every(x => x > 0); }",
        Lang::JavaScript,
    );
    assert_eq!(any_py, any_js, "Python any ≡ JS some");
    assert_eq!(any_py, any_rs, "Python any ≡ Rust any");
    assert_eq!(all_py, all_js, "Python all ≡ JS every");
    assert_ne!(any_py, all_py, "any and all must stay distinct");
    assert!(!any_py.is_empty());
}

/// Value-graph fingerprint of the first function unit.
fn value_fp(interner: &Interner, src: &str, lang: Lang) -> Vec<u64> {
    let il = nose_frontend::lower_source(FileId(0), "t", src.as_bytes(), lang, interner).unwrap();
    let n = normalize(&il, interner, &NormalizeOptions::default());
    nose_normalize::value_fingerprint(&n, first_func(&n), interner)
}

fn value_fp_named(interner: &Interner, src: &str, lang: Lang, name: &str) -> Vec<u64> {
    let il = nose_frontend::lower_source(FileId(0), "t", src.as_bytes(), lang, interner).unwrap();
    let n = normalize(&il, interner, &NormalizeOptions::default());
    let root = n
        .units
        .iter()
        .find(|unit| {
            unit.name
                .is_some_and(|symbol| interner.resolve(symbol) == name)
        })
        .map(|unit| unit.root)
        .unwrap_or_else(|| panic!("expected function unit named {name}"));
    nose_normalize::value_fingerprint(&n, root, interner)
}

#[test]
fn python_docstrings_are_function_semantic_noops() {
    let i = Interner::new();
    let plain = "def f(i, j):\n    if i == j:\n        return 1\n    return 0\n";
    let docstring = "def g(i, j):\n    \"\"\"Return one when the indexes match.\"\"\"\n    if i == j:\n        return 1\n    else:\n        return 0\n";
    let other_docstring = "def h(i, j):\n    \"\"\"Different documentation text.\"\"\"\n    if i == j:\n        return 1\n    return 0\n";

    assert_eq!(
        value_fp(&i, plain, Lang::Python),
        value_fp(&i, docstring, Lang::Python),
        "a Python function docstring must not change call behavior"
    );
    assert_eq!(
        value_fp(&i, plain, Lang::Python),
        value_fp(&i, other_docstring, Lang::Python),
        "docstring text is metadata, not function return behavior"
    );

    let returned_red = "def f():\n    return \"red\"\n";
    let returned_blue = "def g():\n    return \"blue\"\n";
    assert_ne!(
        value_fp(&i, returned_red, Lang::Python),
        value_fp(&i, returned_blue, Lang::Python),
        "returned strings are behavior-defining values"
    );

    let f_string = "def f(x):\n    f\"{x}\"\n    return 1\n";
    let no_effect = "def g(x):\n    return 1\n";
    assert_ne!(
        value_fp(&i, f_string, Lang::Python),
        value_fp(&i, no_effect, Lang::Python),
        "a leading f-string expression is not a static docstring proof"
    );
}

#[test]
fn value_graph_ignores_statement_order() {
    // x and y are each used twice → NOT inlined; only the value graph (not the
    // AST) makes the two statement orders converge.
    let i = Interner::new();
    let a = "def f(a, b):\n    x = a + 1\n    y = b + 1\n    return x * y + x\n";
    let b = "def g(a, b):\n    y = b + 1\n    x = a + 1\n    return x * y + x\n";
    assert_eq!(value_fp(&i, a, Lang::Python), value_fp(&i, b, Lang::Python));
}

#[test]
fn value_graph_cse_temp_vs_repeated() {
    let i = Interner::new();
    let temp = "def f(a, b):\n    t = a + b\n    return t + t\n";
    let repeated = "def g(a, b):\n    return (a + b) + (a + b)\n";
    assert_eq!(
        value_fp(&i, temp, Lang::Python),
        value_fp(&i, repeated, Lang::Python)
    );
}

#[test]
fn value_graph_distinguishes_different_ops() {
    let i = Interner::new();
    let add = "def f(a, b):\n    return a + b\n";
    let sub = "def g(a, b):\n    return a - b\n";
    assert_ne!(
        value_fp(&i, add, Lang::Python),
        value_fp(&i, sub, Lang::Python)
    );
}

#[test]
fn value_graph_distinguishes_range_start_offset() {
    // `range(len(a))` sums every element; `range(1, len(a))` skips a[0]. They are
    // behaviorally DIFFERENT, so their value fingerprints must differ. Abstracting
    // `a[i]` → `Elem(a)` for a *partial* range (dropping the start bound) is a false
    // merge — a genuine soundness bug.
    let i = Interner::new();
    let full =
        "def f(a):\n    s = 0\n    for i in range(len(a)):\n        s += a[i]\n    return s\n";
    let skip =
        "def g(a):\n    s = 0\n    for i in range(1, len(a)):\n        s += a[i]\n    return s\n";
    assert_ne!(
        value_fp(&i, full, Lang::Python),
        value_fp(&i, skip, Lang::Python),
        "a partial range (skipping a[0]) must not fingerprint identically to the full range"
    );
}

#[test]
fn value_graph_distinguishes_constants() {
    // Behavior-defining numeric constants must stay distinct in the value graph
    // (the §AT axis-split): `x%7` ≢ `x%11`, `return 100` ≢ `return 200`. Large ints
    // were abstracted to one `Int` class — a latent false merge.
    let i = Interner::new();
    let m7 = "def f(x):\n    return x % 7\n";
    let m11 = "def f(x):\n    return x % 11\n";
    assert_ne!(
        value_fp(&i, m7, Lang::Python),
        value_fp(&i, m11, Lang::Python),
        "x%7 and x%11 are behaviorally different"
    );
    let a = "def f(x):\n    return x + 100\n";
    let b = "def f(x):\n    return x + 200\n";
    assert_ne!(
        value_fp(&i, a, Lang::Python),
        value_fp(&i, b, Lang::Python),
        "x+100 and x+200 are behaviorally different"
    );
}

#[test]
fn value_graph_distinguishes_for_in_vs_of() {
    // JS `for (x of it)` iterates VALUES, `for (x in it)` iterates KEYS — different.
    let i = Interner::new();
    let of = "function f(a){ let s = 0; for (const x of a) { s += x; } return s; }";
    let in_ = "function f(a){ let s = 0; for (const x in a) { s += x; } return s; }";
    assert_ne!(
        value_fp(&i, of, Lang::JavaScript),
        value_fp(&i, in_, Lang::JavaScript),
        "for-of (values) must differ from for-in (keys)"
    );
}

#[test]
fn value_graph_distinguishes_conditional_early_return() {
    // A conditional early `return;` (void) changes which later code runs — it must not
    // be invisible. Two loops differing only in an early-exit guard must differ.
    let i = Interner::new();
    let early = "def f(xs, x):\n    for v in xs:\n        if x > 0:\n            return\n        g(v)\n    h()\n";
    let always = "def f(xs, x):\n    for v in xs:\n        return\n        g(v)\n    h()\n";
    assert_ne!(
        value_fp(&i, early, Lang::Python),
        value_fp(&i, always, Lang::Python),
        "a conditional early return must differ from an unconditional one"
    );
}

#[test]
fn value_graph_distinguishes_throw_from_expression_effect() {
    let i = Interner::new();
    let thrown = "function f() { throw \"x\"; }";
    let expr = "function f() { \"x\"; }";
    assert_ne!(
        value_fp(&i, thrown, Lang::JavaScript),
        value_fp(&i, expr, Lang::JavaScript),
        "throw is terminal error behavior, not a plain expression effect"
    );
}

#[test]
fn value_graph_reads_field_written_in_unit() {
    let i = Interner::new();
    let read_field = "def f(self):\n    self.x = 7\n    return self.x\n";
    let return_value = "def f(self):\n    self.x = 7\n    return 7\n";
    assert_eq!(
        value_fp(&i, read_field, Lang::Python),
        value_fp(&i, return_value, Lang::Python),
        "a field read after a same-unit field write should resolve to the written value"
    );
}

#[test]
fn value_graph_skips_try_handler_after_normal_return() {
    let i = Interner::new();
    let try_return =
        "def f():\n    try:\n        return 1\n    except Exception:\n        return 7\n";
    let plain_return = "def f():\n    return 1\n";
    assert_eq!(
        value_fp(&i, try_return, Lang::Python),
        value_fp(&i, plain_return, Lang::Python),
        "a try handler should not contribute when the try body already returned normally"
    );
}

#[test]
fn value_graph_runs_try_handler_after_bare_throw() {
    let i = Interner::new();
    let try_throw = "function f() { try { throw \"x\"; } catch (err) { return 7; } }";
    let plain_return = "function f() { return 7; }";
    assert_eq!(
        value_fp(&i, try_throw, Lang::JavaScript),
        value_fp(&i, plain_return, Lang::JavaScript),
        "a side-effect-free throw body should be replaced by the simple catch handler"
    );
}

#[test]
fn value_graph_runs_try_handler_after_pure_throw_prefix() {
    let i = Interner::new();
    let try_throw = "function f() { try { 1 + 2; throw \"x\"; } catch (err) { return 7; } }";
    let plain_return = "function f() { return 7; }";
    assert_eq!(
        value_fp(&i, try_throw, Lang::JavaScript),
        value_fp(&i, plain_return, Lang::JavaScript),
        "pure statements before a throw should not block the simple catch handler"
    );
}

#[test]
fn value_graph_keeps_try_throw_prefix_effects() {
    let i = Interner::new();
    let effect_then_throw = "def f():\n    try:\n        print(1)\n        raise Exception()\n    except Exception:\n        return 7\n";
    let plain_return = "def f():\n    return 7\n";
    assert_ne!(
        value_fp(&i, effect_then_throw, Lang::Python),
        value_fp(&i, plain_return, Lang::Python),
        "observable effects before a throw must not be discarded with the try body"
    );
}

#[test]
fn value_graph_runs_try_handler_after_static_expr_err() {
    let i = Interner::new();
    let try_err = "def f():\n    try:\n        1 / 0\n    except Exception:\n        return 7\n";
    let plain_return = "def f():\n    return 7\n";
    assert_eq!(
        value_fp(&i, try_err, Lang::Python),
        value_fp(&i, plain_return, Lang::Python),
        "a statically visible expression error should run the simple catch handler"
    );
}

#[test]
fn value_graph_runs_try_handler_after_static_return_err() {
    let i = Interner::new();
    let try_err =
        "def f():\n    try:\n        return 1 / 0\n    except Exception:\n        return 7\n";
    let plain_return = "def f():\n    return 7\n";
    assert_eq!(
        value_fp(&i, try_err, Lang::Python),
        value_fp(&i, plain_return, Lang::Python),
        "a statically visible return expression error is not a normal try-body return"
    );
}

#[test]
fn value_graph_runs_try_handler_after_static_ternary_condition_err() {
    let i = Interner::new();
    let try_err =
        "def f():\n    try:\n        return 1 if 1 / 0 else 2\n    except Exception:\n        return 7\n";
    let plain_return = "def f():\n    return 7\n";
    assert_eq!(
        value_fp(&i, try_err, Lang::Python),
        value_fp(&i, plain_return, Lang::Python),
        "a statically visible ternary condition error should run the simple catch handler"
    );
}

#[test]
fn value_graph_runs_try_handler_after_static_selected_ternary_branch_err() {
    let i = Interner::new();
    let try_err =
        "def f():\n    try:\n        return 1 / 0 if True else 2\n    except Exception:\n        return 7\n";
    let plain_return = "def f():\n    return 7\n";
    assert_eq!(
        value_fp(&i, try_err, Lang::Python),
        value_fp(&i, plain_return, Lang::Python),
        "a statically selected ternary branch error should run the simple catch handler"
    );
}

#[test]
fn value_graph_runs_try_handler_after_static_pow_err() {
    let i = Interner::new();
    let try_err =
        "def f():\n    try:\n        return 2 ** -1\n    except Exception:\n        return 7\n";
    let plain_return = "def f():\n    return 7\n";
    assert_eq!(
        value_fp(&i, try_err, Lang::Python),
        value_fp(&i, plain_return, Lang::Python),
        "a statically visible pow exponent error should run the simple catch handler"
    );
}

#[test]
fn value_graph_runs_try_handler_after_static_unary_operand_err() {
    let i = Interner::new();
    let try_err =
        "def f():\n    try:\n        return -(1 / 0)\n    except Exception:\n        return 7\n";
    let plain_return = "def f():\n    return 7\n";
    assert_eq!(
        value_fp(&i, try_err, Lang::Python),
        value_fp(&i, plain_return, Lang::Python),
        "a statically visible unary operand error should run the simple catch handler"
    );
}

#[test]
fn value_graph_runs_try_handler_after_static_binop_left_err() {
    let i = Interner::new();
    let try_err =
        "def f():\n    try:\n        return (1 / 0) + print(1)\n    except Exception:\n        return 7\n";
    let plain_return = "def f():\n    return 7\n";
    assert_eq!(
        value_fp(&i, try_err, Lang::Python),
        value_fp(&i, plain_return, Lang::Python),
        "a statically visible binary left operand error should run the simple catch handler"
    );
}

#[test]
fn value_graph_keeps_try_binop_left_effects_before_static_op_err() {
    let i = Interner::new();
    let effect_then_err =
        "def f():\n    try:\n        return print(1) / 0\n    except Exception:\n        return 7\n";
    let plain_return = "def f():\n    return 7\n";
    assert_ne!(
        value_fp(&i, effect_then_err, Lang::Python),
        value_fp(&i, plain_return, Lang::Python),
        "observable left operand effects before a binary op error must not be discarded"
    );
}

#[test]
fn value_graph_runs_try_handler_after_static_index_base_err() {
    let i = Interner::new();
    let try_err =
        "def f():\n    try:\n        return (1 / 0)[print(1)]\n    except Exception:\n        return 7\n";
    let plain_return = "def f():\n    return 7\n";
    assert_eq!(
        value_fp(&i, try_err, Lang::Python),
        value_fp(&i, plain_return, Lang::Python),
        "a statically visible index base error should run the simple catch handler"
    );
}

#[test]
fn value_graph_runs_try_handler_after_static_field_receiver_err() {
    let i = Interner::new();
    let try_err =
        "def f():\n    try:\n        return (1 / 0).x\n    except Exception:\n        return 7\n";
    let plain_return = "def f():\n    return 7\n";
    assert_eq!(
        value_fp(&i, try_err, Lang::Python),
        value_fp(&i, plain_return, Lang::Python),
        "a statically visible field receiver error should run the simple catch handler"
    );
}

#[test]
fn value_graph_runs_try_handler_after_static_field_assignment_receiver_err() {
    let i = Interner::new();
    let try_err =
        "def f():\n    try:\n        (1 / 0).x = 7\n    except Exception:\n        return 7\n";
    let plain_return = "def f():\n    return 7\n";
    assert_eq!(
        value_fp(&i, try_err, Lang::Python),
        value_fp(&i, plain_return, Lang::Python),
        "a static field assignment receiver error should run the simple catch handler"
    );
}

#[test]
fn value_graph_keeps_try_index_base_effects_before_static_index_err() {
    let i = Interner::new();
    let effect_then_err =
        "def f():\n    try:\n        return print(1)[1 / 0]\n    except Exception:\n        return 7\n";
    let plain_return = "def f():\n    return 7\n";
    assert_ne!(
        value_fp(&i, effect_then_err, Lang::Python),
        value_fp(&i, plain_return, Lang::Python),
        "observable base effects before an index error must not be discarded"
    );
}

#[test]
fn value_graph_runs_try_handler_after_static_index_assignment_target_err() {
    let i = Interner::new();
    let try_err =
        "def f(xs):\n    try:\n        xs[1 / 0] = 2\n    except Exception:\n        return 7\n";
    let plain_return = "def f(xs):\n    return 7\n";
    assert_eq!(
        value_fp(&i, try_err, Lang::Python),
        value_fp(&i, plain_return, Lang::Python),
        "a static index assignment target error should run the simple catch handler"
    );
}

#[test]
fn value_graph_runs_try_handler_after_static_index_assignment_base_err() {
    let i = Interner::new();
    let try_err =
        "def f():\n    try:\n        (1 / 0)[print(1)] = 2\n    except Exception:\n        return 7\n";
    let plain_return = "def f():\n    return 7\n";
    assert_eq!(
        value_fp(&i, try_err, Lang::Python),
        value_fp(&i, plain_return, Lang::Python),
        "a static index assignment base error should run the simple catch handler before subscript effects"
    );
}

#[test]
fn value_graph_keeps_try_index_assignment_rhs_effects_before_target_err() {
    let i = Interner::new();
    let effect_then_err =
        "def f(xs):\n    try:\n        xs[1 / 0] = print(1)\n    except Exception:\n        return 7\n";
    let plain_return = "def f(xs):\n    return 7\n";
    assert_ne!(
        value_fp(&i, effect_then_err, Lang::Python),
        value_fp(&i, plain_return, Lang::Python),
        "observable RHS effects before an index assignment target error must not be discarded"
    );
}

#[test]
fn value_graph_runs_try_handler_after_static_seq_item_err() {
    let i = Interner::new();
    let try_err =
        "def f():\n    try:\n        return [1 / 0]\n    except Exception:\n        return 7\n";
    let plain_return = "def f():\n    return 7\n";
    assert_eq!(
        value_fp(&i, try_err, Lang::Python),
        value_fp(&i, plain_return, Lang::Python),
        "a statically visible sequence item error should run the simple catch handler"
    );
}

#[test]
fn value_graph_runs_try_handler_after_first_static_seq_item_err() {
    let i = Interner::new();
    let try_err =
        "def f():\n    try:\n        return [1 / 0, print(1)]\n    except Exception:\n        return 7\n";
    let plain_return = "def f():\n    return 7\n";
    assert_eq!(
        value_fp(&i, try_err, Lang::Python),
        value_fp(&i, plain_return, Lang::Python),
        "a first sequence item error should run the simple catch handler before later effects"
    );
}

#[test]
fn value_graph_keeps_try_seq_item_effects_before_static_err() {
    let i = Interner::new();
    let effect_then_err =
        "def f():\n    try:\n        return [print(1), 1 / 0]\n    except Exception:\n        return 7\n";
    let plain_return = "def f():\n    return 7\n";
    assert_ne!(
        value_fp(&i, effect_then_err, Lang::Python),
        value_fp(&i, plain_return, Lang::Python),
        "observable sequence item effects before an error must not be discarded"
    );
}

#[test]
fn value_graph_runs_try_handler_after_static_hof_lambda_err() {
    let i = Interner::new();
    let try_err = "def f():\n    try:\n        return [1 / 0 for x in [1]]\n    except Exception:\n        return 7\n";
    let plain_return = "def f():\n    return 7\n";
    assert_eq!(
        value_fp(&i, try_err, Lang::Python),
        value_fp(&i, plain_return, Lang::Python),
        "a statically visible HoF lambda error should run the simple catch handler"
    );
}

#[test]
fn value_graph_skips_try_handler_for_empty_static_hof_lambda_err() {
    let i = Interner::new();
    let empty_map =
        "def f():\n    try:\n        return [1 / 0 for x in []]\n    except Exception:\n        return 7\n";
    let plain_return = "def f():\n    return 7\n";
    assert_ne!(
        value_fp(&i, empty_map, Lang::Python),
        value_fp(&i, plain_return, Lang::Python),
        "a static lambda error is not observable when a known-empty collection skips it"
    );
}

#[test]
fn value_graph_runs_try_handler_after_static_reduce_lambda_err() {
    let i = Interner::new();
    let try_err =
        "def f():\n    try:\n        return reduce(lambda a, x: 1 / 0, [1], 0)\n    except Exception:\n        return 7\n";
    let plain_return = "def f():\n    return 7\n";
    assert_eq!(
        value_fp(&i, try_err, Lang::Python),
        value_fp(&i, plain_return, Lang::Python),
        "a statically visible reduce lambda error should run the simple catch handler"
    );
}

#[test]
fn value_graph_skips_try_handler_for_empty_static_reduce_lambda_err() {
    let i = Interner::new();
    let empty_reduce =
        "def f():\n    try:\n        return reduce(lambda a, x: 1 / 0, [], 0)\n    except Exception:\n        return 7\n";
    let plain_return = "def f():\n    return 7\n";
    assert_ne!(
        value_fp(&i, empty_reduce, Lang::Python),
        value_fp(&i, plain_return, Lang::Python),
        "a static reduce lambda error is not observable when a known-empty collection skips it"
    );
}

#[test]
fn value_graph_runs_try_handler_after_static_builtin_arg_err() {
    let i = Interner::new();
    let try_err =
        "def f():\n    try:\n        print(1 / 0)\n    except Exception:\n        return 7\n";
    let plain_return = "def f():\n    return 7\n";
    assert_eq!(
        value_fp(&i, try_err, Lang::Python),
        value_fp(&i, plain_return, Lang::Python),
        "a statically visible eager builtin argument error should run the simple catch handler"
    );
}

#[test]
fn value_graph_runs_try_handler_after_static_range_step_err() {
    let i = Interner::new();
    let try_err =
        "def f():\n    try:\n        return range(1, 5, 0)\n    except Exception:\n        return 7\n";
    let plain_return = "def f():\n    return 7\n";
    assert_eq!(
        value_fp(&i, try_err, Lang::Python),
        value_fp(&i, plain_return, Lang::Python),
        "a statically visible range zero-step error should run the simple catch handler"
    );
}

#[test]
fn value_graph_runs_try_handler_after_static_opaque_call_arg_err() {
    let i = Interner::new();
    let try_err =
        "def f():\n    try:\n        unknown(1 / 0)\n    except Exception:\n        return 7\n";
    let plain_return = "def f():\n    return 7\n";
    assert_eq!(
        value_fp(&i, try_err, Lang::Python),
        value_fp(&i, plain_return, Lang::Python),
        "a statically visible opaque call argument error should run the simple catch handler"
    );
}

#[test]
fn value_graph_keeps_try_opaque_call_arg_prefix_effects() {
    let i = Interner::new();
    let effect_then_err =
        "def f():\n    try:\n        unknown(print(1), 1 / 0)\n    except Exception:\n        return 7\n";
    let plain_return = "def f():\n    return 7\n";
    assert_ne!(
        value_fp(&i, effect_then_err, Lang::Python),
        value_fp(&i, plain_return, Lang::Python),
        "observable argument effects before a runtime error must not be discarded"
    );
}

#[test]
fn value_graph_keeps_try_static_expr_err_prefix_effects() {
    let i = Interner::new();
    let effect_then_err = "def f():\n    try:\n        print(1)\n        1 / 0\n    except Exception:\n        return 7\n";
    let plain_return = "def f():\n    return 7\n";
    assert_ne!(
        value_fp(&i, effect_then_err, Lang::Python),
        value_fp(&i, plain_return, Lang::Python),
        "observable effects before a runtime error must not be discarded with the try body"
    );
}

#[test]
fn value_graph_distinguishes_membership_and_negation() {
    // `in` is directional membership, not equality: `a in b` ≠ `b in a` ≠ `a == b`.
    // And `not in` / `is not` must keep their negation (`x is not None` ≢ `x is None`).
    let i = Interner::new();
    let inb = "def f(a, b):\n    return a in b\n";
    let bin = "def f(a, b):\n    return b in a\n";
    let eqb = "def f(a, b):\n    return a == b\n";
    assert_ne!(
        value_fp(&i, inb, Lang::Python),
        value_fp(&i, bin, Lang::Python),
        "a in b must differ from b in a (membership is directional)"
    );
    assert_ne!(
        value_fp(&i, inb, Lang::Python),
        value_fp(&i, eqb, Lang::Python),
        "a in b must differ from a == b"
    );
    let isn = "def f(a):\n    return a is None\n";
    let isnot = "def f(a):\n    return a is not None\n";
    assert_ne!(
        value_fp(&i, isn, Lang::Python),
        value_fp(&i, isnot, Lang::Python),
        "a is None must differ from a is not None (negation)"
    );
    let notin = "def f(a, b):\n    return a not in b\n";
    assert_ne!(
        value_fp(&i, inb, Lang::Python),
        value_fp(&i, notin, Lang::Python),
        "a in b must differ from a not in b (negation)"
    );
}

#[test]
fn map_key_membership_converges_cross_language_with_boundaries() {
    let i = Interner::new();
    let py = "def f(lookup, other_lookup, key, other):\n    return key in lookup\n";
    let py_method =
        "def f(lookup, other_lookup, key, other):\n    return lookup.__contains__(key)\n";
    let py_keys_in = "def f(lookup: dict[str, str], other_lookup: dict[str, str], key: str, other: str) -> bool:\n    return key in lookup.keys()\n";
    let py_keys_contains = "def f(lookup: dict[str, str], other_lookup: dict[str, str], key: str, other: str) -> bool:\n    return lookup.keys().__contains__(key)\n";
    let go = "package p\n\nfunc F(lookup map[string]string, otherLookup map[string]string, key string, other string) bool { _, ok := lookup[key]; return ok }\n";
    let java = "import java.util.Map;\n\nclass C { static boolean f(Map<String, String> lookup, Map<String, String> other_lookup, String key, String other) { return lookup.containsKey(key); } }\n";
    let java_key_set = "import java.util.Map;\n\nclass C { static boolean f(Map<String, String> lookup, Map<String, String> other_lookup, String key, String other) { return lookup.keySet().contains(key); } }\n";
    let rust = "use std::collections::HashMap;\n\npub fn f(lookup: &HashMap<String, String>, other_lookup: &HashMap<String, String>, key: &str, other: &str) -> bool { lookup.contains_key(key) }\n";
    let rust_get = "use std::collections::HashMap;\n\npub fn f(lookup: &HashMap<String, String>, other_lookup: &HashMap<String, String>, key: &str, other: &str) -> bool { lookup.get(key).is_some() }\n";
    let ruby = "def f(lookup, other_lookup, key, other)\n  lookup.key?(key)\nend\n";
    let ruby_has = "def f(lookup, other_lookup, key, other)\n  lookup.has_key?(key)\nend\n";
    let ts_array_from_keys = "function f(lookup: Map<string, string>, other_lookup: Map<string, string>, key: string, other: string): boolean { return Array.from(lookup.keys()).includes(key); }";
    let typed_set_same_names = "function f(lookup: Set<string>, other_lookup: Set<string>, key: string, other: string): boolean { return lookup.has(key); }";
    let wrong_key =
        "def f(lookup, other_lookup, key, other):\n    return lookup.__contains__(other)\n";
    let wrong_map =
        "def f(lookup, other_lookup, key, other):\n    return other_lookup.__contains__(key)\n";
    let value_membership =
        "def f(lookup, other_lookup, key, other):\n    return key in lookup.values()\n";
    let py_keys_wrong_key = "def f(lookup: dict[str, str], other_lookup: dict[str, str], key: str, other: str) -> bool:\n    return other in lookup.keys()\n";
    let py_keys_wrong_map = "def f(lookup: dict[str, str], other_lookup: dict[str, str], key: str, other: str) -> bool:\n    return key in other_lookup.keys()\n";
    let py_values_view = "def f(lookup: dict[str, str], other_lookup: dict[str, str], key: str, other: str) -> bool:\n    return key in lookup.values()\n";
    let ts_array_from_keys_wrong_key = "function f(lookup: Map<string, string>, other_lookup: Map<string, string>, key: string, other: string): boolean { return Array.from(lookup.keys()).includes(other); }";
    let ts_array_from_keys_wrong_map = "function f(lookup: Map<string, string>, other_lookup: Map<string, string>, key: string, other: string): boolean { return Array.from(other_lookup.keys()).includes(key); }";
    let ts_array_from_values = "function f(lookup: Map<string, string>, other_lookup: Map<string, string>, key: string, other: string): boolean { return Array.from(lookup.values()).includes(key); }";

    let fp = value_fp(&i, py, Lang::Python);
    assert_eq!(fp, value_fp(&i, py_method, Lang::Python));
    assert_eq!(fp, value_fp(&i, py_keys_in, Lang::Python));
    assert_eq!(fp, value_fp(&i, py_keys_contains, Lang::Python));
    assert_eq!(fp, value_fp(&i, go, Lang::Go));
    assert_eq!(fp, value_fp(&i, java, Lang::Java));
    assert_eq!(fp, value_fp(&i, java_key_set, Lang::Java));
    assert_eq!(fp, value_fp(&i, rust, Lang::Rust));
    assert_eq!(fp, value_fp(&i, rust_get, Lang::Rust));
    assert_eq!(fp, value_fp(&i, ruby, Lang::Ruby));
    assert_eq!(fp, value_fp(&i, ruby_has, Lang::Ruby));
    assert_eq!(fp, value_fp(&i, ts_array_from_keys, Lang::TypeScript));
    assert_ne!(fp, value_fp(&i, typed_set_same_names, Lang::TypeScript));
    assert_ne!(fp, value_fp(&i, wrong_key, Lang::Python));
    assert_ne!(fp, value_fp(&i, wrong_map, Lang::Python));
    assert_ne!(fp, value_fp(&i, value_membership, Lang::Python));
    assert_ne!(fp, value_fp(&i, py_keys_wrong_key, Lang::Python));
    assert_ne!(fp, value_fp(&i, py_keys_wrong_map, Lang::Python));
    assert_ne!(fp, value_fp(&i, py_values_view, Lang::Python));
    assert_ne!(
        fp,
        value_fp(&i, ts_array_from_keys_wrong_key, Lang::TypeScript)
    );
    assert_ne!(
        fp,
        value_fp(&i, ts_array_from_keys_wrong_map, Lang::TypeScript)
    );
    assert_ne!(fp, value_fp(&i, ts_array_from_values, Lang::TypeScript));
}

#[test]
fn import_named_and_namespace_member_coordinates_converge() {
    let i = Interner::new();
    let js_named = "import { helper } from \"./shared-math\";\nfunction f(value) { return helper(value + 1); }\n";
    let js_namespace = "import * as mathOps from \"./shared-math\";\nfunction f(value) { return mathOps.helper(value + 1); }\n";
    let js_wrong_member = "import * as mathOps from \"./shared-math\";\nfunction f(value) { return mathOps.otherHelper(value + 1); }\n";
    let ts_namespace = "import * as mathOps from \"./shared-math\";\nfunction f(value: number): number { return mathOps.helper(value + 1); }\n";
    let py_named =
        "from shared_math import helper\n\ndef f(value):\n    return helper(value + 1)\n";
    let py_namespace =
        "import shared_math as math_ops\n\ndef f(value):\n    return math_ops.helper(value + 1)\n";
    let py_wrong_member =
        "import shared_math as math_ops\n\ndef f(value):\n    return math_ops.other_helper(value + 1)\n";

    let fp = value_fp(&i, js_named, Lang::JavaScript);
    assert_eq!(fp, value_fp(&i, js_namespace, Lang::JavaScript));
    assert_eq!(fp, value_fp(&i, ts_namespace, Lang::TypeScript));
    assert_ne!(fp, value_fp(&i, js_wrong_member, Lang::JavaScript));

    let py_fp = value_fp(&i, py_named, Lang::Python);
    assert_eq!(py_fp, value_fp(&i, py_namespace, Lang::Python));
    assert_ne!(py_fp, value_fp(&i, py_wrong_member, Lang::Python));
}

#[test]
fn js_namespace_imports_ignore_parameter_shadow_mutations_only() {
    let i = Interner::new();
    let plain = r#"
import * as path from "node:path";

export const replaceRootDirInPath = (rootDir: string, filePath: string): string => {
  if (!filePath.startsWith("<rootDir>")) {
    return filePath;
  }

  return path.resolve(
    rootDir,
    path.normalize(`./${filePath.slice("<rootDir>".length)}`),
  );
};
"#;
    let shadowed_param = r#"
import * as path from "node:path";

export const escapeGlobCharacters = (path: string): string =>
  path.replaceAll(/([!()*?[\\\]{}])/g, "\\$1");

export const replaceRootDirInPath = (rootDir: string, filePath: string): string => {
  if (!filePath.startsWith("<rootDir>")) {
    return filePath;
  }

  return path.resolve(
    rootDir,
    path.normalize(`./${filePath.slice("<rootDir>".length)}`),
  );
};
"#;
    let unshadowed_mutation = r#"
import * as path from "node:path";

export const touchPath = (): void => {
  path.replaceAll("x", "y");
};

export const replaceRootDirInPath = (rootDir: string, filePath: string): string => {
  if (!filePath.startsWith("<rootDir>")) {
    return filePath;
  }

  return path.resolve(
    rootDir,
    path.normalize(`./${filePath.slice("<rootDir>".length)}`),
  );
};
"#;
    let fake_receiver = r#"
const path = {
  normalize(value: string): string {
    return value;
  },
  resolve(rootDir: string, value: string): string {
    return value;
  },
};

export const replaceRootDirInPath = (rootDir: string, filePath: string): string => {
  if (!filePath.startsWith("<rootDir>")) {
    return filePath;
  }

  return path.resolve(
    rootDir,
    path.normalize(`./${filePath.slice("<rootDir>".length)}`),
  );
};
"#;

    let fp = value_fp_named(&i, plain, Lang::TypeScript, "replaceRootDirInPath");
    assert_eq!(
        fp,
        value_fp_named(&i, shadowed_param, Lang::TypeScript, "replaceRootDirInPath"),
        "a parameter named like the namespace import must not taint the module binding"
    );
    assert_ne!(
        fp,
        value_fp_named(
            &i,
            unshadowed_mutation,
            Lang::TypeScript,
            "replaceRootDirInPath"
        ),
        "an unshadowed mutation-like receiver call must still block the import proof"
    );
    assert_ne!(
        fp,
        value_fp_named(&i, fake_receiver, Lang::TypeScript, "replaceRootDirInPath"),
        "a same-named local object is not a proven import namespace"
    );
}

#[test]
fn collection_membership_set_construction_converges_with_boundaries() {
    let i = Interner::new();
    let py_literal = "def f(value, other):\n    return value in [\"red\", \"blue\"]\n";
    let py_set_factory =
        "def f(value, other):\n    return set([\"red\", \"blue\"]).__contains__(value)\n";
    let py_tuple_factory =
        "def f(value, other):\n    return tuple([\"red\", \"blue\"]).__contains__(value)\n";
    let py_frozenset_factory =
        "def f(value, other):\n    return frozenset([\"red\", \"blue\"]).__contains__(value)\n";
    let py_deque_import = "from collections import deque\n\ndef f(value, other):\n    return deque([\"red\", \"blue\"]).__contains__(value)\n";
    let py_deque_alias = "from collections import deque as Values\n\ndef f(value, other):\n    return Values([\"red\", \"blue\"]).__contains__(value)\n";
    let py_deque_namespace = "import collections\n\ndef f(value, other):\n    return collections.deque([\"red\", \"blue\"]).__contains__(value)\n";
    let py_module_tuple =
        "VALUES = (\"red\", \"blue\")\n\ndef f(value, other):\n    return value in VALUES\n";
    let py_module_set =
        "VALUES = {\"red\", \"blue\"}\n\ndef f(value, other):\n    return value in VALUES\n";
    let js_set_inline =
        "function f(value, other) { return new Set([\"red\", \"blue\"]).has(value); }";
    let js_set_local = "function f(value, other) { const values = new Set([\"red\", \"blue\"]); return values.has(value); }";
    let js_module_set =
        "const VALUES = new Set([\"red\", \"blue\"]);\nfunction f(value, other) { return VALUES.has(value); }";
    let ts_module_set = "const VALUES = new Set<string>([\"red\", \"blue\"]);\nfunction f(value: string, other: string): boolean { return VALUES.has(value); }";
    let js_array_some =
        "function f(value, other) { return [\"red\", \"blue\"].some((item) => item === value); }";
    let ts_array_some = "function f(value: string, other: string): boolean { return [\"red\", \"blue\"].some((item: string) => item === value); }";
    let js_array_indexof_ne =
        "function f(value, other) { return [\"red\", \"blue\"].indexOf(value) !== -1; }";
    let ts_array_indexof_ge = "function f(value: string, other: string): boolean { return [\"red\", \"blue\"].indexOf(value) >= 0; }";
    let js_array_indexof_gt =
        "function f(value, other) { return [\"red\", \"blue\"].indexOf(value) > -1; }";
    let js_array_indexof_reversed =
        "function f(value, other) { return -1 < [\"red\", \"blue\"].indexOf(value); }";
    let js_array_findindex_ne = "function f(value, other) { return [\"red\", \"blue\"].findIndex((item) => item === value) !== -1; }";
    let ts_array_findindex_ge = "function f(value: string, other: string): boolean { return [\"red\", \"blue\"].findIndex((item: string) => item === value) >= 0; }";
    let js_array_findindex_gt = "function f(value, other) { return [\"red\", \"blue\"].findIndex((item) => item === value) > -1; }";
    let js_array_findindex_reversed =
        "function f(value, other) { return -1 < [\"red\", \"blue\"].findIndex((item) => item === value); }";
    let js_array_filter_length_ne = "function f(value, other) { return [\"red\", \"blue\"].filter((item) => item === value).length !== 0; }";
    let ts_array_filter_length_ge = "function f(value: string, other: string): boolean { return [\"red\", \"blue\"].filter((item: string) => item === value).length >= 1; }";
    let js_array_filter_length_gt = "function f(value, other) { return [\"red\", \"blue\"].filter((item) => item === value).length > 0; }";
    let js_array_filter_length_reversed = "function f(value, other) { return 0 < [\"red\", \"blue\"].filter((item) => item === value).length; }";
    let js_array_filter_length_absence_eq = "function f(value, other) { return [\"red\", \"blue\"].filter((item) => item === value).length === 0; }";
    let ts_array_filter_length_absence_le = "function f(value: string, other: string): boolean { return [\"red\", \"blue\"].filter((item: string) => item === value).length <= 0; }";
    let js_array_filter_length_absence_lt = "function f(value, other) { return [\"red\", \"blue\"].filter((item) => item === value).length < 1; }";
    let js_array_filter_length_absence_reversed = "function f(value, other) { return 1 > [\"red\", \"blue\"].filter((item) => item === value).length; }";
    let java_module_list = "import java.util.List;\n\nclass C { static final List<String> VALUES = List.of(\"red\", \"blue\"); static boolean f(String value, String other) { return VALUES.contains(value); } }";
    let ruby_member = "def f(value, other)\n  [\"red\", \"blue\"].member?(value)\nend\n";
    let ruby_set_new_include =
        "require \"set\"\n\ndef f(value, other)\n  Set.new([\"red\", \"blue\"]).include?(value)\nend\n";
    let ruby_set_new_member =
        "require \"set\"\n\ndef f(value, other)\n  Set.new([\"red\", \"blue\"]).member?(value)\nend\n";
    let ruby_set_local = "require \"set\"\n\ndef f(value, other)\n  values = Set.new([\"red\", \"blue\"])\n  values.include?(value)\nend\n";
    let js_wrong_element =
        "function f(value, other) { return new Set([\"red\", \"blue\"]).has(other); }";
    let js_wrong_collection =
        "function f(value, other) { return new Set([\"green\", \"blue\"]).has(value); }";
    let js_array_some_wrong_element =
        "function f(value, other) { return [\"red\", \"blue\"].some((item) => item === other); }";
    let js_array_some_wrong_collection =
        "function f(value, other) { return [\"green\", \"blue\"].some((item) => item === value); }";
    let js_array_indexof_wrong_element =
        "function f(value, other) { return [\"red\", \"blue\"].indexOf(other) !== -1; }";
    let js_array_indexof_wrong_collection =
        "function f(value, other) { return [\"green\", \"blue\"].indexOf(value) >= 0; }";
    let js_array_indexof_value =
        "function f(value, other) { return [\"red\", \"blue\"].indexOf(value); }";
    let js_array_findindex_wrong_element = "function f(value, other) { return [\"red\", \"blue\"].findIndex((item) => item === other) !== -1; }";
    let js_array_findindex_wrong_collection = "function f(value, other) { return [\"green\", \"blue\"].findIndex((item) => item === value) >= 0; }";
    let js_array_findindex_value =
        "function f(value, other) { return [\"red\", \"blue\"].findIndex((item) => item === value); }";
    let js_array_filter_length_wrong_element = "function f(value, other) { return [\"red\", \"blue\"].filter((item) => item === other).length !== 0; }";
    let js_array_filter_length_wrong_collection = "function f(value, other) { return [\"green\", \"blue\"].filter((item) => item === value).length >= 1; }";
    let js_array_filter_length_value =
        "function f(value, other) { return [\"red\", \"blue\"].filter((item) => item === value).length; }";
    let js_array_filter_length_zero = "function f(value, other) { return [\"red\", \"blue\"].filter((item) => item === value).length === 0; }";
    let js_array_filter_length_absence_wrong_element = "function f(value, other) { return [\"red\", \"blue\"].filter((item) => item === other).length === 0; }";
    let js_array_filter_length_absence_wrong_collection = "function f(value, other) { return [\"green\", \"blue\"].filter((item) => item === value).length <= 0; }";
    let js_nan_includes = "function f(value, other) { return [NaN].includes(value); }";
    let js_nan_some = "function f(value, other) { return [NaN].some((item) => item === value); }";
    let js_nan_indexof = "function f(value, other) { return [NaN].indexOf(value) !== -1; }";
    let js_nan_findindex =
        "function f(value, other) { return [NaN].findIndex((item) => item === value) !== -1; }";
    let js_nan_filter_length =
        "function f(value, other) { return [NaN].filter((item) => item === value).length > 0; }";
    let js_nan_filter_length_absence =
        "function f(value, other) { return [NaN].filter((item) => item === value).length === 0; }";
    let py_absence = "def f(value, other):\n    return value not in [\"red\", \"blue\"]\n";
    let js_not_includes =
        "function f(value, other) { return ![\"red\", \"blue\"].includes(value); }";
    let js_array_every_absence =
        "function f(value, other) { return [\"red\", \"blue\"].every((item) => item !== value); }";
    let ts_array_every_absence = "function f(value: string, other: string): boolean { return [\"red\", \"blue\"].every((item: string) => item !== value); }";
    let js_array_every_wrong_element =
        "function f(value, other) { return [\"red\", \"blue\"].every((item) => item !== other); }";
    let js_array_every_wrong_collection =
        "function f(value, other) { return [\"green\", \"blue\"].every((item) => item !== value); }";
    let js_nan_not_includes = "function f(value, other) { return ![NaN].includes(value); }";
    let js_nan_every = "function f(value, other) { return [NaN].every((item) => item !== value); }";
    let js_shadowed_set =
        "function f(Set, value, other) { return new Set([\"red\", \"blue\"]).has(value); }";
    let js_module_set_mutated = "const VALUES = new Set([\"red\", \"blue\"]);\nVALUES.add(\"green\");\nfunction f(value, other) { return VALUES.has(value); }";
    let ts_module_set_shadowed = "const Set: any = function(_values: any) { return { has: function() { return false; } }; };\nconst VALUES = new Set([\"red\", \"blue\"]);\nfunction f(value: string, other: string): boolean { return VALUES.has(value); }";
    let java_list_of = "import java.util.List;\n\nclass C { static boolean f(String value, String other) { return List.of(\"red\", \"blue\").contains(value); } }";
    let java_set_of = "import java.util.Set;\n\nclass C { static boolean f(String value, String other) { return Set.of(\"red\", \"blue\").contains(value); } }";
    let java_arrays_aslist = "import java.util.Arrays;\n\nclass C { static boolean f(String value, String other) { return Arrays.asList(\"red\", \"blue\").contains(value); } }";
    let go_slices_package = "package p\n\nimport \"slices\"\n\nvar values = []string{\"red\", \"blue\"}\n\nfunc F(value string, other string) bool { return slices.Contains(values, value) }\n";
    let go_slices_alias = "package p\n\nimport sl \"slices\"\n\nvar values = []string{\"red\", \"blue\"}\n\nfunc F(value string, other string) bool { return sl.Contains(values, value) }\n";
    let go_slices_const = "package p\n\nimport \"slices\"\n\nconst first = \"red\"\nvar values = []string{first, \"blue\"}\n\nfunc F(value string, other string) bool { return slices.Contains(values, value) }\n";
    let go_slices_local = "package p\n\nimport \"slices\"\n\nfunc F(value string, other string) bool {\n    values := []string{\"red\", \"blue\"}\n    return slices.Contains(values, value)\n}\n";
    let java_local_list = "import java.util.List;\n\nclass C { static boolean f(String value, String other) { var values = List.of(\"red\", \"blue\"); return values.contains(value); } }";
    let rust_local_array = "pub fn f(value: &str, other: &str) -> bool {\n    let values = [\"red\", \"blue\"];\n    values.contains(&value)\n}\n";
    let rust_local_typed_array = "pub fn f(value: &str, other: &str) -> bool {\n    let values: [&str; 2] = [\"red\", \"blue\"];\n    values.contains(&value)\n}\n";
    let rust_local_slice_ref = "pub fn f(value: &str, other: &str) -> bool {\n    let values: &[&str] = &[\"red\", \"blue\"];\n    values.contains(&value)\n}\n";
    let rust_local_vec = "pub fn f(value: &str, other: &str) -> bool {\n    let values = vec![\"red\", \"blue\"];\n    values.contains(&value)\n}\n";
    let rust_std_hashset = "pub fn f(value: &str, other: &str) -> bool {\n    let values = std::collections::HashSet::from([\"red\", \"blue\"]);\n    values.contains(&value)\n}\n";
    let rust_std_btreeset = "pub fn f(value: &str, other: &str) -> bool {\n    let values = std::collections::BTreeSet::from([\"red\", \"blue\"]);\n    values.contains(&value)\n}\n";
    let rust_std_vecdeque = "pub fn f(value: &str, other: &str) -> bool {\n    let values = std::collections::VecDeque::from([\"red\", \"blue\"]);\n    values.contains(&value)\n}\n";
    let java_wrong_element = "import java.util.List;\n\nclass C { static boolean f(String value, String other) { return List.of(\"red\", \"blue\").contains(other); } }";
    let java_wrong_collection = "import java.util.Set;\n\nclass C { static boolean f(String value, String other) { return Set.of(\"green\", \"blue\").contains(value); } }";
    let java_shadowed_list = "class C { static boolean f(Object List, String value, String other) { return List.of(\"red\", \"blue\").contains(value); } }";
    let java_local_list_class = "class C { static boolean f(String value, String other) { return List.of(\"red\", \"blue\").contains(value); } }\nclass List { static Box of(String a, String b) { return new Box(); } }\nclass Box { boolean contains(String value) { return false; } }";
    let java_module_list_shadowed = "class C { static final List<String> VALUES = List.of(\"red\", \"blue\"); static boolean f(String value, String other) { return VALUES.contains(value); } }\nclass List<T> { static java.util.List<String> of(String left, String right) { return java.util.List.of(\"green\", right); } }";
    let py_factory_wrong_element =
        "def f(value, other):\n    return set([\"red\", \"blue\"]).__contains__(other)\n";
    let py_factory_wrong_collection =
        "def f(value, other):\n    return set([\"green\", \"blue\"]).__contains__(value)\n";
    let py_factory_shadowed = "def f(value, other):\n    def set(_values):\n        class Box:\n            def __contains__(self, _value):\n                return False\n        return Box()\n    return set([\"red\", \"blue\"]).__contains__(value)\n";
    let py_deque_wrong_element = "from collections import deque\n\ndef f(value, other):\n    return deque([\"red\", \"blue\"]).__contains__(other)\n";
    let py_deque_wrong_collection = "from collections import deque\n\ndef f(value, other):\n    return deque([\"green\", \"blue\"]).__contains__(value)\n";
    let py_deque_missing_import =
        "def f(value, other):\n    return deque([\"red\", \"blue\"]).__contains__(value)\n";
    let py_deque_shadowed = "from collections import deque\n\ndef deque(_values):\n    class Box:\n        def __contains__(self, _value):\n            return False\n    return Box()\n\ndef f(value, other):\n    return deque([\"red\", \"blue\"]).__contains__(value)\n";
    let py_deque_mutated = "from collections import deque\n\ndef f(value, other):\n    values = deque([\"red\", \"blue\"])\n    values.append(\"green\")\n    return values.__contains__(value)\n";
    let py_module_mutated = "VALUES = [\"red\", \"blue\"]\nVALUES.append(\"green\")\n\ndef f(value, other):\n    return value in VALUES\n";
    let go_slices_wrong_element = "package p\n\nimport \"slices\"\n\nvar values = []string{\"red\", \"blue\"}\n\nfunc F(value string, other string) bool { return slices.Contains(values, other) }\n";
    let go_slices_wrong_collection = "package p\n\nimport \"slices\"\n\nvar values = []string{\"green\", \"blue\"}\n\nfunc F(value string, other string) bool { return slices.Contains(values, value) }\n";
    let go_slices_mutated = "package p\n\nimport \"slices\"\n\nvar values = append([]string{\"red\", \"blue\"}, \"green\")\n\nfunc F(value string, other string) bool { return slices.Contains(values, value) }\n";
    let go_slices_local_mutated = "package p\n\nimport \"slices\"\n\nfunc F(value string, other string) bool {\n    values := []string{\"red\", \"blue\"}\n    values = append(values, \"green\")\n    return slices.Contains(values, value)\n}\n";
    let go_slices_unimported = "package p\n\ntype fakeSlices struct{}\nfunc (fakeSlices) Contains(values []string, value string) bool { return false }\nvar slices fakeSlices\nvar values = []string{\"red\", \"blue\"}\n\nfunc F(value string, other string) bool { return slices.Contains(values, value) }\n";
    let java_local_list_mutated = "import java.util.ArrayList;\nimport java.util.List;\n\nclass C { static boolean f(String value, String other) { var values = new ArrayList<String>(List.of(\"red\", \"blue\")); values.add(\"green\"); return values.contains(value); } }";
    let rust_local_wrong_element = "pub fn f(value: &str, other: &str) -> bool {\n    let values = [\"red\", \"blue\"];\n    values.contains(&other)\n}\n";
    let rust_local_wrong_collection = "pub fn f(value: &str, other: &str) -> bool {\n    let values = [\"green\", \"blue\"];\n    values.contains(&value)\n}\n";
    let rust_local_mutated = "pub fn f(value: &str, other: &str) -> bool {\n    let mut values = vec![\"red\", \"blue\"];\n    values.push(\"green\");\n    values.contains(&value)\n}\n";
    let rust_local_custom_receiver = "struct Values;\nimpl Values { fn contains(&self, _value: &&str) -> bool { false } }\npub fn f(value: &str, other: &str) -> bool {\n    let values = Values;\n    values.contains(&value)\n}\n";
    let rust_std_wrong_element = "pub fn f(value: &str, other: &str) -> bool {\n    let values = std::collections::HashSet::from([\"red\", \"blue\"]);\n    values.contains(&other)\n}\n";
    let rust_std_wrong_collection = "pub fn f(value: &str, other: &str) -> bool {\n    let values = std::collections::BTreeSet::from([\"green\", \"blue\"]);\n    values.contains(&value)\n}\n";
    let rust_std_mutated = "pub fn f(value: &str, other: &str) -> bool {\n    let mut values = std::collections::HashSet::from([\"red\", \"blue\"]);\n    values.insert(\"green\");\n    values.contains(&value)\n}\n";
    let ruby_set_wrong_element =
        "require \"set\"\n\ndef f(value, other)\n  Set.new([\"red\", \"blue\"]).include?(other)\nend\n";
    let ruby_set_wrong_collection =
        "require \"set\"\n\ndef f(value, other)\n  Set.new([\"green\", \"blue\"]).include?(value)\nend\n";
    let ruby_set_missing_require =
        "def f(value, other)\n  Set.new([\"red\", \"blue\"]).include?(value)\nend\n";
    let ruby_set_shadowed = "require \"set\"\n\nclass Set\n  def self.new(_values)\n    Box.new\n  end\nend\n\nclass Box\n  def include?(_value)\n    false\n  end\nend\n\ndef f(value, other)\n  Set.new([\"red\", \"blue\"]).include?(value)\nend\n";
    let ruby_set_mutated = "require \"set\"\n\ndef f(value, other)\n  values = Set.new([\"red\", \"blue\"])\n  values.add(\"green\")\n  values.include?(value)\nend\n";

    let literal_fp = value_fp(&i, py_literal, Lang::Python);
    assert_eq!(literal_fp, value_fp(&i, py_set_factory, Lang::Python));
    assert_eq!(literal_fp, value_fp(&i, py_tuple_factory, Lang::Python));
    assert_eq!(literal_fp, value_fp(&i, py_frozenset_factory, Lang::Python));
    assert_eq!(literal_fp, value_fp(&i, py_deque_import, Lang::Python));
    assert_eq!(literal_fp, value_fp(&i, py_deque_alias, Lang::Python));
    assert_eq!(literal_fp, value_fp(&i, py_deque_namespace, Lang::Python));
    assert_eq!(literal_fp, value_fp(&i, py_module_tuple, Lang::Python));
    assert_eq!(literal_fp, value_fp(&i, py_module_set, Lang::Python));
    assert_eq!(literal_fp, value_fp(&i, js_set_inline, Lang::JavaScript));
    assert_eq!(literal_fp, value_fp(&i, js_set_local, Lang::JavaScript));
    assert_eq!(literal_fp, value_fp(&i, js_module_set, Lang::JavaScript));
    assert_eq!(literal_fp, value_fp(&i, ts_module_set, Lang::TypeScript));
    assert_eq!(literal_fp, value_fp(&i, js_array_some, Lang::JavaScript));
    assert_eq!(literal_fp, value_fp(&i, ts_array_some, Lang::TypeScript));
    assert_eq!(
        literal_fp,
        value_fp(&i, js_array_indexof_ne, Lang::JavaScript)
    );
    assert_eq!(
        literal_fp,
        value_fp(&i, ts_array_indexof_ge, Lang::TypeScript)
    );
    assert_eq!(
        literal_fp,
        value_fp(&i, js_array_indexof_gt, Lang::JavaScript)
    );
    assert_eq!(
        literal_fp,
        value_fp(&i, js_array_indexof_reversed, Lang::JavaScript)
    );
    assert_eq!(
        literal_fp,
        value_fp(&i, js_array_findindex_ne, Lang::JavaScript)
    );
    assert_eq!(
        literal_fp,
        value_fp(&i, ts_array_findindex_ge, Lang::TypeScript)
    );
    assert_eq!(
        literal_fp,
        value_fp(&i, js_array_findindex_gt, Lang::JavaScript)
    );
    assert_eq!(
        literal_fp,
        value_fp(&i, js_array_findindex_reversed, Lang::JavaScript)
    );
    assert_eq!(
        literal_fp,
        value_fp(&i, js_array_filter_length_ne, Lang::JavaScript)
    );
    assert_eq!(
        literal_fp,
        value_fp(&i, ts_array_filter_length_ge, Lang::TypeScript)
    );
    assert_eq!(
        literal_fp,
        value_fp(&i, js_array_filter_length_gt, Lang::JavaScript)
    );
    assert_eq!(
        literal_fp,
        value_fp(&i, js_array_filter_length_reversed, Lang::JavaScript)
    );
    assert_eq!(literal_fp, value_fp(&i, java_list_of, Lang::Java));
    assert_eq!(literal_fp, value_fp(&i, java_set_of, Lang::Java));
    assert_eq!(literal_fp, value_fp(&i, java_arrays_aslist, Lang::Java));
    assert_eq!(literal_fp, value_fp(&i, java_module_list, Lang::Java));
    assert_eq!(literal_fp, value_fp(&i, go_slices_package, Lang::Go));
    assert_eq!(literal_fp, value_fp(&i, go_slices_alias, Lang::Go));
    assert_eq!(literal_fp, value_fp(&i, go_slices_const, Lang::Go));
    assert_eq!(literal_fp, value_fp(&i, go_slices_local, Lang::Go));
    assert_eq!(literal_fp, value_fp(&i, java_local_list, Lang::Java));
    assert_eq!(literal_fp, value_fp(&i, rust_local_array, Lang::Rust));
    assert_eq!(literal_fp, value_fp(&i, rust_local_typed_array, Lang::Rust));
    assert_eq!(literal_fp, value_fp(&i, rust_local_slice_ref, Lang::Rust));
    assert_eq!(literal_fp, value_fp(&i, rust_local_vec, Lang::Rust));
    assert_eq!(literal_fp, value_fp(&i, rust_std_hashset, Lang::Rust));
    assert_eq!(literal_fp, value_fp(&i, rust_std_btreeset, Lang::Rust));
    assert_eq!(literal_fp, value_fp(&i, rust_std_vecdeque, Lang::Rust));
    assert_eq!(literal_fp, value_fp(&i, ruby_member, Lang::Ruby));
    assert_eq!(literal_fp, value_fp(&i, ruby_set_new_include, Lang::Ruby));
    assert_eq!(literal_fp, value_fp(&i, ruby_set_new_member, Lang::Ruby));
    assert_eq!(literal_fp, value_fp(&i, ruby_set_local, Lang::Ruby));
    assert_ne!(literal_fp, value_fp(&i, js_wrong_element, Lang::JavaScript));
    assert_ne!(
        literal_fp,
        value_fp(&i, js_wrong_collection, Lang::JavaScript)
    );
    assert_ne!(
        literal_fp,
        value_fp(&i, js_array_some_wrong_element, Lang::JavaScript)
    );
    assert_ne!(
        literal_fp,
        value_fp(&i, js_array_some_wrong_collection, Lang::JavaScript)
    );
    assert_ne!(
        literal_fp,
        value_fp(&i, js_array_indexof_wrong_element, Lang::JavaScript)
    );
    assert_ne!(
        literal_fp,
        value_fp(&i, js_array_indexof_wrong_collection, Lang::JavaScript)
    );
    assert_ne!(
        literal_fp,
        value_fp(&i, js_array_indexof_value, Lang::JavaScript)
    );
    assert_ne!(
        literal_fp,
        value_fp(&i, js_array_findindex_wrong_element, Lang::JavaScript)
    );
    assert_ne!(
        literal_fp,
        value_fp(&i, js_array_findindex_wrong_collection, Lang::JavaScript)
    );
    assert_ne!(
        literal_fp,
        value_fp(&i, js_array_findindex_value, Lang::JavaScript)
    );
    assert_ne!(
        literal_fp,
        value_fp(&i, js_array_filter_length_wrong_element, Lang::JavaScript)
    );
    assert_ne!(
        literal_fp,
        value_fp(
            &i,
            js_array_filter_length_wrong_collection,
            Lang::JavaScript
        )
    );
    assert_ne!(
        literal_fp,
        value_fp(&i, js_array_filter_length_value, Lang::JavaScript)
    );
    assert_ne!(
        literal_fp,
        value_fp(&i, js_array_filter_length_zero, Lang::JavaScript)
    );
    assert_ne!(
        value_fp(&i, js_nan_includes, Lang::JavaScript),
        value_fp(&i, js_nan_some, Lang::JavaScript)
    );
    assert_ne!(
        value_fp(&i, js_nan_includes, Lang::JavaScript),
        value_fp(&i, js_nan_indexof, Lang::JavaScript)
    );
    assert_ne!(
        value_fp(&i, js_nan_includes, Lang::JavaScript),
        value_fp(&i, js_nan_findindex, Lang::JavaScript)
    );
    assert_ne!(
        value_fp(&i, js_nan_includes, Lang::JavaScript),
        value_fp(&i, js_nan_filter_length, Lang::JavaScript)
    );
    assert_ne!(
        value_fp(&i, js_nan_not_includes, Lang::JavaScript),
        value_fp(&i, js_nan_filter_length_absence, Lang::JavaScript)
    );
    let absence_fp = value_fp(&i, py_absence, Lang::Python);
    assert_ne!(literal_fp, absence_fp);
    assert_eq!(absence_fp, value_fp(&i, js_not_includes, Lang::JavaScript));
    assert_eq!(
        absence_fp,
        value_fp(&i, js_array_every_absence, Lang::JavaScript)
    );
    assert_eq!(
        absence_fp,
        value_fp(&i, ts_array_every_absence, Lang::TypeScript)
    );
    assert_eq!(
        absence_fp,
        value_fp(&i, js_array_filter_length_absence_eq, Lang::JavaScript)
    );
    assert_eq!(
        absence_fp,
        value_fp(&i, ts_array_filter_length_absence_le, Lang::TypeScript)
    );
    assert_eq!(
        absence_fp,
        value_fp(&i, js_array_filter_length_absence_lt, Lang::JavaScript)
    );
    assert_eq!(
        absence_fp,
        value_fp(
            &i,
            js_array_filter_length_absence_reversed,
            Lang::JavaScript
        )
    );
    assert_ne!(
        absence_fp,
        value_fp(&i, js_array_every_wrong_element, Lang::JavaScript)
    );
    assert_ne!(
        absence_fp,
        value_fp(&i, js_array_every_wrong_collection, Lang::JavaScript)
    );
    assert_ne!(
        absence_fp,
        value_fp(
            &i,
            js_array_filter_length_absence_wrong_element,
            Lang::JavaScript
        )
    );
    assert_ne!(
        absence_fp,
        value_fp(
            &i,
            js_array_filter_length_absence_wrong_collection,
            Lang::JavaScript
        )
    );
    assert_ne!(
        value_fp(&i, js_nan_not_includes, Lang::JavaScript),
        value_fp(&i, js_nan_every, Lang::JavaScript)
    );
    assert_ne!(literal_fp, value_fp(&i, js_shadowed_set, Lang::JavaScript));
    assert_ne!(
        literal_fp,
        value_fp(&i, js_module_set_mutated, Lang::JavaScript)
    );
    assert_ne!(
        literal_fp,
        value_fp(&i, ts_module_set_shadowed, Lang::TypeScript)
    );
    assert_ne!(literal_fp, value_fp(&i, java_wrong_element, Lang::Java));
    assert_ne!(literal_fp, value_fp(&i, java_wrong_collection, Lang::Java));
    assert_ne!(literal_fp, value_fp(&i, java_shadowed_list, Lang::Java));
    assert_ne!(literal_fp, value_fp(&i, java_local_list_class, Lang::Java));
    assert_ne!(
        literal_fp,
        value_fp(&i, java_module_list_shadowed, Lang::Java)
    );
    assert_ne!(
        literal_fp,
        value_fp(&i, py_factory_wrong_element, Lang::Python)
    );
    assert_ne!(
        literal_fp,
        value_fp(&i, py_factory_wrong_collection, Lang::Python)
    );
    assert_ne!(literal_fp, value_fp(&i, py_factory_shadowed, Lang::Python));
    assert_ne!(
        literal_fp,
        value_fp(&i, py_deque_wrong_element, Lang::Python)
    );
    assert_ne!(
        literal_fp,
        value_fp(&i, py_deque_wrong_collection, Lang::Python)
    );
    assert_ne!(
        literal_fp,
        value_fp(&i, py_deque_missing_import, Lang::Python)
    );
    assert_ne!(
        literal_fp,
        value_fp_named(&i, py_deque_shadowed, Lang::Python, "f")
    );
    assert_ne!(literal_fp, value_fp(&i, py_deque_mutated, Lang::Python));
    assert_ne!(literal_fp, value_fp(&i, py_module_mutated, Lang::Python));
    assert_ne!(literal_fp, value_fp(&i, go_slices_wrong_element, Lang::Go));
    assert_ne!(
        literal_fp,
        value_fp(&i, go_slices_wrong_collection, Lang::Go)
    );
    assert_ne!(literal_fp, value_fp(&i, go_slices_mutated, Lang::Go));
    assert_ne!(literal_fp, value_fp(&i, go_slices_local_mutated, Lang::Go));
    assert_ne!(literal_fp, value_fp(&i, go_slices_unimported, Lang::Go));
    assert_ne!(
        literal_fp,
        value_fp(&i, java_local_list_mutated, Lang::Java)
    );
    assert_ne!(
        literal_fp,
        value_fp(&i, rust_local_wrong_element, Lang::Rust)
    );
    assert_ne!(
        literal_fp,
        value_fp(&i, rust_local_wrong_collection, Lang::Rust)
    );
    assert_ne!(literal_fp, value_fp(&i, rust_local_mutated, Lang::Rust));
    assert_ne!(
        literal_fp,
        value_fp(&i, rust_local_custom_receiver, Lang::Rust)
    );
    assert_ne!(literal_fp, value_fp(&i, rust_std_wrong_element, Lang::Rust));
    assert_ne!(
        literal_fp,
        value_fp(&i, rust_std_wrong_collection, Lang::Rust)
    );
    assert_ne!(literal_fp, value_fp(&i, rust_std_mutated, Lang::Rust));
    assert_ne!(literal_fp, value_fp(&i, ruby_set_wrong_element, Lang::Ruby));
    assert_ne!(
        literal_fp,
        value_fp(&i, ruby_set_wrong_collection, Lang::Ruby)
    );
    assert_ne!(
        literal_fp,
        value_fp(&i, ruby_set_missing_require, Lang::Ruby)
    );
    assert_ne!(literal_fp, value_fp(&i, ruby_set_shadowed, Lang::Ruby));
    assert_ne!(literal_fp, value_fp(&i, ruby_set_mutated, Lang::Ruby));

    let ts_array = "function f(values: string[], value: string, other: string): boolean { return values.includes(value); }";
    let ts_set = "function f(values: Set<string>, value: string, other: string): boolean { return values.has(value); }";
    let py_tuple =
        "def f(values: tuple[str, ...], value: str, other: str) -> bool:\n    return value in values\n";
    let py_alias_sequence = "from typing import Sequence as Values\n\ndef f(values: Values[str], value: str, other: str, other_values: Values[str]) -> bool:\n    return value in values\n";
    let py_alias_container = "from collections.abc import Container as Values\n\ndef f(values: Values[str], value: str, other: str, other_values: Values[str]) -> bool:\n    return value in values\n";
    let py_alias_set = "from typing import Set as Values\n\ndef f(values: Values[str], value: str, other: str, other_values: Values[str]) -> bool:\n    return value in values\n";
    let java_queue = "import java.util.Queue;\n\nclass C { static boolean f(Queue<String> values, String value, String other) { return values.contains(value); } }\n";
    let rust_vecdeque = "use std::collections::VecDeque;\n\npub fn f(values: &VecDeque<&str>, value: &str, other: &str) -> bool { values.contains(&value) }\n";
    let ts_untyped = "function f(values, value, other) { return values.has(value); }";
    let py_alias_wrong_element = "from typing import Sequence as Values\n\ndef f(values: Values[str], value: str, other: str, other_values: Values[str]) -> bool:\n    return other in values\n";
    let py_alias_wrong_receiver = "from typing import Sequence as Values\n\ndef f(values: Values[str], value: str, other: str, other_values: Values[str]) -> bool:\n    return value in other_values\n";
    let py_alias_unresolved = "def f(values: Values[str], value: str, other: str, other_values: Values[str]) -> bool:\n    return value in values\n";
    let py_alias_shadowed = "from typing import Sequence as Values\nValues = str\n\ndef f(values: Values[str], value: str, other: str, other_values: Values[str]) -> bool:\n    return value in values\n";
    let typed_fp = value_fp(&i, ts_array, Lang::TypeScript);
    assert_eq!(typed_fp, value_fp(&i, ts_set, Lang::TypeScript));
    assert_eq!(typed_fp, value_fp(&i, py_tuple, Lang::Python));
    assert_eq!(typed_fp, value_fp(&i, py_alias_sequence, Lang::Python));
    assert_eq!(typed_fp, value_fp(&i, py_alias_container, Lang::Python));
    assert_eq!(typed_fp, value_fp(&i, py_alias_set, Lang::Python));
    assert_eq!(typed_fp, value_fp(&i, java_queue, Lang::Java));
    assert_eq!(typed_fp, value_fp(&i, rust_vecdeque, Lang::Rust));
    assert_ne!(typed_fp, value_fp(&i, ts_untyped, Lang::TypeScript));
    assert_ne!(typed_fp, value_fp(&i, py_alias_wrong_element, Lang::Python));
    assert_ne!(
        typed_fp,
        value_fp(&i, py_alias_wrong_receiver, Lang::Python)
    );
    assert_ne!(typed_fp, value_fp(&i, py_alias_unresolved, Lang::Python));
    assert_ne!(typed_fp, value_fp(&i, py_alias_shadowed, Lang::Python));
}

#[test]
fn java_arrays_aslist_single_argument_respects_array_provenance() {
    let i = Interner::new();
    let array_membership = "import java.util.Arrays;\n\nclass C { static boolean f(String[] values, String value) { return Arrays.asList(values).contains(value); } }\n";
    let list_membership = "import java.util.Arrays;\nimport java.util.List;\n\nclass C { static boolean f(List<String> values, String value) { return Arrays.asList(values).contains(value); } }\n";
    let singleton_list_membership = "import java.util.List;\n\nclass C { static boolean f(String[] values, String value) { return List.of(values).contains(value); } }\n";

    let array_fp = value_fp(&i, array_membership, Lang::Java);
    assert_ne!(array_fp, value_fp(&i, list_membership, Lang::Java));
    assert_ne!(
        array_fp,
        value_fp(&i, singleton_list_membership, Lang::Java)
    );
}

#[test]
fn typed_empty_checks_keep_array_collection_and_string_domains_distinct() {
    let i = Interner::new();
    let java_list_size =
        "class C { static boolean f(java.util.List<Integer> values) { return values == null || values.size() == 0; } }\n";
    let java_list_named =
        "class C { static boolean f(java.util.List<Integer> values) { return values == null || values.isEmpty(); } }\n";
    let java_queue_named = "import java.util.Queue;\n\nclass C { static boolean f(Queue<String> values) { return values == null || values.isEmpty(); } }\n";
    let java_array_length =
        "class C { static boolean f(Object[] values) { return values == null || values.length == 0; } }\n";
    let java_string_named =
        "class C { static boolean f(String value) { return value == null || value.isEmpty(); } }\n";

    let list_fp = value_fp(&i, java_list_size, Lang::Java);
    assert_eq!(list_fp, value_fp(&i, java_list_named, Lang::Java));
    assert_eq!(list_fp, value_fp(&i, java_queue_named, Lang::Java));
    assert_ne!(list_fp, value_fp(&i, java_array_length, Lang::Java));
    assert_ne!(list_fp, value_fp(&i, java_string_named, Lang::Java));
    assert_ne!(
        value_fp(&i, java_array_length, Lang::Java),
        value_fp(&i, java_string_named, Lang::Java)
    );
}

#[test]
fn literal_map_default_lookup_converges_with_js_map_construction_boundaries() {
    let i = Interner::new();
    let py_literal = "def f(key, other):\n    return {\"red\": 1, \"blue\": 2}.get(key, 0)\n";
    let ruby_literal = "def f(key, other)\n  {\"red\" => 1, \"blue\" => 2}.fetch(key, 0)\nend\n";
    let js_inline =
        "function f(key, other) { return new Map([[\"red\", 1], [\"blue\", 2]]).get(key) ?? 0; }";
    let js_local = "function f(key, other) { const lookup = new Map([[\"red\", 1], [\"blue\", 2]]); return lookup.get(key) ?? 0; }";
    let js_has_get = "function f(key, other) { const lookup = new Map([[\"red\", 1], [\"blue\", 2]]); return lookup.has(key) ? lookup.get(key) : 0; }";
    let ts_inline = "function f(key: string, other: string): number { return new Map<string, number>([[\"red\", 1], [\"blue\", 2]]).get(key) ?? 0; }";
    let js_wrong_key =
        "function f(key, other) { return new Map([[\"red\", 1], [\"blue\", 2]]).get(other) ?? 0; }";
    let js_wrong_default =
        "function f(key, other) { return new Map([[\"red\", 1], [\"blue\", 2]]).get(key) ?? 9; }";
    let js_wrong_map =
        "function f(key, other) { return new Map([[\"red\", 9], [\"blue\", 2]]).get(key) ?? 0; }";
    let js_untyped = "function f(lookup, key, other) { return lookup.get(key) ?? 0; }";
    let js_shadowed_map = "function f(key, other, Map) { return new Map([[\"red\", 1], [\"blue\", 2]]).get(key) ?? 0; }";

    let fp = value_fp(&i, py_literal, Lang::Python);
    assert_eq!(fp, value_fp(&i, ruby_literal, Lang::Ruby));
    assert_eq!(fp, value_fp(&i, js_inline, Lang::JavaScript));
    assert_eq!(fp, value_fp(&i, js_local, Lang::JavaScript));
    assert_eq!(fp, value_fp(&i, js_has_get, Lang::JavaScript));
    assert_eq!(fp, value_fp(&i, ts_inline, Lang::TypeScript));
    assert_ne!(fp, value_fp(&i, js_wrong_key, Lang::JavaScript));
    assert_ne!(fp, value_fp(&i, js_wrong_default, Lang::JavaScript));
    assert_ne!(fp, value_fp(&i, js_wrong_map, Lang::JavaScript));
    assert_ne!(fp, value_fp(&i, js_untyped, Lang::JavaScript));
    assert_ne!(fp, value_fp(&i, js_shadowed_map, Lang::JavaScript));
}

#[test]
fn literal_map_default_lookup_converges_with_java_map_factory_boundaries() {
    let i = Interner::new();
    let py_literal = "def f(key, other):\n    return {\"red\": 1, \"blue\": 2}.get(key, 0)\n";
    let ruby_literal = "def f(key, other)\n  {\"red\" => 1, \"blue\" => 2}.fetch(key, 0)\nend\n";
    let java_map_of = "import java.util.Map;\n\nclass C { static int f(String key, String other) { return Map.of(\"red\", 1, \"blue\", 2).getOrDefault(key, 0); } }\n";
    let java_map_of_entries = "import java.util.Map;\n\nclass C { static int f(String key, String other) { return Map.ofEntries(Map.entry(\"red\", 1), Map.entry(\"blue\", 2)).getOrDefault(key, 0); } }\n";
    let java_map_local = "import java.util.Map;\n\nclass C { static int f(String key, String other) { Map<String, Integer> lookup = Map.of(\"red\", 1, \"blue\", 2); return lookup.getOrDefault(key, 0); } }\n";
    let java_wrong_key = "import java.util.Map;\n\nclass C { static int f(String key, String other) { return Map.of(\"red\", 1, \"blue\", 2).getOrDefault(other, 0); } }\n";
    let java_wrong_default = "import java.util.Map;\n\nclass C { static int f(String key, String other) { return Map.of(\"red\", 1, \"blue\", 2).getOrDefault(key, 9); } }\n";
    let java_wrong_map = "import java.util.Map;\n\nclass C { static int f(String key, String other) { return Map.of(\"red\", 9, \"blue\", 2).getOrDefault(key, 0); } }\n";
    let java_shadowed_factory = "class C { static class MapFactory { java.util.Map<String, Integer> of(Object... values) { return java.util.Map.of(); } } static int f(String key, String other, MapFactory Map) { return Map.of(\"red\", 1, \"blue\", 2).getOrDefault(key, 0); } }\n";
    let java_type_shadow = "class C { static int f(String key, String other) { return Map.of(\"red\", 1, \"blue\", 2).getOrDefault(key, 0); } }\nclass Map { static java.util.Map<String, Integer> of(Object... values) { return java.util.Map.of(); } }\n";

    let fp = value_fp(&i, py_literal, Lang::Python);
    assert_eq!(fp, value_fp(&i, ruby_literal, Lang::Ruby));
    assert_eq!(fp, value_fp(&i, java_map_of, Lang::Java));
    assert_eq!(fp, value_fp(&i, java_map_of_entries, Lang::Java));
    assert_eq!(fp, value_fp(&i, java_map_local, Lang::Java));
    assert_ne!(fp, value_fp(&i, java_wrong_key, Lang::Java));
    assert_ne!(fp, value_fp(&i, java_wrong_default, Lang::Java));
    assert_ne!(fp, value_fp(&i, java_wrong_map, Lang::Java));
    assert_ne!(fp, value_fp(&i, java_shadowed_factory, Lang::Java));
    assert_ne!(fp, value_fp(&i, java_type_shadow, Lang::Java));
}

#[test]
fn literal_map_default_lookup_converges_with_rust_std_map_factory_boundaries() {
    let i = Interner::new();
    let py_literal = "def f(key, other):\n    return {\"red\": 1, \"blue\": 2}.get(key, 0)\n";
    let rust_hashmap_inline = "pub fn f(key: &str, other: &str) -> i32 {\n    *std::collections::HashMap::from([(\"red\", 1), (\"blue\", 2)]).get(key).unwrap_or(&0)\n}\n";
    let rust_btreemap_inline = "pub fn f(key: &str, other: &str) -> i32 {\n    *std::collections::BTreeMap::from([(\"red\", 1), (\"blue\", 2)]).get(key).unwrap_or(&0)\n}\n";
    let rust_hashmap_local = "pub fn f(key: &str, other: &str) -> i32 {\n    let lookup = std::collections::HashMap::from([(\"red\", 1), (\"blue\", 2)]);\n    *lookup.get(key).unwrap_or(&0)\n}\n";
    let rust_wrong_key = "pub fn f(key: &str, other: &str) -> i32 {\n    *std::collections::HashMap::from([(\"red\", 1), (\"blue\", 2)]).get(other).unwrap_or(&0)\n}\n";
    let rust_wrong_default = "pub fn f(key: &str, other: &str) -> i32 {\n    *std::collections::HashMap::from([(\"red\", 1), (\"blue\", 2)]).get(key).unwrap_or(&9)\n}\n";
    let rust_wrong_map = "pub fn f(key: &str, other: &str) -> i32 {\n    *std::collections::HashMap::from([(\"red\", 9), (\"blue\", 2)]).get(key).unwrap_or(&0)\n}\n";
    let rust_mutated = "pub fn f(key: &str, other: &str) -> i32 {\n    let mut lookup = std::collections::HashMap::from([(\"red\", 1), (\"blue\", 2)]);\n    lookup.insert(\"red\", 9);\n    *lookup.get(key).unwrap_or(&0)\n}\n";

    let fp = value_fp(&i, py_literal, Lang::Python);
    assert_eq!(fp, value_fp(&i, rust_hashmap_inline, Lang::Rust));
    assert_eq!(fp, value_fp(&i, rust_btreemap_inline, Lang::Rust));
    assert_eq!(fp, value_fp(&i, rust_hashmap_local, Lang::Rust));
    assert_ne!(fp, value_fp(&i, rust_wrong_key, Lang::Rust));
    assert_ne!(fp, value_fp(&i, rust_wrong_default, Lang::Rust));
    assert_ne!(fp, value_fp(&i, rust_wrong_map, Lang::Rust));
    assert_ne!(fp, value_fp(&i, rust_mutated, Lang::Rust));
}

#[test]
fn literal_map_default_lookup_converges_with_go_literal_map_index_boundaries() {
    let i = Interner::new();
    let py_literal = "def f(key, other):\n    return {\"red\": 1, \"blue\": 2}.get(key, 0)\n";
    let ruby_literal = "def f(key, other)\n  {\"red\" => 1, \"blue\" => 2}.fetch(key, 0)\nend\n";
    let go_inline = "package p\n\nfunc F(key string, other string) int { return map[string]int{\"red\": 1, \"blue\": 2}[key] }\n";
    let go_local = "package p\n\nfunc F(key string, other string) int { lookup := map[string]int{\"red\": 1, \"blue\": 2}; return lookup[key] }\n";
    let go_var = "package p\n\nfunc F(key string, other string) int { var lookup = map[string]int{\"red\": 1, \"blue\": 2}; return lookup[key] }\n";
    let go_wrong_key =
        "package p\n\nfunc F(key string, other string) int { return map[string]int{\"red\": 1, \"blue\": 2}[other] }\n";
    let go_wrong_map =
        "package p\n\nfunc F(key string, other string) int { return map[string]int{\"red\": 9, \"blue\": 2}[key] }\n";
    let py_int_key_literal = "def f(key, other):\n    return {0: 1, 1: 2}.get(key, 0)\n";
    let go_keyed_slice =
        "package p\n\nfunc F(key int, other int) int { return []int{0: 1, 1: 2}[key] }\n";
    let py_string_literal =
        "def f(key, other):\n    return {\"red\": \"apple\", \"blue\": \"berry\"}.get(key, \"\")\n";
    let ruby_string_literal =
        "def f(key, other)\n  {\"red\" => \"apple\", \"blue\" => \"berry\"}.fetch(key, \"\")\nend\n";
    let go_string_inline =
        "package p\n\nfunc F(key string, other string) string { return map[string]string{\"red\": \"apple\", \"blue\": \"berry\"}[key] }\n";
    let go_string_local =
        "package p\n\nfunc F(key string, other string) string { lookup := map[string]string{\"red\": \"apple\", \"blue\": \"berry\"}; return lookup[key] }\n";
    let go_string_wrong_key =
        "package p\n\nfunc F(key string, other string) string { return map[string]string{\"red\": \"apple\", \"blue\": \"berry\"}[other] }\n";
    let py_string_int_key_literal =
        "def f(key, other):\n    return {0: \"apple\", 1: \"berry\"}.get(key, \"\")\n";
    let go_string_keyed_slice =
        "package p\n\nfunc F(key int, other int) string { return []string{0: \"apple\", 1: \"berry\"}[key] }\n";
    let py_bool_literal =
        "def f(key, other):\n    return {\"red\": True, \"blue\": False}.get(key, False)\n";
    let ruby_bool_literal =
        "def f(key, other)\n  {\"red\" => true, \"blue\" => false}.fetch(key, false)\nend\n";
    let go_bool_inline =
        "package p\n\nfunc F(key string, other string) bool { return map[string]bool{\"red\": true, \"blue\": false}[key] }\n";
    let go_bool_wrong_map =
        "package p\n\nfunc F(key string, other string) bool { return map[string]bool{\"red\": false, \"blue\": false}[key] }\n";
    let py_float_literal =
        "def f(key, other):\n    return {\"red\": 1.5, \"blue\": 2.5}.get(key, 0.0)\n";
    let ruby_float_literal =
        "def f(key, other)\n  {\"red\" => 1.5, \"blue\" => 2.5}.fetch(key, 0.0)\nend\n";
    let go_float_inline =
        "package p\n\nfunc F(key string, other string) float64 { return map[string]float64{\"red\": 1.5, \"blue\": 2.5}[key] }\n";
    let go_float_local =
        "package p\n\nfunc F(key string, other string) float64 { lookup := map[string]float64{\"red\": 1.5, \"blue\": 2.5}; return lookup[key] }\n";
    let go_float_wrong_key =
        "package p\n\nfunc F(key string, other string) float64 { return map[string]float64{\"red\": 1.5, \"blue\": 2.5}[other] }\n";
    let py_nil_literal =
        "def f(key, other):\n    return {\"red\": None, \"blue\": None}.get(key, None)\n";
    let ruby_nil_literal =
        "def f(key, other)\n  {\"red\" => nil, \"blue\" => nil}.fetch(key, nil)\nend\n";
    let go_nil_inline =
        "package p\n\ntype Item struct{}\n\nfunc F(key string, other string) *Item { return map[string]*Item{\"red\": nil, \"blue\": nil}[key] }\n";
    let go_nil_wrong_map =
        "package p\n\nfunc F(key string, other string) string { return map[string]string{\"red\": \"apple\", \"blue\": \"berry\"}[key] }\n";
    let go_mixed_value =
        "package p\n\nfunc F(key string, other string) interface{} { return map[string]interface{}{\"red\": \"apple\", \"blue\": false}[key] }\n";

    let fp = value_fp(&i, py_literal, Lang::Python);
    assert_eq!(fp, value_fp(&i, ruby_literal, Lang::Ruby));
    assert_eq!(fp, value_fp(&i, go_inline, Lang::Go));
    assert_eq!(fp, value_fp(&i, go_local, Lang::Go));
    assert_eq!(fp, value_fp(&i, go_var, Lang::Go));
    assert_ne!(fp, value_fp(&i, go_wrong_key, Lang::Go));
    assert_ne!(fp, value_fp(&i, go_wrong_map, Lang::Go));
    assert_ne!(
        value_fp(&i, py_int_key_literal, Lang::Python),
        value_fp(&i, go_keyed_slice, Lang::Go)
    );
    assert_ne!(fp, value_fp(&i, go_string_inline, Lang::Go));

    let string_fp = value_fp(&i, py_string_literal, Lang::Python);
    assert_eq!(string_fp, value_fp(&i, ruby_string_literal, Lang::Ruby));
    assert_eq!(string_fp, value_fp(&i, go_string_inline, Lang::Go));
    assert_eq!(string_fp, value_fp(&i, go_string_local, Lang::Go));
    assert_ne!(string_fp, value_fp(&i, go_string_wrong_key, Lang::Go));
    assert_ne!(string_fp, value_fp(&i, go_mixed_value, Lang::Go));
    assert_ne!(
        value_fp(&i, py_string_int_key_literal, Lang::Python),
        value_fp(&i, go_string_keyed_slice, Lang::Go)
    );

    let bool_fp = value_fp(&i, py_bool_literal, Lang::Python);
    assert_eq!(bool_fp, value_fp(&i, ruby_bool_literal, Lang::Ruby));
    assert_eq!(bool_fp, value_fp(&i, go_bool_inline, Lang::Go));
    assert_ne!(bool_fp, value_fp(&i, go_bool_wrong_map, Lang::Go));

    let float_fp = value_fp(&i, py_float_literal, Lang::Python);
    assert_eq!(float_fp, value_fp(&i, ruby_float_literal, Lang::Ruby));
    assert_eq!(float_fp, value_fp(&i, go_float_inline, Lang::Go));
    assert_eq!(float_fp, value_fp(&i, go_float_local, Lang::Go));
    assert_ne!(float_fp, value_fp(&i, go_float_wrong_key, Lang::Go));

    let nil_fp = value_fp(&i, py_nil_literal, Lang::Python);
    assert_eq!(nil_fp, value_fp(&i, ruby_nil_literal, Lang::Ruby));
    assert_eq!(nil_fp, value_fp(&i, go_nil_inline, Lang::Go));
    assert_ne!(nil_fp, value_fp(&i, go_nil_wrong_map, Lang::Go));
}

#[test]
fn literal_map_default_lookup_converges_with_module_map_bindings() {
    let i = Interner::new();
    let py_literal = "def f(key, other):\n    return {\"red\": 1, \"blue\": 2}.get(key, 0)\n";
    let js_module = "const LOOKUP = new Map([[\"red\", 1], [\"blue\", 2]]);\nfunction f(key, other) { return LOOKUP.get(key) ?? 0; }\n";
    let ts_module = "const LOOKUP = new Map<string, number>([[\"red\", 1], [\"blue\", 2]]);\nfunction f(key: string, other: string): number { return LOOKUP.get(key) ?? 0; }\n";
    let java_static = "import java.util.Map;\n\nclass C { static final Map<String, Integer> LOOKUP = Map.of(\"red\", 1, \"blue\", 2); static int f(String key, String other) { return LOOKUP.getOrDefault(key, 0); } }\n";
    let js_wrong_key = "const LOOKUP = new Map([[\"red\", 1], [\"blue\", 2]]);\nfunction f(key, other) { return LOOKUP.get(other) ?? 0; }\n";
    let ts_wrong_default = "const LOOKUP = new Map<string, number>([[\"red\", 1], [\"blue\", 2]]);\nfunction f(key: string, other: string): number { return LOOKUP.get(key) ?? 9; }\n";
    let java_wrong_map = "import java.util.Map;\n\nclass C { static final Map<String, Integer> LOOKUP = Map.of(\"red\", 9, \"blue\", 2); static int f(String key, String other) { return LOOKUP.getOrDefault(key, 0); } }\n";
    let js_mutated = "const LOOKUP = new Map([[\"red\", 1], [\"blue\", 2]]);\nLOOKUP.set(\"red\", 9);\nfunction f(key, other) { return LOOKUP.get(key) ?? 0; }\n";
    let ts_shadowed = "const Map: any = function(_entries: any) { return { get: function() { return 9; } }; };\nconst LOOKUP = new Map([[\"red\", 1], [\"blue\", 2]]);\nfunction f(key: string, other: string): number { return LOOKUP.get(key) ?? 0; }\n";
    let java_shadowed = "class C { static final Map<String, Integer> LOOKUP = Map.of(\"red\", 1, \"blue\", 2); static int f(String key, String other) { return LOOKUP.getOrDefault(key, 0); } }\nclass Map { static java.util.Map<String, Integer> of(Object... values) { return java.util.Map.of(); } }\n";

    let fp = value_fp(&i, py_literal, Lang::Python);
    assert_eq!(fp, value_fp(&i, js_module, Lang::JavaScript));
    assert_eq!(fp, value_fp(&i, ts_module, Lang::TypeScript));
    assert_eq!(fp, value_fp(&i, java_static, Lang::Java));
    assert_ne!(fp, value_fp(&i, js_wrong_key, Lang::JavaScript));
    assert_ne!(fp, value_fp(&i, ts_wrong_default, Lang::TypeScript));
    assert_ne!(fp, value_fp(&i, java_wrong_map, Lang::Java));
    assert_ne!(fp, value_fp(&i, js_mutated, Lang::JavaScript));
    assert_ne!(fp, value_fp(&i, ts_shadowed, Lang::TypeScript));
    assert_ne!(fp, value_fp(&i, java_shadowed, Lang::Java));
}

#[test]
fn literal_map_default_lookup_converges_with_js_object_own_property_boundaries() {
    let i = Interner::new();
    let py_literal = "def f(key, other):\n    return {\"red\": 1, \"blue\": 2}.get(key, 0)\n";
    let ruby_literal = "def f(key, other)\n  {\"red\" => 1, \"blue\" => 2}.fetch(key, 0)\nend\n";
    let js_hasown = "function f(key, other) { const values = { \"red\": 1, \"blue\": 2 }; return Object.hasOwn(values, key) ? values[key] : 0; }";
    let js_call = "function f(key, other) { const values = { \"red\": 1, \"blue\": 2 }; return Object.prototype.hasOwnProperty.call(values, key) ? values[key] : 0; }";
    let ts_negated = "function f(key: string, other: string): number { const values: Record<string, number> = { \"red\": 1, \"blue\": 2 }; return !Object.hasOwn(values, key) ? 0 : values[key]; }";
    let js_wrong_key = "function f(key, other) { const values = { \"red\": 1, \"blue\": 2 }; return Object.hasOwn(values, other) ? values[other] : 0; }";
    let js_wrong_default = "function f(key, other) { const values = { \"red\": 1, \"blue\": 2 }; return Object.hasOwn(values, key) ? values[key] : 9; }";
    let js_wrong_map = "function f(key, other) { const values = { \"red\": 9, \"blue\": 2 }; return Object.hasOwn(values, key) ? values[key] : 0; }";
    let js_unguarded = "function f(key, other) { const values = { \"red\": 1, \"blue\": 2 }; return values[key] ?? 0; }";
    let js_in = "function f(key, other) { const values = { \"red\": 1, \"blue\": 2 }; return key in values ? values[key] : 0; }";
    let js_method = "function f(key, other) { const values = { \"red\": 1, \"blue\": 2 }; return values.hasOwnProperty(key) ? values[key] : 0; }";
    let js_shadowed_object = "function f(key, other, Object) { const values = { \"red\": 1, \"blue\": 2 }; return Object.hasOwn(values, key) ? values[key] : 0; }";

    let fp = value_fp(&i, py_literal, Lang::Python);
    assert_eq!(fp, value_fp(&i, ruby_literal, Lang::Ruby));
    assert_eq!(fp, value_fp(&i, js_hasown, Lang::JavaScript));
    assert_eq!(fp, value_fp(&i, js_call, Lang::JavaScript));
    assert_eq!(fp, value_fp(&i, ts_negated, Lang::TypeScript));
    assert_ne!(fp, value_fp(&i, js_wrong_key, Lang::JavaScript));
    assert_ne!(fp, value_fp(&i, js_wrong_default, Lang::JavaScript));
    assert_ne!(fp, value_fp(&i, js_wrong_map, Lang::JavaScript));
    assert_ne!(fp, value_fp(&i, js_unguarded, Lang::JavaScript));
    assert_ne!(fp, value_fp(&i, js_in, Lang::JavaScript));
    assert_ne!(fp, value_fp(&i, js_method, Lang::JavaScript));
    assert_ne!(fp, value_fp(&i, js_shadowed_object, Lang::JavaScript));
}

#[test]
fn map_default_lookup_converges_cross_language_with_boundaries() {
    let i = Interner::new();
    let go = "package p\n\nfunc F(lookup map[string]int, otherLookup map[string]int, key string, otherKey string, fallback int, otherDefault int) int { value, ok := lookup[key]; if !ok { value = fallback }; return value }\n";
    let java_explicit = "import java.util.Map;\n\nclass C { static int f(Map<String, Integer> lookup, Map<String, Integer> other_lookup, String key, String other_key, int fallback, int other_default) { return lookup.containsKey(key) ? lookup.get(key) : fallback; } }\n";
    let java_builtin = "import java.util.Map;\n\nclass C { static int f(Map<String, Integer> lookup, Map<String, Integer> other_lookup, String key, String other_key, int fallback, int other_default) { return lookup.getOrDefault(key, fallback); } }\n";
    let java_guard_return = "import java.util.Map;\n\nclass C { static int f(Map<String, Integer> lookup, Map<String, Integer> other_lookup, String key, String other_key, int fallback, int other_default) { if (lookup.containsKey(key)) { return lookup.get(key); } return fallback; } }\n";
    let rust_explicit = "use std::collections::HashMap;\n\npub fn f(lookup: &HashMap<&str, i32>, other_lookup: &HashMap<&str, i32>, key: &str, other_key: &str, fallback: i32, other_default: i32) -> i32 { if lookup.contains_key(key) { lookup[key] } else { fallback } }\n";
    let rust_unwrap = "use std::collections::HashMap;\n\npub fn f(lookup: &HashMap<&str, i32>, other_lookup: &HashMap<&str, i32>, key: &str, other_key: &str, fallback: i32, other_default: i32) -> i32 { *lookup.get(key).unwrap_or(&fallback) }\n";
    let ts_nullish = "function f(lookup: Map<string, number>, other_lookup: Map<string, number>, key: string, other_key: string, fallback: number, other_default: number): number { return lookup.get(key) ?? fallback; }\n";
    let ts_has_get = "function f(lookup: Map<string, number>, other_lookup: Map<string, number>, key: string, other_key: string, fallback: number, other_default: number): number { return lookup.has(key) ? lookup.get(key) : fallback; }\n";
    let ts_temp_guard = "function f(lookup: Map<string, number>, other_lookup: Map<string, number>, key: string, other_key: string, fallback: number, other_default: number): number { const selected = lookup.get(key); return selected === undefined ? fallback : selected; }\n";
    let ts_guard_return = "function f(lookup: Map<string, number>, other_lookup: Map<string, number>, key: string, other_key: string, fallback: number, other_default: number): number { if (lookup.has(key)) { return lookup.get(key)!; } return fallback; }\n";
    let py_dict = "def f(lookup: dict[str, int], other_lookup: dict[str, int], key: str, other_key: str, fallback: int, other_default: int) -> int:\n    return lookup.get(key, fallback)\n";
    let py_guard_return = "def f(lookup: dict[str, int], other_lookup: dict[str, int], key: str, other_key: str, fallback: int, other_default: int) -> int:\n    if key in lookup:\n        return lookup[key]\n    return fallback\n";
    let py_mapping = "from collections.abc import Mapping\n\ndef f(lookup: Mapping[str, int], other_lookup: Mapping[str, int], key: str, other_key: str, fallback: int, other_default: int) -> int:\n    return lookup.get(key, fallback)\n";
    let py_mutable_mapping = "from collections.abc import MutableMapping\n\ndef f(lookup: MutableMapping[str, int], other_lookup: MutableMapping[str, int], key: str, other_key: str, fallback: int, other_default: int) -> int:\n    return lookup.get(key, fallback)\n";
    let py_alias_mapping = "from collections.abc import Mapping as MapLike\n\ndef f(lookup: MapLike[str, int], other_lookup: MapLike[str, int], key: str, other_key: str, fallback: int, other_default: int) -> int:\n    return lookup.get(key, fallback)\n";
    let py_alias_mutable_mapping = "from collections.abc import MutableMapping as MapLike\n\ndef f(lookup: MapLike[str, int], other_lookup: MapLike[str, int], key: str, other_key: str, fallback: int, other_default: int) -> int:\n    return lookup.get(key, fallback)\n";
    let py_alias_dict = "from typing import Dict as MapLike\n\ndef f(lookup: MapLike[str, int], other_lookup: MapLike[str, int], key: str, other_key: str, fallback: int, other_default: int) -> int:\n    return lookup.get(key, fallback)\n";
    let wrong_key = "import java.util.Map;\n\nclass C { static int f(Map<String, Integer> lookup, Map<String, Integer> other_lookup, String key, String other_key, int fallback, int other_default) { return lookup.getOrDefault(other_key, fallback); } }\n";
    let wrong_default = "use std::collections::HashMap;\n\npub fn f(lookup: &HashMap<&str, i32>, other_lookup: &HashMap<&str, i32>, key: &str, other_key: &str, fallback: i32, other_default: i32) -> i32 { *lookup.get(key).unwrap_or(&other_default) }\n";
    let wrong_map = "package p\n\nfunc F(lookup map[string]int, otherLookup map[string]int, key string, otherKey string, fallback int, otherDefault int) int { value, ok := otherLookup[key]; if !ok { value = fallback }; return value }\n";
    let ts_wrong_key = "function f(lookup: Map<string, number>, other_lookup: Map<string, number>, key: string, other_key: string, fallback: number, other_default: number): number { return lookup.get(other_key) ?? fallback; }\n";
    let ts_wrong_default = "function f(lookup: Map<string, number>, other_lookup: Map<string, number>, key: string, other_key: string, fallback: number, other_default: number): number { return lookup.get(key) ?? other_default; }\n";
    let ts_wrong_map = "function f(lookup: Map<string, number>, other_lookup: Map<string, number>, key: string, other_key: string, fallback: number, other_default: number): number { return other_lookup.get(key) ?? fallback; }\n";
    let ts_untyped = "function f(lookup, other_lookup, key, other_key, fallback, other_default) { return lookup.get(key) ?? fallback; }\n";
    let py_wrong_key = "def f(lookup: dict[str, int], other_lookup: dict[str, int], key: str, other_key: str, fallback: int, other_default: int) -> int:\n    return lookup.get(other_key, fallback)\n";
    let py_wrong_default = "def f(lookup: dict[str, int], other_lookup: dict[str, int], key: str, other_key: str, fallback: int, other_default: int) -> int:\n    return lookup.get(key, other_default)\n";
    let py_wrong_map = "def f(lookup: dict[str, int], other_lookup: dict[str, int], key: str, other_key: str, fallback: int, other_default: int) -> int:\n    return other_lookup.get(key, fallback)\n";
    let py_untyped = "def f(lookup, other_lookup, key, other_key, fallback, other_default):\n    return lookup.get(key, fallback)\n";
    let py_alias_wrong_key = "from collections.abc import Mapping as MapLike\n\ndef f(lookup: MapLike[str, int], other_lookup: MapLike[str, int], key: str, other_key: str, fallback: int, other_default: int) -> int:\n    return lookup.get(other_key, fallback)\n";
    let py_alias_wrong_default = "from collections.abc import Mapping as MapLike\n\ndef f(lookup: MapLike[str, int], other_lookup: MapLike[str, int], key: str, other_key: str, fallback: int, other_default: int) -> int:\n    return lookup.get(key, other_default)\n";
    let py_alias_wrong_map = "from collections.abc import Mapping as MapLike\n\ndef f(lookup: MapLike[str, int], other_lookup: MapLike[str, int], key: str, other_key: str, fallback: int, other_default: int) -> int:\n    return other_lookup.get(key, fallback)\n";
    let py_alias_unresolved = "def f(lookup: MapLike[str, int], other_lookup: MapLike[str, int], key: str, other_key: str, fallback: int, other_default: int) -> int:\n    return lookup.get(key, fallback)\n";
    let py_alias_shadowed = "from collections.abc import Mapping as MapLike\nMapLike = list\n\ndef f(lookup: MapLike[str, int], other_lookup: MapLike[str, int], key: str, other_key: str, fallback: int, other_default: int) -> int:\n    return lookup.get(key, fallback)\n";
    let guard_wrong_key = "function f(lookup: Map<string, number>, other_lookup: Map<string, number>, key: string, other_key: string, fallback: number, other_default: number): number { if (lookup.has(other_key)) { return lookup.get(other_key)!; } return fallback; }\n";
    let guard_wrong_default = "import java.util.Map;\n\nclass C { static int f(Map<String, Integer> lookup, Map<String, Integer> other_lookup, String key, String other_key, int fallback, int other_default) { if (lookup.containsKey(key)) { return lookup.get(key); } return other_default; } }\n";
    let guard_wrong_map = "def f(lookup: dict[str, int], other_lookup: dict[str, int], key: str, other_key: str, fallback: int, other_default: int) -> int:\n    if key in other_lookup:\n        return other_lookup[key]\n    return fallback\n";

    let fp = value_fp(&i, go, Lang::Go);
    assert_eq!(fp, value_fp(&i, java_explicit, Lang::Java));
    assert_eq!(fp, value_fp(&i, java_builtin, Lang::Java));
    assert_eq!(fp, value_fp(&i, java_guard_return, Lang::Java));
    assert_eq!(fp, value_fp(&i, rust_explicit, Lang::Rust));
    assert_eq!(fp, value_fp(&i, rust_unwrap, Lang::Rust));
    assert_eq!(fp, value_fp(&i, ts_nullish, Lang::TypeScript));
    assert_eq!(fp, value_fp(&i, ts_has_get, Lang::TypeScript));
    assert_eq!(fp, value_fp(&i, ts_temp_guard, Lang::TypeScript));
    assert_eq!(fp, value_fp(&i, ts_guard_return, Lang::TypeScript));
    assert_eq!(fp, value_fp(&i, py_dict, Lang::Python));
    assert_eq!(fp, value_fp(&i, py_guard_return, Lang::Python));
    assert_eq!(fp, value_fp(&i, py_mapping, Lang::Python));
    assert_eq!(fp, value_fp(&i, py_mutable_mapping, Lang::Python));
    assert_eq!(fp, value_fp(&i, py_alias_mapping, Lang::Python));
    assert_eq!(fp, value_fp(&i, py_alias_mutable_mapping, Lang::Python));
    assert_eq!(fp, value_fp(&i, py_alias_dict, Lang::Python));
    assert_ne!(fp, value_fp(&i, wrong_key, Lang::Java));
    assert_ne!(fp, value_fp(&i, wrong_default, Lang::Rust));
    assert_ne!(fp, value_fp(&i, wrong_map, Lang::Go));
    assert_ne!(fp, value_fp(&i, ts_wrong_key, Lang::TypeScript));
    assert_ne!(fp, value_fp(&i, ts_wrong_default, Lang::TypeScript));
    assert_ne!(fp, value_fp(&i, ts_wrong_map, Lang::TypeScript));
    assert_ne!(fp, value_fp(&i, ts_untyped, Lang::TypeScript));
    assert_ne!(fp, value_fp(&i, py_wrong_key, Lang::Python));
    assert_ne!(fp, value_fp(&i, py_wrong_default, Lang::Python));
    assert_ne!(fp, value_fp(&i, py_wrong_map, Lang::Python));
    assert_ne!(fp, value_fp(&i, py_untyped, Lang::Python));
    assert_ne!(fp, value_fp(&i, py_alias_wrong_key, Lang::Python));
    assert_ne!(fp, value_fp(&i, py_alias_wrong_default, Lang::Python));
    assert_ne!(fp, value_fp(&i, py_alias_wrong_map, Lang::Python));
    assert_ne!(fp, value_fp(&i, py_alias_unresolved, Lang::Python));
    assert_ne!(fp, value_fp(&i, py_alias_shadowed, Lang::Python));
    assert_ne!(fp, value_fp(&i, guard_wrong_key, Lang::TypeScript));
    assert_ne!(fp, value_fp(&i, guard_wrong_default, Lang::Java));
    assert_ne!(fp, value_fp(&i, guard_wrong_map, Lang::Python));
}

#[test]
fn option_defaulting_converges_with_nullish_default_boundaries() {
    let i = Interner::new();
    let js = "function f(value, fallback, other, otherDefault) { return value ?? fallback; }";
    let js_guard = "function f(value, fallback, other, otherDefault) { if (value == null) { return fallback; } return value; }";
    let ts_guard = "function f(value: number | null | undefined, fallback: number, other: number | null | undefined, otherDefault: number): number { return value == null ? fallback : value; }";
    let rust_unwrap = "pub fn f(value: Option<i32>, fallback: i32, other: Option<i32>, other_default: i32) -> i32 { value.unwrap_or(fallback) }\n";
    let rust_unwrap_else = "pub fn f(value: Option<i32>, fallback: i32, other: Option<i32>, other_default: i32) -> i32 { value.unwrap_or_else(|| fallback) }\n";
    let rust_map_or = "pub fn f(value: Option<i32>, fallback: i32, other: Option<i32>, other_default: i32) -> i32 { value.map_or(fallback, |inner| inner) }\n";
    let rust_guard = "pub fn f(value: Option<i32>, fallback: i32, other: Option<i32>, other_default: i32) -> i32 { if value.is_some() { value.unwrap_or(fallback) } else { fallback } }\n";
    let wrong_default = "pub fn f(value: Option<i32>, fallback: i32, other: Option<i32>, other_default: i32) -> i32 { value.unwrap_or(other_default) }\n";
    let wrong_value = "pub fn f(value: Option<i32>, fallback: i32, other: Option<i32>, other_default: i32) -> i32 { other.unwrap_or(fallback) }\n";
    let truthy_or =
        "function f(value, fallback, other, otherDefault) { return value || fallback; }";

    let fp = value_fp(&i, js, Lang::JavaScript);
    assert_eq!(fp, value_fp(&i, js_guard, Lang::JavaScript));
    assert_eq!(fp, value_fp(&i, ts_guard, Lang::TypeScript));
    assert_eq!(fp, value_fp(&i, rust_unwrap, Lang::Rust));
    assert_eq!(fp, value_fp(&i, rust_unwrap_else, Lang::Rust));
    assert_eq!(fp, value_fp(&i, rust_map_or, Lang::Rust));
    assert_eq!(fp, value_fp(&i, rust_guard, Lang::Rust));
    assert_ne!(fp, value_fp(&i, wrong_default, Lang::Rust));
    assert_ne!(fp, value_fp(&i, wrong_value, Lang::Rust));
    assert_ne!(fp, value_fp(&i, truthy_or, Lang::JavaScript));
}

#[test]
fn repeated_nullish_default_with_same_fallback_collapses() {
    let i = Interner::new();
    let single = "function f(value, fallback, otherDefault) { return value ?? fallback; }";
    let repeated =
        "function f(value, fallback, otherDefault) { return (value ?? fallback) ?? fallback; }";
    let different_default =
        "function f(value, fallback, otherDefault) { return (value ?? fallback) ?? otherDefault; }";
    let fp = value_fp(&i, single, Lang::JavaScript);
    assert_eq!(fp, value_fp(&i, repeated, Lang::JavaScript));
    assert_ne!(fp, value_fp(&i, different_default, Lang::JavaScript));
}

#[test]
fn rust_if_let_option_presence_converges_with_option_predicates() {
    let i = Interner::new();
    let if_some = "pub fn f(value: Option<i32>) -> bool {\n    if let Some(_) = value { true } else { false }\n}\n";
    let is_some = "pub fn g(value: Option<i32>) -> bool {\n    value.is_some()\n}\n";
    let if_none = "pub fn h(value: Option<i32>) -> bool {\n    if let None = value { true } else { false }\n}\n";
    assert_eq!(
        value_fp(&i, if_some, Lang::Rust),
        value_fp(&i, is_some, Lang::Rust),
        "if let Some(_) should converge with is_some()"
    );
    assert_ne!(
        value_fp(&i, if_some, Lang::Rust),
        value_fp(&i, if_none, Lang::Rust),
        "if let Some(_) must stay distinct from if let None"
    );
}

#[test]
fn value_graph_distinguishes_boolean_literals() {
    // `True` and `False` are behavior-defining (like `0`≠`1`): a predicate
    // `if x>0: return True else False` and its negation (booleans swapped) compute
    // opposite results and must not collapse. The bool *value* was abstracted away.
    let i = Interner::new();
    let p = "def f(x):\n    if x > 0:\n        return True\n    return False\n";
    let q = "def g(x):\n    if x > 0:\n        return False\n    return True\n";
    assert_ne!(
        value_fp(&i, p, Lang::Python),
        value_fp(&i, q, Lang::Python),
        "a predicate and its boolean-swapped negation must not fingerprint identically"
    );
    // Cross-language: the same predicate in TS converges with Python.
    let ts = "function f(x) { if (x > 0) { return true; } return false; }";
    assert_eq!(
        value_fp(&i, p, Lang::Python),
        value_fp(&i, ts, Lang::TypeScript),
        "same predicate should converge across languages"
    );
}

#[test]
fn value_graph_distinguishes_free_callees() {
    // Calls to DIFFERENT global functions must not collapse: alpha-renaming assigned
    // free names a positional cid by occurrence, so `foo(x)`/`bar(x)` became
    // identical IL. `max(a,b)`/`min(a,b)` must also remain distinct after their
    // scalar-choice builtin canonicalization.
    let i = Interner::new();
    let foo = "def f(x):\n    return foo(x)\n";
    let bar = "def f(x):\n    return bar(x)\n";
    assert_ne!(
        value_fp(&i, foo, Lang::Python),
        value_fp(&i, bar, Lang::Python),
        "calls to different globals foo(x) vs bar(x) must differ"
    );
    let mx = "def f(a, b):\n    return max(a, b)\n";
    let mn = "def f(a, b):\n    return min(a, b)\n";
    assert_ne!(
        value_fp(&i, mx, Lang::Python),
        value_fp(&i, mn, Lang::Python),
        "max(a,b) vs min(a,b) must differ"
    );
    // …but the SAME callee in two alpha-renamed copies must still converge.
    let g1 = "def f(items):\n    return helper(items)\n";
    let g2 = "def g(seq):\n    return helper(seq)\n";
    assert_eq!(
        value_fp(&i, g1, Lang::Python),
        value_fp(&i, g2, Lang::Python),
        "same callee with renamed locals must still converge"
    );
}

#[test]
fn value_graph_distinguishes_slice_bounds() {
    // `a[1:]` (drop first) and `a[:1]` (keep first) are different slices — collecting
    // only the slice's named children collapsed both to `Seq(1)`, losing whether the
    // `1` is the start or the stop. They must not fingerprint identically.
    let i = Interner::new();
    let drop1 = "def f(a):\n    return a[1:]\n";
    let keep1 = "def g(a):\n    return a[:1]\n";
    assert_ne!(
        value_fp(&i, drop1, Lang::Python),
        value_fp(&i, keep1, Lang::Python),
        "different slice bounds (a[1:] vs a[:1]) must not fingerprint identically"
    );
}

#[test]
fn value_graph_distinguishes_slice_bounds_go_rust() {
    // Same slice-position bug in Go's `a[1:]`/`a[:1]` and Rust's `&a[1..]`/`&a[..1]`,
    // plus Rust inclusivity `1..2` vs `1..=2`.
    let i = Interner::new();
    let g1 = "package p\nfunc f(a []int) []int {\n\treturn a[1:]\n}\n";
    let g2 = "package p\nfunc f(a []int) []int {\n\treturn a[:1]\n}\n";
    assert_ne!(
        value_fp(&i, g1, Lang::Go),
        value_fp(&i, g2, Lang::Go),
        "Go a[1:] vs a[:1] must differ"
    );
    let r1 = "fn f(a: &[i64]) -> &[i64] {\n    &a[1..]\n}\n";
    let r2 = "fn f(a: &[i64]) -> &[i64] {\n    &a[..1]\n}\n";
    let r3 = "fn f(a: &[i64]) -> &[i64] {\n    &a[1..2]\n}\n";
    let r4 = "fn f(a: &[i64]) -> &[i64] {\n    &a[1..=2]\n}\n";
    assert_ne!(
        value_fp(&i, r1, Lang::Rust),
        value_fp(&i, r2, Lang::Rust),
        "Rust &a[1..] vs &a[..1] must differ"
    );
    assert_ne!(
        value_fp(&i, r3, Lang::Rust),
        value_fp(&i, r4, Lang::Rust),
        "Rust 1..2 (exclusive) vs 1..=2 (inclusive) must differ"
    );
}

#[test]
fn value_graph_distinguishes_while_stride() {
    // `while i<len: s+=a[i]; i+=1` sums every element; `i+=2` sums every other. A
    // non-unit stride visits a subset, so `a[i]` is NOT `Elem(a)` — the two must not
    // fingerprint identically (the while-loop analog of the range-start bug).
    let i = Interner::new();
    let all = "def f(a):\n    s=0\n    i=0\n    while i < len(a):\n        s += a[i]\n        i += 1\n    return s\n";
    let evn = "def g(a):\n    s=0\n    i=0\n    while i < len(a):\n        s += a[i]\n        i += 2\n    return s\n";
    assert_ne!(
        value_fp(&i, all, Lang::Python),
        value_fp(&i, evn, Lang::Python),
        "a strided while-loop must not fingerprint identically to a unit-stride one"
    );
}

#[test]
fn value_graph_distinguishes_early_break() {
    // A loop that `break`s early (sum until acc>100) computes a PREFIX sum — a
    // different value than the full sum. They must not fingerprint identically;
    // `break` cannot be treated as a no-op.
    let i = Interner::new();
    let full = "def f(xs):\n    acc = 0\n    for x in xs:\n        acc += x\n    return acc\n";
    let brk = "def g(xs):\n    acc = 0\n    for x in xs:\n        acc += x\n        if acc > 100:\n            break\n    return acc\n";
    assert_ne!(
        value_fp(&i, full, Lang::Python),
        value_fp(&i, brk, Lang::Python),
        "an early-break loop must not fingerprint identically to a full-iteration loop"
    );
}

#[test]
fn statically_false_loop_guard_skips_dead_body() {
    let i = Interner::new();
    let exact = "class C { static long f(float[] vertex, int strideInBytes, float[] vertices, int numVertices) { final int size = strideInBytes / 4; for (int i = 0; i < numVertices; i++) { final int offset = i * size; boolean found = true; for (int j = 0; !found && j < size; j++) if (vertices[offset + j] != vertex[j]) found = false; if (found) return (long)i; } return -1; } }";
    let epsilon = "class C { static long f(float[] vertex, int strideInBytes, float[] vertices, int numVertices, float epsilon) { final int size = strideInBytes / 4; for (int i = 0; i < numVertices; i++) { final int offset = i * size; boolean found = true; for (int j = 0; !found && j < size; j++) if ((vertices[offset + j] > vertex[j] ? vertices[offset + j] - vertex[j] : vertex[j] - vertices[offset + j]) > epsilon) found = false; if (found) return (long)i; } return -1; } }";
    let executes_from_false = "class C { static long f(float[] vertex, int strideInBytes, float[] vertices, int numVertices) { final int size = strideInBytes / 4; for (int i = 0; i < numVertices; i++) { final int offset = i * size; boolean found = false; for (int j = 0; !found && j < size; j++) if (vertices[offset + j] == vertex[j]) found = true; if (found) return (long)i; } return -1; } }";
    let positive_guard = "class C { static long f(float[] vertex, int strideInBytes, float[] vertices, int numVertices) { final int size = strideInBytes / 4; for (int i = 0; i < numVertices; i++) { final int offset = i * size; boolean found = true; for (int j = 0; found && j < size; j++) if (vertices[offset + j] != vertex[j]) found = false; if (found) return (long)i; } return -1; } }";
    let reassigned_guard = "class C { static long f(float[] vertex, int strideInBytes, float[] vertices, int numVertices) { final int size = strideInBytes / 4; for (int i = 0; i < numVertices; i++) { final int offset = i * size; boolean found = true; found = vertices == vertex; for (int j = 0; !found && j < size; j++) if (vertices[offset + j] != vertex[j]) found = false; if (found) return (long)i; } return -1; } }";
    let fp = value_fp(&i, exact, Lang::Java);
    assert_eq!(
        fp,
        value_fp(&i, epsilon, Lang::Java),
        "a loop guarded by !found after found=true has an unreachable body"
    );
    assert_ne!(fp, value_fp(&i, executes_from_false, Lang::Java));
    assert_ne!(fp, value_fp(&i, positive_guard, Lang::Java));
    assert_ne!(fp, value_fp(&i, reassigned_guard, Lang::Java));
}

#[test]
fn java_low_bit_toggle_parity_converges_with_xor() {
    let i = Interner::new();
    let even_branch = "class C { static int f(int edgeId) { return edgeId % 2 == 0 ? edgeId + 1 : edgeId - 1; } }";
    let xor = "class C { static int g(int edgeKey) { return edgeKey ^ 1; } }";
    let odd_branch = "class C { static int h(int edgeId) { return edgeId % 2 != 0 ? edgeId - 1 : edgeId + 1; } }";
    let reversed = "class C { static int r(int edgeId) { return edgeId % 2 == 0 ? edgeId - 1 : edgeId + 1; } }";
    let xor_two = "class C { static int x(int edgeId) { return edgeId ^ 2; } }";
    let positive_one = "class C { static int p(int edgeId) { return edgeId % 2 == 1 ? edgeId - 1 : edgeId + 1; } }";
    let wrong_delta = "class C { static int w(int edgeId) { return edgeId % 2 == 0 ? edgeId + 1 : edgeId - 2; } }";

    let fp = value_fp(&i, even_branch, Lang::Java);
    assert_eq!(
        fp,
        value_fp(&i, xor, Lang::Java),
        "Java even/odd +/-1 reverse-edge idiom should converge with low-bit xor"
    );
    assert_eq!(
        fp,
        value_fp(&i, odd_branch, Lang::Java),
        "the equivalent != 0 branch order should also converge"
    );
    assert_ne!(fp, value_fp(&i, reversed, Lang::Java));
    assert_ne!(fp, value_fp(&i, xor_two, Lang::Java));
    assert_ne!(fp, value_fp(&i, positive_one, Lang::Java));
    assert_ne!(fp, value_fp(&i, wrong_delta, Lang::Java));
}

#[test]
fn c_u16_big_endian_byte_pack_converges_with_boundaries() {
    let i = Interner::new();
    let add_casted = r#"
typedef unsigned char u8;
unsigned int f(const u8 *a) {
    return (((unsigned int)a[0]) << 8) + ((unsigned int)a[1]);
}
"#;
    let add_uncasted = r#"
typedef unsigned char u8;
int g(u8 *p) {
    return (p[0] << 8) + p[1];
}
"#;
    let bit_or = r#"
unsigned int h(unsigned char *a) {
    return (a[0] << 8) | a[1];
}
"#;
    let uint8_or = r#"
unsigned int j(const uint8_t *a) {
    return (a[0] << 8) | a[1];
}
"#;
    let wrong_order = r#"
typedef unsigned char u8;
unsigned int r(const u8 *a) {
    return (a[1] << 8) | a[0];
}
"#;
    let overlapping_lane = r#"
typedef unsigned char u8;
unsigned int o(const u8 *a) {
    return (a[0] << 4) | a[1];
}
"#;
    let wrong_second_byte = r#"
typedef unsigned char u8;
unsigned int w(const u8 *a) {
    return (a[0] << 8) | a[2];
}
"#;
    let unproven_alias = r#"
typedef unsigned short u8;
unsigned int u(const u8 *a) {
    return (a[0] << 8) | a[1];
}
"#;
    let int_pointer = r#"
unsigned int q(const int *a) {
    return (a[0] << 8) | a[1];
}
"#;

    let fp = value_fp(&i, add_casted, Lang::C);
    assert!(
        fp.len() >= 4,
        "C u16 byte-pack fingerprint must stay large enough for exact scan buckets: {} atoms",
        fp.len()
    );
    assert_eq!(fp, value_fp(&i, add_uncasted, Lang::C));
    assert_eq!(fp, value_fp(&i, bit_or, Lang::C));
    assert_eq!(fp, value_fp(&i, uint8_or, Lang::C));
    assert_ne!(fp, value_fp(&i, wrong_order, Lang::C));
    assert_ne!(fp, value_fp(&i, overlapping_lane, Lang::C));
    assert_ne!(fp, value_fp(&i, wrong_second_byte, Lang::C));
    assert_ne!(fp, value_fp(&i, unproven_alias, Lang::C));
    assert_ne!(fp, value_fp(&i, int_pointer, Lang::C));
}

#[test]
fn c_u32_big_endian_byte_pack_requires_unsigned_high_lane() {
    let i = Interner::new();
    let add_casted_alias = r#"
typedef unsigned char u8;
typedef unsigned int u32;
u32 f(const u8 *a) {
    return (((u32)a[0]) << 24) + (((u32)a[1]) << 16) + (((u32)a[2]) << 8) + ((u32)a[3]);
}
"#;
    let or_casted_alias = r#"
typedef unsigned char u8;
typedef unsigned int u32;
u32 g(u8 *a) {
    return ((u32)a[0] << 24) | ((u32)a[1] << 16) | ((u32)a[2] << 8) | ((u32)a[3]);
}
"#;
    let unsigned_int_cast = r#"
unsigned int h(unsigned char *a) {
    return ((unsigned int)a[0] << 24) + ((unsigned int)a[1] << 16) + ((unsigned int)a[2] << 8) + (unsigned int)a[3];
}
"#;
    let high_lane_uncasted = r#"
typedef unsigned char u8;
typedef unsigned int u32;
u32 u(const u8 *a) {
    return (a[0] << 24) | (a[1] << 16) | (a[2] << 8) | a[3];
}
"#;
    let wrong_order = r#"
typedef unsigned char u8;
typedef unsigned int u32;
u32 r(const u8 *a) {
    return ((u32)a[1] << 24) | ((u32)a[0] << 16) | ((u32)a[2] << 8) | ((u32)a[3]);
}
"#;
    let wrong_alias = r#"
typedef unsigned char u8;
typedef signed int u32;
u32 s(const u8 *a) {
    return ((u32)a[0] << 24) | ((u32)a[1] << 16) | ((u32)a[2] << 8) | ((u32)a[3]);
}
"#;
    let int_pointer = r#"
typedef unsigned int u32;
u32 q(const int *a) {
    return ((u32)a[0] << 24) | ((u32)a[1] << 16) | ((u32)a[2] << 8) | ((u32)a[3]);
}
"#;

    let fp = value_fp(&i, add_casted_alias, Lang::C);
    assert!(
        fp.len() >= 4,
        "C u32 byte-pack fingerprint must stay large enough for exact scan buckets: {} atoms",
        fp.len()
    );
    assert_eq!(fp, value_fp(&i, or_casted_alias, Lang::C));
    assert_eq!(fp, value_fp(&i, unsigned_int_cast, Lang::C));
    assert_ne!(fp, value_fp(&i, high_lane_uncasted, Lang::C));
    assert_ne!(fp, value_fp(&i, wrong_order, Lang::C));
    assert_ne!(fp, value_fp(&i, wrong_alias, Lang::C));
    assert_ne!(fp, value_fp(&i, int_pointer, Lang::C));
}

#[test]
fn value_graph_cross_language_reorder() {
    // Same computation, different statement order, different language.
    let i = Interner::new();
    let py = "def f(a, b):\n    p = a * b\n    q = a + b\n    return p + q + p\n";
    let ts = "function g(a, b){ const q = a + b; const p = a * b; return p + q + p; }";
    assert_eq!(
        value_fp(&i, py, Lang::Python),
        value_fp(&i, ts, Lang::TypeScript)
    );
}

/// Exploratory probe (research): candidate SOUND algebraic/boolean equivalences that
/// stress phase-ordering — does a single bottom-up `mk` pass reach the canonical form,
/// or would a fixpoint/saturation be needed? Not an assertion; a frontier map.
/// Run: cargo test convergence_probe5 -- --nocapture
#[test]
fn convergence_probe5() {
    let i = Interner::new();
    let pairs: &[(&str, &str, Lang, &str, Lang)] = &[
        // Distribution in the EXPANSION direction (current code only FACTORS).
        (
            "distribute-expand",
            "def f(a,b,c):\n    return c*(a+b)\n",
            Lang::Python,
            "def g(a,b,c):\n    return c*a+c*b\n",
            Lang::Python,
        ),
        // Factor where the shared multiplicand is on the LEFT of one product.
        (
            "factor-left-shared",
            "def f(a,b,c):\n    return c*a+b*c\n",
            Lang::Python,
            "def g(a,b,c):\n    return (a+b)*c\n",
            Lang::Python,
        ),
        // De Morgan composed with comparison-direction: needs algebra THEN compare-canon.
        (
            "demorgan+cmp",
            "def f(a,b):\n    return not (a>b or a==b)\n",
            Lang::Python,
            "def g(a,b):\n    return a<b\n",
            Lang::Python,
        ),
        // Nested distribution requiring re-canonicalization of a synthesized node.
        (
            "distribute-3term",
            "def f(a,b,d,c):\n    return a*c+b*c+d*c\n",
            Lang::Python,
            "def g(a,b,d,c):\n    return (a+b+d)*c\n",
            Lang::Python,
        ),
        // Distribution feeding AC sort: (a+b)*c + e  vs  c*b + c*a + e
        (
            "distribute-then-ac",
            "def f(a,b,c,e):\n    return c*b+c*a+e\n",
            Lang::Python,
            "def g(a,b,c,e):\n    return (a+b)*c+e\n",
            Lang::Python,
        ),
        // Double negation pushed through a comparison then re-canon.
        (
            "not-not-cmp",
            "def f(a,b):\n    return not (not (a>b))\n",
            Lang::Python,
            "def g(a,b):\n    return b<a\n",
            Lang::Python,
        ),
        // Negation distributed then factored back.
        (
            "neg-distribute-factor",
            "def f(a,b,c):\n    return -(a*c+b*c)\n",
            Lang::Python,
            "def g(a,b,c):\n    return -((a+b)*c)\n",
            Lang::Python,
        ),
        // Decompose the demorgan+cmp gap:
        // (a) lattice fact alone: (a<=b) ∧ (a!=b) ≡ a<b
        (
            "lattice-le-ne",
            "def f(a,b):\n    return a<=b and a!=b\n",
            Lang::Python,
            "def g(a,b):\n    return a<b\n",
            Lang::Python,
        ),
        // (b) De Morgan over OR alone in the value graph
        (
            "demorgan-or",
            "def f(a,b,c):\n    return not (a<b or c<b)\n",
            Lang::Python,
            "def g(a,b,c):\n    return a>=b and c>=b\n",
            Lang::Python,
        ),
        // (c) De Morgan over AND alone
        (
            "demorgan-and",
            "def f(a,b,c):\n    return not (a<b and c<b)\n",
            Lang::Python,
            "def g(a,b,c):\n    return a>=b or c>=b\n",
            Lang::Python,
        ),
    ];
    let mut gaps = 0;
    for (name, a, la, b, lb) in pairs {
        let eq = value_fp(&i, a, *la) == value_fp(&i, b, *lb);
        if !eq {
            gaps += 1;
        }
        eprintln!("  [{}] {}", if eq { "CONVERGE" } else { "  GAP   " }, name);
    }
    eprintln!("probe5: {}/{} converge", pairs.len() - gaps, pairs.len());
}

#[test]
fn pointer_length_contract_is_exposed() {
    // A C function `f(int *xs, int n)` whose loop bound is `n` records the pointer-length
    // contract (array_pos=0, length_pos=1) so the behavioral oracle interprets it under
    // `n = len(xs)` — the same convention the value graph used to drop `n` and merge it with
    // the `len`-based form. A function that does NOT use a length param records none.
    let i = Interner::new();
    let c = "int sum_small(int *xs, int n) {\n int t=0;\n for (int i=0;i<n;i++){ if (xs[i]<3){ t+=xs[i]; } }\n return t;\n}\n";
    let lowered = nose_frontend::lower_source(FileId(0), "a.c", c.as_bytes(), Lang::C, &i).unwrap();
    let n = normalize(&lowered, &i, &NormalizeOptions::default());
    let contracts = nose_normalize::value_fingerprint_contracts(&n, n.units[0].root, &i);
    assert_eq!(
        contracts,
        vec![(0, 1)],
        "C (xs, n) must record contract (0,1)"
    );

    // The aligned two-array form `f(a, b, n)` shares `n` as the length of both.
    let dot = "int dot(int *a, int *b, int n) {\n int t=0;\n for (int i=0;i<n;i++){ t+=a[i]*b[i]; }\n return t;\n}\n";
    let ld = nose_frontend::lower_source(FileId(0), "d.c", dot.as_bytes(), Lang::C, &i).unwrap();
    let nd = normalize(&ld, &i, &NormalizeOptions::default());
    let dc = nose_normalize::value_fingerprint_contracts(&nd, nd.units[0].root, &i);
    assert!(
        dc.contains(&(0, 2)) || dc.contains(&(1, 2)),
        "aligned (a, b, n) must record a shared length contract at pos 2, got {dc:?}"
    );

    // A `len`-based form (no length param) records no contract.
    let py = "def sum_small(xs):\n    t=0\n    for x in xs:\n        if x<3:\n            t+=x\n    return t\n";
    let lp =
        nose_frontend::lower_source(FileId(0), "a.py", py.as_bytes(), Lang::Python, &i).unwrap();
    let np = normalize(&lp, &i, &NormalizeOptions::default());
    assert!(
        nose_normalize::value_fingerprint_contracts(&np, np.units[0].root, &i).is_empty(),
        "a len-based form uses no pointer-length contract"
    );
}

#[test]
fn lattice_strict_comparison_converges_and_separates() {
    // SOUND lattice canon on a total order: `(x ≤ y) ∧ (x ≠ y) ≡ x < y` and the dual
    // `(x < y) ∨ (x = y) ≡ x ≤ y`. Declaring the one `∧` rule composes through the
    // recursive `mk` fixpoint (De Morgan + comparison-direction canon) to also close
    // `not (a > b or a == b) ≡ a < b`, cross-language.
    let i = Interner::new();
    let lt = value_fp(&i, "def f(a,b):\n    return a<b\n", Lang::Python);
    assert_eq!(
        lt,
        value_fp(&i, "def g(a,b):\n    return a<=b and a!=b\n", Lang::Python),
        "(a<=b) and (a!=b) must converge with a<b"
    );
    assert_eq!(
        lt,
        value_fp(&i, "def g(a,b):\n    return a!=b and a<=b\n", Lang::Python),
        "operand order of the conjunction must not matter"
    );
    assert_eq!(
        lt,
        value_fp(
            &i,
            "def g(a,b):\n    return not (a>b or a==b)\n",
            Lang::Python
        ),
        "De Morgan + comparison-direction must compose into the lattice canon"
    );
    // Cross-language: a JS strict-less written as the conjunction.
    assert_eq!(
        lt,
        value_fp(
            &i,
            "function g(a,b){ return a<=b && a!=b; }",
            Lang::JavaScript
        ),
        "the lattice canon is language-agnostic"
    );
    let le = value_fp(&i, "def f(a,b):\n    return a<=b\n", Lang::Python);
    assert_eq!(
        le,
        value_fp(&i, "def g(a,b):\n    return a<b or a==b\n", Lang::Python),
        "(a<b) or (a==b) must converge with a<=b"
    );

    // HARD NEGATIVES — the rule must not over-fire (these are different computations):
    assert_ne!(
        lt,
        value_fp(&i, "def g(a,b):\n    return a<=b\n", Lang::Python),
        "a<b must NOT merge with a<=b"
    );
    assert_ne!(
        lt,
        value_fp(
            &i,
            "def g(a,b,c):\n    return a<=b and a!=c\n",
            Lang::Python
        ),
        "the inequality must be over the SAME operands, not a third variable"
    );
    assert_ne!(
        lt,
        value_fp(&i, "def g(a,b):\n    return a<=b or a!=b\n", Lang::Python),
        "the connective matters: (a<=b) OR (a!=b) is not a<b"
    );
}

#[test]
fn detection_smoke_groups_clones_excludes_decoy() {
    // Two clones (a sum loop in Python and TS) plus an unrelated decoy.
    let interner = Interner::new();
    let py = "def sum_list(items):\n    total = 0\n    i = 0\n    while i < len(items):\n        total += items[i]\n        i = i + 1\n    return total\n";
    let ts = "function total(xs) {\n  let acc = 0;\n  for (let k = 0; k < xs.length; k++) {\n    acc += xs[k];\n  }\n  return acc;\n}\n";
    let decoy = "def greet(name):\n    msg = 'hello ' + name\n    print(msg)\n    print(name)\n    return msg\n";

    let files = vec![
        nose_frontend::lower_source(FileId(0), "a.py", py.as_bytes(), Lang::Python, &interner)
            .unwrap(),
        nose_frontend::lower_source(
            FileId(1),
            "b.ts",
            ts.as_bytes(),
            Lang::TypeScript,
            &interner,
        )
        .unwrap(),
        nose_frontend::lower_source(FileId(2), "c.py", decoy.as_bytes(), Lang::Python, &interner)
            .unwrap(),
    ];
    let corpus = Corpus::new(interner, files);

    let opts = DetectOptions {
        min_lines: 2,
        min_tokens: 12,
        ..Default::default()
    };
    let detector = StructuralDetector::strict(opts.jaccard_weight);
    let report = detect(&corpus, &opts, &detector);

    // Multi-granularity units may cluster the clone at both function and block
    // level, so assert by content rather than group count: the two sum files
    // appear together, the decoy never does, and a cross-language pair is found.
    assert!(
        !report.groups.is_empty(),
        "expected at least one clone group"
    );
    let files_in_groups: std::collections::HashSet<&str> = report
        .groups
        .iter()
        .flat_map(|g| g.members.iter().map(|m| m.file.as_str()))
        .collect();
    assert!(
        files_in_groups.contains("a.py"),
        "py clone should be grouped"
    );
    assert!(
        files_in_groups.contains("b.ts"),
        "ts clone should be grouped"
    );
    assert!(
        !files_in_groups.contains("c.py"),
        "decoy must not be grouped"
    );
    assert!(
        report.duplicates.iter().any(|d| d.cross_language),
        "cross-language pair expected"
    );
}

/// Normalization must be **idempotent** — a canonicalizing pipeline should reach a
/// fixpoint, so re-normalizing already-canonical IL changes nothing. A pass that
/// fails this is a smell (it hasn't converged) and would make detection sensitive
/// to how many times IL was processed. We compare the whole-file root hash.
#[test]
fn normalization_is_idempotent() {
    let i = Interner::new();
    let samples = [
        ("def f(items):\n    t = 0\n    for x in items:\n        if x > 0:\n            t = t + x * 2\n    return t\n", Lang::Python),
        ("function g(a,b){ let r = a ? b : a + 1; while(a < b){ a = a + 1; } return r; }", Lang::JavaScript),
        ("fn h(xs: &[i32]) -> i32 { let mut s = 0; for x in xs { s += *x; } s }", Lang::Rust),
        ("def k(a,b,c):\n    t = (a + b) + c\n    if not (a and b):\n        return t\n    return c\n", Lang::Python),
    ];
    for (src, lang) in samples {
        let il = nose_frontend::lower_source(FileId(0), "t", src.as_bytes(), lang, &i).unwrap();
        let once = normalize(&il, &i, &NormalizeOptions::default());
        let twice = normalize(&once, &i, &NormalizeOptions::default());
        let h1 = subtree_hashes(&once, &i)[once.root.0 as usize];
        let h2 = subtree_hashes(&twice, &i)[twice.root.0 as usize];
        assert_eq!(h1, h2, "normalize not idempotent for {lang:?}: {src}");
    }
}

/// DISTRIBUTION / FACTORING (Num-gated): `a*c + b*c` ≡ `(a+b)*c`. The value graph factors
/// a shared multiplicand out of a sum of products when every leaf is proven numeric
/// (`value_graph/rules/factor_distribute.rs`, Lean obligation
/// `normalize.value_graph.factor_distribute`). The `*`
/// operands here are provably `Num`, so the factoring fires and the two forms converge.
#[test]
fn distribution_factors_common_multiplicand() {
    let i = Interner::new();
    assert_eq!(
        value_fp(&i, "def f(a,b,c):\n    return a*c+b*c\n", Lang::Python),
        value_fp(&i, "def g(a,b,c):\n    return (a+b)*c\n", Lang::Python),
        "a*c+b*c should factor to (a+b)*c on proven-numeric leaves"
    );
    // Three-term chain factors transitively: `a*c + b*c + d*c` ≡ `(a+b+d)*c`.
    assert_eq!(
        value_fp(
            &i,
            "def f(a,b,c,d):\n    return a*c+b*c+d*c\n",
            Lang::Python
        ),
        value_fp(&i, "def g(a,b,c,d):\n    return (a+b+d)*c\n", Lang::Python),
        "a*c+b*c+d*c should factor to (a+b+d)*c"
    );
}

/// FILTER FUSION: `filter(q, filter(p, xs))` ≡ `filter(p∧q, xs)`. The value graph carries a
/// filter's element so nested filters fuse (`value_graph.rs` `HoFKind::Filter` arm, Lean
/// `normalize.value_graph.functor`). A two-filter comprehension, an explicitly nested one, and
/// (cross-language) a JS `.filter().filter()` all converge to one filtered stream.
#[test]
fn filter_fusion_converges() {
    let i = Interner::new();
    assert_eq!(
        value_fp(
            &i,
            "def f(xs):\n    return [x for x in xs if x>0 if x<10]\n",
            Lang::Python
        ),
        value_fp(
            &i,
            "def g(xs):\n    return [x for x in [y for y in xs if y>0] if x<10]\n",
            Lang::Python
        ),
        "two stacked filters should fuse with an explicitly nested filter"
    );
    assert_eq!(
        value_fp(
            &i,
            "def f(xs):\n    return [x for x in xs if x>0 if x<10]\n",
            Lang::Python
        ),
        value_fp(
            &i,
            "function g(xs){ return xs.filter(x=>x>0).filter(x=>x<10); }",
            Lang::JavaScript
        ),
        "Python two-filter comprehension should converge with JS chained .filter().filter()"
    );
}

/// DICT-BUILDER: `{k: v for x in xs}` ≡ `d={}; for x in xs: d[k]=v`. The dict-building loop
/// is recognized as a builder of `DictEntry`s, the same node the comprehension produces.
#[test]
fn dict_comprehension_converges_with_building_loop() {
    let i = Interner::new();
    assert_eq!(
        value_fp(
            &i,
            "def f(xs):\n    return {x: x*x for x in xs}\n",
            Lang::Python
        ),
        value_fp(
            &i,
            "def g(xs):\n    d={}\n    for x in xs:\n        d[x]=x*x\n    return d\n",
            Lang::Python
        ),
        "dict comprehension should converge with the dict-building loop"
    );
}

/// SOUNDNESS GUARD for the dict-builder: a dict comprehension must stay DISTINCT from a list
/// of tuples — `{k: v for x in xs}` and `[(k, v) for x in xs]` build different values, so a
/// `DictEntry` must not collide with a tuple `Seq`. (Dicts are not oracle-modeled, so this
/// representational distinctness is what prevents the false merge.)
#[test]
fn dict_comprehension_distinct_from_tuple_list() {
    let i = Interner::new();
    assert_ne!(
        value_fp(
            &i,
            "def f(xs):\n    return {x: x*x for x in xs}\n",
            Lang::Python
        ),
        value_fp(
            &i,
            "def g(xs):\n    return [(x, x*x) for x in xs]\n",
            Lang::Python
        ),
        "a dict comprehension must NOT merge with a list of tuples (different behavior)"
    );
}

/// REDUCE-LAMBDA SELECTION: `reduce(λa,b. a if a>b else b, xs)` ≡ `max(xs)` (and the `<`
/// form ≡ `min`). The explicit fold's selection lambda is recognized as a min/max selection
/// reduction (which carries no accumulator seed), so it converges with the builtin.
#[test]
fn reduce_lambda_selection_converges() {
    let i = Interner::new();
    assert_eq!(
        value_fp(
            &i,
            "def f(xs):\n    return reduce(lambda a,b: a if a>b else b, xs)\n",
            Lang::Python
        ),
        value_fp(&i, "def g(xs):\n    return max(xs)\n", Lang::Python),
        "reduce(λ. a if a>b else b) should converge with max()"
    );
    assert_eq!(
        value_fp(
            &i,
            "def f(xs):\n    return reduce(lambda a,b: a if a<b else b, xs)\n",
            Lang::Python
        ),
        value_fp(&i, "def g(xs):\n    return min(xs)\n", Lang::Python),
        "reduce(λ. a if a<b else b) should converge with min()"
    );
}

/// COUNT of a filtered comprehension equals the sum of 1s: `len([x for x in xs if p])` ≡
/// `sum(1 for x in xs if p)` ≡ (cross-language) a Rust `xs.iter().filter(p).count()`.
#[test]
fn len_of_filtered_comprehension_is_count() {
    let i = Interner::new();
    let count_loop = value_fp(
        &i,
        "def g(xs):\n    return sum(1 for x in xs if x>0)\n",
        Lang::Python,
    );
    assert_eq!(
        value_fp(
            &i,
            "def f(xs):\n    return len([x for x in xs if x>0])\n",
            Lang::Python
        ),
        count_loop,
        "len of a filtered comprehension should equal sum(1 …)"
    );
    assert_eq!(
        value_fp(
            &i,
            "fn h(xs:&[i64])->usize{ xs.iter().filter(|x| **x>0).count() }",
            Lang::Rust
        ),
        count_loop,
        "Rust .filter(p).count() should converge with Python sum(1 for x if p)"
    );
}

/// Cross-language METHOD-FORM iterator reductions: a Rust `xs.iter().filter(p).sum()`
/// converges with the Python generator `sum(x for x in xs if p)` (method-form `.sum()`
/// canonicalizes to the same `Builtin::Sum` over the filtered stream).
#[test]
fn rust_iterator_reductions_converge() {
    let i = Interner::new();
    assert_eq!(
        value_fp(
            &i,
            "def f(xs):\n    return sum(x for x in xs if x>0)\n",
            Lang::Python
        ),
        value_fp(
            &i,
            "fn g(xs:&[i64])->i64{ xs.iter().filter(|x| **x>0).sum() }",
            Lang::Rust
        ),
        "Python filtered sum generator should converge with Rust .iter().filter().sum()"
    );
}

/// Convergence probe (research): diverse genuinely-equivalent pairs that a strong IL
/// SHOULD converge. Prints which converge vs not — the non-converging ones are IL gaps to
/// close. Not an assertion (a map of the frontier). Run: cargo test convergence_probe -- --nocapture
#[test]
fn convergence_probe() {
    let pairs: &[(&str, &str, &str)] = &[
        ("nested-if vs conjunction",
         "def f(a,b,c):\n    if a>0:\n        if b>0:\n            return c+1\n    return c+2\n",
         "def g(a,b,c):\n    if a>0 and b>0:\n        return c+1\n    return c+2\n"),
        ("else-after-return vs guard",
         "def f(a,b):\n    if a>0:\n        return b+1\n    else:\n        return b+2\n",
         "def g(a,b):\n    if a>0:\n        return b+1\n    return b+2\n"),
        ("ternary vs if-assign",
         "def f(a,b):\n    x = b+1 if a>0 else b+2\n    return x*3\n",
         "def g(a,b):\n    if a>0:\n        x = b+1\n    else:\n        x = b+2\n    return x*3\n"),
        ("filter fusion",
         "def f(xs):\n    return [x for x in xs if x>0 if x<10]\n",
         "def g(xs):\n    return [x for x in [y for y in xs if y>0] if x<10]\n"),
        ("map-filter (filter then map)",
         "def f(xs):\n    return [h(x) for x in xs if x>0]\n",
         "def g(xs):\n    return [h(y) for y in [x for x in xs if x>0]]\n"),
        ("fold-map fusion (sum of squares)",
         "def f(xs):\n    return sum(x*x for x in xs)\n",
         "def g(xs):\n    t = 0\n    for x in xs:\n        t += x*x\n    return t\n"),
        ("De Morgan",
         "def f(a,b):\n    return not (a>0 and b>0)\n",
         "def g(a,b):\n    return a<=0 or b<=0\n"),
        ("comparison swap inside",
         "def f(a,b):\n    return (a>b) and (b>0)\n",
         "def g(a,b):\n    return (0<b) and (b<a)\n"),
        ("double map then sum",
         "def f(xs):\n    return sum(g(f(x)) for x in xs)\n",
         "def g2(xs):\n    return sum(g(y) for y in [f(x) for x in xs])\n"),
        ("while vs for-range sum",
         "def f(xs):\n    t=0\n    for i in range(len(xs)):\n        t+=xs[i]\n    return t\n",
         "def g(xs):\n    t=0\n    i=0\n    while i<len(xs):\n        t+=xs[i]\n        i+=1\n    return t\n"),
    ];
    let i = Interner::new();
    let mut gaps = 0;
    for (name, a, b) in pairs {
        let eq = value_fp(&i, a, Lang::Python) == value_fp(&i, b, Lang::Python);
        if !eq {
            gaps += 1;
        }
        eprintln!("  [{}] {}", if eq { "CONVERGE" } else { "  GAP   " }, name);
    }
    eprintln!(
        "convergence probe: {}/{} converge",
        pairs.len() - gaps,
        pairs.len()
    );
}

/// Cross-language + more-construct convergence probe (research): the SAME algorithm in
/// different languages / forms should converge to one fingerprint. Maps the frontier.
#[test]
fn convergence_probe_xlang() {
    // (name, srcA, langA, srcB, langB)
    let pairs: &[(&str, &str, Lang, &str, Lang)] = &[
        ("sum-loop Py vs JS-reduce",
         "def f(xs):\n    t=0\n    for x in xs:\n        t+=x\n    return t\n", Lang::Python,
         "function f(xs){ return xs.reduce((a,x)=>a+x, 0); }", Lang::JavaScript),
        ("sum-loop Py vs Go",
         "def f(xs):\n    t=0\n    for x in xs:\n        t+=x\n    return t\n", Lang::Python,
         "package p\nfunc f(xs []int) int {\n\tt := 0\n\tfor _, x := range xs {\n\t\tt += x\n\t}\n\treturn t\n}\n", Lang::Go),
        ("sum-loop Py vs Rust-fold",
         "def f(xs):\n    t=0\n    for x in xs:\n        t+=x\n    return t\n", Lang::Python,
         "fn f(xs: &[i64]) -> i64 { xs.iter().fold(0, |a, x| a + x) }", Lang::Rust),
        ("map Py vs JS",
         "def f(xs):\n    return [x*x for x in xs]\n", Lang::Python,
         "function f(xs){ return xs.map(x => x*x); }", Lang::JavaScript),
        ("guard Py vs Go",
         "def f(a,b):\n    if a>0:\n        return b+1\n    return b+2\n", Lang::Python,
         "package p\nfunc f(a,b int) int {\n\tif a>0 {\n\t\treturn b+1\n\t}\n\treturn b+2\n}\n", Lang::Go),
        ("x*2 vs x+x", "def f(x):\n    return x*2\n", Lang::Python, "def g(x):\n    return x+x\n", Lang::Python),
        ("abs idioms Py", "def f(x):\n    return x if x>=0 else -x\n", Lang::Python, "def g(x):\n    return abs(x)\n", Lang::Python),
        ("compound assign", "def f(a,b):\n    a += b\n    a *= 2\n    return a\n", Lang::Python, "def g(a,b):\n    return (a+b)*2\n", Lang::Python),
        ("min idioms", "def f(a,b):\n    return a if a<b else b\n", Lang::Python, "def g(a,b):\n    return min(a,b)\n", Lang::Python),
        ("count loop vs sum-1", "def f(xs):\n    c=0\n    for x in xs:\n        if x>0:\n            c+=1\n    return c\n", Lang::Python, "def g(xs):\n    return sum(1 for x in xs if x>0)\n", Lang::Python),
    ];
    let i = Interner::new();
    let mut gaps = 0;
    for (name, a, la, b, lb) in pairs {
        let eq = value_fp(&i, a, *la) == value_fp(&i, b, *lb);
        if !eq {
            gaps += 1;
        }
        eprintln!("  [{}] {}", if eq { "CONVERGE" } else { "  GAP   " }, name);
    }
    eprintln!(
        "xlang probe: {}/{} converge",
        pairs.len() - gaps,
        pairs.len()
    );
}

/// More-construct convergence probe (research, batch 2): widen the frontier map.
#[test]
fn convergence_probe2() {
    let i = Interner::new();
    let pairs: &[(&str, &str, Lang, &str, Lang)] = &[
        ("chained compare", "def f(a,b,c):\n    return a<b<c\n", Lang::Python, "def g(a,b,c):\n    return a<b and b<c\n", Lang::Python),
        ("aug-sub vs assign", "def f(a,b):\n    a -= b\n    return a\n", Lang::Python, "def g(a,b):\n    return a-b\n", Lang::Python),
        ("not-eq vs !=", "def f(a,b):\n    return not (a==b)\n", Lang::Python, "def g(a,b):\n    return a!=b\n", Lang::Python),
        ("double not", "def f(a,b):\n    return not (not (a<b))\n", Lang::Python, "def g(a,b):\n    return a<b\n", Lang::Python),
        ("or-default vs ternary", "def f(a,b):\n    return a if a else b\n", Lang::Python, "def g(a,b):\n    return a or b\n", Lang::Python),
        ("nested ternary vs elif", "def f(a):\n    return 1 if a>0 else (2 if a<0 else 3)\n", Lang::Python, "def g(a):\n    if a>0:\n        return 1\n    elif a<0:\n        return 2\n    return 3\n", Lang::Python),
        ("product loop vs reduce", "def f(xs):\n    p=1\n    for x in xs:\n        p*=x\n    return p\n", Lang::Python, "def g(xs):\n    r=1\n    for x in xs:\n        r=r*x\n    return r\n", Lang::Python),
        ("max loop vs max()", "def f(xs):\n    m=xs[0]\n    for x in xs:\n        if x>m:\n            m=x\n    return m\n", Lang::Python, "def g(xs):\n    return max(xs)\n", Lang::Python),
        ("filter-map JS vs Py", "function f(xs){ return xs.filter(x=>x>0).map(x=>h(x)); }", Lang::JavaScript, "def g(xs):\n    return [h(x) for x in xs if x>0]\n", Lang::Python),
        ("early-continue vs filter", "def f(xs):\n    t=0\n    for x in xs:\n        if x<=0:\n            continue\n        t+=x\n    return t\n", Lang::Python, "def g(xs):\n    return sum(x for x in xs if x>0)\n", Lang::Python),
        ("swap temps", "def f(a,b):\n    t=a\n    a=b\n    b=t\n    return a-b\n", Lang::Python, "def g(a,b):\n    return b-a\n", Lang::Python),
        ("redundant paren-group", "def f(a,b,c):\n    return (a+b)+c\n", Lang::Python, "def g(a,b,c):\n    return a+(b+c)\n", Lang::Python),
    ];
    let mut gaps = 0;
    for (name, a, la, b, lb) in pairs {
        let eq = value_fp(&i, a, *la) == value_fp(&i, b, *lb);
        if !eq {
            gaps += 1;
        }
        eprintln!("  [{}] {}", if eq { "CONVERGE" } else { "  GAP   " }, name);
    }
    eprintln!("probe2: {}/{} converge", pairs.len() - gaps, pairs.len());
}

/// Convergence probe batch 3 (research): slices, enumerate, dict, recursion, more xlang.
#[test]
fn convergence_probe3() {
    let i = Interner::new();
    let pairs: &[(&str, &str, Lang, &str, Lang)] = &[
        ("enumerate vs range-index", "def f(xs):\n    t=0\n    for i,x in enumerate(xs):\n        t+=i*x\n    return t\n", Lang::Python, "def g(xs):\n    t=0\n    for i in range(len(xs)):\n        t+=i*xs[i]\n    return t\n", Lang::Python),
        ("dict-comp vs loop", "def f(xs):\n    return {x: x*x for x in xs}\n", Lang::Python, "def g(xs):\n    d={}\n    for x in xs:\n        d[x]=x*x\n    return d\n", Lang::Python),
        ("any vs or-loop", "def f(xs):\n    return any(x>0 for x in xs)\n", Lang::Python, "def g(xs):\n    for x in xs:\n        if x>0:\n            return True\n    return False\n", Lang::Python),
        ("all vs and-loop", "def f(xs):\n    return all(x>0 for x in xs)\n", Lang::Python, "def g(xs):\n    for x in xs:\n        if not (x>0):\n            return False\n    return True\n", Lang::Python),
        ("reversed-compare", "def f(a,b):\n    return a>=b\n", Lang::Python, "def g(a,b):\n    return b<=a\n", Lang::Python),
        ("neg distribute", "def f(a,b):\n    return -(a+b)\n", Lang::Python, "def g(a,b):\n    return -a-b\n", Lang::Python),
        ("mul-add factor", "def f(a,b,c):\n    return a*c+b*c\n", Lang::Python, "def g(a,b,c):\n    return (a+b)*c\n", Lang::Python),
        ("string concat order", "def f(a,b):\n    return a+b+a\n", Lang::Python, "def g(a,b):\n    return a+(b+a)\n", Lang::Python),
        ("Go for-i vs Py range", "package p\nfunc f(xs []int) int {\n\tt:=0\n\tfor i:=0;i<len(xs);i++{\n\t\tt+=xs[i]\n\t}\n\treturn t\n}\n", Lang::Go, "def g(xs):\n    t=0\n    for i in range(len(xs)):\n        t+=xs[i]\n    return t\n", Lang::Python),
        ("Rust map vs Py comp", "fn f(xs:&[i64])->Vec<i64>{ xs.iter().map(|x| x*x).collect() }", Lang::Rust, "def g(xs):\n    return [x*x for x in xs]\n", Lang::Python),
        ("compound vs explicit", "def f(a):\n    a //= 2\n    return a\n", Lang::Python, "def g(a):\n    return a // 2\n", Lang::Python),
        ("nested-neg", "def f(x):\n    return -(-(-x))\n", Lang::Python, "def g(x):\n    return -x\n", Lang::Python),
    ];
    let mut gaps = 0;
    for (name, a, la, b, lb) in pairs {
        let eq = value_fp(&i, a, *la) == value_fp(&i, b, *lb);
        if !eq {
            gaps += 1;
        }
        eprintln!("  [{}] {}", if eq { "CONVERGE" } else { "  GAP   " }, name);
    }
    eprintln!("probe3: {}/{} converge", pairs.len() - gaps, pairs.len());
}

/// Convergence probe batch 4 (research): candidate Type-4 equivalences to scope which to
/// close next — negative indexing, count-of-filter, reduce-lambda selection, more idioms.
#[test]
fn convergence_probe4() {
    let i = Interner::new();
    let pairs: &[(&str, &str, Lang, &str, Lang)] = &[
        (
            "neg-index last",
            "def f(s):\n    return s[len(s)-1]\n",
            Lang::Python,
            "def g(s):\n    return s[-1]\n",
            Lang::Python,
        ),
        (
            "neg-index k",
            "def f(s):\n    return s[len(s)-2]\n",
            Lang::Python,
            "def g(s):\n    return s[-2]\n",
            Lang::Python,
        ),
        (
            "len-count vs sum-1",
            "def f(xs):\n    return len([x for x in xs if x>0])\n",
            Lang::Python,
            "def g(xs):\n    return sum(1 for x in xs if x>0)\n",
            Lang::Python,
        ),
        (
            "reduce-lambda max vs max()",
            "def f(xs):\n    return reduce(lambda a,b: a if a>b else b, xs)\n",
            Lang::Python,
            "def g(xs):\n    return max(xs)\n",
            Lang::Python,
        ),
        (
            "reduce-lambda min vs min()",
            "def f(xs):\n    return reduce(lambda a,b: a if a<b else b, xs)\n",
            Lang::Python,
            "def g(xs):\n    return min(xs)\n",
            Lang::Python,
        ),
        (
            "not-in vs not(in)",
            "def f(a,b):\n    return a not in b\n",
            Lang::Python,
            "def g(a,b):\n    return not (a in b)\n",
            Lang::Python,
        ),
        (
            "filter Py vs JS",
            "def f(xs):\n    return [x for x in xs if x>0]\n",
            Lang::Python,
            "function g(xs){ return xs.filter(x=>x>0); }",
            Lang::JavaScript,
        ),
        (
            "sum-filter Py vs Rust",
            "def f(xs):\n    return sum(x for x in xs if x>0)\n",
            Lang::Python,
            "fn g(xs:&[i64])->i64{ xs.iter().filter(|x| **x>0).sum() }",
            Lang::Rust,
        ),
    ];
    let mut gaps = 0;
    for (name, a, la, b, lb) in pairs {
        let eq = value_fp(&i, a, *la) == value_fp(&i, b, *lb);
        if !eq {
            gaps += 1;
        }
        eprintln!("  [{}] {}", if eq { "CONVERGE" } else { "  GAP   " }, name);
    }
    eprintln!("probe4: {}/{} converge", pairs.len() - gaps, pairs.len());
}

// ---------------------------------------------------------------------------
// Recursion ↔ iteration (recursion.rs). Tail recursion and numeric structural
// recursion are rewritten to the loop a programmer would have written, so they
// converge with hand-written iteration and with each other — cross-language too.
// The negatives guard the soundness boundary: a different op, scale, or base must
// keep a distinct value fingerprint (no false merge).
// ---------------------------------------------------------------------------

#[test]
fn tail_recursion_converges_with_while_loop() {
    let i = Interner::new();
    let rec = "def f(n, acc):\n    if n == 0:\n        return acc\n    return f(n - 1, acc + n)\n";
    let loopv = "def g(n, acc):\n    while n != 0:\n        acc = acc + n\n        n = n - 1\n    return acc\n";
    assert_eq!(
        value_fp(&i, rec, Lang::Python),
        value_fp(&i, loopv, Lang::Python),
        "tail-accumulator recursion should converge with the equivalent while loop"
    );
}

#[test]
fn tail_recursion_converges_cross_language() {
    // Python accumulator recursion ≡ a JavaScript while loop — the shared IL makes the
    // recursion→iteration rewrite cross-language for free.
    let i = Interner::new();
    let py = "def f(n, acc):\n    if n == 0:\n        return acc\n    return f(n - 1, acc + n)\n";
    let js = "function g(n, acc){ while(n != 0){ acc = acc + n; n = n - 1; } return acc; }";
    assert_eq!(
        value_fp(&i, py, Lang::Python),
        value_fp(&i, js, Lang::JavaScript),
        "Python tail recursion and JS while loop should converge"
    );
}

#[test]
fn structural_recursion_sum_converges_with_loop() {
    // `n + f(n-1)` is a `+`-monoid fold (identity 0) → accumulator loop.
    let i = Interner::new();
    let rec = "def f(n):\n    if n == 0:\n        return 0\n    return n + f(n - 1)\n";
    let loopv = "def g(n):\n    acc = 0\n    while n != 0:\n        acc = acc + n\n        n = n - 1\n    return acc\n";
    assert_eq!(
        value_fp(&i, rec, Lang::Python),
        value_fp(&i, loopv, Lang::Python),
        "structural sum recursion should converge with the accumulator loop"
    );
}

#[test]
fn structural_recursion_factorial_converges_with_loop() {
    // `n * f(n-1)` is a `*`-monoid fold (identity 1) → accumulator loop.
    let i = Interner::new();
    let rec = "def f(n):\n    if n == 0:\n        return 1\n    return n * f(n - 1)\n";
    let loopv = "def g(n):\n    acc = 1\n    while n != 0:\n        acc = acc * n\n        n = n - 1\n    return acc\n";
    assert_eq!(
        value_fp(&i, rec, Lang::Python),
        value_fp(&i, loopv, Lang::Python),
        "structural factorial recursion should converge with the accumulator loop"
    );
}

#[test]
fn two_structural_recursions_converge() {
    // Independent of any loop: two same-shape recursions must share a fingerprint.
    let i = Interner::new();
    let f = "def f(n):\n    if n == 0:\n        return 0\n    return n + f(n - 1)\n";
    let g = "def h(m):\n    if m == 0:\n        return 0\n    return m + h(m - 1)\n";
    assert_eq!(value_fp(&i, f, Lang::Python), value_fp(&i, g, Lang::Python),);
}

#[test]
fn recursion_does_not_falsely_merge() {
    // The soundness boundary: a different combine op / scale / base case must NOT collapse
    // onto the sum. (Subtraction is not an associative monoid, so it is never rewritten.)
    let i = Interner::new();
    let sum = "def f(n):\n    if n == 0:\n        return 0\n    return n + f(n - 1)\n";
    let product = "def g(n):\n    if n == 0:\n        return 1\n    return n * g(n - 1)\n";
    let scaled = "def g(n):\n    if n == 0:\n        return 0\n    return 2 * n + g(n - 1)\n";
    let subtract = "def g(n):\n    if n == 0:\n        return 0\n    return n - g(n - 1)\n";
    let base5 = "def g(n):\n    if n == 0:\n        return 5\n    return n + g(n - 1)\n";
    let fp = value_fp(&i, sum, Lang::Python);
    for (label, other) in [
        ("product", product),
        ("scaled", scaled),
        ("subtraction", subtract),
        ("non-identity base", base5),
    ] {
        assert_ne!(
            fp,
            value_fp(&i, other, Lang::Python),
            "sum recursion must not merge with {label}"
        );
    }
}

#[test]
fn interp_executes_self_recursion() {
    // The oracle form keeps recursion un-rewritten (it stops before the recursion pass), so
    // this exercises the interpreter's self-call support directly: factorial must evaluate.
    use nose_normalize::{run_unit, Value};
    let i = Interner::new();
    let src = "def fact(n):\n    if n <= 0:\n        return 1\n    return n * fact(n - 1)\n";
    let il = nose_frontend::lower_source(FileId(0), "t", src.as_bytes(), Lang::Python, &i).unwrap();
    let oracle = normalize(
        &il,
        &i,
        &NormalizeOptions {
            oracle: true,
            ..NormalizeOptions::default()
        },
    );
    let root = first_func(&oracle);
    let beh = run_unit(&oracle, root, &[Value::Int(5)]).expect("recursion should interpret");
    assert_eq!(
        beh.ret,
        Value::Int(120),
        "5! = 120 via interpreted recursion"
    );
}

#[test]
fn loop_accumulator_seed_is_not_abstracted() {
    // A loop-carried accumulator that is not a clean collection reduction (a numeric
    // countdown fold) still depends on its pre-loop SEED. Regression: the compact
    // `Recurrence` value keyed only on the per-iteration update, so a parameter-seeded
    // accumulator (`acc=a` → returns `a + Σ`) collapsed onto a zero-seeded one
    // (`total=0` → returns `Σ`) — a false merge. They must now stay distinct.
    let i = Interner::new();
    let param_seed = "def f(n, acc):\n    while n > 0:\n        acc = acc + n\n        n = n - 1\n    return acc\n";
    let zero_seed = "def g(n):\n    total = 0\n    while n > 0:\n        total = total + n\n        n = n - 1\n    return total\n";
    assert_ne!(
        value_fp(&i, param_seed, Lang::Python),
        value_fp(&i, zero_seed, Lang::Python),
        "a parameter-seeded accumulator must not merge with a zero-seeded one"
    );
    // Same seed (both 0) and same update still converge — the fix only adds the seed to the
    // key, it does not over-split.
    let zero_seed2 = "def h(m):\n    s = 0\n    while m > 0:\n        s = s + m\n        m = m - 1\n    return s\n";
    assert_eq!(
        value_fp(&i, zero_seed, Lang::Python),
        value_fp(&i, zero_seed2, Lang::Python),
        "two zero-seeded countdown sums must still converge"
    );
}

#[test]
fn c_hex_literal_with_e_lowers_to_int_not_float() {
    // 0xE5 is a hex INTEGER (229); the 'E' is a hex digit, not a float exponent.
    let i = Interner::new();
    let il = nose_frontend::lower_source(FileId(0), "t", b"int f(){ return 0xE5; }", Lang::C, &i)
        .unwrap();
    let root = first_func(&il);
    let s = il.to_sexpr(root, &i);
    assert!(
        !s.to_lowercase().contains("float"),
        "0xE5 (hex int) must not lower to a float literal: {s}"
    );
}
