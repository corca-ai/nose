use super::*;

pub(in crate::packs::compiled) fn python_stdlib_type_domain_conformance_refs() -> Vec<&'static str>
{
    let mut refs = Vec::with_capacity(PYTHON_STDLIB_TYPE_DOMAIN_ALIAS_CONTRACTS.len() * 2);
    for row in PYTHON_STDLIB_TYPE_DOMAIN_ALIAS_CONTRACTS {
        refs.push(row.positive_fixture);
        refs.push(row.hard_negative_fixture);
    }
    refs.extend(PYTHON_STDLIB_TYPE_DOMAIN_HARD_NEGATIVE_REFS);
    refs.sort_unstable();
    refs.dedup();
    refs
}

pub(in crate::packs::compiled) fn value_graph_law_ids() -> Vec<&'static str> {
    pack_facing_value_laws()
        .iter()
        .map(|law| law.law_id)
        .collect()
}

pub(in crate::packs::compiled) fn value_graph_law_conformance_refs() -> Vec<&'static str> {
    let mut refs = pack_facing_value_laws()
        .iter()
        .flat_map(|law| law.conformance_refs.iter().copied())
        .collect::<Vec<_>>();
    refs.sort_unstable();
    refs.dedup();
    refs
}

pub(in crate::packs::compiled) fn value_graph_law_counts() -> SemanticPackCounts {
    let laws = pack_facing_value_laws();
    SemanticPackCounts {
        evidence_producers: 0,
        contracts: 0,
        value_laws: laws.len(),
        positive_fixtures: laws
            .iter()
            .map(|law| {
                law.conformance_refs
                    .iter()
                    .filter(|id| !id.contains("hard-negative"))
                    .count()
            })
            .sum(),
        hard_negatives: laws
            .iter()
            .map(|law| {
                law.conformance_refs
                    .iter()
                    .filter(|id| id.contains("hard-negative"))
                    .count()
            })
            .sum(),
    }
}
