use super::*;

pub(super) fn post_lower_record_swift_map_factory_result_domain(
    il: &mut Il,
    interner: &Interner,
    call: NodeId,
    api: EvidenceId,
) {
    let Some(dependencies) =
        post_lower_swift_dictionary_result_domain_dependencies(il, interner, call, api)
    else {
        return;
    };
    let _ = post_lower_find_or_push_evidence(
        il,
        EvidenceAnchor::node(il.node(call).span, NodeKind::Call),
        EvidenceKind::Domain(DomainEvidence::Map),
        "library_api_result_domain",
        dependencies,
    );
}

fn post_lower_swift_dictionary_result_domain_dependencies(
    il: &Il,
    interner: &Interner,
    call: NodeId,
    api: EvidenceId,
) -> Option<Vec<EvidenceId>> {
    let [_, kwarg] = il.children(call) else {
        return None;
    };
    if il.kind(*kwarg) != NodeKind::KwArg {
        return None;
    }
    let [entries] = il.children(*kwarg) else {
        return None;
    };
    if !nose_semantics::seq_surface_contract_for_node(il, interner, *entries)
        .is_some_and(|contract| contract.map_entry_list)
    {
        return None;
    }
    let mut dependencies = vec![api];
    dependencies.push(post_lower_sequence_surface_evidence_id(
        il,
        *entries,
        SequenceSurfaceKind::Collection,
    )?);
    let mut key_nodes = Vec::new();
    for &entry in il.children(*entries) {
        if nose_semantics::seq_surface_contract_for_node(il, interner, entry)
            .is_none_or(|contract| contract.value_tag != nose_semantics::SEQ_VALUE_TUPLE)
        {
            return None;
        }
        dependencies.push(post_lower_sequence_surface_evidence_id(
            il,
            entry,
            SequenceSurfaceKind::Tuple,
        )?);
        let [key, _value] = il.children(entry) else {
            return None;
        };
        key_nodes.push(*key);
    }
    if nose_semantics::nodes_contain_duplicate_static_literal_keys(il, key_nodes) {
        return None;
    }
    Some(dependencies)
}
