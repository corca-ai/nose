use nose_il::{
    stable_symbol_hash, Builtin, DomainEvidence, EvidenceAnchor, EvidenceEmitter, EvidenceId,
    EvidenceKind, EvidenceStatus, Il, Interner, LibraryApiEvidenceKind, NodeId, NodeKind, Payload,
    SequenceSurfaceKind, Symbol, SymbolEvidenceKind,
};
use nose_semantics::{
    builder_append_method_contract, library_api_callee_contract_hash, library_api_contract_id_hash,
    library_api_free_name_shadow_safe, library_api_property_dependencies_for_field_with_cache,
    library_api_receiver_dependencies_for_call_with_cache, library_method_call_contract,
    library_property_builtin_contract, library_receiver_method_api_contract,
    library_rust_option_none_sentinel_contract, library_rust_option_some_constructor_contract,
    LibraryApiCalleeContract, LibraryApiDependencyCache, MethodBuiltinArgs,
    MethodEffectReceiverContract, MethodReceiverContract, MethodSemanticContract,
    FIRST_PARTY_PACK_ID,
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
        if record_builder_append_method_library_api(il, interner, call) {
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
    upsert_first_party_evidence(
        il,
        EvidenceAnchor::node(il.node(call).span, NodeKind::Call),
        EvidenceKind::LibraryApi(LibraryApiEvidenceKind::Contract {
            contract_hash: library_api_contract_id_hash(contract.id),
            callee_hash: library_api_callee_contract_hash(contract.callee),
            arity: arg_count as u16,
        }),
        "library_api_builder_append_method",
        dependencies,
    );
    true
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
        return Some(dependencies);
    }

    let LibraryApiCalleeContract::Method { method, .. } = callee else {
        return None;
    };
    let callee_node = *il.children(call).first()?;
    let receiver = method_callee_receiver_node(il, interner, callee_node, method)?;
    let seed_dependency = local_collection_seed_dependency_id(il, call, receiver)?;
    let receiver_domain = upsert_first_party_evidence(
        il,
        EvidenceAnchor::node(il.node(receiver).span, il.kind(receiver)),
        EvidenceKind::Domain(DomainEvidence::Collection),
        "library_api_builder_append_receiver_domain",
        vec![seed_dependency],
    );
    Some(vec![receiver_domain])
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
        let dependency = collection_seed_dependency_id(il, *rhs)?;
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

fn collection_seed_dependency_id(il: &Il, node: NodeId) -> Option<EvidenceId> {
    domain_evidence_id_for_node(il, node, DomainEvidence::Collection).or_else(|| {
        sequence_surface_evidence_id_for_node(il, node, SequenceSurfaceKind::Collection)
    })
}

fn domain_evidence_id_for_node(
    il: &Il,
    node: NodeId,
    expected: DomainEvidence,
) -> Option<EvidenceId> {
    let anchor = EvidenceAnchor::node(il.node(node).span, il.kind(node));
    il.evidence.iter().find_map(|record| {
        (record.anchor == anchor
            && record.kind == EvidenceKind::Domain(expected)
            && record.status == EvidenceStatus::Asserted
            && il.evidence_dependencies_asserted(record))
        .then_some(record.id)
    })
}

fn sequence_surface_evidence_id_for_node(
    il: &Il,
    node: NodeId,
    expected: SequenceSurfaceKind,
) -> Option<EvidenceId> {
    if il.kind(node) != NodeKind::Seq {
        return None;
    }
    let anchor = EvidenceAnchor::sequence(il.node(node).span);
    il.evidence.iter().find_map(|record| {
        (record.anchor == anchor
            && record.kind == EvidenceKind::SequenceSurface(expected)
            && record.status == EvidenceStatus::Asserted
            && il.evidence_dependencies_asserted(record))
        .then_some(record.id)
    })
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

fn binding_node_name(il: &Il, node: NodeId) -> Option<Symbol> {
    if il.kind(node) != NodeKind::Var {
        return None;
    }
    match il.node(node).payload {
        Payload::Name(symbol) => Some(symbol),
        Payload::Cid(cid) => il.cid_names.get(cid as usize).copied(),
        _ => None,
    }
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

#[cfg(test)]
mod tests {
    use super::*;
    use nose_il::{FileId, FileMeta, IlBuilder, Lang, Span};
    use nose_semantics::admitted_builder_append_method_call_args;

    fn sp(byte: u32) -> Span {
        Span::new(FileId(0), byte, byte + 1, byte, byte + 1)
    }

    fn method_call_il(
        interner: &mut Interner,
        lang: Lang,
        method: &str,
        arg_count: usize,
    ) -> (Il, NodeId, NodeId, Option<NodeId>) {
        let mut builder = IlBuilder::new(FileId(0));
        let name = interner.intern("r");
        let seed_span = sp(1);
        let seed = builder.add(NodeKind::Seq, Payload::None, seed_span, &[]);
        let target = builder.add(NodeKind::Var, Payload::Name(name), sp(2), &[]);
        let assign = builder.add(NodeKind::Assign, Payload::None, sp(2), &[target, seed]);
        let receiver = builder.add(NodeKind::Var, Payload::Name(name), sp(3), &[]);
        let field = builder.add(
            NodeKind::Field,
            Payload::Name(interner.intern(method)),
            sp(3),
            &[receiver],
        );
        let args: Vec<NodeId> = (0..arg_count)
            .map(|idx| builder.add(NodeKind::Var, Payload::Cid((idx + 1) as u32), sp(4), &[]))
            .collect();
        let first_arg = args.first().copied();
        let mut children = Vec::with_capacity(args.len() + 1);
        children.push(field);
        children.extend(args);
        let call = builder.add(NodeKind::Call, Payload::None, sp(5), &children);
        let root = builder.add(NodeKind::Func, Payload::None, sp(6), &[assign, call]);
        let mut il = builder.finish(
            root,
            FileMeta {
                path: "method".into(),
                lang,
            },
            Vec::new(),
            Vec::new(),
        );
        il.find_or_push_first_party_evidence(
            EvidenceAnchor::sequence(seed_span),
            EvidenceKind::SequenceSurface(SequenceSurfaceKind::Collection),
            FIRST_PARTY_PACK_ID,
            "test_collection_seed",
            Vec::new(),
        );
        (il, call, receiver, first_arg)
    }

    #[test]
    fn builder_append_method_api_evidence_admits_first_party_rows() {
        for (lang, method) in [
            (Lang::Python, "append"),
            (Lang::JavaScript, "push"),
            (Lang::Java, "add"),
            (Lang::Rust, "push"),
        ] {
            let mut interner = Interner::new();
            let (mut il, call, receiver, item) = method_call_il(&mut interner, lang, method, 1);

            run(&mut il, &interner);

            let (admitted_receiver, admitted_item) =
                admitted_builder_append_method_call_args(&il, &interner, call)
                    .expect("builder append method evidence");
            assert_eq!(admitted_receiver, receiver);
            assert_eq!(Some(admitted_item), item);
        }
    }

    #[test]
    fn builder_append_method_api_evidence_is_language_and_arity_scoped() {
        for (lang, method, arg_count) in [
            (Lang::Ruby, "push", 1),
            (Lang::Python, "append", 2),
            (Lang::JavaScript, "push", 2),
        ] {
            let mut interner = Interner::new();
            let (mut il, call, _, _) = method_call_il(&mut interner, lang, method, arg_count);

            run(&mut il, &interner);

            assert!(admitted_builder_append_method_call_args(&il, &interner, call).is_none());
        }
    }
}
