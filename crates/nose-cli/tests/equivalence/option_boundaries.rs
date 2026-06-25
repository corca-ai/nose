use super::*;

#[test]
fn rust_constructor_pattern_variant_test_stays_distinct() {
    let i = Interner::new();
    // #390: a binding constructor pattern's variant test lowers to the constructor PATH (the
    // discriminant), not the whole pattern as an opaque Raw node. The discriminant must still
    // discriminate — matching `Some(_)` vs `Ok(_)` are different variants and stay distinct, even
    // now that binding extraction makes the *bodies* converge (see
    // `rust_constructor_pattern_binding_extraction_converges`): the arm conditions still differ.
    let some = "pub fn f(x: Option<i32>) -> i32 { match x { Some(a) => a + 1, None => 0 } }\n";
    let ok = "pub fn f(x: Result<i32, i32>) -> i32 { match x { Ok(a) => a + 1, Err(_) => 0 } }\n";
    assert_ne!(
        value_fp(&i, some, Lang::Rust),
        value_fp(&i, ok, Lang::Rust),
        "Some(_) and Ok(_) are different variants — must stay distinct"
    );
}

#[test]
fn rust_constructor_pattern_binding_extraction_converges() {
    // #390 follow-up: a match arm projects its payload binding (`Some(v)` → `v = x.0`) ahead of
    // the body so the body's uses of it alpha-canonicalize. Two copies that differ ONLY in the
    // bound name now converge — closing the split the #390 lowering left open.
    let i = Interner::new();
    let some_a =
        "pub fn f(x: Option<i32>) -> i32 { match x { Some(a) => a * 2 + 1, None => 0 } }\n";
    let some_b =
        "pub fn g(x: Option<i32>) -> i32 { match x { Some(b) => b * 2 + 1, None => 0 } }\n";
    assert_eq!(
        value_fp(&i, some_a, Lang::Rust),
        value_fp(&i, some_b, Lang::Rust),
        "`Some(a) => a*2+1` and `Some(b) => b*2+1` differ only in the bound name — must converge"
    );
    // The body still gates: a different arm computation must NOT merge (no false merge).
    let some_c =
        "pub fn h(x: Option<i32>) -> i32 { match x { Some(c) => c * 3 + 1, None => 0 } }\n";
    assert_ne!(
        value_fp(&i, some_a, Lang::Rust),
        value_fp(&i, some_c, Lang::Rust),
        "different arithmetic in the arm body must stay distinct"
    );
    // Cross-variant stays distinct even though both bodies are now `v = x.0; …` (the arm
    // *condition* — `x == Some` vs `x == Ok` — keeps them apart).
    let ok_a =
        "pub fn k(x: Result<i32, i32>) -> i32 { match x { Ok(a) => a * 2 + 1, Err(_) => 0 } }\n";
    assert_ne!(
        value_fp(&i, some_a, Lang::Rust),
        value_fp(&i, ok_a, Lang::Rust),
        "Some and Ok are different variants — must stay distinct after binding extraction"
    );
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
fn rust_if_let_result_channels_converge_with_result_predicates() {
    let i = Interner::new();
    let if_ok = "pub fn f(value: Result<i32, i32>) -> bool {\n    if let Ok(_) = value { true } else { false }\n}\n";
    let is_ok = "pub fn g(value: Result<i32, i32>) -> bool {\n    value.is_ok()\n}\n";
    let if_err = "pub fn h(value: Result<i32, i32>) -> bool {\n    if let Err(_) = value { true } else { false }\n}\n";
    let is_err = "pub fn i(value: Result<i32, i32>) -> bool {\n    value.is_err()\n}\n";
    let shadowed_ok = "struct Ok<T>(T);\npub fn f(value: Ok<i32>) -> bool {\n    if let Ok(_) = value { true } else { false }\n}\n";
    let shadowed_result_is_ok = "struct Result<T, E> { value: T, err: E }\nimpl<T, E> Result<T, E> { fn is_ok(&self) -> bool { false } }\npub fn f(value: Result<i32, i32>) -> bool {\n    value.is_ok()\n}\n";
    let result_unwrap_else = "pub fn f(value: Result<i32, i32>, fallback: i32) -> i32 {\n    value.unwrap_or_else(|_| fallback)\n}\n";
    let result_fallback =
        "pub fn g(value: Result<i32, i32>, fallback: i32) -> i32 {\n    fallback\n}\n";

    assert_eq!(
        value_fp(&i, if_ok, Lang::Rust),
        value_fp(&i, is_ok, Lang::Rust),
        "if let Ok(_) should converge with is_ok()"
    );
    assert_eq!(
        value_fp(&i, if_err, Lang::Rust),
        value_fp(&i, is_err, Lang::Rust),
        "if let Err(_) should converge with is_err()"
    );
    assert_ne!(
        value_fp(&i, if_ok, Lang::Rust),
        value_fp(&i, if_err, Lang::Rust),
        "Ok and Err channels must stay distinct"
    );
    assert_ne!(
        value_fp(&i, if_ok, Lang::Rust),
        value_fp_named(&i, shadowed_ok, Lang::Rust, "f"),
        "a local Rust Ok pattern must not be treated as Result::Ok"
    );
    assert_ne!(
        value_fp(&i, is_ok, Lang::Rust),
        value_fp_named(&i, shadowed_result_is_ok, Lang::Rust, "f"),
        "a local Rust Result receiver must not be treated as std Result::is_ok"
    );
    assert_ne!(
        value_fp(&i, result_unwrap_else, Lang::Rust),
        value_fp(&i, result_fallback, Lang::Rust),
        "Result callback/defaulting APIs are not admitted by the narrow predicate slice"
    );
}
