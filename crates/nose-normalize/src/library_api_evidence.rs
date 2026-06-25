mod recording;
use recording::*;

use nose_il::{
    stable_symbol_hash, Builtin, DomainEvidence, EvidenceAnchor, EvidenceEmitter, EvidenceId,
    EvidenceKind, EvidenceRecord, EvidenceStatus, Il, Interner, LibraryApiEvidenceKind, NodeId,
    NodeKind, Payload, SequenceSurfaceKind, Symbol, SymbolEvidenceKind,
};
use nose_semantics::{
    admitted_library_api_result_domain_for_call_record, builder_append_method_contract,
    language_core_evidence_provenance, library_api_callee_contract_hash,
    library_api_contract_id_hash, library_api_free_name_shadow_safe,
    library_api_property_dependencies_for_field_with_cache,
    library_api_receiver_dependencies_for_call_with_cache, library_method_call_contract,
    library_property_builtin_contract, library_rust_option_none_sentinel_contract,
    library_rust_option_some_constructor_contract,
    proven_receiver_method_api_contract_for_call_with_cache, sequence_surface_kind_for_tag,
    LibraryApiCalleeContract, LibraryApiDependencyCache, MethodBuiltinArgs,
    MethodEffectReceiverContract, MethodReceiverContract, MethodSemanticContract,
    BUILTIN_COMPAT_PACK_ID, BUILTIN_METHOD_CALL_PROTOCOL_PACK_ID,
    BUILTIN_METHOD_CALL_PROTOCOL_PRODUCER_ID, RUST_STDLIB_OPTION_PRODUCER_ID,
};

pub(crate) fn run(il: &mut Il, interner: &Interner) {
    let calls: Vec<NodeId> = il
        .nodes
        .iter()
        .enumerate()
        .filter_map(|(idx, node)| (node.kind == NodeKind::Call).then_some(NodeId(idx as u32)))
        .collect();
    let fields: Vec<NodeId> = il
        .nodes
        .iter()
        .enumerate()
        .filter_map(|(idx, node)| (node.kind == NodeKind::Field).then_some(NodeId(idx as u32)))
        .collect();
    let vars: Vec<NodeId> = il
        .nodes
        .iter()
        .enumerate()
        .filter_map(|(idx, node)| (node.kind == NodeKind::Var).then_some(NodeId(idx as u32)))
        .collect();
    let mut dependency_cache = LibraryApiDependencyCache::default();
    for &call in &calls {
        if !record_rust_option_some_library_api(il, interner, call)
            && !record_builder_append_method_library_api(il, interner, call)
        {
            record_receiver_method_library_api(il, interner, call, &mut dependency_cache);
        }
        record_library_api_call_result_domains(il, interner, call);
    }
    for field in fields {
        record_property_library_api(il, interner, field, &mut dependency_cache);
    }
    for var in vars {
        record_rust_option_none_library_api(il, interner, var);
    }
}

fn record_rust_option_some_library_api(il: &mut Il, interner: &Interner, call: NodeId) -> bool {
    let kids = il.children(call);
    let Some((&callee, args)) = kids.split_first() else {
        return false;
    };
    let Some(name) = node_name(il, interner, callee) else {
        return false;
    };
    let arg_count = args.len();
    let Some(contract) =
        library_rust_option_some_constructor_contract(il.meta.lang, name, arg_count)
    else {
        return false;
    };
    let LibraryApiCalleeContract::FreeName { name, shadow } = contract.callee else {
        return false;
    };
    if !library_api_free_name_shadow_safe(il.meta.lang, name, shadow, |candidate| {
        file_defines_name_visible_at(il, interner, candidate, il.node(callee).span)
    }) {
        return false;
    }
    let Some(symbol_dependency) = unshadowed_symbol_evidence_id(il, callee, name) else {
        return false;
    };
    upsert_builtin_evidence_with_pack_id(
        il,
        EvidenceAnchor::node(il.node(call).span, NodeKind::Call),
        EvidenceKind::LibraryApi(LibraryApiEvidenceKind::Contract {
            contract_hash: library_api_contract_id_hash(contract.id),
            callee_hash: library_api_callee_contract_hash(contract.callee),
            arity: arg_count as u16,
        }),
        contract.pack_id,
        RUST_STDLIB_OPTION_PRODUCER_ID,
        vec![symbol_dependency],
    );
    true
}

fn record_builder_append_method_library_api(
    il: &mut Il,
    interner: &Interner,
    call: NodeId,
) -> bool {
    let kids = il.children(call);
    let Some((&callee, args)) = kids.split_first() else {
        return false;
    };
    if il.kind(callee) != NodeKind::Field || !matches!(il.node(call).payload, Payload::None) {
        return false;
    }
    let Payload::Name(method) = il.node(callee).payload else {
        return false;
    };
    let method = interner.resolve(method);
    let arg_count = args.len();
    let Some(effect) = builder_append_method_contract(il.meta.lang, method, arg_count) else {
        return false;
    };
    if effect.receiver != MethodEffectReceiverContract::ActiveCollectionBuilder {
        return false;
    }
    let Some(contract) = library_method_call_contract(il.meta.lang, method, arg_count) else {
        return false;
    };
    if contract.result.semantic != MethodSemanticContract::Builtin(Builtin::Append)
        || contract.result.args != MethodBuiltinArgs::ReceiverThenAll
    {
        return false;
    }
    let Some(dependencies) =
        builder_append_method_dependencies(il, interner, call, contract.callee)
    else {
        return false;
    };
    upsert_builtin_evidence_with_pack_id(
        il,
        EvidenceAnchor::node(il.node(call).span, NodeKind::Call),
        EvidenceKind::LibraryApi(LibraryApiEvidenceKind::Contract {
            contract_hash: library_api_contract_id_hash(contract.id),
            callee_hash: library_api_callee_contract_hash(contract.callee),
            arity: arg_count as u16,
        }),
        BUILTIN_METHOD_CALL_PROTOCOL_PACK_ID,
        BUILTIN_METHOD_CALL_PROTOCOL_PRODUCER_ID,
        dependencies,
    );
    true
}

fn record_library_api_call_result_domains(il: &mut Il, interner: &Interner, call: NodeId) {
    let anchor = EvidenceAnchor::node(il.node(call).span, NodeKind::Call);
    let domains: Vec<_> = il
        .evidence_anchored_at(anchor.span())
        .filter(|record| record.anchor == anchor)
        .filter(|record| !call_result_domain_already_materialized(il, anchor, record.id))
        .filter_map(|record| {
            admitted_library_api_result_domain_for_call_record(il, interner, call, record)
                .map(|domain| (record.id, domain))
        })
        .collect();
    for (api, domain) in domains {
        record_library_api_result_domain(il, call, domain, api);
    }
}

fn call_result_domain_already_materialized(
    il: &Il,
    anchor: EvidenceAnchor,
    api: EvidenceId,
) -> bool {
    il.evidence_anchored_at(anchor.span()).any(|record| {
        record.anchor == anchor
            && matches!(record.kind, EvidenceKind::Domain(_))
            && record.status == EvidenceStatus::Asserted
            && record.dependencies.as_slice() == [api]
    })
}

fn builder_append_method_dependencies(
    il: &mut Il,
    interner: &Interner,
    call: NodeId,
    callee: LibraryApiCalleeContract,
) -> Option<Vec<EvidenceId>> {
    let mut dependency_cache = LibraryApiDependencyCache::default();
    if let Some(dependencies) = library_api_receiver_dependencies_for_call_with_cache(
        il,
        interner,
        call,
        callee,
        &mut dependency_cache,
    ) {
        if let Some(current_dependency) = dependencies
            .iter()
            .copied()
            .find(|&dependency| language_core_append_receiver_domain_dependency(il, dependency))
        {
            close_legacy_duplicates_for_language_core_dependency(il, current_dependency);
            return Some(dependencies);
        }
        if dependencies
            .iter()
            .copied()
            .any(|dependency| legacy_first_party_append_receiver_domain_dependency(il, dependency))
        {
            if let Some((receiver, seed_dependency)) =
                builder_append_local_receiver_seed(il, interner, call, callee)
            {
                let receiver_domain =
                    upsert_local_collection_receiver_domain(il, receiver, seed_dependency);
                return Some(vec![receiver_domain]);
            }
        }
        return Some(dependencies);
    }

    let (receiver, seed_dependency) =
        builder_append_local_receiver_seed(il, interner, call, callee)?;
    let receiver_domain = upsert_local_collection_receiver_domain(il, receiver, seed_dependency);
    Some(vec![receiver_domain])
}

fn builder_append_local_receiver_seed(
    il: &Il,
    interner: &Interner,
    call: NodeId,
    callee: LibraryApiCalleeContract,
) -> Option<(NodeId, EvidenceId)> {
    let LibraryApiCalleeContract::Method { method, .. } = callee else {
        return None;
    };
    let callee_node = *il.children(call).first()?;
    let receiver = method_callee_receiver_node(il, interner, callee_node, method)?;
    let seed_dependency = local_collection_seed_dependency_id(il, interner, call, receiver)?;
    Some((receiver, seed_dependency))
}

fn upsert_local_collection_receiver_domain(
    il: &mut Il,
    receiver: NodeId,
    seed_dependency: EvidenceId,
) -> EvidenceId {
    upsert_language_core_evidence(
        il,
        EvidenceAnchor::node(il.node(receiver).span, il.kind(receiver)),
        EvidenceKind::Domain(DomainEvidence::Collection),
        vec![seed_dependency],
    )
}

fn language_core_append_receiver_domain_dependency(il: &Il, dependency: EvidenceId) -> bool {
    let Some(record) = il.evidence_record_by_id(dependency) else {
        return false;
    };
    let EvidenceKind::Domain(domain) = record.kind else {
        return false;
    };
    let (pack_id, producer_id) = language_core_evidence_provenance(il.meta.lang);
    domain.is_array_collection_or_set()
        && record.status == EvidenceStatus::Asserted
        && record.provenance.emitter == EvidenceEmitter::Builtin
        && record.provenance.pack_hash == Some(stable_symbol_hash(pack_id))
        && record.provenance.rule_hash == Some(stable_symbol_hash(producer_id))
}

fn close_legacy_duplicates_for_language_core_dependency(il: &mut Il, dependency: EvidenceId) {
    let Some(record) = il.evidence_record_by_id(dependency) else {
        return;
    };
    let anchor = record.anchor;
    let kind = record.kind;
    let legacy_pack_hash = stable_symbol_hash(BUILTIN_COMPAT_PACK_ID);
    for idx in il.evidence_indices_anchored_at(anchor.span()) {
        let duplicate = &mut il.evidence[idx as usize];
        if duplicate.id != dependency
            && duplicate.anchor == anchor
            && duplicate.kind == kind
            && duplicate.status == EvidenceStatus::Asserted
            && duplicate.provenance.emitter == EvidenceEmitter::Builtin
            && duplicate.provenance.pack_hash == Some(legacy_pack_hash)
        {
            duplicate.status = EvidenceStatus::Ambiguous;
        }
    }
}

fn legacy_first_party_append_receiver_domain_dependency(il: &Il, dependency: EvidenceId) -> bool {
    let Some(record) = il.evidence_record_by_id(dependency) else {
        return false;
    };
    let EvidenceKind::Domain(domain) = record.kind else {
        return false;
    };
    domain.is_array_collection_or_set()
        && record.status == EvidenceStatus::Asserted
        && record.provenance.emitter == EvidenceEmitter::Builtin
        && record.provenance.pack_hash == Some(stable_symbol_hash(BUILTIN_COMPAT_PACK_ID))
}

fn method_callee_receiver_node(
    il: &Il,
    interner: &Interner,
    callee: NodeId,
    expected_method: &str,
) -> Option<NodeId> {
    if il.kind(callee) != NodeKind::Field {
        return None;
    }
    let Payload::Name(method) = il.node(callee).payload else {
        return None;
    };
    if interner.resolve(method) != expected_method {
        return None;
    }
    il.children(callee).first().copied()
}

fn local_collection_seed_dependency_id(
    il: &Il,
    interner: &Interner,
    call: NodeId,
    receiver: NodeId,
) -> Option<EvidenceId> {
    let receiver_name = binding_node_name(il, receiver)?;
    let receiver_scope = nearest_scope(il, receiver);
    let call_span = il.node(call).span;
    let mut found = None;
    for (idx, node) in il.nodes.iter().enumerate() {
        if node.kind != NodeKind::Assign
            || node.span.file != call_span.file
            || node.span.end_byte > call_span.start_byte
            || nearest_scope(il, NodeId(idx as u32)) != receiver_scope
        {
            continue;
        }
        let assign = NodeId(idx as u32);
        let [target, rhs] = il.children(assign) else {
            continue;
        };
        if binding_node_name(il, *target) != Some(receiver_name) {
            continue;
        }
        let dependency = collection_seed_dependency_id(il, interner, *rhs)?;
        match found {
            None => found = Some(dependency),
            Some(_) => return None,
        }
    }
    found
}

fn nearest_scope(il: &Il, node: NodeId) -> Option<NodeId> {
    il.nearest_scope(node)
}

fn collection_seed_dependency_id(il: &Il, interner: &Interner, node: NodeId) -> Option<EvidenceId> {
    domain_evidence_id_for_node(il, node, DomainEvidence::Collection).or_else(|| {
        sequence_surface_evidence_id_for_node(il, interner, node, SequenceSurfaceKind::Collection)
    })
}

fn domain_evidence_id_for_node(
    il: &Il,
    node: NodeId,
    expected: DomainEvidence,
) -> Option<EvidenceId> {
    let anchor = EvidenceAnchor::node(il.node(node).span, il.kind(node));
    il.evidence_anchored_at(anchor.span()).find_map(|record| {
        (record.anchor == anchor
            && record.kind == EvidenceKind::Domain(expected)
            && record.status == EvidenceStatus::Asserted
            && il.evidence_dependencies_asserted(record))
        .then_some(record.id)
    })
}

fn sequence_surface_evidence_id_for_node(
    il: &Il,
    interner: &Interner,
    node: NodeId,
    expected: SequenceSurfaceKind,
) -> Option<EvidenceId> {
    if il.kind(node) != NodeKind::Seq {
        return None;
    }
    let raw_tag = match il.node(node).payload {
        Payload::None => None,
        Payload::Name(name) => Some(interner.resolve(name)),
        _ => return None,
    };
    if sequence_surface_kind_for_tag(il.meta.lang, raw_tag) != Some(expected) {
        return None;
    }
    let anchor = EvidenceAnchor::sequence(il.node(node).span);
    let mut found = None;
    for record in il.evidence_anchored_at(anchor.span()) {
        if record.anchor != anchor {
            continue;
        }
        let EvidenceKind::SequenceSurface(kind) = record.kind else {
            continue;
        };
        if !sequence_surface_record_has_language_core_provenance(il, record) {
            continue;
        }
        if record.status != EvidenceStatus::Asserted || !il.evidence_dependencies_asserted(record) {
            return None;
        }
        match found {
            None => found = Some((kind, record.id)),
            Some((existing, _)) if existing == kind => {}
            Some(_) => return None,
        }
    }
    found.and_then(|(kind, id)| (kind == expected).then_some(id))
}

fn sequence_surface_record_has_language_core_provenance(il: &Il, record: &EvidenceRecord) -> bool {
    if record.provenance.emitter != EvidenceEmitter::Builtin {
        return false;
    }
    let (pack_id, producer_id) = language_core_evidence_provenance(il.meta.lang);
    record.provenance.pack_hash == Some(stable_symbol_hash(pack_id))
        && record.provenance.rule_hash == Some(stable_symbol_hash(producer_id))
}

#[cfg(test)]
mod tests;
