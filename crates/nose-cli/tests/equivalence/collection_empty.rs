use super::*;

#[test]
fn swift_collection_empty_checks_converge_with_boundaries() {
    let i = Interner::new();
    let swift_count_empty =
        "func f(_ items: [Int], _ other: [Int]) -> Bool {\n    return items.count == 0\n}\n";
    let swift_named_empty =
        "func f(_ values: [Int], _ other: [Int]) -> Bool {\n    return values.isEmpty\n}\n";
    let java_named_empty =
        "import java.util.List;\nclass C { boolean f(List<Integer> values) { return values.isEmpty(); } }\n";
    let swift_count_nonempty =
        "func f(_ items: [Int], _ other: [Int]) -> Bool {\n    return items.count != 0\n}\n";
    let swift_named_nonempty =
        "func f(_ values: [Int], _ other: [Int]) -> Bool {\n    return !values.isEmpty\n}\n";
    let swift_threshold =
        "func f(_ items: [Int], _ other: [Int]) -> Bool {\n    return items.count == 1\n}\n";
    let swift_wrong_receiver =
        "func f(_ items: [Int], _ other: [Int]) -> Bool {\n    return other.isEmpty\n}\n";
    let swift_string_receiver =
        "func f(_ value: String, _ other: String) -> Bool {\n    return value.isEmpty\n}\n";
    let swift_custom_receiver = "struct EmptyBox {\n    var isEmpty: Bool { return false }\n}\n\nfunc f(_ value: EmptyBox, _ other: EmptyBox) -> Bool {\n    return value.isEmpty\n}\n";
    let swift_mutated_receiver =
        "func f(_ items: [Int], _ other: [Int]) -> Bool {\n    var current = items\n    current.append(1)\n    return current.isEmpty\n}\n";

    let empty_fp = value_fp(&i, swift_count_empty, Lang::Swift);
    assert_eq!(empty_fp, value_fp(&i, swift_named_empty, Lang::Swift));
    assert_eq!(empty_fp, value_fp(&i, java_named_empty, Lang::Java));

    let nonempty_fp = value_fp(&i, swift_count_nonempty, Lang::Swift);
    assert_eq!(nonempty_fp, value_fp(&i, swift_named_nonempty, Lang::Swift));
    assert_ne!(empty_fp, nonempty_fp);
    assert_ne!(empty_fp, value_fp(&i, swift_threshold, Lang::Swift));
    assert_ne!(empty_fp, value_fp(&i, swift_wrong_receiver, Lang::Swift));
    assert_ne!(empty_fp, value_fp(&i, swift_string_receiver, Lang::Swift));
    assert_ne!(
        empty_fp,
        value_fp_named(&i, swift_custom_receiver, Lang::Swift, "f")
    );
    assert_ne!(empty_fp, value_fp(&i, swift_mutated_receiver, Lang::Swift));
}
