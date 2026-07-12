use super::*;

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
    let js =
        "function g(items: number[]): number[] {\n  return items.map((x: number) => x * 2);\n}\n";
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
