use super::*;

const PY_ONE_LEVEL_FLATTEN: &str =
    "def f(groups):\n    return [value for group in groups for value in group]\n";

const SWIFT_ONE_LEVEL_FLATTEN: &str = r#"
func f(_ groups: [[Bool]]) -> [Bool] {
    return groups.flatMap { (group: [Bool]) in
        group.map { value in value }
    }
}
"#;

const SWIFT_IDENTITY_FLATTEN: &str = r#"
func f(_ groups: [[Bool]]) -> [Bool] {
    return groups.flatMap { (group: [Bool]) in group }
}
"#;

const PY_CROSS_PRODUCT: &str = "def f(xs, ys):\n    return [(x, y) for x in xs for y in ys]\n";

const SWIFT_CROSS_PRODUCT: &str = r#"
func f(_ xs: [Bool], _ ys: [Bool]) -> [(Bool, Bool)] {
    return xs.flatMap { x in ys.map { y in (x, y) } }
}
"#;

fn assert_flat_map_boundary(expected: &[u64], interner: &Interner, source: &str, label: &str) {
    assert_ne!(
        expected,
        value_fp_named(interner, source, Lang::Swift, "f"),
        "Swift one-level flatMap boundary must stay split: {label}"
    );
}

#[test]
fn swift_flat_map_converges_for_proven_one_level_sources_and_emitted_values() {
    let interner = Interner::new();
    let nested = value_fp(&interner, PY_ONE_LEVEL_FLATTEN, Lang::Python);
    assert!(
        nested.len() >= 4,
        "the one-level flatten claim must clear the exact semantic floor, got {nested:?}"
    );
    assert_eq!(
        nested,
        value_fp(&interner, SWIFT_ONE_LEVEL_FLATTEN, Lang::Swift),
        "a proven Swift flatMap/map must match the corresponding nested traversal"
    );
    assert_eq!(
        nested,
        value_fp(&interner, SWIFT_IDENTITY_FLATTEN, Lang::Swift),
        "flatMap over the proven inner collection itself is the same one-level flatten"
    );

    let cross_product = value_fp(&interner, PY_CROSS_PRODUCT, Lang::Python);
    assert_eq!(
        cross_product,
        value_fp(&interner, SWIFT_CROSS_PRODUCT, Lang::Swift),
        "independent plain outer and inner sources must preserve their nested order"
    );
}

#[test]
fn swift_flat_map_preserves_depth_order_value_and_source_boundaries() {
    let interner = Interner::new();
    let nested = value_fp(&interner, PY_ONE_LEVEL_FLATTEN, Lang::Python);
    let zero_flatten = r#"
func f(_ groups: [[Bool]]) -> [[Bool]] {
    return groups.map { (group: [Bool]) in group.map { value in value } }
}
"#;
    let derived_outer = r#"
func f(_ groups: [[Bool]]) -> [Bool] {
    return groups.map { group in group }.flatMap { (group: [Bool]) in
        group.map { value in value }
    }
}
"#;
    let derived_inner = r#"
func f(_ groups: [[Bool]]) -> [Bool] {
    return groups.flatMap { (group: [Bool]) in
        group.filter { value in value }.map { value in value }
    }
}
"#;
    let scalar_result = r#"
func f(_ values: [Bool]) -> [Bool] {
    return values.flatMap { value in value }
}
"#;
    for (source, label) in [
        (zero_flatten, "map keeps the inner collection nested"),
        (derived_outer, "derived outer source"),
        (derived_inner, "derived or filtered inner source"),
        (scalar_result, "scalar callback result"),
    ] {
        assert_flat_map_boundary(&nested, &interner, source, label);
    }

    let cross_product = value_fp(&interner, PY_CROSS_PRODUCT, Lang::Python);
    let reordered = r#"
func f(_ xs: [Bool], _ ys: [Bool]) -> [(Bool, Bool)] {
    return ys.flatMap { y in xs.map { x in (x, y) } }
}
"#;
    let changed_value = r#"
func f(_ xs: [Bool], _ ys: [Bool]) -> [(Bool, Bool)] {
    return xs.flatMap { x in ys.map { y in (y, x) } }
}
"#;
    let wrong_inner_source = r#"
func f(_ xs: [Bool], _ ys: [Bool], _ other: [Bool]) -> [(Bool, Bool)] {
    return xs.flatMap { x in other.map { y in (x, y) } }
}
"#;
    for (source, label) in [
        (reordered, "reordered outer and inner traversal"),
        (changed_value, "changed emitted value coordinate"),
        (wrong_inner_source, "wrong inner source coordinate"),
    ] {
        assert_flat_map_boundary(&cross_product, &interner, source, label);
    }
}

#[test]
fn swift_flat_map_keeps_recursive_depth_and_effects_closed() {
    let interner = Interner::new();
    let reference = r#"
def f(groups):
    return [row for rows in groups for row in rows]
"#;
    let expected = value_fp(&interner, reference, Lang::Python);
    let recursive_depth = r#"
func f(_ groups: [[[Bool]]]) -> [Bool] {
    return groups.flatMap { (rows: [[Bool]]) in
        rows.flatMap { (row: [Bool]) in row.map { value in value } }
    }
}
"#;
    let effectful_outer = r#"
func observe(_ group: [Bool]) {}
func f(_ groups: [[Bool]]) -> [Bool] {
    return groups.flatMap { (group: [Bool]) in
        observe(group)
        return group.map { value in value }
    }
}
"#;
    let effectful_inner = r#"
func observe(_ value: Bool) {}
func f(_ groups: [[Bool]]) -> [Bool] {
    return groups.flatMap { (group: [Bool]) in
        group.map { value in
            observe(value)
            return value
        }
    }
}
"#;
    for (source, label) in [
        (recursive_depth, "recursive/two-level flatten"),
        (effectful_outer, "effectful outer callback"),
        (effectful_inner, "effectful emitted-value callback"),
    ] {
        assert_flat_map_boundary(&expected, &interner, source, label);
    }
}

#[test]
fn swift_flat_map_requires_plain_sources_and_stdlib_dispatch() {
    let interner = Interner::new();
    let expected = value_fp(&interner, PY_ONE_LEVEL_FLATTEN, Lang::Python);
    let imported_namespace = format!("import Foundation\n{SWIFT_ONE_LEVEL_FLATTEN}");
    let nominal_source = r#"
func f(_ groups: Array<Array<Bool>>) -> [Bool] {
    return groups.flatMap { (group: [Bool]) in group.map { value in value } }
}
"#;
    let modified_source = r#"
func f(_ groups: sending [[Bool]]) -> [Bool] {
    return groups.flatMap { (group: [Bool]) in group.map { value in value } }
}
"#;
    let custom_flat_map = r#"
extension Array where Element == [Bool] {
    func flatMap<T>(_ transform: ([Bool]) -> [T]) -> [T] { [] }
}
func f(_ groups: [[Bool]]) -> [Bool] {
    return groups.flatMap { group in group.map { value in value } }
}
"#;
    let custom_inner_map = r#"
extension Array where Element == Bool {
    func map<T>(_ transform: (Bool) -> T) -> [T] { [] }
}
func f(_ groups: [[Bool]]) -> [Bool] {
    return groups.flatMap { group in group.map { value in value } }
}
"#;
    let callable_property = r#"
extension Array where Element == [Bool] {
    var `flatMap`: ((([Bool]) -> [Bool]) -> [Bool]) { { _ in [] } }
}
func f(_ groups: [[Bool]]) -> [Bool] {
    return groups.flatMap { group in group.map { value in value } }
}
"#;
    for (source, label) in [
        (imported_namespace.as_str(), "imported extension namespace"),
        (nominal_source, "nominal Array source"),
        (modified_source, "modified bracket-array source"),
        (custom_flat_map, "custom flatMap overload"),
        (custom_inner_map, "custom inner map overload"),
        (callable_property, "callable flatMap property"),
    ] {
        assert_flat_map_boundary(&expected, &interner, source, label);
    }
}

#[test]
fn swift_flat_map_cross_file_dispatch_ambiguity_stays_closed() {
    let reference = "def f(groups):\n    return [value for group in groups for value in group]\n";
    let consumer = SWIFT_ONE_LEVEL_FLATTEN;
    for (index, declaration) in [
        r#"extension Array where Element == [Bool] {
    func flatMap<T>(_ transform: ([Bool]) -> [T]) -> [T] { [] }
}
"#,
        r#"extension Array where Element == Bool {
    func map<T>(_ transform: (Bool) -> T) -> [T] { [] }
}
"#,
    ]
    .into_iter()
    .enumerate()
    {
        let (dir, corpus) = lower_temp_corpus(
            &format!("swift_flat_map_cross_file_{index}"),
            &[
                ("reference.py", reference),
                ("overload.swift", declaration),
                ("consumer.swift", consumer),
            ],
        );
        assert_ne!(
            corpus_value_fp(&corpus, "reference.py", "f"),
            corpus_value_fp(&corpus, "consumer.swift", "f"),
            "a cross-file flatMap/map overload must close the one-level proof"
        );
        std::fs::remove_dir_all(dir).unwrap();
    }
}

#[test]
fn swift_flat_map_rejects_attributed_callbacks_and_sibling_param_proof_borrowing() {
    let interner = Interner::new();
    let expected = value_fp(&interner, PY_ONE_LEVEL_FLATTEN, Lang::Python);
    let wrapped_outer = r#"
@propertyWrapper
struct ForceCollection {
    var wrappedValue: [Bool]
    init(wrappedValue: [Bool]) { self.wrappedValue = [true] }
}
func f(_ groups: [[Bool]]) -> [Bool] {
    groups.flatMap { (@ForceCollection group: [Bool]) in group }
}
"#;
    let wrapped_inner = r#"
@propertyWrapper
struct ForceValue {
    var wrappedValue: Bool
    init(wrappedValue: Bool) { self.wrappedValue = true }
}
func f(_ groups: [[Bool]]) -> [Bool] {
    groups.flatMap { (group: [Bool]) in
        group.map { (@ForceValue value: Bool) in value }
    }
}
"#;
    assert_flat_map_boundary(
        &expected,
        &interner,
        wrapped_outer,
        "property-wrapped inner collection coordinate",
    );
    assert_flat_map_boundary(
        &expected,
        &interner,
        wrapped_inner,
        "property-wrapped emitted value coordinate",
    );

    let captured_array = r#"
func f(_ groups: [[Bool]], _ source: [Bool]) -> [Bool] {
    groups.flatMap { group in source }
}
"#;
    let sibling_bracket_proof = r#"
func helper(_ flag: Bool, _ source: [Bool]) -> [Bool] { source }
func f(_ groups: [[Bool]], _ source: Set<Bool>) -> [Bool] {
    groups.flatMap { group in source }
}
"#;
    let normalized_has_flat_map = |source: &str| {
        let il = nose_frontend::lower_source(
            FileId(0),
            "scope.swift",
            source.as_bytes(),
            Lang::Swift,
            &interner,
        )
        .unwrap();
        normalize(&il, &interner, &NormalizeOptions::default())
            .nodes
            .iter()
            .any(|node| node.payload == nose_il::Payload::HoF(nose_il::HoFKind::FlatMap))
    };
    assert!(normalized_has_flat_map(captured_array));
    assert!(
        !normalized_has_flat_map(sibling_bracket_proof),
        "a Set source must not borrow bracket-array proof from a sibling function parameter"
    );

    let (dir, corpus) = lower_temp_corpus(
        "swift_flat_map_sibling_param_scope",
        &[
            ("captured_array.swift", captured_array),
            ("sibling_set.swift", sibling_bracket_proof),
        ],
    );
    let report = detect(
        &corpus,
        &DetectOptions {
            min_lines: 1,
            min_tokens: 1,
            block_units: false,
            ..Default::default()
        },
        &nose_detect::ExactBehaviorDetector,
    );
    assert!(
        !report.groups.iter().any(|group| {
            group
                .members
                .iter()
                .any(|member| member.file.ends_with("captured_array.swift"))
                && group
                    .members
                    .iter()
                    .any(|member| member.file.ends_with("sibling_set.swift"))
        }),
        "lexically unrelated collection proofs must not create an exact family"
    );
    std::fs::remove_dir_all(dir).unwrap();
}

#[test]
fn swift_flat_map_raw_custom_dispatch_does_not_form_an_exact_family() {
    let standard = r#"
func f(_ groups: [[Bool]]) -> [Bool] {
    groups.flatMap { (group: [Bool]) in group }
}
"#;
    let custom = r#"
func g(_ groups: Values) -> [Bool] {
    groups.flatMap { (group: [Bool]) in group }
}
"#;
    let overload = r#"
struct Values {
    func flatMap(_ transform: ([Bool]) -> [Bool]) -> [Bool] { [] }
}
"#;
    let (dir, corpus) = lower_temp_corpus(
        "swift_flat_map_raw_custom_dispatch",
        &[
            ("standard.swift", standard),
            ("custom.swift", custom),
            ("overload.swift", overload),
        ],
    );
    let options = DetectOptions {
        min_lines: 1,
        min_tokens: 1,
        block_units: false,
        ..Default::default()
    };
    let report = detect(&corpus, &options, &nose_detect::ExactBehaviorDetector);
    assert!(
        !report.groups.iter().any(|group| {
            group
                .members
                .iter()
                .any(|member| member.file.ends_with("standard.swift"))
                && group
                    .members
                    .iter()
                    .any(|member| member.file.ends_with("custom.swift"))
        }),
        "surviving raw Swift flatMap calls must not borrow opaque same-selector identity"
    );
    std::fs::remove_dir_all(dir).unwrap();
}
