use super::*;
use crate::PYTHON_STDLIB_TYPE_DOMAIN_PRODUCER_ID;

pub(super) fn assert_group() {
    let receiver_membership = builtin_pack_descriptor(RECEIVER_MEMBERSHIP_PROTOCOL_PACK_ID)
        .expect("receiver-membership protocol descriptor");
    assert_eq!(receiver_membership.kind, SemanticPackKind::ProtocolPack);
    assert_eq!(
        receiver_membership.supported_languages,
        &[
            "python",
            "ruby",
            "java",
            "rust",
            "swift",
            "javascript",
            "typescript",
            "vue",
            "svelte",
            "html"
        ]
    );
    assert_eq!(
        receiver_membership.supported_packages,
        &[
            "Array",
            "Collection",
            "Hash",
            "Map",
            "Set",
            "Swift.Collection",
            "dict",
            "java.util",
            "std::collections"
        ]
    );
    assert_eq!(
        receiver_membership.evidence_producer_ids,
        &[RECEIVER_MEMBERSHIP_PROTOCOL_PRODUCER_ID]
    );
    assert!(receiver_membership.source_fact_producer_ids.is_empty());
    assert_eq!(
        receiver_membership.contract_ids,
        &[RECEIVER_MEMBERSHIP_CONTRACT_ID]
    );
    assert_eq!(receiver_membership.counts().evidence_producers, 1);
    assert_eq!(receiver_membership.counts().contracts, 1);
    assert_eq!(receiver_membership.counts().positive_fixtures, 10);
    assert_eq!(receiver_membership.counts().hard_negatives, 3);
    assert!(receiver_membership
        .conformance_refs()
        .contains(&"receiver-membership-go-slices-contains-out-of-scope-hard-negative"));

    let map_key_view = builtin_pack_descriptor(MAP_KEY_VIEW_PROTOCOL_PACK_ID)
        .expect("map-key-view protocol descriptor");
    assert_eq!(map_key_view.kind, SemanticPackKind::ProtocolPack);
    assert_eq!(
        map_key_view.supported_languages,
        &[
            "python",
            "ruby",
            "java",
            "javascript",
            "typescript",
            "vue",
            "svelte",
            "html"
        ]
    );
    assert_eq!(
        map_key_view.supported_packages,
        &["dict", "Hash", "Map", "Object", "java.util"]
    );
    assert_eq!(
        map_key_view.evidence_producer_ids,
        &[MAP_KEY_VIEW_PROTOCOL_PRODUCER_ID]
    );
    assert!(map_key_view.source_fact_producer_ids.is_empty());
    assert_eq!(
        map_key_view.contract_ids,
        &[
            MAP_KEY_VIEW_COLLECTION_CONTRACT_ID,
            MAP_KEY_VIEW_ITERATOR_CONTRACT_ID
        ]
    );
    assert_eq!(map_key_view.counts().evidence_producers, 1);
    assert_eq!(map_key_view.counts().contracts, 2);
    assert_eq!(map_key_view.counts().positive_fixtures, 5);
    assert_eq!(map_key_view.counts().hard_negatives, 6);
    assert!(map_key_view
        .conformance_refs()
        .contains(&"map-key-view-non-map-receiver-hard-negative"));

    let iterator_identity_adapters = builtin_pack_descriptor(ITERATOR_IDENTITY_ADAPTER_PACK_ID)
        .expect("iterator identity adapter protocol descriptor");
    assert_eq!(
        iterator_identity_adapters.kind,
        SemanticPackKind::ProtocolPack
    );
    assert_eq!(
        iterator_identity_adapters.supported_languages,
        &["java", "rust"]
    );
    assert_eq!(
        iterator_identity_adapters.supported_packages,
        &["core::iter", "java.util.stream"]
    );
    assert_eq!(
        iterator_identity_adapters.evidence_producer_ids,
        &[ITERATOR_IDENTITY_ADAPTER_PRODUCER_ID]
    );
    assert!(iterator_identity_adapters
        .source_fact_producer_ids
        .is_empty());
    assert_eq!(
        iterator_identity_adapters.contract_ids,
        &[ITERATOR_IDENTITY_ADAPTER_CONTRACT_ID]
    );
    assert_eq!(iterator_identity_adapters.counts().evidence_producers, 1);
    assert_eq!(iterator_identity_adapters.counts().contracts, 1);
    assert_eq!(iterator_identity_adapters.counts().positive_fixtures, 3);
    assert_eq!(iterator_identity_adapters.counts().hard_negatives, 2);
    assert!(iterator_identity_adapters
        .conformance_refs()
        .contains(&"iterator-identity-non-iterable-receiver-hard-negative"));

    let js_promise = builtin_pack_descriptor(JS_LIKE_BUILTIN_PROMISE_PACK_ID)
        .expect("JavaScript builtins Promise descriptor");
    assert_eq!(js_promise.kind, SemanticPackKind::StdlibPack);
    assert_eq!(
        js_promise.supported_languages,
        &["javascript", "typescript"]
    );
    assert_eq!(js_promise.supported_packages, &["Promise"]);
    assert_eq!(
        js_promise.evidence_producer_ids,
        &[JS_LIKE_BUILTIN_PROMISE_PRODUCER_ID]
    );
    assert!(js_promise.source_fact_producer_ids.is_empty());
    assert_eq!(
        js_promise.contract_ids,
        &[
            JS_LIKE_BUILTIN_PROMISE_RESOLVE_CONTRACT_ID,
            JS_LIKE_BUILTIN_PROMISE_THEN_CONTRACT_ID,
        ]
    );
    assert_eq!(js_promise.counts().evidence_producers, 1);
    assert_eq!(js_promise.counts().contracts, 2);
    assert_eq!(js_promise.counts().positive_fixtures, 2);
    assert_eq!(js_promise.counts().hard_negatives, 3);
    assert!(js_promise
        .conformance_refs()
        .contains(&"js-promise-resolve-shadowed-hard-negative"));

    let js_array = builtin_pack_descriptor(JS_LIKE_BUILTIN_ARRAY_PACK_ID)
        .expect("JavaScript builtins Array descriptor");
    assert_eq!(js_array.kind, SemanticPackKind::StdlibPack);
    assert_eq!(js_array.supported_languages, &["javascript", "typescript"]);
    assert_eq!(js_array.supported_packages, &["Array"]);
    assert_eq!(
        js_array.evidence_producer_ids,
        &[JS_LIKE_BUILTIN_ARRAY_PRODUCER_ID]
    );
    assert!(js_array.source_fact_producer_ids.is_empty());
    assert_eq!(
        js_array.contract_ids,
        &[
            JS_LIKE_BUILTIN_ARRAY_FROM_CONTRACT_ID,
            JS_LIKE_BUILTIN_ARRAY_IS_ARRAY_CONTRACT_ID,
            JS_LIKE_BUILTIN_ARRAY_HOF_CONTRACT_ID,
            JS_LIKE_BUILTIN_ARRAY_BOOL_REDUCTION_CONTRACT_ID,
        ]
    );
    assert_eq!(js_array.counts().evidence_producers, 1);
    assert_eq!(js_array.counts().contracts, 4);
    assert_eq!(js_array.counts().positive_fixtures, 7);
    assert_eq!(js_array.counts().hard_negatives, 11);
    assert!(js_array
        .conformance_refs()
        .contains(&"js-array-from-shadowed-hard-negative"));

    let js_boolean = builtin_pack_descriptor(JS_LIKE_BUILTIN_BOOLEAN_PACK_ID)
        .expect("JavaScript builtins Boolean descriptor");
    assert_eq!(js_boolean.kind, SemanticPackKind::StdlibPack);
    assert_eq!(
        js_boolean.supported_languages,
        &["javascript", "typescript"]
    );
    assert_eq!(js_boolean.supported_packages, &["Boolean"]);
    assert_eq!(
        js_boolean.evidence_producer_ids,
        &[JS_LIKE_BUILTIN_BOOLEAN_PRODUCER_ID]
    );
    assert!(js_boolean.source_fact_producer_ids.is_empty());
    assert_eq!(
        js_boolean.contract_ids,
        &[JS_LIKE_BUILTIN_BOOLEAN_CONTRACT_ID]
    );
    assert_eq!(js_boolean.counts().evidence_producers, 1);
    assert_eq!(js_boolean.counts().contracts, 1);
    assert_eq!(js_boolean.counts().positive_fixtures, 1);
    assert_eq!(js_boolean.counts().hard_negatives, 2);
    assert!(js_boolean
        .conformance_refs()
        .contains(&"js-boolean-coercion-shadowed-hard-negative"));

    let js_regex = builtin_pack_descriptor(JS_LIKE_BUILTIN_REGEX_PACK_ID)
        .expect("JavaScript builtins RegExp descriptor");
    assert_eq!(js_regex.kind, SemanticPackKind::StdlibPack);
    assert_eq!(js_regex.supported_languages, &["javascript", "typescript"]);
    assert_eq!(js_regex.supported_packages, &["RegExp"]);
    assert_eq!(
        js_regex.evidence_producer_ids,
        &[JS_LIKE_BUILTIN_REGEX_PRODUCER_ID]
    );
    assert!(js_regex.source_fact_producer_ids.is_empty());
    assert_eq!(
        js_regex.contract_ids,
        &[JS_LIKE_BUILTIN_REGEX_TEST_CONTRACT_ID]
    );
    assert_eq!(js_regex.counts().evidence_producers, 1);
    assert_eq!(js_regex.counts().contracts, 1);
    assert_eq!(js_regex.counts().positive_fixtures, 1);
    assert_eq!(js_regex.counts().hard_negatives, 2);
    assert!(js_regex
        .conformance_refs()
        .contains(&"js-regex-test-string-receiver-hard-negative"));

    let js_static_index = builtin_pack_descriptor(JS_LIKE_BUILTIN_STATIC_INDEX_MEMBERSHIP_PACK_ID)
        .expect("JavaScript builtins static index membership descriptor");
    assert_eq!(js_static_index.kind, SemanticPackKind::StdlibPack);
    assert_eq!(
        js_static_index.supported_languages,
        &["javascript", "typescript"]
    );
    assert_eq!(js_static_index.supported_packages, &["Array"]);
    assert_eq!(
        js_static_index.evidence_producer_ids,
        &[JS_LIKE_BUILTIN_STATIC_INDEX_MEMBERSHIP_PRODUCER_ID]
    );
    assert!(js_static_index.source_fact_producer_ids.is_empty());
    assert_eq!(
        js_static_index.contract_ids,
        &[
            JS_LIKE_BUILTIN_STATIC_INDEX_MEMBERSHIP_INDEX_OF_CONTRACT_ID,
            JS_LIKE_BUILTIN_STATIC_INDEX_MEMBERSHIP_FIND_INDEX_CONTRACT_ID,
        ]
    );
    assert_eq!(js_static_index.counts().evidence_producers, 1);
    assert_eq!(js_static_index.counts().contracts, 2);
    assert_eq!(js_static_index.counts().positive_fixtures, 2);
    assert_eq!(js_static_index.counts().hard_negatives, 2);
    assert!(js_static_index
        .conformance_refs()
        .contains(&"js-static-index-membership-non-literal-receiver-hard-negative"));

    let js_collection_constructors =
        builtin_pack_descriptor(JS_LIKE_BUILTIN_COLLECTION_CONSTRUCTOR_PACK_ID)
            .expect("JavaScript builtins collection constructor descriptor");
    assert_eq!(
        js_collection_constructors.kind,
        SemanticPackKind::StdlibPack
    );
    assert_eq!(
        js_collection_constructors.supported_languages,
        &["javascript", "typescript"]
    );
    assert_eq!(
        js_collection_constructors.supported_packages,
        &["Map", "Set"]
    );
    assert_eq!(
        js_collection_constructors.evidence_producer_ids,
        &[JS_LIKE_BUILTIN_COLLECTION_CONSTRUCTOR_PRODUCER_ID]
    );
    assert!(js_collection_constructors
        .source_fact_producer_ids
        .is_empty());
    assert_eq!(
        js_collection_constructors.contract_ids,
        &[
            JS_LIKE_BUILTIN_SET_CONSTRUCTOR_CONTRACT_ID,
            JS_LIKE_BUILTIN_MAP_CONSTRUCTOR_CONTRACT_ID,
        ]
    );
    assert_eq!(js_collection_constructors.counts().evidence_producers, 1);
    assert_eq!(js_collection_constructors.counts().contracts, 2);
    assert_eq!(js_collection_constructors.counts().positive_fixtures, 2);
    assert_eq!(js_collection_constructors.counts().hard_negatives, 3);
    assert!(js_collection_constructors
        .conformance_refs()
        .contains(&"js-collection-constructor-missing-construct-hard-negative"));

    let python = builtin_pack_descriptor(PYTHON_STDLIB_TYPE_DOMAIN_PACK_ID)
        .expect("Python stdlib descriptor");
    assert_eq!(python.kind, SemanticPackKind::StdlibPack);
    assert_eq!(python.supported_languages, &["python"]);
    assert_eq!(
        python.supported_packages,
        &["typing", "collections.abc", "asyncio"]
    );
    assert_eq!(
        python.evidence_producer_ids,
        &[PYTHON_STDLIB_TYPE_DOMAIN_PRODUCER_ID]
    );
    assert_eq!(
        python.contract_ids,
        &["python.stdlib.type-domain-alias.contract"]
    );
    assert_eq!(
        python.type_domain_alias_contracts,
        PYTHON_STDLIB_TYPE_DOMAIN_ALIAS_CONTRACTS
    );
    assert!(python
        .type_domain_alias_contracts
        .iter()
        .all(|row| row.pack_id == PYTHON_STDLIB_TYPE_DOMAIN_PACK_ID));
    assert!(python
        .type_domain_alias_contracts
        .iter()
        .all(|row| row.producer_id == PYTHON_STDLIB_TYPE_DOMAIN_PRODUCER_ID));
    assert!(python
        .type_domain_alias_contracts
        .iter()
        .all(|row| python.contract_ids.contains(&row.contract_id)));
    assert_eq!(python.counts().evidence_producers, 1);
    assert!(python.source_fact_producer_ids.is_empty());
    assert_eq!(python.counts().contracts, 1);
    assert_eq!(
        python.counts().positive_fixtures,
        PYTHON_STDLIB_TYPE_DOMAIN_ALIAS_CONTRACTS.len()
    );
    assert!(python
        .conformance_refs()
        .contains(&"python-typing-dict-domain-positive"));
    assert!(python
        .conformance_refs()
        .contains(&"python-typing-domain-wrong-module-hard-negative"));
}
