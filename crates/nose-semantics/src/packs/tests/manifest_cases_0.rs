use super::*;

#[test]
fn builtin_compat_pack_hash_matches_evidence_provenance_hash_policy() {
    let pack = builtin_compat_semantic_pack();
    assert_eq!(pack.id, BUILTIN_COMPAT_PACK_ID);
    assert_eq!(pack.hash, stable_symbol_hash(BUILTIN_COMPAT_PACK_ID));
    assert_eq!(pack.influence, SemanticPackInfluence::EvidenceAndContracts);
    let set = SemanticPackSet::builtin_only();
    let c = set
        .packs()
        .iter()
        .find(|pack| pack.id == C_LANGUAGE_PACK_ID)
        .expect("C summary");
    assert_eq!(c.id, C_LANGUAGE_PACK_ID);
    assert_eq!(c.hash, stable_symbol_hash(C_LANGUAGE_PACK_ID));
    assert_eq!(c.kind, SemanticPackKind::LanguagePack);
    assert_eq!(c.counts.evidence_producers, 3);
    let python_builtins = set
        .packs()
        .iter()
        .find(|pack| pack.id == PYTHON_BUILTIN_COLLECTION_FACTORY_PACK_ID)
        .expect("Python builtins summary");
    assert_eq!(
        python_builtins.id,
        PYTHON_BUILTIN_COLLECTION_FACTORY_PACK_ID
    );
    assert_eq!(
        python_builtins.hash,
        stable_symbol_hash(PYTHON_BUILTIN_COLLECTION_FACTORY_PACK_ID)
    );
    assert_eq!(python_builtins.kind, SemanticPackKind::StdlibPack);
    assert_eq!(
        python_builtins.influence,
        SemanticPackInfluence::EvidenceAndContracts
    );
    assert_eq!(python_builtins.counts.evidence_producers, 1);
    assert_eq!(python_builtins.counts.contracts, 1);
    assert_eq!(python_builtins.counts.positive_fixtures, 4);
    assert_eq!(python_builtins.counts.hard_negatives, 2);
    let python_stdlib_collections = set
        .packs()
        .iter()
        .find(|pack| pack.id == PYTHON_STDLIB_COLLECTION_FACTORY_PACK_ID)
        .expect("Python stdlib collections summary");
    assert_eq!(
        python_stdlib_collections.hash,
        stable_symbol_hash(PYTHON_STDLIB_COLLECTION_FACTORY_PACK_ID)
    );
    assert_eq!(python_stdlib_collections.kind, SemanticPackKind::StdlibPack);
    assert_eq!(
        python_stdlib_collections.influence,
        SemanticPackInfluence::EvidenceAndContracts
    );
    assert_eq!(python_stdlib_collections.counts.evidence_producers, 1);
    assert_eq!(python_stdlib_collections.counts.contracts, 1);
    assert_eq!(python_stdlib_collections.counts.positive_fixtures, 3);
    assert_eq!(python_stdlib_collections.counts.hard_negatives, 2);
    let ruby_set = set
        .packs()
        .iter()
        .find(|pack| pack.id == RUBY_STDLIB_SET_PACK_ID)
        .expect("Ruby stdlib Set summary");
    assert_eq!(ruby_set.hash, stable_symbol_hash(RUBY_STDLIB_SET_PACK_ID));
    assert_eq!(ruby_set.kind, SemanticPackKind::StdlibPack);
    assert_eq!(
        ruby_set.influence,
        SemanticPackInfluence::EvidenceAndContracts
    );
    assert_eq!(ruby_set.counts.evidence_producers, 1);
    assert_eq!(ruby_set.counts.contracts, 1);
    assert_eq!(ruby_set.counts.positive_fixtures, 3);
    assert_eq!(ruby_set.counts.hard_negatives, 3);
    let rust_vec = set
        .packs()
        .iter()
        .find(|pack| pack.id == RUST_STDLIB_VEC_PACK_ID)
        .expect("Rust stdlib Vec summary");
    assert_eq!(rust_vec.hash, stable_symbol_hash(RUST_STDLIB_VEC_PACK_ID));
    assert_eq!(rust_vec.kind, SemanticPackKind::StdlibPack);
    assert_eq!(
        rust_vec.influence,
        SemanticPackInfluence::EvidenceAndContracts
    );
    assert_eq!(rust_vec.counts.evidence_producers, 1);
    assert_eq!(rust_vec.counts.contracts, 2);
    assert_eq!(rust_vec.counts.positive_fixtures, 2);
    assert_eq!(rust_vec.counts.hard_negatives, 2);
    let rust_option = set
        .packs()
        .iter()
        .find(|pack| pack.id == RUST_STDLIB_OPTION_PACK_ID)
        .expect("Rust stdlib Option summary");
    assert_eq!(
        rust_option.hash,
        stable_symbol_hash(RUST_STDLIB_OPTION_PACK_ID)
    );
    assert_eq!(rust_option.kind, SemanticPackKind::StdlibPack);
    assert_eq!(
        rust_option.influence,
        SemanticPackInfluence::EvidenceAndContracts
    );
    assert_eq!(rust_option.counts.evidence_producers, 1);
    assert_eq!(rust_option.counts.contracts, 3);
    assert_eq!(rust_option.counts.positive_fixtures, 3);
    assert_eq!(rust_option.counts.hard_negatives, 3);
    let rust_integer_methods = set
        .packs()
        .iter()
        .find(|pack| pack.id == RUST_STDLIB_INTEGER_METHOD_PACK_ID)
        .expect("Rust stdlib integer method summary");
    assert_eq!(
        rust_integer_methods.hash,
        stable_symbol_hash(RUST_STDLIB_INTEGER_METHOD_PACK_ID)
    );
    assert_eq!(rust_integer_methods.kind, SemanticPackKind::StdlibPack);
    assert_eq!(
        rust_integer_methods.influence,
        SemanticPackInfluence::EvidenceAndContracts
    );
    assert_eq!(rust_integer_methods.counts.evidence_producers, 1);
    assert_eq!(rust_integer_methods.counts.contracts, 4);
    assert_eq!(rust_integer_methods.counts.positive_fixtures, 4);
    assert_eq!(rust_integer_methods.counts.hard_negatives, 2);
    let rust_stdlib_collections = set
        .packs()
        .iter()
        .find(|pack| pack.id == RUST_STDLIB_COLLECTION_FACTORY_PACK_ID)
        .expect("Rust stdlib collection factory summary");
    assert_eq!(
        rust_stdlib_collections.hash,
        stable_symbol_hash(RUST_STDLIB_COLLECTION_FACTORY_PACK_ID)
    );
    assert_eq!(rust_stdlib_collections.kind, SemanticPackKind::StdlibPack);
    assert_eq!(
        rust_stdlib_collections.influence,
        SemanticPackInfluence::EvidenceAndContracts
    );
    assert_eq!(rust_stdlib_collections.counts.evidence_producers, 1);
    assert_eq!(rust_stdlib_collections.counts.contracts, 1);
    assert_eq!(rust_stdlib_collections.counts.positive_fixtures, 3);
    assert_eq!(rust_stdlib_collections.counts.hard_negatives, 2);
    let rust_stdlib_maps = set
        .packs()
        .iter()
        .find(|pack| pack.id == RUST_STDLIB_MAP_FACTORY_PACK_ID)
        .expect("Rust stdlib map factory summary");
    assert_eq!(
        rust_stdlib_maps.hash,
        stable_symbol_hash(RUST_STDLIB_MAP_FACTORY_PACK_ID)
    );
    assert_eq!(rust_stdlib_maps.kind, SemanticPackKind::StdlibPack);
    assert_eq!(
        rust_stdlib_maps.influence,
        SemanticPackInfluence::EvidenceAndContracts
    );
    assert_eq!(rust_stdlib_maps.counts.evidence_producers, 1);
    assert_eq!(rust_stdlib_maps.counts.contracts, 1);
    assert_eq!(rust_stdlib_maps.counts.positive_fixtures, 2);
    assert_eq!(rust_stdlib_maps.counts.hard_negatives, 2);
    let swift_stdlib_collections = set
        .packs()
        .iter()
        .find(|pack| pack.id == SWIFT_STDLIB_COLLECTION_FACTORY_PACK_ID)
        .expect("Swift stdlib collection factory summary");
    assert_eq!(
        swift_stdlib_collections.hash,
        stable_symbol_hash(SWIFT_STDLIB_COLLECTION_FACTORY_PACK_ID)
    );
    assert_eq!(swift_stdlib_collections.kind, SemanticPackKind::StdlibPack);
    assert_eq!(
        swift_stdlib_collections.influence,
        SemanticPackInfluence::EvidenceAndContracts
    );
    assert_eq!(swift_stdlib_collections.counts.evidence_producers, 1);
    assert_eq!(swift_stdlib_collections.counts.contracts, 3);
    assert_eq!(swift_stdlib_collections.counts.positive_fixtures, 3);
    assert_eq!(swift_stdlib_collections.counts.hard_negatives, 4);
    let java_stdlib_math = set
        .packs()
        .iter()
        .find(|pack| pack.id == JAVA_STDLIB_MATH_PACK_ID)
        .expect("Java stdlib Math summary");
    assert_eq!(
        java_stdlib_math.hash,
        stable_symbol_hash(JAVA_STDLIB_MATH_PACK_ID)
    );
    assert_eq!(java_stdlib_math.kind, SemanticPackKind::StdlibPack);
    assert_eq!(
        java_stdlib_math.influence,
        SemanticPackInfluence::EvidenceAndContracts
    );
    assert_eq!(java_stdlib_math.counts.evidence_producers, 1);
    assert_eq!(java_stdlib_math.counts.contracts, 3);
    assert_eq!(java_stdlib_math.counts.positive_fixtures, 3);
    assert_eq!(java_stdlib_math.counts.hard_negatives, 3);
    let map_get = set
        .packs()
        .iter()
        .find(|pack| pack.id == MAP_GET_PROTOCOL_PACK_ID)
        .expect("map-get protocol summary");
    assert_eq!(map_get.hash, stable_symbol_hash(MAP_GET_PROTOCOL_PACK_ID));
    assert_eq!(map_get.kind, SemanticPackKind::ProtocolPack);
    assert_eq!(
        map_get.influence,
        SemanticPackInfluence::EvidenceAndContracts
    );
    assert_eq!(map_get.counts.evidence_producers, 1);
    assert_eq!(map_get.counts.contracts, 1);
    assert_eq!(map_get.counts.positive_fixtures, 3);
    assert_eq!(map_get.counts.hard_negatives, 2);
    let map_get_default = set
        .packs()
        .iter()
        .find(|pack| pack.id == MAP_GET_DEFAULT_PROTOCOL_PACK_ID)
        .expect("map-get-default protocol summary");
    assert_eq!(
        map_get_default.hash,
        stable_symbol_hash(MAP_GET_DEFAULT_PROTOCOL_PACK_ID)
    );
    assert_eq!(map_get_default.kind, SemanticPackKind::ProtocolPack);
    assert_eq!(
        map_get_default.influence,
        SemanticPackInfluence::EvidenceAndContracts
    );
    assert_eq!(map_get_default.counts.evidence_producers, 1);
    assert_eq!(map_get_default.counts.contracts, 1);
    assert_eq!(map_get_default.counts.positive_fixtures, 3);
    assert_eq!(map_get_default.counts.hard_negatives, 2);
    let free_function_builtin = set
        .packs()
        .iter()
        .find(|pack| pack.id == FREE_FUNCTION_BUILTIN_PROTOCOL_PACK_ID)
        .expect("free-function builtin protocol summary");
    assert_eq!(
        free_function_builtin.hash,
        stable_symbol_hash(FREE_FUNCTION_BUILTIN_PROTOCOL_PACK_ID)
    );
    assert_eq!(free_function_builtin.kind, SemanticPackKind::ProtocolPack);
    assert_eq!(
        free_function_builtin.influence,
        SemanticPackInfluence::EvidenceAndContracts
    );
    assert_eq!(free_function_builtin.counts.evidence_producers, 1);
    assert_eq!(free_function_builtin.counts.contracts, 1);
    assert_eq!(free_function_builtin.counts.positive_fixtures, 6);
    assert_eq!(free_function_builtin.counts.hard_negatives, 4);
    let receiver_membership = set
        .packs()
        .iter()
        .find(|pack| pack.id == RECEIVER_MEMBERSHIP_PROTOCOL_PACK_ID)
        .expect("receiver-membership protocol summary");
    assert_eq!(
        receiver_membership.hash,
        stable_symbol_hash(RECEIVER_MEMBERSHIP_PROTOCOL_PACK_ID)
    );
    assert_eq!(receiver_membership.kind, SemanticPackKind::ProtocolPack);
    assert_eq!(
        receiver_membership.influence,
        SemanticPackInfluence::EvidenceAndContracts
    );
    assert_eq!(receiver_membership.counts.evidence_producers, 1);
    assert_eq!(receiver_membership.counts.contracts, 1);
    assert_eq!(receiver_membership.counts.positive_fixtures, 10);
    assert_eq!(receiver_membership.counts.hard_negatives, 3);
    let map_key_view = set
        .packs()
        .iter()
        .find(|pack| pack.id == MAP_KEY_VIEW_PROTOCOL_PACK_ID)
        .expect("map-key-view protocol summary");
    assert_eq!(
        map_key_view.hash,
        stable_symbol_hash(MAP_KEY_VIEW_PROTOCOL_PACK_ID)
    );
    assert_eq!(map_key_view.kind, SemanticPackKind::ProtocolPack);
    assert_eq!(
        map_key_view.influence,
        SemanticPackInfluence::EvidenceAndContracts
    );
    assert_eq!(map_key_view.counts.evidence_producers, 1);
    assert_eq!(map_key_view.counts.contracts, 2);
    assert_eq!(map_key_view.counts.positive_fixtures, 4);
    assert_eq!(map_key_view.counts.hard_negatives, 2);
    let property_builtin = set
        .packs()
        .iter()
        .find(|pack| pack.id == PROPERTY_BUILTIN_PROTOCOL_PACK_ID)
        .expect("property-builtin protocol summary");
    assert_eq!(
        property_builtin.hash,
        stable_symbol_hash(PROPERTY_BUILTIN_PROTOCOL_PACK_ID)
    );
    assert_eq!(property_builtin.kind, SemanticPackKind::ProtocolPack);
    assert_eq!(
        property_builtin.influence,
        SemanticPackInfluence::EvidenceAndContracts
    );
    assert_eq!(property_builtin.counts.evidence_producers, 1);
    assert_eq!(property_builtin.counts.contracts, 2);
    assert_eq!(property_builtin.counts.positive_fixtures, 4);
    assert_eq!(property_builtin.counts.hard_negatives, 3);
    let java_stdlib_maps = set
        .packs()
        .iter()
        .find(|pack| pack.id == JAVA_STDLIB_MAP_FACTORY_PACK_ID)
        .expect("Java stdlib map factory summary");
    assert_eq!(
        java_stdlib_maps.hash,
        stable_symbol_hash(JAVA_STDLIB_MAP_FACTORY_PACK_ID)
    );
    assert_eq!(java_stdlib_maps.kind, SemanticPackKind::StdlibPack);
    assert_eq!(
        java_stdlib_maps.influence,
        SemanticPackInfluence::EvidenceAndContracts
    );
    assert_eq!(java_stdlib_maps.counts.evidence_producers, 1);
    assert_eq!(java_stdlib_maps.counts.contracts, 4);
    assert_eq!(java_stdlib_maps.counts.positive_fixtures, 4);
    assert_eq!(java_stdlib_maps.counts.hard_negatives, 4);
    let java_stdlib_entries = set
        .packs()
        .iter()
        .find(|pack| pack.id == JAVA_STDLIB_MAP_ENTRY_PACK_ID)
        .expect("Java stdlib map entry summary");
    assert_eq!(
        java_stdlib_entries.hash,
        stable_symbol_hash(JAVA_STDLIB_MAP_ENTRY_PACK_ID)
    );
    assert_eq!(java_stdlib_entries.kind, SemanticPackKind::StdlibPack);
    assert_eq!(
        java_stdlib_entries.influence,
        SemanticPackInfluence::EvidenceAndContracts
    );
    assert_eq!(java_stdlib_entries.counts.evidence_producers, 1);
    assert_eq!(java_stdlib_entries.counts.contracts, 1);
    assert_eq!(java_stdlib_entries.counts.positive_fixtures, 1);
    assert_eq!(java_stdlib_entries.counts.hard_negatives, 2);
    let java_stdlib_collections = set
        .packs()
        .iter()
        .find(|pack| pack.id == JAVA_STDLIB_COLLECTION_FACTORY_PACK_ID)
        .expect("Java stdlib collection factory summary");
    assert_eq!(
        java_stdlib_collections.hash,
        stable_symbol_hash(JAVA_STDLIB_COLLECTION_FACTORY_PACK_ID)
    );
    assert_eq!(java_stdlib_collections.kind, SemanticPackKind::StdlibPack);
    assert_eq!(
        java_stdlib_collections.influence,
        SemanticPackInfluence::EvidenceAndContracts
    );
    assert_eq!(java_stdlib_collections.counts.evidence_producers, 1);
    assert_eq!(java_stdlib_collections.counts.contracts, 7);
    assert_eq!(java_stdlib_collections.counts.positive_fixtures, 7);
    assert_eq!(java_stdlib_collections.counts.hard_negatives, 5);
    let java_stdlib_constructors = set
        .packs()
        .iter()
        .find(|pack| pack.id == JAVA_STDLIB_COLLECTION_CONSTRUCTOR_PACK_ID)
        .expect("Java stdlib collection constructor summary");
    assert_eq!(
        java_stdlib_constructors.hash,
        stable_symbol_hash(JAVA_STDLIB_COLLECTION_CONSTRUCTOR_PACK_ID)
    );
    assert_eq!(java_stdlib_constructors.kind, SemanticPackKind::StdlibPack);
    assert_eq!(
        java_stdlib_constructors.influence,
        SemanticPackInfluence::EvidenceAndContracts
    );
    assert_eq!(java_stdlib_constructors.counts.evidence_producers, 1);
    assert_eq!(java_stdlib_constructors.counts.contracts, 1);
    assert_eq!(java_stdlib_constructors.counts.positive_fixtures, 2);
    assert_eq!(java_stdlib_constructors.counts.hard_negatives, 3);
    let java_static_adapters = set
        .packs()
        .iter()
        .find(|pack| pack.id == JAVA_STDLIB_STATIC_COLLECTION_ADAPTER_PACK_ID)
        .expect("Java stdlib static collection adapter summary");
    assert_eq!(
        java_static_adapters.hash,
        stable_symbol_hash(JAVA_STDLIB_STATIC_COLLECTION_ADAPTER_PACK_ID)
    );
    assert_eq!(java_static_adapters.kind, SemanticPackKind::StdlibPack);
    assert_eq!(
        java_static_adapters.influence,
        SemanticPackInfluence::EvidenceAndContracts
    );
    assert_eq!(java_static_adapters.counts.evidence_producers, 1);
    assert_eq!(java_static_adapters.counts.contracts, 1);
    assert_eq!(java_static_adapters.counts.positive_fixtures, 1);
    assert_eq!(java_static_adapters.counts.hard_negatives, 2);
    let iterator_identity_adapters = set
        .packs()
        .iter()
        .find(|pack| pack.id == ITERATOR_IDENTITY_ADAPTER_PACK_ID)
        .expect("iterator identity adapter protocol summary");
    assert_eq!(
        iterator_identity_adapters.hash,
        stable_symbol_hash(ITERATOR_IDENTITY_ADAPTER_PACK_ID)
    );
    assert_eq!(
        iterator_identity_adapters.kind,
        SemanticPackKind::ProtocolPack
    );
    assert_eq!(
        iterator_identity_adapters.influence,
        SemanticPackInfluence::EvidenceAndContracts
    );
    assert_eq!(iterator_identity_adapters.counts.evidence_producers, 1);
    assert_eq!(iterator_identity_adapters.counts.contracts, 1);
    assert_eq!(iterator_identity_adapters.counts.positive_fixtures, 3);
    assert_eq!(iterator_identity_adapters.counts.hard_negatives, 2);
    let js_promise = set
        .packs()
        .iter()
        .find(|pack| pack.id == JS_LIKE_BUILTIN_PROMISE_PACK_ID)
        .expect("JavaScript builtins Promise summary");
    assert_eq!(
        js_promise.hash,
        stable_symbol_hash(JS_LIKE_BUILTIN_PROMISE_PACK_ID)
    );
    assert_eq!(js_promise.kind, SemanticPackKind::StdlibPack);
    assert_eq!(
        js_promise.influence,
        SemanticPackInfluence::EvidenceAndContracts
    );
    assert_eq!(js_promise.counts.evidence_producers, 1);
    assert_eq!(js_promise.counts.contracts, 2);
    assert_eq!(js_promise.counts.positive_fixtures, 2);
    assert_eq!(js_promise.counts.hard_negatives, 3);
    let js_array = set
        .packs()
        .iter()
        .find(|pack| pack.id == JS_LIKE_BUILTIN_ARRAY_PACK_ID)
        .expect("JavaScript builtins Array summary");
    assert_eq!(
        js_array.hash,
        stable_symbol_hash(JS_LIKE_BUILTIN_ARRAY_PACK_ID)
    );
    assert_eq!(js_array.kind, SemanticPackKind::StdlibPack);
    assert_eq!(
        js_array.influence,
        SemanticPackInfluence::EvidenceAndContracts
    );
    assert_eq!(js_array.counts.evidence_producers, 1);
    assert_eq!(js_array.counts.contracts, 2);
    assert_eq!(js_array.counts.positive_fixtures, 2);
    assert_eq!(js_array.counts.hard_negatives, 3);
    let js_boolean = set
        .packs()
        .iter()
        .find(|pack| pack.id == JS_LIKE_BUILTIN_BOOLEAN_PACK_ID)
        .expect("JavaScript builtins Boolean summary");
    assert_eq!(
        js_boolean.hash,
        stable_symbol_hash(JS_LIKE_BUILTIN_BOOLEAN_PACK_ID)
    );
    assert_eq!(js_boolean.kind, SemanticPackKind::StdlibPack);
    assert_eq!(
        js_boolean.influence,
        SemanticPackInfluence::EvidenceAndContracts
    );
    assert_eq!(js_boolean.counts.evidence_producers, 1);
    assert_eq!(js_boolean.counts.contracts, 1);
    assert_eq!(js_boolean.counts.positive_fixtures, 1);
    assert_eq!(js_boolean.counts.hard_negatives, 2);
    let js_regex = set
        .packs()
        .iter()
        .find(|pack| pack.id == JS_LIKE_BUILTIN_REGEX_PACK_ID)
        .expect("JavaScript builtins RegExp summary");
    assert_eq!(
        js_regex.hash,
        stable_symbol_hash(JS_LIKE_BUILTIN_REGEX_PACK_ID)
    );
    assert_eq!(js_regex.kind, SemanticPackKind::StdlibPack);
    assert_eq!(
        js_regex.influence,
        SemanticPackInfluence::EvidenceAndContracts
    );
    assert_eq!(js_regex.counts.evidence_producers, 1);
    assert_eq!(js_regex.counts.contracts, 1);
    assert_eq!(js_regex.counts.positive_fixtures, 1);
    assert_eq!(js_regex.counts.hard_negatives, 2);
    let js_static_index = set
        .packs()
        .iter()
        .find(|pack| pack.id == JS_LIKE_BUILTIN_STATIC_INDEX_MEMBERSHIP_PACK_ID)
        .expect("JavaScript builtins static index membership summary");
    assert_eq!(
        js_static_index.hash,
        stable_symbol_hash(JS_LIKE_BUILTIN_STATIC_INDEX_MEMBERSHIP_PACK_ID)
    );
    assert_eq!(js_static_index.kind, SemanticPackKind::StdlibPack);
    assert_eq!(
        js_static_index.influence,
        SemanticPackInfluence::EvidenceAndContracts
    );
    assert_eq!(js_static_index.counts.evidence_producers, 1);
    assert_eq!(js_static_index.counts.contracts, 2);
    assert_eq!(js_static_index.counts.positive_fixtures, 2);
    assert_eq!(js_static_index.counts.hard_negatives, 2);
    let js_collection_constructors = set
        .packs()
        .iter()
        .find(|pack| pack.id == JS_LIKE_BUILTIN_COLLECTION_CONSTRUCTOR_PACK_ID)
        .expect("JavaScript builtins collection constructor summary");
    assert_eq!(
        js_collection_constructors.hash,
        stable_symbol_hash(JS_LIKE_BUILTIN_COLLECTION_CONSTRUCTOR_PACK_ID)
    );
    assert_eq!(
        js_collection_constructors.kind,
        SemanticPackKind::StdlibPack
    );
    assert_eq!(
        js_collection_constructors.influence,
        SemanticPackInfluence::EvidenceAndContracts
    );
    assert_eq!(js_collection_constructors.counts.evidence_producers, 1);
    assert_eq!(js_collection_constructors.counts.contracts, 2);
    assert_eq!(js_collection_constructors.counts.positive_fixtures, 2);
    assert_eq!(js_collection_constructors.counts.hard_negatives, 3);
    let python_stdlib_math = set
        .packs()
        .iter()
        .find(|pack| pack.id == PYTHON_STDLIB_MATH_PACK_ID)
        .expect("Python stdlib math summary");
    assert_eq!(
        python_stdlib_math.hash,
        stable_symbol_hash(PYTHON_STDLIB_MATH_PACK_ID)
    );
    assert_eq!(python_stdlib_math.kind, SemanticPackKind::StdlibPack);
    assert_eq!(
        python_stdlib_math.influence,
        SemanticPackInfluence::EvidenceAndContracts
    );
    assert_eq!(python_stdlib_math.counts.evidence_producers, 1);
    assert_eq!(python_stdlib_math.counts.contracts, 1);
    assert_eq!(python_stdlib_math.counts.positive_fixtures, 1);
    assert_eq!(python_stdlib_math.counts.hard_negatives, 2);
    let python = python_stdlib_type_domain_pack();
    assert_eq!(python.id, PYTHON_STDLIB_TYPE_DOMAIN_PACK_ID);
    assert_eq!(
        python.hash,
        stable_symbol_hash(PYTHON_STDLIB_TYPE_DOMAIN_PACK_ID)
    );
    assert_eq!(python.kind, SemanticPackKind::StdlibPack);
    assert_eq!(
        python.influence,
        SemanticPackInfluence::EvidenceAndContracts
    );
    assert_eq!(python.counts.evidence_producers, 1);
    assert_eq!(python.counts.contracts, 1);
    assert_eq!(
        python.counts.positive_fixtures,
        PYTHON_STDLIB_TYPE_DOMAIN_ALIAS_CONTRACTS.len()
    );
    let laws = value_graph_law_pack();
    assert_eq!(laws.id, VALUE_GRAPH_LAW_PACK_ID);
    assert_eq!(laws.hash, stable_symbol_hash(VALUE_GRAPH_LAW_PACK_ID));
    assert_eq!(laws.kind, SemanticPackKind::LawPack);
    assert_eq!(laws.counts.value_laws, pack_facing_value_laws().len());
    assert_eq!(laws.counts.positive_fixtures, 2);
    assert_eq!(laws.counts.hard_negatives, 4);
}
