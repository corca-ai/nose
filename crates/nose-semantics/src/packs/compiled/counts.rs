use super::*;

mod dynamic;
mod swift;
pub(super) use dynamic::*;
pub(super) use swift::*;

pub(super) fn empty_counts() -> SemanticPackCounts {
    SemanticPackCounts {
        evidence_producers: 0,
        contracts: 0,
        value_laws: 0,
        positive_fixtures: 0,
        hard_negatives: 0,
    }
}

pub(super) fn c_language_counts() -> SemanticPackCounts {
    SemanticPackCounts {
        evidence_producers: C_LANGUAGE_PRODUCER_IDS.len(),
        contracts: 0,
        value_laws: 0,
        positive_fixtures: C_LANGUAGE_CONFORMANCE_REFS
            .iter()
            .filter(|id| !id.contains("hard-negative"))
            .count(),
        hard_negatives: C_LANGUAGE_CONFORMANCE_REFS
            .iter()
            .filter(|id| id.contains("hard-negative"))
            .count(),
    }
}

pub(super) fn language_core_and_source_fact_counts() -> SemanticPackCounts {
    SemanticPackCounts {
        evidence_producers: 2,
        contracts: 0,
        value_laws: 0,
        positive_fixtures: 0,
        hard_negatives: 0,
    }
}

pub(super) fn python_builtin_collection_factory_counts() -> SemanticPackCounts {
    SemanticPackCounts {
        evidence_producers: PYTHON_BUILTIN_COLLECTION_FACTORY_PRODUCER_IDS.len(),
        contracts: PYTHON_BUILTIN_COLLECTION_FACTORY_CONTRACT_IDS.len(),
        value_laws: 0,
        positive_fixtures: PYTHON_BUILTIN_COLLECTION_FACTORY_CONFORMANCE_REFS
            .iter()
            .filter(|id| !id.contains("hard-negative"))
            .count(),
        hard_negatives: PYTHON_BUILTIN_COLLECTION_FACTORY_CONFORMANCE_REFS
            .iter()
            .filter(|id| id.contains("hard-negative"))
            .count(),
    }
}

pub(super) fn python_stdlib_collection_factory_counts() -> SemanticPackCounts {
    SemanticPackCounts {
        evidence_producers: PYTHON_STDLIB_COLLECTION_FACTORY_PRODUCER_IDS.len(),
        contracts: PYTHON_STDLIB_COLLECTION_FACTORY_CONTRACT_IDS.len(),
        value_laws: 0,
        positive_fixtures: PYTHON_STDLIB_COLLECTION_FACTORY_CONFORMANCE_REFS
            .iter()
            .filter(|id| !id.contains("hard-negative"))
            .count(),
        hard_negatives: PYTHON_STDLIB_COLLECTION_FACTORY_CONFORMANCE_REFS
            .iter()
            .filter(|id| id.contains("hard-negative"))
            .count(),
    }
}

pub(super) fn python_stdlib_math_counts() -> SemanticPackCounts {
    SemanticPackCounts {
        evidence_producers: PYTHON_STDLIB_MATH_PRODUCER_IDS.len(),
        contracts: PYTHON_STDLIB_MATH_CONTRACT_IDS.len(),
        value_laws: 0,
        positive_fixtures: PYTHON_STDLIB_MATH_CONFORMANCE_REFS
            .iter()
            .filter(|id| !id.contains("hard-negative"))
            .count(),
        hard_negatives: PYTHON_STDLIB_MATH_CONFORMANCE_REFS
            .iter()
            .filter(|id| id.contains("hard-negative"))
            .count(),
    }
}

pub(super) fn js_like_builtin_promise_counts() -> SemanticPackCounts {
    SemanticPackCounts {
        evidence_producers: JS_LIKE_BUILTIN_PROMISE_PRODUCER_IDS.len(),
        contracts: JS_LIKE_BUILTIN_PROMISE_CONTRACT_IDS.len(),
        value_laws: 0,
        positive_fixtures: JS_LIKE_BUILTIN_PROMISE_CONFORMANCE_REFS
            .iter()
            .filter(|id| !id.contains("hard-negative"))
            .count(),
        hard_negatives: JS_LIKE_BUILTIN_PROMISE_CONFORMANCE_REFS
            .iter()
            .filter(|id| id.contains("hard-negative"))
            .count(),
    }
}

pub(super) fn js_like_builtin_array_counts() -> SemanticPackCounts {
    SemanticPackCounts {
        evidence_producers: JS_LIKE_BUILTIN_ARRAY_PRODUCER_IDS.len(),
        contracts: JS_LIKE_BUILTIN_ARRAY_CONTRACT_IDS.len(),
        value_laws: 0,
        positive_fixtures: JS_LIKE_BUILTIN_ARRAY_CONFORMANCE_REFS
            .iter()
            .filter(|id| !id.contains("hard-negative"))
            .count(),
        hard_negatives: JS_LIKE_BUILTIN_ARRAY_CONFORMANCE_REFS
            .iter()
            .filter(|id| id.contains("hard-negative"))
            .count(),
    }
}

pub(super) fn js_like_builtin_boolean_counts() -> SemanticPackCounts {
    SemanticPackCounts {
        evidence_producers: JS_LIKE_BUILTIN_BOOLEAN_PRODUCER_IDS.len(),
        contracts: JS_LIKE_BUILTIN_BOOLEAN_CONTRACT_IDS.len(),
        value_laws: 0,
        positive_fixtures: JS_LIKE_BUILTIN_BOOLEAN_CONFORMANCE_REFS
            .iter()
            .filter(|id| !id.contains("hard-negative"))
            .count(),
        hard_negatives: JS_LIKE_BUILTIN_BOOLEAN_CONFORMANCE_REFS
            .iter()
            .filter(|id| id.contains("hard-negative"))
            .count(),
    }
}

pub(super) fn js_like_builtin_regex_counts() -> SemanticPackCounts {
    SemanticPackCounts {
        evidence_producers: JS_LIKE_BUILTIN_REGEX_PRODUCER_IDS.len(),
        contracts: JS_LIKE_BUILTIN_REGEX_CONTRACT_IDS.len(),
        value_laws: 0,
        positive_fixtures: JS_LIKE_BUILTIN_REGEX_CONFORMANCE_REFS
            .iter()
            .filter(|id| !id.contains("hard-negative"))
            .count(),
        hard_negatives: JS_LIKE_BUILTIN_REGEX_CONFORMANCE_REFS
            .iter()
            .filter(|id| id.contains("hard-negative"))
            .count(),
    }
}

pub(super) fn js_like_builtin_static_index_membership_counts() -> SemanticPackCounts {
    SemanticPackCounts {
        evidence_producers: JS_LIKE_BUILTIN_STATIC_INDEX_MEMBERSHIP_PRODUCER_IDS.len(),
        contracts: JS_LIKE_BUILTIN_STATIC_INDEX_MEMBERSHIP_CONTRACT_IDS.len(),
        value_laws: 0,
        positive_fixtures: JS_LIKE_BUILTIN_STATIC_INDEX_MEMBERSHIP_CONFORMANCE_REFS
            .iter()
            .filter(|id| !id.contains("hard-negative"))
            .count(),
        hard_negatives: JS_LIKE_BUILTIN_STATIC_INDEX_MEMBERSHIP_CONFORMANCE_REFS
            .iter()
            .filter(|id| id.contains("hard-negative"))
            .count(),
    }
}

pub(super) fn js_like_builtin_collection_constructor_counts() -> SemanticPackCounts {
    SemanticPackCounts {
        evidence_producers: JS_LIKE_BUILTIN_COLLECTION_CONSTRUCTOR_PRODUCER_IDS.len(),
        contracts: JS_LIKE_BUILTIN_COLLECTION_CONSTRUCTOR_CONTRACT_IDS.len(),
        value_laws: 0,
        positive_fixtures: JS_LIKE_BUILTIN_COLLECTION_CONSTRUCTOR_CONFORMANCE_REFS
            .iter()
            .filter(|id| !id.contains("hard-negative"))
            .count(),
        hard_negatives: JS_LIKE_BUILTIN_COLLECTION_CONSTRUCTOR_CONFORMANCE_REFS
            .iter()
            .filter(|id| id.contains("hard-negative"))
            .count(),
    }
}

pub(super) fn ruby_stdlib_set_counts() -> SemanticPackCounts {
    SemanticPackCounts {
        evidence_producers: RUBY_STDLIB_SET_PRODUCER_IDS.len(),
        contracts: RUBY_STDLIB_SET_CONTRACT_IDS.len(),
        value_laws: 0,
        positive_fixtures: RUBY_STDLIB_SET_CONFORMANCE_REFS
            .iter()
            .filter(|id| !id.contains("hard-negative"))
            .count(),
        hard_negatives: RUBY_STDLIB_SET_CONFORMANCE_REFS
            .iter()
            .filter(|id| id.contains("hard-negative"))
            .count(),
    }
}

pub(super) fn rust_stdlib_collection_factory_counts() -> SemanticPackCounts {
    SemanticPackCounts {
        evidence_producers: RUST_STDLIB_COLLECTION_FACTORY_PRODUCER_IDS.len(),
        contracts: RUST_STDLIB_COLLECTION_FACTORY_CONTRACT_IDS.len(),
        value_laws: 0,
        positive_fixtures: RUST_STDLIB_COLLECTION_FACTORY_CONFORMANCE_REFS
            .iter()
            .filter(|id| !id.contains("hard-negative"))
            .count(),
        hard_negatives: RUST_STDLIB_COLLECTION_FACTORY_CONFORMANCE_REFS
            .iter()
            .filter(|id| id.contains("hard-negative"))
            .count(),
    }
}

pub(super) fn rust_stdlib_map_factory_counts() -> SemanticPackCounts {
    SemanticPackCounts {
        evidence_producers: RUST_STDLIB_MAP_FACTORY_PRODUCER_IDS.len(),
        contracts: RUST_STDLIB_MAP_FACTORY_CONTRACT_IDS.len(),
        value_laws: 0,
        positive_fixtures: RUST_STDLIB_MAP_FACTORY_CONFORMANCE_REFS
            .iter()
            .filter(|id| !id.contains("hard-negative"))
            .count(),
        hard_negatives: RUST_STDLIB_MAP_FACTORY_CONFORMANCE_REFS
            .iter()
            .filter(|id| id.contains("hard-negative"))
            .count(),
    }
}

pub(super) fn rust_stdlib_option_counts() -> SemanticPackCounts {
    SemanticPackCounts {
        evidence_producers: RUST_STDLIB_OPTION_PRODUCER_IDS.len(),
        contracts: RUST_STDLIB_OPTION_CONTRACT_IDS.len(),
        value_laws: 0,
        positive_fixtures: RUST_STDLIB_OPTION_CONFORMANCE_REFS
            .iter()
            .filter(|id| !id.contains("hard-negative"))
            .count(),
        hard_negatives: RUST_STDLIB_OPTION_CONFORMANCE_REFS
            .iter()
            .filter(|id| id.contains("hard-negative"))
            .count(),
    }
}

pub(super) fn rust_stdlib_result_counts() -> SemanticPackCounts {
    SemanticPackCounts {
        evidence_producers: RUST_STDLIB_RESULT_PRODUCER_IDS.len(),
        contracts: RUST_STDLIB_RESULT_CONTRACT_IDS.len(),
        value_laws: 0,
        positive_fixtures: RUST_STDLIB_RESULT_CONFORMANCE_REFS
            .iter()
            .filter(|id| !id.contains("hard-negative"))
            .count(),
        hard_negatives: RUST_STDLIB_RESULT_CONFORMANCE_REFS
            .iter()
            .filter(|id| id.contains("hard-negative"))
            .count(),
    }
}

pub(super) fn rust_stdlib_integer_method_counts() -> SemanticPackCounts {
    SemanticPackCounts {
        evidence_producers: RUST_STDLIB_INTEGER_METHOD_PRODUCER_IDS.len(),
        contracts: RUST_STDLIB_INTEGER_METHOD_CONTRACT_IDS.len(),
        value_laws: 0,
        positive_fixtures: RUST_STDLIB_INTEGER_METHOD_CONFORMANCE_REFS
            .iter()
            .filter(|id| !id.contains("hard-negative"))
            .count(),
        hard_negatives: RUST_STDLIB_INTEGER_METHOD_CONFORMANCE_REFS
            .iter()
            .filter(|id| id.contains("hard-negative"))
            .count(),
    }
}

pub(super) fn java_stdlib_map_factory_counts() -> SemanticPackCounts {
    SemanticPackCounts {
        evidence_producers: JAVA_STDLIB_MAP_FACTORY_PRODUCER_IDS.len(),
        contracts: JAVA_STDLIB_MAP_FACTORY_CONTRACT_IDS.len(),
        value_laws: 0,
        positive_fixtures: JAVA_STDLIB_MAP_FACTORY_CONFORMANCE_REFS
            .iter()
            .filter(|id| !id.contains("hard-negative"))
            .count(),
        hard_negatives: JAVA_STDLIB_MAP_FACTORY_CONFORMANCE_REFS
            .iter()
            .filter(|id| id.contains("hard-negative"))
            .count(),
    }
}

pub(super) fn java_stdlib_math_counts() -> SemanticPackCounts {
    SemanticPackCounts {
        evidence_producers: JAVA_STDLIB_MATH_PRODUCER_IDS.len(),
        contracts: JAVA_STDLIB_MATH_CONTRACT_IDS.len(),
        value_laws: 0,
        positive_fixtures: JAVA_STDLIB_MATH_CONFORMANCE_REFS
            .iter()
            .filter(|id| !id.contains("hard-negative"))
            .count(),
        hard_negatives: JAVA_STDLIB_MATH_CONFORMANCE_REFS
            .iter()
            .filter(|id| id.contains("hard-negative"))
            .count(),
    }
}

pub(super) fn map_get_protocol_counts() -> SemanticPackCounts {
    SemanticPackCounts {
        evidence_producers: MAP_GET_PROTOCOL_PRODUCER_IDS.len(),
        contracts: MAP_GET_PROTOCOL_CONTRACT_IDS.len(),
        value_laws: 0,
        positive_fixtures: MAP_GET_PROTOCOL_CONFORMANCE_REFS
            .iter()
            .filter(|id| !id.contains("hard-negative"))
            .count(),
        hard_negatives: MAP_GET_PROTOCOL_CONFORMANCE_REFS
            .iter()
            .filter(|id| id.contains("hard-negative"))
            .count(),
    }
}

pub(super) fn map_get_default_protocol_counts() -> SemanticPackCounts {
    SemanticPackCounts {
        evidence_producers: MAP_GET_DEFAULT_PROTOCOL_PRODUCER_IDS.len(),
        contracts: MAP_GET_DEFAULT_PROTOCOL_CONTRACT_IDS.len(),
        value_laws: 0,
        positive_fixtures: MAP_GET_DEFAULT_PROTOCOL_CONFORMANCE_REFS
            .iter()
            .filter(|id| !id.contains("hard-negative"))
            .count(),
        hard_negatives: MAP_GET_DEFAULT_PROTOCOL_CONFORMANCE_REFS
            .iter()
            .filter(|id| id.contains("hard-negative"))
            .count(),
    }
}

pub(super) fn free_function_builtin_protocol_counts() -> SemanticPackCounts {
    SemanticPackCounts {
        evidence_producers: FREE_FUNCTION_BUILTIN_PROTOCOL_PRODUCER_IDS.len(),
        contracts: FREE_FUNCTION_BUILTIN_PROTOCOL_CONTRACT_IDS.len(),
        value_laws: 0,
        positive_fixtures: FREE_FUNCTION_BUILTIN_PROTOCOL_CONFORMANCE_REFS
            .iter()
            .filter(|id| !id.contains("hard-negative"))
            .count(),
        hard_negatives: FREE_FUNCTION_BUILTIN_PROTOCOL_CONFORMANCE_REFS
            .iter()
            .filter(|id| id.contains("hard-negative"))
            .count(),
    }
}

pub(super) fn python_iterator_builtin_protocol_counts() -> SemanticPackCounts {
    SemanticPackCounts {
        evidence_producers: PYTHON_ITERATOR_BUILTIN_PROTOCOL_PRODUCER_IDS.len(),
        contracts: PYTHON_ITERATOR_BUILTIN_PROTOCOL_CONTRACT_IDS.len(),
        value_laws: 0,
        positive_fixtures: PYTHON_ITERATOR_BUILTIN_PROTOCOL_CONFORMANCE_REFS
            .iter()
            .filter(|id| !id.contains("hard-negative"))
            .count(),
        hard_negatives: PYTHON_ITERATOR_BUILTIN_PROTOCOL_CONFORMANCE_REFS
            .iter()
            .filter(|id| id.contains("hard-negative"))
            .count(),
    }
}

pub(super) fn receiver_membership_protocol_counts() -> SemanticPackCounts {
    SemanticPackCounts {
        evidence_producers: RECEIVER_MEMBERSHIP_PROTOCOL_PRODUCER_IDS.len(),
        contracts: RECEIVER_MEMBERSHIP_PROTOCOL_CONTRACT_IDS.len(),
        value_laws: 0,
        positive_fixtures: RECEIVER_MEMBERSHIP_PROTOCOL_CONFORMANCE_REFS
            .iter()
            .filter(|id| !id.contains("hard-negative"))
            .count(),
        hard_negatives: RECEIVER_MEMBERSHIP_PROTOCOL_CONFORMANCE_REFS
            .iter()
            .filter(|id| id.contains("hard-negative"))
            .count(),
    }
}

pub(super) fn map_key_view_protocol_counts() -> SemanticPackCounts {
    SemanticPackCounts {
        evidence_producers: MAP_KEY_VIEW_PROTOCOL_PRODUCER_IDS.len(),
        contracts: MAP_KEY_VIEW_PROTOCOL_CONTRACT_IDS.len(),
        value_laws: 0,
        positive_fixtures: MAP_KEY_VIEW_PROTOCOL_CONFORMANCE_REFS
            .iter()
            .filter(|id| !id.contains("hard-negative"))
            .count(),
        hard_negatives: MAP_KEY_VIEW_PROTOCOL_CONFORMANCE_REFS
            .iter()
            .filter(|id| id.contains("hard-negative"))
            .count(),
    }
}

pub(super) fn property_builtin_protocol_counts() -> SemanticPackCounts {
    SemanticPackCounts {
        evidence_producers: PROPERTY_BUILTIN_PROTOCOL_PRODUCER_IDS.len(),
        contracts: PROPERTY_BUILTIN_PROTOCOL_CONTRACT_IDS.len(),
        value_laws: 0,
        positive_fixtures: PROPERTY_BUILTIN_PROTOCOL_CONFORMANCE_REFS
            .iter()
            .filter(|id| !id.contains("hard-negative"))
            .count(),
        hard_negatives: PROPERTY_BUILTIN_PROTOCOL_CONFORMANCE_REFS
            .iter()
            .filter(|id| id.contains("hard-negative"))
            .count(),
    }
}

pub(super) fn builtin_method_call_protocol_counts() -> SemanticPackCounts {
    SemanticPackCounts {
        evidence_producers: BUILTIN_METHOD_CALL_PROTOCOL_PRODUCER_IDS.len(),
        contracts: BUILTIN_METHOD_CALL_PROTOCOL_CONTRACT_IDS.len(),
        value_laws: 0,
        positive_fixtures: BUILTIN_METHOD_CALL_PROTOCOL_CONFORMANCE_REFS
            .iter()
            .filter(|id| !id.contains("hard-negative"))
            .count(),
        hard_negatives: BUILTIN_METHOD_CALL_PROTOCOL_CONFORMANCE_REFS
            .iter()
            .filter(|id| id.contains("hard-negative"))
            .count(),
    }
}

pub(super) fn sequence_hof_adapter_protocol_counts() -> SemanticPackCounts {
    SemanticPackCounts {
        evidence_producers: SEQUENCE_HOF_ADAPTER_PROTOCOL_PRODUCER_IDS.len(),
        contracts: SEQUENCE_HOF_ADAPTER_PROTOCOL_CONTRACT_IDS.len(),
        value_laws: 0,
        positive_fixtures: SEQUENCE_HOF_ADAPTER_PROTOCOL_CONFORMANCE_REFS
            .iter()
            .filter(|id| !id.contains("hard-negative"))
            .count(),
        hard_negatives: SEQUENCE_HOF_ADAPTER_PROTOCOL_CONFORMANCE_REFS
            .iter()
            .filter(|id| id.contains("hard-negative"))
            .count(),
    }
}

pub(super) fn go_stdlib_namespace_call_counts() -> SemanticPackCounts {
    SemanticPackCounts {
        evidence_producers: GO_STDLIB_NAMESPACE_CALL_PRODUCER_IDS.len(),
        contracts: GO_STDLIB_NAMESPACE_CALL_CONTRACT_IDS.len(),
        value_laws: 0,
        positive_fixtures: GO_STDLIB_NAMESPACE_CALL_CONFORMANCE_REFS
            .iter()
            .filter(|id| !id.contains("hard-negative"))
            .count(),
        hard_negatives: GO_STDLIB_NAMESPACE_CALL_CONFORMANCE_REFS
            .iter()
            .filter(|id| id.contains("hard-negative"))
            .count(),
    }
}

pub(super) fn java_stdlib_map_entry_counts() -> SemanticPackCounts {
    SemanticPackCounts {
        evidence_producers: JAVA_STDLIB_MAP_ENTRY_PRODUCER_IDS.len(),
        contracts: JAVA_STDLIB_MAP_ENTRY_CONTRACT_IDS.len(),
        value_laws: 0,
        positive_fixtures: JAVA_STDLIB_MAP_ENTRY_CONFORMANCE_REFS
            .iter()
            .filter(|id| !id.contains("hard-negative"))
            .count(),
        hard_negatives: JAVA_STDLIB_MAP_ENTRY_CONFORMANCE_REFS
            .iter()
            .filter(|id| id.contains("hard-negative"))
            .count(),
    }
}

pub(super) fn java_stdlib_collection_factory_counts() -> SemanticPackCounts {
    SemanticPackCounts {
        evidence_producers: JAVA_STDLIB_COLLECTION_FACTORY_PRODUCER_IDS.len(),
        contracts: JAVA_STDLIB_COLLECTION_FACTORY_CONTRACT_IDS.len(),
        value_laws: 0,
        positive_fixtures: JAVA_STDLIB_COLLECTION_FACTORY_CONFORMANCE_REFS
            .iter()
            .filter(|id| !id.contains("hard-negative"))
            .count(),
        hard_negatives: JAVA_STDLIB_COLLECTION_FACTORY_CONFORMANCE_REFS
            .iter()
            .filter(|id| id.contains("hard-negative"))
            .count(),
    }
}

pub(super) fn java_guava_immutable_collection_factory_counts() -> SemanticPackCounts {
    SemanticPackCounts {
        evidence_producers: JAVA_GUAVA_IMMUTABLE_COLLECTION_FACTORY_PRODUCER_IDS.len(),
        contracts: JAVA_GUAVA_IMMUTABLE_COLLECTION_FACTORY_CONTRACT_IDS.len(),
        value_laws: 0,
        positive_fixtures: JAVA_GUAVA_IMMUTABLE_COLLECTION_FACTORY_CONFORMANCE_REFS
            .iter()
            .filter(|id| !id.contains("hard-negative"))
            .count(),
        hard_negatives: JAVA_GUAVA_IMMUTABLE_COLLECTION_FACTORY_CONFORMANCE_REFS
            .iter()
            .filter(|id| id.contains("hard-negative"))
            .count(),
    }
}

pub(super) fn java_stdlib_collection_constructor_counts() -> SemanticPackCounts {
    SemanticPackCounts {
        evidence_producers: JAVA_STDLIB_COLLECTION_CONSTRUCTOR_PRODUCER_IDS.len(),
        contracts: JAVA_STDLIB_COLLECTION_CONSTRUCTOR_CONTRACT_IDS.len(),
        value_laws: 0,
        positive_fixtures: JAVA_STDLIB_COLLECTION_CONSTRUCTOR_CONFORMANCE_REFS
            .iter()
            .filter(|id| !id.contains("hard-negative"))
            .count(),
        hard_negatives: JAVA_STDLIB_COLLECTION_CONSTRUCTOR_CONFORMANCE_REFS
            .iter()
            .filter(|id| id.contains("hard-negative"))
            .count(),
    }
}

pub(super) fn java_stdlib_static_collection_adapter_counts() -> SemanticPackCounts {
    SemanticPackCounts {
        evidence_producers: JAVA_STDLIB_STATIC_COLLECTION_ADAPTER_PRODUCER_IDS.len(),
        contracts: JAVA_STDLIB_STATIC_COLLECTION_ADAPTER_CONTRACT_IDS.len(),
        value_laws: 0,
        positive_fixtures: JAVA_STDLIB_STATIC_COLLECTION_ADAPTER_CONFORMANCE_REFS
            .iter()
            .filter(|id| !id.contains("hard-negative"))
            .count(),
        hard_negatives: JAVA_STDLIB_STATIC_COLLECTION_ADAPTER_CONFORMANCE_REFS
            .iter()
            .filter(|id| id.contains("hard-negative"))
            .count(),
    }
}

pub(super) fn iterator_identity_adapter_counts() -> SemanticPackCounts {
    SemanticPackCounts {
        evidence_producers: ITERATOR_IDENTITY_ADAPTER_PRODUCER_IDS.len(),
        contracts: ITERATOR_IDENTITY_ADAPTER_CONTRACT_IDS.len(),
        value_laws: 0,
        positive_fixtures: ITERATOR_IDENTITY_ADAPTER_CONFORMANCE_REFS
            .iter()
            .filter(|id| !id.contains("hard-negative"))
            .count(),
        hard_negatives: ITERATOR_IDENTITY_ADAPTER_CONFORMANCE_REFS
            .iter()
            .filter(|id| id.contains("hard-negative"))
            .count(),
    }
}

pub(super) fn rust_stdlib_vec_counts() -> SemanticPackCounts {
    SemanticPackCounts {
        evidence_producers: RUST_STDLIB_VEC_PRODUCER_IDS.len(),
        contracts: RUST_STDLIB_VEC_CONTRACT_IDS.len(),
        value_laws: 0,
        positive_fixtures: RUST_STDLIB_VEC_CONFORMANCE_REFS
            .iter()
            .filter(|id| !id.contains("hard-negative"))
            .count(),
        hard_negatives: RUST_STDLIB_VEC_CONFORMANCE_REFS
            .iter()
            .filter(|id| id.contains("hard-negative"))
            .count(),
    }
}

pub(super) fn python_stdlib_type_domain_counts() -> SemanticPackCounts {
    SemanticPackCounts {
        evidence_producers: PYTHON_STDLIB_TYPE_DOMAIN_PRODUCER_IDS.len(),
        contracts: PYTHON_STDLIB_TYPE_DOMAIN_CONTRACT_IDS.len(),
        value_laws: 0,
        positive_fixtures: PYTHON_STDLIB_TYPE_DOMAIN_ALIAS_CONTRACTS.len(),
        hard_negatives: 2,
    }
}
