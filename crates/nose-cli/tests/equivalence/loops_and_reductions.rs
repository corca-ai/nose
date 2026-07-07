use super::*;

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
fn python_all_demorgan_converges_with_early_return_loop() {
    let i = Interner::new();
    let positive = "\
def all_not_zero_or_one(xs):
    return all(x != 0 and x != 1 for x in xs)

def loop_no_zero_or_one(xs):
    for x in xs:
        if x == 0 or x == 1:
            return False
    return True
";
    let all_fp = value_fp_named(&i, positive, Lang::Python, "all_not_zero_or_one");
    assert_eq!(
        all_fp,
        value_fp_named(&i, positive, Lang::Python, "loop_no_zero_or_one"),
        "all(x != 0 and x != 1) should converge with the De Morgan counterexample loop"
    );

    let changed_predicate = "\
def all_not_zero_or_one(xs):
    return all(x != 0 and x != 1 for x in xs)

def all_changed_predicate(xs):
    return all(x != 0 or x != 1 for x in xs)
";
    assert_ne!(
        value_fp_named(&i, changed_predicate, Lang::Python, "all_not_zero_or_one"),
        value_fp_named(&i, changed_predicate, Lang::Python, "all_changed_predicate"),
        "changing the predicate connective must stay distinct"
    );

    let value_returning = "\
def boolean_demorgan_predicate(x):
    return x != 0 and x != 1

def value_returning_operand(x):
    return x != 0 and marker(x)

def marker(x):
    return x
";
    assert_ne!(
        value_fp_named(
            &i,
            value_returning,
            Lang::Python,
            "boolean_demorgan_predicate"
        ),
        value_fp_named(&i, value_returning, Lang::Python, "value_returning_operand"),
        "boolean De Morgan must not admit value-returning and/or operands"
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
