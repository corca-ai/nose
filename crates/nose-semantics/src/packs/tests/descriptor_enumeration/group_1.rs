use super::*;

pub(super) fn assert_group() {
    let java_stdlib_math =
        builtin_pack_descriptor(JAVA_STDLIB_MATH_PACK_ID).expect("Java stdlib Math descriptor");
    assert_eq!(java_stdlib_math.kind, SemanticPackKind::StdlibPack);
    assert_eq!(java_stdlib_math.supported_languages, &["java"]);
    assert_eq!(java_stdlib_math.supported_packages, &["java.lang"]);
    assert_eq!(
        java_stdlib_math.evidence_producer_ids,
        &[JAVA_STDLIB_MATH_PRODUCER_ID]
    );
    assert!(java_stdlib_math.source_fact_producer_ids.is_empty());
    assert_eq!(
        java_stdlib_math.contract_ids,
        &[
            SCALAR_INTEGER_METHOD_ABS_CONTRACT_ID,
            SCALAR_INTEGER_METHOD_MIN_CONTRACT_ID,
            SCALAR_INTEGER_METHOD_MAX_CONTRACT_ID
        ]
    );
    assert_eq!(java_stdlib_math.counts().evidence_producers, 1);
    assert_eq!(java_stdlib_math.counts().contracts, 3);
    assert_eq!(java_stdlib_math.counts().positive_fixtures, 3);
    assert_eq!(java_stdlib_math.counts().hard_negatives, 3);
    assert!(java_stdlib_math
        .conformance_refs()
        .contains(&"java-math-shadowed-math-hard-negative"));
    assert!(java_stdlib_math
        .conformance_refs()
        .contains(&"java-math-non-integer-argument-hard-negative"));
    assert!(!java_stdlib_math
        .contract_ids
        .contains(&SCALAR_INTEGER_METHOD_CLAMP_CONTRACT_ID));

    let java_stdlib_maps = builtin_pack_descriptor(JAVA_STDLIB_MAP_FACTORY_PACK_ID)
        .expect("Java stdlib map factory descriptor");
    assert_eq!(java_stdlib_maps.kind, SemanticPackKind::StdlibPack);
    assert_eq!(java_stdlib_maps.supported_languages, &["java"]);
    assert_eq!(java_stdlib_maps.supported_packages, &["java.util"]);
    assert_eq!(
        java_stdlib_maps.evidence_producer_ids,
        &[JAVA_STDLIB_MAP_FACTORY_PRODUCER_ID]
    );
    assert!(java_stdlib_maps.source_fact_producer_ids.is_empty());
    assert_eq!(
        java_stdlib_maps.contract_ids,
        &[
            JAVA_STDLIB_MAP_FACTORY_OF_CONTRACT_ID,
            JAVA_STDLIB_MAP_FACTORY_OF_ENTRIES_CONTRACT_ID,
            JAVA_STDLIB_MAP_FACTORY_COLLECTIONS_EMPTY_MAP_CONTRACT_ID,
            JAVA_STDLIB_MAP_FACTORY_COLLECTIONS_SINGLETON_MAP_CONTRACT_ID
        ]
    );
    assert_eq!(java_stdlib_maps.counts().evidence_producers, 1);
    assert_eq!(java_stdlib_maps.counts().contracts, 4);
    assert_eq!(java_stdlib_maps.counts().positive_fixtures, 4);
    assert_eq!(java_stdlib_maps.counts().hard_negatives, 4);
    assert!(java_stdlib_maps
        .conformance_refs()
        .contains(&"java-map-missing-import-hard-negative"));
    assert!(!java_stdlib_maps
        .contract_ids
        .contains(&"java.map_entry_factory"));

    let java_stdlib_entries = builtin_pack_descriptor(JAVA_STDLIB_MAP_ENTRY_PACK_ID)
        .expect("Java stdlib map entry descriptor");
    assert_eq!(java_stdlib_entries.kind, SemanticPackKind::StdlibPack);
    assert_eq!(java_stdlib_entries.supported_languages, &["java"]);
    assert_eq!(java_stdlib_entries.supported_packages, &["java.util"]);
    assert_eq!(
        java_stdlib_entries.evidence_producer_ids,
        &[JAVA_STDLIB_MAP_ENTRY_PRODUCER_ID]
    );
    assert!(java_stdlib_entries.source_fact_producer_ids.is_empty());
    assert_eq!(
        java_stdlib_entries.contract_ids,
        &[JAVA_STDLIB_MAP_ENTRY_CONTRACT_ID]
    );
    assert_eq!(java_stdlib_entries.counts().evidence_producers, 1);
    assert_eq!(java_stdlib_entries.counts().contracts, 1);
    assert_eq!(java_stdlib_entries.counts().positive_fixtures, 1);
    assert_eq!(java_stdlib_entries.counts().hard_negatives, 2);
    assert!(java_stdlib_entries
        .conformance_refs()
        .contains(&"java-map-entry-shadowed-map-hard-negative"));

    let java_stdlib_collections = builtin_pack_descriptor(JAVA_STDLIB_COLLECTION_FACTORY_PACK_ID)
        .expect("Java stdlib collection factory descriptor");
    assert_eq!(java_stdlib_collections.kind, SemanticPackKind::StdlibPack);
    assert_eq!(java_stdlib_collections.supported_languages, &["java"]);
    assert_eq!(java_stdlib_collections.supported_packages, &["java.util"]);
    assert_eq!(
        java_stdlib_collections.evidence_producer_ids,
        &[JAVA_STDLIB_COLLECTION_FACTORY_PRODUCER_ID]
    );
    assert!(java_stdlib_collections.source_fact_producer_ids.is_empty());
    assert_eq!(
        java_stdlib_collections.contract_ids,
        &[
            JAVA_STDLIB_COLLECTION_FACTORY_LIST_OF_CONTRACT_ID,
            JAVA_STDLIB_COLLECTION_FACTORY_SET_OF_CONTRACT_ID,
            JAVA_STDLIB_COLLECTION_FACTORY_ARRAYS_AS_LIST_CONTRACT_ID,
            JAVA_STDLIB_COLLECTION_FACTORY_COLLECTIONS_EMPTY_LIST_CONTRACT_ID,
            JAVA_STDLIB_COLLECTION_FACTORY_COLLECTIONS_EMPTY_SET_CONTRACT_ID,
            JAVA_STDLIB_COLLECTION_FACTORY_COLLECTIONS_SINGLETON_CONTRACT_ID,
            JAVA_STDLIB_COLLECTION_FACTORY_COLLECTIONS_SINGLETON_LIST_CONTRACT_ID
        ]
    );
    assert_eq!(java_stdlib_collections.counts().evidence_producers, 1);
    assert_eq!(java_stdlib_collections.counts().contracts, 7);
    assert_eq!(java_stdlib_collections.counts().positive_fixtures, 7);
    assert_eq!(java_stdlib_collections.counts().hard_negatives, 5);
    assert!(java_stdlib_collections
        .conformance_refs()
        .contains(&"java-collection-missing-import-hard-negative"));
    assert!(!java_stdlib_collections
        .contract_ids
        .contains(&"java.collection_constructor.empty_list"));

    let java_guava_collections =
        builtin_pack_descriptor(JAVA_GUAVA_IMMUTABLE_COLLECTION_FACTORY_PACK_ID)
            .expect("Java Guava immutable collection factory descriptor");
    assert_eq!(java_guava_collections.kind, SemanticPackKind::LibraryPack);
    assert_eq!(java_guava_collections.supported_languages, &["java"]);
    assert_eq!(
        java_guava_collections.supported_packages,
        &["com.google.common.collect"]
    );
    assert_eq!(
        java_guava_collections.evidence_producer_ids,
        &[JAVA_GUAVA_IMMUTABLE_COLLECTION_FACTORY_PRODUCER_ID]
    );
    assert_eq!(
        java_guava_collections.contract_ids,
        &[
            JAVA_GUAVA_IMMUTABLE_LIST_OF_CONTRACT_ID,
            JAVA_GUAVA_IMMUTABLE_SET_OF_CONTRACT_ID,
            JAVA_GUAVA_IMMUTABLE_MAP_OF_CONTRACT_ID
        ]
    );
    assert_eq!(java_guava_collections.counts().evidence_producers, 1);
    assert_eq!(java_guava_collections.counts().contracts, 3);
    assert_eq!(java_guava_collections.counts().positive_fixtures, 3);
    assert_eq!(java_guava_collections.counts().hard_negatives, 4);
    assert!(java_guava_collections
        .conformance_refs()
        .contains(&"java-guava-immutable-shadowed-type-hard-negative"));
    assert!(java_guava_collections
        .contract_ids
        .contains(&"java.map_factory.guava_immutable_map_of"));

    let java_stdlib_constructors =
        builtin_pack_descriptor(JAVA_STDLIB_COLLECTION_CONSTRUCTOR_PACK_ID)
            .expect("Java stdlib collection constructor descriptor");
    assert_eq!(java_stdlib_constructors.kind, SemanticPackKind::StdlibPack);
    assert_eq!(java_stdlib_constructors.supported_languages, &["java"]);
    assert_eq!(java_stdlib_constructors.supported_packages, &["java.util"]);
    assert_eq!(
        java_stdlib_constructors.evidence_producer_ids,
        &[JAVA_STDLIB_COLLECTION_CONSTRUCTOR_PRODUCER_ID]
    );
    assert!(java_stdlib_constructors.source_fact_producer_ids.is_empty());
    assert_eq!(
        java_stdlib_constructors.contract_ids,
        &[JAVA_STDLIB_COLLECTION_CONSTRUCTOR_EMPTY_LIST_CONTRACT_ID]
    );
    assert_eq!(java_stdlib_constructors.counts().evidence_producers, 1);
    assert_eq!(java_stdlib_constructors.counts().contracts, 1);
    assert_eq!(java_stdlib_constructors.counts().positive_fixtures, 2);
    assert_eq!(java_stdlib_constructors.counts().hard_negatives, 3);
    assert!(java_stdlib_constructors
        .conformance_refs()
        .contains(&"java-constructor-conflicting-import-hard-negative"));

    let java_static_adapters =
        builtin_pack_descriptor(JAVA_STDLIB_STATIC_COLLECTION_ADAPTER_PACK_ID)
            .expect("Java stdlib static collection adapter descriptor");
    assert_eq!(java_static_adapters.kind, SemanticPackKind::StdlibPack);
    assert_eq!(java_static_adapters.supported_languages, &["java"]);
    assert_eq!(java_static_adapters.supported_packages, &["java.util"]);
    assert_eq!(
        java_static_adapters.evidence_producer_ids,
        &[JAVA_STDLIB_STATIC_COLLECTION_ADAPTER_PRODUCER_ID]
    );
    assert!(java_static_adapters.source_fact_producer_ids.is_empty());
    assert_eq!(
        java_static_adapters.contract_ids,
        &[JAVA_STDLIB_STATIC_COLLECTION_ADAPTER_CONTRACT_ID]
    );
    assert_eq!(java_static_adapters.counts().evidence_producers, 1);
    assert_eq!(java_static_adapters.counts().contracts, 1);
    assert_eq!(java_static_adapters.counts().positive_fixtures, 1);
    assert_eq!(java_static_adapters.counts().hard_negatives, 2);
    assert!(java_static_adapters
        .conformance_refs()
        .contains(&"java-arrays-stream-shadowed-arrays-hard-negative"));

    let map_get =
        builtin_pack_descriptor(MAP_GET_PROTOCOL_PACK_ID).expect("map-get protocol descriptor");
    assert_eq!(map_get.kind, SemanticPackKind::ProtocolPack);
    assert_eq!(
        map_get.supported_languages,
        &[
            "java",
            "rust",
            "javascript",
            "typescript",
            "vue",
            "svelte",
            "html"
        ]
    );
    assert_eq!(
        map_get.supported_packages,
        &["Map", "java.util", "std::collections"]
    );
    assert_eq!(
        map_get.evidence_producer_ids,
        &[MAP_GET_PROTOCOL_PRODUCER_ID]
    );
    assert!(map_get.source_fact_producer_ids.is_empty());
    assert_eq!(map_get.contract_ids, &[MAP_GET_CONTRACT_ID]);
    assert_eq!(map_get.counts().evidence_producers, 1);
    assert_eq!(map_get.counts().contracts, 1);
    assert_eq!(map_get.counts().positive_fixtures, 3);
    assert_eq!(map_get.counts().hard_negatives, 2);
    assert!(map_get
        .conformance_refs()
        .contains(&"map-get-non-map-receiver-hard-negative"));

    let map_get_default = builtin_pack_descriptor(MAP_GET_DEFAULT_PROTOCOL_PACK_ID)
        .expect("map-get-default protocol descriptor");
    assert_eq!(map_get_default.kind, SemanticPackKind::ProtocolPack);
    assert_eq!(
        map_get_default.supported_languages,
        &["python", "ruby", "java"]
    );
    assert_eq!(
        map_get_default.supported_packages,
        &["dict", "Hash", "java.util"]
    );
    assert_eq!(
        map_get_default.evidence_producer_ids,
        &[MAP_GET_DEFAULT_PROTOCOL_PRODUCER_ID]
    );
    assert!(map_get_default.source_fact_producer_ids.is_empty());
    assert_eq!(map_get_default.contract_ids, &[MAP_GET_DEFAULT_CONTRACT_ID]);
    assert_eq!(map_get_default.counts().evidence_producers, 1);
    assert_eq!(map_get_default.counts().contracts, 1);
    assert_eq!(map_get_default.counts().positive_fixtures, 3);
    assert_eq!(map_get_default.counts().hard_negatives, 2);
    assert!(map_get_default
        .conformance_refs()
        .contains(&"map-get-default-non-map-receiver-hard-negative"));

    let free_function_builtin = builtin_pack_descriptor(FREE_FUNCTION_BUILTIN_PROTOCOL_PACK_ID)
        .expect("free-function builtin protocol descriptor");
    assert_eq!(free_function_builtin.kind, SemanticPackKind::ProtocolPack);
    assert_eq!(
        free_function_builtin.supported_languages,
        &["python", "go", "swift"]
    );
    assert_eq!(
        free_function_builtin.supported_packages,
        &["builtins", "go.predeclared", "Swift"]
    );
    assert_eq!(
        free_function_builtin.evidence_producer_ids,
        &[FREE_FUNCTION_BUILTIN_PROTOCOL_PRODUCER_ID]
    );
    assert!(free_function_builtin.source_fact_producer_ids.is_empty());
    assert_eq!(
        free_function_builtin.contract_ids,
        &[FREE_FUNCTION_BUILTIN_CONTRACT_ID]
    );
    assert_eq!(free_function_builtin.counts().evidence_producers, 1);
    assert_eq!(free_function_builtin.counts().contracts, 1);
    assert_eq!(free_function_builtin.counts().positive_fixtures, 6);
    assert_eq!(free_function_builtin.counts().hard_negatives, 4);
    assert!(free_function_builtin
        .conformance_refs()
        .contains(&"free-function-builtin-compatibility-pack-hard-negative"));

    let python_iterator_builtin = builtin_pack_descriptor(PYTHON_ITERATOR_BUILTIN_PROTOCOL_PACK_ID)
        .expect("Python iterator builtin protocol descriptor");
    assert_eq!(python_iterator_builtin.kind, SemanticPackKind::ProtocolPack);
    assert_eq!(python_iterator_builtin.supported_languages, &["python"]);
    assert_eq!(python_iterator_builtin.supported_packages, &["builtins"]);
    assert_eq!(
        python_iterator_builtin.evidence_producer_ids,
        &[PYTHON_ITERATOR_BUILTIN_PROTOCOL_PRODUCER_ID]
    );
    assert!(python_iterator_builtin.source_fact_producer_ids.is_empty());
    assert_eq!(
        python_iterator_builtin.contract_ids,
        &[
            PYTHON_ITERATOR_BUILTIN_CONTRACT_ID,
            FREE_FUNCTION_HOF_CONTRACT_ID
        ]
    );
    assert_eq!(python_iterator_builtin.counts().evidence_producers, 1);
    assert_eq!(python_iterator_builtin.counts().contracts, 2);
    assert_eq!(python_iterator_builtin.counts().positive_fixtures, 7);
    assert_eq!(python_iterator_builtin.counts().hard_negatives, 7);
    assert!(python_iterator_builtin
        .conformance_refs()
        .contains(&"python-iterator-builtin-missing-source-proof-hard-negative"));

    let builtin_method_call = builtin_pack_descriptor(BUILTIN_METHOD_CALL_PROTOCOL_PACK_ID)
        .expect("builtin method-call protocol descriptor");
    assert_eq!(builtin_method_call.kind, SemanticPackKind::ProtocolPack);
    assert_eq!(
        builtin_method_call.supported_languages,
        &[
            "python",
            "javascript",
            "typescript",
            "vue",
            "svelte",
            "html",
            "rust",
            "java",
            "ruby",
            "swift"
        ]
    );
    assert_eq!(
        builtin_method_call.supported_packages,
        &["Collection", "Option", "String", "console", "functools"]
    );
    assert_eq!(
        builtin_method_call.evidence_producer_ids,
        &[BUILTIN_METHOD_CALL_PROTOCOL_PRODUCER_ID]
    );
    assert!(builtin_method_call.source_fact_producer_ids.is_empty());
    assert_eq!(
        builtin_method_call.contract_ids,
        &[BUILTIN_METHOD_CALL_CONTRACT_ID]
    );
    assert_eq!(builtin_method_call.counts().evidence_producers, 1);
    assert_eq!(builtin_method_call.counts().contracts, 1);
    assert_eq!(builtin_method_call.counts().positive_fixtures, 8);
    assert_eq!(builtin_method_call.counts().hard_negatives, 3);
    assert!(builtin_method_call
        .conformance_refs()
        .contains(&"builtin-method-call-wrong-pack-hard-negative"));

    let sequence_hof_adapter = builtin_pack_descriptor(SEQUENCE_HOF_ADAPTER_PROTOCOL_PACK_ID)
        .expect("sequence HOF adapter protocol descriptor");
    assert_eq!(sequence_hof_adapter.kind, SemanticPackKind::ProtocolPack);
    assert_eq!(sequence_hof_adapter.supported_languages, &["rust", "swift"]);
    assert_eq!(
        sequence_hof_adapter.supported_packages,
        &["core::iter", "Swift.Collection"]
    );
    assert_eq!(
        sequence_hof_adapter.evidence_producer_ids,
        &[SEQUENCE_HOF_ADAPTER_PROTOCOL_PRODUCER_ID]
    );
    assert!(sequence_hof_adapter.source_fact_producer_ids.is_empty());
    assert_eq!(
        sequence_hof_adapter.contract_ids,
        &[SEQUENCE_HOF_ADAPTER_CONTRACT_ID]
    );
    assert_eq!(sequence_hof_adapter.counts().evidence_producers, 1);
    assert_eq!(sequence_hof_adapter.counts().contracts, 1);
    assert_eq!(sequence_hof_adapter.counts().positive_fixtures, 10);
    assert_eq!(sequence_hof_adapter.counts().hard_negatives, 14);
    assert!(sequence_hof_adapter
        .conformance_refs()
        .contains(&"rust-iterator-hof-missing-terminal-proof-hard-negative"));
    assert!(sequence_hof_adapter
        .conformance_refs()
        .contains(&"rust-iterator-hof-one-shot-reuse-hard-negative"));
    assert!(sequence_hof_adapter
        .conformance_refs()
        .contains(&"swift-sequence-hof-flat-map-positive"));
    assert!(sequence_hof_adapter
        .conformance_refs()
        .contains(&"swift-sequence-hof-any-sequence-reuse-hard-negative"));

    let go_namespace_call = builtin_pack_descriptor(GO_STDLIB_NAMESPACE_CALL_PACK_ID)
        .expect("Go stdlib namespace-call descriptor");
    assert_eq!(go_namespace_call.kind, SemanticPackKind::StdlibPack);
    assert_eq!(go_namespace_call.supported_languages, &["go"]);
    assert_eq!(
        go_namespace_call.supported_packages,
        &["fmt", "slices", "strings"]
    );
    assert_eq!(
        go_namespace_call.evidence_producer_ids,
        &[GO_STDLIB_NAMESPACE_CALL_PRODUCER_ID]
    );
    assert!(go_namespace_call.source_fact_producer_ids.is_empty());
    assert_eq!(
        go_namespace_call.contract_ids,
        &[GO_STDLIB_NAMESPACE_CALL_CONTRACT_ID]
    );
    assert_eq!(go_namespace_call.counts().evidence_producers, 1);
    assert_eq!(go_namespace_call.counts().contracts, 1);
    assert_eq!(go_namespace_call.counts().positive_fixtures, 5);
    assert_eq!(go_namespace_call.counts().hard_negatives, 2);
    assert!(go_namespace_call
        .conformance_refs()
        .contains(&"go-stdlib-namespace-call-wrong-pack-hard-negative"));

    let property_builtin = builtin_pack_descriptor(PROPERTY_BUILTIN_PROTOCOL_PACK_ID)
        .expect("property builtin protocol descriptor");
    assert_eq!(property_builtin.kind, SemanticPackKind::ProtocolPack);
    assert_eq!(
        property_builtin.supported_languages,
        &[
            "javascript",
            "typescript",
            "vue",
            "svelte",
            "html",
            "java",
            "swift"
        ]
    );
    assert_eq!(
        property_builtin.supported_packages,
        &["Array", "Collection", "Swift.Collection", "java.lang"]
    );
    assert_eq!(
        property_builtin.evidence_producer_ids,
        &[PROPERTY_BUILTIN_PROTOCOL_PRODUCER_ID]
    );
    assert!(property_builtin.source_fact_producer_ids.is_empty());
    assert_eq!(
        property_builtin.contract_ids,
        &[
            PROPERTY_BUILTIN_LEN_CONTRACT_ID,
            PROPERTY_BUILTIN_IS_EMPTY_CONTRACT_ID
        ]
    );
    assert_eq!(property_builtin.counts().evidence_producers, 1);
    assert_eq!(property_builtin.counts().contracts, 2);
    assert_eq!(property_builtin.counts().positive_fixtures, 4);
    assert_eq!(property_builtin.counts().hard_negatives, 3);
    assert!(property_builtin
        .conformance_refs()
        .contains(&"property-builtin-wrong-pack-hard-negative"));
}
