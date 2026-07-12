use super::*;

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
    let ts = "function f(xs: number[]): number[] { return xs.map((x: number) => x * 2); }";
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
