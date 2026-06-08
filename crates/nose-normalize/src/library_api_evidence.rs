use nose_il::{
    stable_symbol_hash, EvidenceAnchor, EvidenceEmitter, EvidenceId, EvidenceKind, EvidenceStatus,
    Il, Interner, LibraryApiEvidenceKind, NodeId, NodeKind, Payload, SymbolEvidenceKind,
};
use nose_semantics::{
    library_api_callee_contract_hash, library_api_contract_id_hash,
    library_api_receiver_dependencies_for_call_with_cache, library_receiver_method_api_contract,
    LibraryApiCalleeContract, LibraryApiDependencyCache, MethodReceiverContract,
    FIRST_PARTY_PACK_ID,
};

pub(crate) fn run(il: &mut Il, interner: &Interner) {
    let calls: Vec<NodeId> = il
        .nodes
        .iter()
        .enumerate()
        .filter_map(|(idx, node)| (node.kind == NodeKind::Call).then_some(NodeId(idx as u32)))
        .collect();
    let mut dependency_cache = LibraryApiDependencyCache::default();
    for call in calls {
        record_receiver_method_library_api(il, interner, call, &mut dependency_cache);
    }
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
