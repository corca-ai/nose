use super::*;

#[allow(clippy::too_many_arguments)]
pub(in crate::lower) fn post_lower_library_api_evidence_with_pack_id(
    il: &mut Il,
    call: NodeId,
    id: LibraryApiContractId,
    callee: LibraryApiCalleeContract,
    arg_count: usize,
    pack_id: &str,
    rule: &str,
    dependencies: Vec<EvidenceId>,
) -> EvidenceId {
    il.find_or_push_builtin_evidence(
        EvidenceAnchor::node(il.node(call).span, NodeKind::Call),
        EvidenceKind::LibraryApi(LibraryApiEvidenceKind::Contract {
            contract_hash: library_api_contract_id_hash(id),
            callee_hash: library_api_callee_contract_hash(callee),
            arity: arg_count as u16,
        }),
        pack_id,
        rule,
        dependencies,
    )
}

#[allow(clippy::too_many_arguments)]
pub(in crate::lower) fn post_lower_library_api_node_evidence_with_pack_id(
    il: &mut Il,
    node: NodeId,
    id: LibraryApiContractId,
    callee: LibraryApiCalleeContract,
    arg_count: usize,
    pack_id: &str,
    rule: &str,
    dependencies: Vec<EvidenceId>,
) -> EvidenceId {
    il.find_or_push_builtin_evidence(
        EvidenceAnchor::node(il.node(node).span, il.kind(node)),
        EvidenceKind::LibraryApi(LibraryApiEvidenceKind::Contract {
            contract_hash: library_api_contract_id_hash(id),
            callee_hash: library_api_callee_contract_hash(callee),
            arity: arg_count as u16,
        }),
        pack_id,
        rule,
        dependencies,
    )
}

pub(in crate::lower) fn post_lower_record_library_api_result_domain(
    il: &mut Il,
    call: NodeId,
    result_domain: Option<DomainEvidence>,
    api: EvidenceId,
) -> Option<EvidenceId> {
    result_domain.and_then(|domain| {
        post_lower_find_or_push_evidence(
            il,
            EvidenceAnchor::node(il.node(call).span, NodeKind::Call),
            EvidenceKind::Domain(domain),
            "library_api_result_domain",
            vec![api],
        )
    })
}

pub(in crate::lower) fn post_lower_record_library_api_node_result_domain(
    il: &mut Il,
    node: NodeId,
    domain: DomainEvidence,
    api: EvidenceId,
) {
    let _ = post_lower_find_or_push_evidence(
        il,
        EvidenceAnchor::node(il.node(node).span, il.kind(node)),
        EvidenceKind::Domain(domain),
        "library_api_result_domain",
        vec![api],
    );
}

pub(in crate::lower) fn post_lower_sequence_surface_evidence_id(
    il: &Il,
    node: NodeId,
    surface: SequenceSurfaceKind,
) -> Option<EvidenceId> {
    let span = il.node(node).span;
    il.evidence.iter().find_map(|record| {
        (record.anchor == EvidenceAnchor::sequence(span)
            && record.kind == EvidenceKind::SequenceSurface(surface)
            && record.status == EvidenceStatus::Asserted)
            .then_some(record.id)
    })
}
