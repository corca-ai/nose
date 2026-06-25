use super::*;

pub(in crate::library_api) fn library_api_contract_obligations_match_call(
    il: &Il,
    interner: Option<&Interner>,
    call: NodeId,
    id: LibraryApiContractId,
    record: &EvidenceRecord,
) -> bool {
    if method_hof_callback_obligation_required(il.meta.lang, id) {
        return method_hof_callback_obligation_matches_node(il, interner, call);
    }

    let source_args = match (il.meta.lang, id) {
        (Lang::Python, LibraryApiContractId::FreeFunctionHof(HoFKind::Map | HoFKind::Filter)) => {
            &[1usize][..]
        }
        (Lang::Python, LibraryApiContractId::FreeFunctionBuiltin(Builtin::Zip)) => &[0usize, 1][..],
        (
            Lang::Python,
            LibraryApiContractId::FreeFunctionBuiltin(
                Builtin::Enumerate | Builtin::Any | Builtin::All,
            ),
        ) => &[0usize][..],
        _ => return true,
    };
    let source_arg_offset = if matches!(il.node(call).payload, Payload::Builtin(_)) {
        0
    } else {
        1
    };
    source_args.iter().all(|&arg_idx| {
        il.children(call)
            .get(arg_idx + source_arg_offset)
            .copied()
            .is_some_and(|arg| {
                library_api_record_has_iterable_source_dependency(il, interner, record, arg)
            })
    })
}

pub(in crate::library_api) fn library_api_contract_obligations_match_node(
    il: &Il,
    interner: Option<&Interner>,
    node: NodeId,
    id: LibraryApiContractId,
) -> bool {
    if method_hof_callback_obligation_required(il.meta.lang, id) {
        return method_hof_callback_obligation_matches_node(il, interner, node);
    }
    true
}

pub(in crate::library_api) fn library_api_contract_requires_call_obligations(
    lang: Lang,
    id: LibraryApiContractId,
) -> bool {
    iterable_source_obligation_required(lang, id)
        || method_hof_callback_obligation_required(lang, id)
}

fn iterable_source_obligation_required(lang: Lang, id: LibraryApiContractId) -> bool {
    matches!(
        (lang, id),
        (
            Lang::Python,
            LibraryApiContractId::FreeFunctionHof(HoFKind::Map | HoFKind::Filter)
        ) | (
            Lang::Python,
            LibraryApiContractId::FreeFunctionBuiltin(Builtin::Zip | Builtin::Enumerate)
        )
    )
}

fn method_hof_callback_obligation_required(lang: Lang, id: LibraryApiContractId) -> bool {
    match id {
        LibraryApiContractId::MethodCall(
            MethodSemanticContract::HoF(HoFKind::Map | HoFKind::Filter | HoFKind::FlatMap)
            | MethodSemanticContract::Builtin(Builtin::Any | Builtin::All),
        ) if js_like_lang(lang) => true,
        LibraryApiContractId::MethodCall(MethodSemanticContract::HoF(
            HoFKind::Map | HoFKind::Filter | HoFKind::FlatMap,
        )) if lang == Lang::Swift => true,
        _ => false,
    }
}

fn method_hof_callback_obligation_matches_node(
    il: &Il,
    interner: Option<&Interner>,
    node: NodeId,
) -> bool {
    let Some(&callback) = il.children(node).get(1) else {
        return false;
    };
    method_hof_callback_effect_closed(il, interner, callback)
}

fn method_hof_callback_effect_closed(
    il: &Il,
    interner: Option<&Interner>,
    callback: NodeId,
) -> bool {
    if !matches!(il.kind(callback), NodeKind::Func | NodeKind::Lambda) {
        return false;
    }
    let mut stack = vec![callback];
    while let Some(node) = stack.pop() {
        if il.kind(node) == NodeKind::Call {
            if !method_hof_callback_nested_call_effect_closed(il, interner, node) {
                return false;
            }
            continue;
        }
        if il.kind(node) == NodeKind::HoF {
            if library_api_dependency_id_for_normalized_hof(il, interner, node).is_none() {
                return false;
            }
            continue;
        }
        if !method_hof_callback_node_effect_closed(il.kind(node)) {
            return false;
        }
        stack.extend(il.children(node).iter().copied());
    }
    true
}

fn method_hof_callback_node_effect_closed(kind: NodeKind) -> bool {
    matches!(
        kind,
        NodeKind::Func
            | NodeKind::Lambda
            | NodeKind::Param
            | NodeKind::Block
            | NodeKind::Return
            | NodeKind::If
            | NodeKind::Var
            | NodeKind::Lit
            | NodeKind::BinOp
            | NodeKind::UnOp
            | NodeKind::Seq
    )
}

fn method_hof_callback_nested_call_effect_closed(
    il: &Il,
    interner: Option<&Interner>,
    call: NodeId,
) -> bool {
    let Some(interner) = interner else {
        return false;
    };
    let Some(&callee) = il.children(call).first() else {
        return false;
    };
    let NodeKind::Field = il.kind(callee) else {
        return false;
    };
    let Payload::Name(method) = il.node(callee).payload else {
        return false;
    };
    let arg_count = il.children(call).len().saturating_sub(1);
    library_method_call_contracts(il.meta.lang, interner.resolve(method), arg_count)
        .into_iter()
        .filter(|contract| method_hof_callback_nested_method_call(il.meta.lang, contract.result))
        .any(|contract| {
            matches!(
                library_api_contract_evidence_for_call(
                    il,
                    interner,
                    call,
                    contract.id,
                    contract.callee,
                    arg_count,
                ),
                LibraryApiEvidenceStatus::Admitted
            )
        })
}

fn method_hof_callback_nested_method_call(lang: Lang, contract: MethodCallContract) -> bool {
    js_like_array_hof_method_call(lang, contract) || swift_sequence_hof_method_call(lang, contract)
}

fn library_api_record_has_iterable_source_dependency(
    il: &Il,
    interner: Option<&Interner>,
    record: &EvidenceRecord,
    source: NodeId,
) -> bool {
    record.dependencies.iter().copied().any(|dependency_id| {
        let Some(dependency) = il.evidence_record_by_id(dependency_id) else {
            return false;
        };
        dependency.status == EvidenceStatus::Asserted
            && il.evidence_dependencies_asserted(dependency)
            && dependency_proves_iterable_source(il, interner, dependency, source)
    })
}

fn dependency_proves_iterable_source(
    il: &Il,
    interner: Option<&Interner>,
    dependency: &EvidenceRecord,
    source: NodeId,
) -> bool {
    let source_span = il.node(source).span;
    match dependency.kind {
        EvidenceKind::Domain(domain) => {
            (domain.is_iterable_or_iterator() || domain.is_array_collection_or_set())
                && dependency_domain_anchor_matches_source(il, interner, dependency.anchor, source)
        }
        EvidenceKind::SequenceSurface(SequenceSurfaceKind::Collection) => {
            dependency.anchor == EvidenceAnchor::sequence(source_span)
                && evidence_record_has_language_core_provenance(il, dependency)
        }
        EvidenceKind::Source(SourceFactKind::Comprehension(kind)) => {
            dependency.anchor == EvidenceAnchor::source_span(source_span)
                && source_comprehension_is_iterable(il.meta.lang, kind)
                && evidence_record_has_language_source_fact_provenance(il, dependency)
        }
        EvidenceKind::LibraryApi(LibraryApiEvidenceKind::Contract {
            contract_hash,
            callee_hash,
            arity,
        }) if dependency.anchor == EvidenceAnchor::node(source_span, NodeKind::Call) => {
            let Some(id) = library_api_contract_id_from_hash(contract_hash) else {
                return false;
            };
            let Some(callee) = library_api_callee_contract_for_hash(il.meta.lang, id, callee_hash)
            else {
                return false;
            };
            library_api_source_dependency_contract_is_iterable(
                il, interner, id, callee, arity, dependency,
            )
        }
        _ => false,
    }
}

fn source_comprehension_is_iterable(lang: Lang, kind: SourceComprehensionKind) -> bool {
    matches!(
        (lang, kind),
        (
            Lang::Python,
            SourceComprehensionKind::PythonGeneratorExpression
                | SourceComprehensionKind::PythonListComprehension
        )
    )
}

fn evidence_record_has_language_core_provenance(il: &Il, record: &EvidenceRecord) -> bool {
    let (pack_id, producer_id) = language_core_evidence_provenance(il.meta.lang);
    library_api_record_has_builtin_provenance(record, pack_id, producer_id)
}

fn evidence_record_has_language_source_fact_provenance(il: &Il, record: &EvidenceRecord) -> bool {
    let (pack_id, producer_id) = language_source_fact_provenance(il.meta.lang);
    library_api_record_has_builtin_provenance(record, pack_id, producer_id)
}

fn dependency_domain_anchor_matches_source(
    il: &Il,
    interner: Option<&Interner>,
    anchor: EvidenceAnchor,
    source: NodeId,
) -> bool {
    if anchor == EvidenceAnchor::node(il.node(source).span, il.kind(source)) {
        return true;
    }
    let Some(interner) = interner else {
        return param_anchor_matches_source_cid(il, anchor, source);
    };
    let mut cache = LibraryApiDependencyCache::default();
    domain_dependency_anchor_matches_receiver(il, interner, source, anchor, &mut cache)
}

fn param_anchor_matches_source_cid(il: &Il, anchor: EvidenceAnchor, source: NodeId) -> bool {
    let EvidenceAnchor::Param { span } = anchor else {
        return false;
    };
    let Payload::Cid(cid) = il.node(source).payload else {
        return false;
    };
    il.nodes.iter().any(|node| {
        node.kind == NodeKind::Param
            && node.span == span
            && matches!(node.payload, Payload::Cid(param_cid) if param_cid == cid)
    })
}

fn library_api_source_dependency_contract_is_iterable(
    il: &Il,
    interner: Option<&Interner>,
    id: LibraryApiContractId,
    callee: LibraryApiCalleeContract,
    arity: u16,
    record: &EvidenceRecord,
) -> bool {
    match id {
        LibraryApiContractId::FreeFunctionHof(HoFKind::Map | HoFKind::Filter)
        | LibraryApiContractId::FreeFunctionBuiltin(Builtin::Zip | Builtin::Enumerate)
        | LibraryApiContractId::IteratorIdentityAdapter
        | LibraryApiContractId::StaticCollectionAdapter
        | LibraryApiContractId::MethodCall(
            MethodSemanticContract::HoF(_) | MethodSemanticContract::Builtin(Builtin::Zip),
        ) => {
            library_api_record_provenance_matches_contract(il.meta.lang, id, callee, record)
                && library_api_callee_contract_for_hash(
                    il.meta.lang,
                    id,
                    library_api_callee_contract_hash(callee),
                )
                .is_some()
                && arity < u16::MAX
                && library_api_source_dependency_obligations_match(il, interner, id, record)
        }
        _ => false,
    }
}

fn library_api_source_dependency_obligations_match(
    il: &Il,
    interner: Option<&Interner>,
    id: LibraryApiContractId,
    record: &EvidenceRecord,
) -> bool {
    if !library_api_contract_requires_call_obligations(il.meta.lang, id) {
        return true;
    }
    node_at_span_with_kind(il, record.anchor.span(), NodeKind::Call).is_some_and(|call| {
        library_api_contract_obligations_match_call(il, interner, call, id, record)
    })
}
