use super::*;

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
    assert_ne!(
        value_fp(&i, typed_left, Lang::TypeScript),
        value_fp(&i, typed_grouped, Lang::TypeScript),
        "TypeScript number is IEEE-754 and must preserve source association"
    );

    let derived_left = "function f(a:number,b:number,c:number,d:number,e:number,f:number): number { return (a*b + c*d) + e*f; }";
    let derived_right = "function g(a:number,b:number,c:number,d:number,e:number,f:number): number { return a*b + (c*d + e*f); }";
    assert_ne!(
        value_fp(&i, derived_left, Lang::TypeScript),
        value_fp(&i, derived_right, Lang::TypeScript),
        "float possibility must propagate through derived Number expressions"
    );

    let bound_left = "function f(a:number,b:number,c:number,d:number,e:number,f:number): number { const x = a*b; return ((x + c*d) + e*f) + x; }";
    let bound_right = "function g(a:number,b:number,c:number,d:number,e:number,f:number): number { const x = a*b; return (x + (c*d + e*f)) + x; }";
    assert_ne!(
        value_fp(&i, bound_left, Lang::TypeScript),
        value_fp(&i, bound_right, Lang::TypeScript),
        "float possibility must survive a multi-use local binding"
    );

    let literal_left =
        "function f(): number { return (100000000*100000000 + -100000000*100000000) + 1; }";
    let literal_right =
        "function g(): number { return 100000000*100000000 + (-100000000*100000000 + 1); }";
    assert_ne!(
        value_fp(&i, literal_left, Lang::TypeScript),
        value_fp(&i, literal_right, Lang::TypeScript),
        "JavaScript integer literals still produce non-associative IEEE-754 Number arithmetic"
    );

    let bitwise_left = "function f(): number { return ((3|0)*(3|0))*4503599627370495; }";
    let bitwise_right = "function g(): number { return (3|0)*((3|0)*4503599627370495); }";
    assert_ne!(
        value_fp(&i, bitwise_left, Lang::TypeScript),
        value_fp(&i, bitwise_right, Lang::TypeScript),
        "int32 results still enter non-associative JavaScript Number arithmetic"
    );

    let call_left = "declare function a(): number; declare function b(): number; declare function c(): number; function f(): number { return (a()*b())*c(); }";
    let call_right = "declare function a(): number; declare function b(): number; declare function c(): number; function g(): number { return a()*(b()*c()); }";
    assert_ne!(
        value_fp(&i, call_left, Lang::TypeScript),
        value_fp(&i, call_right, Lang::TypeScript),
        "opaque number-returning calls must not license JavaScript multiplication reassociation"
    );

    let overflow = "function f(): number { return 4611686018427387904 * 4; }";
    let zero = "function g(): number { return 0 * 4; }";
    assert_ne!(
        value_fp(&i, overflow, Lang::TypeScript),
        value_fp(&i, zero, Lang::TypeScript),
        "JavaScript Number constant folding must not use wrapping i64 multiplication"
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
