use super::*;

pub(super) fn record_static_global_method_library_api(
    il: &mut Il,
    interner: &Interner,
    call: NodeId,
) -> bool {
    let Some((callee, receiver, receiver_name, method, arg_count)) =
        static_global_method_call_parts(il, interner, call)
    else {
        return false;
    };
    if let Some(contract) =
        library_promise_resolve_contract(il.meta.lang, receiver_name, method, arg_count)
    {
        return record_static_global_method_contract(
            il,
            callee,
            receiver,
            call,
            arg_count,
            StaticGlobalMethodContract {
                pack_id: contract.pack_id,
                id: contract.id,
                callee: contract.callee,
            },
        );
    }
    if let Some(contract) =
        library_promise_aggregate_contract(il.meta.lang, receiver_name, method, arg_count)
    {
        return record_static_global_method_contract(
            il,
            callee,
            receiver,
            call,
            arg_count,
            StaticGlobalMethodContract {
                pack_id: contract.pack_id,
                id: contract.id,
                callee: contract.callee,
            },
        );
    }
    false
}

struct StaticGlobalMethodContract {
    pack_id: &'static str,
    id: LibraryApiContractId,
    callee: LibraryApiCalleeContract,
}

fn record_static_global_method_contract(
    il: &mut Il,
    callee: NodeId,
    receiver: NodeId,
    call: NodeId,
    arg_count: usize,
    contract: StaticGlobalMethodContract,
) -> bool {
    let LibraryApiCalleeContract::StaticGlobalMethod {
        receiver: expected_receiver,
        qualified_path,
        requires_unshadowed_receiver,
        ..
    } = contract.callee
    else {
        return false;
    };
    let Some(qualified_dependency) =
        qualified_global_symbol_evidence_id(il, callee, qualified_path)
    else {
        return false;
    };
    let mut dependencies = vec![qualified_dependency];
    if requires_unshadowed_receiver {
        let Some(receiver_dependency) =
            unshadowed_symbol_evidence_id(il, receiver, expected_receiver)
        else {
            return false;
        };
        dependencies.push(receiver_dependency);
    }
    upsert_builtin_evidence_with_pack_id(
        il,
        EvidenceAnchor::node(il.node(call).span, NodeKind::Call),
        EvidenceKind::LibraryApi(LibraryApiEvidenceKind::Contract {
            contract_hash: library_api_contract_id_hash(contract.id),
            callee_hash: library_api_callee_contract_hash(contract.callee),
            arity: arg_count as u16,
        }),
        contract.pack_id,
        JS_LIKE_BUILTIN_PROMISE_PRODUCER_ID,
        dependencies,
    );
    true
}

fn static_global_method_call_parts<'a>(
    il: &Il,
    interner: &'a Interner,
    call: NodeId,
) -> Option<(NodeId, NodeId, &'a str, &'a str, usize)> {
    let kids = il.children(call);
    let (&callee, args) = kids.split_first()?;
    if il.kind(callee) != NodeKind::Field || !matches!(il.node(call).payload, Payload::None) {
        return None;
    }
    let Payload::Name(method) = il.node(callee).payload else {
        return None;
    };
    let receiver = *il.children(callee).first()?;
    let receiver_name = node_name(il, interner, receiver)?;
    Some((
        callee,
        receiver,
        receiver_name,
        interner.resolve(method),
        args.len(),
    ))
}

fn qualified_global_symbol_evidence_id(
    il: &Il,
    node: NodeId,
    expected: &str,
) -> Option<EvidenceId> {
    let anchor = EvidenceAnchor::node(il.node(node).span, il.kind(node));
    let expected_hash = stable_symbol_hash(expected);
    for record in il.evidence_anchored_at(anchor.span()) {
        if record.anchor != anchor
            || record.status != EvidenceStatus::Asserted
            || !il.evidence_dependencies_asserted(record)
        {
            continue;
        }
        if matches!(
            record.kind,
            EvidenceKind::Symbol(SymbolEvidenceKind::QualifiedGlobal { path_hash })
                if path_hash == expected_hash
        ) {
            return Some(record.id);
        }
    }
    None
}
