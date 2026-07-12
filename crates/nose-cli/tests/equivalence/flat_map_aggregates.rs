use super::*;

#[test]
fn java_flat_map_sum_converges_with_nested_reduction() {
    let i = Interner::new();
    let nested = "class C { static int f(int[] xs, int[] ys) { int total = 0; for (int x : xs) { for (int y : ys) { total += x + y; } } return total; } }";
    let aggregate = "import java.util.Arrays; class C { static int f(int[] xs, int[] ys) { return Arrays.stream(xs).flatMap(x -> Arrays.stream(ys).map(y -> x + y)).reduce(0, (total, value) -> total + value); } }";

    assert_eq!(
        value_fp(&i, nested, Lang::Java),
        value_fp(&i, aggregate, Lang::Java),
        "Java Stream.flatMap reduce should consume the proven flattened element coordinate",
    );
}

#[test]
fn java_filtered_flat_map_sum_preserves_outer_and_inner_guards() {
    let i = Interner::new();
    let nested = "class C { static int f(int[] xs, int[] ys) { int total = 0; for (int x : xs) { if (x > 0) { for (int y : ys) { if (y < 10) { total += x + y; } } } } return total; } }";
    let aggregate = "import java.util.Arrays; class C { static int f(int[] xs, int[] ys) { return Arrays.stream(xs).filter(x -> x > 0).flatMap(x -> Arrays.stream(ys).filter(y -> y < 10).map(y -> x + y)).reduce(0, (total, value) -> total + value); } }";
    let wrong_outer_guard = "import java.util.Arrays; class C { static int f(int[] xs, int[] ys) { return Arrays.stream(xs).filter(x -> x < 0).flatMap(x -> Arrays.stream(ys).filter(y -> y < 10).map(y -> x + y)).reduce(0, (total, value) -> total + value); } }";
    let wrong_inner_guard = "import java.util.Arrays; class C { static int f(int[] xs, int[] ys) { return Arrays.stream(xs).filter(x -> x > 0).flatMap(x -> Arrays.stream(ys).filter(y -> y > 10).map(y -> x + y)).reduce(0, (total, value) -> total + value); } }";

    let expected = value_fp(&i, nested, Lang::Java);
    assert_eq!(
        expected,
        value_fp(&i, aggregate, Lang::Java),
        "Java flat-map aggregate guards should stay attached to their traversal coordinates",
    );
    assert_ne!(
        expected,
        value_fp(&i, wrong_outer_guard, Lang::Java),
        "changing the Java outer aggregate guard must stay distinct",
    );
    assert_ne!(
        expected,
        value_fp(&i, wrong_inner_guard, Lang::Java),
        "changing the Java inner aggregate guard must stay distinct",
    );
}

#[test]
fn java_flat_map_sum_preserves_identity_step_source_depth_effect_and_dispatch() {
    let i = Interner::new();
    let nested = "class C { static int f(int[] xs, int[] ys, int[] other) { int total = 0; for (int x : xs) { for (int y : ys) { total += x + y; } } return total; } }";
    let wrong_seed = "import java.util.Arrays; class C { static int f(int[] xs, int[] ys, int[] other) { return Arrays.stream(xs).flatMap(x -> Arrays.stream(ys).map(y -> x + y)).reduce(1, (total, value) -> total + value); } }";
    let wrong_step = "import java.util.Arrays; class C { static int f(int[] xs, int[] ys, int[] other) { return Arrays.stream(xs).flatMap(x -> Arrays.stream(ys).map(y -> x + y)).reduce(0, (total, value) -> total + value + 1); } }";
    let wrong_source = "import java.util.Arrays; class C { static int f(int[] xs, int[] ys, int[] other) { return Arrays.stream(xs).flatMap(x -> Arrays.stream(other).map(y -> x + y)).reduce(0, (total, value) -> total + value); } }";
    let wrong_depth = "import java.util.Arrays; class C { static Object f(int[] xs, int[] ys, int[] other) { return Arrays.stream(xs).map(x -> Arrays.stream(ys).map(y -> x + y)); } }";
    let effectful = "import java.util.Arrays; class C { static void observe(int x) {} static int f(int[] xs, int[] ys, int[] other) { return Arrays.stream(xs).flatMap(x -> { observe(x); return Arrays.stream(ys).map(y -> x + y); }).reduce(0, (total, value) -> total + value); } }";
    let custom_dispatch = "import java.util.Arrays; class C { static int f(int[] xs, int[] ys, int[] other) { return Arrays.stream(xs).flatMap(x -> Arrays.stream(ys).map(y -> x + y)).reduce(0, (total, value) -> total + value); } } class Arrays {}";
    let expected = value_fp(&i, nested, Lang::Java);

    for (source, boundary) in [
        (wrong_seed, "reduction identity"),
        (wrong_step, "reduction step"),
        (wrong_source, "inner source"),
        (wrong_depth, "flatten depth"),
        (effectful, "callback effect"),
        (custom_dispatch, "custom dispatch"),
    ] {
        assert_ne!(
            expected,
            value_fp(&i, source, Lang::Java),
            "changing the Java flat-map aggregate {boundary} must stay distinct",
        );
    }
}

#[test]
fn java_flat_map_aggregate_closes_when_inner_cardinality_is_not_in_the_emitted_value() {
    let i = Interner::new();
    let from_ys = "import java.util.Arrays; class C { static int f(int[] xs, int[] ys, int[] other) { return Arrays.stream(xs).flatMap(x -> Arrays.stream(ys).map(y -> x)).reduce(0, (total, value) -> total + value); } }";
    let from_other = "import java.util.Arrays; class C { static int f(int[] xs, int[] ys, int[] other) { return Arrays.stream(xs).flatMap(x -> Arrays.stream(other).map(y -> x)).reduce(0, (total, value) -> total + value); } }";
    let direct_outer = "import java.util.Arrays; class C { static int f(int[] xs, int[] ys, int[] other) { return Arrays.stream(xs).reduce(0, (total, value) -> total + value); } }";

    assert_ne!(
        value_fp(&i, from_ys, Lang::Java),
        value_fp(&i, from_other, Lang::Java),
        "an inner map that ignores its element must still retain the inner source coordinate",
    );
    assert_ne!(
        value_fp(&i, from_ys, Lang::Java),
        value_fp(&i, direct_outer, Lang::Java),
        "repeating each outer value once per inner element must not collapse to one outer reduction",
    );
}

#[test]
fn java_flat_map_aggregate_closes_when_nested_coordinates_alias_the_same_source() {
    let i = Interner::new();
    let direct = "import java.util.Arrays; class C { static int f(int[] xs) { return Arrays.stream(xs).reduce(0, (total, value) -> total + value); } }";
    let captured_outer = "import java.util.Arrays; class C { static int f(int[] xs) { return Arrays.stream(xs).flatMap(x -> Arrays.stream(xs).map(y -> x)).reduce(0, (total, value) -> total + value); } }";
    let emitted_inner = "import java.util.Arrays; class C { static int f(int[] xs) { return Arrays.stream(xs).flatMap(x -> Arrays.stream(xs).map(y -> y)).reduce(0, (total, value) -> total + value); } }";
    let emitted_both = "import java.util.Arrays; class C { static int f(int[] xs) { return Arrays.stream(xs).flatMap(x -> Arrays.stream(xs).map(y -> x + y)).reduce(0, (total, value) -> total + value); } }";
    let direct_fp = value_fp(&i, direct, Lang::Java);

    for (source, emitted) in [
        (captured_outer, "outer value"),
        (emitted_inner, "inner value"),
        (emitted_both, "outer and inner values"),
    ] {
        assert_ne!(
            direct_fp,
            value_fp(&i, source, Lang::Java),
            "same-source nested iteration emitting {emitted} must retain two distinct repetition coordinates",
        );
    }

    let derived_outer = "import java.util.Arrays; class C { static int f(int[] xs) { return Arrays.stream(xs).map(a -> a + 1).flatMap(x -> Arrays.stream(xs).map(y -> x + y)).reduce(0, (total, value) -> total + value); } }";
    let direct_derived = "import java.util.Arrays; class C { static int f(int[] xs) { return Arrays.stream(xs).map(a -> (a + 1) + a).reduce(0, (total, value) -> total + value); } }";
    assert_ne!(
        value_fp(&i, derived_outer, Lang::Java),
        value_fp(&i, direct_derived, Lang::Java),
        "a derived outer coordinate must not hide repeated iteration over its original source",
    );
}

#[test]
fn java_recursive_flat_map_aggregate_stays_closed() {
    let i = Interner::new();
    let nested = "class C { static int f(int[] xs, int[] ys, int[] zs) { int total = 0; for (int x : xs) { for (int y : ys) { for (int z : zs) { total += x + y + z; } } } return total; } }";
    let recursive = "import java.util.Arrays; class C { static int f(int[] xs, int[] ys, int[] zs) { return Arrays.stream(xs).flatMap(x -> Arrays.stream(ys).flatMap(y -> Arrays.stream(zs).map(z -> x + y + z))).reduce(0, (total, value) -> total + value); } }";
    let wrapped_recursive = "import java.util.Arrays; class C { static int f(int[] xs, int[] ys, int[] zs) { return Arrays.stream(xs).flatMap(x -> Arrays.stream(ys).flatMap(y -> Arrays.stream(zs).map(z -> x + y + z)).map(value -> value)).reduce(0, (total, value) -> total + value); } }";

    let nested_fp = value_fp(&i, nested, Lang::Java);
    for (source, shape) in [
        (recursive, "direct recursive output"),
        (
            wrapped_recursive,
            "recursive output wrapped in identity map",
        ),
    ] {
        assert_ne!(
            nested_fp,
            value_fp(&i, source, Lang::Java),
            "{shape} remains closed outside the controlled one-level aggregate proof",
        );
    }
}

#[test]
fn java_flat_map_reducer_step_must_consume_the_flattened_value() {
    let i = Interner::new();
    let from_ys = "import java.util.Arrays; class C { static int f(int[] xs, int[] ys, int[] other) { return Arrays.stream(xs).flatMap(x -> Arrays.stream(ys).map(y -> x + y)).reduce(0, (total, value) -> total + 1); } }";
    let from_other = "import java.util.Arrays; class C { static int f(int[] xs, int[] ys, int[] other) { return Arrays.stream(xs).flatMap(x -> Arrays.stream(other).map(y -> x + y)).reduce(0, (total, value) -> total + 1); } }";

    assert_ne!(
        value_fp(&i, from_ys, Lang::Java),
        value_fp(&i, from_other, Lang::Java),
        "a reducer step that ignores the flattened value must retain source cardinality by staying opaque",
    );
}

#[test]
fn swift_flat_map_reduce_stays_closed_without_fold_dispatch_proof() {
    let i = Interner::new();
    let source = r#"
func nestedSum(_ groups: [[Int]]) -> Int {
    var total = 0
    for group in groups {
        for value in group {
            total += value
        }
    }
    return total
}

func aggregateSum(_ groups: [[Int]]) -> Int {
    return groups.flatMap { (group: [Int]) in group.map { value in value } }
        .reduce(0) { (total: Int, value: Int) in total + value }
}
"#;

    let reference = value_fp(
        &i,
        "def reference(groups):\n    return sum(value for group in groups for value in group)\n",
        Lang::Python,
    );
    let nested = value_fp_named(&i, source, Lang::Swift, "nestedSum");
    let aggregate = value_fp_named(&i, source, Lang::Swift, "aggregateSum");
    assert_eq!(
        reference, nested,
        "Swift nested reduction should match the reference"
    );
    assert_ne!(
        nested, aggregate,
        "Swift reduce remains closed until its fold callback and overload dispatch are separately proven",
    );
}

#[test]
fn swift_flat_map_all_satisfy_converges_with_nested_counterexample_loop() {
    let i = Interner::new();
    let source = r#"
func nestedAll(_ groups: [[Int]], _ minimum: Int) -> Bool {
    for group in groups {
        for value in group {
            if !(value >= minimum) {
                return false
            }
        }
    }
    return true
}

func aggregateAll(_ groups: [[Int]], _ minimum: Int) -> Bool {
    return groups.flatMap { (group: [Int]) in group.map { value in value } }
        .allSatisfy { value in value >= minimum }
}
"#;

    let reference = value_fp(
        &i,
        "def reference(groups, minimum):\n    return all(value >= minimum for group in groups for value in group)\n",
        Lang::Python,
    );
    let nested = value_fp_named(&i, source, Lang::Swift, "nestedAll");
    let aggregate = value_fp_named(&i, source, Lang::Swift, "aggregateAll");
    assert_eq!(
        reference, nested,
        "Swift nested all loop should match the reference"
    );
    assert_eq!(
        nested, aggregate,
        "Swift allSatisfy should preserve the flattened terminal predicate coordinate",
    );
}

#[test]
fn swift_flat_map_aggregate_closes_when_inner_cardinality_is_not_in_the_emitted_value() {
    let i = Interner::new();
    let source = r#"
func aggregateFromYs(_ xs: [Int], _ ys: [Int], _ other: [Int], _ minimum: Int) -> Bool {
    return xs.flatMap { (x: Int) in
        ys.map { y in x }
    }.allSatisfy { value in value >= minimum }
}

func aggregateFromOther(_ xs: [Int], _ ys: [Int], _ other: [Int], _ minimum: Int) -> Bool {
    return xs.flatMap { (x: Int) in
        other.map { y in x }
    }.allSatisfy { value in value >= minimum }
}

func directOuter(_ xs: [Int], _ ys: [Int], _ other: [Int], _ minimum: Int) -> Bool {
    return xs.allSatisfy { value in value >= minimum }
}
"#;

    assert_ne!(
        value_fp_named(&i, source, Lang::Swift, "aggregateFromYs"),
        value_fp_named(&i, source, Lang::Swift, "aggregateFromOther"),
        "Swift flatMap must retain an inner source whose element is ignored by the emitted value",
    );
    assert_ne!(
        value_fp_named(&i, source, Lang::Swift, "aggregateFromYs"),
        value_fp_named(&i, source, Lang::Swift, "directOuter"),
        "Swift flatMap must retain inner-source emptiness instead of collapsing to a direct outer terminal",
    );
}

#[test]
fn swift_flat_map_terminal_predicate_must_consume_the_flattened_value() {
    let i = Interner::new();
    let source = r#"
func aggregateGroups(_ groups: [[Int]], _ other: [[Int]]) -> Bool {
    return groups.flatMap { (group: [Int]) in group.map { value in value } }
        .allSatisfy { value in false }
}

func aggregateOther(_ groups: [[Int]], _ other: [[Int]]) -> Bool {
    return other.flatMap { (group: [Int]) in group.map { value in value } }
        .allSatisfy { value in false }
}
"#;

    assert_ne!(
        value_fp_named(&i, source, Lang::Swift, "aggregateGroups"),
        value_fp_named(&i, source, Lang::Swift, "aggregateOther"),
        "a terminal predicate that ignores the flattened value must retain empty/non-empty source behavior by staying opaque",
    );
}

#[test]
fn swift_eager_flat_map_filter_with_overloadable_predicate_stays_closed() {
    let i = Interner::new();
    let source = r#"
var comparisons = 0
func == (left: [Int], right: [Int]) -> Bool {
    comparisons += 1
    return true
}

func nestedAll(_ groups: [[Int]], _ minimum: Int) -> Bool {
    for group in groups {
        if group == group {
            for value in group {
                if !(value >= minimum) {
                    return false
                }
            }
        }
    }
    return true
}

func aggregateAll(_ groups: [[Int]], _ minimum: Int) -> Bool {
    return groups.filter { group in group == group }
        .flatMap { (group: [Int]) in group.map { value in value } }
        .allSatisfy { value in value >= minimum }
}

func nestedControl(_ groups: [[Int]], _ enabled: Bool, _ minimum: Int) -> Bool {
    for group in groups {
        if enabled {
            for value in group {
                if !(value >= minimum) {
                    return false
                }
            }
        }
    }
    return true
}

func aggregateControl(_ groups: [[Int]], _ enabled: Bool, _ minimum: Int) -> Bool {
    return groups.filter { group in enabled }
        .flatMap { (group: [Int]) in group.map { value in value } }
        .allSatisfy { value in value >= minimum }
}
"#;

    assert_eq!(
        value_fp_named(&i, source, Lang::Swift, "nestedControl"),
        value_fp_named(&i, source, Lang::Swift, "aggregateControl"),
        "the controlled pure-filter shape should reach the admitted aggregate bridge",
    );
    assert_ne!(
        value_fp_named(&i, source, Lang::Swift, "nestedAll"),
        value_fp_named(&i, source, Lang::Swift, "aggregateAll"),
        "an eager Swift filter with overloadable predicate dispatch must not merge with a short-circuit nested loop",
    );
}

#[test]
fn swift_filtered_flat_map_all_satisfy_preserves_guard_coordinates() {
    let i = Interner::new();
    let source = r#"
func nestedFiltered(
    _ groups: [[Int]],
    _ outerGuard: Bool,
    _ innerGuard: Bool,
    _ minimum: Int
) -> Bool {
    for group in groups {
        if outerGuard {
            for value in group {
                if innerGuard && !(value >= minimum) {
                    return false
                }
            }
        }
    }
    return true
}

func aggregateFiltered(
    _ groups: [[Int]],
    _ outerGuard: Bool,
    _ innerGuard: Bool,
    _ minimum: Int
) -> Bool {
    return groups.filter { group in outerGuard }.flatMap { (group: [Int]) in
        group.filter { value in innerGuard }.map { value in value }
    }.allSatisfy { value in value >= minimum }
}

func aggregateWrongOuterGuard(
    _ groups: [[Int]],
    _ outerGuard: Bool,
    _ innerGuard: Bool,
    _ minimum: Int
) -> Bool {
    return groups.filter { group in !outerGuard }.flatMap { (group: [Int]) in
        group.filter { value in innerGuard }.map { value in value }
    }.allSatisfy { value in value >= minimum }
}

func aggregateWrongInnerGuard(
    _ groups: [[Int]],
    _ outerGuard: Bool,
    _ innerGuard: Bool,
    _ minimum: Int
) -> Bool {
    return groups.filter { group in outerGuard }.flatMap { (group: [Int]) in
        group.filter { value in !innerGuard }.map { value in value }
    }.allSatisfy { value in value >= minimum }
}

func aggregateWrongTerminal(
    _ groups: [[Int]],
    _ outerGuard: Bool,
    _ innerGuard: Bool,
    _ minimum: Int
) -> Bool {
    return groups.filter { group in outerGuard }.flatMap { (group: [Int]) in
        group.filter { value in innerGuard }.map { value in value }
    }.allSatisfy { value in value > minimum }
}
"#;

    let expected = value_fp_named(&i, source, Lang::Swift, "nestedFiltered");
    assert_eq!(
        expected,
        value_fp_named(&i, source, Lang::Swift, "aggregateFiltered"),
        "Swift aggregate guards should stay attached to their outer and inner traversal coordinates",
    );
    for (name, boundary) in [
        ("aggregateWrongOuterGuard", "outer guard"),
        ("aggregateWrongInnerGuard", "inner guard"),
        ("aggregateWrongTerminal", "terminal predicate"),
    ] {
        assert_ne!(
            expected,
            value_fp_named(&i, source, Lang::Swift, name),
            "changing the Swift flat-map aggregate {boundary} must stay distinct",
        );
    }
}

#[test]
fn swift_flat_map_aggregate_custom_terminal_dispatch_stays_closed() {
    let i = Interner::new();
    let source = r#"
extension Array where Element == Int {
    func allSatisfy(_ predicate: (Int) -> Bool) -> Bool {
        return false
    }
}

func nestedAll(_ groups: [[Int]], _ minimum: Int) -> Bool {
    for group in groups {
        for value in group {
            if !(value >= minimum) {
                return false
            }
        }
    }
    return true
}

func customAggregateAll(_ groups: [[Int]], _ minimum: Int) -> Bool {
    return groups.flatMap { (group: [Int]) in group.map { value in value } }
        .allSatisfy { value in value >= minimum }
}
"#;

    assert_ne!(
        value_fp_named(&i, source, Lang::Swift, "nestedAll"),
        value_fp_named(&i, source, Lang::Swift, "customAggregateAll"),
        "a visible custom allSatisfy overload must close Swift terminal aggregate admission",
    );
}
