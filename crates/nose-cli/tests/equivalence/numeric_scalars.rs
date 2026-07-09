use super::*;

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
    let go_predeclared_min = "package p\n\nfunc f(left int, right int, other int) int { selected := min(left, right); return selected + other }\n";
    let java_min = "class C { static int f(int left, int right, int other) { int selected = Math.min(left, right); return selected + other; } }\n";
    let java_double_min = "class C { static double f(double left, double right, double other) { double selected = Math.min(left, right); return selected + other; } }\n";
    let c_min = "#include <math.h>\n\ndouble f(double left, double right, double other) { double selected = fmin(left, right); return selected + other; }\n";
    let ruby_min =
        "def f(left, right, other)\n  selected = [left, right].min\n  selected + other\nend\n";
    let rust_min = "pub fn f(left: i64, right: i64, other: i64) -> i64 { let selected = left.min(right); selected + other }\n";
    let py_max = "def f(left, right, other):\n    selected = left if left >= right else right\n    return selected + other\n";
    let ruby_max =
        "def f(left, right, other)\n  selected = [left, right].max\n  selected + other\nend\n";
    let go_predeclared_max = "package p\n\nfunc f(left int, right int, other int) int { selected := max(left, right); return selected + other }\n";
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
    assert_eq!(fp, value_fp(&i, go_predeclared_min, Lang::Go));
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
        value_fp(&i, go_predeclared_max, Lang::Go)
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
    let go_shadowed_min = "package p\n\nfunc min(_left int, _right int) int { return 0 }\n\nfunc f(left int, right int, other int) int { selected := min(left, right); return selected + other }\n";
    let go_local_shadowed_min = "package p\n\nfunc f(left int, right int, other int) int { min := func(_left int, _right int) int { return 0 }; selected := min(left, right); return selected + other }\n";
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
    assert_ne!(fp, value_fp(&i, go_shadowed_min, Lang::Go));
    assert_ne!(fp, value_fp(&i, go_local_shadowed_min, Lang::Go));
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
fn go_numeric_clamp_minmax_requires_integer_bound_proof() {
    let i = Interner::new();
    let go_minmax_guarded = "\
package p

func f(x int, lo int, hi int) int {
    if hi < lo {
        return lo
    }
    return min(max(x, lo), hi)
}
";
    let go_maxmin_guarded = "\
package p

func f(x int, lo int, hi int) int {
    if hi < lo {
        return lo
    }
    return max(min(x, hi), lo)
}
";
    let go_maxmin_unproven = "\
package p

func f(x int, lo int, hi int) int {
    return max(min(x, hi), lo)
}
";
    let go_swapped_bounds = "\
package p

func f(x int, lo int, hi int) int {
    if hi < lo {
        return lo
    }
    return min(max(x, hi), lo)
}
";
    let go_wrong_nesting = "\
package p

func f(x int, lo int, hi int) int {
    if hi < lo {
        return lo
    }
    return max(min(x, lo), hi)
}
";
    let go_float_guarded = "\
package p

func f(x float64, lo float64, hi float64) float64 {
    if hi < lo {
        return lo
    }
    return max(min(x, hi), lo)
}
";

    let guarded_fp = value_fp(&i, go_minmax_guarded, Lang::Go);
    assert_eq!(
        guarded_fp,
        value_fp(&i, go_maxmin_guarded, Lang::Go),
        "Go integer min/max clamp compositions should converge only with an exiting lo<=hi guard"
    );
    assert_ne!(
        guarded_fp,
        value_fp(&i, go_maxmin_unproven, Lang::Go),
        "Go parameter names and min/max shape alone must not prove bound order"
    );
    assert_ne!(
        guarded_fp,
        value_fp(&i, go_swapped_bounds, Lang::Go),
        "Go swapped clamp bounds must stay distinct"
    );
    assert_ne!(
        guarded_fp,
        value_fp(&i, go_wrong_nesting, Lang::Go),
        "Go wrong min/max nesting must stay distinct"
    );
    assert_ne!(
        guarded_fp,
        value_fp(&i, go_float_guarded, Lang::Go),
        "Go float min/max clamp forms remain outside the integer clamp law"
    );
}

#[test]
fn go_numeric_clamp_rejects_cmp_ordered_domains_even_when_guarded() {
    let i = Interner::new();
    let go_minmax_guarded = "\
package p

func f(x int, lo int, hi int) int {
    if hi < lo {
        return lo
    }
    return min(max(x, lo), hi)
}
";
    let go_generic_ordered = "\
package p

import \"cmp\"

func f[T cmp.Ordered](x T, lo T, hi T) T {
    return max(min(x, hi), lo)
}
";
    let go_generic_ordered_guarded = "\
package p

import \"cmp\"

func f[T cmp.Ordered](x T, lo T, hi T) T {
    if hi < lo {
        return lo
    }
    return max(min(x, hi), lo)
}
";

    let guarded_fp = value_fp(&i, go_minmax_guarded, Lang::Go);
    assert_ne!(
        guarded_fp,
        value_fp(&i, go_generic_ordered, Lang::Go),
        "Go cmp.Ordered generic helpers do not prove integer-only domain or bound order"
    );
    assert_ne!(
        guarded_fp,
        value_fp(&i, go_generic_ordered_guarded, Lang::Go),
        "Go cmp.Ordered generic helpers with bound-order proof still do not prove integer-only domain"
    );
}

#[test]
fn swift_numeric_clamp_literal_ternary_converges_with_minmax() {
    let i = Interner::new();
    let ternary = r#"
func f(_ value: Int) -> Int {
    return value < 0 ? 0 : (value > 10 ? 10 : value)
}
"#;
    let minmax = r#"
func f(_ value: Int) -> Int {
    return min(max(value, 0), 10)
}
"#;
    let wrong_bound = r#"
func f(_ value: Int) -> Int {
    return min(max(value, 0), 9)
}
"#;
    let double_ternary = r#"
func f(_ value: Double) -> Double {
    return value < 0.0 ? 0.0 : (value > 10.0 ? 10.0 : value)
}
"#;
    let double_minmax = r#"
func f(_ value: Double) -> Double {
    return min(max(value, 0.0), 10.0)
}
"#;

    let fp = value_fp(&i, ternary, Lang::Swift);
    assert_eq!(
        fp,
        value_fp(&i, minmax, Lang::Swift),
        "literal ordered Swift Int clamp ternaries should converge with min/max composition"
    );
    assert_ne!(
        fp,
        value_fp(&i, wrong_bound, Lang::Swift),
        "changing a clamp bound changes behavior"
    );
    assert_ne!(
        value_fp(&i, double_ternary, Lang::Swift),
        value_fp(&i, double_minmax, Lang::Swift),
        "Swift Double clamp forms stay closed across the NaN-sensitive boundary"
    );
}
