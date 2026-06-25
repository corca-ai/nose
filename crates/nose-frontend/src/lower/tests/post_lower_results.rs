use super::*;

mod object_key_views;

#[test]
fn post_lowering_emits_result_domains_for_supported_factories() {
    let interner = Interner::new();
    assert_python_factory_result_domains(&interner);
    assert_rust_and_ruby_factory_result_domains(&interner);
    assert_swift_collection_factory_result_domains(&interner);
}

fn assert_python_factory_result_domains(interner: &Interner) {
    let py_list = lower_fixture(
        "builtin_list.py",
        b"def f(values):\n    return list(values)\n",
        Lang::Python,
        interner,
    );
    let list_contract =
        library_free_name_collection_factory_contract(Lang::Python, "list").unwrap();
    let list_api = contract_api_ids(&py_list.evidence, list_contract.id, list_contract.callee);
    assert!(result_domain_depends_on_any_api(
        &py_list.evidence,
        DomainEvidence::Collection,
        &list_api,
    ));

    let py_set = lower_fixture(
        "builtin_set.py",
        b"def f(values):\n    return set(values)\n",
        Lang::Python,
        interner,
    );
    let set_contract = library_free_name_collection_factory_contract(Lang::Python, "set").unwrap();
    let set_api = contract_api_ids(&py_set.evidence, set_contract.id, set_contract.callee);
    assert!(result_domain_depends_on_any_api(
        &py_set.evidence,
        DomainEvidence::Set,
        &set_api,
    ));

    let shadowed_py = lower_fixture(
        "shadowed.py",
        b"def f(list, values):\n    return list(values)\n",
        Lang::Python,
        interner,
    );
    assert_eq!(
        result_domain_record_count(&shadowed_py.evidence, DomainEvidence::Collection),
        0,
        "shadowed list(...) must not emit result-domain evidence"
    );
}

fn assert_rust_and_ruby_factory_result_domains(interner: &Interner) {
    let rust_vec = lower_fixture(
        "vec.rs",
        b"fn f() { let xs = Vec::new(); }",
        Lang::Rust,
        interner,
    );
    let vec_contract = library_rust_vec_new_factory_contract(Lang::Rust, "Vec::new").unwrap();
    let vec_api = contract_api_ids(&rust_vec.evidence, vec_contract.id, vec_contract.callee);
    assert!(result_domain_depends_on_any_api(
        &rust_vec.evidence,
        DomainEvidence::Collection,
        &vec_api,
    ));

    let qualified_rust_vec = lower_fixture(
        "qualified_vec.rs",
        b"fn f() { let xs = std::vec::Vec::new(); }",
        Lang::Rust,
        interner,
    );
    let qualified_vec_contract =
        library_rust_vec_new_factory_contract(Lang::Rust, "std::vec::Vec::new").unwrap();
    let qualified_vec_api = contract_api_ids(
        &qualified_rust_vec.evidence,
        qualified_vec_contract.id,
        qualified_vec_contract.callee,
    );
    assert!(result_domain_depends_on_any_api(
        &qualified_rust_vec.evidence,
        DomainEvidence::Collection,
        &qualified_vec_api,
    ));

    let rust_vec_alias_shadow = lower_fixture(
        "vec_type_alias_shadow.rs",
        b"struct Custom;\nimpl Custom { fn new() -> Self { Custom } }\ntype Vec = Custom;\nfn f() { let xs = Vec::new(); }",
        Lang::Rust,
        interner,
    );
    assert_eq!(
        result_domain_record_count(&rust_vec_alias_shadow.evidence, DomainEvidence::Collection),
        0,
        "Rust type aliases named Vec must not emit stdlib Vec result-domain evidence"
    );

    let rust_hashset = lower_fixture(
        "hashset.rs",
        b"fn f() { let xs = std::collections::HashSet::from([1, 2]); }",
        Lang::Rust,
        interner,
    );
    let hashset_contract = library_free_name_collection_factory_contract(
        Lang::Rust,
        "std::collections::HashSet::from",
    )
    .unwrap();
    let hashset_api = contract_api_ids(
        &rust_hashset.evidence,
        hashset_contract.id,
        hashset_contract.callee,
    );
    assert!(result_domain_depends_on_any_api(
        &rust_hashset.evidence,
        DomainEvidence::Set,
        &hashset_api,
    ));

    let rust_vecdeque = lower_fixture(
        "vecdeque.rs",
        b"fn f() { let xs = std::collections::VecDeque::from([1, 2]); }",
        Lang::Rust,
        interner,
    );
    let vecdeque_contract = library_free_name_collection_factory_contract(
        Lang::Rust,
        "std::collections::VecDeque::from",
    )
    .unwrap();
    let vecdeque_api = contract_api_ids(
        &rust_vecdeque.evidence,
        vecdeque_contract.id,
        vecdeque_contract.callee,
    );
    assert!(result_domain_depends_on_any_api(
        &rust_vecdeque.evidence,
        DomainEvidence::Collection,
        &vecdeque_api,
    ));

    let rust_shadowed_std = lower_fixture(
        "hashset_shadowed_std.rs",
        b"mod std { pub mod collections { pub struct HashSet; } }\nfn f() { let xs = std::collections::HashSet::from([1, 2]); }",
        Lang::Rust,
        interner,
    );
    assert_eq!(
        result_domain_record_count(&rust_shadowed_std.evidence, DomainEvidence::Set),
        0,
        "local std module must not emit stdlib HashSet result-domain evidence"
    );

    let rust_vec_macro = lower_fixture(
        "vec_macro.rs",
        b"fn f() { let xs = vec![1, 2]; }",
        Lang::Rust,
        interner,
    );
    let vec_macro_contract = library_rust_vec_macro_factory_contract(Lang::Rust, "vec").unwrap();
    let vec_macro_api = contract_api_ids(
        &rust_vec_macro.evidence,
        vec_macro_contract.id,
        vec_macro_contract.callee,
    );
    assert!(result_domain_depends_on_any_api(
        &rust_vec_macro.evidence,
        DomainEvidence::Collection,
        &vec_macro_api,
    ));

    let rust_map = lower_fixture(
        "hash_map.rs",
        b"fn f() { let xs = std::collections::HashMap::from([(\"red\", 1)]); }",
        Lang::Rust,
        interner,
    );
    let map_contract =
        library_free_name_map_factory_contract(Lang::Rust, "std::collections::HashMap::from")
            .unwrap();
    let map_api = contract_api_ids(&rust_map.evidence, map_contract.id, map_contract.callee);
    assert!(result_domain_depends_on_any_api(
        &rust_map.evidence,
        DomainEvidence::Map,
        &map_api,
    ));

    let rust_btreemap = lower_fixture(
        "btree_map.rs",
        b"fn f() { let xs = std::collections::BTreeMap::from([(\"red\", 1)]); }",
        Lang::Rust,
        interner,
    );
    let btreemap_contract =
        library_free_name_map_factory_contract(Lang::Rust, "std::collections::BTreeMap::from")
            .unwrap();
    let btreemap_api = contract_api_ids(
        &rust_btreemap.evidence,
        btreemap_contract.id,
        btreemap_contract.callee,
    );
    assert!(result_domain_depends_on_any_api(
        &rust_btreemap.evidence,
        DomainEvidence::Map,
        &btreemap_api,
    ));

    let rust_map_shadowed_std = lower_fixture(
        "hashmap_shadowed_std.rs",
        b"mod std { pub mod collections { pub struct HashMap; } }\nfn f() { let xs = std::collections::HashMap::from([(\"red\", 1)]); }",
        Lang::Rust,
        interner,
    );
    assert_eq!(
        result_domain_record_count(&rust_map_shadowed_std.evidence, DomainEvidence::Map),
        0,
        "local std module must not emit stdlib HashMap result-domain evidence"
    );

    let ruby = lower_fixture(
        "set.rb",
        b"require \"set\"\n\ndef f(values)\n  Set.new(values)\nend\n",
        Lang::Ruby,
        interner,
    );
    let ruby_contract = library_ruby_set_factory_contract(Lang::Ruby, "Set", "new", 1).unwrap();
    let ruby_api = contract_api_ids(&ruby.evidence, ruby_contract.id, ruby_contract.callee);
    assert!(result_domain_depends_on_any_api(
        &ruby.evidence,
        DomainEvidence::Set,
        &ruby_api,
    ));

    let missing_require = lower_fixture(
        "set_missing_require.rb",
        b"def f(values)\n  Set.new(values)\nend\n",
        Lang::Ruby,
        interner,
    );
    assert_eq!(
        result_domain_record_count(&missing_require.evidence, DomainEvidence::Set),
        0,
        "Ruby Set.new must not emit result-domain evidence without require proof"
    );
}

fn assert_swift_collection_factory_result_domains(interner: &Interner) {
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
        interner,
    );
    let array_contract = library_free_name_collection_factory_contract(Lang::Swift, "Array")
        .expect("Swift Array contract");
    let array_api = contract_api_ids(&swift.evidence, array_contract.id, array_contract.callee);
    assert!(result_domain_depends_on_any_api(
        &swift.evidence,
        DomainEvidence::Array,
        &array_api,
    ));
    let set_contract = library_free_name_collection_factory_contract(Lang::Swift, "Set")
        .expect("Swift Set contract");
    let set_api = contract_api_ids(&swift.evidence, set_contract.id, set_contract.callee);
    assert!(result_domain_depends_on_any_api(
        &swift.evidence,
        DomainEvidence::Set,
        &set_api,
    ));
    let dictionary_contract =
        library_swift_map_factory_contract(Lang::Swift, "Dictionary", "uniqueKeysWithValues")
            .expect("Swift Dictionary contract");
    let dictionary_api = contract_api_ids(
        &swift.evidence,
        dictionary_contract.id,
        dictionary_contract.callee,
    );
    assert!(result_domain_depends_on_any_api(
        &swift.evidence,
        DomainEvidence::Map,
        &dictionary_api,
    ));

    let array_literal_label = lower_fixture(
        "array_literal_label.swift",
        br#"func f() {
  _ = Array(arrayLiteral: 1)
}
"#,
        Lang::Swift,
        interner,
    );
    assert_eq!(
        result_domain_record_count(&array_literal_label.evidence, DomainEvidence::Array),
        0,
        "labeled Array initializers must not emit Array(sequence) result-domain evidence"
    );

    let typealias_shadows = lower_fixture(
        "swift_typealias_shadowed_factories.swift",
        br#"struct MyArray {
  init(_ values: [Int]) {}
}
struct MyDictionary {
  init(uniqueKeysWithValues values: [(String, Int)]) {}
}
typealias Array = MyArray
typealias Dictionary = MyDictionary
func f(values: [Int]) {
  _ = Array(values)
  _ = Dictionary(uniqueKeysWithValues: [("a", 1)])
}
"#,
        Lang::Swift,
        interner,
    );
    assert_eq!(
        result_domain_record_count(&typealias_shadows.evidence, DomainEvidence::Array),
        0,
        "Swift typealias shadows must not emit Array(sequence) result-domain evidence"
    );
    assert_eq!(
        result_domain_record_count(&typealias_shadows.evidence, DomainEvidence::Map),
        0,
        "Swift typealias shadows must not emit Dictionary map result-domain evidence"
    );

    let dictionary_implicit_entries = lower_fixture(
        "dictionary_implicit_entries.swift",
        br#"func f(values: [(String, Int)]) {
  _ = Dictionary(uniqueKeysWithValues: values)
}
"#,
        Lang::Swift,
        interner,
    );
    assert_eq!(
        result_domain_record_count(&dictionary_implicit_entries.evidence, DomainEvidence::Map),
        0,
        "Dictionary result-domain evidence requires explicit tuple-entry sequence proof"
    );

    let duplicate_dictionary = lower_fixture(
        "dictionary_duplicate_keys.swift",
        br#"func f() {
  _ = Dictionary(uniqueKeysWithValues: [("a", 1), ("a", 2)])
}
"#,
        Lang::Swift,
        interner,
    );
    assert_eq!(
        result_domain_record_count(&duplicate_dictionary.evidence, DomainEvidence::Map),
        0,
        "Dictionary result-domain evidence must stay closed for static duplicate keys"
    );

    let dictionary_wrong_label = lower_fixture(
        "dictionary_wrong_label.swift",
        br#"func f(values: [Int]) {
  _ = Dictionary(grouping: values)
}
"#,
        Lang::Swift,
        interner,
    );
    assert_eq!(
        result_domain_record_count(&dictionary_wrong_label.evidence, DomainEvidence::Map),
        0,
        "non-uniqueKeysWithValues Dictionary initializers must not emit Map result-domain evidence"
    );
}

#[test]
fn result_domain_evidence_requires_live_library_api_dependency() {
    let interner = Interner::new();
    let mut il = crate::lower_source(
        FileId(0),
        "set.js",
        b"function f(value) { return new Set([value]).has(value); }",
        Lang::JavaScript,
        &interner,
    )
    .expect("js lowering should succeed");
    let call = call_node_with_result_domain(&il, DomainEvidence::Set)
        .expect("new Set result should carry Set domain evidence");
    assert_eq!(
        nose_semantics::domain_evidence_for_node(&il, call),
        Some(DomainEvidence::Set)
    );

    for record in &mut il.evidence {
        if matches!(record.kind, EvidenceKind::LibraryApi(_)) {
            record.status = EvidenceStatus::Ambiguous;
        }
    }
    assert_eq!(
        nose_semantics::domain_evidence_for_node(&il, call),
        None,
        "receiver-domain proof must close when the LibraryApi dependency is ambiguous"
    );
}

#[test]
fn java_empty_collection_constructor_emits_occurrence_evidence() {
    let interner = Interner::new();
    let il = crate::lower_source(
        FileId(0),
        "C.java",
        b"import java.util.ArrayList;\nclass C { Object f() { return new ArrayList<>(); } }\n",
        Lang::Java,
        &interner,
    )
    .expect("java lowering should succeed");
    let contract =
        library_java_collection_constructor_contract(Lang::Java, "ArrayList", 0).unwrap();
    let api = library_api_evidence_ids_in_records(
        &il.evidence,
        library_api_contract_id_hash(contract.id),
        library_api_callee_contract_hash(contract.callee),
    );
    assert_eq!(api.len(), 1);
    let api_record = il.evidence_record_by_id(api[0]).expect("api record");
    assert_eq!(
        api_record.provenance.pack_hash,
        Some(stable_symbol_hash(
            JAVA_STDLIB_COLLECTION_CONSTRUCTOR_PACK_ID
        ))
    );
    assert_eq!(
        api_record.provenance.rule_hash,
        Some(stable_symbol_hash(
            JAVA_STDLIB_COLLECTION_CONSTRUCTOR_PRODUCER_ID
        ))
    );
    assert!(
        il.evidence.iter().any(|record| {
            record.kind == EvidenceKind::Domain(DomainEvidence::Collection)
                && record.dependencies.len() == 1
                && api.contains(&record.dependencies[0])
        }),
        "Java constructor result-domain evidence must depend on the LibraryApi occurrence"
    );
}

#[test]
fn java_empty_collection_constructor_wildcard_import_is_dependency_backed() {
    let interner = Interner::new();
    let wildcard = crate::lower_source(
        FileId(0),
        "C.java",
        b"import java.util.*;\nclass C { Object f() { return new ArrayList<>(); } }\n",
        Lang::Java,
        &interner,
    )
    .expect("java lowering should succeed");
    let contract =
        library_java_collection_constructor_contract(Lang::Java, "ArrayList", 0).unwrap();
    let api = wildcard
        .evidence
        .iter()
        .find(|record| {
            matches!(
                record.kind,
                EvidenceKind::LibraryApi(LibraryApiEvidenceKind::Contract {
                    contract_hash,
                    callee_hash,
                    ..
                }) if contract_hash == library_api_contract_id_hash(contract.id)
                    && callee_hash == library_api_callee_contract_hash(contract.callee)
            )
        })
        .expect("wildcard java.util import should admit supported ArrayList constructor");
    assert_eq!(
        api.provenance.pack_hash,
        Some(stable_symbol_hash(
            JAVA_STDLIB_COLLECTION_CONSTRUCTOR_PACK_ID
        ))
    );
    assert_eq!(
        api.provenance.rule_hash,
        Some(stable_symbol_hash(
            JAVA_STDLIB_COLLECTION_CONSTRUCTOR_PRODUCER_ID
        ))
    );
    assert!(api.dependencies.iter().any(|id| {
        wildcard.evidence_record_by_id(*id).is_some_and(|record| {
            matches!(
                record.kind,
                EvidenceKind::Import(ImportEvidenceKind::Wildcard { module_hash })
                    if module_hash == stable_symbol_hash("java.util")
            )
        })
    }));

    let shadowed = crate::lower_source(
            FileId(0),
            "C.java",
            b"import java.util.*;\nclass ArrayList<T> {}\nclass C { Object f() { return new ArrayList<>(); } }\n",
            Lang::Java,
            &interner,
        )
        .expect("java lowering should succeed");
    assert_eq!(
        library_api_evidence_count_in_records(
            &shadowed.evidence,
            library_api_contract_id_hash(contract.id),
            library_api_callee_contract_hash(contract.callee),
        ),
        0,
        "local ArrayList type must close the java.util constructor occurrence"
    );

    let explicit_conflict = crate::lower_source(
            FileId(0),
            "C.java",
            b"import java.util.*;\nimport other.ArrayList;\nclass C { Object f() { return new ArrayList<>(); } }\n",
            Lang::Java,
            &interner,
        )
        .expect("java lowering should succeed");
    assert_eq!(
        library_api_evidence_count_in_records(
            &explicit_conflict.evidence,
            library_api_contract_id_hash(contract.id),
            library_api_callee_contract_hash(contract.callee),
        ),
        0,
        "explicit same-name imports must close java.util wildcard constructor proof"
    );

    let exact_import_conflict = crate::lower_source(
            FileId(0),
            "C.java",
            b"import other.ArrayList;\nimport java.util.ArrayList;\nclass C { Object f() { return new ArrayList<>(); } }\n",
            Lang::Java,
            &interner,
        )
        .expect("java lowering should succeed");
    assert_eq!(
        library_api_evidence_count_in_records(
            &exact_import_conflict.evidence,
            library_api_contract_id_hash(contract.id),
            library_api_callee_contract_hash(contract.callee),
        ),
        0,
        "conflicting exact imports must close the java.util constructor occurrence"
    );
}
