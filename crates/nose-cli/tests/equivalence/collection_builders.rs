use super::*;

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

/// Multi-clause comprehensions are flat maps, not nested maps. They should converge with
/// an equivalent nested append loop and JS `.flatMap(... .map(...))`, while staying
/// distinct from a nested list comprehension.
#[test]
fn multi_clause_comprehension_converges_as_flat_map() {
    let i = Interner::new();
    let comp = value_fp(
        &i,
        "def f(xs, ys):\n    return [x + y for x in xs for y in ys]\n",
        Lang::Python,
    );
    let loop_py = value_fp(
        &i,
        "def g(xs, ys):\n    r = []\n    for x in xs:\n        for y in ys:\n            r.append(x + y)\n    return r\n",
        Lang::Python,
    );
    let flat_map_js = value_fp(
        &i,
        "function h(xs: number[], ys: number[]): number[] { return xs.flatMap((x: number) => ys.map((y: number) => x + y)); }",
        Lang::TypeScript,
    );
    let nested_list = value_fp(
        &i,
        "def h(xs, ys):\n    return [[x + y for y in ys] for x in xs]\n",
        Lang::Python,
    );

    assert_eq!(
        comp, loop_py,
        "flat comprehension should match nested append loop"
    );
    assert_eq!(
        comp, flat_map_js,
        "flat comprehension should match JS flatMap/map"
    );
    assert_ne!(
        comp, nested_list,
        "flat-map and nested-list comprehensions differ"
    );
}

/// Aggregates over a flat-map stream consume the flattened element stream, not the outer
/// mapped collection. Keep this bridge explicit so FlatMap is not accidentally treated as
/// the filtered-Map representation (`Hof(Map, [contrib, pred])`).
#[test]
fn flat_map_sum_aggregate_converges_with_nested_reduction_loop() {
    let i = Interner::new();
    let sum_gen = value_fp(
        &i,
        "def f(xs, ys):\n    return sum(x + y for x in xs for y in ys)\n",
        Lang::Python,
    );
    let sum_loop = value_fp(
        &i,
        "def g(xs, ys):\n    total = 0\n    for x in xs:\n        for y in ys:\n            total = total + x + y\n    return total\n",
        Lang::Python,
    );
    let sum_js = value_fp(
        &i,
        "function h(xs: number[], ys: number[]): number { return xs.flatMap((x: number) => ys.map((y: number) => x + y)).reduce((a, v) => a + v, 0); }",
        Lang::TypeScript,
    );
    let wrong_seed = value_fp(
        &i,
        "def bad(xs, ys):\n    total = 1\n    for x in xs:\n        for y in ys:\n            total = total + x + y\n    return total\n",
        Lang::Python,
    );
    let nested_list = value_fp(
        &i,
        "def nested(xs, ys):\n    return sum([x + y for y in ys] for x in xs)\n",
        Lang::Python,
    );

    assert_eq!(
        sum_gen, sum_loop,
        "sum over a flat-map generator should match the nested reduction loop"
    );
    assert_eq!(
        sum_gen, sum_js,
        "sum over a flatMap/map chain should match the flattened reduction"
    );
    assert_ne!(
        sum_gen, wrong_seed,
        "changing the additive seed changes aggregate behavior"
    );
    assert_ne!(
        sum_gen, nested_list,
        "aggregating nested list rows is not aggregating the flattened stream"
    );
}

#[test]
fn flat_map_max_aggregate_keeps_loop_seed_behavior_defining() {
    let i = Interner::new();
    let max_gen = value_fp(
        &i,
        "def f(xs, ys):\n    return max(x + y for x in xs for y in ys)\n",
        Lang::Python,
    );
    let max_loop = value_fp(
        &i,
        "def g(xs, ys):\n    best = 0\n    for x in xs:\n        for y in ys:\n            v = x + y\n            if v > best:\n                best = v\n    return best\n",
        Lang::Python,
    );
    let max_loop_same_seed = value_fp(
        &i,
        "def h(left, right):\n    top = 0\n    for a in left:\n        for b in right:\n            cand = a + b\n            if cand > top:\n                top = cand\n    return top\n",
        Lang::Python,
    );

    // `max(gen)` errs on empty input and tracks all-negative maxima; a `best = 0`
    // seeded loop clamps at 0 in both cases. The seed is behavior-defining, so the
    // two must NOT merge — while equal-seeded loops still converge with each other.
    assert_ne!(
        max_gen, max_loop,
        "a zero-seeded selection loop clamps at its seed; true max(...) does not"
    );
    assert_eq!(
        max_loop, max_loop_same_seed,
        "equally-seeded nested selection loops should still converge"
    );
}

#[test]
fn flat_map_any_aggregate_converges_with_nested_early_return_loop() {
    let i = Interner::new();
    let any_gen = value_fp(
        &i,
        "def f(xs, ys):\n    return any(x + y > 0 for x in xs for y in ys)\n",
        Lang::Python,
    );
    let any_loop = value_fp(
        &i,
        "def g(xs, ys):\n    for x in xs:\n        for y in ys:\n            if x + y > 0:\n                return True\n    return False\n",
        Lang::Python,
    );
    let any_bad_predicate = value_fp(
        &i,
        "def bad(xs, ys):\n    return any(x + y < 0 for x in xs for y in ys)\n",
        Lang::Python,
    );

    assert_eq!(
        any_gen, any_loop,
        "any over a flat-map generator should match the nested early-return loop"
    );
    assert_ne!(
        any_gen, any_bad_predicate,
        "changing the flattened any predicate changes behavior"
    );
}

#[test]
fn flat_map_outer_independent_aggregate_keeps_outer_cardinality() {
    let i = Interner::new();
    let outer_independent_flat = value_fp(
        &i,
        "def f(xs, ys):\n    return sum(y for x in xs for y in ys)\n",
        Lang::Python,
    );
    let outer_independent_loop = value_fp(
        &i,
        "def g(xs, ys):\n    total = 0\n    for x in xs:\n        for y in ys:\n            total = total + y\n    return total\n",
        Lang::Python,
    );
    let direct_inner_sum = value_fp(
        &i,
        "def h(xs, ys):\n    return sum(y for y in ys)\n",
        Lang::Python,
    );

    assert_ne!(
        outer_independent_flat, direct_inner_sum,
        "a flat-map aggregate that ignores the outer value still depends on outer cardinality"
    );
    assert_ne!(
        outer_independent_loop, direct_inner_sum,
        "a nested loop that ignores the outer value still depends on outer cardinality"
    );
}

#[test]
fn filtered_flat_map_sum_converges_with_nested_guarded_reduction() {
    let i = Interner::new();
    let filtered_sum_gen = value_fp(
        &i,
        "def f(xs, ys):\n    return sum(x + y for x in xs if x > 0 for y in ys if y < 10)\n",
        Lang::Python,
    );
    let filtered_sum_loop = value_fp(
        &i,
        "def g(xs, ys):\n    total = 0\n    for x in xs:\n        if x > 0:\n            for y in ys:\n                if y < 10:\n                    total = total + x + y\n    return total\n",
        Lang::Python,
    );
    let filtered_sum_js = value_fp(
        &i,
        "function h(xs: number[], ys: number[]): number { return xs.filter(x => x > 0).flatMap(x => ys.filter(y => y < 10).map(y => x + y)).reduce((a, v) => a + v, 0); }",
        Lang::TypeScript,
    );
    let filtered_sum_outer_changed = value_fp(
        &i,
        "def bad(xs, ys):\n    return sum(x + y for x in xs if False for y in ys if y < 10)\n",
        Lang::Python,
    );
    let filtered_sum_inner_changed = value_fp(
        &i,
        "def bad(xs, ys):\n    return sum(x + y for x in xs if x > 0 for y in ys if False)\n",
        Lang::Python,
    );

    assert_eq!(
        filtered_sum_gen, filtered_sum_loop,
        "filtered flat-map sums should match equivalent nested guarded reductions"
    );
    assert_ne!(
        filtered_sum_gen, filtered_sum_js,
        "TypeScript number[] callback elements are not yet numeric proof for relational predicates"
    );
    assert_ne!(
        filtered_sum_gen, filtered_sum_outer_changed,
        "changing the outer flat-map aggregate predicate changes behavior"
    );
    assert_ne!(
        filtered_sum_gen, filtered_sum_inner_changed,
        "changing the inner flat-map aggregate predicate changes behavior"
    );
}

#[test]
fn filtered_flat_map_any_all_converge_with_nested_guarded_loops() {
    let i = Interner::new();
    let filtered_any_gen = value_fp(
        &i,
        "def f(xs, ys):\n    return any(x + y > 0 for x in xs if x > 0 for y in ys if y < 10)\n",
        Lang::Python,
    );
    let filtered_any_js = value_fp(
        &i,
        "function h(xs: number[], ys: number[]): boolean { return xs.filter(x => x > 0).flatMap(x => ys.filter(y => y < 10).map(y => x + y)).some(v => v > 0); }",
        Lang::TypeScript,
    );
    let filtered_any_loop = value_fp(
        &i,
        "def g(xs, ys):\n    for x in xs:\n        if x > 0:\n            for y in ys:\n                if y < 10 and x + y > 0:\n                    return True\n    return False\n",
        Lang::Python,
    );
    let filtered_any_terminal_changed = value_fp(
        &i,
        "function h(xs: number[], ys: number[]): boolean { return xs.filter(x => x > 0).flatMap(x => ys.filter(y => y < 10).map(y => x + y)).some(v => false); }",
        Lang::TypeScript,
    );
    let filtered_all_gen = value_fp(
        &i,
        "def f(xs, ys):\n    return all(x + y > 0 for x in xs if x > 0 for y in ys if y < 10)\n",
        Lang::Python,
    );
    let filtered_all_js = value_fp(
        &i,
        "function h(xs: number[], ys: number[]): boolean { return xs.filter(x => x > 0).flatMap(x => ys.filter(y => y < 10).map(y => x + y)).every(v => v > 0); }",
        Lang::TypeScript,
    );
    let filtered_all_loop = value_fp(
        &i,
        "def g(xs, ys):\n    for x in xs:\n        if x > 0:\n            for y in ys:\n                if y < 10 and not (x + y > 0):\n                    return False\n    return True\n",
        Lang::Python,
    );
    let filtered_all_terminal_changed = value_fp(
        &i,
        "function h(xs: number[], ys: number[]): boolean { return xs.filter(x => x > 0).flatMap(x => ys.filter(y => y < 10).map(y => x + y)).every(v => false); }",
        Lang::TypeScript,
    );

    assert_ne!(
        filtered_any_gen, filtered_any_js,
        "TypeScript number[] callback elements are not yet numeric proof for relational predicates"
    );
    assert_eq!(
        filtered_any_gen, filtered_any_loop,
        "filtered flat-map any should match the equivalent nested guarded early-return loop"
    );
    assert_ne!(
        filtered_any_gen, filtered_any_terminal_changed,
        "changing the terminal method predicate changes behavior"
    );
    assert_ne!(
        filtered_all_gen, filtered_all_js,
        "TypeScript number[] callback elements are not yet numeric proof for relational predicates"
    );
    assert_eq!(
        filtered_all_gen, filtered_all_loop,
        "filtered flat-map all should match the equivalent nested guarded early-return loop"
    );
    assert_ne!(
        filtered_all_gen, filtered_all_terminal_changed,
        "changing the terminal universal predicate changes behavior"
    );
}

/// Cross-language `any`/`all` predicate reductions converge when the predicate is proven in the
/// same primitive domain. Rust iterator predicates still converge with Python generators; TS
/// `number[]` callbacks stay closed until element-domain proof exists. `any` and `all` stay
/// DISTINCT (different short-circuit behavior).
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
        "function g(xs: number[]): boolean { return xs.some(x => x > 0); }",
        Lang::TypeScript,
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
        "function g(xs: number[]): boolean { return xs.every(x => x > 0); }",
        Lang::TypeScript,
    );
    assert_ne!(
        any_py, any_js,
        "TypeScript number[] callback elements are not yet numeric proof for relational predicates"
    );
    assert_eq!(any_py, any_rs, "Python any ≡ Rust any");
    assert_ne!(
        all_py, all_js,
        "TypeScript number[] callback elements are not yet numeric proof for relational predicates"
    );
    assert_ne!(any_py, all_py, "any and all must stay distinct");
    assert!(!any_py.is_empty());
}

#[test]
fn rust_filter_map_converges_with_filtered_map_and_guarded_builder() {
    let i = Interner::new();
    let filtered_py = value_fp(
        &i,
        "def f(xs):\n    return [x * 2 for x in xs if x > 0]\n",
        Lang::Python,
    );
    let filtered_js = value_fp(
        &i,
        "function f(xs){ return xs.filter(x => x > 0).map(x => x * 2); }",
        Lang::JavaScript,
    );
    let filtered_ts = value_fp(
        &i,
        "function f(xs: number[]): number[] { return xs.filter((x) => x > 0).map((x) => x * 2); }",
        Lang::TypeScript,
    );
    let filter_map_rs = value_fp(
        &i,
        "fn f(xs: &[i32]) -> Vec<i32> { xs.iter().copied().filter_map(|x| if x > 0 { Some(x * 2) } else { None }).collect() }",
        Lang::Rust,
    );
    let match_option_rs = value_fp(
        &i,
        "fn f(xs: &[i32]) -> Vec<i32> { xs.iter().copied().filter_map(|x| match x { _ if x > 0 => Some(x * 2), _ => None }).collect() }",
        Lang::Rust,
    );
    let and_then_rs = value_fp(
        &i,
        "fn f(xs: &[i32]) -> Vec<i32> { xs.iter().copied().filter_map(|x| Some(x).and_then(|value| if value > 0 { Some(value * 2) } else { None })).collect() }",
        Lang::Rust,
    );
    let guarded_builder_rs = value_fp(
        &i,
        "fn f(xs: &[i32]) -> Vec<i32> { let out = Vec::new(); for x in xs { if *x > 0 { out.push(*x * 2); } } out }",
        Lang::Rust,
    );
    let mapped_none_rs = value_fp(
        &i,
        "fn f(xs: &[i32]) -> Vec<Option<i32>> { xs.iter().copied().map(|x| if x > 0 { Some(x * 2) } else { None }).collect() }",
        Lang::Rust,
    );
    let changed_value_rs = value_fp(
        &i,
        "fn f(xs: &[i32]) -> Vec<i32> { xs.iter().copied().filter_map(|x| if x > 0 { Some(x * 3) } else { None }).collect() }",
        Lang::Rust,
    );
    assert_ne!(
        filtered_py, filtered_js,
        "untyped JS parameter method calls lack a receiver proof and must stay opaque"
    );
    assert_ne!(
        filtered_py, filtered_ts,
        "TypeScript number[] callback elements are not yet numeric proof for relational predicates"
    );
    assert_eq!(
        filtered_py, filter_map_rs,
        "Rust i32 slice callback elements provide total-order proof for filter_map"
    );
    assert_eq!(
        filtered_py, match_option_rs,
        "Rust filter_map match guards should use the same total-order proof"
    );
    assert_eq!(
        filtered_py, and_then_rs,
        "Rust Option::and_then filter_map chains should use the same total-order proof"
    );
    assert_eq!(
        filtered_py, guarded_builder_rs,
        "Rust guarded Vec::new/push builder should use the same filtered-map value"
    );
    assert_ne!(
        filtered_py, mapped_none_rs,
        "mapping None as a value is not the same as dropping it"
    );
    assert_ne!(
        filtered_py, changed_value_rs,
        "changing the emitted Some value must stay distinct"
    );
}

#[test]
fn rust_filter_map_keeps_falsey_and_none_payload_boundaries() {
    let i = Interner::new();
    let filtered_py = value_fp(
        &i,
        "def f(xs):\n    return [x * 2 for x in xs if x > 0]\n",
        Lang::Python,
    );
    let falsey_py = value_fp(
        &i,
        "def f(xs):\n    return [0 for x in xs if x > 0]\n",
        Lang::Python,
    );
    let filtered_none_py = value_fp(
        &i,
        "def f(xs):\n    return [None for x in xs if x > 0]\n",
        Lang::Python,
    );
    let falsey_rs = value_fp(
        &i,
        "fn f(xs: &[i32]) -> Vec<i32> { xs.iter().copied().filter_map(|x| if x > 0 { Some(0) } else { None }).collect() }",
        Lang::Rust,
    );
    let wrapped_none_rs = value_fp(
        &i,
        "fn f(xs: &[i32]) -> Vec<Option<i32>> { xs.iter().copied().filter_map(|x| if x > 0 { Some(None) } else { None }).collect() }",
        Lang::Rust,
    );
    let dropped_falsey_rs = value_fp(
        &i,
        "fn f(xs: &[i32]) -> Vec<i32> { xs.iter().copied().filter_map(|x| if x > 0 { Some(0) } else { None }).filter(|x| *x != 0).collect() }",
        Lang::Rust,
    );

    assert_eq!(
        falsey_py, falsey_rs,
        "Rust filter_map emits falsey payloads rather than treating them as absence"
    );
    assert_eq!(
        filtered_none_py, wrapped_none_rs,
        "Rust filter_map emits wrapped None payloads rather than dropping them"
    );
    assert_ne!(
        filtered_py, wrapped_none_rs,
        "emitting None payloads must stay distinct from emitting mapped values"
    );
    assert_ne!(
        falsey_rs, dropped_falsey_rs,
        "truthy filtering after emitting 0 must stay distinct"
    );
}

#[test]
fn rust_filter_map_respects_shadowed_std_name_boundaries() {
    let i = Interner::new();
    let filtered_py = value_fp(
        &i,
        "def f(xs):\n    return [x * 2 for x in xs if x > 0]\n",
        Lang::Python,
    );
    let shadowed_some_rs = value_fp_named(
        &i,
        "fn Some(_value: i32) -> Option<i32> { None }\nfn f(xs: &[i32]) -> Vec<i32> { xs.iter().copied().filter_map(|x| if x > 0 { Some(x * 2) } else { None }).collect() }",
        Lang::Rust,
        "f",
    );
    let shadowed_none_rs = value_fp_named(
        &i,
        "const None: Option<i32> = Some(0);\nfn f(xs: &[i32]) -> Vec<i32> { xs.iter().copied().filter_map(|x| if x > 0 { Some(x * 2) } else { None }).collect() }",
        Lang::Rust,
        "f",
    );
    let shadowed_vec_rs = value_fp_named(
        &i,
        "struct Vec;\nimpl Vec { fn new() -> Vec { Vec } fn push(&self, _value: i32) {} }\nfn f(xs: &[i32]) -> Vec { let out = Vec::new(); for x in xs { if *x > 0 { out.push(*x * 2); } } out }",
        Lang::Rust,
        "f",
    );

    assert_ne!(
        filtered_py, shadowed_some_rs,
        "a local Rust Some definition must not be treated as Option::Some"
    );
    assert_ne!(
        filtered_py, shadowed_none_rs,
        "a local Rust None definition must not be treated as Option::None"
    );
    assert_ne!(
        filtered_py, shadowed_vec_rs,
        "a local Rust Vec definition must not be treated as std Vec::new"
    );
}
