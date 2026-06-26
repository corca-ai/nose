use super::*;

#[test]
fn swift_foreach_and_standard_apis_join_existing_semantic_families() {
    let i = Interner::new();
    let py_loop =
        "def f(xs):\n    total = 0\n    for x in xs:\n        if x > 0:\n            total = total + x\n    return total\n";
    let swift_loop = "func f(_ xs: [Int]) -> Int {\n    var total = 0\n    for x in xs {\n        if x > 0 {\n            total = total + x\n        }\n    }\n    return total\n}\n";
    assert_eq!(
        value_fp(&i, swift_loop, Lang::Swift),
        value_fp(&i, py_loop, Lang::Python),
        "Swift typed for-in accumulator should converge with the Python foreach accumulator"
    );

    let swift_count = "func f(_ xs: [Int]) -> Int {\n    return xs.count + 1\n}\n";
    let ts_count = "function f(xs: number[]): number {\n    return xs.length + 1;\n}\n";
    assert_eq!(
        value_fp(&i, swift_count, Lang::Swift),
        value_fp(&i, ts_count, Lang::TypeScript),
        "Swift count should converge with JS-family length"
    );

    let swift_empty = "func f(_ xs: [Int]) -> Bool {\n    return xs.isEmpty\n}\n";
    let java_empty =
        "import java.util.List;\nclass C { boolean f(List<Integer> xs) { return xs.isEmpty(); } }\n";
    assert_eq!(
        value_fp(&i, swift_empty, Lang::Swift),
        value_fp(&i, java_empty, Lang::Java),
        "Swift isEmpty should converge with Java collection isEmpty"
    );

    let swift_contains = "func f(_ xs: [Int]) -> Bool {\n    return xs.contains(3)\n}\n";
    let ts_contains = "function f(xs: number[]): boolean {\n    return xs.includes(3);\n}\n";
    let swift_contains_changed = "func f(_ xs: [Int]) -> Bool {\n    return xs.contains(4)\n}\n";
    assert_eq!(
        value_fp(&i, swift_contains, Lang::Swift),
        value_fp(&i, ts_contains, Lang::TypeScript),
        "Swift contains should converge with JS-family includes"
    );
    assert_ne!(
        value_fp(&i, swift_contains, Lang::Swift),
        value_fp(&i, swift_contains_changed, Lang::Swift),
        "changing the Swift membership element is a hard negative"
    );

    let swift_string =
        "func f(_ s: String) -> Bool {\n    return s.hasPrefix(\"a\") || s.hasSuffix(\"z\")\n}\n";
    let ts_string = "function f(s: string): boolean {\n    return s.startsWith(\"a\") || s.endsWith(\"z\");\n}\n";
    let swift_string_changed =
        "func f(_ s: String) -> Bool {\n    return s.hasPrefix(\"b\") || s.hasSuffix(\"z\")\n}\n";
    assert_eq!(
        value_fp(&i, swift_string, Lang::Swift),
        value_fp(&i, ts_string, Lang::TypeScript),
        "Swift string affix APIs should converge with JS-family affix APIs"
    );
    assert_ne!(
        value_fp(&i, swift_string, Lang::Swift),
        value_fp(&i, swift_string_changed, Lang::Swift),
        "changing a Swift string literal in an affix predicate is a hard negative"
    );

    let swift_nullish =
        "func f(_ value: Int?, _ fallback: Int, _ other: Int?) -> Int {\n    return value ?? fallback\n}\n";
    let swift_nullish_renamed =
        "func g(_ candidate: Int?, _ defaultValue: Int, _ ignored: Int?) -> Int {\n    return candidate ?? defaultValue\n}\n";
    let swift_wrong_default =
        "func f(_ value: Int?, _ fallback: Int, _ other: Int?) -> Int {\n    return value ?? 0\n}\n";
    let swift_wrong_value =
        "func f(_ value: Int?, _ fallback: Int, _ other: Int?) -> Int {\n    return other ?? fallback\n}\n";
    assert_eq!(
        unit_hash(&i, swift_nullish, Lang::Swift),
        unit_hash(&i, swift_nullish_renamed, Lang::Swift),
        "Swift nil-coalescing should normalize alpha-equivalently"
    );
    assert_ne!(
        unit_hash(&i, swift_nullish, Lang::Swift),
        unit_hash(&i, swift_wrong_default, Lang::Swift),
        "changing the Swift nil-coalescing fallback is a hard negative"
    );
    assert_ne!(
        unit_hash(&i, swift_nullish, Lang::Swift),
        unit_hash(&i, swift_wrong_value, Lang::Swift),
        "changing the Swift nil-coalescing value coordinate is a hard negative"
    );
}

#[test]
fn string_affix_coordinate_sources_keep_safe_boundaries() {
    let i = Interner::new();
    let ts_param = "function f(subject: string, prefix: string, other: string): boolean {\n    return subject.startsWith(prefix);\n}\n";
    let swift_param = "func f(_ subject: String, _ prefix: String, _ other: String) -> Bool {\n    return subject.hasPrefix(prefix)\n}\n";
    let py_param = "def f(subject: str, prefix: str, other: str) -> bool:\n    return subject.startswith(prefix)\n";
    let ts_wrong_param = "function f(subject: string, prefix: string, other: string): boolean {\n    return subject.startsWith(other);\n}\n";
    let ts_dynamic = "function f(subject: string, prefix: string): boolean {\n    const normalized = prefix.trim();\n    return subject.startsWith(normalized);\n}\n";

    let param_fp = value_fp(&i, ts_param, Lang::TypeScript);
    assert_eq!(param_fp, value_fp(&i, swift_param, Lang::Swift));
    assert_eq!(param_fp, value_fp(&i, py_param, Lang::Python));
    assert_ne!(param_fp, value_fp(&i, ts_wrong_param, Lang::TypeScript));
    assert_ne!(param_fp, value_fp(&i, ts_dynamic, Lang::TypeScript));

    let py_literal = "def f(subject: str) -> bool:\n    return subject.startswith(\"pre\")\n";
    let ts_local = "function f(subject: string): boolean {\n    const prefix = \"pre\";\n    return subject.startsWith(prefix);\n}\n";
    let py_module =
        "PREFIX = \"pre\"\ndef f(subject: str) -> bool:\n    return subject.startswith(PREFIX)\n";
    let ts_mutated = "let PREFIX = \"pre\";\nPREFIX = \"other\";\nfunction f(subject: string): boolean {\n    return subject.startsWith(PREFIX);\n}\n";
    let literal_fp = value_fp(&i, py_literal, Lang::Python);
    assert_eq!(literal_fp, value_fp(&i, ts_local, Lang::TypeScript));
    assert_eq!(literal_fp, value_fp(&i, py_module, Lang::Python));
    assert_ne!(literal_fp, value_fp(&i, ts_mutated, Lang::TypeScript));

    let py_tuple =
        "def f(subject: str) -> bool:\n    return subject.startswith((\"pre\", \"alt\"))\n";
    let js_offset = "function f(subject) {\n    return subject.startsWith(\"pre\", 1);\n}\n";
    let java_offset =
        "class C { boolean f(String subject) { return subject.startsWith(\"pre\", 1); } }\n";
    assert_ne!(literal_fp, value_fp(&i, py_tuple, Lang::Python));
    assert_ne!(literal_fp, value_fp(&i, js_offset, Lang::JavaScript));
    assert_ne!(literal_fp, value_fp(&i, java_offset, Lang::Java));
}

#[test]
fn python_iterator_materializer_identity_stays_behavior_defining() {
    let i = Interner::new();
    let list_map = "def f(xs: list[int]):\n    return list(map(lambda x: x + 1, xs))\n";
    let list_comp = "def f(xs: list[int]):\n    return [x + 1 for x in xs]\n";
    let tuple_map = "def f(xs: list[int]):\n    return tuple(map(lambda x: x + 1, xs))\n";
    let set_map = "def f(xs: list[int]):\n    return set(map(lambda x: x + 1, xs))\n";
    let frozenset_map = "def f(xs: list[int]):\n    return frozenset(map(lambda x: x + 1, xs))\n";

    let list_fp = value_fp(&i, list_map, Lang::Python);
    assert_eq!(
        list_fp,
        value_fp(&i, list_comp, Lang::Python),
        "list(map(lambda, xs)) should keep converging with the equivalent list comprehension"
    );
    assert_ne!(
        list_fp,
        value_fp(&i, tuple_map, Lang::Python),
        "tuple(map(...)) materialization is not a list materialization"
    );
    assert_ne!(
        list_fp,
        value_fp(&i, set_map, Lang::Python),
        "set(map(...)) materialization is not a list materialization"
    );
    assert_ne!(
        value_fp(&i, tuple_map, Lang::Python),
        value_fp(&i, set_map, Lang::Python),
        "tuple and set materializers preserve different collection semantics"
    );
    assert_ne!(
        value_fp(&i, set_map, Lang::Python),
        value_fp(&i, frozenset_map, Lang::Python),
        "set and frozenset materializers preserve mutability/hashability boundaries"
    );
}

#[test]
fn java_stream_aggregates_converge_with_loops() {
    // Java stream pipelines should lower into the same shared iteration/reduction
    // shapes as enhanced-for loops: `Arrays.stream(xs)` is just the source collection,
    // and `anyMatch`/`allMatch` are predicate reductions.
    let i = Interner::new();
    let sum_loop = "class C { static int f(int[] xs) { int total = 0; for (int x : xs) { if (x > 0) { total += x; } } return total; } }";
    let sum_stream = "import java.util.Arrays; class C { static int f(int[] xs) { return Arrays.stream(xs).filter(x -> x > 0).reduce(0, (total, x) -> total + x); } }";
    let count_loop = "class C { static int f(int[] xs) { int count = 0; for (int x : xs) { if (x > 0) { count += 1; } } return count; } }";
    let count_stream = "import java.util.Arrays; class C { static int f(int[] xs) { return (int) Arrays.stream(xs).filter(x -> x > 0).count(); } }";
    let any_loop = "class C { static boolean f(int[] xs) { for (int x : xs) { if (x > 0) { return true; } } return false; } }";
    let any_stream = "import java.util.Arrays; class C { static boolean f(int[] xs) { return Arrays.stream(xs).anyMatch(x -> x > 0); } }";
    let all_loop = "class C { static boolean f(int[] xs) { for (int x : xs) { if (!(x >= 0)) { return false; } } return true; } }";
    let all_stream = "import java.util.Arrays; class C { static boolean f(int[] xs) { return Arrays.stream(xs).allMatch(x -> x >= 0); } }";
    let bad_seed =
        "import java.util.Arrays; class C { static int f(int[] xs) { return Arrays.stream(xs).filter(x -> x > 0).reduce(1, (total, x) -> total + x); } }";
    let missing_arrays_import = "class C { static int f(int[] xs) { return Arrays.stream(xs).filter(x -> x > 0).reduce(0, (total, x) -> total + x); } }";
    let shadowed_arrays =
        "import java.util.Arrays; class Arrays {} class C { static int f(int[] xs) { return Arrays.stream(xs).filter(x -> x > 0).reduce(0, (total, x) -> total + x); } }";
    let sum_fp = value_fp(&i, sum_loop, Lang::Java);
    assert_eq!(sum_fp, value_fp(&i, sum_stream, Lang::Java));
    assert_eq!(
        return_fp(&i, count_loop, Lang::Java),
        return_fp(&i, count_stream, Lang::Java)
    );
    assert_eq!(
        value_fp(&i, any_loop, Lang::Java),
        value_fp(&i, any_stream, Lang::Java)
    );
    assert_eq!(
        value_fp(&i, all_loop, Lang::Java),
        value_fp(&i, all_stream, Lang::Java)
    );
    assert_ne!(sum_fp, value_fp(&i, bad_seed, Lang::Java));
    assert_ne!(sum_fp, value_fp(&i, missing_arrays_import, Lang::Java));
    assert_ne!(sum_fp, value_fp(&i, shadowed_arrays, Lang::Java));
}

#[test]
fn java_stream_flat_map_converges_with_python_comprehension() {
    let i = Interner::new();
    let py_flat = "def f(xs, ys):\n    return [x + y for x in xs for y in ys]\n";
    let java_flat = "import java.util.Arrays; class C { static Object f(int[] xs, int[] ys) { return Arrays.stream(xs).flatMap(x -> Arrays.stream(ys).map(y -> x + y)); } }";
    let py_nested = "def f(xs, ys):\n    return [[x + y for y in ys] for x in xs]\n";
    let java_nested = "import java.util.Arrays; class C { static Object f(int[] xs, int[] ys) { return Arrays.stream(xs).map(x -> Arrays.stream(ys).map(y -> x + y)); } }";

    let flat_fp = value_fp(&i, py_flat, Lang::Python);
    let nested_fp = value_fp(&i, py_nested, Lang::Python);
    assert_eq!(
        flat_fp,
        value_fp(&i, java_flat, Lang::Java),
        "Java Stream.flatMap/map should match Python multi-clause comprehension"
    );
    assert_eq!(
        nested_fp,
        value_fp(&i, java_nested, Lang::Java),
        "Java Stream.map returning streams should stay nested"
    );
    assert_ne!(
        flat_fp, nested_fp,
        "flatMap and map-returning-stream must remain distinct"
    );
}

#[test]
fn swift_flat_map_converges_with_nested_builder_loop() {
    let i = Interner::new();
    let builder = r#"
func f(_ groups: [[Int]]) -> [Int] {
    var out: [Int] = []
    for xs in groups {
        for y in xs {
            out.append(y)
        }
    }
    return out
}
"#;
    let flat = r#"
func f(_ groups: [[Int]]) -> [Int] {
    return groups.flatMap { (xs: [Int]) in xs.map { y in y } }
}
"#;
    let changed = r#"
func f(_ groups: [[Int]]) -> [Int] {
    return groups.flatMap { (xs: [Int]) in xs.map { y in y + 1 } }
}
"#;
    let nested = r#"
func f(_ groups: [[Int]]) -> [[Int]] {
    return groups.map { (xs: [Int]) in xs.map { y in y } }
}
"#;

    let fp = value_fp(&i, builder, Lang::Swift);
    assert_eq!(
        fp,
        value_fp(&i, flat, Lang::Swift),
        "Swift nested append builders and flatMap/map should share the flattened stream value"
    );
    assert_ne!(
        fp,
        value_fp(&i, changed, Lang::Swift),
        "changing the emitted flattened element must stay distinct"
    );
    assert_ne!(
        fp,
        value_fp(&i, nested, Lang::Swift),
        "map returning inner arrays is not a flatMap"
    );
}

#[test]
fn rust_vec_new_builder_loop_stays_distinct_from_flat_map_without_nested_element_proof() {
    // `xss: &[Vec<_>]` proves the outer parameter is a collection, but the current semantic
    // kernel does not yet carry the element-type proof needed to know that the lambda parameter
    // `xs` is itself a collection. Keep the `.flat_map(|xs| xs.iter()...)` form fail-closed until
    // nested collection receiver proofs exist.
    let i = Interner::new();
    let builder = "pub fn f(xss: &[Vec<i64>]) -> Vec<i64> { let mut out = Vec::new(); for xs in xss { for y in xs { out.push(*y); } } out }";
    let flat = "pub fn f(xss: &[Vec<i64>]) -> Vec<i64> { xss.iter().flat_map(|xs| xs.iter().map(|y| *y)).collect() }";
    let changed = "pub fn f(xss: &[Vec<i64>]) -> Vec<i64> { xss.iter().flat_map(|xs| xs.iter().map(|y| y + 1)).collect() }";
    let bfp = value_fp(&i, builder, Lang::Rust);
    assert_ne!(
        bfp,
        value_fp(&i, flat, Lang::Rust),
        "nested flat_map must not converge without a nested element collection proof"
    );
    assert_ne!(
        bfp,
        value_fp(&i, changed, Lang::Rust),
        "a changed inner element (y + 1) stays a hard negative"
    );
}
