use super::*;

mod canonical;
mod normalized_hof;
mod receiver_proofs;

pub(crate) use canonical::language_core_builtin_at_call;
use canonical::library_api_method_call_record_contract;
pub use canonical::{
    library_api_dependency_id_for_canonical_builtin_call,
    library_api_dependency_id_for_canonical_builtin_call_with_interner,
    library_api_dependency_id_for_canonical_builtin_method_call,
    library_api_dependency_id_for_canonical_builtin_method_call_with_interner,
};
use normalized_hof::{
    normalized_hof_free_function_dependencies_match, normalized_hof_method_call_dependencies_match,
};

pub(crate) fn library_api_dependency_id_for_normalized_hof(
    il: &Il,
    receiver: NodeId,
) -> Option<EvidenceId> {
    let Payload::HoF(kind) = il.node(receiver).payload else {
        return None;
    };
    let anchor = EvidenceAnchor::node(il.node(receiver).span, NodeKind::Call);
    let mut found = None;
    for record in il.evidence_anchored_at(anchor.span()) {
        if record.anchor != anchor
            || record.status != EvidenceStatus::Asserted
            || !il.evidence_dependencies_asserted(record)
        {
            continue;
        }
        let EvidenceKind::LibraryApi(LibraryApiEvidenceKind::Contract {
            contract_hash,
            callee_hash,
            arity,
        }) = record.kind
        else {
            continue;
        };
        let Some(expected_id) = normalized_hof_contract_id_from_hash(kind, contract_hash) else {
            continue;
        };
        let Some(callee) =
            library_api_callee_contract_for_hash(il.meta.lang, expected_id, callee_hash)
        else {
            continue;
        };
        if !library_api_record_provenance_matches_contract(
            il.meta.lang,
            expected_id,
            callee,
            record,
        ) || !normalized_hof_dependencies_match(il, receiver, record, expected_id, callee, arity)
        {
            continue;
        }
        match found {
            None => found = Some(record.id),
            Some(existing) if existing == record.id => {}
            Some(_) => return None,
        }
    }
    found
}

fn normalized_hof_contract_id_from_hash(
    kind: HoFKind,
    contract_hash: u64,
) -> Option<LibraryApiContractId> {
    [
        LibraryApiContractId::MethodCall(MethodSemanticContract::HoF(kind)),
        LibraryApiContractId::FreeFunctionHof(kind),
    ]
    .into_iter()
    .find(|id| library_api_contract_id_hash(*id) == contract_hash)
}

fn normalized_hof_dependencies_match(
    il: &Il,
    hof: NodeId,
    record: &EvidenceRecord,
    id: LibraryApiContractId,
    callee: LibraryApiCalleeContract,
    arity: u16,
) -> bool {
    match id {
        LibraryApiContractId::MethodCall(MethodSemanticContract::HoF(_)) => {
            library_api_method_call_record_contract(il, id, callee, arity).is_some_and(|contract| {
                normalized_hof_method_call_dependencies_match(il, hof, record, contract)
            })
        }
        LibraryApiContractId::FreeFunctionHof(kind) => {
            library_api_free_function_hof_record_contract(il, kind, callee, arity).is_some_and(
                |contract| {
                    normalized_hof_free_function_dependencies_match(
                        il,
                        hof,
                        record,
                        contract.result.name,
                    )
                },
            )
        }
        _ => false,
    }
}

fn library_api_free_function_hof_record_contract(
    il: &Il,
    kind: HoFKind,
    callee: LibraryApiCalleeContract,
    arity: u16,
) -> Option<LibraryFreeFunctionHofContract> {
    let LibraryApiCalleeContract::FreeName { name, .. } = callee else {
        return None;
    };
    let contract = library_free_function_hof_contract(il.meta.lang, name, arity as usize)?;
    (contract.id == LibraryApiContractId::FreeFunctionHof(kind) && contract.callee == callee)
        .then_some(contract)
}

pub(in crate::library_api) fn library_api_dependency_id_for_protocol_call(
    il: &Il,
    interner: &Interner,
    call: NodeId,
) -> Option<EvidenceId> {
    if let Some(id) = library_api_dependency_id_for_call(
        il,
        interner,
        call,
        LibraryApiContractId::IteratorIdentityAdapter,
    ) {
        return Some(id);
    }
    if let Some(id) = library_api_dependency_id_for_call(
        il,
        interner,
        call,
        LibraryApiContractId::StaticCollectionAdapter,
    ) {
        return Some(id);
    }
    library_api_dependency_id_for_call_predicate(il, interner, call, |id| {
        matches!(
            id,
            LibraryApiContractId::FreeFunctionHof(_)
                | LibraryApiContractId::FreeFunctionBuiltin(Builtin::Zip | Builtin::Enumerate)
                | LibraryApiContractId::MethodCall(
                    MethodSemanticContract::HoF(_) | MethodSemanticContract::Builtin(Builtin::Zip)
                )
        )
    })
}

pub(in crate::library_api) fn library_api_dependency_id_for_receiver_domain_call(
    il: &Il,
    interner: &Interner,
    call: NodeId,
    contract: MethodReceiverContract,
) -> Option<EvidenceId> {
    let requirement = method_receiver_domain_requirement(contract)?;
    library_api_dependency_id_for_receiver_domain_requirement(il, interner, call, requirement)
}

pub(in crate::library_api) fn library_api_dependency_id_for_receiver_domain_requirement(
    il: &Il,
    interner: &Interner,
    call: NodeId,
    requirement: DomainRequirement,
) -> Option<EvidenceId> {
    library_api_dependency_id_for_call_contract(il, interner, call, |id, callee, arity| {
        library_api_contract_result_domain_for_arity(id, callee, arity)
            .is_some_and(|domain| requirement.accepts(domain))
    })
}

pub(in crate::library_api) fn library_api_dependency_id_for_call(
    il: &Il,
    interner: &Interner,
    call: NodeId,
    id: LibraryApiContractId,
) -> Option<EvidenceId> {
    library_api_dependency_id_for_call_predicate(il, interner, call, |actual| actual == id)
}

pub(in crate::library_api) fn library_api_contract_id_builtin_result(
    id: LibraryApiContractId,
) -> Option<Builtin> {
    match id {
        LibraryApiContractId::PropertyBuiltin(builtin)
        | LibraryApiContractId::FreeFunctionBuiltin(builtin) => Some(builtin),
        LibraryApiContractId::MethodCall(MethodSemanticContract::Builtin(builtin)) => Some(builtin),
        LibraryApiContractId::ScalarIntegerMethod(ScalarIntegerMethod::Abs) => Some(Builtin::Abs),
        LibraryApiContractId::ScalarIntegerMethod(ScalarIntegerMethod::Min) => Some(Builtin::Min),
        LibraryApiContractId::ScalarIntegerMethod(ScalarIntegerMethod::Max) => Some(Builtin::Max),
        _ => None,
    }
}

pub(in crate::library_api) fn library_api_dependency_id_for_map_key_view_call(
    il: &Il,
    interner: &Interner,
    call: NodeId,
    allowed: &[MapKeyViewKind],
) -> Option<EvidenceId> {
    library_api_dependency_id_for_call_predicate(
        il,
        interner,
        call,
        |id| matches!(id, LibraryApiContractId::MapKeyView(kind) if allowed.contains(&kind)),
    )
}

pub(in crate::library_api) fn library_api_dependency_id_for_call_predicate(
    il: &Il,
    interner: &Interner,
    call: NodeId,
    accepts: impl Fn(LibraryApiContractId) -> bool,
) -> Option<EvidenceId> {
    library_api_dependency_id_for_call_contract(il, interner, call, |id, _, _| accepts(id))
}

pub(in crate::library_api) fn library_api_dependency_id_for_call_contract(
    il: &Il,
    interner: &Interner,
    call: NodeId,
    accepts: impl Fn(LibraryApiContractId, LibraryApiCalleeContract, u16) -> bool,
) -> Option<EvidenceId> {
    if il.kind(call) != NodeKind::Call {
        return None;
    }
    let anchor = EvidenceAnchor::node(il.node(call).span, NodeKind::Call);
    let mut found = None;
    for record in il.evidence_anchored_at(anchor.span()) {
        if record.anchor != anchor
            || record.status != EvidenceStatus::Asserted
            || !il.evidence_dependencies_asserted(record)
        {
            continue;
        }
        let EvidenceKind::LibraryApi(LibraryApiEvidenceKind::Contract {
            contract_hash,
            callee_hash,
            arity,
        }) = record.kind
        else {
            continue;
        };
        let Some(id) = library_api_contract_id_from_hash(contract_hash) else {
            continue;
        };
        let Some(callee) = library_api_callee_contract_for_hash(il.meta.lang, id, callee_hash)
        else {
            continue;
        };
        if !accepts(id, callee, arity) {
            continue;
        }
        if !library_api_record_admitted_for_current_shape(il, interner, call, record) {
            continue;
        }
        match found {
            None => found = Some(record.id),
            Some(existing) if existing == record.id => {}
            Some(_) => return None,
        }
    }
    found
}
