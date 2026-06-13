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

fn count_nodes(il: &nose_il::Il, root: NodeId, kind: Option<nose_il::NodeKind>) -> usize {
    let own = match kind {
        Some(kind) => usize::from(il.node(root).kind == kind),
        None => 1,
    };
    own + il
        .children(root)
        .iter()
        .map(|child| count_nodes(il, *child, kind))
        .sum::<usize>()
}

#[test]
fn foreach_accumulator_is_interpretable_iterating_a_nonlist_is_err_not_unsupported() {
    // The headline foreach-accumulator: iterating a LIST computes; iterating a non-iterable
    // (a scalar) is a runtime TYPE ERROR (`Err`), NOT an unmodelable construct. So the unit stays
    // interpretable on every battery row (list → value, scalar → Err) and the oracle can check it
    // instead of excluding it. Before this, the scalar case returned `Unsupported` (None), which
    // dropped the whole unit from the oracle.
    let i = Interner::new();
    let il = nose_frontend::lower_source(
        FileId(0),
        "t",
        b"def sum_pos(xs):\n    t = 0\n    for x in xs:\n        if x > 0:\n            t = t + x\n    return t\n",
        Lang::Python,
        &i,
    )
    .unwrap();
    let n = normalize(&il, &i, &NormalizeOptions::default());
    let f = first_func(&n);
    use nose_normalize::{run_unit, Value};
    let list = Value::List(vec![Value::Int(2), Value::Int(-1), Value::Int(5)]);
    assert_eq!(
        run_unit(&n, &i, f, &[list])
            .expect("list input is interpretable")
            .ret,
        Value::Int(7),
        "summing the positives of [2,-1,5] is 7",
    );
    let scalar =
        run_unit(&n, &i, f, &[Value::Int(3)]).expect("scalar input stays interpretable (Err)");
    assert_eq!(
        scalar.ret,
        Value::Err,
        "iterating a scalar is a runtime type error (Err), not Unsupported",
    );
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
    let prod_reduce =
        "import functools\n\ndef p(xs):\n    return functools.reduce(lambda a, b: a * b, xs, 1)\n";
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
    // Annotate the params `: int` so the long `+` chain is PROVEN numeric and commutes
    // (#283-C gates untyped `+` reorder; this test is about AC compaction, not the gate).
    let typed_params = params
        .iter()
        .map(|p| format!("{p}: int"))
        .collect::<Vec<_>>()
        .join(", ");
    let src = |expr: &str| format!("def f({typed_params}):\n    return {expr}\n");

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
fn large_generated_hof_chains_stay_compact_and_distinct() {
    fn hof_chain_expr(depth: usize, seed: usize) -> String {
        let mut expr = "xs".to_string();
        for i in 0..depth {
            let threshold = (i + seed) % 7;
            let delta = (i + seed) % 11;
            expr = format!("{expr}.filter((x) => x > {threshold}).map((x) => x + {delta})");
        }
        format!("{expr}.reduce((acc, x) => acc + x, 0)")
    }

    let i = Interner::new();
    let deep_src = format!("function f(xs) {{ return {}; }}", hof_chain_expr(32, 0));
    let changed_src = format!("function f(xs) {{ return {}; }}", hof_chain_expr(32, 1));
    let wide_terms = (0..12)
        .map(|seed| hof_chain_expr(6, seed))
        .collect::<Vec<_>>()
        .join(" + ");
    let wide_src = format!("function f(xs) {{ return {wide_terms}; }}");

    let deep_fp = value_fp(&i, &deep_src, Lang::JavaScript);
    assert_ne!(
        deep_fp,
        value_fp(&i, &changed_src, Lang::JavaScript),
        "deep HoF budget smoke must keep changed predicates and maps distinct"
    );
    assert!(
        deep_fp.len() <= 450,
        "deep HoF chain should keep a compact value fingerprint: {} nodes",
        deep_fp.len()
    );

    let wide_fp = value_fp(&i, &wide_src, Lang::JavaScript);
    assert!(
        wide_fp.len() <= 1200,
        "wide HoF chain should keep a compact value fingerprint: {} nodes",
        wide_fp.len()
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
    let loop_js = "function f(xs: number[]): number { let total = 0; for (const x of xs) { if (x > 0) { total += x; } } return total; }";
    let reduce_js =
        "function f(xs: number[]): number { return xs.filter(x => x > 0).reduce((total, x) => total + x, 0); }";
    let reduce_rs = "fn f(xs: &[i64]) -> i64 { xs.iter().copied().filter(|x| *x > 0).fold(0, |total, x| total + x) }";
    let bad_init =
        "function f(xs: number[]): number { return xs.filter(x => x > 0).reduce((total, x) => total + x, 1); }";
    let loop_fp = value_fp(&i, loop_js, Lang::TypeScript);
    assert_eq!(
        loop_fp,
        value_fp(&i, reduce_js, Lang::TypeScript),
        "JS filter().reduce(sum) should converge with the guarded loop"
    );
    assert_ne!(
        loop_fp,
        value_fp(&i, reduce_rs, Lang::Rust),
        "TypeScript number[] element domains are not yet numeric proof for cross-language relational predicates"
    );
    assert_ne!(
        loop_fp,
        value_fp(&i, bad_init, Lang::TypeScript),
        "changing the reduce seed is a hard negative"
    );
}

#[test]
fn filtered_count_aggregates_converge_with_count_loop() {
    // `filter(p).length` and Rust `filter(p).count()` are count-filter reductions:
    // add 1 when the predicate holds, otherwise add 0. Ruby `count { p }` stays opaque
    // until a pack/receiver proof can establish that `xs` is Ruby's collection protocol.
    let i = Interner::new();
    let loop_js = "function f(xs: number[]): number { let count = 0; for (const x of xs) { if (x > 0) { count += 1; } } return count; }";
    let len_js = "function f(xs: number[]): number { return xs.filter(x => x > 0).length; }";
    let count_rs =
        "fn f(xs: &[i64]) -> i64 { xs.iter().copied().filter(|x| *x > 0).count() as i64 }";
    let count_rb = "def f(xs)\n  xs.count { |x| x > 0 }\nend\n";
    let bad_predicate =
        "function f(xs: number[]): number { return xs.filter(x => x >= 0).length; }";
    let loop_fp = return_fp(&i, loop_js, Lang::TypeScript);
    assert_eq!(
        loop_fp,
        return_fp(&i, len_js, Lang::TypeScript),
        "JS filter().length should converge with a guarded count loop"
    );
    assert_ne!(
        loop_fp,
        return_fp(&i, count_rs, Lang::Rust),
        "TypeScript number[] element domains are not yet numeric proof for cross-language relational predicates"
    );
    assert_ne!(
        loop_fp,
        return_fp(&i, count_rb, Lang::Ruby),
        "Ruby count block must stay closed without a receiver/protocol proof"
    );
    assert_ne!(
        loop_fp,
        return_fp(&i, bad_predicate, Lang::TypeScript),
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
    let missing_arrays_import = "class C { static int f(int[] xs) { return Arrays.stream(xs).filter(x -> x > 0).reduce(0, (total, x) -> total + x); } }";
    let shadowed_arrays =
        "import java.util.Arrays; class Arrays {} class C { static int f(int[] xs) { return Arrays.stream(xs).filter(x -> x > 0).reduce(0, (total, x) -> total + x); } }";
    let sum_fp = value_fp(&i, sum_loop, Lang::Java);
    assert_eq!(sum_fp, value_fp(&i, sum_stream, Lang::Java));
    assert_eq!(
        return_fp(&i, count_loop, Lang::Java),
        return_fp(&i, count_stream, Lang::Java)
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
    assert_ne!(sum_fp, value_fp(&i, missing_arrays_import, Lang::Java));
    assert_ne!(sum_fp, value_fp(&i, shadowed_arrays, Lang::Java));
}

#[test]
fn java_stream_flat_map_converges_with_python_comprehension() {
    let i = Interner::new();
    let py_flat = "def f(xs, ys):\n    return [x + y for x in xs for y in ys]\n";
    let java_flat = "import java.util.Arrays; class C { static Object f(int[] xs, int[] ys) { return Arrays.stream(xs).flatMap(x -> Arrays.stream(ys).map(y -> x + y)); } }";
    let py_nested = "def f(xs, ys):\n    return [[x + y for y in ys] for x in xs]\n";
    let java_nested = "import java.util.Arrays; class C { static Object f(int[] xs, int[] ys) { return Arrays.stream(xs).map(x -> Arrays.stream(ys).map(y -> x + y)); } }";

    let flat_fp = value_fp(&i, py_flat, Lang::Python);
    let nested_fp = value_fp(&i, py_nested, Lang::Python);
    assert_eq!(
        flat_fp,
        value_fp(&i, java_flat, Lang::Java),
        "Java Stream.flatMap/map should match Python multi-clause comprehension"
    );
    assert_eq!(
        nested_fp,
        value_fp(&i, java_nested, Lang::Java),
        "Java Stream.map returning streams should stay nested"
    );
    assert_ne!(
        flat_fp, nested_fp,
        "flatMap and map-returning-stream must remain distinct"
    );
}

#[test]
fn rust_vec_new_builder_loop_stays_distinct_from_flat_map_without_nested_element_proof() {
    // `xss: &[Vec<_>]` proves the outer parameter is a collection, but the current semantic
    // kernel does not yet carry the element-type proof needed to know that the lambda parameter
    // `xs` is itself a collection. Keep the `.flat_map(|xs| xs.iter()...)` form fail-closed until
    // nested collection receiver proofs exist.
    let i = Interner::new();
    let builder = "pub fn f(xss: &[Vec<i64>]) -> Vec<i64> { let mut out = Vec::new(); for xs in xss { for y in xs { out.push(*y); } } out }";
    let flat = "pub fn f(xss: &[Vec<i64>]) -> Vec<i64> { xss.iter().flat_map(|xs| xs.iter().map(|y| *y)).collect() }";
    let changed = "pub fn f(xss: &[Vec<i64>]) -> Vec<i64> { xss.iter().flat_map(|xs| xs.iter().map(|y| y + 1)).collect() }";
    let bfp = value_fp(&i, builder, Lang::Rust);
    assert_ne!(
        bfp,
        value_fp(&i, flat, Lang::Rust),
        "nested flat_map must not converge without a nested element collection proof"
    );
    assert_ne!(
        bfp,
        value_fp(&i, changed, Lang::Rust),
        "a changed inner element (y + 1) stays a hard negative"
    );
}

#[test]
fn rust_recursion_converges_with_iteration_via_return_unwrap() {
    // Numeric structural recursion `fac(n) = n*fac(n-1)` (base 1) converges with its
    // accumulator loop — now in Rust too. Rust lowers `return e;` wrapped in `ExprStmt`, which
    // used to hide the bare-`Return` shape `recursion::recognize` matches on; desugar now
    // unwraps `ExprStmt(Return|Throw)`, so the recursion→iteration canon fires uniformly and
    // converges cross-language with the Python loop. The sum monoid stays a hard negative.
    let i = Interner::new();
    let py_loop = "def fac(n):\n    acc = 1\n    while n != 0:\n        acc = acc * n\n        n = n - 1\n    return acc\n";
    let rust_rec = "pub fn fac(n: i64) -> i64 { if n == 0 { return 1; } return n * fac(n - 1); }";
    let rust_loop = "pub fn fac(mut n: i64) -> i64 { let mut acc = 1; while n != 0 { acc = acc * n; n = n - 1; } return acc; }";
    let sum_loop = "def g(n):\n    acc = 0\n    while n != 0:\n        acc = acc + n\n        n = n - 1\n    return acc\n";
    let fold_fp = value_fp(&i, py_loop, Lang::Python);
    assert_eq!(
        fold_fp,
        value_fp(&i, rust_rec, Lang::Rust),
        "rust recursion must converge cross-language with the python accumulator loop"
    );
    assert_eq!(
        value_fp(&i, rust_rec, Lang::Rust),
        value_fp(&i, rust_loop, Lang::Rust),
        "rust recursion must converge with the rust accumulator loop"
    );
    assert_ne!(
        fold_fp,
        value_fp(&i, sum_loop, Lang::Python),
        "the sum monoid (acc + n, base 0) must stay a hard negative"
    );
}

#[test]
fn ruby_shovel_builder_each_stays_closed_without_receiver_proof() {
    // Ruby `xs.each { ... }` stays an ordinary block call until a pack supplies receiver/protocol
    // proof for `xs`. The Ruby `<<` builder signal is still retained inside the opaque call body,
    // but the default analyzer must not infer Enumerable semantics from the method name alone.
    let i = Interner::new();
    let py_comp = "def f(xs):\n    return [x * x for x in xs]\n";
    let ruby_build = "def f(xs)\n  out = []\n  xs.each { |x| out << x * x }\n  out\nend\n";
    let ruby_diff = "def f(xs)\n  out = []\n  xs.each { |x| out << x + 1 }\n  out\nend\n";
    let comp_fp = value_fp(&i, py_comp, Lang::Python);
    assert_ne!(
        comp_fp,
        value_fp(&i, ruby_build, Lang::Ruby),
        "ruby each builder must stay closed without receiver/protocol proof"
    );
    assert_ne!(
        value_fp(&i, ruby_build, Lang::Ruby),
        value_fp(&i, ruby_diff, Lang::Ruby),
        "a different per-element contribution must stay distinct"
    );
}

#[test]
fn java_arraylist_add_builder_loop_converges_with_comprehension() {
    // Java builds a list with `List<T> out = new ArrayList<>(); for (…) out.add(e); return out`.
    // Modeling `new ArrayList<>()` as the empty `array` Seq and `out.add(e)` as the per-element
    // build (scoped by the empty-Seq seed — so overloaded `.add` on a Set/BigInteger never
    // enters) makes the Java builder loop converge with the Python comprehension. A different
    // contribution stays a hard negative.
    let i = Interner::new();
    let py_comp = "def f(xs):\n    return [x * x for x in xs]\n";
    let java_build = "import java.util.*;\nclass C { static List<Integer> f(int[] xs) { List<Integer> out = new ArrayList<>(); for (int x : xs) { out.add(x * x); } return out; } }\n";
    let java_qualified_build = "class C { static java.util.List<Integer> f(int[] xs) { java.util.List<Integer> out = new java.util.ArrayList<>(); for (int x : xs) { out.add(x * x); } return out; } }\n";
    let java_build_diff = "import java.util.*;\nclass C { static List<Integer> f(int[] xs) { List<Integer> out = new ArrayList<>(); for (int x : xs) { out.add(x + 1); } return out; } }\n";
    let java_unimported_arraylist = "class C { static Object f(int[] xs) { var out = new ArrayList<Integer>(); for (int x : xs) { out.add(x * x); } return out; } }\n";
    let java_shadowed_arraylist = "import java.util.*;\nclass ArrayList<T> { void add(T value) {} }\nclass C { static ArrayList<Integer> f(int[] xs) { ArrayList<Integer> out = new ArrayList<>(); for (int x : xs) { out.add(x * x); } return out; } }\n";
    let java_conflicting_arraylist_import = "import other.ArrayList;\nimport java.util.*;\nclass C { static Object f(int[] xs) { var out = new ArrayList<Integer>(); for (int x : xs) { out.add(x * x); } return out; } }\n";
    let java_conflicting_exact_arraylist_import = "import other.ArrayList;\nimport java.util.ArrayList;\nclass C { static Object f(int[] xs) { var out = new ArrayList<Integer>(); for (int x : xs) { out.add(x * x); } return out; } }\n";
    let comp_fp = value_fp(&i, py_comp, Lang::Python);
    assert_eq!(
        comp_fp,
        value_fp(&i, java_build, Lang::Java),
        "java ArrayList+add builder loop must converge with the python comprehension"
    );
    assert_eq!(
        comp_fp,
        value_fp(&i, java_qualified_build, Lang::Java),
        "fully-qualified java.util.ArrayList must not require a separate import proof"
    );
    assert_ne!(
        comp_fp,
        value_fp(&i, java_build_diff, Lang::Java),
        "a different per-element contribution must stay distinct"
    );
    assert_ne!(
        comp_fp,
        value_fp(&i, java_unimported_arraylist, Lang::Java),
        "simple ArrayList constructors need a java.util import proof before exact builder seeding"
    );
    assert_ne!(
        comp_fp,
        value_fp(&i, java_shadowed_arraylist, Lang::Java),
        "a local ArrayList type must not mint the java.util empty-list builder seed"
    );
    assert_ne!(
        comp_fp,
        value_fp(&i, java_conflicting_arraylist_import, Lang::Java),
        "a conflicting explicit ArrayList import must override java.util wildcard proof"
    );
    assert_ne!(
        comp_fp,
        value_fp(&i, java_conflicting_exact_arraylist_import, Lang::Java),
        "a conflicting explicit ArrayList import must close even an exact java.util import proof"
    );
}

#[test]
fn java_static_final_map_field_converges_with_inline_factory_lookup() {
    let i = Interner::new();
    let inline = "import java.util.Map;\n\nclass JavaMapOf {\n  static int lookup(String key, String other) {\n    return Map.of(\"red\", 1, \"blue\", 2).getOrDefault(key, 0);\n  }\n}\n";
    let field = "import java.util.Map;\n\nclass JavaModuleMap {\n  static final Map<String, Integer> LOOKUP = Map.of(\"red\", 1, \"blue\", 2);\n\n  static int lookup(String key, String other) {\n    return LOOKUP.getOrDefault(key, 0);\n  }\n}\n";
    assert_eq!(
        value_fp(&i, inline, Lang::Java),
        value_fp(&i, field, Lang::Java),
        "Java static final literal map fields must seed method-level map proof"
    );
}

#[test]
fn go_functional_append_builder_loop_converges_with_comprehension() {
    // Go builds a list with `out := []T{}; for … { out = append(out, e) }` — a FUNCTIONAL
    // append (reassignment), not the effect-form `out.append(e)` of Python/JS. Recognizing the
    // `r = append(r, e)` reassign as the same per-element `Map` build (and excluding the builder
    // var from numeric loop-carried seeding) makes the Go builder loop converge with the Python
    // comprehension. The changed-contribution form stays a hard negative.
    let i = Interner::new();
    let py_comp = "def f(xs):\n    return [x * x for x in xs]\n";
    let go_build = "package p\nfunc f(xs []int) []int {\n\tout := []int{}\n\tfor _, x := range xs {\n\t\tout = append(out, x*x)\n\t}\n\treturn out\n}\n";
    let go_build_diff = "package p\nfunc f(xs []int) []int {\n\tout := []int{}\n\tfor _, x := range xs {\n\t\tout = append(out, x+1)\n\t}\n\treturn out\n}\n";
    let comp_fp = value_fp(&i, py_comp, Lang::Python);
    assert_eq!(
        comp_fp,
        value_fp(&i, go_build, Lang::Go),
        "go functional-append builder loop must converge with the python comprehension"
    );
    assert_ne!(
        comp_fp,
        value_fp(&i, go_build_diff, Lang::Go),
        "a different per-element contribution must stay distinct"
    );
}

#[test]
fn interprocedural_inline_handles_multi_statement_pure_helper() {
    // A pure helper with LOCAL temporaries (`sub = …; disc = …; return sub - disc`) inlines just
    // like a single-`return` one — the multi-statement body is a pure value computation, so a
    // caller of it converges with the same logic written inline. An effectful (field-writing)
    // helper still does NOT inline (its effect can't be dropped).
    let i = Interner::new();
    let helper = "def base(item):\n    sub = item.price * item.qty\n    disc = sub * 0.1\n    return sub - disc\n\ndef total(item):\n    return base(item) + 5\n";
    let inline = "def total(item):\n    sub = item.price * item.qty\n    disc = sub * 0.1\n    return (sub - disc) + 5\n";
    assert_eq!(
        value_fp_named(&i, helper, Lang::Python, "total"),
        value_fp_named(&i, inline, Lang::Python, "total"),
        "a multi-statement pure helper must inline like a single-return one",
    );
    let eff = "def bump(box, x):\n    box.count = box.count + 1\n    return x * 2\n\ndef use(box, x):\n    return bump(box, x) + 5\n";
    let eff_free = "def use(box, x):\n    return x * 2 + 5\n";
    assert_ne!(
        value_fp_named(&i, eff, Lang::Python, "use"),
        value_fp_named(&i, eff_free, Lang::Python, "use"),
        "an effectful (field-writing) helper must not be inlined",
    );
}

#[test]
fn interprocedural_pure_inline_converges_extract_method() {
    // A function whose body inlines a computation converges with one that calls a PURE extracted
    // helper for it — `f(args)` is β-reduced to the helper's body (interprocedural summary), the
    // extract-method equivalence. An EFFECTFUL helper (a field write the value-only inline would
    // drop) is NOT inlined, so its caller stays distinct from the effect-free version.
    let i = Interner::new();
    let inline = "def price(item):\n    return item.price * item.qty * (1 + 0.1)\n";
    let helper = "def base(item):\n    return item.price * item.qty\n\ndef price(item):\n    return base(item) * (1 + 0.1)\n";
    assert_eq!(
        value_fp_named(&i, inline, Lang::Python, "price"),
        value_fp_named(&i, helper, Lang::Python, "price"),
        "calling a pure extracted helper must converge with the inlined computation"
    );
    let eff_helper = "def bump(box, x):\n    box.count = box.count + 1\n    return x * 2\n\ndef use(box, x):\n    return bump(box, x) + 5\n";
    let eff_free = "def use(box, x):\n    return x * 2 + 5\n";
    assert_ne!(
        value_fp_named(&i, eff_helper, Lang::Python, "use"),
        value_fp_named(&i, eff_free, Lang::Python, "use"),
        "an effectful (field-writing) helper must not be inlined — its effect can't be dropped"
    );
}

#[test]
fn generalized_inline_loop_accumulator_helper_converges() {
    // The flagship interprocedural case: `foo` calling a LOOP-ACCUMULATOR helper must
    // converge with `foo` written with the loop inline. The straight-line-only inline
    // whitelist could never admit `bar`; the generalized admission evaluates the body
    // through the ordinary statement processor (the loop becomes the same `Reduce`)
    // behind the sink fence.
    let i = Interner::new();
    let helper = "function foo(values) {\n    const total = bar(values)\n    return total / values.length\n}\n\nfunction bar(values) {\n    let result = 0\n    for(let i = 0; i < values.length; i++) {\n        result += values[i]\n    }\n    return result\n}\n";
    let inline = "function foo(values) {\n    let total = 0\n    for(let i = 0; i < values.length; i++) {\n        total += values[i]\n    }\n    return total / values.length\n}\n";
    assert_eq!(
        value_fp_named(&i, helper, Lang::JavaScript, "foo"),
        value_fp_named(&i, inline, Lang::JavaScript, "foo"),
        "a pure loop-accumulator helper must inline and converge with the inline form",
    );
}

#[test]
fn generalized_inline_builder_loop_helper_converges_with_comprehension_caller() {
    // A helper whose body is a pure list-BUILDER loop inlines into its caller and
    // converges with a caller using the comprehension form directly — composing the
    // interprocedural axis with the loop↔comprehension Type-4 axis.
    let i = Interner::new();
    let helper = "def squares(xs):\n    out = []\n    for x in xs:\n        out.append(x * x)\n    return out\n\ndef use(xs):\n    return len(squares(xs))\n";
    let comp = "def use(xs):\n    return len([x * x for x in xs])\n";
    assert_eq!(
        value_fp_named(&i, helper, Lang::Python, "use"),
        value_fp_named(&i, comp, Lang::Python, "use"),
        "a pure builder-loop helper must inline and converge with the comprehension",
    );
}

#[test]
fn generalized_inline_guard_clause_helper_converges_with_ternary() {
    // A guard-clause helper (`if c: return a` then an unconditional return) folds its
    // captured returns to the same `Phi` a ternary builds, so the caller converges with
    // the ternary form written inline.
    let i = Interner::new();
    let helper = "def clamp0(x):\n    if x < 0:\n        return 0\n    return x\n\ndef use(x):\n    return clamp0(x) * 2\n";
    let ternary = "def use(x):\n    return (0 if x < 0 else x) * 2\n";
    assert_eq!(
        value_fp_named(&i, helper, Lang::Python, "use"),
        value_fp_named(&i, ternary, Lang::Python, "use"),
        "a guard-clause helper must fold to the ternary's Phi and converge",
    );
}

#[test]
fn generalized_inline_exhaustive_if_else_tail_converges() {
    // A body ending in an exhaustive `if/else` (both arms return) is admitted: the two
    // captured guarded returns are complementary and fold to one `Phi`.
    let i = Interner::new();
    let helper = "def pick(a, b, flag):\n    if flag:\n        return a\n    else:\n        return b\n\ndef use(a, b, flag):\n    return pick(a, b, flag) + 1\n";
    let ternary = "def use(a, b, flag):\n    return (a if flag else b) + 1\n";
    assert_eq!(
        value_fp_named(&i, helper, Lang::Python, "use"),
        value_fp_named(&i, ternary, Lang::Python, "use"),
        "an exhaustive if/else tail must fold to the ternary's Phi and converge",
    );
}

#[test]
fn generalized_inline_nested_pure_helpers_converge() {
    // Pure helpers calling pure helpers inline transitively (bounded by the inline
    // stack), so a two-hop composition converges with the flat form.
    let i = Interner::new();
    let nested = "def double(x):\n    return x * 2\n\ndef double_sum(xs):\n    t = 0\n    for x in xs:\n        t += double(x)\n    return t\n\ndef use(xs):\n    return double_sum(xs) + 1\n";
    let flat = "def use(xs):\n    t = 0\n    for x in xs:\n        t += x * 2\n    return t + 1\n";
    assert_eq!(
        value_fp_named(&i, nested, Lang::Python, "use"),
        value_fp_named(&i, flat, Lang::Python, "use"),
        "nested pure helpers must inline transitively",
    );
}

#[test]
fn generalized_inline_congruence_callers_of_equal_helpers_converge() {
    // Two callers of two DIFFERENT-NAMED but body-identical pure loop helpers converge:
    // inlining keys the call by the callee's semantics, never its name.
    let i = Interner::new();
    let src = "def sum_a(xs):\n    t = 0\n    for x in xs:\n        t += x\n    return t\n\ndef sum_b(ys):\n    t = 0\n    for y in ys:\n        t += y\n    return t\n\ndef use_a(xs):\n    return sum_a(xs) / len(xs)\n\ndef use_b(xs):\n    return sum_b(xs) / len(xs)\n";
    assert_eq!(
        value_fp_named(&i, src, Lang::Python, "use_a"),
        value_fp_named(&i, src, Lang::Python, "use_b"),
        "callers of behaviorally-equal helpers must converge regardless of helper name",
    );
}

#[test]
fn generalized_inline_rejects_effectful_loop_helper() {
    // A loop helper that ALSO appends to a caller-supplied list is NOT pure — inlining
    // it value-only would drop the append. The sink fence rejects it, so the caller must
    // NOT converge with the pure loop written inline (which is exactly the fingerprint a
    // broken fence would produce).
    let i = Interner::new();
    let eff = "def log_sum(xs, log):\n    t = 0\n    for x in xs:\n        log.append(x)\n        t += x\n    return t\n\ndef use(xs, log):\n    return log_sum(xs, log) / len(xs)\n";
    let pure_inline =
        "def use(xs, log):\n    t = 0\n    for x in xs:\n        t += x\n    return t / len(xs)\n";
    assert_ne!(
        value_fp_named(&i, eff, Lang::Python, "use"),
        value_fp_named(&i, pure_inline, Lang::Python, "use"),
        "an effectful loop helper must not inline — dropping its append is a false merge",
    );
}

#[test]
fn generalized_inline_rejects_recursive_helpers_without_hanging() {
    // Self- and mutual recursion fail closed (the inline stack excludes re-entry) and
    // must neither hang nor panic; the recursive caller stays distinct from a
    // non-recursive one.
    let i = Interner::new();
    let self_rec = "def fact(n):\n    if n <= 1:\n        return 1\n    return fact(n - 1) * n\n\ndef use(n):\n    return fact(n) + 1\n";
    let mutual = "def even(n):\n    if n == 0:\n        return True\n    return odd(n - 1)\n\ndef odd(n):\n    if n == 0:\n        return False\n    return even(n - 1)\n\ndef use(n):\n    return even(n)\n";
    let trivial = "def use(n):\n    return n + 1\n";
    assert_ne!(
        value_fp_named(&i, self_rec, Lang::Python, "use"),
        value_fp_named(&i, trivial, Lang::Python, "use"),
        "a self-recursive helper caller stays distinct (fail-closed, no hang)",
    );
    assert_ne!(
        value_fp_named(&i, mutual, Lang::Python, "use"),
        value_fp_named(&i, trivial, Lang::Python, "use"),
        "a mutually-recursive helper caller stays distinct (fail-closed, no hang)",
    );
}

#[test]
fn sub_dag_anchor_shared_when_units_share_a_heavy_computation() {
    // Two functions that share a large sub-computation (subtotal/tax/shipping/grand) but differ
    // elsewhere are a PARTIAL / sub-DAG clone — they share a heavy anchor (an extractable common
    // computation) even though whole-unit fingerprints differ. If the shared computation itself
    // diverges (different shipping rule / sign), no anchor is shared.
    let i = Interner::new();
    let a = "function a(items) {\n  const subtotal = items.map(x => x.price * x.qty).reduce((s, x) => s + x, 0);\n  const tax = subtotal * rate;\n  const ship = subtotal > 100 ? 0 : 15;\n  const grand = subtotal + tax + ship;\n  renderInvoice(grand);\n  return grand;\n}\n";
    let b = "function b(items) {\n  const subtotal = items.map(x => x.price * x.qty).reduce((s, x) => s + x, 0);\n  const tax = subtotal * rate;\n  const ship = subtotal > 100 ? 0 : 15;\n  const grand = subtotal + tax + ship;\n  saveOrder(grand);\n  notify(grand);\n}\n";
    let c = "function c(items) {\n  const subtotal = items.map(x => x.price * x.qty).reduce((s, x) => s + x, 0);\n  const tax = subtotal * rate;\n  const ship = subtotal > 200 ? 0 : 25;\n  const grand = subtotal - tax + ship;\n  saveOrder(grand);\n  notify(grand);\n}\n";
    let aa = value_anchors(&i, a, Lang::TypeScript);
    assert!(
        !aa.is_empty(),
        "a heavy shared computation must yield an anchor"
    );
    assert!(
        shares_any(&aa, &value_anchors(&i, b, Lang::TypeScript)),
        "units sharing a heavy computation must share an anchor (partial clone)"
    );
    assert!(
        !shares_any(&aa, &value_anchors(&i, c, Lang::TypeScript)),
        "when the shared computation diverges, no anchor is shared"
    );
}

#[test]
fn promise_then_chain_stays_opaque_without_receiver_proof() {
    // A `.then` name is not itself proof of Promise/thenable semantics. Until the frontend or a
    // semantic pack carries a resolved Promise-like receiver proof, exact value fingerprints must
    // keep `.then` opaque rather than beta-reducing an arbitrary user method.
    let i = Interner::new();
    let await_form = "function f(id) {\n  const r = await db.get(id);\n  return r.x + 1;\n}\n";
    let then_form = "function f(id) {\n  return db.get(id).then(r => r.x + 1);\n}\n";
    let then_diff = "function f(id) {\n  return db.get(id).then(r => r.y - 1);\n}\n";
    let av = value_fp(&i, await_form, Lang::TypeScript);
    assert_ne!(
        av,
        value_fp(&i, then_form, Lang::TypeScript),
        "a `.then` continuation must not converge with await without receiver proof"
    );
    assert_ne!(
        av,
        value_fp(&i, then_diff, Lang::TypeScript),
        "a different continuation expression must stay distinct"
    );
}

#[test]
fn proven_promise_then_chains_converge_without_sync_erasure() {
    let i = Interner::new();
    let chained =
        "function f() {\n  return Promise.resolve(1).then(x => x + 1).then(z => z * 2);\n}\n";
    let single = "function f() {\n  return Promise.resolve(1).then(x => (x + 1) * 2);\n}\n";
    assert_eq!(
        value_fp(&i, chained, Lang::TypeScript),
        value_fp(&i, single, Lang::TypeScript),
        "supported Promise.resolve(...).then(...) chains should converge through proven Promise receiver evidence"
    );

    let promise_return = "function f() {\n  return Promise.resolve(1).then(x => x + 1);\n}\n";
    let sync_return = "function f() {\n  return 1 + 1;\n}\n";
    assert_ne!(
        value_fp(&i, promise_return, Lang::TypeScript),
        value_fp(&i, sync_return, Lang::TypeScript),
        "Promise continuations keep a Promise boundary and must not converge with synchronous payloads"
    );
}

#[test]
fn go_slice_literal_converges_with_array_but_struct_stays_distinct() {
    // A Go slice literal `[]int{1,2,3}` is an ordered sequence — it converges with a Python
    // list / JS array. A Go STRUCT literal `Point{1,2,3}` is a record, NOT a collection, and
    // must stay distinct (no `Point{1,2,3}` ≡ `[1,2,3]` false merge). Tagging composite
    // literals by type (slice/array → `array`, map → `composite_literal`, struct → `go_struct`)
    // removes the old blanket tag that collapsed all three to one value.
    let i = Interner::new();
    let py_list = "def f():\n    return [1, 2, 3]\n";
    let go_slice = "package p\nfunc f() []int { return []int{1, 2, 3} }\n";
    let go_struct =
        "package p\ntype Point struct{ x, y, z int }\nfunc f() Point { return Point{1, 2, 3} }\n";
    let list_fp = value_fp(&i, py_list, Lang::Python);
    assert_eq!(
        list_fp,
        value_fp(&i, go_slice, Lang::Go),
        "go slice literal must converge with the python list literal"
    );
    assert_ne!(
        list_fp,
        value_fp(&i, go_struct, Lang::Go),
        "a go struct literal must stay distinct from a list (it is a record, not a collection)"
    );
}

#[test]
fn seq_surface_contracts_keep_maps_out_of_collection_membership() {
    let i = Interner::new();
    let py_membership = "def f(x):\n    return x in [\"red\", \"blue\"]\n";
    let go_slice_membership = "package p\nimport \"slices\"\nfunc f(x string) bool { return slices.Contains([]string{\"red\", \"blue\"}, x) }\n";
    let go_map_as_slice_membership = "package p\nimport \"slices\"\nfunc f(x string) bool { return slices.Contains(map[string]int{\"red\": 0, \"blue\": 0}, x) }\n";
    let go_zero_map_lookup =
        "package p\nfunc f(x string) int { return map[string]int{\"red\": 0, \"blue\": 0}[x] }\n";
    let go_empty_map = "package p\nfunc f() map[string]int { return map[string]int{} }\n";
    let go_empty_slice = "package p\nfunc f() []int { return []int{} }\n";

    let membership = value_fp(&i, py_membership, Lang::Python);
    assert_eq!(
        membership,
        value_fp(&i, go_slice_membership, Lang::Go),
        "Go slices.Contains over a slice literal is a proven collection membership"
    );
    assert_ne!(
        membership,
        value_fp(&i, go_map_as_slice_membership, Lang::Go),
        "Go map composite literals must not leak into collection membership semantics"
    );
    assert_ne!(
        value_fp(&i, go_zero_map_lookup, Lang::Go),
        value_fp(&i, go_map_as_slice_membership, Lang::Go),
        "the supported Go zero-map lookup contract is separate from collection membership"
    );
    assert_ne!(
        value_fp(&i, go_empty_map, Lang::Go),
        value_fp(&i, go_empty_slice, Lang::Go),
        "empty Go map literals must not fall back to the empty collection value tag"
    );
}

#[test]
fn js_object_length_and_computed_keys_stay_outside_exact_collection_contracts() {
    let i = Interner::new();
    let py_dict_len = "def f():\n    return len({\"length\": 99, \"a\": 0})\n";
    let js_object_length = "function f() { return ({ length: 99, a: 0 }).length; }";
    let py_object = "def f():\n    return {\"red\": 1, \"blue\": 2}\n";
    let js_static_object = "function f() { return { red: 1, blue: 2 }; }";
    let js_computed_object = "function f(k) { return { [k]: 1, blue: 2 }; }";

    assert_ne!(
        value_fp(&i, py_dict_len, Lang::Python),
        value_fp(&i, js_object_length, Lang::JavaScript),
        "JS object `.length` is a property read, not map/dict cardinality"
    );
    assert_eq!(
        value_fp(&i, py_object, Lang::Python),
        value_fp(&i, js_static_object, Lang::JavaScript),
        "static JS object keys remain an exact map/object literal surface"
    );
    assert_ne!(
        value_fp(&i, js_static_object, Lang::JavaScript),
        value_fp(&i, js_computed_object, Lang::JavaScript),
        "computed object keys need a future key-evaluation contract before exact map semantics"
    );
}

#[test]
fn method_recursion_requires_explicit_call_target_evidence() {
    // Method bodies are not admitted from a bare same-name call. Java static/private/final
    // dispatch and Ruby top-level method lookup need exact source/pack target evidence before
    // recursion→iteration can treat `fac(...)` as direct self-recursion. Free-function recursion
    // remains covered by `rust_recursion_converges_with_iteration_via_return_unwrap`.
    let i = Interner::new();
    let py_loop = "def fac(n):\n    acc = 1\n    while n != 0:\n        acc = acc * n\n        n = n - 1\n    return acc\n";
    let java_rec =
        "class C { static int fac(int n) { if (n == 0) { return 1; } return n * fac(n - 1); } }";
    let ruby_rec = "def fac(n)\n  return 1 if n == 0\n  n * fac(n - 1)\nend\n";
    let sum_loop = "def g(n):\n    acc = 0\n    while n != 0:\n        acc = acc + n\n        n = n - 1\n    return acc\n";
    let fold = value_fp(&i, py_loop, Lang::Python);
    assert_ne!(
        fold,
        value_fp_named(&i, java_rec, Lang::Java, "fac"),
        "java method recursion must stay closed without direct call-target evidence"
    );
    assert_ne!(
        fold,
        value_fp_named(&i, ruby_rec, Lang::Ruby, "fac"),
        "ruby method recursion must stay closed without direct call-target evidence"
    );
    assert_ne!(
        fold,
        value_fp(&i, sum_loop, Lang::Python),
        "the sum monoid (acc + n, base 0) must stay a hard negative"
    );
}

#[test]
fn flatmap_identity_converges_with_explicit_flatten_but_inner_map_stays_closed() {
    // `xss.flatMap(xs => xs)` is a flatten semantically and converges with an explicit nested
    // builder loop. The inner `xs.map(...)` form still stays closed until the kernel carries
    // nested element collection proofs for `xs`.
    let i = Interner::new();
    let identity = "function f(xss: number[][]): number[] { return xss.flatMap(xs => xs); }";
    let inner_map =
        "function f(xss: number[][]): number[] { return xss.flatMap(xs => xs.map(y => y)); }";
    let builder = "function f(xss: number[][]): number[] { const out: number[] = []; for (const xs of xss) { for (const y of xs) { out.push(y); } } return out; }";
    let changed =
        "function f(xss: number[][]): number[] { return xss.flatMap(xs => xs.map(y => y + 1)); }";
    let id_fp = value_fp(&i, identity, Lang::TypeScript);
    assert_ne!(
        id_fp,
        value_fp(&i, inner_map, Lang::TypeScript),
        "inner map must stay closed without nested element collection proof"
    );
    assert_eq!(
        id_fp,
        value_fp(&i, builder, Lang::TypeScript),
        "flatMap(id) should converge with the explicit nested builder loop"
    );
    assert_ne!(
        id_fp,
        value_fp(&i, changed, Lang::TypeScript),
        "a changed inner element (y + 1) must stay distinct (hard negative)"
    );
}

#[test]
fn ruby_select_reduce_converges_with_guarded_loop() {
    // Ruby `select { p }.reduce(init) { |a, x| ... }` is the same filtered fold only when
    // `xs` is proven to be Ruby's collection protocol. With no receiver proof in this snippet,
    // the method chain stays closed. The changed seed remains a hard negative.
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
    assert_ne!(sum_fp, value_fp(&i, reduce_rb, Lang::Ruby));
    assert_ne!(
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
    assert_ne!(any_fp, value_fp(&i, any_call, Lang::Ruby));
    assert_ne!(
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
    let aliased_prod_py =
        "import math as m\n\ndef f(xs):\n    return m.prod((x for x in xs if x > 0), start=1)\n";
    let bad_seed =
        "import math\n\ndef f(xs):\n    return math.prod((x for x in xs if x > 0), start=2)\n";
    let missing_import = "def f(xs):\n    return math.prod((x for x in xs if x > 0), start=1)\n";
    let shadowed_math = "import math\nmath = object()\n\ndef f(xs):\n    return math.prod((x for x in xs if x > 0), start=1)\n";
    let parameter_shadowed_math =
        "import math\n\ndef f(xs, math):\n    return math.prod((x for x in xs if x > 0), start=1)\n";
    let local_shadowed_math =
        "import math\n\ndef f(xs):\n    math = object()\n    return math.prod((x for x in xs if x > 0), start=1)\n";
    let loop_fp = value_fp(&i, loop_py, Lang::Python);
    assert_eq!(loop_fp, value_fp(&i, prod_py, Lang::Python));
    assert_eq!(loop_fp, value_fp(&i, aliased_prod_py, Lang::Python));
    assert_ne!(loop_fp, value_fp(&i, bad_seed, Lang::Python));
    assert_ne!(loop_fp, value_fp(&i, missing_import, Lang::Python));
    assert_ne!(loop_fp, value_fp(&i, shadowed_math, Lang::Python));
    assert_ne!(loop_fp, value_fp(&i, parameter_shadowed_math, Lang::Python));
    assert_ne!(loop_fp, value_fp(&i, local_shadowed_math, Lang::Python));
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
    assert_ne!(
        c_fp,
        value_fp(&i, ruby, Lang::Ruby),
        "ruby each must stay closed until receiver/protocol proof exists"
    );
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
    assert_ne!(
        any_fp,
        value_fp(&i, ruby_any, Lang::Ruby),
        "ruby each must stay closed until receiver/protocol proof exists"
    );
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
    let reduce_js =
        "function f(xs: number[]): number { return xs.reduce((best, x) => x > best ? x : best, 0); }";
    let rust_max =
        "fn f(xs: &[i32]) -> i32 { let mut best = 0; for &x in xs { if x > best { best = x; } } best }";
    let rust_fold_max =
        "fn f(xs: &[i32]) -> i32 { xs.iter().copied().fold(0, |best, x| if x > best { x } else { best }) }";
    let py_min = "def f(xs):\n    best = 0\n    for x in xs:\n        if x < best:\n            best = x\n    return best\n";
    let reduce_py =
        "import functools\n\ndef f(xs):\n    return functools.reduce(lambda best, x: x if x < best else best, xs, 0)\n";
    let rust_min =
        "fn f(xs: &[i32]) -> i32 { let mut best = 0; for &x in xs { if x < best { best = x; } } best }";
    let rust_fold_min =
        "fn f(xs: &[i32]) -> i32 { xs.iter().copied().fold(0, |best, x| if x < best { x } else { best }) }";
    let bad_min = "def f(xs):\n    best = 0\n    for x in xs:\n        if x < best:\n            best = x\n    return best\n";

    let max_fp = value_fp(&i, py_max, Lang::Python);
    assert_ne!(
        max_fp,
        value_fp(&i, js_max, Lang::JavaScript),
        "untyped JS relational comparison can be string-ordered and must stay closed"
    );
    assert_ne!(
        max_fp,
        value_fp(&i, reduce_js, Lang::TypeScript),
        "TypeScript number[] callback elements are not yet numeric proof for relational predicates"
    );
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
    assert_ne!(
        fp,
        value_fp(&i, ruby_each, Lang::Ruby),
        "ruby each_with_index must stay closed until receiver/protocol proof exists"
    );
    assert_ne!(fp, value_fp(&i, ruby_while, Lang::Ruby));
    assert_eq!(
        return_fp(&i, py_loop, Lang::Python),
        return_fp(&i, java_for, Lang::Java)
    );
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
    // Integer `abs(x)` and the `x if x>=0 else -x` idiom canonicalize to one Abs value (§AI).
    let i = Interner::new();
    let call = "def f(x: int):\n    return abs(x)\n";
    let tern = "def g(x: int):\n    return x if x >= 0 else -x\n";
    assert_eq!(
        value_fp(&i, call, Lang::Python),
        value_fp(&i, tern, Lang::Python),
        "integer abs(x) should converge with the conditional-negate idiom"
    );
    let untyped_call = "def f(x):\n    return abs(x)\n";
    let untyped_tern = "def g(x):\n    return x if x >= 0 else -x\n";
    assert_ne!(
        value_fp(&i, untyped_call, Lang::Python),
        value_fp(&i, untyped_tern, Lang::Python),
        "untyped Python abs must not merge with the signed-zero-sensitive ternary idiom"
    );
}

#[test]
fn scalar_abs_axis_converges_with_unused_alternate_param() {
    let i = Interner::new();
    let call = "def f(value: int, other: int):\n    return abs(value)\n";
    let tern = "def g(value: int, other: int):\n    return value if value >= 0 else -value\n";
    assert_eq!(
        value_fp(&i, call, Lang::Python),
        value_fp(&i, tern, Lang::Python)
    );
}

#[test]
fn scalar_abs_builtins_converge_cross_language_with_shadow_boundary() {
    let i = Interner::new();
    let py = "def f(value: int, other: int):\n    magnitude = value if value >= 0 else -value\n    return magnitude + other\n";
    let js =
        "function f(value, other) { const magnitude = Math.abs(value); return magnitude + other; }";
    let ts = "function f(value: number, other: number): number { const magnitude = Math.abs(value); return magnitude + other; }";
    let go = "package p\n\nimport \"math\"\n\nfunc F(value float64, other float64) float64 { magnitude := math.Abs(value); return magnitude + other }\n";
    let java = "class C { static int f(int value, int other) { int magnitude = Math.abs(value); return magnitude + other; } }\n";
    let java_double = "class C { static double f(double value, double other) { double magnitude = Math.abs(value); return magnitude + other; } }\n";
    let ruby_abs = "def f(value, other)\n  magnitude = value.abs\n  magnitude + other\nend\n";
    let rust_abs =
        "pub fn f(value: i64, other: i64) -> i64 { let magnitude = value.abs(); magnitude + other }\n";
    let shadowed_js = "function f(Math, value, other) { const magnitude = Math.abs(value); return magnitude + other; }";
    let local_shadowed_js = "function f(value, other) { const Math = { abs: function(_value) { return 0; } }; const magnitude = Math.abs(value); return magnitude + other; }";
    let java_shadowed_math_param = "class Math { int abs(int value) { return 0; } }\nclass C { static int f(Math Math, int value, int other) { int magnitude = Math.abs(value); return magnitude + other; } }\n";
    let ts_number_method_abs = "function f(value: number, other: number): number { const magnitude = value.abs(); return magnitude + other; }";
    let rust_float_abs = "pub fn f(value: f64, other: f64) -> f64 { let magnitude = value.abs(); magnitude + other }\n";
    let custom_rust_abs = "struct Wrap(i64);\nimpl Wrap { fn abs(&self) -> i64 { 0 } }\npub fn f(value: Wrap) -> i64 { let magnitude = value.abs(); magnitude + 1 }\n";
    let fp = value_fp(&i, py, Lang::Python);
    assert_ne!(fp, value_fp(&i, js, Lang::JavaScript));
    assert_ne!(
        fp,
        value_fp(&i, ts, Lang::TypeScript),
        "TypeScript Math.abs over number keeps the signed-zero boundary closed"
    );
    assert_ne!(
        fp,
        value_fp(&i, go, Lang::Go),
        "Go math.Abs over float64 keeps the signed-zero boundary closed"
    );
    assert_eq!(fp, value_fp(&i, java, Lang::Java));
    assert_ne!(
        fp,
        value_fp(&i, java_double, Lang::Java),
        "Java Math.abs over double keeps the signed-zero boundary closed"
    );
    assert_ne!(fp, value_fp(&i, ruby_abs, Lang::Ruby));
    assert_eq!(fp, value_fp(&i, rust_abs, Lang::Rust));
    assert_ne!(fp, value_fp(&i, shadowed_js, Lang::JavaScript));
    assert_ne!(fp, value_fp(&i, local_shadowed_js, Lang::JavaScript));
    assert_ne!(
        fp,
        value_fp_named(&i, java_shadowed_math_param, Lang::Java, "f")
    );
    assert_ne!(fp, value_fp(&i, ts_number_method_abs, Lang::TypeScript));
    assert_ne!(fp, value_fp(&i, rust_float_abs, Lang::Rust));
    assert_ne!(fp, value_fp(&i, custom_rust_abs, Lang::Rust));
}

#[test]
fn scalar_minmax_builtins_converge_cross_language() {
    let i = Interner::new();
    let py_min = "def f(left, right, other):\n    selected = left if left <= right else right\n    return selected + other\n";
    let py_min_call =
        "def f(left, right, other):\n    selected = min(left, right)\n    return selected + other\n";
    let js_min = "function f(left, right, other) { const selected = Math.min(left, right); return selected + other; }";
    let ts_min = "function f(left: number, right: number, other: number): number { const selected = Math.min(left, right); return selected + other; }";
    let js_free_min =
        "function f(left, right, other) { const selected = min(left, right); return selected + other; }";
    let go_min = "package p\n\nimport \"math\"\n\nfunc F(left float64, right float64, other float64) float64 { selected := math.Min(left, right); return selected + other }\n";
    let java_min = "class C { static int f(int left, int right, int other) { int selected = Math.min(left, right); return selected + other; } }\n";
    let java_double_min = "class C { static double f(double left, double right, double other) { double selected = Math.min(left, right); return selected + other; } }\n";
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

    let fp = value_fp(&i, py_min, Lang::Python);
    assert_eq!(fp, value_fp(&i, py_min_call, Lang::Python));
    assert_ne!(
        fp,
        value_fp(&i, js_min, Lang::JavaScript),
        "JS Math.min returns NaN when any argument is NaN, unlike the ternary min idiom"
    );
    assert_ne!(
        fp,
        value_fp(&i, ts_min, Lang::TypeScript),
        "TypeScript Math.min over number keeps the same NaN boundary as JS"
    );
    assert_ne!(fp, value_fp(&i, js_free_min, Lang::JavaScript));
    assert_ne!(
        fp,
        value_fp(&i, go_min, Lang::Go),
        "Go math.Min is a float64 API and keeps the NaN boundary closed"
    );
    assert_eq!(fp, value_fp(&i, java_min, Lang::Java));
    assert_ne!(
        fp,
        value_fp(&i, java_double_min, Lang::Java),
        "Java Math.min over double keeps the NaN boundary closed"
    );
    assert_ne!(fp, value_fp(&i, c_min, Lang::C));
    assert_ne!(fp, value_fp(&i, ruby_min, Lang::Ruby));
    assert_eq!(fp, value_fp(&i, rust_min, Lang::Rust));
    assert_ne!(fp, value_fp(&i, py_max, Lang::Python));
    assert_ne!(
        value_fp(&i, py_max, Lang::Python),
        value_fp(&i, ruby_max, Lang::Ruby)
    );
    assert_eq!(
        value_fp(&i, py_max, Lang::Python),
        value_fp(&i, rust_max, Lang::Rust)
    );
    assert_ne!(fp, value_fp(&i, py_wrong_value, Lang::Python));
}

#[test]
fn scalar_minmax_builtins_respect_shadow_boundaries() {
    let i = Interner::new();
    let py_min = "def f(left, right, other):\n    selected = left if left <= right else right\n    return selected + other\n";
    let py_max = "def f(left, right, other):\n    selected = left if left >= right else right\n    return selected + other\n";
    let py_shadowed_min =
        "def min(_left, _right):\n    return 0\n\ndef f(left, right, other):\n    selected = min(left, right)\n    return selected + other\n";
    let py_local_shadowed_min =
        "def f(left, right, other):\n    min = lambda _left, _right: 0\n    selected = min(left, right)\n    return selected + other\n";
    let shadowed_js = "function f(left, right, other) { const Math = { min: function(_left, _right) { return 0; } }; const selected = Math.min(left, right); return selected + other; }";
    let destructured_shadowed_js = "function f(scope, left, right, other) { const { Math } = scope; const selected = Math.min(left, right); return selected + other; }";
    let java_shadowed_math_type = "class C { static int f(int left, int right, int other) { int selected = Math.min(left, right); return selected + other; } }\nclass Math { static int min(int left, int right) { return 0; } }\n";
    let ts_number_method_min = "function f(left: number, right: number, other: number): number { const selected = left.min(right); return selected + other; }";
    let rust_float_min = "pub fn f(left: f64, right: f64, other: f64) -> f64 { let selected = left.min(right); selected + other }\n";
    let custom_rust_min = "struct Wrap(i64);\nimpl Wrap { fn min(&self, _right: i64) -> i64 { 0 } }\npub fn f(left: Wrap, right: i64, other: i64) -> i64 { let selected = left.min(right); selected + other }\n";
    let custom_rust_max = "struct Wrap(i64);\nimpl Wrap { fn max(&self, _right: i64) -> i64 { 0 } }\npub fn f(left: Wrap, right: i64, other: i64) -> i64 { let selected = left.max(right); selected + other }\n";

    let fp = value_fp(&i, py_min, Lang::Python);
    assert_ne!(fp, value_fp(&i, py_shadowed_min, Lang::Python));
    assert_ne!(fp, value_fp(&i, py_local_shadowed_min, Lang::Python));
    assert_ne!(fp, value_fp(&i, shadowed_js, Lang::JavaScript));
    assert_ne!(fp, value_fp(&i, destructured_shadowed_js, Lang::JavaScript));
    assert_ne!(
        fp,
        value_fp_named(&i, java_shadowed_math_type, Lang::Java, "f")
    );
    assert_ne!(fp, value_fp(&i, ts_number_method_min, Lang::TypeScript));
    assert_ne!(fp, value_fp(&i, rust_float_min, Lang::Rust));
    assert_ne!(fp, value_fp(&i, custom_rust_min, Lang::Rust));
    assert_ne!(
        value_fp(&i, py_max, Lang::Python),
        value_fp(&i, custom_rust_max, Lang::Rust)
    );
}

#[test]
fn numeric_clamp_minmax_compositions_require_bound_proof() {
    let i = Interner::new();
    let minmax_guarded = "def f(x: int, lo: int, hi: int):\n    if hi < lo:\n        raise 0\n    return min(max(x, lo), hi)\n";
    let maxmin_guarded = "def f(x: int, lo: int, hi: int):\n    if hi < lo:\n        raise 0\n    return max(min(x, hi), lo)\n";
    let minmax_unproven = "def f(x: int, lo: int, hi: int):\n    return min(max(x, lo), hi)\n";
    let maxmin_unproven = "def f(x: int, lo: int, hi: int):\n    return max(min(x, hi), lo)\n";
    let swapped_bounds = "def f(x: int, lo: int, hi: int):\n    if hi < lo:\n        raise 0\n    return min(max(x, hi), lo)\n";
    let float_minmax = "def f(x: float, lo: float, hi: float):\n    if hi < lo:\n        raise 0\n    return min(max(x, lo), hi)\n";
    let float_maxmin = "def f(x: float, lo: float, hi: float):\n    if hi < lo:\n        raise 0\n    return max(min(x, hi), lo)\n";

    let guarded_fp = value_fp(&i, minmax_guarded, Lang::Python);
    assert_eq!(
        guarded_fp,
        value_fp(&i, maxmin_guarded, Lang::Python),
        "proof-backed integer clamp min/max compositions should converge"
    );
    assert_ne!(
        value_fp(&i, minmax_unproven, Lang::Python),
        value_fp(&i, maxmin_unproven, Lang::Python),
        "unproven parameter bound order must not canonicalize"
    );
    assert_ne!(
        guarded_fp,
        value_fp(&i, swapped_bounds, Lang::Python),
        "swapped bounds are not the same clamp"
    );
    assert_ne!(
        value_fp(&i, float_minmax, Lang::Python),
        value_fp(&i, float_maxmin, Lang::Python),
        "float/NaN-sensitive Number domains need a separate proof"
    );
}

#[test]
fn numeric_clamp_surface_bridge_requires_bound_proof() {
    let i = Interner::new();
    let minmax_guarded = "def f(x: int, lo: int, hi: int):\n    if hi < lo:\n        raise 0\n    return min(max(x, lo), hi)\n";
    let ternary_guarded = "def f(x: int, lo: int, hi: int):\n    if hi < lo:\n        raise 0\n    return lo if x < lo else (hi if x > hi else x)\n";
    let ternary_unproven =
        "def f(x: int, lo: int, hi: int):\n    return lo if x < lo else (hi if x > hi else x)\n";
    let ternary_swapped = "def f(x: int, lo: int, hi: int):\n    if hi < lo:\n        raise 0\n    return hi if x < hi else (lo if x > lo else x)\n";
    let float_ternary = "def f(x: float, lo: float, hi: float):\n    if hi < lo:\n        raise 0\n    return lo if x < lo else (hi if x > hi else x)\n";
    let literal_minmax = "def f(x: int):\n    return min(max(x, 0), 10)\n";
    let rust_literal_clamp = "fn f(x: i64) -> i64 { x.clamp(0, 10) }";
    let rust_guarded_minmax =
        "fn f(x: i64, lo: i64, hi: i64) -> i64 { if hi < lo { panic!(); } x.max(lo).min(hi) }";
    let rust_guarded_clamp =
        "fn f(x: i64, lo: i64, hi: i64) -> i64 { if hi < lo { panic!(); } x.clamp(lo, hi) }";
    let rust_unproven_clamp = "fn f(x: i64, lo: i64, hi: i64) -> i64 { x.clamp(lo, hi) }";
    let ts_number_method_clamp = "function f(x: number): number { return x.clamp(0, 10); }";
    let rust_float_clamp = "fn f(x: f64) -> f64 { x.clamp(0.0, 10.0) }";
    let rust_custom_clamp = "struct Wrap(i64);\nimpl Wrap { fn clamp(&self, _lo: i64, _hi: i64) -> i64 { 0 } }\nfn f(x: Wrap) -> i64 { x.clamp(0, 10) }\n";

    let guarded_fp = value_fp(&i, minmax_guarded, Lang::Python);
    assert_eq!(
        guarded_fp,
        value_fp(&i, ternary_guarded, Lang::Python),
        "proof-backed two-comparison ternary clamp should converge with min/max Clamp"
    );
    assert_eq!(
        value_fp(&i, literal_minmax, Lang::Python),
        value_fp(&i, rust_literal_clamp, Lang::Rust),
        "literal ordered Rust .clamp should converge with literal min/max Clamp"
    );
    assert_eq!(
        value_fp(&i, rust_guarded_minmax, Lang::Rust),
        value_fp(&i, rust_guarded_clamp, Lang::Rust),
        "guarded Rust .clamp should converge with guarded Rust min/max"
    );
    assert_ne!(
        value_fp(&i, ternary_unproven, Lang::Python),
        guarded_fp,
        "unproven parameter bound order must not bridge ternary clamp"
    );
    assert_ne!(
        guarded_fp,
        value_fp(&i, ternary_swapped, Lang::Python),
        "swapped two-comparison clamp bounds are behaviorally different"
    );
    assert_ne!(
        guarded_fp,
        value_fp(&i, float_ternary, Lang::Python),
        "float/NaN-sensitive ternary clamp needs a separate proof"
    );
    assert_ne!(
        value_fp(&i, rust_unproven_clamp, Lang::Rust),
        value_fp(&i, rust_literal_clamp, Lang::Rust),
        "method name alone must not prove parameter bound order"
    );
    assert_ne!(
        value_fp(&i, rust_literal_clamp, Lang::Rust),
        value_fp(&i, ts_number_method_clamp, Lang::TypeScript),
        "numeric method selectors outside a specific language/API contract must stay closed"
    );
    assert_ne!(
        value_fp(&i, rust_literal_clamp, Lang::Rust),
        value_fp(&i, rust_float_clamp, Lang::Rust),
        "float/NaN-sensitive Rust clamp needs a separate contract and proof"
    );
    assert_ne!(
        value_fp(&i, rust_literal_clamp, Lang::Rust),
        value_fp(&i, rust_custom_clamp, Lang::Rust),
        "custom clamp methods must stay outside the numeric library bridge"
    );
}

#[test]
fn conditional_abs_reduction_keeps_unproved_abs_aggregate_closed() {
    // A branch in the per-element contribution is still a single reduction:
    // `total += (x < 0 ? -x : x)` can converge across proven integer surfaces, but
    // Python `sum(abs(x) for x in xs)` stays closed until element-domain proof exists.
    let i = Interner::new();
    let py_loop = "def f(xs: list[int]):\n    total = 0\n    for x in xs:\n        if x < 0:\n            total += -x\n        else:\n            total += x\n    return total\n";
    let py_abs_sum = "def f(xs: list[int]):\n    return sum(abs(x) for x in xs)\n";
    let js_reduce =
        "function f(xs: number[]): number { return xs.reduce((total, x) => total + (x < 0 ? -x : x), 0); }";
    let rust_fold =
        "fn f(xs: &[i32]) -> i32 { xs.iter().copied().fold(0, |total, x| total + if x < 0 { -x } else { x }) }";
    let c_loop = "int f(int *xs, int n) { int total = 0; for (int i = 0; i < n; i++) { if (xs[i] < 0) { total += -xs[i]; } else { total += xs[i]; } } return total; }";
    let bad_sum =
        "def f(xs):\n    total = 0\n    for x in xs:\n        total += x\n    return total\n";
    let fp = value_fp(&i, py_loop, Lang::Python);
    assert_ne!(
        fp,
        value_fp(&i, py_abs_sum, Lang::Python),
        "Python list[int] does not yet prove generator element integer domains for abs"
    );
    assert_ne!(
        fp,
        value_fp(&i, js_reduce, Lang::TypeScript),
        "TypeScript number[] callback elements are not yet numeric proof for relational predicates"
    );
    assert_eq!(
        fp,
        value_fp(&i, rust_fold, Lang::Rust),
        "Rust i32 slice elements provide integer-domain proof for abs-pattern lowering"
    );
    assert_eq!(
        fp,
        value_fp(&i, c_loop, Lang::C),
        "C int pointer elements provide integer-domain proof for abs-pattern lowering"
    );
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
    assert_ne!(
        fp,
        value_fp(&i, greater_first, Lang::C),
        "C pointer-loaded comparator locals do not yet prove total-order guard inversion"
    );
    assert_ne!(
        fp,
        value_fp(&i, ternary, Lang::C),
        "C pointer-loaded comparator ternaries stay closed without total-order proof"
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
    // `+` commutativity is reconciled by the value graph ONLY when an operand is proven
    // non-concat (#283-C): untyped `a + b` could be string concat (`"x"+"y" != "y"+"x"`),
    // so it stays ordered; an int-annotated `+` is provably numeric and still commutes.
    let untyped_a = "def f(a, b):\n    return a + b\n";
    let untyped_b = "def g(a, b):\n    return b + a\n";
    assert_ne!(
        value_fp(&i, untyped_a, Lang::Python),
        value_fp(&i, untyped_b, Lang::Python),
        "untyped a + b must not merge with b + a (string concat is ordered)"
    );
    let typed_a = "def f(a: int, b: int):\n    return a + b\n";
    let typed_b = "def g(a: int, b: int):\n    return b + a\n";
    assert_eq!(
        value_fp(&i, typed_a, Lang::Python),
        value_fp(&i, typed_b, Lang::Python),
        "int-annotated a + b is provably numeric — must still commute"
    );
}

#[test]
fn cross_language_summation_converges() {
    let i = Interner::new();
    let py = "def f(items):\n    total = 0\n    i = 0\n    while i < len(items):\n        total += items[i]\n        i = i + 1\n    return total\n";
    let ts = "function f(items: number[]): number { let total=0; for(let i=0;i<items.length;i++){ total += items[i]; } return total; }";
    let go = "package m\nfunc F(items []int) int {\n\ttotal := 0\n\tfor i := 0; i < len(items); i++ {\n\t\ttotal += items[i]\n\t}\n\treturn total\n}\n";
    let hp = return_fp(&i, py, Lang::Python);
    assert_eq!(hp, return_fp(&i, ts, Lang::TypeScript), "py == ts");
    assert_eq!(hp, return_fp(&i, go, Lang::Go), "py == go");
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
fn continue_guard_unwraps_requires_total_order_proof() {
    // `for x: if c: continue; body` ≡ `for x: if not c: body` needs proof that
    // the inverted guard is a total-order dual. Untyped collection elements stay closed.
    let i = Interner::new();
    let cont = "def f(xs):\n    for x in xs:\n        if x < 0:\n            continue\n        process(x)\n";
    let guard = "def g(xs):\n    for x in xs:\n        if x >= 0:\n            process(x)\n";
    assert_ne!(
        unit_hash(&i, cont, Lang::Python),
        unit_hash(&i, guard, Lang::Python),
        "continue-guard inversion over untyped elements must keep the NaN boundary closed"
    );
}

#[test]
fn branch_orientation_inverts_comparison_canonically() {
    // Untyped order-comparison branch inversion is not sound in the behavioral
    // fingerprint: NaN makes `!(a < b)` differ from `a >= b`.
    let i = Interner::new();
    let lt = "def f(a, b, x, y):\n    if a < b:\n        r = x\n    else:\n        r = y\n    return r\n";
    let ge = "def g(a, b, x, y):\n    if a >= b:\n        r = y\n    else:\n        r = x\n    return r\n";
    assert_ne!(
        value_fp(&i, lt, Lang::Python),
        value_fp(&i, ge, Lang::Python),
        "untyped a<b/else must not merge with a>=b/swapped"
    );

    // Integer-proven operands can still use the total-order dual and should converge.
    let int_lt = "def f(a: int, b: int, x: int, y: int):\n    if a < b:\n        r = x\n    else:\n        r = y\n    return r\n";
    let int_ge = "def g(a: int, b: int, x: int, y: int):\n    if a >= b:\n        r = y\n    else:\n        r = x\n    return r\n";
    assert_eq!(
        value_fp(&i, int_lt, Lang::Python),
        value_fp(&i, int_ge, Lang::Python),
        "integer a<b/else should converge with a>=b/swapped"
    );

    let int_le = "def f(a: int, b: int, x: int, y: int):\n    if a <= b:\n        r = x\n    else:\n        r = y\n    return r\n";
    let int_gt = "def g(a: int, b: int, x: int, y: int):\n    if a > b:\n        r = y\n    else:\n        r = x\n    return r\n";
    assert_eq!(
        value_fp(&i, int_le, Lang::Python),
        value_fp(&i, int_gt, Lang::Python),
        "integer a<=b/else should converge with a>b/swapped"
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
    let js = "function g(items: number[]): number[] {\n  return items.map(x => x * 2);\n}\n";
    assert_eq!(
        unit_hash(&i, py, Lang::Python),
        unit_hash(&i, js, Lang::TypeScript),
        "comprehension == .map (HoF canonicalization)"
    );
}

#[test]
fn find_max_converges_py_rust_but_ruby_each_stays_closed() {
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
    assert_ne!(
        h,
        unit_hash(&i, rb, Lang::Ruby),
        "ruby each must stay closed until receiver/protocol proof exists"
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
fn ruby_each_stays_closed_without_receiver_proof() {
    // Ruby `xs.each { |x| ... }` is just a method call unless a pack proves that `xs` has
    // Ruby Enumerable semantics. The analyzer must not infer a foreach loop from the name `each`.
    let i = Interner::new();
    let rb =
        "def f(items)\n  total = 0\n  items.each do |x|\n    total += x\n  end\n  total\nend\n";
    let py =
        "def f(items):\n    total = 0\n    for x in items:\n        total += x\n    return total\n";
    assert_ne!(
        unit_hash(&i, rb, Lang::Ruby),
        unit_hash(&i, py, Lang::Python),
        "ruby each must stay closed without receiver/protocol proof"
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
fn ruby_test_dsl_block_units_converge_and_keep_literal_boundaries() {
    let i = Interner::new();
    let a = "it 'adds values' do\n  total = price + tax\n  assert_equal total, actual\nend\n";
    let b = "test 'adds values copy' do\n  sum = price + tax\n  assert_equal sum, actual\nend\n";
    assert_eq!(
        value_fp_named(&i, a, Lang::Ruby, "it:adds values"),
        value_fp_named(&i, b, Lang::Ruby, "test:adds values copy"),
        "equivalent Ruby test DSL block bodies should converge as block units"
    );

    let expected_one = "it 'expects one' do\n  assert_equal 1, actual\nend\n";
    let expected_two = "it 'expects two' do\n  assert_equal 2, actual\nend\n";
    assert_ne!(
        value_fp_named(&i, expected_one, Lang::Ruby, "it:expects one"),
        value_fp_named(&i, expected_two, Lang::Ruby, "it:expects two"),
        "different assertion literals must keep Ruby test DSL block units split"
    );
}

#[test]
fn rust_macro_rules_arm_units_survive_feature_extraction() {
    let i = Interner::new();
    let src = r#"
macro_rules! sample {
    ($arg:expr) => {{
        let value = $arg;
        if value > 0 {
            panic!("bad");
        }
        value
    }};
    ($other:expr) => {{
        let value = $other;
        if value > 1 {
            panic!("worse");
        }
        value
    }};
}
"#;
    let il = nose_frontend::lower_source(FileId(0), "sample.rs", src.as_bytes(), Lang::Rust, &i)
        .expect("lower");
    let lowered_names: Vec<_> = il
        .units
        .iter()
        .map(|unit| {
            let span = il.node(unit.root).span;
            (
                unit.kind,
                unit.name.map(|name| i.resolve(name).to_string()),
                span.start_line,
                span.end_line,
                count_nodes(&il, unit.root, None),
                count_nodes(&il, unit.root, Some(nose_il::NodeKind::Raw)),
            )
        })
        .collect();
    let normalized = normalize(&il, &i, &NormalizeOptions::default());
    let normalized_names: Vec<_> = normalized
        .units
        .iter()
        .map(|unit| {
            let span = normalized.node(unit.root).span;
            (
                unit.kind,
                unit.name.map(|name| i.resolve(name).to_string()),
                span.start_line,
                span.end_line,
                count_nodes(&normalized, unit.root, None),
                count_nodes(&normalized, unit.root, Some(nose_il::NodeKind::Raw)),
            )
        })
        .collect();
    let opts = DetectOptions {
        min_lines: 1,
        min_tokens: 1,
        ..Default::default()
    };
    let units = nose_detect::units_of_file(&il, &i, &opts);
    let names: Vec<_> = units
        .iter()
        .filter_map(|unit| unit.name.as_deref())
        .collect();
    assert!(
        names.contains(&"sample:arm0") && names.contains(&"sample:arm1"),
        "macro_rules! arm units should survive feature extraction: lowered={lowered_names:?} normalized={normalized_names:?} kept={names:?}"
    );
    let arm_summaries: Vec<_> = units
        .iter()
        .filter(|unit| {
            unit.name
                .as_deref()
                .is_some_and(|name| name.starts_with("sample:arm"))
        })
        .map(|unit| (unit.name.as_deref(), unit.token_count, unit.exact_safe))
        .collect();
    assert!(
        units
            .iter()
            .filter(|unit| unit.name.as_deref().is_some_and(|name| name.starts_with("sample:arm")))
            .all(|unit| !unit.exact_safe && unit.token_count > 1),
        "macro_rules! arm units should be matchable but not exact-safe semantic proofs: {arm_summaries:?}"
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
    let ts = "function f(xs: number[]): number[] { return xs.map(x => x * 2); }";
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
    let go = "package m\n\nimport \"fmt\"\n\nfunc F(x int) {\n\tfmt.Println(x)\n}\n";
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
fn cfg_continue_guard_requires_total_order_proof() {
    let i = Interner::new();
    let cont = "def f(xs):\n    total = 0\n    for x in xs:\n        if x < 0:\n            continue\n        total = total + x\n    return total\n";
    let nested = "def g(ys):\n    total = 0\n    for y in ys:\n        if y >= 0:\n            total = total + y\n    return total\n";
    assert_ne!(
        unit_hash(&i, cont, Lang::Python),
        unit_hash(&i, nested, Lang::Python)
    );
}

#[test]
fn algebra_associativity() {
    let i = Interner::new();
    // ASSOCIATIVITY (`(a+b)+c` ≡ `a+(b+c)`) is sound for ALL types — string concat is
    // associative — so the value graph reconciles it even for UNTYPED params. COMMUTATIVITY
    // (`(a+b)+c` ≡ `c+(a+b)`) is gated on non-concat (#283-C): untyped stays distinct, an
    // int-annotated chain still fully canonicalizes. (Reconciled by the value graph, not the
    // algebra IL pass — so check the fingerprint, not the IL hash.)
    let left = "def f(a, b, c):\n    return (a + b) + c\n";
    let right = "def g(a, b, c):\n    return a + (b + c)\n";
    let mixed = "def h(a, b, c):\n    return c + (a + b)\n";
    let hl = value_fp(&i, left, Lang::Python);
    assert_eq!(
        hl,
        value_fp(&i, right, Lang::Python),
        "associativity is sound untyped"
    );
    assert_ne!(
        hl,
        value_fp(&i, mixed, Lang::Python),
        "untyped commutativity stays gated (operands could be strings)"
    );
    let t = |src: &str| value_fp(&i, src, Lang::Python);
    let tl = t("def f(a: int, b: int, c: int):\n    return (a + b) + c\n");
    assert_eq!(
        tl,
        t("def g(a: int, b: int, c: int):\n    return a + (b + c)\n")
    );
    assert_eq!(
        tl,
        t("def h(a: int, b: int, c: int):\n    return c + (a + b)\n"),
        "int-annotated + fully commutes and associates"
    );
}

#[test]
fn float_subtraction_is_not_reassociated_while_integer_subtraction_is() {
    // #283 C-float: a `-` carrying a PROVEN-float operand is kept as a literal `Sub` rather
    // than routed through the AC `+` normalization (`a - b` ≡ `a + (-b)`), because that
    // reassociation is float-unsound — `(1e100 + x) - 1e100` (= 0.0, the large term swallows
    // x) must not converge with the regrouped `(1e100 - 1e100) + x` (= x). Integer `-` still
    // normalizes and converges (two's-complement subtraction is associative). Pure `+`/`*`
    // float associativity is also held now (next test); the fully-untyped `(a+b)+c` with no
    // float marker still flattens — that is the remaining full Float value kind (§3.3).
    let i = Interner::new();
    let t = |src: &str| value_fp(&i, src, Lang::Python);
    // Float literal: the two groupings compute different floats and must NOT merge.
    assert_ne!(
        t("def f(x):\n    return (1e100 + x) - 1e100\n"),
        t("def g(x):\n    return (1e100 - 1e100) + x\n"),
        "float-literal subtraction must not reassociate across groupings"
    );
    // Control — the same shape with an integer literal is sound and still converges.
    assert_eq!(
        t("def f(x):\n    return (5 + x) - 5\n"),
        t("def g(x):\n    return (5 - 5) + x\n"),
        "integer subtraction must still reassociate (sound)"
    );
}

#[test]
fn float_addition_and_multiplication_are_held_unassociated() {
    // #283 C-float: float `+`/`*` is non-associative. A chain with a SYNTACTICALLY-float leaf
    // — a float literal or a `/` (true-division) result — keeps its source grouping in BOTH
    // the algebra IL pass (`chain_has_syntactic_float` → don't reassociate) and the value
    // graph (`proven_float` → don't flatten), so the groupings fingerprint distinctly.
    let i = Interner::new();
    let t = |src: &str| value_fp(&i, src, Lang::Python);
    assert_ne!(
        t("def f(a, b):\n    return (1.0 + a) + b\n"),
        t("def g(a, b):\n    return 1.0 + (a + b)\n"),
        "float-literal `+` must not reassociate"
    );
    assert_ne!(
        t("def f(a, b):\n    return (1.5 * a) * b\n"),
        t("def g(a, b):\n    return 1.5 * (a * b)\n"),
        "float-literal `*` must not reassociate"
    );
    assert_ne!(
        t("def f(a, b, c):\n    return (a / b + c) + 1.0\n"),
        t("def g(a, b, c):\n    return a / b + (c + 1.0)\n"),
        "true-division (float) `+` must not reassociate"
    );
    // Control — integer reassociation is sound and still converges.
    assert_eq!(
        t("def p(a, b):\n    return (2 + a) + b\n"),
        t("def q(a, b):\n    return 2 + (a + b)\n"),
        "integer `+` still reassociates (sound)"
    );
    // Control — the SAME float grouping written twice still converges (the hold is
    // grouping-sensitive, not a blanket exclusion).
    assert_eq!(
        t("def p(a, b):\n    return (1.0 + a) + b\n"),
        t("def q(x, y):\n    return (1.0 + x) + y\n"),
        "identical float grouping still converges"
    );
}

#[test]
fn float_typed_param_addition_is_held_unassociated() {
    // #283 C-float: a `+`/`*` chain over FLOAT-TYPED params (`: float`, `f64`, `double`,
    // `float64`) is non-associative even with no syntactic float leaf — the float-ness comes
    // from the param's type evidence (`proven_float` via the param domain), not a literal. The
    // grouping is held in BOTH layers: the algebra IL pass (`chain_has_float` over
    // `float_param_cids` → don't reassociate) and the value graph (`proven_float` → don't
    // flatten, in the general AND the string-coercion `+` path for Java/TS). Untyped/int-typed
    // chains keep flattening (sound: integer `+` IS associative; the fully-untyped float case
    // is the remaining Value::Float kind, §3.3).
    let i = Interner::new();

    // Python `: float`
    assert_ne!(
        value_fp(&i, "def f(a: float, b: float, c: float):\n    return (a + b) + c\n", Lang::Python),
        value_fp(&i, "def g(a: float, b: float, c: float):\n    return a + (b + c)\n", Lang::Python),
        "python float-typed `+` must not reassociate"
    );
    // Rust `f64`
    assert_ne!(
        value_fp(&i, "fn f(a: f64, b: f64, c: f64) -> f64 { (a + b) + c }", Lang::Rust),
        value_fp(&i, "fn g(a: f64, b: f64, c: f64) -> f64 { a + (b + c) }", Lang::Rust),
        "rust f64 `+` must not reassociate"
    );
    // Java `double` — the value graph routes Java `+` through the string-coercion path, which
    // must honor the float hold too (the regression this test guards).
    assert_ne!(
        value_fp(&i, "class C { static double f(double a, double b, double c) { return (a + b) + c; } }", Lang::Java),
        value_fp(&i, "class C { static double g(double a, double b, double c) { return a + (b + c); } }", Lang::Java),
        "java double `+` must not reassociate"
    );
    // Go `float64`
    assert_ne!(
        value_fp(&i, "package m\nfunc f(a float64, b float64, c float64) float64 { return (a + b) + c }", Lang::Go),
        value_fp(&i, "package m\nfunc g(a float64, b float64, c float64) float64 { return a + (b + c) }", Lang::Go),
        "go float64 `+` must not reassociate"
    );

    // Control — INT-typed params still reassociate (sound): split-only on float evidence.
    assert_eq!(
        value_fp(&i, "def p(a: int, b: int, c: int):\n    return (a + b) + c\n", Lang::Python),
        value_fp(&i, "def q(a: int, b: int, c: int):\n    return a + (b + c)\n", Lang::Python),
        "python int-typed `+` still reassociates"
    );
    assert_eq!(
        value_fp(&i, "class C { static int p(int a, int b, int c) { return (a + b) + c; } }", Lang::Java),
        value_fp(&i, "class C { static int q(int a, int b, int c) { return a + (b + c); } }", Lang::Java),
        "java int `+` still reassociates"
    );
    // Control — fully-untyped params still reassociate (the i64 oracle is associative;
    // distinguishing this is the full Value::Float kind, §3.3).
    assert_eq!(
        value_fp(&i, "def p(a, b, c):\n    return (a + b) + c\n", Lang::Python),
        value_fp(&i, "def q(a, b, c):\n    return a + (b + c)\n", Lang::Python),
        "untyped `+` still reassociates"
    );
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
        "function h(xs: number[], ys: number[]): number[] { return xs.flatMap(x => ys.map(y => x + y)); }",
        Lang::TypeScript,
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

/// Aggregates over a flat-map stream consume the flattened element stream, not the outer
/// mapped collection. Keep this bridge explicit so FlatMap is not accidentally treated as
/// the filtered-Map representation (`Hof(Map, [contrib, pred])`).
#[test]
fn flat_map_sum_aggregate_converges_with_nested_reduction_loop() {
    let i = Interner::new();
    let sum_gen = value_fp(
        &i,
        "def f(xs, ys):\n    return sum(x + y for x in xs for y in ys)\n",
        Lang::Python,
    );
    let sum_loop = value_fp(
        &i,
        "def g(xs, ys):\n    total = 0\n    for x in xs:\n        for y in ys:\n            total = total + x + y\n    return total\n",
        Lang::Python,
    );
    let sum_js = value_fp(
        &i,
        "function h(xs: number[], ys: number[]): number { return xs.flatMap(x => ys.map(y => x + y)).reduce((a, v) => a + v, 0); }",
        Lang::TypeScript,
    );
    let wrong_seed = value_fp(
        &i,
        "def bad(xs, ys):\n    total = 1\n    for x in xs:\n        for y in ys:\n            total = total + x + y\n    return total\n",
        Lang::Python,
    );
    let nested_list = value_fp(
        &i,
        "def nested(xs, ys):\n    return sum([x + y for y in ys] for x in xs)\n",
        Lang::Python,
    );

    assert_eq!(
        sum_gen, sum_loop,
        "sum over a flat-map generator should match the nested reduction loop"
    );
    assert_eq!(
        sum_gen, sum_js,
        "sum over a flatMap/map chain should match the flattened reduction"
    );
    assert_ne!(
        sum_gen, wrong_seed,
        "changing the additive seed changes aggregate behavior"
    );
    assert_ne!(
        sum_gen, nested_list,
        "aggregating nested list rows is not aggregating the flattened stream"
    );
}

#[test]
fn flat_map_max_aggregate_keeps_loop_seed_behavior_defining() {
    let i = Interner::new();
    let max_gen = value_fp(
        &i,
        "def f(xs, ys):\n    return max(x + y for x in xs for y in ys)\n",
        Lang::Python,
    );
    let max_loop = value_fp(
        &i,
        "def g(xs, ys):\n    best = 0\n    for x in xs:\n        for y in ys:\n            v = x + y\n            if v > best:\n                best = v\n    return best\n",
        Lang::Python,
    );
    let max_loop_same_seed = value_fp(
        &i,
        "def h(left, right):\n    top = 0\n    for a in left:\n        for b in right:\n            cand = a + b\n            if cand > top:\n                top = cand\n    return top\n",
        Lang::Python,
    );

    // `max(gen)` errs on empty input and tracks all-negative maxima; a `best = 0`
    // seeded loop clamps at 0 in both cases. The seed is behavior-defining, so the
    // two must NOT merge — while equal-seeded loops still converge with each other.
    assert_ne!(
        max_gen, max_loop,
        "a zero-seeded selection loop clamps at its seed; true max(...) does not"
    );
    assert_eq!(
        max_loop, max_loop_same_seed,
        "equally-seeded nested selection loops should still converge"
    );
}

#[test]
fn flat_map_any_aggregate_converges_with_nested_early_return_loop() {
    let i = Interner::new();
    let any_gen = value_fp(
        &i,
        "def f(xs, ys):\n    return any(x + y > 0 for x in xs for y in ys)\n",
        Lang::Python,
    );
    let any_loop = value_fp(
        &i,
        "def g(xs, ys):\n    for x in xs:\n        for y in ys:\n            if x + y > 0:\n                return True\n    return False\n",
        Lang::Python,
    );
    let any_bad_predicate = value_fp(
        &i,
        "def bad(xs, ys):\n    return any(x + y < 0 for x in xs for y in ys)\n",
        Lang::Python,
    );

    assert_eq!(
        any_gen, any_loop,
        "any over a flat-map generator should match the nested early-return loop"
    );
    assert_ne!(
        any_gen, any_bad_predicate,
        "changing the flattened any predicate changes behavior"
    );
}

#[test]
fn flat_map_outer_independent_aggregate_keeps_outer_cardinality() {
    let i = Interner::new();
    let outer_independent_flat = value_fp(
        &i,
        "def f(xs, ys):\n    return sum(y for x in xs for y in ys)\n",
        Lang::Python,
    );
    let outer_independent_loop = value_fp(
        &i,
        "def g(xs, ys):\n    total = 0\n    for x in xs:\n        for y in ys:\n            total = total + y\n    return total\n",
        Lang::Python,
    );
    let direct_inner_sum = value_fp(
        &i,
        "def h(xs, ys):\n    return sum(y for y in ys)\n",
        Lang::Python,
    );

    assert_ne!(
        outer_independent_flat, direct_inner_sum,
        "a flat-map aggregate that ignores the outer value still depends on outer cardinality"
    );
    assert_ne!(
        outer_independent_loop, direct_inner_sum,
        "a nested loop that ignores the outer value still depends on outer cardinality"
    );
}

#[test]
fn filtered_flat_map_sum_converges_with_nested_guarded_reduction() {
    let i = Interner::new();
    let filtered_sum_gen = value_fp(
        &i,
        "def f(xs, ys):\n    return sum(x + y for x in xs if x > 0 for y in ys if y < 10)\n",
        Lang::Python,
    );
    let filtered_sum_loop = value_fp(
        &i,
        "def g(xs, ys):\n    total = 0\n    for x in xs:\n        if x > 0:\n            for y in ys:\n                if y < 10:\n                    total = total + x + y\n    return total\n",
        Lang::Python,
    );
    let filtered_sum_js = value_fp(
        &i,
        "function h(xs: number[], ys: number[]): number { return xs.filter(x => x > 0).flatMap(x => ys.filter(y => y < 10).map(y => x + y)).reduce((a, v) => a + v, 0); }",
        Lang::TypeScript,
    );
    let filtered_sum_outer_changed = value_fp(
        &i,
        "def bad(xs, ys):\n    return sum(x + y for x in xs if False for y in ys if y < 10)\n",
        Lang::Python,
    );
    let filtered_sum_inner_changed = value_fp(
        &i,
        "def bad(xs, ys):\n    return sum(x + y for x in xs if x > 0 for y in ys if False)\n",
        Lang::Python,
    );

    assert_eq!(
        filtered_sum_gen, filtered_sum_loop,
        "filtered flat-map sums should match equivalent nested guarded reductions"
    );
    assert_ne!(
        filtered_sum_gen, filtered_sum_js,
        "TypeScript number[] callback elements are not yet numeric proof for relational predicates"
    );
    assert_ne!(
        filtered_sum_gen, filtered_sum_outer_changed,
        "changing the outer flat-map aggregate predicate changes behavior"
    );
    assert_ne!(
        filtered_sum_gen, filtered_sum_inner_changed,
        "changing the inner flat-map aggregate predicate changes behavior"
    );
}

#[test]
fn filtered_flat_map_any_all_converge_with_nested_guarded_loops() {
    let i = Interner::new();
    let filtered_any_gen = value_fp(
        &i,
        "def f(xs, ys):\n    return any(x + y > 0 for x in xs if x > 0 for y in ys if y < 10)\n",
        Lang::Python,
    );
    let filtered_any_js = value_fp(
        &i,
        "function h(xs: number[], ys: number[]): boolean { return xs.filter(x => x > 0).flatMap(x => ys.filter(y => y < 10).map(y => x + y)).some(v => v > 0); }",
        Lang::TypeScript,
    );
    let filtered_any_loop = value_fp(
        &i,
        "def g(xs, ys):\n    for x in xs:\n        if x > 0:\n            for y in ys:\n                if y < 10 and x + y > 0:\n                    return True\n    return False\n",
        Lang::Python,
    );
    let filtered_any_terminal_changed = value_fp(
        &i,
        "function h(xs: number[], ys: number[]): boolean { return xs.filter(x => x > 0).flatMap(x => ys.filter(y => y < 10).map(y => x + y)).some(v => false); }",
        Lang::TypeScript,
    );
    let filtered_all_gen = value_fp(
        &i,
        "def f(xs, ys):\n    return all(x + y > 0 for x in xs if x > 0 for y in ys if y < 10)\n",
        Lang::Python,
    );
    let filtered_all_js = value_fp(
        &i,
        "function h(xs: number[], ys: number[]): boolean { return xs.filter(x => x > 0).flatMap(x => ys.filter(y => y < 10).map(y => x + y)).every(v => v > 0); }",
        Lang::TypeScript,
    );
    let filtered_all_loop = value_fp(
        &i,
        "def g(xs, ys):\n    for x in xs:\n        if x > 0:\n            for y in ys:\n                if y < 10 and not (x + y > 0):\n                    return False\n    return True\n",
        Lang::Python,
    );
    let filtered_all_terminal_changed = value_fp(
        &i,
        "function h(xs: number[], ys: number[]): boolean { return xs.filter(x => x > 0).flatMap(x => ys.filter(y => y < 10).map(y => x + y)).every(v => false); }",
        Lang::TypeScript,
    );

    assert_ne!(
        filtered_any_gen, filtered_any_js,
        "TypeScript number[] callback elements are not yet numeric proof for relational predicates"
    );
    assert_eq!(
        filtered_any_gen, filtered_any_loop,
        "filtered flat-map any should match the equivalent nested guarded early-return loop"
    );
    assert_ne!(
        filtered_any_gen, filtered_any_terminal_changed,
        "changing the terminal method predicate changes behavior"
    );
    assert_ne!(
        filtered_all_gen, filtered_all_js,
        "TypeScript number[] callback elements are not yet numeric proof for relational predicates"
    );
    assert_eq!(
        filtered_all_gen, filtered_all_loop,
        "filtered flat-map all should match the equivalent nested guarded early-return loop"
    );
    assert_ne!(
        filtered_all_gen, filtered_all_terminal_changed,
        "changing the terminal universal predicate changes behavior"
    );
}

/// Cross-language `any`/`all` predicate reductions converge when the predicate is proven in the
/// same primitive domain. Rust iterator predicates still converge with Python generators; TS
/// `number[]` callbacks stay closed until element-domain proof exists. `any` and `all` stay
/// DISTINCT (different short-circuit behavior).
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
        "function g(xs: number[]): boolean { return xs.some(x => x > 0); }",
        Lang::TypeScript,
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
        "function g(xs: number[]): boolean { return xs.every(x => x > 0); }",
        Lang::TypeScript,
    );
    assert_ne!(
        any_py, any_js,
        "TypeScript number[] callback elements are not yet numeric proof for relational predicates"
    );
    assert_eq!(any_py, any_rs, "Python any ≡ Rust any");
    assert_ne!(
        all_py, all_js,
        "TypeScript number[] callback elements are not yet numeric proof for relational predicates"
    );
    assert_ne!(any_py, all_py, "any and all must stay distinct");
    assert!(!any_py.is_empty());
}

#[test]
fn rust_filter_map_converges_with_filtered_map_and_guarded_builder() {
    let i = Interner::new();
    let filtered_py = value_fp(
        &i,
        "def f(xs):\n    return [x * 2 for x in xs if x > 0]\n",
        Lang::Python,
    );
    let filtered_js = value_fp(
        &i,
        "function f(xs){ return xs.filter(x => x > 0).map(x => x * 2); }",
        Lang::JavaScript,
    );
    let filtered_ts = value_fp(
        &i,
        "function f(xs: number[]): number[] { return xs.filter((x) => x > 0).map((x) => x * 2); }",
        Lang::TypeScript,
    );
    let filter_map_rs = value_fp(
        &i,
        "fn f(xs: &[i32]) -> Vec<i32> { xs.iter().copied().filter_map(|x| if x > 0 { Some(x * 2) } else { None }).collect() }",
        Lang::Rust,
    );
    let match_option_rs = value_fp(
        &i,
        "fn f(xs: &[i32]) -> Vec<i32> { xs.iter().copied().filter_map(|x| match x { _ if x > 0 => Some(x * 2), _ => None }).collect() }",
        Lang::Rust,
    );
    let and_then_rs = value_fp(
        &i,
        "fn f(xs: &[i32]) -> Vec<i32> { xs.iter().copied().filter_map(|x| Some(x).and_then(|value| if value > 0 { Some(value * 2) } else { None })).collect() }",
        Lang::Rust,
    );
    let guarded_builder_rs = value_fp(
        &i,
        "fn f(xs: &[i32]) -> Vec<i32> { let out = Vec::new(); for x in xs { if *x > 0 { out.push(*x * 2); } } out }",
        Lang::Rust,
    );
    let mapped_none_rs = value_fp(
        &i,
        "fn f(xs: &[i32]) -> Vec<Option<i32>> { xs.iter().copied().map(|x| if x > 0 { Some(x * 2) } else { None }).collect() }",
        Lang::Rust,
    );
    let changed_value_rs = value_fp(
        &i,
        "fn f(xs: &[i32]) -> Vec<i32> { xs.iter().copied().filter_map(|x| if x > 0 { Some(x * 3) } else { None }).collect() }",
        Lang::Rust,
    );
    assert_ne!(
        filtered_py, filtered_js,
        "untyped JS parameter method calls lack a receiver proof and must stay opaque"
    );
    assert_ne!(
        filtered_py, filtered_ts,
        "TypeScript number[] callback elements are not yet numeric proof for relational predicates"
    );
    assert_eq!(
        filtered_py, filter_map_rs,
        "Rust i32 slice callback elements provide total-order proof for filter_map"
    );
    assert_eq!(
        filtered_py, match_option_rs,
        "Rust filter_map match guards should use the same total-order proof"
    );
    assert_eq!(
        filtered_py, and_then_rs,
        "Rust Option::and_then filter_map chains should use the same total-order proof"
    );
    assert_eq!(
        filtered_py, guarded_builder_rs,
        "Rust guarded Vec::new/push builder should use the same filtered-map value"
    );
    assert_ne!(
        filtered_py, mapped_none_rs,
        "mapping None as a value is not the same as dropping it"
    );
    assert_ne!(
        filtered_py, changed_value_rs,
        "changing the emitted Some value must stay distinct"
    );
}

#[test]
fn rust_filter_map_keeps_falsey_and_none_payload_boundaries() {
    let i = Interner::new();
    let filtered_py = value_fp(
        &i,
        "def f(xs):\n    return [x * 2 for x in xs if x > 0]\n",
        Lang::Python,
    );
    let falsey_py = value_fp(
        &i,
        "def f(xs):\n    return [0 for x in xs if x > 0]\n",
        Lang::Python,
    );
    let filtered_none_py = value_fp(
        &i,
        "def f(xs):\n    return [None for x in xs if x > 0]\n",
        Lang::Python,
    );
    let falsey_rs = value_fp(
        &i,
        "fn f(xs: &[i32]) -> Vec<i32> { xs.iter().copied().filter_map(|x| if x > 0 { Some(0) } else { None }).collect() }",
        Lang::Rust,
    );
    let wrapped_none_rs = value_fp(
        &i,
        "fn f(xs: &[i32]) -> Vec<Option<i32>> { xs.iter().copied().filter_map(|x| if x > 0 { Some(None) } else { None }).collect() }",
        Lang::Rust,
    );
    let dropped_falsey_rs = value_fp(
        &i,
        "fn f(xs: &[i32]) -> Vec<i32> { xs.iter().copied().filter_map(|x| if x > 0 { Some(0) } else { None }).filter(|x| *x != 0).collect() }",
        Lang::Rust,
    );

    assert_eq!(
        falsey_py, falsey_rs,
        "Rust filter_map emits falsey payloads rather than treating them as absence"
    );
    assert_eq!(
        filtered_none_py, wrapped_none_rs,
        "Rust filter_map emits wrapped None payloads rather than dropping them"
    );
    assert_ne!(
        filtered_py, wrapped_none_rs,
        "emitting None payloads must stay distinct from emitting mapped values"
    );
    assert_ne!(
        falsey_rs, dropped_falsey_rs,
        "truthy filtering after emitting 0 must stay distinct"
    );
}

#[test]
fn rust_filter_map_respects_shadowed_std_name_boundaries() {
    let i = Interner::new();
    let filtered_py = value_fp(
        &i,
        "def f(xs):\n    return [x * 2 for x in xs if x > 0]\n",
        Lang::Python,
    );
    let shadowed_some_rs = value_fp_named(
        &i,
        "fn Some(_value: i32) -> Option<i32> { None }\nfn f(xs: &[i32]) -> Vec<i32> { xs.iter().copied().filter_map(|x| if x > 0 { Some(x * 2) } else { None }).collect() }",
        Lang::Rust,
        "f",
    );
    let shadowed_none_rs = value_fp_named(
        &i,
        "const None: Option<i32> = Some(0);\nfn f(xs: &[i32]) -> Vec<i32> { xs.iter().copied().filter_map(|x| if x > 0 { Some(x * 2) } else { None }).collect() }",
        Lang::Rust,
        "f",
    );
    let shadowed_vec_rs = value_fp_named(
        &i,
        "struct Vec;\nimpl Vec { fn new() -> Vec { Vec } fn push(&self, _value: i32) {} }\nfn f(xs: &[i32]) -> Vec { let out = Vec::new(); for x in xs { if *x > 0 { out.push(*x * 2); } } out }",
        Lang::Rust,
        "f",
    );

    assert_ne!(
        filtered_py, shadowed_some_rs,
        "a local Rust Some definition must not be treated as Option::Some"
    );
    assert_ne!(
        filtered_py, shadowed_none_rs,
        "a local Rust None definition must not be treated as Option::None"
    );
    assert_ne!(
        filtered_py, shadowed_vec_rs,
        "a local Rust Vec definition must not be treated as std Vec::new"
    );
}

/// Value-graph fingerprint of the first function unit.
fn value_fp(interner: &Interner, src: &str, lang: Lang) -> Vec<u64> {
    let il = nose_frontend::lower_source(FileId(0), "t", src.as_bytes(), lang, interner).unwrap();
    let n = normalize(&il, interner, &NormalizeOptions::default());
    nose_normalize::value_fingerprint(&n, first_func(&n), interner)
}

fn return_fp(interner: &Interner, src: &str, lang: Lang) -> Vec<u64> {
    let il = nose_frontend::lower_source(FileId(0), "t", src.as_bytes(), lang, interner).unwrap();
    let n = normalize(&il, interner, &NormalizeOptions::default());
    nose_normalize::value_fingerprint_lits(&n, first_func(&n), interner).2
}

fn value_anchors(interner: &Interner, src: &str, lang: Lang) -> Vec<u64> {
    let il = nose_frontend::lower_source(FileId(0), "t", src.as_bytes(), lang, interner).unwrap();
    let n = normalize(&il, interner, &NormalizeOptions::default());
    nose_normalize::value_anchors(&n, first_func(&n), interner)
        .into_iter()
        .map(|anchor| anchor.hash)
        .collect()
}

fn shares_any(a: &[u64], b: &[u64]) -> bool {
    a.iter().any(|x| b.contains(x))
}

fn full_anchors(interner: &Interner, src: &str, lang: Lang) -> Vec<nose_normalize::Anchor> {
    let il = nose_frontend::lower_source(FileId(0), "t", src.as_bytes(), lang, interner).unwrap();
    let n = normalize(&il, interner, &NormalizeOptions::default());
    nose_normalize::value_anchors(&n, first_func(&n), interner)
}

#[test]
fn sub_dag_anchor_carries_source_line_range_of_the_shared_computation() {
    // A heavy sub-DAG anchor records WHERE its computation lives (line range), so a partial clone
    // can report the shared lines. The SAME computation placed on different lines in two functions
    // yields the same anchor hash but each unit's own line range.
    let i = Interner::new();
    let body = |head: &str| {
        format!(
            "{head}\n  const totals = items.map(i => i.price * i.qty).reduce((s, x) => s + x, 0);\n  const tax = totals * 0.1;\n  const shipping = totals > 100 ? 0 : 15;\n  const grand = totals + tax + shipping;\n  log(grand);\n  return grand;\n}}\n"
        )
    };
    // Same heavy computation, but two extra lines push it down in `g`.
    let a = body("function f(items) {");
    let b = body("function g(items) {\n  log(1);\n  log(2);");
    let aa = full_anchors(&i, &a, Lang::TypeScript);
    let bb = full_anchors(&i, &b, Lang::TypeScript);
    // At least one anchor carries a real (non-zero) line range.
    assert!(
        aa.iter().any(|x| x.line_start > 0),
        "an anchor should record its source line range, got {aa:?}",
    );
    // The shared computation produces the same hash in both, on each unit's own line.
    let sh = aa
        .iter()
        .filter(|x| bb.iter().any(|y| y.hash == x.hash))
        .max_by_key(|x| x.weight)
        .expect("the shared computation should produce a common anchor hash");
    let in_b = bb.iter().find(|y| y.hash == sh.hash).unwrap();
    // Each unit reports the shared computation at ITS OWN location: the two leading `log` lines in
    // `g` shift every line down by exactly 2.
    assert!(
        sh.line_start > 0 && in_b.line_start > 0,
        "both carry a real line: {sh:?} / {in_b:?}"
    );
    assert_eq!(
        in_b.line_start,
        sh.line_start + 2,
        "g's shared computation is 2 lines below f's (the two extra `log` lines)",
    );
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

fn class_value_fp(interner: &Interner, src: &str, lang: Lang, name: &str) -> Vec<u64> {
    let il = nose_frontend::lower_source(FileId(0), "t", src.as_bytes(), lang, interner).unwrap();
    let n = normalize(&il, interner, &NormalizeOptions::default());
    let root = n
        .units
        .iter()
        .find(|unit| {
            matches!(unit.kind, UnitKind::Class)
                && unit
                    .name
                    .is_some_and(|symbol| interner.resolve(symbol) == name)
        })
        .map(|unit| unit.root)
        .unwrap_or_else(|| panic!("expected class unit named {name}"));
    nose_normalize::value_fingerprint(&n, root, interner)
}

fn corpus_value_fp(corpus: &Corpus, path_suffix: &str, name: &str) -> Vec<u64> {
    let il = corpus
        .files
        .iter()
        .find(|il| il.meta.path.ends_with(path_suffix))
        .unwrap_or_else(|| panic!("expected corpus file ending with {path_suffix}"));
    let n = normalize(il, &corpus.interner, &NormalizeOptions::default());
    let root = n
        .units
        .iter()
        .find(|unit| {
            unit.name
                .is_some_and(|symbol| corpus.interner.resolve(symbol) == name)
        })
        .map(|unit| unit.root)
        .unwrap_or_else(|| panic!("expected function unit named {name} in {path_suffix}"));
    nose_normalize::value_fingerprint(&n, root, &corpus.interner)
}

/// Write `files` into a fresh per-process temp dir named after `tag` and lower
/// them together as one corpus. Callers remove the returned dir when done.
fn lower_temp_corpus(tag: &str, files: &[(&str, &str)]) -> (std::path::PathBuf, Corpus) {
    let dir = std::env::temp_dir().join(format!("{tag}_{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    for (name, contents) in files {
        std::fs::write(dir.join(name), contents).unwrap();
    }
    let corpus = nose_frontend::lower_corpus_many(&[dir.as_path()]);
    (dir, corpus)
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
    let java_read_field = "class C { int x; int f() { this.x = 7; return this.x; } }";
    let java_return_value = "class C { int x; int f() { this.x = 7; return 7; } }";
    let read_other_receiver = "def f(a, b):\n    a.x = 7\n    return b.x\n";
    let read_written_receiver = "def f(a, b):\n    a.x = 7\n    return a.x\n";
    let unknown_alias_receiver = "def f(a):\n    r = receiver(a)\n    r.x = 7\n    return a.x\n";
    let computed_receiver = "def f(a):\n    receiver(a).x = 7\n    return receiver(a).x\n";
    assert_ne!(
        value_fp(&i, read_field, Lang::Python),
        value_fp(&i, return_value, Lang::Python),
        "raw Python attribute spelling is not enough proof for exact field-state readback"
    );
    assert_eq!(
        value_fp(&i, java_read_field, Lang::Java),
        value_fp(&i, java_return_value, Lang::Java),
        "a Java this.field read after an effect-proven same-unit write should resolve to the written value"
    );
    assert_ne!(
        value_fp(&i, read_other_receiver, Lang::Python),
        value_fp(&i, read_written_receiver, Lang::Python),
        "a same-named field write on one receiver must not satisfy a read on another receiver"
    );
    assert_ne!(
        value_fp(&i, unknown_alias_receiver, Lang::Python),
        value_fp(&i, return_value, Lang::Python),
        "field-state readback must not assume call-result aliasing without receiver-place proof"
    );
    assert_ne!(
        value_fp(&i, computed_receiver, Lang::Python),
        value_fp(&i, return_value, Lang::Python),
        "computed field receivers must not enter same-unit field-state caching"
    );
}

#[test]
fn value_graph_field_state_is_receiver_aware() {
    let i = Interner::new();
    let same_receiver_order_a = "def f(self):\n    self.x = 1\n    self.y = 2\n";
    let same_receiver_order_b = "def f(self):\n    self.y = 2\n    self.x = 1\n";
    let java_same_receiver_order_a =
        "class C { int x; int y; void f() { this.x = 1; this.y = 2; } }";
    let java_same_receiver_order_b =
        "class C { int x; int y; void f() { this.y = 2; this.x = 1; } }";
    let crossed_receivers_a = "def f(a, b):\n    a.x = 1\n    b.x = 2\n";
    let crossed_receivers_b = "def f(a, b):\n    b.x = 1\n    a.x = 2\n";
    assert_ne!(
        value_fp(&i, same_receiver_order_a, Lang::Python),
        value_fp(&i, same_receiver_order_b, Lang::Python),
        "raw Python attribute writes stay ordered because property/setter effects are not proven"
    );
    assert_eq!(
        value_fp(&i, java_same_receiver_order_a, Lang::Java),
        value_fp(&i, java_same_receiver_order_b, Lang::Java),
        "effect-proven Java final writes to distinct this.fields should commute"
    );
    assert_ne!(
        value_fp(&i, crossed_receivers_a, Lang::Python),
        value_fp(&i, crossed_receivers_b, Lang::Python),
        "same-named field writes on different receivers must preserve receiver identity"
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
        "import functools\n\ndef f():\n    try:\n        return functools.reduce(lambda a, x: 1 / 0, [1], 0)\n    except Exception:\n        return 7\n";
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
        "import functools\n\ndef f():\n    try:\n        return functools.reduce(lambda a, x: 1 / 0, [], 0)\n    except Exception:\n        return 7\n";
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
fn map_key_membership_converges_cross_language() {
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
    let ts_direct_keys_includes = "function f(lookup: Map<string, string>, other_lookup: Map<string, string>, key: string, other: string): boolean { return lookup.keys().includes(key); }";
    let typed_set_same_names = "function f(lookup: Set<string>, other_lookup: Set<string>, key: string, other: string): boolean { return lookup.has(key); }";

    let fp = value_fp(&i, py, Lang::Python);
    assert_ne!(fp, value_fp(&i, py_method, Lang::Python));
    assert_eq!(fp, value_fp(&i, py_keys_in, Lang::Python));
    assert_eq!(fp, value_fp(&i, py_keys_contains, Lang::Python));
    assert_eq!(fp, value_fp(&i, go, Lang::Go));
    assert_eq!(fp, value_fp(&i, java, Lang::Java));
    assert_eq!(fp, value_fp(&i, java_key_set, Lang::Java));
    assert_eq!(fp, value_fp(&i, rust, Lang::Rust));
    assert_eq!(fp, value_fp(&i, rust_get, Lang::Rust));
    assert_ne!(fp, value_fp(&i, ruby, Lang::Ruby));
    assert_ne!(fp, value_fp(&i, ruby_has, Lang::Ruby));
    assert_eq!(fp, value_fp(&i, ts_array_from_keys, Lang::TypeScript));
    assert_ne!(
        fp,
        value_fp(&i, ts_direct_keys_includes, Lang::TypeScript),
        "Map.keys() is an iterator view; direct .includes is not a proven key-view collection"
    );
    assert_ne!(fp, value_fp(&i, typed_set_same_names, Lang::TypeScript));
}

#[test]
fn map_key_membership_keeps_wrong_coordinate_boundaries() {
    let i = Interner::new();
    let py = "def f(lookup, other_lookup, key, other):\n    return key in lookup\n";
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
    let ts_array_from_shadowed_array = "function f(lookup: Map<string, string>, other_lookup: Map<string, string>, key: string, other: string, Array: any): boolean { return Array.from(lookup.keys()).includes(key); }";

    let fp = value_fp(&i, py, Lang::Python);
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
    assert_ne!(
        fp,
        value_fp(&i, ts_array_from_shadowed_array, Lang::TypeScript)
    );
}

#[test]
fn import_named_and_namespace_member_coordinates_converge() {
    let i = Interner::new();
    let js_named = "import { helper } from \"./shared-math\";\nfunction f(value) { return helper(value + 1); }\n";
    let js_namespace = "import * as mathOps from \"./shared-math\";\nfunction f(value) { return mathOps.helper(value + 1); }\n";
    let js_wrong_member = "import * as mathOps from \"./shared-math\";\nfunction f(value) { return mathOps.otherHelper(value + 1); }\n";
    let ts_named = "import { helper } from \"./shared-math\";\nfunction f(value: number): number { return helper(value + 1); }\n";
    let ts_namespace = "import * as mathOps from \"./shared-math\";\nfunction f(value: number): number { return mathOps.helper(value + 1); }\n";
    let ts_type_only =
        "import type { helper } from \"./shared-math\";\nfunction f(value: number): number { return helper(value + 1); }\n";
    let ts_mixed_type_only = "import { helper, type otherHelper } from \"./shared-math\";\nfunction f(value: number): number { return otherHelper(value + 1); }\n";
    let py_named =
        "from shared_math import helper\n\ndef f(value):\n    return helper(value + 1)\n";
    let py_namespace =
        "import shared_math as math_ops\n\ndef f(value):\n    return math_ops.helper(value + 1)\n";
    let py_wrong_member =
        "import shared_math as math_ops\n\ndef f(value):\n    return math_ops.other_helper(value + 1)\n";

    let fp = value_fp(&i, js_named, Lang::JavaScript);
    assert_eq!(fp, value_fp(&i, js_namespace, Lang::JavaScript));
    assert_ne!(fp, value_fp(&i, js_wrong_member, Lang::JavaScript));

    let ts_fp = value_fp(&i, ts_named, Lang::TypeScript);
    assert_eq!(ts_fp, value_fp(&i, ts_namespace, Lang::TypeScript));
    assert_ne!(fp, ts_fp);
    assert_ne!(fp, value_fp(&i, ts_type_only, Lang::TypeScript));
    assert_ne!(fp, value_fp(&i, ts_mixed_type_only, Lang::TypeScript));

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
    let unshadowed_non_mutating_js_method = r#"
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
    let fp = value_fp_named(&i, plain, Lang::TypeScript, "replaceRootDirInPath");
    assert_eq!(
        fp,
        value_fp_named(&i, shadowed_param, Lang::TypeScript, "replaceRootDirInPath"),
        "a parameter named like the namespace import must not taint the module binding"
    );
    assert_eq!(
        fp,
        value_fp_named(
            &i,
            unshadowed_non_mutating_js_method,
            Lang::TypeScript,
            "replaceRootDirInPath"
        ),
        "a Java-only mutation-like method name must not taint a TypeScript namespace import"
    );
}

// A long, flat sequence of independent convergence assertions — its cognitive
// complexity (133) and line count are breadth, not deep branching, so it sits
// above the production-oriented gates. Splitting it would not aid readability.
#[allow(clippy::cognitive_complexity, clippy::too_many_lines)]
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
    let js_set_call = "function f(value, other) { return Set([\"red\", \"blue\"]).has(value); }";
    let js_module_set =
        "const VALUES = new Set([\"red\", \"blue\"]);\nfunction f(value, other) { return VALUES.has(value); }";
    let ts_module_set = "const VALUES = new Set<string>([\"red\", \"blue\"]);\nfunction f(value: string, other: string): boolean { return VALUES.has(value); }";
    let js_array_contains =
        "function f(value, other) { return [\"red\", \"blue\"].contains(value); }";
    let js_array_some =
        "function f(value, other) { return [\"red\", \"blue\"].some((item) => item === value); }";
    let ts_array_some = "function f(value: string, other: string): boolean { return [\"red\", \"blue\"].some((item: string) => item === value); }";
    let js_array_indexof_ne =
        "function f(value, other) { return [\"red\", \"blue\"].indexOf(value) !== -1; }";
    let js_sequence_indexof_ne =
        "function f(value, other) { return (\"red\", \"blue\").indexOf(value) !== -1; }";
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
    let js_global_shadowed_set = "function Set(values) { return { has: function() { return false; } }; }\nfunction f(value, other) { return new Set([\"red\", \"blue\"]).has(value); }";
    let js_module_set_mutated = "const VALUES = new Set([\"red\", \"blue\"]);\nVALUES.add(\"green\");\nfunction f(value, other) { return VALUES.has(value); }";
    let js_module_array_fill_mutated = "const VALUES = [\"red\", \"blue\"];\nVALUES.fill(\"green\");\nfunction f(value, other) { return VALUES.includes(value); }";
    let js_local_array_copywithin_mutated = "function f(value, other) { const values = [\"red\", \"blue\"]; values.copyWithin(0, 1); return values.includes(value); }";
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
    let rust_std_shadowed = "mod std { pub mod collections { pub struct HashSet; } }\npub fn f(value: &str, other: &str) -> bool {\n    let values = std::collections::HashSet::from([\"red\", \"blue\"]);\n    values.contains(&value)\n}\n";
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
    assert_ne!(
        literal_fp,
        value_fp(&i, py_module_tuple, Lang::Python),
        "module-bound tuple literals no longer reopen as membership collections without surface/domain evidence"
    );
    assert_eq!(literal_fp, value_fp(&i, py_module_set, Lang::Python));
    assert_eq!(literal_fp, value_fp(&i, js_set_inline, Lang::JavaScript));
    assert_eq!(literal_fp, value_fp(&i, js_set_local, Lang::JavaScript));
    assert_eq!(literal_fp, value_fp(&i, js_module_set, Lang::JavaScript));
    assert_eq!(literal_fp, value_fp(&i, ts_module_set, Lang::TypeScript));
    assert_ne!(literal_fp, value_fp(&i, js_set_call, Lang::JavaScript));
    assert_ne!(
        literal_fp,
        value_fp(&i, js_array_contains, Lang::JavaScript),
        "JavaScript .contains is not a standard array membership contract"
    );
    assert_eq!(literal_fp, value_fp(&i, js_array_some, Lang::JavaScript));
    assert_eq!(literal_fp, value_fp(&i, ts_array_some, Lang::TypeScript));
    assert_eq!(
        literal_fp,
        value_fp(&i, js_array_indexof_ne, Lang::JavaScript)
    );
    assert_ne!(
        literal_fp,
        value_fp(&i, js_sequence_indexof_ne, Lang::JavaScript),
        "JS sequence expressions must not prove static array membership"
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
    assert_ne!(
        literal_fp,
        value_fp_named(&i, rust_std_shadowed, Lang::Rust, "f"),
        "a local Rust std module must not be treated as the standard library"
    );
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
        value_fp(&i, js_global_shadowed_set, Lang::JavaScript),
        "construct syntax alone must not prove a shadowed JS Set global"
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
        value_fp(&i, js_module_array_fill_mutated, Lang::JavaScript),
        "JS Array.fill must invalidate module/local collection proofs"
    );
    assert_ne!(
        literal_fp,
        value_fp(&i, js_local_array_copywithin_mutated, Lang::JavaScript),
        "JS Array.copyWithin must invalidate local collection proofs"
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
    let parameter_shadowed_arrays = "import java.util.Arrays;\n\nclass FakeArrays { java.util.List<String> asList(String[] values) { return java.util.List.of(\"green\"); } }\nclass C { static boolean f(FakeArrays Arrays, String[] values, String value) { return Arrays.asList(values).contains(value); } }\n";

    let array_fp = value_fp(&i, array_membership, Lang::Java);
    assert_ne!(array_fp, value_fp(&i, list_membership, Lang::Java));
    assert_ne!(
        array_fp,
        value_fp(&i, singleton_list_membership, Lang::Java)
    );
    assert_ne!(
        array_fp,
        value_fp(&i, parameter_shadowed_arrays, Lang::Java)
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
    let js_call =
        "function f(key, other) { return Map([[\"red\", 1], [\"blue\", 2]]).get(key) ?? 0; }";
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
    let js_global_shadowed_map = "function Map(entries) { return { get: function() { return 99; } }; }\nfunction f(key, other) { return new Map([[\"red\", 1], [\"blue\", 2]]).get(key) ?? 0; }";

    let fp = value_fp(&i, py_literal, Lang::Python);
    assert_eq!(fp, value_fp(&i, ruby_literal, Lang::Ruby));
    assert_eq!(fp, value_fp(&i, js_inline, Lang::JavaScript));
    assert_eq!(fp, value_fp(&i, js_local, Lang::JavaScript));
    assert_eq!(fp, value_fp(&i, js_has_get, Lang::JavaScript));
    assert_eq!(fp, value_fp(&i, ts_inline, Lang::TypeScript));
    assert_ne!(fp, value_fp(&i, js_call, Lang::JavaScript));
    assert_ne!(fp, value_fp(&i, js_wrong_key, Lang::JavaScript));
    assert_ne!(fp, value_fp(&i, js_wrong_default, Lang::JavaScript));
    assert_ne!(fp, value_fp(&i, js_wrong_map, Lang::JavaScript));
    assert_ne!(fp, value_fp(&i, js_untyped, Lang::JavaScript));
    assert_ne!(fp, value_fp(&i, js_shadowed_map, Lang::JavaScript));
    assert_ne!(
        fp,
        value_fp(&i, js_global_shadowed_map, Lang::JavaScript),
        "construct syntax alone must not prove a shadowed JS Map global"
    );
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
    let go_string_inline =
        "package p\n\nfunc F(key string, other string) string { return map[string]string{\"red\": \"apple\", \"blue\": \"berry\"}[key] }\n";

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
}

#[test]
fn literal_map_default_lookup_converges_with_go_literal_string_map_boundaries() {
    let i = Interner::new();
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
    let go_mixed_value =
        "package p\n\nfunc F(key string, other string) interface{} { return map[string]interface{}{\"red\": \"apple\", \"blue\": false}[key] }\n";

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
}

#[test]
fn literal_map_default_lookup_converges_with_go_literal_scalar_map_boundaries() {
    let i = Interner::new();
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
fn literal_map_default_lookup_converges_with_imported_python_literal_binding() {
    let (dir, corpus) = lower_temp_corpus(
        "nose_imported_map_default",
        &[
            (
                "local.py",
                "def lookup(key, other):\n    return {\"red\": 1, \"blue\": 2}.get(key, 0)\n",
            ),
            ("tables.py", "LOOKUP = {\"red\": 1, \"blue\": 2}\n"),
            (
                "imported.py",
                "from tables import LOOKUP\n\ndef lookup(key, other):\n    return LOOKUP.get(key, 0)\n",
            ),
            (
                "wrong_map.py",
                "from tables import LOOKUP\n\ndef lookup(key, other):\n    return {\"red\": 9, \"blue\": 2}.get(key, 0)\n",
            ),
            (
                "mutated_tables.py",
                "LOOKUP = {\"red\": 1, \"blue\": 2}\nLOOKUP.clear()\n",
            ),
            (
                "mutated_index_tables.py",
                "LOOKUP = {\"red\": 1, \"blue\": 2}\nLOOKUP[\"red\"] = 9\n",
            ),
            (
                "escaped_tables.py",
                "LOOKUP = {\"red\": 1, \"blue\": 2}\nmutate(LOOKUP)\n",
            ),
            (
                "imported_mutated_provider.py",
                "from mutated_tables import LOOKUP\n\ndef lookup(key, other):\n    return LOOKUP.get(key, 0)\n",
            ),
            (
                "imported_mutated_index_provider.py",
                "from mutated_index_tables import LOOKUP\n\ndef lookup(key, other):\n    return LOOKUP.get(key, 0)\n",
            ),
            (
                "imported_escaped_provider.py",
                "from escaped_tables import LOOKUP\n\ndef lookup(key, other):\n    return LOOKUP.get(key, 0)\n",
            ),
            (
                "imported_mutated_receiver.py",
                "from tables import LOOKUP\nLOOKUP.clear()\n\ndef lookup(key, other):\n    return LOOKUP.get(key, 0)\n",
            ),
            (
                "imported_mutated_index_receiver.py",
                "from tables import LOOKUP\nLOOKUP[\"red\"] = 9\n\ndef lookup(key, other):\n    return LOOKUP.get(key, 0)\n",
            ),
        ],
    );
    let local = corpus_value_fp(&corpus, "local.py", "lookup");
    assert_eq!(
        local,
        corpus_value_fp(&corpus, "imported.py", "lookup"),
        "imported immutable literal map binding should prove the same lookup/default coordinates"
    );
    assert_ne!(
        local,
        corpus_value_fp(&corpus, "wrong_map.py", "lookup"),
        "different literal map contents must stay distinct"
    );
    assert_ne!(
        local,
        corpus_value_fp(&corpus, "imported_mutated_provider.py", "lookup"),
        "provider mutation must block imported literal provenance"
    );
    assert_ne!(
        local,
        corpus_value_fp(&corpus, "imported_mutated_index_provider.py", "lookup"),
        "provider index write must block imported literal provenance"
    );
    assert_ne!(
        local,
        corpus_value_fp(&corpus, "imported_escaped_provider.py", "lookup"),
        "provider opaque argument escape must block imported literal provenance"
    );
    assert_ne!(
        local,
        corpus_value_fp(&corpus, "imported_mutated_receiver.py", "lookup"),
        "importer mutation must block imported literal provenance"
    );
    assert_ne!(
        local,
        corpus_value_fp(&corpus, "imported_mutated_index_receiver.py", "lookup"),
        "importer index write must block imported literal provenance"
    );

    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn literal_map_default_lookup_converges_with_js_ts_imported_bindings() {
    let (dir, corpus) = lower_temp_corpus(
        "nose_imported_js_ts_map_default",
        &[
            (
                "local.py",
                "def lookup(key, other):\n    return {\"red\": 1, \"blue\": 2}.get(key, 0)\n",
            ),
            (
                "js_tables.js",
                "export const LOOKUP = new Map([[\"red\", 1], [\"blue\", 2]]);\n",
            ),
            (
                "js_imported.js",
                "import { LOOKUP } from './js_tables';\nexport function lookup(key, other) {\n  return LOOKUP.get(key) ?? 0;\n}\n",
            ),
            (
                "js_mutated_tables.js",
                "export const LOOKUP = new Map([[\"red\", 1], [\"blue\", 2]]);\nLOOKUP.set(\"red\", 9);\n",
            ),
            (
                "js_imported_mutated_provider.js",
                "import { LOOKUP } from './js_mutated_tables';\nexport function lookup(key, other) {\n  return LOOKUP.get(key) ?? 0;\n}\n",
            ),
            (
                "js_imported_mutated_receiver.js",
                "import { LOOKUP } from './js_tables';\nLOOKUP.set(\"red\", 9);\nexport function lookup(key, other) {\n  return LOOKUP.get(key) ?? 0;\n}\n",
            ),
            (
                "js_wrong_map.js",
                "import { LOOKUP } from './js_tables';\nexport function lookup(key, other) {\n  return new Map([[\"red\", 9], [\"blue\", 2]]).get(key) ?? 0;\n}\n",
            ),
            (
                "ts_tables.ts",
                "export const LOOKUP = new Map<string, number>([[\"red\", 1], [\"blue\", 2]]);\n",
            ),
            (
                "ts_imported.ts",
                "import { LOOKUP } from './ts_tables';\nexport function lookup(key: string, other: string): number {\n  return LOOKUP.get(key) ?? 0;\n}\n",
            ),
        ],
    );
    let local = corpus_value_fp(&corpus, "local.py", "lookup");
    assert_ne!(
        local,
        corpus_value_fp(&corpus, "js_imported.js", "lookup"),
        "JS imported Map bindings stay closed until constructor/provenance proof exists"
    );
    assert_ne!(
        local,
        corpus_value_fp(&corpus, "ts_imported.ts", "lookup"),
        "TS imported Map bindings stay closed until constructor/provenance proof exists"
    );
    assert_ne!(
        local,
        corpus_value_fp(&corpus, "js_imported_mutated_provider.js", "lookup"),
        "provider mutation must block JS imported map provenance"
    );
    assert_ne!(
        local,
        corpus_value_fp(&corpus, "js_imported_mutated_receiver.js", "lookup"),
        "importer mutation must keep JS imported receiver distinct"
    );
    assert_ne!(
        local,
        corpus_value_fp(&corpus, "js_wrong_map.js", "lookup"),
        "different JS map contents must stay distinct"
    );

    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn literal_map_default_lookup_converges_with_java_imported_bindings() {
    let (dir, corpus) = lower_temp_corpus(
        "nose_imported_java_map_default",
        &[
            (
                "local.py",
                "def lookup(key, other):\n    return {\"red\": 1, \"blue\": 2}.get(key, 0)\n",
            ),
            (
                "Tables.java",
                "import java.util.Map;\n\nclass Tables {\n  static final Map<String, Integer> LOOKUP = Map.of(\"red\", 1, \"blue\", 2);\n}\n",
            ),
            (
                "JavaImported.java",
                "import static Tables.LOOKUP;\n\nclass JavaImported {\n  static int lookup(String key, String other) {\n    return LOOKUP.getOrDefault(key, 0);\n  }\n}\n",
            ),
            (
                "JavaImportedWithLocalMapShadow.java",
                "import static Tables.LOOKUP;\n\nclass JavaImportedWithLocalMapShadow {\n  static int lookup(String key, String other) {\n    return LOOKUP.getOrDefault(key, 0);\n  }\n}\n\nclass Map {}\n",
            ),
            (
                "WrongTables.java",
                "import java.util.Map;\n\nclass WrongTables {\n  static final Map<String, Integer> LOOKUP = Map.of(\"red\", 9, \"blue\", 2);\n}\n",
            ),
            (
                "JavaImportedWrongMap.java",
                "import static WrongTables.LOOKUP;\n\nclass JavaImportedWrongMap {\n  static int lookup(String key, String other) {\n    return LOOKUP.getOrDefault(key, 0);\n  }\n}\n",
            ),
            (
                "MissingMapImportTables.java",
                "class MissingMapImportTables {\n  static final Map<String, Integer> LOOKUP = Map.of(\"red\", 1, \"blue\", 2);\n}\n",
            ),
            (
                "JavaImportedMissingMapImport.java",
                "import static MissingMapImportTables.LOOKUP;\n\nclass JavaImportedMissingMapImport {\n  static int lookup(String key, String other) {\n    return LOOKUP.getOrDefault(key, 0);\n  }\n}\n",
            ),
        ],
    );
    let local = corpus_value_fp(&corpus, "local.py", "lookup");
    assert_eq!(
        local,
        corpus_value_fp(&corpus, "JavaImported.java", "lookup"),
        "Java static import should prove the same literal map/default coordinates"
    );
    assert_eq!(
        local,
        corpus_value_fp(&corpus, "JavaImportedWithLocalMapShadow.java", "lookup"),
        "provider-proven Java static import should not be invalidated by importer-local Map shadowing"
    );
    assert_ne!(
        local,
        corpus_value_fp(&corpus, "JavaImportedWrongMap.java", "lookup"),
        "different Java imported map contents must stay distinct"
    );
    assert_ne!(
        local,
        corpus_value_fp(&corpus, "JavaImportedMissingMapImport.java", "lookup"),
        "Java imported Map.of provider must require java.util.Map proof"
    );

    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn literal_map_default_lookup_converges_with_rust_imported_bindings() {
    let (dir, corpus) = lower_temp_corpus(
        "nose_imported_rust_map_default",
        &[
            (
                "local.py",
                "def lookup(key, other):\n    return {\"red\": 1, \"blue\": 2}.get(key, 0)\n",
            ),
            (
                "tables.rs",
                "pub const LOOKUP: [(&str, i32); 2] = [(\"red\", 1), (\"blue\", 2)];\n",
            ),
            (
                "rust_imported.rs",
                "use tables::LOOKUP;\n\npub fn lookup(key: &str, other: &str) -> i32 {\n    *std::collections::HashMap::from(LOOKUP).get(key).unwrap_or(&0)\n}\n",
            ),
            (
                "rust_imported_shadowed_std.rs",
                "use tables::LOOKUP;\n\nmod std { pub mod collections { pub struct HashMap; } }\n\npub fn lookup(key: &str, other: &str) -> i32 {\n    *std::collections::HashMap::from(LOOKUP).get(key).unwrap_or(&0)\n}\n",
            ),
            (
                "wrong_tables.rs",
                "pub const LOOKUP: [(&str, i32); 2] = [(\"red\", 9), (\"blue\", 2)];\n",
            ),
            (
                "rust_imported_wrong_map.rs",
                "use wrong_tables::LOOKUP;\n\npub fn lookup(key: &str, other: &str) -> i32 {\n    *std::collections::HashMap::from(LOOKUP).get(key).unwrap_or(&0)\n}\n",
            ),
        ],
    );
    let local = corpus_value_fp(&corpus, "local.py", "lookup");
    assert_eq!(
        local,
        corpus_value_fp(&corpus, "rust_imported.rs", "lookup"),
        "Rust use-imported const entries should prove the same map/default coordinates"
    );
    assert_ne!(
        local,
        corpus_value_fp(&corpus, "rust_imported_shadowed_std.rs", "lookup"),
        "a local Rust std module must block imported std map factory provenance"
    );
    assert_ne!(
        local,
        corpus_value_fp(&corpus, "rust_imported_wrong_map.rs", "lookup"),
        "different Rust imported map contents must stay distinct"
    );

    let _ = std::fs::remove_dir_all(&dir);
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
fn map_default_lookup_converges_cross_language() {
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
}

#[test]
fn map_default_lookup_keeps_wrong_coordinate_boundaries() {
    let i = Interner::new();
    let go = "package p\n\nfunc F(lookup map[string]int, otherLookup map[string]int, key string, otherKey string, fallback int, otherDefault int) int { value, ok := lookup[key]; if !ok { value = fallback }; return value }\n";
    let wrong_key = "import java.util.Map;\n\nclass C { static int f(Map<String, Integer> lookup, Map<String, Integer> other_lookup, String key, String other_key, int fallback, int other_default) { return lookup.getOrDefault(other_key, fallback); } }\n";
    let wrong_default = "use std::collections::HashMap;\n\npub fn f(lookup: &HashMap<&str, i32>, other_lookup: &HashMap<&str, i32>, key: &str, other_key: &str, fallback: i32, other_default: i32) -> i32 { *lookup.get(key).unwrap_or(&other_default) }\n";
    let wrong_map = "package p\n\nfunc F(lookup map[string]int, otherLookup map[string]int, key string, otherKey string, fallback int, otherDefault int) int { value, ok := otherLookup[key]; if !ok { value = fallback }; return value }\n";
    let ts_wrong_key = "function f(lookup: Map<string, number>, other_lookup: Map<string, number>, key: string, other_key: string, fallback: number, other_default: number): number { return lookup.get(other_key) ?? fallback; }\n";
    let ts_wrong_default = "function f(lookup: Map<string, number>, other_lookup: Map<string, number>, key: string, other_key: string, fallback: number, other_default: number): number { return lookup.get(key) ?? other_default; }\n";
    let ts_wrong_map = "function f(lookup: Map<string, number>, other_lookup: Map<string, number>, key: string, other_key: string, fallback: number, other_default: number): number { return other_lookup.get(key) ?? fallback; }\n";
    let ts_untyped = "function f(lookup, other_lookup, key, other_key, fallback, other_default) { return lookup.get(key) ?? fallback; }\n";
    let ts_temp_shadowed_undefined = "function f(lookup: Map<string, number>, other_lookup: Map<string, number>, key: string, other_key: string, fallback: number, other_default: number, undefined: number): number { const selected = lookup.get(key); return selected === undefined ? fallback : selected; }\n";
    let py_wrong_key = "def f(lookup: dict[str, int], other_lookup: dict[str, int], key: str, other_key: str, fallback: int, other_default: int) -> int:\n    return lookup.get(other_key, fallback)\n";
    let py_wrong_default = "def f(lookup: dict[str, int], other_lookup: dict[str, int], key: str, other_key: str, fallback: int, other_default: int) -> int:\n    return lookup.get(key, other_default)\n";
    let py_wrong_map = "def f(lookup: dict[str, int], other_lookup: dict[str, int], key: str, other_key: str, fallback: int, other_default: int) -> int:\n    return other_lookup.get(key, fallback)\n";
    let py_untyped = "def f(lookup, other_lookup, key, other_key, fallback, other_default):\n    return lookup.get(key, fallback)\n";

    let fp = value_fp(&i, go, Lang::Go);
    assert_ne!(fp, value_fp(&i, wrong_key, Lang::Java));
    assert_ne!(fp, value_fp(&i, wrong_default, Lang::Rust));
    assert_ne!(fp, value_fp(&i, wrong_map, Lang::Go));
    assert_ne!(fp, value_fp(&i, ts_wrong_key, Lang::TypeScript));
    assert_ne!(fp, value_fp(&i, ts_wrong_default, Lang::TypeScript));
    assert_ne!(fp, value_fp(&i, ts_wrong_map, Lang::TypeScript));
    assert_ne!(fp, value_fp(&i, ts_untyped, Lang::TypeScript));
    assert_ne!(
        fp,
        value_fp(&i, ts_temp_shadowed_undefined, Lang::TypeScript)
    );
    assert_ne!(fp, value_fp(&i, py_wrong_key, Lang::Python));
    assert_ne!(fp, value_fp(&i, py_wrong_default, Lang::Python));
    assert_ne!(fp, value_fp(&i, py_wrong_map, Lang::Python));
    assert_ne!(fp, value_fp(&i, py_untyped, Lang::Python));
}

#[test]
fn map_default_lookup_keeps_alias_and_guard_boundaries() {
    let i = Interner::new();
    let go = "package p\n\nfunc F(lookup map[string]int, otherLookup map[string]int, key string, otherKey string, fallback int, otherDefault int) int { value, ok := lookup[key]; if !ok { value = fallback }; return value }\n";
    let py_alias_wrong_key = "from collections.abc import Mapping as MapLike\n\ndef f(lookup: MapLike[str, int], other_lookup: MapLike[str, int], key: str, other_key: str, fallback: int, other_default: int) -> int:\n    return lookup.get(other_key, fallback)\n";
    let py_alias_wrong_default = "from collections.abc import Mapping as MapLike\n\ndef f(lookup: MapLike[str, int], other_lookup: MapLike[str, int], key: str, other_key: str, fallback: int, other_default: int) -> int:\n    return lookup.get(key, other_default)\n";
    let py_alias_wrong_map = "from collections.abc import Mapping as MapLike\n\ndef f(lookup: MapLike[str, int], other_lookup: MapLike[str, int], key: str, other_key: str, fallback: int, other_default: int) -> int:\n    return other_lookup.get(key, fallback)\n";
    let py_alias_unresolved = "def f(lookup: MapLike[str, int], other_lookup: MapLike[str, int], key: str, other_key: str, fallback: int, other_default: int) -> int:\n    return lookup.get(key, fallback)\n";
    let py_alias_shadowed = "from collections.abc import Mapping as MapLike\nMapLike = list\n\ndef f(lookup: MapLike[str, int], other_lookup: MapLike[str, int], key: str, other_key: str, fallback: int, other_default: int) -> int:\n    return lookup.get(key, fallback)\n";
    let guard_wrong_key = "function f(lookup: Map<string, number>, other_lookup: Map<string, number>, key: string, other_key: string, fallback: number, other_default: number): number { if (lookup.has(other_key)) { return lookup.get(other_key)!; } return fallback; }\n";
    let guard_wrong_default = "import java.util.Map;\n\nclass C { static int f(Map<String, Integer> lookup, Map<String, Integer> other_lookup, String key, String other_key, int fallback, int other_default) { if (lookup.containsKey(key)) { return lookup.get(key); } return other_default; } }\n";
    let guard_wrong_map = "def f(lookup: dict[str, int], other_lookup: dict[str, int], key: str, other_key: str, fallback: int, other_default: int) -> int:\n    if key in other_lookup:\n        return other_lookup[key]\n    return fallback\n";

    let fp = value_fp(&i, go, Lang::Go);
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
    let shadowed_undefined = "function f(value, fallback, other, otherDefault, undefined) { return value === undefined ? fallback : value; }";

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
    assert_ne!(fp, value_fp(&i, shadowed_undefined, Lang::JavaScript));
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
    let shadowed_some_pattern = "struct Some<T>(T);\npub fn f(value: Some<i32>) -> bool {\n    if let Some(_) = value { true } else { false }\n}\n";
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
    assert_ne!(
        value_fp(&i, if_some, Lang::Rust),
        value_fp_named(&i, shadowed_some_pattern, Lang::Rust, "f"),
        "a local Rust Some pattern must not be treated as Option::Some"
    );
}

#[test]
fn value_graph_distinguishes_boolean_literals() {
    // `True` and `False` are behavior-defining (like `0`≠`1`): a predicate
    // `if x>0: return True else False` and its negation (booleans swapped) compute
    // opposite results and must not collapse. The bool *value* was abstracted away.
    let i = Interner::new();
    let p = "def f(x: int):\n    if x > 0:\n        return True\n    return False\n";
    let q = "def g(x: int):\n    if x > 0:\n        return False\n    return True\n";
    assert_ne!(
        value_fp(&i, p, Lang::Python),
        value_fp(&i, q, Lang::Python),
        "a predicate and its boolean-swapped negation must not fingerprint identically"
    );
    // Cross-language: the same integer predicate in Java converges with Python.
    let java = "class C { static boolean f(int x) { if (x > 0) { return true; } return false; } }";
    assert_eq!(
        value_fp(&i, p, Lang::Python),
        value_fp(&i, java, Lang::Java),
        "same integer predicate should converge across languages"
    );
    // TypeScript `number` keeps its NaN comparison boundary closed.
    let ts = "function f(x: number): boolean { if (x > 0) { return true; } return false; }";
    assert_ne!(
        value_fp(&i, p, Lang::Python),
        value_fp(&i, ts, Lang::TypeScript),
        "TS number predicates must not merge with integer-only predicates"
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
    let py = "def f(a: int, b: int):\n    p = a * b\n    q = a + b\n    return p + q + p\n";
    let ts = "function g(a: number, b: number): number { const q = a + b; const p = a * b; return p + q + p; }";
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
    // `not (a > b or a == b) ≡ a < b` for integer-proven operands.
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
    let lt_int = value_fp(&i, "def f(a: int,b: int):\n    return a<b\n", Lang::Python);
    assert_eq!(
        lt_int,
        value_fp(
            &i,
            "def g(a: int,b: int):\n    return not (a>b or a==b)\n",
            Lang::Python
        ),
        "De Morgan + comparison-direction must compose into the lattice canon"
    );
    assert_ne!(
        lt,
        value_fp(
            &i,
            "def g(a,b):\n    return not (a>b or a==b)\n",
            Lang::Python
        ),
        "untyped De Morgan plus order negation must keep the NaN boundary closed"
    );
    // Cross-language: a typed TS strict-less written as the conjunction.
    assert_eq!(
        lt,
        value_fp(
            &i,
            "function g(a: number, b: number): boolean { return a <= b && a !== b; }",
            Lang::TypeScript
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
    let ts = "function total(xs: number[]): number {\n  let acc = 0;\n  for (const x of xs) {\n    acc += x;\n  }\n  return acc;\n}\n";
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

/// DISTRIBUTION / FACTORING (domain-gated): `a*c + b*c` ≡ `(a+b)*c`. The value graph factors
/// a shared multiplicand out of a sum of products when every leaf is proven numeric
/// (`value_graph/rules/factor_distribute.rs`, Lean obligation
/// `normalize.value_graph.factor_distribute`). The `*`
/// operands here are statically typed integers, so the factoring fires and the two forms converge.
#[test]
fn distribution_factors_common_multiplicand() {
    let i = Interner::new();
    assert_eq!(
        value_fp(
            &i,
            "fn f(a: i64, b: i64, c: i64) -> i64 { a*c+b*c }\n",
            Lang::Rust
        ),
        value_fp(
            &i,
            "fn g(a: i64, b: i64, c: i64) -> i64 { (a+b)*c }\n",
            Lang::Rust
        ),
        "a*c+b*c should factor to (a+b)*c on proven-numeric leaves"
    );
    // Three-term chain factors transitively: `a*c + b*c + d*c` ≡ `(a+b+d)*c`.
    assert_eq!(
        value_fp(
            &i,
            "fn f(a: i64, b: i64, c: i64, d: i64) -> i64 { a*c+b*c+d*c }\n",
            Lang::Rust
        ),
        value_fp(
            &i,
            "fn g(a: i64, b: i64, c: i64, d: i64) -> i64 { (a+b+d)*c }\n",
            Lang::Rust
        ),
        "a*c+b*c+d*c should factor to (a+b+d)*c"
    );
}

#[test]
fn value_law_domains_keep_string_concat_order_but_numeric_add_commutes() {
    let i = Interner::new();
    let numeric_a_b = "class C { static int f(int a, int b) { return a + b; } }";
    let numeric_b_a = "class C { static int g(int a, int b) { return b + a; } }";
    assert_eq!(
        value_fp(&i, numeric_a_b, Lang::Java),
        value_fp(&i, numeric_b_a, Lang::Java),
        "integer-domain add should still commute under the value-law contract"
    );

    let string_a_b = "class C { static String f(String a, String b) { return a + b; } }";
    let string_b_a = "class C { static String g(String a, String b) { return b + a; } }";
    assert_ne!(
        value_fp(&i, string_a_b, Lang::Java),
        value_fp(&i, string_b_a, Lang::Java),
        "string-domain concat is ordered and must not inherit numeric add laws"
    );
}

#[test]
fn value_law_domains_keep_python_string_repetition_ordered() {
    let i = Interner::new();
    let left_then_right = "def f(s: str, t: str):\n    return s * 2 + t * 2\n";
    let right_then_left = "def g(s: str, t: str):\n    return t * 2 + s * 2\n";
    assert_ne!(
        value_fp(&i, left_then_right, Lang::Python),
        value_fp(&i, right_then_left, Lang::Python),
        "typed Python str repetition feeds ordered concat, not numeric add sorting"
    );
}

#[test]
fn class_value_law_domains_use_method_parameter_evidence() {
    let i = Interner::new();
    let string_a_b = "class C { static String f(String a, String b) { return a + b; } }";
    let string_b_a = "class C { static String f(String a, String b) { return b + a; } }";
    assert_ne!(
        class_value_fp(&i, string_a_b, Lang::Java, "C"),
        class_value_fp(&i, string_b_a, Lang::Java, "C"),
        "class-unit fingerprints must preserve method string-concat order"
    );
}

/// FILTER FUSION: `filter(q, filter(p, xs))` ≡ `filter(p∧q, xs)`. The value graph carries a
/// filter's element so nested filters fuse (`value_graph.rs` `HoFKind::Filter` arm, Lean
/// `normalize.value_graph.functor`). A two-filter comprehension and an explicitly nested one
/// converge; TS `number[]` callbacks stay closed until element-domain proof exists.
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
    assert_ne!(
        value_fp(
            &i,
            "def f(xs):\n    return [x for x in xs if x>0 if x<10]\n",
            Lang::Python
        ),
        value_fp(
            &i,
            "function g(xs: number[]): number[] { return xs.filter(x=>x>0).filter(x=>x<10); }",
            Lang::TypeScript
        ),
        "TypeScript number[] callback elements are not yet numeric proof for relational predicates"
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
            "import functools\n\ndef f(xs):\n    return functools.reduce(lambda a,b: a if a>b else b, xs)\n",
            Lang::Python
        ),
        value_fp(&i, "def g(xs):\n    return max(xs)\n", Lang::Python),
        "reduce(λ. a if a>b else b) should converge with max()"
    );
    assert_eq!(
        value_fp(
            &i,
            "import functools\n\ndef f(xs):\n    return functools.reduce(lambda a,b: a if a<b else b, xs)\n",
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
        ("abs idioms Py", "def f(x: int):\n    return x if x>=0 else -x\n", Lang::Python, "def g(x: int):\n    return abs(x)\n", Lang::Python),
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
    // Python accumulator recursion ≡ a typed TypeScript while loop — the shared IL makes the
    // recursion→iteration rewrite cross-language for free.
    let i = Interner::new();
    let py = "def f(n, acc):\n    if n == 0:\n        return acc\n    return f(n - 1, acc + n)\n";
    let ts = "function g(n: number, acc: number): number { while (n !== 0) { acc = acc + n; n = n - 1; } return acc; }";
    assert_eq!(
        value_fp(&i, py, Lang::Python),
        value_fp(&i, ts, Lang::TypeScript),
        "Python tail recursion and typed TypeScript while loop should converge"
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
fn structural_recursion_consumes_parameter_domain_evidence() {
    let i = Interner::new();
    let rec =
        "def f(n: int, x: int):\n    if n == 0:\n        return 0\n    return x + f(n - 1, x)\n";
    let loopv = "def g(n: int, x: int):\n    acc = 0\n    while n != 0:\n        acc = acc + x\n        n = n - 1\n    return acc\n";
    assert_eq!(
        value_fp(&i, rec, Lang::Python),
        value_fp(&i, loopv, Lang::Python),
        "structural fold law must consume annotation-derived ValueDomain evidence"
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
    let beh = run_unit(&oracle, &i, root, &[Value::Int(5)]).expect("recursion should interpret");
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

#[test]
fn python_true_division_stays_distinct_from_floor_division() {
    // `5 / 2 == 2.5` but `5 // 2 == 2` — collapsing both onto one Div op merged
    // behaviorally-different functions into one semantic family (a false merge).
    let i = Interner::new();
    let true_div = "def f(xs, d):\n    out = []\n    for x in xs:\n        out.append(x / d)\n    return out\n";
    let floor_div = "def g(xs, d):\n    out = []\n    for x in xs:\n        out.append(x // d)\n    return out\n";
    assert_ne!(
        value_fp(&i, true_div, Lang::Python),
        value_fp(&i, floor_div, Lang::Python),
        "true division and floor division must not share a fingerprint"
    );
    // Floor division still converges with itself across renames.
    let floor_div2 = "def h(items, n):\n    res = []\n    for v in items:\n        res.append(v // n)\n    return res\n";
    assert_eq!(
        value_fp(&i, floor_div, Lang::Python),
        value_fp(&i, floor_div2, Lang::Python),
        "alpha-renamed floor divisions must still converge"
    );
}

#[test]
fn python_floor_division_interprets_with_floor_semantics() {
    // The oracle's FloorDiv rounds toward −∞ like Python `//` — NOT toward zero
    // like `Op::Div` — so a bad canonicalization between the two cannot hide.
    let i = Interner::new();
    let il = nose_frontend::lower_source(
        FileId(0),
        "t",
        b"def f(a, b):\n    return a // b\n",
        Lang::Python,
        &i,
    )
    .unwrap();
    let n = normalize(&il, &i, &NormalizeOptions::default());
    let f = first_func(&n);
    use nose_normalize::{run_unit, Value};
    let run = |a: i64, b: i64| {
        run_unit(&n, &i, f, &[Value::Int(a), Value::Int(b)])
            .unwrap()
            .ret
    };
    assert_eq!(run(5, 2), Value::Int(2));
    assert_eq!(run(-5, 2), Value::Int(-3), "-5 // 2 floors to -3");
    assert_eq!(run(5, -2), Value::Int(-3), "5 // -2 floors to -3");
    assert_eq!(run(-5, -2), Value::Int(2));
    assert_eq!(run(7, 0), Value::Err, "division by zero errs");
}

#[test]
fn python_matmul_stays_distinct_from_elementwise_mul() {
    // `a @ b` (matrix product) is not `a * b` (elementwise); mapping `@` onto Mul
    // merged the two. `@` keeps a raw shape keyed by its own spelling.
    let i = Interner::new();
    let mul = "def f(a, b):\n    c = a * b\n    d = a * c\n    return c, d\n";
    let matmul = "def g(a, b):\n    c = a @ b\n    d = a @ c\n    return c, d\n";
    assert_ne!(
        value_fp(&i, mul, Lang::Python),
        value_fp(&i, matmul, Lang::Python),
        "matmul must not share a fingerprint with elementwise mul"
    );
}

#[test]
fn js_unsigned_shift_stays_distinct_from_signed_shift() {
    // `-5 >> 1 == -3` (sign-extends) but `-5 >>> 1 == 2147483645` (zero-fills);
    // collapsing `>>>` onto Shr merged the two shifts.
    let i = Interner::new();
    let signed = "function f(xs, n) {\n  const out = [];\n  for (const x of xs) out.push(x >> n);\n  return out;\n}";
    let unsigned = "function g(xs, n) {\n  const out = [];\n  for (const x of xs) out.push(x >>> n);\n  return out;\n}";
    assert_ne!(
        value_fp(&i, signed, Lang::JavaScript),
        value_fp(&i, unsigned, Lang::JavaScript),
        "signed and unsigned right shift must not share a fingerprint"
    );
    let unsigned2 = "function h(ys, k) {\n  const res = [];\n  for (const y of ys) res.push(y >>> k);\n  return res;\n}";
    assert_eq!(
        value_fp(&i, unsigned, Lang::JavaScript),
        value_fp(&i, unsigned2, Lang::JavaScript),
        "alpha-renamed unsigned shifts must still converge"
    );
}

#[test]
fn js_shift_is_int32_and_distinct_from_arbitrary_precision() {
    // series 9: `& | ^` were narrowed to int32 (#283-D) but `<<`/`>>` were not, so JS
    // `a << b` (shifts ToInt32(a), 32-bit) false-merged with Python's arbitrary-precision
    // `a << b` — e.g. `1 << 31` is -2147483648 in JS but 2147483648 in Python.
    let i = Interner::new();
    let js_shl = "function f(a, b) { return a << b; }";
    let py_shl = "def f(a, b):\n    return a << b\n";
    let js_shr = "function g(a, b) { return a >> b; }";
    let py_shr = "def g(a, b):\n    return a >> b\n";
    assert_ne!(
        value_fp(&i, js_shl, Lang::JavaScript),
        value_fp(&i, py_shl, Lang::Python),
        "JS `<<` is int32; must not merge with arbitrary-precision Python `<<`"
    );
    assert_ne!(
        value_fp(&i, js_shr, Lang::JavaScript),
        value_fp(&i, py_shr, Lang::Python),
        "JS `>>` is int32; must not merge with arbitrary-precision Python `>>`"
    );
    // Recall preserved: same-language shifts (and JS-vs-JS) still converge.
    let js_shl2 = "function h(x, y) { return x << y; }";
    assert_eq!(
        value_fp(&i, js_shl, Lang::JavaScript),
        value_fp(&i, js_shl2, Lang::JavaScript),
        "two JS `<<` must still converge"
    );
    let py_shl2 = "def h(x, y):\n    return x << y\n";
    assert_eq!(
        value_fp(&i, py_shl, Lang::Python),
        value_fp(&i, py_shl2, Lang::Python),
        "two Python `<<` must still converge"
    );
}

#[test]
fn js_mixed_string_addition_keeps_grouping_ordered() {
    // JS `+` is not just numeric add or string concat: when a string participates,
    // later numeric operands are coerced to strings in left-to-right order.
    // `"a" + 2 + 3` is `"a23"`, while `"a" + (2 + 3)` / `"a" + 5` is `"a5"`.
    // Flattening/folding an untyped JS `+` chain therefore false-merges real code.
    let i = Interner::new();
    let left_assoc = "function f(x) { return x + 2 + 3; }";
    let grouped = "function g(x) { return x + (2 + 3); }";
    let folded = "function h(x) { return x + 5; }";
    assert_ne!(
        value_fp(&i, left_assoc, Lang::JavaScript),
        value_fp(&i, grouped, Lang::JavaScript),
        "untyped JS `x + 2 + 3` must not merge with `x + (2 + 3)`"
    );
    assert_ne!(
        value_fp(&i, left_assoc, Lang::JavaScript),
        value_fp(&i, folded, Lang::JavaScript),
        "untyped JS `x + 2 + 3` must not merge with `x + 5`"
    );

    let typed_left = "function f(x: number): number { return x + 2 + 3; }";
    let typed_grouped = "function g(x: number): number { return x + (2 + 3); }";
    assert_eq!(
        value_fp(&i, typed_left, Lang::TypeScript),
        value_fp(&i, typed_grouped, Lang::TypeScript),
        "TypeScript number evidence should preserve numeric associativity recall"
    );

    let sub = "function f(x) { return x - 3; }";
    let add_neg = "function g(x) { return x + (-3); }";
    assert_ne!(
        value_fp(&i, sub, Lang::JavaScript),
        value_fp(&i, add_neg, Lang::JavaScript),
        "untyped JS `x - 3` must not merge with `x + (-3)`"
    );

    let neg_grouped = "function f(x) { return -(x + 2); }";
    let distributed = "function g(x) { return -x - 2; }";
    assert_ne!(
        value_fp(&i, neg_grouped, Lang::JavaScript),
        value_fp(&i, distributed, Lang::JavaScript),
        "untyped JS `-(x + 2)` must not distribute over potentially-string `+`"
    );

    let typed_sub = "function f(x: number): number { return x - 3; }";
    let typed_add_neg = "function g(x: number): number { return x + (-3); }";
    assert_eq!(
        value_fp(&i, typed_sub, Lang::TypeScript),
        value_fp(&i, typed_add_neg, Lang::TypeScript),
        "TypeScript number evidence should preserve subtraction/add-negation recall"
    );
}

#[test]
fn js_value_returning_logical_operators_keep_operand_order() {
    // JS `||`/`&&` return one of the operand values, not a coerced Bool. With
    // `a = "left"` and `b = "right"`, `a || b` returns `a` while `b || a` returns `b`;
    // `a && b` returns `b` while `b && a` returns `a`.
    let i = Interner::new();
    let or_ab = "function f(a, b) { return a || b; }";
    let or_ba = "function g(a, b) { return b || a; }";
    let and_ab = "function h(a, b) { return a && b; }";
    let and_ba = "function k(a, b) { return b && a; }";
    assert_ne!(
        value_fp(&i, or_ab, Lang::JavaScript),
        value_fp(&i, or_ba, Lang::JavaScript),
        "untyped JS `a || b` must not merge with `b || a`"
    );
    assert_ne!(
        value_fp(&i, and_ab, Lang::JavaScript),
        value_fp(&i, and_ba, Lang::JavaScript),
        "untyped JS `a && b` must not merge with `b && a`"
    );

    let bool_or_ab = "function f(a: boolean, b: boolean): boolean { return a || b; }";
    let bool_or_ba = "function g(a: boolean, b: boolean): boolean { return b || a; }";
    let bool_and_ab = "function h(a: boolean, b: boolean): boolean { return a && b; }";
    let bool_and_ba = "function k(a: boolean, b: boolean): boolean { return b && a; }";
    assert_eq!(
        value_fp(&i, bool_or_ab, Lang::TypeScript),
        value_fp(&i, bool_or_ba, Lang::TypeScript),
        "typed boolean `||` should keep commutative recall"
    );
    assert_eq!(
        value_fp(&i, bool_and_ab, Lang::TypeScript),
        value_fp(&i, bool_and_ba, Lang::TypeScript),
        "typed boolean `&&` should keep commutative recall"
    );
}

#[test]
fn js_loose_equality_stays_distinct_from_strict_equality() {
    // JS loose equality coerces (`false == 0`, `"0" == 0`, `[] == 0`), so it is not
    // semantically interchangeable with strict equality except for the intentionally modeled
    // nullish check (`x == null`) that backs `??`.
    let i = Interner::new();
    let loose_zero = "function f(x) { return x == 0; }";
    let loose_zero_swapped = "function g(y) { return 0 == y; }";
    let strict_zero = "function h(x) { return x === 0; }";
    assert_eq!(
        value_fp(&i, loose_zero, Lang::JavaScript),
        value_fp(&i, loose_zero_swapped, Lang::JavaScript),
        "loose equality itself is symmetric and should still converge across operand order"
    );
    assert_ne!(
        value_fp(&i, loose_zero, Lang::JavaScript),
        value_fp(&i, strict_zero, Lang::JavaScript),
        "loose `x == 0` must not merge with strict `x === 0`"
    );

    let loose_ne_zero = "function f(x) { return x != 0; }";
    let strict_ne_zero = "function h(x) { return x !== 0; }";
    assert_ne!(
        value_fp(&i, loose_ne_zero, Lang::JavaScript),
        value_fp(&i, strict_ne_zero, Lang::JavaScript),
        "loose `x != 0` must not merge with strict `x !== 0`"
    );

    let nullish = "function f(x, d) { return x ?? d; }";
    let loose_null = "function g(x, d) { return x == null ? d : x; }";
    let strict_null = "function h(x, d) { return x === null ? d : x; }";
    assert_eq!(
        value_fp(&i, nullish, Lang::JavaScript),
        value_fp(&i, loose_null, Lang::JavaScript),
        "loose `== null` remains the modeled nullish check"
    );
    assert_ne!(
        value_fp(&i, loose_null, Lang::JavaScript),
        value_fp(&i, strict_null, Lang::JavaScript),
        "strict null equality must stay separate from the nullish loose check"
    );
}

#[test]
fn js_instanceof_stays_distinct_from_equality() {
    // `instanceof` tests a value's prototype chain. It is not equality:
    // `[] instanceof Array` is true, while `[] === Array` is false.
    let i = Interner::new();
    let membership = "function f(x, C) { return x instanceof C; }";
    let renamed_membership = "function h(value, Type) { return value instanceof Type; }";
    let equality = "function g(x, C) { return x === C; }";
    assert_eq!(
        value_fp(&i, membership, Lang::JavaScript),
        value_fp(&i, renamed_membership, Lang::JavaScript),
        "`instanceof` should still converge with the same directional source surface"
    );
    assert_ne!(
        value_fp(&i, membership, Lang::JavaScript),
        value_fp(&i, equality, Lang::JavaScript),
        "`x instanceof C` must not merge with `x === C`"
    );

    let not_membership = "function f(x, C) { return !(x instanceof C); }";
    let not_renamed_membership = "function h(value, Type) { return !(value instanceof Type); }";
    let strict_inequality = "function g(x, C) { return x !== C; }";
    assert_eq!(
        value_fp(&i, not_membership, Lang::JavaScript),
        value_fp(&i, not_renamed_membership, Lang::JavaScript),
        "negated `instanceof` should still converge with the same source surface"
    );
    assert_ne!(
        value_fp(&i, not_membership, Lang::JavaScript),
        value_fp(&i, strict_inequality, Lang::JavaScript),
        "`!(x instanceof C)` must not merge with `x !== C`"
    );
}

#[test]
fn js_relational_comparison_stays_distinct_from_typed_numeric_comparison() {
    // JS relational comparison is not purely numeric for untyped operands:
    // `"2" < "10"` is false because both operands are strings, while `2 < 10` is true.
    let i = Interner::new();
    let js_lt = "function f(a, b) { return a < b; }";
    let ts_lt = "function g(a: number, b: number): boolean { return a < b; }";
    let ts_gt = "function h(a: number, b: number): boolean { return b > a; }";
    assert_eq!(
        value_fp(&i, ts_lt, Lang::TypeScript),
        value_fp(&i, ts_gt, Lang::TypeScript),
        "typed numeric TS comparison should keep primitive comparison laws"
    );
    assert_ne!(
        value_fp(&i, js_lt, Lang::JavaScript),
        value_fp(&i, ts_lt, Lang::TypeScript),
        "untyped JS `<` must not merge with typed numeric `<`"
    );

    let not_lt = "function f(a, b) { return !(a < b); }";
    let ge = "function g(a, b) { return a >= b; }";
    assert_ne!(
        value_fp(&i, not_lt, Lang::JavaScript),
        value_fp(&i, ge, Lang::JavaScript),
        "JS `!(a < b)` must not merge with `a >= b` because NaN makes them differ"
    );

    let py_not_lt = "def f(a, b):\n    return not (a < b)\n";
    let py_ge = "def g(a, b):\n    return a >= b\n";
    assert_ne!(
        value_fp(&i, py_not_lt, Lang::Python),
        value_fp(&i, py_ge, Lang::Python),
        "Python `not (a < b)` must not merge with `a >= b` because NaN makes them differ"
    );
    let py_int_not_lt = "def f(a: int, b: int):\n    return not (a < b)\n";
    let py_int_ge = "def g(a: int, b: int):\n    return a >= b\n";
    assert_eq!(
        value_fp(&i, py_int_not_lt, Lang::Python),
        value_fp(&i, py_int_ge, Lang::Python),
        "integer-proven Python order negation can use total-order duals"
    );
}

#[test]
fn ruby_star_repetition_is_ordered_but_other_multiply_commutes() {
    // series 9: `*` is string/array REPETITION in Ruby and asymmetric — `"ab" * 3` →
    // "ababab" but `3 * "ab"` raises (`Integer#*` rejects a String). Reordering its
    // operands (the algebra pass folded a constant to the end; the value graph sorted
    // by hash) false-merged the two. Only Ruby is gated: Python repetition commutes and
    // JS/Java/Go/C `*` is numeric.
    let i = Interner::new();
    let rb_str_first = "def a\n  \"ab\" * 3\nend\n";
    let rb_int_first = "def b\n  3 * \"ab\"\nend\n";
    assert_ne!(
        value_fp(&i, rb_str_first, Lang::Ruby),
        value_fp(&i, rb_int_first, Lang::Ruby),
        "Ruby `\"ab\" * 3` (repeats) must not merge with `3 * \"ab\"` (raises)"
    );
    let rb_arr_first = "def a\n  [1, 2] * 3\nend\n";
    let rb_arr_int_first = "def b\n  3 * [1, 2]\nend\n";
    assert_ne!(
        value_fp(&i, rb_arr_first, Lang::Ruby),
        value_fp(&i, rb_arr_int_first, Lang::Ruby),
        "Ruby `[1,2] * 3` (repeats) must not merge with `3 * [1,2]` (raises)"
    );
    // Largest-sound-generalization guard: only Ruby is gated.
    let js_xy = "function p(x, y) { return x * y; }";
    let js_yx = "function q(x, y) { return y * x; }";
    assert_eq!(
        value_fp(&i, js_xy, Lang::JavaScript),
        value_fp(&i, js_yx, Lang::JavaScript),
        "JS `x * y` is numeric and must still commute with `y * x`"
    );
    let py_sx = "def p(s):\n    return s * 3\n";
    let py_xs = "def q(s):\n    return 3 * s\n";
    assert_eq!(
        value_fp(&i, py_sx, Lang::Python),
        value_fp(&i, py_xs, Lang::Python),
        "Python `s * 3` repetition commutes (`3 * s` is equal) and must still converge"
    );
}

#[test]
fn js_nullish_assignment_desugars_to_nullish_coalescing() {
    // `x ??= y` is `x = x ?? y` — and is NOT `x += y` (the old unmapped-operator
    // fallback silently defaulted compound assignments to Add).
    let i = Interner::new();
    let compound = "function f(x, y) {\n  x ??= y;\n  return x;\n}";
    let spelled = "function g(x, y) {\n  x = x ?? y;\n  return x;\n}";
    let add = "function h(x, y) {\n  x += y;\n  return x;\n}";
    assert_eq!(
        value_fp(&i, compound, Lang::JavaScript),
        value_fp(&i, spelled, Lang::JavaScript),
        "`x ??= y` should converge with `x = x ?? y`"
    );
    assert_ne!(
        value_fp(&i, compound, Lang::JavaScript),
        value_fp(&i, add, Lang::JavaScript),
        "`x ??= y` must not merge with `x += y`"
    );
}

#[test]
fn dataflow_does_not_unsoundly_inline_a_temp_past_a_write_or_into_a_lambda() {
    // series 9 oracle residue: the copy-propagation inliner must not move a temp's
    // (possibly-raising) read into a position evaluated under a different condition.
    // Two cases, both verified to keep the temp's `Var` binding after normalization:
    //   - `t = a[i]; a[i] = a[j]; a[j] = t` — inlining `t` past the indexed write that
    //     clobbers `a[i]` would silently turn a swap into "set both to a[j]".
    //   - `ind = nodes[k]; [x for x in d if nodes[x] == ind]` — inlining `ind` into the
    //     filter lambda elides its `Err` when `d` is empty (the lambda never runs).
    use nose_il::NodeKind;
    let i = Interner::new();
    let binds_a_var_temp = |il: &nose_il::Il| -> bool {
        let mut stack = vec![first_func(il)];
        while let Some(n) = stack.pop() {
            if il.kind(n) == NodeKind::Assign {
                if let Some(&lhs) = il.children(n).first() {
                    if il.kind(lhs) == NodeKind::Var {
                        return true;
                    }
                }
            }
            stack.extend(il.children(n).iter().copied());
        }
        false
    };
    let normalized = |src: &str, lang: Lang| {
        let il = nose_frontend::lower_source(FileId(0), "t", src.as_bytes(), lang, &i).unwrap();
        normalize(&il, &i, &NormalizeOptions::default())
    };
    let swap = "def swap(a, i, j):\n    t = a[i]\n    a[i] = a[j]\n    a[j] = t\n";
    assert!(
        binds_a_var_temp(&normalized(swap, Lang::Python)),
        "swap's `t = a[i]` must survive — inlining it past `a[i] = a[j]` is unsound",
    );
    let comp =
        "def f(d, nodes, k):\n    ind = nodes[k]\n    return [x for x in d if nodes[x] == ind]\n";
    assert!(
        binds_a_var_temp(&normalized(comp, Lang::Python)),
        "comprehension's `ind = nodes[k]` must not inline into the filter lambda",
    );
}

#[test]
fn js_strict_null_ternary_stays_distinct_from_nullish_coalescing() {
    // `x ?? d` and `x == null ? d : x` both default null AND undefined — they are
    // the same computation. `x === null ? d : x` passes undefined through, so it
    // must NOT join that family (it differs on every undefined input).
    let i = Interner::new();
    let nullish = "function f(x, d) {\n  return x ?? d;\n}";
    let loose = "function g(x, d) {\n  return x == null ? d : x;\n}";
    let strict = "function h(x, d) {\n  return x === null ? d : x;\n}";
    assert_eq!(
        value_fp(&i, nullish, Lang::JavaScript),
        value_fp(&i, loose, Lang::JavaScript),
        "`??` should converge with the loose-equality ternary"
    );
    assert_ne!(
        value_fp(&i, nullish, Lang::JavaScript),
        value_fp(&i, strict, Lang::JavaScript),
        "`??` must not merge with the strict-null ternary"
    );
    // Strict checks still converge with the same strict spelling…
    let strict2 = "function k(v, fb) {\n  return v === null ? fb : v;\n}";
    assert_eq!(
        value_fp(&i, strict, Lang::JavaScript),
        value_fp(&i, strict2, Lang::JavaScript),
        "alpha-renamed strict-null ternaries must still converge"
    );
    // …but `=== null` and `=== undefined` are different checks.
    let strict_undef = "function m(x, d) {\n  return x === undefined ? d : x;\n}";
    assert_ne!(
        value_fp(&i, strict, Lang::JavaScript),
        value_fp(&i, strict_undef, Lang::JavaScript),
        "`=== null` and `=== undefined` must not share a fingerprint"
    );
}

#[test]
fn java_unsigned_shift_assignment_keeps_its_operator() {
    // Java `x >>>= y` used to fall through the unmapped-compound path and lower
    // as a plain `x = y` — merging it with reassignment.
    let i = Interner::new();
    let ushift = "class C { static int f(int x, int y) { x >>>= y; return x; } }";
    let assign = "class D { static int g(int x, int y) { x = y; return x; } }";
    let add = "class E { static int h(int x, int y) { x += y; return x; } }";
    assert_ne!(
        value_fp(&i, ushift, Lang::Java),
        value_fp(&i, assign, Lang::Java),
        "`x >>>= y` must not merge with `x = y`"
    );
    assert_ne!(
        value_fp(&i, ushift, Lang::Java),
        value_fp(&i, add, Lang::Java),
        "`x >>>= y` must not merge with `x += y`"
    );
}

#[test]
fn ruby_exponent_converges_with_python_pow() {
    // Ruby `**` was unmapped (raw); it is the same exponentiation Python spells `**`.
    let i = Interner::new();
    let rb = "def area(base, exp)\n  base ** exp\nend\n";
    let py = "def area(base, exp):\n    return base ** exp\n";
    assert_eq!(
        value_fp(&i, rb, Lang::Ruby),
        value_fp(&i, py, Lang::Python),
        "Ruby `**` should converge with Python `**`"
    );
}

#[test]
fn two_argument_min_max_interpret_as_two_way_selection() {
    // `min(a, b)` (the 2-way selection `[a, b].min()` also canonicalizes to) used to
    // evaluate to Err in the oracle — leaving exactly the convergences the value
    // graph claims for it unverifiable.
    let i = Interner::new();
    let il = nose_frontend::lower_source(
        FileId(0),
        "t",
        b"def f(a, b):\n    return min(a, b), max(a, b)\n",
        Lang::Python,
        &i,
    )
    .unwrap();
    let n = normalize(&il, &i, &NormalizeOptions::default());
    let f = first_func(&n);
    use nose_normalize::{run_unit, Value};
    let out = run_unit(&n, &i, f, &[Value::Int(3), Value::Int(1)])
        .expect("two-scalar min/max is interpretable")
        .ret;
    assert_eq!(
        out,
        Value::List(vec![Value::Int(1), Value::Int(3)]),
        "min(3, 1) is 1 and max(3, 1) is 3"
    );
}

/// Ruby `for x in xs … out << e` is the same list build as a Python comprehension:
/// `for..in` is a language construct (no receiver proof needed, unlike `each`), and
/// the shovel is admitted as an append ONLY through the active-builder seed proof
/// (`out = []`). The shovel operator alone proves nothing — an integer-seeded `<<`
/// stays a shift, and a parameter receiver (no seed) never becomes a builder.
#[test]
fn ruby_for_in_shovel_builder_converges_with_comprehension() {
    let i = Interner::new();
    let comp = value_fp(
        &i,
        "def f(xs):\n    return [x * x for x in xs]\n",
        Lang::Python,
    );
    let ruby_for = value_fp(
        &i,
        "def f(xs)\n  out = []\n  for x in xs\n    out << x * x\n  end\n  out\nend\n",
        Lang::Ruby,
    );
    assert_eq!(comp, ruby_for, "ruby for-in shovel builder ≡ comprehension");

    // Adjacent hard negative: a different per-element contribution stays distinct.
    let ruby_diff = value_fp(
        &i,
        "def f(xs)\n  out = []\n  for x in xs\n    out << x + 1\n  end\n  out\nend\n",
        Lang::Ruby,
    );
    assert_ne!(
        ruby_for, ruby_diff,
        "different contribution must stay distinct"
    );

    // Hard negative: an integer-seeded `<<` is a SHIFT — must not become a builder
    // (and must stay distinct from a doubling accumulator, which it behaviorally is not
    // for non-trivial seeds; here the point is it must not merge with the list build).
    let ruby_shift = value_fp(
        &i,
        "def f(xs)\n  acc = 1\n  for x in xs\n    acc = acc << 1\n  end\n  acc\nend\n",
        Lang::Ruby,
    );
    assert_ne!(
        ruby_for, ruby_shift,
        "integer shovel is a shift, not an append"
    );

    // Hard negative: a parameter receiver has no empty-list seed proof, so its
    // shovel never builds — the loop keeps its opaque per-element effect.
    let ruby_param = value_fp(
        &i,
        "def f(xs, out)\n  for x in xs\n    out << x * x\n  end\n  out\nend\n",
        Lang::Ruby,
    );
    assert_ne!(
        comp, ruby_param,
        "shovel to an unproven (parameter) receiver must stay closed"
    );
}

/// The bare Ruby `for x in xs` loop converges with Python's: tree-sitter-ruby wraps
/// the iterable in an `in` node, which must lower to the iterable itself, not an
/// exact-unsafe `Raw("in")`.
#[test]
fn ruby_for_in_loop_converges_with_python_for() {
    let i = Interner::new();
    let rb = value_fp(
        &i,
        "def f(xs)\n  for x in xs\n    y = x\n  end\n  0\nend\n",
        Lang::Ruby,
    );
    let py = value_fp(
        &i,
        "def f(xs):\n    for x in xs:\n        y = x\n    return 0\n",
        Lang::Python,
    );
    assert_eq!(rb, py, "ruby for-in ≡ python for (no Raw iterable wrapper)");
}

/// #244 — bounded symbolic-condition path exploration. Branching on an opaque
/// call's symbolic result no longer bails the unit under `run_unit_paths`: both
/// arms run, each path's trace records its assumption as a Sym marker (so the
/// behaviors stay symbolic → advisory lane), and the strict `run_unit` contract
/// is unchanged (still bails).
#[test]
fn symbolic_condition_paths_explore_both_arms() {
    use nose_normalize::{run_unit, Value};
    let i = Interner::new();
    let src = "def f(x):\n    if g(x):\n        return 1\n    return 2\n";
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

    // Strict contract unchanged: a symbolic condition bails run_unit.
    assert!(
        run_unit(&oracle, &i, root, &[Value::Int(3)]).is_none(),
        "strict run_unit must still bail on a symbolic condition"
    );

    let mut cap = false;
    let paths = nose_normalize::run_unit_paths(&oracle, &i, root, &[Value::Int(3)], &mut cap)
        .expect("two-arm exploration interprets the unit");
    assert!(!cap, "one site is within the exploration cap");
    assert_eq!(paths.len(), 2, "one symbolic site forks exactly two paths");
    assert_eq!(
        paths[0].ret,
        Value::Int(1),
        "true arm first (deterministic)"
    );
    assert_eq!(paths[1].ret, Value::Int(2), "false arm second");
    for p in &paths {
        assert!(
            nose_normalize::behavior_has_sym(p),
            "every explored path carries its Sym assumption marker: {p:?}"
        );
    }
    assert_ne!(
        paths[0].effects, paths[1].effects,
        "the two arms record different assumptions"
    );

    // Differential alignment: the SAME shape over the same opaque call agrees…
    let twin = nose_normalize::run_unit_paths(&oracle, &i, root, &[Value::Int(3)], &mut cap)
        .expect("twin run");
    assert_eq!(paths, twin, "deterministic across runs");
    // …while branching on a DIFFERENT opaque call yields different assumptions.
    let src_other = "def f(x):\n    if h(x):\n        return 1\n    return 2\n";
    let il2 = nose_frontend::lower_source(FileId(0), "t", src_other.as_bytes(), Lang::Python, &i)
        .unwrap();
    let oracle2 = normalize(
        &il2,
        &i,
        &NormalizeOptions {
            oracle: true,
            ..NormalizeOptions::default()
        },
    );
    let other = nose_normalize::run_unit_paths(
        &oracle2,
        &i,
        first_func(&oracle2),
        &[Value::Int(3)],
        &mut cap,
    )
    .expect("other unit");
    assert_ne!(
        paths, other,
        "a different opaque condition must not align (different assumption markers)"
    );
}

/// #244 fail-closed: more symbolic decision sites than the cap → path-bail,
/// reported via the out-flag, never guessed.
#[test]
fn symbolic_condition_paths_fail_closed_past_the_cap() {
    use nose_normalize::Value;
    let i = Interner::new();
    // 4 sequential symbolic decisions > MAX_SYM_BRANCH_SITES (3).
    let src = "def f(x):\n    a = 1 if g(x) else 2\n    b = 1 if h(x) else 2\n    c = 1 if p(x) else 2\n    d = 1 if q(x) else 2\n    return a + b + c + d\n";
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
    let mut cap = false;
    let out = nose_normalize::run_unit_paths(&oracle, &i, root, &[Value::Int(3)], &mut cap);
    assert!(out.is_none(), "past the site cap the unit fails closed");
    assert!(
        cap,
        "the bail is reported as a path-cap bail for the census"
    );
}

#[test]
fn effectful_commutative_operands_do_not_reorder() {
    // coevo §CE / #283-A: `print(a) + print(b)` commutes by VALUE but the
    // interpreter observes effect order, so reordering it to `print(b) + print(a)`
    // is a false merge. Effect-bearing operands must hold their position; only
    // effect-free numeric operands reorder.
    let i = Interner::new();
    let fwd = "def f(a, b):\n    return print(a) + print(b)\n";
    let rev = "def g(a, b):\n    return print(b) + print(a)\n";
    assert_ne!(
        value_fp(&i, fwd, Lang::Python),
        value_fp(&i, rev, Lang::Python),
        "effectful commutative operands must not reorder into one fingerprint"
    );
    let chain_fwd = "def f(a, b, c):\n    return print(a) + print(b) + print(c)\n";
    let chain_rev = "def g(a, b, c):\n    return print(c) + print(b) + print(a)\n";
    assert_ne!(
        value_fp(&i, chain_fwd, Lang::Python),
        value_fp(&i, chain_rev, Lang::Python),
        "effectful AC chains must not sort into one fingerprint"
    );
    // Effect-FREE numeric operands still converge (the common case is unaffected).
    let pure_fwd = "def f(a, b):\n    return a + b + 1\n";
    let pure_rev = "def g(a, b):\n    return 1 + b + a\n";
    assert_eq!(
        value_fp(&i, pure_fwd, Lang::Python),
        value_fp(&i, pure_rev, Lang::Python),
        "effect-free numeric commutative operands must still converge"
    );
}

#[test]
fn sound_recall_rules_converge_with_hard_negatives() {
    // #284 (coevo §CE / S5-C4): behaviorally-equal forms that nose now converges.
    // The abs law is integer-gated; the bitwise laws preserve error behavior on
    // both sides for non-integer inputs.
    let i = Interner::new();

    // abs(abs x) ≡ abs x for integer-proven operands.
    let abs_nested =
        "def f(x: int):\n    a = x if x >= 0 else -x\n    return a if a >= 0 else -a\n";
    let abs_once = "def g(x: int):\n    return x if x >= 0 else -x\n";
    assert_eq!(
        value_fp(&i, abs_nested, Lang::Python),
        value_fp(&i, abs_once, Lang::Python),
        "abs(abs x) must converge with abs x"
    );

    // ~(a&b) ≡ ~a|~b — bitwise De Morgan; a non-integer Errs on both.
    let demorgan_l = "def f(a, b):\n    return ~(a & b)\n";
    let demorgan_r = "def g(a, b):\n    return (~a) | (~b)\n";
    assert_eq!(
        value_fp(&i, demorgan_l, Lang::Python),
        value_fp(&i, demorgan_r, Lang::Python),
        "~(a&b) must converge with ~a|~b"
    );
    // Hard negative: ~(a|b) ≡ ~a&~b, and must NOT collide with the AND form.
    let demorgan_or = "def h(a, b):\n    return ~(a | b)\n";
    assert_ne!(
        value_fp(&i, demorgan_l, Lang::Python),
        value_fp(&i, demorgan_or, Lang::Python),
        "~(a&b) and ~(a|b) are different functions"
    );

    // max(max(a,b),c) ≡ max(a,max(b,c)) — associative on the ternary semantics
    // (total for all inputs, incl. NaN). Hard negative: min vs max stays distinct.
    let max_l = "def f(a, b, c):\n    m = a if a > b else b\n    return m if m > c else c\n";
    let max_r = "def g(a, b, c):\n    n = b if b > c else c\n    return a if a > n else n\n";
    assert_eq!(
        value_fp(&i, max_l, Lang::Python),
        value_fp(&i, max_r, Lang::Python),
        "nested max must flatten and converge"
    );
    let min_l = "def h(a, b, c):\n    m = a if a < b else b\n    return m if m < c else c\n";
    assert_ne!(
        value_fp(&i, max_l, Lang::Python),
        value_fp(&i, min_l, Lang::Python),
        "max and min chains must stay distinct"
    );
}

#[test]
fn floored_mod_distinguishes_python_ruby_from_c_family() {
    // #283-D: Python/Ruby `%` is FLOORED (remainder takes the divisor's sign);
    // C/Go/Java/JS/Rust `%` is TRUNCATED (dividend's sign). They differ on
    // sign-disagreeing operands (`-1 % 3 == 2` vs `== -1`), so a single `Op::Mod`
    // for all languages was a false merge the interpreter was blind to.
    let i = Interner::new();
    let py = "def rem(a, b):\n    return a % b\n";
    let rb = "def rem(a, b)\n  a % b\nend\n";
    let js = "function rem(a, b){ return a % b; }";
    let go = "package p\nfunc Rem(a int, b int) int { return a % b }\n";

    // Floored ≠ truncated: Python must NOT converge with JS.
    assert_ne!(
        value_fp(&i, py, Lang::Python),
        value_fp(&i, js, Lang::JavaScript),
        "Python floored % must not merge with JS truncated %"
    );
    // Same semantics still converge: Python ≡ Ruby (both floored).
    assert_eq!(
        value_fp(&i, py, Lang::Python),
        value_fp(&i, rb, Lang::Ruby),
        "Python and Ruby % are both floored — must converge"
    );
    // JS ≡ Go (both truncated).
    assert_eq!(
        value_fp(&i, js, Lang::JavaScript),
        value_fp(&i, go, Lang::Go),
        "JS and Go % are both truncated — must converge"
    );
}

#[test]
fn double_negation_cancels_only_for_proven_numeric() {
    // #283-B: `-(-a) → a` is sound ONLY when `a` is a number — on a list `-a` Errs, so
    // `-(-a)` Errs while `a` does not. The value graph used to infer `a: Num` from the very
    // `-` it was about to delete (optimistic), and the algebra pass cancelled `-(-x)`
    // unconditionally. Both are fixed: an UNTYPED param keeps `-(-a)` distinct from `a`; a
    // genuinely-typed (annotated) param still cancels, preserving sound recall.
    let i = Interner::new();
    let negneg_untyped = "def f(a):\n    return -(-a)\n";
    let ident_untyped = "def f(a):\n    return a\n";
    let negneg_typed = "def f(a: int):\n    return -(-a)\n";
    let ident_typed = "def f(a: int):\n    return a\n";

    assert_ne!(
        value_fp(&i, negneg_untyped, Lang::Python),
        value_fp(&i, ident_untyped, Lang::Python),
        "untyped -(-a) must NOT merge with a (it Errs on a list; a does not)"
    );
    assert_eq!(
        value_fp(&i, negneg_typed, Lang::Python),
        value_fp(&i, ident_typed, Lang::Python),
        "int-annotated -(-a) is provably numeric — must still cancel to a"
    );
}

#[test]
fn bitwise_self_idempotence_gates_on_proven_numeric() {
    // #283-B: `a & a → a` / `a | a → a` is sound only for integers (`[1] & [1]` Errs in
    // Python while `[1]` does not). The optimistic value domain inferred `a: Num` from the
    // `&`/`|` itself; now an untyped param stays distinct, an annotated one still folds.
    let i = Interner::new();
    let untyped_and = "def f(a):\n    return a & a\n";
    let untyped_id = "def f(a):\n    return a\n";
    let typed_and = "def f(a: int):\n    return a & a\n";
    let typed_id = "def f(a: int):\n    return a\n";

    assert_ne!(
        value_fp(&i, untyped_and, Lang::Python),
        value_fp(&i, untyped_id, Lang::Python),
        "untyped a & a must NOT merge with a"
    );
    assert_eq!(
        value_fp(&i, typed_and, Lang::Python),
        value_fp(&i, typed_id, Lang::Python),
        "int-annotated a & a is provably numeric — must still fold to a"
    );
}

#[test]
fn untyped_add_commute_gates_on_proven_numeric() {
    // #283-C: `a + b → b + a` (commuting the operands of `+`) is sound only when both are
    // numbers — for strings/lists `+` is ORDERED concat (`"x"+"y" != "y"+"x"`). The detector
    // reordered untyped `+` optimistically. Now the reorder gates on proven-numeric operands:
    // an untyped `a+b` stays distinct from `b+a`, while an int-annotated one still converges.
    let i = Interner::new();
    let fwd_untyped = "def f(a, b):\n    return a + b\n";
    let rev_untyped = "def f(a, b):\n    return b + a\n";
    let fwd_typed = "def f(a: int, b: int):\n    return a + b\n";
    let rev_typed = "def f(a: int, b: int):\n    return b + a\n";

    assert_ne!(
        value_fp(&i, fwd_untyped, Lang::Python),
        value_fp(&i, rev_untyped, Lang::Python),
        "untyped a + b must NOT merge with b + a (string concat is ordered)"
    );
    assert_eq!(
        value_fp(&i, fwd_typed, Lang::Python),
        value_fp(&i, rev_typed, Lang::Python),
        "int-annotated a + b is provably numeric — commuting to b + a must still converge"
    );
}

#[test]
fn js_int32_bitwise_distinguished_from_arbitrary_precision() {
    // #283-D: JS bitwise coerces operands to int32 (`a & b` is `ToInt32(a) & ToInt32(b)`),
    // while Python/Ruby bitwise is arbitrary-precision. They differ for operands outside
    // int32 range (`2^40 & 2^40` is `0` in JS, `2^40` in Python), so one `Bin(BitAnd)` for
    // both was a false merge. JS bitwise leaves now carry a `ToInt32` wrap → distinct
    // fingerprint; within-JS `&` still commutes; the De Morgan canon still fires.
    let i = Interner::new();
    let js = "function f(a, b){ return a & b; }";
    let py = "def f(a, b):\n    return a & b\n";
    let js_swapped = "function g(a, b){ return b & a; }";
    let js_demorgan_a = "function f(a, b){ return ~(a & b); }";
    let js_demorgan_b = "function g(a, b){ return (~a) | (~b); }";

    assert_ne!(
        value_fp(&i, js, Lang::JavaScript),
        value_fp(&i, py, Lang::Python),
        "JS int32 `&` must not merge with Python arbitrary-precision `&`"
    );
    assert_eq!(
        value_fp(&i, js, Lang::JavaScript),
        value_fp(&i, js_swapped, Lang::JavaScript),
        "within JS, `a & b` still commutes with `b & a`"
    );
    assert_eq!(
        value_fp(&i, js_demorgan_a, Lang::JavaScript),
        value_fp(&i, js_demorgan_b, Lang::JavaScript),
        "De Morgan `~(a&b) ≡ ~a|~b` still holds for JS int32 bitwise"
    );
}

#[test]
fn true_div_distinguishes_three_way_division() {
    // #283-D: `/` is three-way — TRUE-float in Python/JS (`7/2 == 3.5`), FLOORED-int in
    // Ruby (`7/2 == 3`, like Python `//`), TRUNCATED-int in C/Go/Java/Rust (`-7/2 == -3`).
    // One `Op::Div` for all was a false merge; Python/JS `/` now lower to `Op::TrueDiv`,
    // Ruby `/` to `Op::FloorDiv`, C-family stays `Op::Div`.
    let i = Interner::new();
    let py = "def f(a, b):\n    return a / b\n";
    let js = "function f(a, b){ return a / b; }";
    let rb = "def f(a, b)\n  a / b\nend\n";
    let c = "int f(int a, int b) { return a / b; }";
    let py_floor = "def f(a, b):\n    return a // b\n";

    // True-float (py/js) ≠ truncated (c) ≠ floored (ruby).
    assert_ne!(
        value_fp(&i, py, Lang::Python),
        value_fp(&i, c, Lang::C),
        "Python true-float / must not merge with C truncated /"
    );
    assert_ne!(
        value_fp(&i, py, Lang::Python),
        value_fp(&i, rb, Lang::Ruby),
        "Python true-float / must not merge with Ruby floored /"
    );
    assert_ne!(
        value_fp(&i, rb, Lang::Ruby),
        value_fp(&i, c, Lang::C),
        "Ruby floored / must not merge with C truncated /"
    );
    // Same semantics still converge: Python ≡ JS (true-float); Ruby ≡ Python `//` (floored).
    assert_eq!(
        value_fp(&i, py, Lang::Python),
        value_fp(&i, js, Lang::JavaScript),
        "Python and JS / are both true-float — must converge"
    );
    assert_eq!(
        value_fp(&i, rb, Lang::Ruby),
        value_fp(&i, py_floor, Lang::Python),
        "Ruby / and Python // are both floored — must converge"
    );
}

#[test]
fn reinvented_helper_containment_fires_and_excludes_callers() {
    // The containment channel: a function that REIMPLEMENTS an existing pure helper
    // inline (without calling it) is reported, with the helper's whole-body value hash
    // matched as an interior sub-DAG anchor of the container. A function that CALLS the
    // helper — whose fingerprint contains the same hash via interprocedural inlining —
    // must NOT be reported: calling is the fix, not the smell.
    let i = Interner::new();
    let opts = DetectOptions {
        min_lines: 1,
        min_tokens: 1,
        ..Default::default()
    };
    // File 0: a straight-line pure helper big enough for the helper floor, plus a
    // caller of it (inlined → contains the helper's value graph → must be excluded).
    let helper_and_caller = "function big(x, y) {\n    return ((x * 2 + 3) * (x - 4)) / ((x + 5) * (y - 7) + (y * y + 11))\n}\n\nfunction use(x, y) {\n    return big(x, y) + 1\n}\n";
    // File 1: a function that reimplements `big`'s computation inline and does more.
    let reinventor = "function manual(x, y) {\n    return (((x * 2 + 3) * (x - 4)) / ((x + 5) * (y - 7) + (y * y + 11))) * 7\n}\n";
    let il0 = nose_frontend::lower_source(
        FileId(0),
        "a.js",
        helper_and_caller.as_bytes(),
        Lang::JavaScript,
        &i,
    )
    .unwrap();
    let il1 = nose_frontend::lower_source(
        FileId(1),
        "b.js",
        reinventor.as_bytes(),
        Lang::JavaScript,
        &i,
    )
    .unwrap();
    let mut units = nose_detect::units_of_file(&il0, &i, &opts);
    units.extend(nose_detect::units_of_file(&il1, &i, &opts));
    let findings = nose_detect::reinvented_helpers(&units);
    assert_eq!(
        findings.len(),
        1,
        "exactly the reimplementation (not the caller) must be reported",
    );
    let f = &findings[0];
    assert_eq!(
        (f.container_file.as_str(), f.container_name.as_deref()),
        ("b.js", Some("manual")),
        "the container is the function reimplementing the helper",
    );
    assert_eq!(
        (f.helper_file.as_str(), f.helper_name.as_deref()),
        ("a.js", Some("big")),
        "the helper is the existing function being reinvented",
    );
}

#[test]
fn reinvented_helper_skips_effectful_and_guard_mismatched_helpers() {
    // An effectful helper (its sink profile is not pure-single-return) never becomes a
    // containment helper — replacing inline code with a call would ADD its effect.
    let i = Interner::new();
    let opts = DetectOptions {
        min_lines: 1,
        min_tokens: 1,
        ..Default::default()
    };
    let eff_helper = "function bigLog(x, y, log) {\n    log.push(x)\n    return ((x * 2 + 3) * (x - 4)) / ((x + 5) * (y - 7) + (y * y + 11))\n}\n";
    let reinventor = "function manual(x, y) {\n    return (((x * 2 + 3) * (x - 4)) / ((x + 5) * (y - 7) + (y * y + 11))) * 7\n}\n";
    let il0 = nose_frontend::lower_source(
        FileId(0),
        "a.js",
        eff_helper.as_bytes(),
        Lang::JavaScript,
        &i,
    )
    .unwrap();
    let il1 = nose_frontend::lower_source(
        FileId(1),
        "b.js",
        reinventor.as_bytes(),
        Lang::JavaScript,
        &i,
    )
    .unwrap();
    let mut units = nose_detect::units_of_file(&il0, &i, &opts);
    units.extend(nose_detect::units_of_file(&il1, &i, &opts));
    assert!(
        nose_detect::reinvented_helpers(&units).is_empty(),
        "an effectful helper must not produce containment findings",
    );
}

#[test]
fn decorated_function_callers_fail_closed_coevo_s6_s2a() {
    // coevo series 6, S2-A: Python decorators are dropped in lowering, so a decorated
    // helper's runtime binding is `decorator(f)`, not the bare body. The fix records a
    // SourceFactKind::Binding(DecoratedDefinition) fact so the decorated def gets no
    // DirectFunction evidence — its callers fall back to an opaque call and are NOT
    // admitted to the exact `semantic` channel (exact_safe=false), so they can never be
    // reported as an "exact behavior match" that hides the decorator's effect. A caller
    // of a PLAIN helper still inlines and stays exact_safe.
    let i = Interner::new();
    let opts = DetectOptions {
        min_lines: 1,
        min_tokens: 1,
        ..Default::default()
    };
    let caller_exact_safe = |src: &str| -> bool {
        let il = nose_frontend::lower_source(FileId(0), "t.py", src.as_bytes(), Lang::Python, &i)
            .unwrap();
        nose_detect::units_of_file(&il, &i, &opts)
            .iter()
            .find(|u| u.name.as_deref() == Some("caller"))
            .expect("caller unit")
            .exact_safe
    };
    let decorated = "def double(f):\n    return lambda x: f(x) * 2\n\n@double\ndef helper(x):\n    return x * 5 + 1\n\ndef caller(a):\n    return helper(a) * 10 + a\n";
    let plain =
        "def helper(x):\n    return x * 5 + 1\n\ndef caller(a):\n    return helper(a) * 10 + a\n";
    assert!(
        !caller_exact_safe(decorated),
        "a caller of a DECORATED helper must not be exact-safe (no inline, fail closed)",
    );
    assert!(
        caller_exact_safe(plain),
        "a caller of a PLAIN helper still inlines and stays exact-safe (no recall loss)",
    );
}

#[test]
fn reinvented_helper_excludes_caller_via_inlined_span_coevo_s6_s2() {
    // coevo series 6, S3-2: a pure caller of a function that inline-reinvents the helper
    // must NOT itself be reported (the called-helper record is one call level deep). The
    // span-containment gate rejects it: the matched anchor's real span lies outside the
    // caller's own line range (it belongs to the inlined callee).
    let i = Interner::new();
    let opts = DetectOptions {
        min_lines: 1,
        min_tokens: 1,
        ..Default::default()
    };
    let helper = "function big(x, y) {\n    return ((x * 2 + 3) * (x - 4)) / ((x + 5) * (y - 7) + (y * y + 11))\n}\n";
    // `reinventor` reimplements `big` inline; `passThrough` merely CALLS reinventor.
    let src = "function reinventor(x, y) {\n    return (((x * 2 + 3) * (x - 4)) / ((x + 5) * (y - 7) + (y * y + 11))) + 1\n}\n\nfunction passThrough(x, y) {\n    return reinventor(x, y) + 2\n}\n";
    let il0 =
        nose_frontend::lower_source(FileId(0), "h.js", helper.as_bytes(), Lang::JavaScript, &i)
            .unwrap();
    let il1 = nose_frontend::lower_source(FileId(1), "r.js", src.as_bytes(), Lang::JavaScript, &i)
        .unwrap();
    let mut units = nose_detect::units_of_file(&il0, &i, &opts);
    units.extend(nose_detect::units_of_file(&il1, &i, &opts));
    let findings = nose_detect::reinvented_helpers(&units);
    assert!(
        findings
            .iter()
            .all(|f| f.container_name.as_deref() != Some("passThrough")),
        "a caller of an inline-reinventor must not be reported as reinventing the helper, got {:?}",
        findings
            .iter()
            .map(|f| f.container_name.clone())
            .collect::<Vec<_>>(),
    );
}

#[test]
fn reinvented_helper_rejects_bound_blind_fold_coevo_s6_s3() {
    // coevo series 6, S3-3: an indexed `while i < n` fold absorbs the bound into a
    // pointer-length contract, dropping it from the value hash, so a fold over a
    // DIFFERENT bound must NOT be reported as containment (it computes a different value).
    let i = Interner::new();
    let opts = DetectOptions {
        min_lines: 1,
        min_tokens: 1,
        ..Default::default()
    };
    let helper = "def poly_sum(xs, n):\n    total = 0\n    i = 0\n    while i < n:\n        total = total + xs[i] * xs[i] + 3 * xs[i] + 7\n        i = i + 1\n    return total\n";
    let container = "def poly_partial(xs, n, k):\n    total = 0\n    i = 0\n    while i < n - 1:\n        total = total + xs[i] * xs[i] + 3 * xs[i] + 7\n        i = i + 1\n    return total * k + 9\n";
    let il0 = nose_frontend::lower_source(FileId(0), "a.py", helper.as_bytes(), Lang::Python, &i)
        .unwrap();
    let il1 =
        nose_frontend::lower_source(FileId(1), "b.py", container.as_bytes(), Lang::Python, &i)
            .unwrap();
    let mut units = nose_detect::units_of_file(&il0, &i, &opts);
    units.extend(nose_detect::units_of_file(&il1, &i, &opts));
    assert!(
        nose_detect::reinvented_helpers(&units).is_empty(),
        "a fold whose bound the value hash drops must not match a different-bound fold",
    );
}

#[test]
fn keyword_argument_mapping_is_by_name_not_position_issue_301() {
    // #301 (coevo series 6, S1): Python keyword arguments lower to a KwArg node carrying
    // the name, so a call's argument identity is BY NAME. Two callers passing the SAME
    // (name -> value) mapping in different orders converge; two passing DIFFERENT
    // mappings do not — the inline binds `helper(b=p, a=q)` to the right parameters.
    let i = Interner::new();
    let helper = "def helper(a, b):\n    base = a - b\n    return base * 3 + a\n";
    let call_ab = "def run(p, q):\n    return helper(a=p, b=q)\n";
    let call_ba_same = "def run(p, q):\n    return helper(b=q, a=p)\n"; // same mapping, reordered
    let call_ba_diff = "def run(p, q):\n    return helper(b=p, a=q)\n"; // different mapping
    let fp = |c: &str| value_fp_named(&i, &format!("{helper}\n{c}"), Lang::Python, "run");
    assert_eq!(
        fp(call_ab),
        fp(call_ba_same),
        "same keyword->value mapping in different order must converge",
    );
    assert_ne!(
        fp(call_ab),
        fp(call_ba_diff),
        "different keyword->value mapping must NOT converge (the #301 false merge)",
    );
}

#[test]
fn keyword_argument_oracle_binds_by_name_issue_301() {
    // The verify oracle must bind keyword args by name too, so it neither mis-binds a
    // reordered keyword call (a silent false merge) nor needlessly excludes it. The
    // differently-mapped callers compute different values on the same inputs.
    use nose_normalize::{run_unit, Value};
    let i = Interner::new();
    let src_diff = "def helper(a, b):\n    return (a - b) * 3 + a\n\ndef run(p, q):\n    return helper(b=p, a=q)\n";
    let il = nose_frontend::lower_source(FileId(0), "t.py", src_diff.as_bytes(), Lang::Python, &i)
        .unwrap();
    let n = normalize(&il, &i, &NormalizeOptions::default());
    let run = n
        .units
        .iter()
        .find(|u| u.name.is_some_and(|s| i.resolve(s) == "run"))
        .map(|u| u.root)
        .expect("run unit");
    // run(p=1, q=2) calls helper(b=1, a=2) → (a-b)*3+a = (2-1)*3+2 = 5.
    assert_eq!(
        run_unit(&n, &i, run, &[Value::Int(1), Value::Int(2)])
            .expect("interpretable")
            .ret,
        Value::Int(5),
        "the oracle must bind helper(b=p, a=q) by name: a=q=2, b=p=1",
    );
}

#[test]
fn global_reassigned_function_fails_closed_issue_302() {
    // #302: a module function reassigned via `global name; name = ...` from inside another
    // function no longer binds its `def` body, so its callers must NOT inline that body
    // (they would false-merge across files that reassign it differently). A LOCAL
    // assignment to the same name (no `global`) is a different binding and must NOT gate
    // the function — the precise distinction the series-6 reassigned-anywhere predicate
    // could not draw (it over-fired). Measured via exact-safety: an inlined caller is
    // exact-safe; an opaque (un-inlined) one is not.
    let i = Interner::new();
    let opts = DetectOptions {
        min_lines: 1,
        min_tokens: 1,
        ..Default::default()
    };
    let caller_exact_safe = |src: &str| -> bool {
        let il = nose_frontend::lower_source(FileId(0), "t.py", src.as_bytes(), Lang::Python, &i)
            .unwrap();
        nose_detect::units_of_file(&il, &i, &opts)
            .iter()
            .find(|u| u.name.as_deref() == Some("caller"))
            .expect("caller unit")
            .exact_safe
    };
    let reassigned = "def helper(x):\n    return x * 5 + 1\n\ndef setup():\n    global helper\n    helper = other\n\ndef caller(a):\n    return helper(a) * 10 + a\n";
    let local_shadow = "def helper(x):\n    return x * 5 + 1\n\ndef elsewhere():\n    helper = 5\n    return helper + 1\n\ndef caller(a):\n    return helper(a) * 10 + a\n";
    assert!(
        !caller_exact_safe(reassigned),
        "a caller of a `global`-reassigned function must not be exact-safe (fail closed)",
    );
    assert!(
        caller_exact_safe(local_shadow),
        "a local `helper = 5` (no `global`) must NOT gate the function — caller still inlines",
    );
}

#[test]
fn splat_argument_is_distinct_from_plain_argument_coevo_s7_s1() {
    // coevo series 7, S1: `f(*args)` unpacks an iterable into positional params and
    // `f(**d)` unpacks a mapping into keywords — neither is `f(arg)`. The frontend used
    // to strip the splat, so `stats(*xs)` lowered identically to `stats(xs)` and the two
    // false-merged (different on `[[1,2,3]]`: len 3 vs 1). A `Splat` node now keeps them
    // distinct.
    let i = Interner::new();
    let helper = "def stats(a):\n    total = len(a)\n    return total * total + total\n";
    let via_splat = "def via(xs):\n    return stats(*xs)\n";
    let via_plain = "def via(xs):\n    return stats(xs)\n";
    let fp = |c: &str| value_fp_named(&i, &format!("{helper}\n{c}"), Lang::Python, "via");
    assert_ne!(
        fp(via_splat),
        fp(via_plain),
        "a `*args` spread must not fingerprint as a plain positional argument",
    );
    let via_kwsplat = "def via(d):\n    return stats(**d)\n";
    assert_ne!(
        fp(via_kwsplat),
        fp(via_plain),
        "a `**kwargs` spread must not fingerprint as a plain positional argument",
    );
    assert_ne!(
        fp(via_splat),
        fp(via_kwsplat),
        "`*args` and `**kwargs` spreads must stay distinct from each other",
    );
}

#[test]
fn global_rebind_recorded_for_all_assignment_forms_coevo_s7_s2() {
    // coevo series 7, S2: the #302 fix recorded `ModuleRebind` only for a plain
    // single-identifier `global helper; helper = x`. Tuple-unpack, aug-assign, and walrus
    // all lower to an `Assign` too and must also withhold the rebound function. Measured
    // via exact-safety: a caller of a rebound function is opaque (not exact-safe).
    let i = Interner::new();
    let opts = DetectOptions {
        min_lines: 1,
        min_tokens: 1,
        ..Default::default()
    };
    let caller_exact_safe = |src: &str| -> bool {
        let il = nose_frontend::lower_source(FileId(0), "t.py", src.as_bytes(), Lang::Python, &i)
            .unwrap();
        nose_detect::units_of_file(&il, &i, &opts)
            .iter()
            .find(|u| u.name.as_deref() == Some("caller"))
            .expect("caller unit")
            .exact_safe
    };
    let helper = "def helper(x):\n    return x * 5 + 1\n";
    let caller = "def caller(a):\n    return helper(a) * 10 + a\n";
    let tuple = format!(
        "{helper}\ndef setup():\n    global helper\n    helper, spare = other, 0\n\n{caller}"
    );
    let aug = format!("{helper}\ndef setup():\n    global helper\n    helper += 1\n\n{caller}");
    let walrus =
        format!("{helper}\ndef setup():\n    global helper\n    (helper := other)\n\n{caller}");
    for (label, src) in [
        ("tuple-unpack", &tuple),
        ("aug-assign", &aug),
        ("walrus", &walrus),
    ] {
        assert!(
            !caller_exact_safe(src),
            "a caller of a `global`-rebound function ({label}) must not be exact-safe",
        );
    }
    // Precise: a LOCAL `helper = 5` (no `global`) still leaves the function inlinable.
    let local =
        format!("{helper}\ndef elsewhere():\n    helper = 5\n    return helper + 1\n\n{caller}");
    assert!(
        caller_exact_safe(&local),
        "a local shadow (no `global`) must NOT withhold the function",
    );
}

#[test]
fn effectful_keyword_reorder_stays_distinct_coevo_s7_s3() {
    // coevo series 7, S3: the keyword name-sort (#304) is sound only for effect-free
    // values — Python evaluates args in SOURCE order, so reordering effectful keyword
    // values changes the effect/exception order. With a call-valued keyword the two
    // orderings must stay distinct; with pure values they still converge.
    let i = Interner::new();
    let eff_a = "def use(x, y):\n    return combine(a=sideA(x), b=sideB(y))\n";
    let eff_b = "def use(x, y):\n    return combine(b=sideB(y), a=sideA(x))\n";
    assert_ne!(
        value_fp_named(&i, eff_a, Lang::Python, "use"),
        value_fp_named(&i, eff_b, Lang::Python, "use"),
        "reordered EFFECTFUL keyword values must not converge (eval order differs)",
    );
    let pure_a = "def use(p, q):\n    return combine(a=p, b=q)\n";
    let pure_b = "def use(p, q):\n    return combine(b=q, a=p)\n";
    assert_eq!(
        value_fp_named(&i, pure_a, Lang::Python, "use"),
        value_fp_named(&i, pure_b, Lang::Python, "use"),
        "reordered PURE keyword values still converge (no observable order)",
    );
}

#[test]
fn string_literal_plus_does_not_commute_issue_308() {
    // #308: a string literal's value-graph `Const` key must stay inside the `String`
    // class range so `proven_non_concat` classifies it correctly. The old
    // `0x2000_0000.wrapping_add(hash)` carried a high-bit hash OUT of range, where the
    // string read as non-concat and `"p" + "q"` wrongly commuted with `"q" + "p"`
    // (different values "pq" vs "qp"). The masked key keeps strings in range.
    let i = Interner::new();
    let pq = "def f():\n    return \"p\" + \"q\"\n";
    let qp = "def f():\n    return \"q\" + \"p\"\n";
    assert_ne!(
        value_fp(&i, pq, Lang::Python),
        value_fp(&i, qp, Lang::Python),
        "string concatenation is ordered — `\"p\"+\"q\"` must not merge with `\"q\"+\"p\"`",
    );
    // The masked key still discriminates distinct strings (no class collision).
    let pr = "def f():\n    return \"p\" + \"r\"\n";
    assert_ne!(
        value_fp(&i, pq, Lang::Python),
        value_fp(&i, pr, Lang::Python),
        "distinct string literals must stay distinct under the masked key",
    );
    // And numeric `+` still commutes (the fix is string-specific, recall preserved).
    let ab = "def f(a, b):\n    return a + b + 1\n";
    let ba = "def f(a, b):\n    return b + a + 1\n";
    assert_eq!(
        value_fp(&i, ab, Lang::Python),
        value_fp(&i, ba, Lang::Python),
        "numeric `+` still commutes — the string fix must not regress it",
    );
}

#[test]
fn reinvented_helper_flags_test_container_for_default_exclusion() {
    // The promotion field audit (2026-06-13) excludes test-container findings from the
    // bare-default surface (a test asserting the helper's value as a literal is circular
    // to "fix"). The `container_in_test` flag drives that — set when the container file
    // is a test path, regardless of the helper's location.
    let i = Interner::new();
    let opts = DetectOptions {
        min_lines: 1,
        min_tokens: 1,
        ..Default::default()
    };
    let helper = "function big(x, y) {\n    return ((x * 2 + 3) * (x - 4)) / ((x + 5) * (y - 7) + (y * y + 11))\n}\n";
    let reinventor = "function manual(x, y) {\n    return (((x * 2 + 3) * (x - 4)) / ((x + 5) * (y - 7) + (y * y + 11))) * 7\n}\n";
    let run = |container_path: &str| -> Option<bool> {
        let il0 = nose_frontend::lower_source(
            FileId(0),
            "helper.js",
            helper.as_bytes(),
            Lang::JavaScript,
            &i,
        )
        .unwrap();
        let il1 = nose_frontend::lower_source(
            FileId(1),
            container_path,
            reinventor.as_bytes(),
            Lang::JavaScript,
            &i,
        )
        .unwrap();
        let mut units = nose_detect::units_of_file(&il0, &i, &opts);
        units.extend(nose_detect::units_of_file(&il1, &i, &opts));
        nose_detect::reinvented_helpers(&units)
            .first()
            .map(|f| f.container_in_test)
    };
    assert_eq!(
        run("src/math.js"),
        Some(false),
        "a prod-path container is not flagged in_test"
    );
    assert_eq!(
        run("test/math.test.js"),
        Some(true),
        "a test-path container is flagged in_test"
    );
}

#[test]
fn literal_const_kind_is_separate_from_value_coevo_s8() {
    // coevo series 8: the value-graph `Const` carries its KIND explicitly and the FULL
    // value/hash in `bits`, so a literal can never wrap its class boundary or truncate.
    // Three false merges the old packed u32 key produced are gone:
    let i = Interner::new();
    // S1-1 — an int whose old key collided with the boolean-true tag.
    let int_big = "def f(x):\n    return x + 536870914\n";
    let bool_true = "def f(x):\n    return x + True\n";
    assert_ne!(
        value_fp(&i, int_big, Lang::Python),
        value_fp(&i, bool_true, Lang::Python),
        "an int literal must not share a fingerprint with the boolean `True`",
    );
    // S1-2 — two ints differing by exactly 2^32 (old `v as u32` truncation collided).
    let int_a = "def f(x):\n    return x + 4294967301\n";
    let int_b = "def f(x):\n    return x + 5\n";
    assert_ne!(
        value_fp(&i, int_a, Lang::Python),
        value_fp(&i, int_b, Lang::Python),
        "ints differing by 2^32 must not collide (full i64 retained)",
    );
    // S2-1 — two short strings whose hashes collide in the old 28-bit mask.
    let s_geu = "def f():\n    return \"geU\"\n";
    let s_aaha = "def f():\n    return \"aaha\"\n";
    assert_ne!(
        value_fp(&i, s_geu, Lang::Python),
        value_fp(&i, s_aaha, Lang::Python),
        "distinct strings must not collide (full 64-bit hash retained)",
    );
    // Recall preserved: equal literals still converge, numeric `+` still commutes.
    assert_eq!(
        value_fp(&i, int_b, Lang::Python),
        value_fp(&i, "def f(x):\n    return x + 5\n", Lang::Python),
        "equal int literals still converge",
    );
    assert_eq!(
        value_fp(&i, "def f(a, b):\n    return a + b + 1\n", Lang::Python),
        value_fp(&i, "def f(a, b):\n    return b + a + 1\n", Lang::Python),
        "numeric `+` still commutes",
    );
}
