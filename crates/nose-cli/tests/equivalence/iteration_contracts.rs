use super::*;

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
    let zipc = "from typing import List\ndef d(a: List[int], b: List[int]):\n    return sum(x * y for x, y in zip(a, b))\n";
    let loopv =
        "from typing import List\ndef d(a: List[int], b: List[int]):\n    s = 0\n    for i in range(len(a)):\n        s = s + a[i] * b[i]\n    return s\n";
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
        "from typing import List\ndef d(a: List[int], b: List[int]):\n    s = 0\n    for i in range(len(a)):\n        s += a[i] * b[i]\n    return s\n";
    let py_zip = "from typing import List\ndef d(a: List[int], b: List[int]):\n    return sum(x * y for x, y in zip(a, b))\n";
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
    let bad_pair_sum = "from typing import List\ndef d(a: List[int], b: List[int]):\n    return sum(x + y for x, y in zip(a, b))\n";

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
    let enum_ = "from typing import List\ndef ff(xs: List[int], t: int):\n    for i, x in enumerate(xs):\n        if x > t:\n            return i\n    return -1\n";
    let rng = "from typing import List\ndef ff(xs: List[int], t: int):\n    for i in range(len(xs)):\n        if xs[i] > t:\n            return i\n    return -1\n";
    assert_eq!(
        value_fp(&i, enum_, Lang::Python),
        value_fp(&i, rng, Lang::Python),
        "enumerate and range-index first-match should converge"
    );
}
