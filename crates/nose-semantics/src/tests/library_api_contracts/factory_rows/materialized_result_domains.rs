use super::*;

#[test]
fn materialized_result_domain_mapping_keeps_unsafe_call_lanes_closed() {
    let as_list = library_java_collection_factory_contract(Lang::Java, "Arrays", "asList").unwrap();
    assert_eq!(
        library_api_materialized_result_domain_for_arity(as_list.id, as_list.callee, 1),
        None,
        "single-argument Arrays.asList has ambiguous element provenance"
    );
    assert_eq!(
        library_api_materialized_result_domain_for_arity(as_list.id, as_list.callee, 2),
        Some(DomainEvidence::Collection)
    );
    let empty_set =
        library_java_collection_factory_contract(Lang::Java, "Collections", "emptySet").unwrap();
    assert_eq!(
        library_api_materialized_result_domain_for_arity(empty_set.id, empty_set.callee, 1),
        None,
        "fixed zero-arity Collections.emptySet must not materialize a domain for wrong arity"
    );
    assert_eq!(
        library_api_materialized_result_domain_for_arity(empty_set.id, empty_set.callee, 0),
        Some(DomainEvidence::Set)
    );
    let singleton_list =
        library_java_collection_factory_contract(Lang::Java, "Collections", "singletonList")
            .unwrap();
    assert_eq!(
        library_api_materialized_result_domain_for_arity(
            singleton_list.id,
            singleton_list.callee,
            2
        ),
        None,
        "fixed single-element Collections.singletonList must not materialize a domain for wrong arity"
    );
    assert_eq!(
        library_api_materialized_result_domain_for_arity(
            singleton_list.id,
            singleton_list.callee,
            1
        ),
        Some(DomainEvidence::Collection)
    );
    let swift_array = library_free_name_collection_factory_contract(Lang::Swift, "Array").unwrap();
    assert_eq!(
        library_api_materialized_result_domain_for_arity(swift_array.id, swift_array.callee, 0),
        None,
        "Swift Array factory support is only for the one-argument sequence initializer"
    );
    assert_eq!(
        library_api_materialized_result_domain_for_arity(swift_array.id, swift_array.callee, 1),
        Some(DomainEvidence::Array)
    );
    let swift_set = library_free_name_collection_factory_contract(Lang::Swift, "Set").unwrap();
    assert_eq!(
        library_api_materialized_result_domain_for_arity(swift_set.id, swift_set.callee, 2),
        None,
        "Swift Set factory support is only for the one-argument sequence initializer"
    );
    assert_eq!(
        library_api_materialized_result_domain_for_arity(swift_set.id, swift_set.callee, 1),
        Some(DomainEvidence::Set)
    );
    let empty_map =
        library_java_map_factory_contract(Lang::Java, "Collections", "emptyMap").unwrap();
    assert_eq!(
        library_api_materialized_result_domain_for_arity(empty_map.id, empty_map.callee, 1),
        None,
        "fixed zero-arity Collections.emptyMap must not materialize a domain for wrong arity"
    );
    assert_eq!(
        library_api_materialized_result_domain_for_arity(empty_map.id, empty_map.callee, 0),
        Some(DomainEvidence::Map)
    );
    let singleton_map =
        library_java_map_factory_contract(Lang::Java, "Collections", "singletonMap").unwrap();
    assert_eq!(
        library_api_materialized_result_domain_for_arity(singleton_map.id, singleton_map.callee, 1),
        None,
        "fixed key/value Collections.singletonMap must not materialize a domain for wrong arity"
    );
    assert_eq!(
        library_api_materialized_result_domain_for_arity(singleton_map.id, singleton_map.callee, 2),
        Some(DomainEvidence::Map)
    );
    let swift_dictionary =
        library_swift_map_factory_contract(Lang::Swift, "Dictionary", "uniqueKeysWithValues")
            .unwrap();
    assert_eq!(
        library_api_materialized_result_domain_for_arity(
            swift_dictionary.id,
            swift_dictionary.callee,
            0
        ),
        None,
        "Swift Dictionary(uniqueKeysWithValues:) support is fixed to one labeled argument"
    );
    assert_eq!(
        library_api_materialized_result_domain_for_arity(
            swift_dictionary.id,
            swift_dictionary.callee,
            1
        ),
        None,
        "Swift Dictionary(uniqueKeysWithValues:) needs tuple-entry proof before result-domain emission"
    );

    let hof = library_method_call_contract(Lang::JavaScript, "map", 1).unwrap();
    assert_eq!(
        library_api_materialized_result_domain_for_arity(hof.id, hof.callee, 1),
        None,
        "HOF compatibility fallback must not become emitted result-domain evidence"
    );

    let map_get = library_map_get_contract(Lang::Rust, "get", 1).unwrap();
    assert_eq!(
        library_api_materialized_result_domain_for_arity(map_get.id, map_get.callee, 1),
        None,
        "Map.get value semantics are not a fixed container result domain"
    );

    let rust_iter = library_iterator_identity_adapter_contract(Lang::Rust, "iter", 0).unwrap();
    assert_eq!(
        library_api_materialized_result_domain_for_arity(rust_iter.id, rust_iter.callee, 0),
        Some(DomainEvidence::Iterator)
    );
    assert_eq!(
        library_api_materialized_result_domain_for_arity(rust_iter.id, rust_iter.callee, 1),
        None,
        "iterator adapter result-domain evidence must stay arity-checked"
    );
    let rust_to_vec = library_iterator_identity_adapter_contract(Lang::Rust, "to_vec", 0).unwrap();
    assert_eq!(
        library_api_materialized_result_domain_for_arity(rust_to_vec.id, rust_to_vec.callee, 0),
        Some(DomainEvidence::Collection)
    );
    let rust_collect =
        library_iterator_identity_adapter_contract(Lang::Rust, "collect", 0).unwrap();
    assert_eq!(
        library_api_materialized_result_domain_for_arity(rust_collect.id, rust_collect.callee, 0),
        None,
        "collect result type is caller-selected and must not emit a fixed result domain"
    );

    let guava_map = library_java_map_factory_contract(Lang::Java, "ImmutableMap", "of").unwrap();
    assert_eq!(
        library_api_materialized_result_domain_for_arity(guava_map.id, guava_map.callee, 20),
        Some(DomainEvidence::Map)
    );
    assert_eq!(
        library_api_materialized_result_domain_for_arity(guava_map.id, guava_map.callee, 21),
        None,
        "odd ImmutableMap.of arity cannot be a Guava overload"
    );
    assert_eq!(
        library_api_materialized_result_domain_for_arity(guava_map.id, guava_map.callee, 22),
        None,
        "Guava ImmutableMap.of has fixed overloads through ten entries"
    );
}
