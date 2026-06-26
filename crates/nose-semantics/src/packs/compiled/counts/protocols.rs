use super::*;

pub(in crate::packs::compiled) fn map_get_protocol_counts() -> SemanticPackCounts {
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

pub(in crate::packs::compiled) fn map_get_default_protocol_counts() -> SemanticPackCounts {
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

pub(in crate::packs::compiled) fn free_function_builtin_protocol_counts() -> SemanticPackCounts {
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

pub(in crate::packs::compiled) fn python_iterator_builtin_protocol_counts() -> SemanticPackCounts {
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

pub(in crate::packs::compiled) fn receiver_membership_protocol_counts() -> SemanticPackCounts {
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

pub(in crate::packs::compiled) fn map_key_view_protocol_counts() -> SemanticPackCounts {
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

pub(in crate::packs::compiled) fn property_builtin_protocol_counts() -> SemanticPackCounts {
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

pub(in crate::packs::compiled) fn builtin_method_call_protocol_counts() -> SemanticPackCounts {
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

pub(in crate::packs::compiled) fn string_affix_predicate_protocol_counts() -> SemanticPackCounts {
    SemanticPackCounts {
        evidence_producers: STRING_AFFIX_PREDICATE_PROTOCOL_PRODUCER_IDS.len(),
        contracts: STRING_AFFIX_PREDICATE_PROTOCOL_CONTRACT_IDS.len(),
        value_laws: 0,
        positive_fixtures: STRING_AFFIX_PREDICATE_PROTOCOL_CONFORMANCE_REFS
            .iter()
            .filter(|id| !id.contains("hard-negative"))
            .count(),
        hard_negatives: STRING_AFFIX_PREDICATE_PROTOCOL_CONFORMANCE_REFS
            .iter()
            .filter(|id| id.contains("hard-negative"))
            .count(),
    }
}

pub(in crate::packs::compiled) fn sequence_hof_adapter_protocol_counts() -> SemanticPackCounts {
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
