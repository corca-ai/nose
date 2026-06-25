use super::canonical::{
    canonical_record_has_unshadowed_symbol_dependency, library_api_method_call_record_contract,
};
use super::protocol_receivers::library_api_dependency_contract_satisfies_protocol_receiver;
use super::*;

pub(super) fn method_call_receiver_dependencies_match(
    il: &Il,
    interner: Option<&Interner>,
    imported_occurrence_cache: &mut ImportedOccurrenceValidationCache,
    call: NodeId,
    record: &EvidenceRecord,
    contract: LibraryMethodCallContract,
) -> bool {
    match contract.result.receiver {
        MethodReceiverContract::UnshadowedGlobal(name) => {
            canonical_record_has_unshadowed_symbol_dependency(il, call, record, name)
        }
        MethodReceiverContract::ImportedNamespace(module) => {
            let Some(interner) = interner else {
                return false;
            };
            canonical_record_has_imported_namespace_dependency(
                il,
                interner,
                imported_occurrence_cache,
                call,
                record,
                module,
            )
        }
        receiver => {
            let Some(receiver_node) =
                canonical_method_receiver_node(il, call, contract.result.args)
            else {
                return false;
            };
            if !receiver_contract_dependency_match(il, interner, record, receiver_node, receiver) {
                return false;
            }
            if receiver == MethodReceiverContract::ExactProtocolPairArgument {
                let Some(&pair) = il.children(call).get(1) else {
                    return false;
                };
                return receiver_contract_dependency_match(
                    il,
                    interner,
                    record,
                    pair,
                    MethodReceiverContract::ExactProtocol,
                );
            }
            true
        }
    }
}

pub(super) fn canonical_method_receiver_node(
    il: &Il,
    call: NodeId,
    args: MethodBuiltinArgs,
) -> Option<NodeId> {
    match args {
        MethodBuiltinArgs::All | MethodBuiltinArgs::First | MethodBuiltinArgs::GoSliceContains => {
            None
        }
        MethodBuiltinArgs::FirstThenReceiver => il.children(call).get(1).copied(),
        MethodBuiltinArgs::Fold => il.children(call).get(1).copied(),
        MethodBuiltinArgs::ReceiverOnly
        | MethodBuiltinArgs::ReceiverThenAll
        | MethodBuiltinArgs::ReceiverAndFirst
        | MethodBuiltinArgs::MapGetDefault
        | MethodBuiltinArgs::MapGetDefaultOrZeroArgLambda
        | MethodBuiltinArgs::RustMapGetOrOptionDefault
        | MethodBuiltinArgs::RustOptionDefaultLambda
        | MethodBuiltinArgs::RustOptionMapOrIdentity
        | MethodBuiltinArgs::RustZip
        | MethodBuiltinArgs::BoolReduction
        | MethodBuiltinArgs::Hof
        | MethodBuiltinArgs::CollectionReduction => il.children(call).first().copied(),
    }
}

pub(super) fn receiver_contract_dependency_match(
    il: &Il,
    interner: Option<&Interner>,
    record: &EvidenceRecord,
    receiver: NodeId,
    contract: MethodReceiverContract,
) -> bool {
    if contract == MethodReceiverContract::LiteralString
        && matches!(il.node(receiver).payload, Payload::LitStr(_))
    {
        return true;
    }
    if contract == MethodReceiverContract::RustMapGetOrExactOption
        && evidence_depends_on_library_api_contract(
            il,
            record,
            LibraryApiContractId::MapGet,
            Some(receiver),
        )
    {
        return true;
    }
    if matches!(il.node(receiver).payload, Payload::HoF(_))
        && library_api_record_depends_on_normalized_hof(il, interner, record, receiver)
    {
        return true;
    }
    if let Some(requirement) = method_receiver_domain_requirement(contract) {
        return library_api_record_depends_on_receiver_requirement(
            il,
            record,
            receiver,
            requirement,
        ) || library_api_record_depends_on_receiver_sequence_surface(
            il, record, receiver, contract,
        ) || library_api_record_depends_on_receiver_result_domain(
            il,
            interner,
            record,
            receiver,
            requirement,
        ) || library_api_record_depends_on_receiver_protocol_api_depth(
            il, interner, record, receiver, contract, 4,
        );
    }
    contract == MethodReceiverContract::ExactMapLiteral
        && library_api_record_depends_on_receiver_sequence_surface(il, record, receiver, contract)
}

pub(super) fn library_api_record_depends_on_normalized_hof(
    il: &Il,
    interner: Option<&Interner>,
    record: &EvidenceRecord,
    receiver: NodeId,
) -> bool {
    library_api_dependency_id_for_normalized_hof(il, interner, receiver)
        .is_some_and(|id| record.dependencies.contains(&id))
}

pub(super) fn canonical_record_has_imported_namespace_dependency(
    il: &Il,
    interner: &Interner,
    imported_occurrence_cache: &mut ImportedOccurrenceValidationCache,
    call: NodeId,
    record: &EvidenceRecord,
    module: &str,
) -> bool {
    let call_span = il.node(call).span;
    let expected = SymbolEvidenceKind::ImportedNamespace {
        module_hash: stable_symbol_hash(module),
    };
    record.dependencies.iter().any(|&id| {
        let Some(dependency) = il.evidence_record_by_id(id) else {
            return false;
        };
        let EvidenceAnchor::Node {
            span,
            kind: NodeKind::Var,
        } = dependency.anchor
        else {
            return false;
        };
        dependency.status == EvidenceStatus::Asserted
            && il.evidence_dependencies_asserted(dependency)
            && dependency.kind == EvidenceKind::Symbol(expected)
            && symbol_record_has_admitted_provenance(il, dependency)
            && span.file == call_span.file
            && span.start_byte == call_span.start_byte
            && span.end_byte <= call_span.end_byte
            && matches!(
                language_core_symbol_identity_at_anchor_matches(il, span, NodeKind::Var, expected),
                EvidenceResolution::Found(true)
            )
            && imported_occurrence_symbol_dependencies_valid_with_cache(
                il,
                interner,
                dependency,
                expected,
                imported_occurrence_cache,
            )
    })
}

pub(super) fn library_api_record_depends_on_receiver_result_domain(
    il: &Il,
    interner: Option<&Interner>,
    record: &EvidenceRecord,
    receiver: NodeId,
    requirement: DomainRequirement,
) -> bool {
    record.dependencies.iter().any(|&id| {
        let Some(dependency) = il.evidence_record_by_id(id) else {
            return false;
        };
        let Some((actual_id, callee, arity)) =
            asserted_library_api_dependency_contract(il, interner, dependency)
        else {
            return false;
        };
        library_api_dependency_anchor_matches_receiver(il, dependency.anchor, receiver)
            && library_api_contract_result_domain_for_arity(actual_id, callee, arity)
                .is_some_and(|domain| requirement.accepts(domain))
    })
}

pub(super) fn library_api_record_depends_on_receiver_protocol_api_depth(
    il: &Il,
    interner: Option<&Interner>,
    record: &EvidenceRecord,
    receiver: NodeId,
    contract: MethodReceiverContract,
    depth: usize,
) -> bool {
    record.dependencies.iter().any(|&id| {
        let Some(dependency) = il.evidence_record_by_id(id) else {
            return false;
        };
        let Some((actual_id, callee, arity)) =
            asserted_library_api_dependency_contract(il, interner, dependency)
        else {
            return false;
        };
        (library_api_dependency_anchor_matches_receiver(il, dependency.anchor, receiver)
            || (depth > 0
                && library_api_dependency_record_has_receiver_proof_depth(
                    il,
                    interner,
                    dependency,
                    receiver,
                    contract,
                    depth - 1,
                )))
            && library_api_dependency_contract_satisfies_protocol_receiver(
                il, actual_id, callee, arity, contract,
            )
    })
}

pub(super) fn asserted_library_api_dependency_contract(
    il: &Il,
    interner: Option<&Interner>,
    dependency: &EvidenceRecord,
) -> Option<(LibraryApiContractId, LibraryApiCalleeContract, u16)> {
    if dependency.status != EvidenceStatus::Asserted
        || !il.evidence_dependencies_asserted(dependency)
    {
        return None;
    }
    let EvidenceKind::LibraryApi(LibraryApiEvidenceKind::Contract {
        contract_hash,
        callee_hash,
        arity,
    }) = dependency.kind
    else {
        return None;
    };
    let actual_id = library_api_contract_id_from_hash(contract_hash)?;
    let callee = library_api_callee_contract_for_hash(il.meta.lang, actual_id, callee_hash)?;
    if !library_api_record_provenance_matches_contract(il.meta.lang, actual_id, callee, dependency)
    {
        return None;
    }
    if matches!(actual_id, LibraryApiContractId::MethodCall(_))
        && library_api_method_call_record_contract(il, actual_id, callee, arity).is_none()
    {
        return None;
    }
    if library_api_contract_requires_call_obligations(il.meta.lang, actual_id)
        && !node_at_span_with_kind(il, dependency.anchor.span(), NodeKind::Call).is_some_and(
            |call| {
                library_api_contract_obligations_match_call(
                    il, interner, call, actual_id, dependency,
                )
            },
        )
    {
        return None;
    }
    Some((actual_id, callee, arity))
}

pub(super) fn library_api_dependency_anchor_matches_receiver(
    il: &Il,
    anchor: EvidenceAnchor,
    receiver: NodeId,
) -> bool {
    let receiver_span = il.node(receiver).span;
    matches!(
        anchor,
        EvidenceAnchor::Node { span, .. }
            if span == receiver_span
                || (span.file == receiver_span.file
                    && span.start_byte <= receiver_span.start_byte
                    && span.end_byte >= receiver_span.end_byte)
    )
}

pub(super) fn library_api_dependency_record_has_receiver_proof_depth(
    il: &Il,
    interner: Option<&Interner>,
    dependency: &EvidenceRecord,
    receiver: NodeId,
    contract: MethodReceiverContract,
    depth: usize,
) -> bool {
    let Some(requirement) = method_receiver_domain_requirement(contract) else {
        return false;
    };
    library_api_record_depends_on_receiver_requirement(il, dependency, receiver, requirement)
        || library_api_record_depends_on_receiver_sequence_surface(
            il, dependency, receiver, contract,
        )
        || library_api_record_depends_on_receiver_result_domain(
            il,
            interner,
            dependency,
            receiver,
            requirement,
        )
        || (depth > 0
            && library_api_record_depends_on_receiver_protocol_api_depth(
                il,
                interner,
                dependency,
                receiver,
                contract,
                depth - 1,
            ))
}

pub(in crate::library_api) fn library_api_record_models_rust_map_get_default(
    il: &Il,
    call: NodeId,
    record: &EvidenceRecord,
    id: LibraryApiContractId,
    builtin: Builtin,
) -> bool {
    if il.meta.lang != Lang::Rust || builtin != Builtin::GetOrDefault {
        return false;
    }
    let EvidenceKind::LibraryApi(LibraryApiEvidenceKind::Contract {
        callee_hash, arity, ..
    }) = record.kind
    else {
        return false;
    };
    if id
        != LibraryApiContractId::MethodCall(MethodSemanticContract::Builtin(
            Builtin::ValueOrDefault,
        ))
        || arity != 1
    {
        return false;
    }
    let Some(LibraryApiCalleeContract::Method {
        receiver: MethodReceiverContract::RustMapGetOrExactOption,
        ..
    }) = library_api_callee_contract_for_hash(il.meta.lang, id, callee_hash)
    else {
        return false;
    };
    let Some(&map_receiver) = il.children(call).first() else {
        return false;
    };
    evidence_depends_on_library_api_contract(
        il,
        record,
        LibraryApiContractId::MapGet,
        Some(map_receiver),
    )
}

pub(in crate::library_api) fn evidence_depends_on_library_api_contract(
    il: &Il,
    record: &EvidenceRecord,
    expected_id: LibraryApiContractId,
    required_receiver: Option<NodeId>,
) -> bool {
    record.dependencies.iter().any(|&id| {
        let Some(dependency) = il.evidence_record_by_id(id) else {
            return false;
        };
        if dependency.status != EvidenceStatus::Asserted
            || !il.evidence_dependencies_asserted(dependency)
        {
            return false;
        }
        let EvidenceKind::LibraryApi(LibraryApiEvidenceKind::Contract {
            contract_hash,
            callee_hash,
            arity,
        }) = dependency.kind
        else {
            return false;
        };
        let Some(actual_id) = library_api_contract_id_from_hash(contract_hash) else {
            return false;
        };
        if actual_id != expected_id {
            return false;
        }
        let Some(callee) =
            library_api_callee_contract_for_hash(il.meta.lang, actual_id, callee_hash)
        else {
            return false;
        };
        library_api_record_provenance_matches_contract(il.meta.lang, actual_id, callee, dependency)
            && library_api_dependency_record_has_expected_arity(actual_id, arity)
            && library_api_dependency_record_has_required_dependencies(
                il,
                dependency,
                actual_id,
                required_receiver,
            )
    })
}

pub(super) fn library_api_dependency_record_has_expected_arity(
    id: LibraryApiContractId,
    arity: u16,
) -> bool {
    match id {
        LibraryApiContractId::MapGet => arity == 1,
        _ => true,
    }
}

pub(super) fn library_api_dependency_record_has_required_dependencies(
    il: &Il,
    record: &EvidenceRecord,
    id: LibraryApiContractId,
    required_receiver: Option<NodeId>,
) -> bool {
    match id {
        LibraryApiContractId::MapGet => required_receiver.is_some_and(|receiver| {
            library_api_record_depends_on_receiver_requirement(
                il,
                record,
                receiver,
                DomainRequirement::MAP,
            )
        }),
        _ => true,
    }
}

pub(super) fn library_api_record_depends_on_receiver_requirement(
    il: &Il,
    record: &EvidenceRecord,
    receiver: NodeId,
    requirement: DomainRequirement,
) -> bool {
    record.dependencies.iter().any(|&id| {
        let Some(dependency) = il.evidence_record_by_id(id) else {
            return false;
        };
        let EvidenceKind::Domain(domain) = dependency.kind else {
            return false;
        };
        dependency.status == EvidenceStatus::Asserted
            && il.evidence_dependencies_asserted(dependency)
            && requirement.accepts(domain)
            && domain_dependency_matches_canonical_receiver(il, dependency.anchor, receiver)
    })
}

pub(super) fn library_api_record_depends_on_receiver_sequence_surface(
    il: &Il,
    record: &EvidenceRecord,
    receiver: NodeId,
    contract: MethodReceiverContract,
) -> bool {
    if il.kind(receiver) != NodeKind::Seq {
        return false;
    }
    record.dependencies.iter().any(|&id| {
        matches!(
            sequence_surface_evidence_record_at_sequence_span(il, il.node(receiver).span),
            EvidenceResolution::Found((kind, dependency_id))
                if dependency_id == id
                    && sequence_surface_kind_satisfies_method_receiver(kind, contract)
        )
    })
}

pub(super) fn sequence_surface_kind_satisfies_method_receiver(
    kind: SequenceSurfaceKind,
    contract: MethodReceiverContract,
) -> bool {
    match contract {
        MethodReceiverContract::ExactArray => kind == SequenceSurfaceKind::Collection,
        MethodReceiverContract::ExactArrayOrCollection
        | MethodReceiverContract::ExactCollection
        | MethodReceiverContract::ExactProtocol
        | MethodReceiverContract::ExactProtocolPairArgument
        | MethodReceiverContract::ExactCollectionOrJavaKeySet => {
            kind == SequenceSurfaceKind::Collection
        }
        MethodReceiverContract::ExactMap | MethodReceiverContract::ExactMapLiteral => {
            kind == SequenceSurfaceKind::Map
        }
        MethodReceiverContract::ExactCollectionOrMap
        | MethodReceiverContract::ExactCollectionOrMapLiteral => {
            matches!(
                kind,
                SequenceSurfaceKind::Collection | SequenceSurfaceKind::Map
            )
        }
        MethodReceiverContract::ExactSetOrMap => kind == SequenceSurfaceKind::Map,
        _ => false,
    }
}

pub(super) fn domain_dependency_matches_canonical_receiver(
    il: &Il,
    anchor: EvidenceAnchor,
    receiver: NodeId,
) -> bool {
    match anchor {
        EvidenceAnchor::Node { span, kind } => {
            span == il.node(receiver).span && kind == il.kind(receiver)
        }
        EvidenceAnchor::Binding { span, .. } => {
            let Payload::Cid(cid) = il.node(receiver).payload else {
                return false;
            };
            il.nodes.iter().enumerate().any(|(idx, node)| {
                node.kind == NodeKind::Assign
                    && il
                        .children(NodeId(idx as u32))
                        .first()
                        .is_some_and(|&lhs| {
                            il.node(lhs).span == span
                                && matches!(il.node(lhs).payload, Payload::Cid(lhs_cid) if lhs_cid == cid)
                        })
            })
        }
        EvidenceAnchor::Param { span } => {
            let Payload::Cid(cid) = il.node(receiver).payload else {
                return false;
            };
            il.nodes.iter().any(|node| {
                node.kind == NodeKind::Param
                    && node.span == span
                    && matches!(node.payload, Payload::Cid(param_cid) if param_cid == cid)
            })
        }
        _ => false,
    }
}
