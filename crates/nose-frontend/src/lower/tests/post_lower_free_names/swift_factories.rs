use super::*;

#[test]
fn post_lowering_emits_swift_collection_factory_occurrences() {
    let interner = Interner::new();
    let swift = lower_fixture(
        "collections.swift",
        br#"func f(values: [Int]) -> Bool {
  let a = Array(values)
  let s = Set(values)
  let d = Dictionary(uniqueKeysWithValues: [("a", 1)])
  return s.contains(1)
}
"#,
        Lang::Swift,
        &interner,
    );
    let array_contract = library_free_name_collection_factory_contract(Lang::Swift, "Array")
        .expect("Swift Array contract");
    let set_contract = library_free_name_collection_factory_contract(Lang::Swift, "Set")
        .expect("Swift Set contract");
    let dictionary_contract =
        library_swift_map_factory_contract(Lang::Swift, "Dictionary", "uniqueKeysWithValues")
            .expect("Swift Dictionary contract");
    for contract in [array_contract, set_contract] {
        assert_eq!(
            contract_api_count(&swift.evidence, contract.id, contract.callee),
            1
        );
        let records = contract_api_records(&swift.evidence, contract.id, contract.callee);
        assert_eq!(
            records[0].provenance.pack_hash,
            Some(stable_symbol_hash(contract.pack_id))
        );
        assert_eq!(
            records[0].provenance.rule_hash,
            Some(stable_symbol_hash(
                SWIFT_STDLIB_COLLECTION_FACTORY_PRODUCER_ID
            ))
        );
    }
    let dictionary_records = contract_api_records(
        &swift.evidence,
        dictionary_contract.id,
        dictionary_contract.callee,
    );
    assert_eq!(dictionary_records.len(), 1);
    assert_eq!(
        dictionary_records[0].provenance.pack_hash,
        Some(stable_symbol_hash(dictionary_contract.pack_id))
    );
    assert_eq!(
        dictionary_records[0].provenance.rule_hash,
        Some(stable_symbol_hash(
            SWIFT_STDLIB_COLLECTION_FACTORY_PRODUCER_ID
        ))
    );

    let shadowed_array = lower_fixture(
        "shadowed_array.swift",
        br#"struct Array {
  init(_ values: [Int]) {}
}
func f(values: [Int]) {
  _ = Array(values)
}
"#,
        Lang::Swift,
        &interner,
    );
    assert_eq!(
        contract_api_count(
            &shadowed_array.evidence,
            array_contract.id,
            array_contract.callee
        ),
        0,
        "local Swift Array type must shadow the stdlib factory"
    );

    let typealias_shadowed_array = lower_fixture(
        "typealias_shadowed_array.swift",
        br#"struct MyArray {
  init(_ values: [Int]) {}
}
typealias Array = MyArray
func f(values: [Int]) {
  _ = Array(values)
}
"#,
        Lang::Swift,
        &interner,
    );
    assert_eq!(
        contract_api_count(
            &typealias_shadowed_array.evidence,
            array_contract.id,
            array_contract.callee
        ),
        0,
        "Swift typealiases named Array must shadow the stdlib factory"
    );

    let typealias_shadowed_dictionary = lower_fixture(
        "typealias_shadowed_dictionary.swift",
        br#"struct MyDictionary {
  init(uniqueKeysWithValues values: [(String, Int)]) {}
}
typealias Dictionary = MyDictionary
func f() {
  _ = Dictionary(uniqueKeysWithValues: [("a", 1)])
}
"#,
        Lang::Swift,
        &interner,
    );
    assert_eq!(
        contract_api_count(
            &typealias_shadowed_dictionary.evidence,
            dictionary_contract.id,
            dictionary_contract.callee
        ),
        0,
        "Swift typealiases named Dictionary must shadow Dictionary(uniqueKeysWithValues:)"
    );

    let array_literal_label = lower_fixture(
        "array_literal_label.swift",
        br#"func f() {
  _ = Array(arrayLiteral: 1)
}
"#,
        Lang::Swift,
        &interner,
    );
    assert_eq!(
        contract_api_count(
            &array_literal_label.evidence,
            array_contract.id,
            array_contract.callee
        ),
        0,
        "labeled Array initializers are outside the Array(sequence) slice"
    );

    let wrong_dictionary_label = lower_fixture(
        "wrong_dictionary_label.swift",
        br#"func f(values: [Int]) {
  _ = Dictionary(grouping: values)
}
"#,
        Lang::Swift,
        &interner,
    );
    assert_eq!(
        contract_api_count(
            &wrong_dictionary_label.evidence,
            dictionary_contract.id,
            dictionary_contract.callee
        ),
        0,
        "only Dictionary(uniqueKeysWithValues:) is admitted in this slice"
    );
}
