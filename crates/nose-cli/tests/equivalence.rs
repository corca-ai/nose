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
fn fstring_multi_interpolation_chains() {
    // Two interpolations fold into two Adds — same shape as the JS template.
    let i = Interner::new();
    let py = "def f(a, b):\n    return f\"{a} and {b}\"\n";
    let js = "function g(a, b){\n  return `${a} and ${b}`;\n}\n";
    assert_eq!(
        unit_hash(&i, py, Lang::Python),
        unit_hash(&i, js, Lang::JavaScript),
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
    // free names a positional cid by occurrence, so `foo(x)`/`bar(x)` and
    // `max(a,b)`/`min(a,b)` (2-arg, not canonical builtins) became identical IL.
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
