use super::*;

pub(in crate::packs::compiled) fn swift_stdlib_collection_factory_counts() -> SemanticPackCounts {
    SemanticPackCounts {
        evidence_producers: SWIFT_STDLIB_COLLECTION_FACTORY_PRODUCER_IDS.len(),
        contracts: SWIFT_STDLIB_COLLECTION_FACTORY_CONTRACT_IDS.len(),
        value_laws: 0,
        positive_fixtures: SWIFT_STDLIB_COLLECTION_FACTORY_CONFORMANCE_REFS
            .iter()
            .filter(|id| !id.contains("hard-negative"))
            .count(),
        hard_negatives: SWIFT_STDLIB_COLLECTION_FACTORY_CONFORMANCE_REFS
            .iter()
            .filter(|id| id.contains("hard-negative"))
            .count(),
    }
}
