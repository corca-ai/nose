use super::*;

pub(super) fn record_property_library_api(
    il: &mut Il,
    interner: &Interner,
    field: NodeId,
    dependency_cache: &mut LibraryApiDependencyCache,
) -> bool {
    if il.kind(field) != NodeKind::Field {
        return false;
    }
    let Payload::Name(property) = il.node(field).payload else {
        return false;
    };
    let Some(contract) =
        library_property_builtin_contract(il.meta.lang, interner.resolve(property))
    else {
        return false;
    };
    let Some(dependencies) = library_api_property_dependencies_for_field_with_cache(
        il,
        interner,
        field,
        contract.callee,
        dependency_cache,
    ) else {
        return false;
    };
    upsert_builtin_evidence_with_pack_id(
        il,
        EvidenceAnchor::node(il.node(field).span, NodeKind::Field),
        EvidenceKind::LibraryApi(LibraryApiEvidenceKind::Contract {
            contract_hash: library_api_contract_id_hash(contract.id),
            callee_hash: library_api_callee_contract_hash(contract.callee),
            arity: 0,
        }),
        contract.pack_id,
        nose_semantics::PROPERTY_BUILTIN_PROTOCOL_PRODUCER_ID,
        dependencies,
    );
    true
}

pub(super) fn record_rust_option_none_library_api(
    il: &mut Il,
    interner: &Interner,
    var: NodeId,
) -> bool {
    let Some(name) = node_name(il, interner, var) else {
        return false;
    };
    let Some(contract) = library_rust_option_none_sentinel_contract(il.meta.lang, name) else {
        return false;
    };
    let LibraryApiCalleeContract::FreeName { name, shadow } = contract.callee else {
        return false;
    };
    if !library_api_free_name_shadow_safe(il.meta.lang, name, shadow, |candidate| {
        file_defines_name_visible_at(il, interner, candidate, il.node(var).span)
    }) {
        return false;
    }
    let Some(symbol_dependency) = unshadowed_symbol_evidence_id(il, var, name) else {
        return false;
    };
    let api = upsert_builtin_evidence_with_pack_id(
        il,
        EvidenceAnchor::node(il.node(var).span, NodeKind::Var),
        EvidenceKind::LibraryApi(LibraryApiEvidenceKind::Contract {
            contract_hash: library_api_contract_id_hash(contract.id),
            callee_hash: library_api_callee_contract_hash(contract.callee),
            arity: 0,
        }),
        contract.pack_id,
        RUST_STDLIB_OPTION_PRODUCER_ID,
        vec![symbol_dependency],
    );
    record_library_api_node_result_domain(il, var, contract.result_domain, api);
    true
}

pub(super) fn record_receiver_method_library_api(
    il: &mut Il,
    interner: &Interner,
    call: NodeId,
    dependency_cache: &mut LibraryApiDependencyCache,
) -> bool {
    let kids = il.children(call);
    let Some((&callee, args)) = kids.split_first() else {
        return false;
    };
    if il.kind(callee) != NodeKind::Field {
        return false;
    }
    let Payload::Name(method) = il.node(callee).payload else {
        return false;
    };
    let method = interner.resolve(method);
    let arg_count = args.len();
    let Some(contract) = library_receiver_method_api_contract(il.meta.lang, method, arg_count)
    else {
        return false;
    };
    seed_receiver_method_dependencies(il, interner, callee, contract.callee);
    let Some(dependencies) = library_api_receiver_dependencies_for_call_with_cache(
        il,
        interner,
        call,
        contract.callee,
        dependency_cache,
    ) else {
        return false;
    };
    let api = upsert_builtin_evidence_with_pack_id(
        il,
        EvidenceAnchor::node(il.node(call).span, NodeKind::Call),
        EvidenceKind::LibraryApi(LibraryApiEvidenceKind::Contract {
            contract_hash: library_api_contract_id_hash(contract.id),
            callee_hash: library_api_callee_contract_hash(contract.callee),
            arity: arg_count as u16,
        }),
        contract.pack_id,
        contract.rule,
        dependencies,
    );
    if let Some(domain) = contract.result_domain {
        record_library_api_result_domain(il, call, domain, api);
    }
    true
}

pub(super) fn record_library_api_result_domain(
    il: &mut Il,
    call: NodeId,
    domain: DomainEvidence,
    api: EvidenceId,
) {
    upsert_language_core_evidence(
        il,
        EvidenceAnchor::node(il.node(call).span, NodeKind::Call),
        EvidenceKind::Domain(domain),
        vec![api],
    );
}

pub(super) fn record_library_api_node_result_domain(
    il: &mut Il,
    node: NodeId,
    domain: DomainEvidence,
    api: EvidenceId,
) {
    upsert_language_core_evidence(
        il,
        EvidenceAnchor::node(il.node(node).span, il.kind(node)),
        EvidenceKind::Domain(domain),
        vec![api],
    );
}

pub(super) fn seed_receiver_method_dependencies(
    il: &mut Il,
    interner: &Interner,
    callee: NodeId,
    callee_contract: LibraryApiCalleeContract,
) {
    let LibraryApiCalleeContract::Method { receiver, .. } = callee_contract else {
        return;
    };
    let Some(&receiver_node) = il.children(callee).first() else {
        return;
    };
    match receiver {
        MethodReceiverContract::UnshadowedGlobal(name) => {
            if node_name(il, interner, receiver_node) == Some(name)
                && !file_defines_name_visible_at(il, interner, name, il.node(receiver_node).span)
            {
                let _ = unshadowed_symbol_evidence_id(il, receiver_node, name);
            }
        }
        MethodReceiverContract::ImportedNamespace(module) => {
            let _ = imported_namespace_symbol_evidence_id(il, interner, receiver_node, module);
        }
        _ => {}
    }
}

pub(super) fn unshadowed_symbol_evidence_id(
    il: &mut Il,
    node: NodeId,
    expected: &str,
) -> Option<EvidenceId> {
    (il.kind(node) == NodeKind::Var).then_some(())?;
    Some(upsert_language_core_evidence(
        il,
        EvidenceAnchor::node(il.node(node).span, NodeKind::Var),
        EvidenceKind::Symbol(SymbolEvidenceKind::UnshadowedGlobal {
            name_hash: stable_symbol_hash(expected),
        }),
        Vec::new(),
    ))
}

pub(super) fn imported_namespace_symbol_evidence_id(
    il: &mut Il,
    interner: &Interner,
    node: NodeId,
    module: &str,
) -> Option<EvidenceId> {
    let expected = SymbolEvidenceKind::ImportedNamespace {
        module_hash: stable_symbol_hash(module),
    };
    let dependency = binding_symbol_evidence_id(il, interner, node, expected)?;
    Some(upsert_language_core_evidence(
        il,
        EvidenceAnchor::node(il.node(node).span, NodeKind::Var),
        EvidenceKind::Symbol(expected),
        vec![dependency],
    ))
}

pub(super) fn binding_symbol_evidence_id(
    il: &Il,
    interner: &Interner,
    node: NodeId,
    expected: SymbolEvidenceKind,
) -> Option<EvidenceId> {
    if il.kind(node) != NodeKind::Var {
        return None;
    }
    let local_hash = node_name_hash(il, interner, node)?;
    il.evidence_binding_anchored(local_hash).find_map(|record| {
        (record.kind == EvidenceKind::Symbol(expected) && record.status == EvidenceStatus::Asserted)
            .then_some(record.id)
    })
}

pub(super) fn node_name<'a>(il: &Il, interner: &'a Interner, node: NodeId) -> Option<&'a str> {
    il.var_binding_name(node)
        .map(|symbol| interner.resolve(symbol))
}

pub(super) fn node_name_hash(il: &Il, interner: &Interner, node: NodeId) -> Option<u64> {
    node_name(il, interner, node).map(stable_symbol_hash)
}

pub(super) fn binding_node_name(il: &Il, node: NodeId) -> Option<Symbol> {
    il.var_binding_name(node)
}

pub(super) fn file_defines_name_visible_at(
    il: &Il,
    interner: &Interner,
    name: &str,
    occurrence_span: nose_il::Span,
) -> bool {
    nose_semantics::file_defines_name_visible_at(il, interner, name, occurrence_span)
}

pub(super) fn upsert_language_core_evidence(
    il: &mut Il,
    anchor: EvidenceAnchor,
    kind: EvidenceKind,
    dependencies: Vec<EvidenceId>,
) -> EvidenceId {
    let (pack_id, producer_id) = language_core_evidence_provenance(il.meta.lang);
    let pack_hash = stable_symbol_hash(pack_id);
    let legacy_pack_hash = stable_symbol_hash(BUILTIN_COMPAT_PACK_ID);
    let rule_hash = stable_symbol_hash(producer_id);
    let mut current_idx = None;
    let mut legacy_idx = None;
    let mut duplicate_indices = Vec::new();
    for idx in il.evidence_indices_anchored_at(anchor.span()) {
        let record = &il.evidence[idx as usize];
        if record.anchor == anchor
            && record.kind == kind
            && record.status == EvidenceStatus::Asserted
            && record.provenance.emitter == EvidenceEmitter::Builtin
        {
            match record.provenance.pack_hash {
                Some(hash) if hash == pack_hash => {
                    if current_idx.is_none() {
                        if let Some(idx) = legacy_idx.take() {
                            duplicate_indices.push(idx);
                        }
                        current_idx = Some(idx);
                    } else {
                        duplicate_indices.push(idx);
                    }
                }
                Some(hash) if hash == legacy_pack_hash => {
                    if current_idx.is_some() {
                        duplicate_indices.push(idx);
                    } else if legacy_idx.is_none() {
                        legacy_idx = Some(idx);
                    } else {
                        duplicate_indices.push(idx);
                    }
                }
                _ => {}
            }
        }
    }
    let Some(idx) = current_idx.or(legacy_idx) else {
        return il.find_or_push_builtin_evidence(anchor, kind, pack_id, producer_id, dependencies);
    };
    let record = &mut il.evidence[idx as usize];
    record.provenance.pack_hash = Some(pack_hash);
    record.provenance.rule_hash = Some(rule_hash);
    record.dependencies = dependencies;
    let id = record.id;
    for duplicate_idx in duplicate_indices {
        il.evidence[duplicate_idx as usize].status = EvidenceStatus::Ambiguous;
    }
    id
}

pub(super) fn upsert_builtin_evidence_with_pack_id(
    il: &mut Il,
    anchor: EvidenceAnchor,
    kind: EvidenceKind,
    pack_id: &str,
    rule: &str,
    dependencies: Vec<EvidenceId>,
) -> EvidenceId {
    let pack_hash = stable_symbol_hash(pack_id);
    let rule_hash = stable_symbol_hash(rule);
    let mut found = None;
    // Index-backed (see `effect_evidence::upsert`): only same-span records can
    // match, and the fields updated in place are read live by the index.
    for idx in il.evidence_indices_anchored_at(anchor.span()) {
        let record = &mut il.evidence[idx as usize];
        if record.anchor == anchor
            && record.kind == kind
            && record.status == EvidenceStatus::Asserted
            && record.provenance.emitter == EvidenceEmitter::Builtin
            && record.provenance.pack_hash == Some(pack_hash)
        {
            if found.is_none() {
                found = Some(record.id);
            }
            record.provenance.rule_hash = Some(rule_hash);
            record.dependencies = dependencies.clone();
        }
    }
    found.unwrap_or_else(|| {
        il.find_or_push_builtin_evidence(anchor, kind, pack_id, rule, dependencies)
    })
}
