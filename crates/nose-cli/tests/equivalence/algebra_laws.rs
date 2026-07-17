use super::*;

#[test]
fn algebra_associativity() {
    let i = Interner::new();
    // ASSOCIATIVITY (`(a+b)+c` ≡ `a+(b+c)`) over UNTYPED params is now HELD: an untyped param
    // could be a float at runtime, and float `+` is NON-associative (#342), so the two groupings
    // no longer converge. COMMUTATIVITY (`(a+b)+c` ≡ `c+(a+b)`) was already gated on non-concat
    // (#283-C) and stays distinct. An INT-ANNOTATED chain is proven integer, so it still fully
    // associates AND commutes. (Reconciled by the value graph — check the fingerprint.)
    let left = "def f(a, b, c):\n    return (a + b) + c\n";
    let right = "def g(a, b, c):\n    return a + (b + c)\n";
    let mixed = "def h(a, b, c):\n    return c + (a + b)\n";
    let hl = value_fp(&i, left, Lang::Python);
    assert_ne!(
        hl,
        value_fp(&i, right, Lang::Python),
        "untyped associativity is now held (operands could be float)"
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
        t("def g(a: int, b: int, c: int):\n    return a + (b + c)\n"),
        "int-annotated + still associates (proven integer)"
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
    // float associativity is also held now (next test), including the fully-untyped `(a+b)+c`
    // case via the `Value::Float` kind (#342).
    let i = Interner::new();
    let t = |src: &str| value_fp(&i, src, Lang::Python);
    // Float literal: the two groupings compute different floats and must NOT merge.
    assert_ne!(
        t("def f(x):\n    return (1e100 + x) - 1e100\n"),
        t("def g(x):\n    return (1e100 - 1e100) + x\n"),
        "float-literal subtraction must not reassociate across groupings"
    );
    // Control — an INT-TYPED `x` reassociates and converges. (An untyped `x` is now held — it
    // could be a float, #342 — so the control annotates `: int` to stay an integer chain.)
    assert_eq!(
        t("def f(x: int):\n    return (5 + x) - 5\n"),
        t("def g(x: int):\n    return (5 - 5) + x\n"),
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
    // Control — INT-TYPED reassociation is sound and still converges. (Untyped `a`/`b` are now
    // HELD too — an untyped param could be a float at runtime, #342 — so the control annotates
    // `: int` to stay an integer chain.)
    assert_eq!(
        t("def p(a: int, b: int):\n    return (2 + a) + b\n"),
        t("def q(a: int, b: int):\n    return 2 + (a + b)\n"),
        "int-typed `+` still reassociates (sound)"
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
    // flatten, in the general AND the string-coercion `+` path for Java/TS). INT-typed chains
    // keep flattening (sound: integer `+` IS associative); fully-untyped chains are now ALSO
    // held — an untyped param could be float — via the `Value::Float` kind (#342).
    let i = Interner::new();

    // Python `: float`
    assert_ne!(
        value_fp(
            &i,
            "def f(a: float, b: float, c: float):\n    return (a + b) + c\n",
            Lang::Python
        ),
        value_fp(
            &i,
            "def g(a: float, b: float, c: float):\n    return a + (b + c)\n",
            Lang::Python
        ),
        "python float-typed `+` must not reassociate"
    );
    // Rust `f64`
    assert_ne!(
        value_fp(
            &i,
            "fn f(a: f64, b: f64, c: f64) -> f64 { (a + b) + c }",
            Lang::Rust
        ),
        value_fp(
            &i,
            "fn g(a: f64, b: f64, c: f64) -> f64 { a + (b + c) }",
            Lang::Rust
        ),
        "rust f64 `+` must not reassociate"
    );
    // Java `double` — the value graph routes Java `+` through the string-coercion path, which
    // must honor the float hold too (the regression this test guards).
    assert_ne!(
        value_fp(
            &i,
            "class C { static double f(double a, double b, double c) { return (a + b) + c; } }",
            Lang::Java
        ),
        value_fp(
            &i,
            "class C { static double g(double a, double b, double c) { return a + (b + c); } }",
            Lang::Java
        ),
        "java double `+` must not reassociate"
    );
    // Go `float64`
    assert_ne!(
        value_fp(
            &i,
            "package m\nfunc f(a float64, b float64, c float64) float64 { return (a + b) + c }",
            Lang::Go
        ),
        value_fp(
            &i,
            "package m\nfunc g(a float64, b float64, c float64) float64 { return a + (b + c) }",
            Lang::Go
        ),
        "go float64 `+` must not reassociate"
    );

    // Control — INT-typed params still reassociate (sound): split-only on float evidence.
    assert_eq!(
        value_fp(
            &i,
            "def p(a: int, b: int, c: int):\n    return (a + b) + c\n",
            Lang::Python
        ),
        value_fp(
            &i,
            "def q(a: int, b: int, c: int):\n    return a + (b + c)\n",
            Lang::Python
        ),
        "python int-typed `+` still reassociates"
    );
    assert_eq!(
        value_fp(
            &i,
            "class C { static int p(int a, int b, int c) { return (a + b) + c; } }",
            Lang::Java
        ),
        value_fp(
            &i,
            "class C { static int q(int a, int b, int c) { return a + (b + c); } }",
            Lang::Java
        ),
        "java int `+` still reassociates"
    );
    // Fully-untyped params are now ALSO held — an untyped param could be a float at runtime,
    // and the oracle witnesses the non-associativity via a float battery (#342, the Value::Float
    // kind, oracle-value-model §3.3). Commutativity (same grouping) and `: int` chains still
    // converge (see `algebra_associativity`).
    assert_ne!(
        value_fp(
            &i,
            "def p(a, b, c):\n    return (a + b) + c\n",
            Lang::Python
        ),
        value_fp(
            &i,
            "def q(a, b, c):\n    return a + (b + c)\n",
            Lang::Python
        ),
        "untyped `+` is now held (possibly float)"
    );
}

#[test]
fn large_float_possible_polynomial_reuses_subtree_evaluations() {
    // #858 preserves the source grouping of possibly-float `+`/`*` trees. Rebuilding a large
    // alternating polynomial must reuse the values already computed for each direct child;
    // recursively interpreting every child again made SymPy's `unrad` unit take 17 seconds.
    let polynomial = (1..=32)
        .map(|power| format!("a**{power} * b**{}", 33 - power))
        .collect::<Vec<_>>()
        .join(" + ");
    let source = format!("def f(a, b):\n    return {polynomial}\n");
    let renamed = format!(
        "def g(x, y):\n    return {}\n",
        polynomial.replace('a', "x").replace('b', "y")
    );
    let interner = Interner::new();
    let fingerprint = value_fp(&interner, &source, Lang::Python);
    assert!(!fingerprint.is_empty());
    assert_eq!(fingerprint, value_fp(&interner, &renamed, Lang::Python));
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
