use super::*;

pub(super) fn post_lower_free_function_hof_api_contract(
    lang: Lang,
    callee_name: &str,
    arg_count: usize,
) -> Option<PostLowerLibraryApiContract> {
    library_free_function_hof_contract(lang, callee_name, arg_count).map(|contract| {
        PostLowerLibraryApiContract {
            id: contract.id,
            callee: contract.callee,
            pack_id: contract.pack_id,
            rule: PYTHON_ITERATOR_BUILTIN_PROTOCOL_PRODUCER_ID,
            result_domain: Some(DomainEvidence::Iterator),
        }
    })
}

pub(super) fn post_lower_free_function_builtin_api_contract(
    lang: Lang,
    callee_name: &str,
    arg_count: usize,
) -> Option<PostLowerLibraryApiContract> {
    library_free_function_builtin_contract(lang, callee_name, arg_count).map(|contract| {
        let (pack_id, rule, result_domain) =
            post_lower_free_function_builtin_pack_and_domain(lang, contract.id);
        PostLowerLibraryApiContract {
            id: contract.id,
            callee: contract.callee,
            pack_id,
            rule,
            result_domain,
        }
    })
}

pub(super) fn post_lower_add_iterator_source_dependencies(
    il: &Il,
    interner: &Interner,
    args: &[NodeId],
    id: LibraryApiContractId,
    dependencies: &mut Vec<EvidenceId>,
) -> bool {
    let Some(source_args) = library_api_contract_iterable_source_argument_indices(il.meta.lang, id)
    else {
        return true;
    };
    for &arg_idx in source_args {
        let Some(&source) = args.get(arg_idx) else {
            return false;
        };
        let Some(dependency) = post_lower_iterable_source_dependency_id(il, interner, source)
        else {
            return false;
        };
        dependencies.push(dependency);
    }
    true
}

fn post_lower_free_function_builtin_pack_and_domain(
    lang: Lang,
    id: LibraryApiContractId,
) -> (&'static str, &'static str, Option<DomainEvidence>) {
    match (lang, id) {
        (
            Lang::Python,
            LibraryApiContractId::FreeFunctionBuiltin(Builtin::Zip | Builtin::Enumerate),
        ) => (
            PYTHON_ITERATOR_BUILTIN_PROTOCOL_PACK_ID,
            PYTHON_ITERATOR_BUILTIN_PROTOCOL_PRODUCER_ID,
            Some(DomainEvidence::Iterator),
        ),
        (Lang::Python, LibraryApiContractId::FreeFunctionBuiltin(Builtin::Any | Builtin::All)) => (
            PYTHON_ITERATOR_BUILTIN_PROTOCOL_PACK_ID,
            PYTHON_ITERATOR_BUILTIN_PROTOCOL_PRODUCER_ID,
            None,
        ),
        _ => (
            FREE_FUNCTION_BUILTIN_PROTOCOL_PACK_ID,
            FREE_FUNCTION_BUILTIN_PROTOCOL_PRODUCER_ID,
            None,
        ),
    }
}

fn post_lower_iterable_source_dependency_id(
    il: &Il,
    interner: &Interner,
    source: NodeId,
) -> Option<EvidenceId> {
    let span = il.node(source).span;
    let node_anchor = EvidenceAnchor::node(span, il.kind(source));
    let sequence_anchor = EvidenceAnchor::sequence(span);
    il.evidence.iter().find_map(|record| {
        if record.status != EvidenceStatus::Asserted {
            return None;
        }
        match record.kind {
            EvidenceKind::Domain(domain)
                if record.anchor == node_anchor
                    && post_lower_record_has_language_core_provenance(il, record)
                    && (domain.is_iterable_or_iterator()
                        || domain.is_array_collection_or_set()) =>
            {
                Some(record.id)
            }
            EvidenceKind::SequenceSurface(SequenceSurfaceKind::Collection)
                if record.anchor == sequence_anchor
                    && post_lower_record_has_language_core_provenance(il, record) =>
            {
                Some(record.id)
            }
            EvidenceKind::Source(SourceFactKind::Comprehension(kind))
                if record.anchor == EvidenceAnchor::source_span(span)
                    && post_lower_source_comprehension_is_iterable(il.meta.lang, kind)
                    && post_lower_record_has_language_source_fact_provenance(il, record) =>
            {
                Some(record.id)
            }
            EvidenceKind::LibraryApi(LibraryApiEvidenceKind::Contract {
                contract_hash,
                callee_hash,
                arity,
            }) if record.anchor == node_anchor
                && post_lower_library_api_contract_is_iterable(
                    il,
                    interner,
                    source,
                    record,
                    contract_hash,
                    callee_hash,
                    arity,
                ) =>
            {
                Some(record.id)
            }
            _ => {
                if il.kind(source) != NodeKind::Var {
                    return None;
                }
                post_lower_param_domain_dependency_matches(il, record, source)
            }
        }
    })
}

fn post_lower_source_comprehension_is_iterable(lang: Lang, kind: SourceComprehensionKind) -> bool {
    matches!(
        (lang, kind),
        (
            Lang::Python,
            SourceComprehensionKind::PythonGeneratorExpression
                | SourceComprehensionKind::PythonListComprehension
        )
    )
}

fn post_lower_record_has_language_core_provenance(il: &Il, record: &EvidenceRecord) -> bool {
    let (pack_id, producer_id) = language_core_evidence_provenance(il.meta.lang);
    record.provenance.emitter == EvidenceEmitter::Builtin
        && record.provenance.pack_hash == Some(stable_symbol_hash(pack_id))
        && record.provenance.rule_hash == Some(stable_symbol_hash(producer_id))
}

fn post_lower_record_has_language_source_fact_provenance(il: &Il, record: &EvidenceRecord) -> bool {
    let (pack_id, producer_id) = language_source_fact_provenance(il.meta.lang);
    record.provenance.emitter == EvidenceEmitter::Builtin
        && record.provenance.pack_hash == Some(stable_symbol_hash(pack_id))
        && record.provenance.rule_hash == Some(stable_symbol_hash(producer_id))
}

fn post_lower_param_domain_dependency_matches(
    il: &Il,
    record: &EvidenceRecord,
    source: NodeId,
) -> Option<EvidenceId> {
    let EvidenceKind::Domain(domain) = record.kind else {
        return None;
    };
    if !(domain.is_iterable_or_iterator() || domain.is_array_collection_or_set()) {
        return None;
    }
    let EvidenceAnchor::Param { span } = record.anchor else {
        return None;
    };
    let source_payload = il.node(source).payload;
    let source_unit = post_lower_enclosing_unit_root(il, source)?;
    if post_lower_subtree_writes_binding(il, source_unit, source_payload) {
        return None;
    }
    il.nodes
        .iter()
        .enumerate()
        .any(|(idx, node)| {
            node.kind == NodeKind::Param
                && node.span == span
                && post_lower_same_binding_payload(node.payload, source_payload)
                && post_lower_enclosing_unit_root(il, NodeId(idx as u32)) == Some(source_unit)
        })
        .then_some(record.id)
}

fn post_lower_library_api_contract_is_iterable(
    il: &Il,
    interner: &Interner,
    source: NodeId,
    record: &EvidenceRecord,
    contract_hash: u64,
    callee_hash: u64,
    arity: u16,
) -> bool {
    if il.meta.lang != Lang::Python
        || arity == u16::MAX
        || !post_lower_python_iterator_record_provenance_matches(record)
    {
        return false;
    }
    post_lower_python_iterator_producer_contracts()
        .into_iter()
        .any(|(id, callee)| {
            library_api_contract_id_hash(id) == contract_hash
                && library_api_callee_contract_hash(callee) == callee_hash
                && matches!(
                    library_api_contract_evidence_for_call(
                        il,
                        interner,
                        source,
                        id,
                        callee,
                        arity as usize,
                    ),
                    LibraryApiEvidenceStatus::Admitted
                )
        })
}

fn post_lower_python_iterator_record_provenance_matches(record: &EvidenceRecord) -> bool {
    record.provenance.emitter == EvidenceEmitter::Builtin
        && record.provenance.pack_hash
            == Some(stable_symbol_hash(PYTHON_ITERATOR_BUILTIN_PROTOCOL_PACK_ID))
        && record.provenance.rule_hash
            == Some(stable_symbol_hash(
                PYTHON_ITERATOR_BUILTIN_PROTOCOL_PRODUCER_ID,
            ))
}

fn post_lower_python_iterator_producer_contracts(
) -> Vec<(LibraryApiContractId, LibraryApiCalleeContract)> {
    let mut contracts = Vec::new();
    for (name, arity) in [("map", 2), ("filter", 2)] {
        if let Some(contract) = library_free_function_hof_contract(Lang::Python, name, arity) {
            contracts.push((contract.id, contract.callee));
        }
    }
    for (name, arity) in [("zip", 2), ("enumerate", 1)] {
        if let Some(contract) = library_free_function_builtin_contract(Lang::Python, name, arity) {
            contracts.push((contract.id, contract.callee));
        }
    }
    contracts
}

fn post_lower_subtree_writes_binding(il: &Il, root: NodeId, payload: Payload) -> bool {
    if binding_write_target(il, root)
        .is_some_and(|target| post_lower_same_binding_payload(il.node(target).payload, payload))
    {
        return true;
    }
    il.children(root)
        .iter()
        .any(|&child| post_lower_subtree_writes_binding(il, child, payload))
}

fn post_lower_same_binding_payload(param: Payload, source: Payload) -> bool {
    match (param, source) {
        (Payload::Cid(param_cid), Payload::Cid(source_cid)) => param_cid == source_cid,
        (Payload::Name(param_name), Payload::Name(source_name)) => param_name == source_name,
        _ => false,
    }
}

fn post_lower_enclosing_unit_root(il: &Il, node: NodeId) -> Option<NodeId> {
    il.units
        .iter()
        .find_map(|unit| post_lower_subtree_contains(il, unit.root, node).then_some(unit.root))
}

fn post_lower_subtree_contains(il: &Il, root: NodeId, target: NodeId) -> bool {
    root == target
        || il
            .children(root)
            .iter()
            .any(|&child| post_lower_subtree_contains(il, child, target))
}
