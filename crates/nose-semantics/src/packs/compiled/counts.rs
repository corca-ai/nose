use super::*;

mod dynamic;
mod protocols;
mod swift;
pub(super) use dynamic::*;
pub(super) use protocols::*;
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
