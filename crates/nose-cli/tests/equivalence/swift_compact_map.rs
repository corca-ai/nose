use super::*;

const PY_FILTERED_IDENTITY: &str = "def f(xs):\n    return [x for x in xs if x]\n";

const SWIFT_COMPACT_MAP: &str = r#"
func f(_ xs: [Bool]) -> [Bool] {
    return xs.compactMap { x in x ? x : nil }
}
"#;

const SWIFT_CUSTOM_NOMINAL_COLLECTION: &str = r#"
@dynamicMemberLookup
struct Collection {
    subscript(dynamicMember name: String) -> (((Bool) -> Bool?) -> [Bool]) { { _ in [] } }
}
func f(_ xs: Collection) -> [Bool] {
    return xs.compactMap { x in x ? x : nil }
}
"#;

const SWIFT_CUSTOM_NOMINAL_ARRAY: &str = r#"
@dynamicMemberLookup
struct Array<Element> {
    subscript(dynamicMember name: String) -> (((Element) -> Element?) -> Swift.Array<Element>) {
        { _ in [] }
    }
}
func f(_ xs: Array<Bool>) -> Swift.Array<Bool> {
    return xs.compactMap { x in x ? x : nil }
}
"#;

const SWIFT_COMPACT_MAP_PROPERTY: &str = r#"
extension Array where Element==Bool{var `compactMap`:(((Bool)->Bool?)->[Bool]){{_ in []}}}
func f(_ xs: [Bool]) -> [Bool] {
    return xs.compactMap { x in x ? x : nil }
}
"#;

const SWIFT_PROPERTY_WRAPPED_PARAMETER: &str = r#"
@propertyWrapper
struct ForceTrue {
    var wrappedValue: [Bool]
    init(wrappedValue: [Bool]) { self.wrappedValue = [true] }
}
func f(@ForceTrue /* source-altering wrapper */ _ xs: [Bool]) -> [Bool] {
    return xs.compactMap { x in x ? x : nil }
}
"#;

const SWIFT_SENDING_PARAMETER: &str = r#"
func f(_ xs: sending [Bool]) -> [Bool] {
    return xs.compactMap { x in x ? x : nil }
}
"#;

const SWIFT_SHARED_PARAMETER: &str = r#"
func f(_ xs: __shared [Bool]) -> [Bool] {
    return xs.compactMap { x in x ? x : nil }
}
"#;

const SWIFT_OWNED_PARAMETER: &str = r#"
func f(_ xs: __owned [Bool]) -> [Bool] {
    return xs.compactMap { x in x ? x : nil }
}
"#;

const SWIFT_STANDARD_COMPACT_MAP_FUNCTION: &str = r#"
func f(_ xs: [Bool]) -> [Bool] {
    return xs.compactMap { value in value ? value : nil }
}
"#;

const SWIFT_CUSTOM_FILTER_MAP_CHAIN: &str = r#"
extension Array where Element == Bool {
    func filter(_ predicate: (Bool) -> Bool) -> [Bool] { [] }
    func map<T>(_ transform: (Bool) -> T) -> [T] { [] }
}
func f(_ xs: [Bool]) -> [Bool] {
    return xs.filter { value in value }.map { value in value }
}
"#;

#[test]
fn swift_compact_map_converges_for_exact_drop_and_emitted_value_coordinates() {
    let interner = Interner::new();
    let expected = value_fp(&interner, PY_FILTERED_IDENTITY, Lang::Python);
    assert!(
        expected.len() >= 4,
        "the compactMap equivalence must clear the exact semantic claim floor, got {expected:?}"
    );
    assert_eq!(
        expected,
        value_fp(&interner, SWIFT_COMPACT_MAP, Lang::Swift),
        "Swift compactMap should expose the same drop predicate and emitted value as a filtered comprehension"
    );
}

#[test]
fn swift_compact_map_executable_fixture_converges() {
    let fixture = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../bench/type4/adversarial/cases/swift_compact_map");
    let corpus = nose_frontend::lower_corpus_many(&[fixture.as_path()]);
    assert_eq!(
        corpus_value_fp(&corpus, "reference.py", "compactMapReference"),
        corpus_value_fp(&corpus, "compact_map.swift", "compactMapExact"),
        "the committed executable fixture must retain the modeled compactMap equivalence"
    );
}

#[test]
fn swift_compact_map_cross_file_closures_survive_repeated_normalization() {
    for fixture_name in [
        "swift_compact_map_custom_overload",
        "swift_compact_map_nil_conformance",
    ] {
        let fixture = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../../bench/type4/adversarial/cases")
            .join(fixture_name);
        let corpus = nose_frontend::lower_corpus_many(&[fixture.as_path()]);
        let raw = corpus
            .files
            .iter()
            .find(|il| il.meta.path.ends_with("consumer.swift"))
            .expect("cross-file compactMap consumer");
        let once = normalize(raw, &corpus.interner, &NormalizeOptions::default());
        let twice = normalize(&once, &corpus.interner, &NormalizeOptions::default());

        for (stage, il) in [("first", &once), ("second", &twice)] {
            assert!(
                !il.nodes.iter().any(|node| {
                    node.payload == nose_il::Payload::HoF(nose_il::HoFKind::FilterMap)
                }),
                "the {stage} normalization must not resurrect {fixture_name} compactMap evidence"
            );
        }
    }
}

#[test]
fn swift_compact_map_imported_nil_literal_namespace_stays_closed() {
    let interner = Interner::new();
    let source = r#"
import Foundation
func f(_ xs: [Bool]) -> [Bool] {
    return xs.compactMap { value in value ? value : nil }
}
"#;
    let raw = nose_frontend::lower_source(
        FileId(0),
        "imported.swift",
        source.as_bytes(),
        Lang::Swift,
        &interner,
    )
    .expect("lower imported Swift compactMap");
    let normalized = normalize(&raw, &interner, &NormalizeOptions::default());
    assert!(
        !normalized
            .nodes
            .iter()
            .any(|node| { node.payload == nose_il::Payload::HoF(nose_il::HoFKind::FilterMap) }),
        "an unresolved imported namespace can carry a hidden nil-literal conformance"
    );
}

#[test]
fn swift_compact_map_preserves_adjacent_option_emission_boundaries() {
    let interner = Interner::new();
    let expected = value_fp(&interner, PY_FILTERED_IDENTITY, Lang::Python);
    let changed_value = r#"
func f(_ xs: [Bool], _ other: Bool) -> [Bool] {
    return xs.compactMap { x in x ? other : nil }
}
"#;
    let changed_drop = r#"
func f(_ xs: [Bool], _ other: Bool) -> [Bool] {
    return xs.compactMap { x in other ? x : nil }
}
"#;
    let wrong_source = r#"
func f(_ xs: [Bool], _ other: [Bool]) -> [Bool] {
    return other.compactMap { x in x ? x : nil }
}
"#;
    let mapped_optional = r#"
func f(_ xs: [Bool]) -> [Bool?] {
    return xs.map { x in x ? x : nil }
}
"#;
    let effectful = r#"
func observe(_ value: Bool) {}
func f(_ xs: [Bool]) -> [Bool] {
    return xs.compactMap { x in
        observe(x)
        return x ? x : nil
    }
}
"#;
    let custom_receiver = r#"
struct Values {
    func compactMap(_ transform: (Bool) -> Bool?) -> [Bool] { return [] }
}
func f(_ xs: Values) -> [Bool] {
    return xs.compactMap { x in x ? x : nil }
}
"#;
    let same_file_overload = r#"
extension Array where Element == Bool {
    func `compactMap`(_ transform: (Bool) -> Bool?) -> [Bool] { return [] }
}
func f(_ xs: [Bool]) -> [Bool] {
    return xs.compactMap { x in x ? x : nil }
}
"#;
    let overloaded_condition = r#"
struct Flag {
    static prefix func !(_ value: Flag) -> Bool { print("effect"); return false }
}
func f(_ xs: [Flag]) -> [Flag] {
    return xs.compactMap { x in !x ? x : nil }
}
"#;
    let optional_emitted_value = r#"
func f(_ xs: [Bool], _ maybe: Bool?) -> [Bool] {
    return xs.compactMap { x in x ? maybe : nil }
}
"#;
    let custom_nil_literal = r#"
struct Nilish: ExpressibleByNilLiteral {
    let tag: Int
    init(nilLiteral: ()) { tag = 99 }
}
func f(_ xs: [Bool], _ value: Nilish) -> [Nilish] {
    return xs.compactMap { flag in flag ? value : nil }
}
"#;
    for (source, label) in [
        (changed_value, "changed emitted value"),
        (changed_drop, "changed drop condition"),
        (wrong_source, "wrong source"),
        (mapped_optional, "mapped optional channel"),
        (effectful, "effectful callback"),
        (custom_receiver, "custom receiver method"),
        (same_file_overload, "same-file compactMap overload"),
        (overloaded_condition, "overloaded drop-condition operator"),
        (optional_emitted_value, "Optional emitted Var"),
        (custom_nil_literal, "custom nil-literal emission channel"),
        (
            SWIFT_CUSTOM_NOMINAL_COLLECTION,
            "custom nominal Collection dynamic member",
        ),
        (
            SWIFT_CUSTOM_NOMINAL_ARRAY,
            "custom nominal Array dynamic member",
        ),
        (SWIFT_COMPACT_MAP_PROPERTY, "compactMap callable property"),
        (
            SWIFT_PROPERTY_WRAPPED_PARAMETER,
            "property-wrapped bracket-array parameter",
        ),
    ] {
        assert_ne!(
            expected,
            value_fp_named(&interner, source, Lang::Swift, "f"),
            "Swift compactMap boundary must stay split: {label}"
        );
    }
}

#[test]
fn swift_compact_map_rejects_parser_recovered_parameter_modifiers() {
    let interner = Interner::new();
    let expected = value_fp(&interner, PY_FILTERED_IDENTITY, Lang::Python);
    for source in [
        SWIFT_SENDING_PARAMETER,
        SWIFT_SHARED_PARAMETER,
        SWIFT_OWNED_PARAMETER,
    ] {
        assert_ne!(
            expected,
            value_fp_named(&interner, source, Lang::Swift, "f")
        );
    }
}

#[test]
fn swift_compact_map_rejects_optional_and_custom_nil_literal_false_merges() {
    let interner = Interner::new();
    let optional_compact_map = r#"
func f(_ xs: [Bool], _ maybe: Bool?) -> [Bool] {
    return xs.compactMap { x in x ? maybe : nil }
}
"#;
    let optional_filter_map = r#"
func f(_ xs: [Bool], _ maybe: Bool?) -> [Bool?] {
    return xs.filter { x in x }.map { _ in maybe }
}
"#;
    assert_ne!(
        value_fp_named(&interner, optional_compact_map, Lang::Swift, "f"),
        value_fp_named(&interner, optional_filter_map, Lang::Swift, "f"),
        "compactMap drops a nil Optional while map emits it as a payload"
    );

    let nilish_decl = r#"
struct Nilish: ExpressibleByNilLiteral {
    let tag: Int
    init(nilLiteral: ()) { tag = 99 }
}
"#;
    let custom_compact_map = format!(
        "{nilish_decl}\nfunc f(_ xs: [Bool], _ value: Nilish) -> [Nilish] {{\n    return xs.compactMap {{ flag in flag ? value : nil }}\n}}\n"
    );
    let custom_filter_map = format!(
        "{nilish_decl}\nfunc f(_ xs: [Bool], _ value: Nilish) -> [Nilish] {{\n    return xs.filter {{ flag in flag }}.map {{ _ in value }}\n}}\n"
    );
    assert_ne!(
        value_fp_named(&interner, &custom_compact_map, Lang::Swift, "f"),
        value_fp_named(&interner, &custom_filter_map, Lang::Swift, "f"),
        "a contextual nil can construct a present custom value rather than Optional absence"
    );

    let direct_retroactive_nil = r#"
extension Bool: @retroactive ExpressibleByNilLiteral {
    public init(nilLiteral: ()) { self = true }
}
func f(_ xs: [Bool]) -> [Bool] {
    return xs.compactMap { value in value ? value : nil }
}
func g(_ xs: [Bool]) -> [Bool] {
    return xs.filter { value in value }.map { value in value }
}
"#;
    let aliased_retroactive_nil = r#"
typealias NilProtocol = ExpressibleByNilLiteral
extension Bool: @retroactive NilProtocol {
    public init(nilLiteral: ()) { self = true }
}
func f(_ xs: [Bool]) -> [Bool] {
    return xs.compactMap { value in value ? value : nil }
}
func g(_ xs: [Bool]) -> [Bool] {
    return xs.filter { value in value }.map { value in value }
}
"#;
    for source in [direct_retroactive_nil, aliased_retroactive_nil] {
        assert_ne!(
            value_fp_named(&interner, source, Lang::Swift, "f"),
            value_fp_named(&interner, source, Lang::Swift, "g"),
            "retroactive Bool nil-literal conformance can turn the drop branch into true"
        );
    }
}

#[test]
fn swift_compact_map_keeps_source_losing_receivers_out_of_exact_families() {
    let cases = [
        (
            r#"func f(_ xs: [Bool], _ other: [Bool], _ flag: Bool) -> [Bool] {
    return xs.compactMap { value in flag ? flag : nil }
}
"#,
            r#"func f(_ xs: [Bool], _ other: [Bool], _ flag: Bool) -> [Bool] {
    return other.compactMap { value in flag ? flag : nil }
}
"#,
            "captured condition and emission",
        ),
        (
            r#"func f(_ xs: [Bool], _ other: [Bool], _ flag: Bool) -> [Bool] {
    return xs.map { value in flag }.compactMap { value in value ? value : nil }
}
"#,
            r#"func f(_ xs: [Bool], _ other: [Bool], _ flag: Bool) -> [Bool] {
    return other.map { value in flag }.compactMap { value in value ? value : nil }
}
"#,
            "nested source-losing HOF receiver",
        ),
        (
            r#"func f(_ xs: [Bool], _ other: [Bool], _ flag: Bool) -> [Bool] {
    let values: [Bool] = xs.map { value in flag }
    return values.compactMap { value in value ? value : nil }
}
"#,
            r#"func f(_ xs: [Bool], _ other: [Bool], _ flag: Bool) -> [Bool] {
    let values: [Bool] = other.map { value in flag }
    return values.compactMap { value in value ? value : nil }
}
"#,
            "local alias of a source-losing HOF",
        ),
        (
            SWIFT_STANDARD_COMPACT_MAP_FUNCTION,
            SWIFT_CUSTOM_FILTER_MAP_CHAIN,
            "custom filter/map dispatch on the same bracket-array receiver",
        ),
    ];

    for (index, (left, right, label)) in cases.into_iter().enumerate() {
        let (dir, corpus) = lower_temp_corpus(
            &format!("swift_compact_map_source_{index}"),
            &[("left.swift", left), ("right.swift", right)],
        );
        for il in &corpus.files {
            let normalized = normalize(il, &corpus.interner, &NormalizeOptions::default());
            assert!(
                !normalized.nodes.iter().any(|node| {
                    node.payload == nose_il::Payload::HoF(nose_il::HoFKind::FilterMap)
                }),
                "unmodeled compactMap must remain raw: {label}"
            );
        }
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
                    .any(|member| member.file.ends_with("left.swift"))
                    && group
                        .members
                        .iter()
                        .any(|member| member.file.ends_with("right.swift"))
            }),
            "unmodeled compactMap sources must not form an exact family: {label}"
        );
        std::fs::remove_dir_all(dir).unwrap();
    }
}
