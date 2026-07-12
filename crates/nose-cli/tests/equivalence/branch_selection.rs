use super::*;

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
fn filtered_constant_selection_keeps_source_and_empty_input_boundaries() {
    let i = Interner::new();
    for terminal in ["max", "min"] {
        let from_xs = format!("def f(xs, ys):\n    return {terminal}(1 for x in xs if x > 0)\n");
        let from_ys = format!("def f(xs, ys):\n    return {terminal}(1 for y in ys if y > 0)\n");
        assert_ne!(
            value_fp(&i, &from_xs, Lang::Python),
            value_fp(&i, &from_ys, Lang::Python),
            "filtered {terminal} over different sources must retain source identity and empty-input behavior"
        );
    }
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
