use super::*;

pub(super) fn record_post_lower_object_key_view_library_api(
    il: &mut Il,
    interner: &Interner,
    call: NodeId,
) -> bool {
    let Some((callee, arg_count)) = post_lower_static_global_call_parts(il, interner, call) else {
        return false;
    };
    let Some(contract) =
        library_object_key_view_contract(il.meta.lang, "Object", "keys", arg_count)
    else {
        return false;
    };
    let Some(qualified) = post_lower_qualified_global_symbol_evidence_id(il, callee, "Object.keys")
    else {
        return false;
    };
    let Some(root) = post_lower_static_global_receiver_node(il, interner, callee, "Object") else {
        return false;
    };
    let Some(root_dependency) = post_lower_unshadowed_symbol_evidence_id(il, root, "Object") else {
        return false;
    };
    let Some(mut dependencies) =
        js_object_key_view_argument_dependency_ids_for_call(il, interner, call)
    else {
        return false;
    };
    dependencies.insert(0, root_dependency);
    dependencies.insert(0, qualified);
    let api = post_lower_library_api_evidence_with_pack_id(
        il,
        call,
        contract.id,
        contract.callee,
        arg_count,
        contract.pack_id,
        MAP_KEY_VIEW_PROTOCOL_PRODUCER_ID,
        dependencies,
    );
    post_lower_record_library_api_result_domain(il, call, Some(DomainEvidence::Array), api);
    true
}

fn post_lower_static_global_call_parts(
    il: &Il,
    interner: &Interner,
    call: NodeId,
) -> Option<(NodeId, usize)> {
    if il.kind(call) != NodeKind::Call {
        return None;
    }
    let (&callee, args) = il.children(call).split_first()?;
    let (receiver, method) = post_lower_static_global_callee_parts(il, interner, callee)?;
    (receiver == "Object" && method == "keys").then_some((callee, args.len()))
}

fn post_lower_static_global_callee_parts<'a>(
    il: &Il,
    interner: &'a Interner,
    callee: NodeId,
) -> Option<(&'a str, &'a str)> {
    if il.kind(callee) != NodeKind::Field {
        return None;
    }
    let Payload::Name(method) = il.node(callee).payload else {
        return None;
    };
    let receiver = il.children(callee).first().copied()?;
    let receiver = post_lower_var_name(il, interner, receiver)?;
    Some((receiver, interner.resolve(method)))
}

fn post_lower_static_global_receiver_node(
    il: &Il,
    interner: &Interner,
    callee: NodeId,
    expected: &str,
) -> Option<NodeId> {
    let receiver = il.children(callee).first().copied()?;
    matches!(post_lower_var_name(il, interner, receiver), Some(name) if name == expected)
        .then_some(receiver)
}

fn post_lower_qualified_global_symbol_evidence_id(
    il: &Il,
    node: NodeId,
    path: &str,
) -> Option<EvidenceId> {
    il.evidence.iter().find_map(|record| {
        (record.anchor == EvidenceAnchor::node(il.node(node).span, il.kind(node))
            && record.kind
                == EvidenceKind::Symbol(SymbolEvidenceKind::QualifiedGlobal {
                    path_hash: stable_symbol_hash(path),
                })
            && record.status == EvidenceStatus::Asserted
            && il.evidence_dependencies_asserted(record))
        .then_some(record.id)
    })
}
