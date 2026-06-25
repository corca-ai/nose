use super::*;

mod api_records;
mod domain;
mod surface;

pub(crate) use api_records::{
    language_core_builtin_at_call, library_api_dependency_id_for_normalized_hof,
};
pub(in crate::library_api) use api_records::{
    library_api_dependency_id_for_call, library_api_dependency_id_for_map_key_view_call,
    library_api_dependency_id_for_protocol_call,
    library_api_dependency_id_for_receiver_domain_call,
    library_api_dependency_id_for_receiver_domain_requirement,
};
pub use api_records::{
    library_api_dependency_id_for_canonical_builtin_call,
    library_api_dependency_id_for_canonical_builtin_call_with_interner,
    library_api_dependency_id_for_canonical_builtin_method_call,
    library_api_dependency_id_for_canonical_builtin_method_call_with_interner,
};
pub(in crate::library_api) use domain::{
    domain_dependency_anchor_matches_receiver, domain_dependency_id_for_receiver_requirement,
    domain_or_sequence_dependency_ids,
};
pub(in crate::library_api) use surface::{
    sequence_surface_dependency_id_for_receiver, static_index_membership_receiver_dependency_id,
    static_index_membership_receiver_dependency_id_at_span,
};

pub fn library_api_receiver_dependencies_for_call(
    il: &Il,
    interner: &Interner,
    call: NodeId,
    callee: LibraryApiCalleeContract,
) -> Option<Vec<EvidenceId>> {
    let mut cache = LibraryApiDependencyCache::default();
    library_api_receiver_dependencies_for_call_with_cache(il, interner, call, callee, &mut cache)
}

#[derive(Default)]
pub struct LibraryApiDependencyCache {
    binding_lhs_by_reference: FxHashMap<NodeId, EvidenceResolution<NodeId>>,
    receiver_param_span_by_reference: FxHashMap<NodeId, Option<Span>>,
    name_assigned_in_scope: FxHashMap<(NodeId, Symbol), bool>,
}

pub fn library_api_receiver_dependencies_for_call_with_cache(
    il: &Il,
    interner: &Interner,
    call: NodeId,
    callee: LibraryApiCalleeContract,
    cache: &mut LibraryApiDependencyCache,
) -> Option<Vec<EvidenceId>> {
    let (&callee_node, args) = il.children(call).split_first()?;
    match callee {
        LibraryApiCalleeContract::Method { method, receiver } => {
            let receiver_node = method_callee_receiver(il, interner, callee_node, method)?;
            let mut dependencies =
                method_receiver_dependency_ids(il, interner, receiver_node, receiver, args, cache)?;
            if receiver == MethodReceiverContract::UnshadowedGlobal("Math") {
                dependencies.extend(integer_value_argument_dependency_ids(
                    il, interner, args, cache,
                )?);
            }
            Some(dependencies)
        }
        LibraryApiCalleeContract::IteratorAdapterMethod { method, receiver } => {
            let receiver_node = method_callee_receiver(il, interner, callee_node, method)?;
            iterator_adapter_receiver_dependency_ids(il, interner, receiver_node, receiver, cache)
        }
        LibraryApiCalleeContract::AsyncMethod { method, receiver } => {
            let receiver_node = method_callee_receiver(il, interner, callee_node, method)?;
            async_receiver_dependency_ids(il, interner, receiver_node, receiver, cache)
        }
        LibraryApiCalleeContract::StaticIndexMembershipMethod { method, receiver } => {
            let receiver_node = method_callee_receiver(il, interner, callee_node, method)?;
            static_index_membership_receiver_dependency_id(il, interner, receiver_node, receiver)
                .map(|dependency| vec![dependency])
        }
        _ => Some(Vec::new()),
    }
}

pub fn proven_receiver_method_api_contract_for_call_with_cache(
    il: &mut Il,
    interner: &Interner,
    call: NodeId,
    cache: &mut LibraryApiDependencyCache,
    mut seed_dependencies: impl FnMut(&mut Il, &Interner, NodeId, LibraryApiCalleeContract),
) -> Option<(usize, LibraryReceiverMethodApiContract, Vec<EvidenceId>)> {
    let (callee_node, method, arg_count) = {
        let kids = il.children(call);
        let (&callee_node, args) = kids.split_first()?;
        if il.kind(callee_node) != NodeKind::Field {
            return None;
        }
        let Payload::Name(method) = il.node(callee_node).payload else {
            return None;
        };
        (callee_node, interner.resolve(method), args.len())
    };
    for contract in library_receiver_method_api_contracts(il.meta.lang, method, arg_count) {
        seed_dependencies(il, interner, callee_node, contract.callee);
        if let Some(dependencies) = library_api_receiver_dependencies_for_call_with_cache(
            il,
            interner,
            call,
            contract.callee,
            cache,
        ) {
            return Some((arg_count, contract, dependencies));
        }
    }
    None
}

pub fn library_api_property_dependencies_for_field_with_cache(
    il: &Il,
    interner: &Interner,
    field: NodeId,
    callee: LibraryApiCalleeContract,
    cache: &mut LibraryApiDependencyCache,
) -> Option<Vec<EvidenceId>> {
    let LibraryApiCalleeContract::Property { property, receiver } = callee else {
        return None;
    };
    if !field_method_matches(il, interner, field, property) {
        return None;
    }
    let receiver_node = il.children(field).first().copied()?;
    method_receiver_dependency_ids(il, interner, receiver_node, receiver, &[], cache)
}

fn integer_value_argument_dependency_ids(
    il: &Il,
    interner: &Interner,
    args: &[NodeId],
    cache: &mut LibraryApiDependencyCache,
) -> Option<Vec<EvidenceId>> {
    let mut dependencies = Vec::new();
    for &arg in args {
        if matches!(il.node(arg).payload, Payload::LitInt(_)) {
            continue;
        }
        let dependency = domain_dependency_id_for_receiver_requirement(
            il,
            interner,
            arg,
            DomainRequirement::INTEGER,
            cache,
        )
        .or_else(|| {
            library_api_dependency_id_for_receiver_domain_requirement(
                il,
                interner,
                arg,
                DomainRequirement::INTEGER,
            )
        })?;
        dependencies.push(dependency);
    }
    Some(dependencies)
}

pub(in crate::library_api) fn method_receiver_dependency_ids(
    il: &Il,
    interner: &Interner,
    receiver: NodeId,
    contract: MethodReceiverContract,
    args: &[NodeId],
    cache: &mut LibraryApiDependencyCache,
) -> Option<Vec<EvidenceId>> {
    let mut dependencies = receiver_dependency_ids(il, interner, receiver, contract, cache)?;
    if contract == MethodReceiverContract::ExactProtocolPairArgument {
        let pair = *args.first()?;
        dependencies.extend(receiver_dependency_ids(
            il,
            interner,
            pair,
            MethodReceiverContract::ExactProtocol,
            cache,
        )?);
    }
    Some(dependencies)
}

pub(in crate::library_api) fn iterator_adapter_receiver_dependency_ids(
    il: &Il,
    interner: &Interner,
    receiver: NodeId,
    contract: IteratorAdapterReceiverContract,
    cache: &mut LibraryApiDependencyCache,
) -> Option<Vec<EvidenceId>> {
    match contract {
        IteratorAdapterReceiverContract::ExactIterableValue => receiver_dependency_ids(
            il,
            interner,
            receiver,
            MethodReceiverContract::ExactProtocol,
            cache,
        ),
    }
}

pub(in crate::library_api) fn async_receiver_dependency_ids(
    il: &Il,
    interner: &Interner,
    receiver: NodeId,
    contract: AsyncReceiverContract,
    cache: &mut LibraryApiDependencyCache,
) -> Option<Vec<EvidenceId>> {
    match contract {
        AsyncReceiverContract::ExactPromiseLike => domain_dependency_id_for_receiver_requirement(
            il,
            interner,
            receiver,
            DomainRequirement::PROMISE_LIKE,
            cache,
        )
        .or_else(|| {
            library_api_dependency_id_for_receiver_domain_requirement(
                il,
                interner,
                receiver,
                DomainRequirement::PROMISE_LIKE,
            )
        })
        .map(|id| vec![id]),
    }
}

pub(in crate::library_api) fn method_receiver_dependencies_at_span(
    il: &Il,
    interner: &Interner,
    receiver_span: Span,
    contract: MethodReceiverContract,
) -> Option<Vec<EvidenceId>> {
    let receiver = node_at_span(il, receiver_span)?;
    let mut cache = LibraryApiDependencyCache::default();
    receiver_dependency_ids(il, interner, receiver, contract, &mut cache)
}

pub(in crate::library_api) fn iterator_adapter_receiver_dependencies_at_span(
    il: &Il,
    interner: &Interner,
    receiver_span: Span,
    contract: IteratorAdapterReceiverContract,
) -> Option<Vec<EvidenceId>> {
    let receiver = node_at_span(il, receiver_span)?;
    let mut cache = LibraryApiDependencyCache::default();
    iterator_adapter_receiver_dependency_ids(il, interner, receiver, contract, &mut cache)
}

pub(in crate::library_api) fn async_receiver_dependencies_at_span(
    il: &Il,
    interner: &Interner,
    receiver_span: Span,
    contract: AsyncReceiverContract,
) -> Option<Vec<EvidenceId>> {
    let receiver = node_at_span(il, receiver_span)?;
    let mut cache = LibraryApiDependencyCache::default();
    async_receiver_dependency_ids(il, interner, receiver, contract, &mut cache)
}

pub(in crate::library_api) fn receiver_dependency_ids(
    il: &Il,
    interner: &Interner,
    receiver: NodeId,
    contract: MethodReceiverContract,
    cache: &mut LibraryApiDependencyCache,
) -> Option<Vec<EvidenceId>> {
    match contract {
        MethodReceiverContract::LiteralString => {
            matches!(il.node(receiver).payload, Payload::LitStr(_)).then_some(Vec::new())
        }
        MethodReceiverContract::UnshadowedGlobal(global) => {
            Some(vec![symbol_dependency_id_for_node(
                il,
                receiver,
                SymbolEvidenceKind::UnshadowedGlobal {
                    name_hash: stable_symbol_hash(global),
                },
            )?])
        }
        MethodReceiverContract::ImportedNamespace(module) => {
            Some(vec![imported_symbol_dependency_id_for_node(
                il,
                interner,
                receiver,
                SymbolEvidenceKind::ImportedNamespace {
                    module_hash: stable_symbol_hash(module),
                },
            )?])
        }
        MethodReceiverContract::ExactMapLiteral => {
            Some(vec![sequence_surface_dependency_id_for_receiver(
                il, interner, receiver, contract,
            )?])
        }
        MethodReceiverContract::ExactCollectionOrMapLiteral => {
            domain_or_sequence_dependency_ids(il, interner, receiver, contract, cache)
        }
        MethodReceiverContract::ExactArrayOrCollection
        | MethodReceiverContract::ExactCollection
        | MethodReceiverContract::ExactCollectionOrMap => {
            collection_receiver_dependency_ids(il, interner, receiver, contract, cache)
        }
        MethodReceiverContract::RustMapGetOrExactOption => {
            if let Some(ids) =
                domain_or_sequence_dependency_ids(il, interner, receiver, contract, cache)
            {
                return Some(ids);
            }
            library_api_dependency_id_for_call(il, interner, receiver, LibraryApiContractId::MapGet)
                .map(|id| vec![id])
        }
        MethodReceiverContract::ExactCollectionOrJavaKeySet => {
            if let Some(ids) =
                domain_or_sequence_dependency_ids(il, interner, receiver, contract, cache)
            {
                return Some(ids);
            }
            if let Some(id) = library_api_dependency_id_for_call(
                il,
                interner,
                receiver,
                LibraryApiContractId::MapKeyView(MapKeyViewKind::Collection),
            ) {
                return Some(vec![id]);
            }
            library_api_dependency_id_for_receiver_domain_call(il, interner, receiver, contract)
                .map(|id| vec![id])
        }
        MethodReceiverContract::ExactProtocol => {
            exact_protocol_receiver_dependency_ids(il, interner, receiver, contract, cache)
        }
        MethodReceiverContract::ExactProtocolPairArgument => {
            protocol_pair_argument_receiver_dependency_ids(il, interner, receiver, cache)
        }
        _ => domain_or_sequence_dependency_ids(il, interner, receiver, contract, cache).or_else(
            || {
                library_api_dependency_id_for_receiver_domain_call(il, interner, receiver, contract)
                    .map(|id| vec![id])
            },
        ),
    }
}

pub(in crate::library_api) fn collection_receiver_dependency_ids(
    il: &Il,
    interner: &Interner,
    receiver: NodeId,
    contract: MethodReceiverContract,
    cache: &mut LibraryApiDependencyCache,
) -> Option<Vec<EvidenceId>> {
    if let Some(ids) = domain_or_sequence_dependency_ids(il, interner, receiver, contract, cache) {
        return Some(ids);
    }
    if let Some(id) =
        library_api_dependency_id_for_receiver_domain_call(il, interner, receiver, contract)
    {
        return Some(vec![id]);
    }
    library_api_dependency_id_for_map_key_view_call(
        il,
        interner,
        receiver,
        &[MapKeyViewKind::Collection],
    )
    .map(|id| vec![id])
}

pub(in crate::library_api) fn exact_protocol_receiver_dependency_ids(
    il: &Il,
    interner: &Interner,
    receiver: NodeId,
    contract: MethodReceiverContract,
    cache: &mut LibraryApiDependencyCache,
) -> Option<Vec<EvidenceId>> {
    if let Some(ids) = domain_or_sequence_dependency_ids(il, interner, receiver, contract, cache) {
        return Some(ids);
    }
    if let Some(id) = library_api_dependency_id_for_map_key_view_call(
        il,
        interner,
        receiver,
        &[MapKeyViewKind::Collection, MapKeyViewKind::Iterator],
    ) {
        return Some(vec![id]);
    }
    if let Some(id) =
        library_api_dependency_id_for_receiver_domain_call(il, interner, receiver, contract)
    {
        return Some(vec![id]);
    }
    if let Some(id) = library_api_dependency_id_for_normalized_hof(il, Some(interner), receiver) {
        return Some(vec![id]);
    }
    library_api_dependency_id_for_protocol_call(il, interner, receiver).map(|id| vec![id])
}

pub(in crate::library_api) fn protocol_pair_argument_receiver_dependency_ids(
    il: &Il,
    interner: &Interner,
    receiver: NodeId,
    cache: &mut LibraryApiDependencyCache,
) -> Option<Vec<EvidenceId>> {
    domain_or_sequence_dependency_ids(
        il,
        interner,
        receiver,
        MethodReceiverContract::ExactProtocol,
        cache,
    )
    .or_else(|| {
        library_api_dependency_id_for_map_key_view_call(
            il,
            interner,
            receiver,
            &[MapKeyViewKind::Collection, MapKeyViewKind::Iterator],
        )
        .map(|id| vec![id])
    })
    .or_else(|| {
        library_api_dependency_id_for_receiver_domain_call(
            il,
            interner,
            receiver,
            MethodReceiverContract::ExactProtocol,
        )
        .map(|id| vec![id])
    })
    .or_else(|| {
        library_api_dependency_id_for_normalized_hof(il, Some(interner), receiver)
            .map(|id| vec![id])
    })
    .or_else(|| {
        library_api_dependency_id_for_protocol_call(il, interner, receiver).map(|id| vec![id])
    })
}

pub(in crate::library_api) fn symbol_dependency_id_for_node(
    il: &Il,
    node: NodeId,
    expected: SymbolEvidenceKind,
) -> Option<EvidenceId> {
    let anchor = EvidenceAnchor::node(il.node(node).span, il.kind(node));
    let requires_admitted_record = matches!(expected, SymbolEvidenceKind::UnshadowedGlobal { .. });
    if requires_admitted_record
        && !matches!(
            language_core_symbol_identity_at_anchor_matches(
                il,
                il.node(node).span,
                il.kind(node),
                expected
            ),
            EvidenceResolution::Found(true)
        )
    {
        return None;
    }
    il.evidence_anchored_at(anchor.span()).find_map(|record| {
        (record.anchor == anchor
            && record.status == EvidenceStatus::Asserted
            && record.kind == EvidenceKind::Symbol(expected)
            && (!requires_admitted_record || symbol_record_has_admitted_provenance(il, record))
            && il.evidence_dependencies_asserted(record))
        .then_some(record.id)
    })
}

pub(in crate::library_api) fn imported_symbol_dependency_id_for_node(
    il: &Il,
    interner: &Interner,
    node: NodeId,
    expected: SymbolEvidenceKind,
) -> Option<EvidenceId> {
    let anchor = EvidenceAnchor::node(il.node(node).span, il.kind(node));
    let requires_admitted_record = matches!(expected, SymbolEvidenceKind::ImportedNamespace { .. });
    if requires_admitted_record
        && !matches!(
            language_core_symbol_identity_at_anchor_matches(
                il,
                il.node(node).span,
                il.kind(node),
                expected
            ),
            EvidenceResolution::Found(true)
        )
    {
        return None;
    }
    il.evidence_anchored_at(anchor.span()).find_map(|record| {
        (record.anchor == anchor
            && record.status == EvidenceStatus::Asserted
            && record.kind == EvidenceKind::Symbol(expected)
            && (!requires_admitted_record || symbol_record_has_admitted_provenance(il, record))
            && (!requires_admitted_record || il.evidence_dependencies_asserted(record))
            && imported_occurrence_symbol_dependencies_valid(il, interner, record, expected))
        .then_some(record.id)
    })
}
