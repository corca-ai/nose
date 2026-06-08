use nose_il::{
    stable_symbol_hash, DomainEvidence, EvidenceAnchor, EvidenceEmitter, EvidenceId, EvidenceKind,
    EvidenceStatus, Il, Interner, LibraryApiEvidenceKind, NodeId, NodeKind, Payload,
    SymbolEvidenceKind,
};
use nose_semantics::{
    library_api_callee_contract_hash, library_api_contract_id_hash,
    library_api_free_name_shadow_safe, library_api_property_dependencies_for_field_with_cache,
    library_api_receiver_dependencies_for_call_with_cache, library_property_builtin_contract,
    library_receiver_method_api_contract, library_rust_option_none_sentinel_contract,
    library_rust_option_some_constructor_contract, LibraryApiCalleeContract,
    LibraryApiDependencyCache, MethodReceiverContract, FIRST_PARTY_PACK_ID,
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
    for call in calls {
        if record_rust_option_some_library_api(il, interner, call) {
            continue;
        }
        record_receiver_method_library_api(il, interner, call, &mut dependency_cache);
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
    let api = upsert_first_party_evidence(
        il,
        EvidenceAnchor::node(il.node(call).span, NodeKind::Call),
        EvidenceKind::LibraryApi(LibraryApiEvidenceKind::Contract {
            contract_hash: library_api_contract_id_hash(contract.id),
            callee_hash: library_api_callee_contract_hash(contract.callee),
            arity: arg_count as u16,
        }),
        "library_api_rust_option_some_constructor",
        vec![symbol_dependency],
    );
    record_library_api_result_domain(il, call, contract.result_domain, api);
    true
}

fn record_property_library_api(
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
    upsert_first_party_evidence(
        il,
        EvidenceAnchor::node(il.node(field).span, NodeKind::Field),
        EvidenceKind::LibraryApi(LibraryApiEvidenceKind::Contract {
            contract_hash: library_api_contract_id_hash(contract.id),
            callee_hash: library_api_callee_contract_hash(contract.callee),
            arity: 0,
        }),
        "library_api_property_builtin",
        dependencies,
    );
    true
}

fn record_rust_option_none_library_api(il: &mut Il, interner: &Interner, var: NodeId) -> bool {
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
    let api = upsert_first_party_evidence(
        il,
        EvidenceAnchor::node(il.node(var).span, NodeKind::Var),
        EvidenceKind::LibraryApi(LibraryApiEvidenceKind::Contract {
            contract_hash: library_api_contract_id_hash(contract.id),
            callee_hash: library_api_callee_contract_hash(contract.callee),
            arity: 0,
        }),
        "library_api_rust_option_none_sentinel",
        vec![symbol_dependency],
    );
    record_library_api_node_result_domain(il, var, contract.result_domain, api);
    true
}

fn record_receiver_method_library_api(
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
    upsert_first_party_evidence(
        il,
        EvidenceAnchor::node(il.node(call).span, NodeKind::Call),
        EvidenceKind::LibraryApi(LibraryApiEvidenceKind::Contract {
            contract_hash: library_api_contract_id_hash(contract.id),
            callee_hash: library_api_callee_contract_hash(contract.callee),
            arity: arg_count as u16,
        }),
        contract.rule,
        dependencies,
    );
    true
}

fn record_library_api_result_domain(
    il: &mut Il,
    call: NodeId,
    domain: DomainEvidence,
    api: EvidenceId,
) {
    upsert_first_party_evidence(
        il,
        EvidenceAnchor::node(il.node(call).span, NodeKind::Call),
        EvidenceKind::Domain(domain),
        "library_api_result_domain",
        vec![api],
    );
}

fn record_library_api_node_result_domain(
    il: &mut Il,
    node: NodeId,
    domain: DomainEvidence,
    api: EvidenceId,
) {
    upsert_first_party_evidence(
        il,
        EvidenceAnchor::node(il.node(node).span, il.kind(node)),
        EvidenceKind::Domain(domain),
        "library_api_result_domain",
        vec![api],
    );
}

fn seed_receiver_method_dependencies(
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

fn unshadowed_symbol_evidence_id(il: &mut Il, node: NodeId, expected: &str) -> Option<EvidenceId> {
    (il.kind(node) == NodeKind::Var).then_some(())?;
    Some(upsert_first_party_evidence(
        il,
        EvidenceAnchor::node(il.node(node).span, NodeKind::Var),
        EvidenceKind::Symbol(SymbolEvidenceKind::UnshadowedGlobal {
            name_hash: stable_symbol_hash(expected),
        }),
        "symbol_unshadowed_global_normalize",
        Vec::new(),
    ))
}

fn imported_namespace_symbol_evidence_id(
    il: &mut Il,
    interner: &Interner,
    node: NodeId,
    module: &str,
) -> Option<EvidenceId> {
    let expected = SymbolEvidenceKind::ImportedNamespace {
        module_hash: stable_symbol_hash(module),
    };
    let dependency = binding_symbol_evidence_id(il, interner, node, expected)?;
    Some(upsert_first_party_evidence(
        il,
        EvidenceAnchor::node(il.node(node).span, NodeKind::Var),
        EvidenceKind::Symbol(expected),
        "symbol_imported_namespace_occurrence_normalize",
        vec![dependency],
    ))
}

fn binding_symbol_evidence_id(
    il: &Il,
    interner: &Interner,
    node: NodeId,
    expected: SymbolEvidenceKind,
) -> Option<EvidenceId> {
    if il.kind(node) != NodeKind::Var {
        return None;
    }
    let local_hash = node_name_hash(il, interner, node)?;
    il.evidence.iter().find_map(|record| {
        (matches!(
            record.anchor,
            EvidenceAnchor::Binding {
                local_hash: anchor_hash,
                ..
            } if anchor_hash == local_hash
        ) && record.kind == EvidenceKind::Symbol(expected)
            && record.status == EvidenceStatus::Asserted)
            .then_some(record.id)
    })
}

fn node_name<'a>(il: &Il, interner: &'a Interner, node: NodeId) -> Option<&'a str> {
    if il.kind(node) != NodeKind::Var {
        return None;
    }
    match il.node(node).payload {
        Payload::Name(symbol) => Some(interner.resolve(symbol)),
        Payload::Cid(cid) => il
            .cid_names
            .get(cid as usize)
            .map(|&symbol| interner.resolve(symbol)),
        _ => None,
    }
}

fn node_name_hash(il: &Il, interner: &Interner, node: NodeId) -> Option<u64> {
    node_name(il, interner, node).map(stable_symbol_hash)
}

fn file_defines_name_visible_at(
    il: &Il,
    interner: &Interner,
    name: &str,
    occurrence_span: nose_il::Span,
) -> bool {
    nose_semantics::file_defines_name_visible_at(il, interner, name, occurrence_span)
}

fn upsert_first_party_evidence(
    il: &mut Il,
    anchor: EvidenceAnchor,
    kind: EvidenceKind,
    rule: &str,
    dependencies: Vec<EvidenceId>,
) -> EvidenceId {
    let pack_hash = stable_symbol_hash(FIRST_PARTY_PACK_ID);
    let rule_hash = stable_symbol_hash(rule);
    let mut found = None;
    for record in &mut il.evidence {
        if record.anchor == anchor
            && record.kind == kind
            && record.status == EvidenceStatus::Asserted
            && record.provenance.emitter == EvidenceEmitter::FirstParty
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
        il.find_or_push_first_party_evidence(anchor, kind, FIRST_PARTY_PACK_ID, rule, dependencies)
    })
}
