use super::*;

pub(super) fn record_post_lower_receiver_method_library_api(
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
    seed_post_lower_receiver_method_dependencies(il, interner, callee, contract.callee);
    let Some(dependencies) = library_api_receiver_dependencies_for_call_with_cache(
        il,
        interner,
        call,
        contract.callee,
        dependency_cache,
    ) else {
        return false;
    };
    record_post_lower_library_api_contract(
        il,
        call,
        arg_count,
        PostLowerLibraryApiContract {
            id: contract.id,
            callee: contract.callee,
            pack_id: contract.pack_id,
            rule: contract.rule,
            result_domain: contract.result_domain,
        },
        dependencies,
    );
    true
}

fn seed_post_lower_receiver_method_dependencies(
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
            if post_lower_var_name(il, interner, receiver_node) == Some(name)
                && !post_lower_file_defines_name_visible_at(
                    il,
                    interner,
                    name,
                    il.node(receiver_node).span,
                )
            {
                let _ = post_lower_unshadowed_symbol_evidence_id(il, receiver_node, name);
            }
        }
        MethodReceiverContract::ImportedNamespace(module) => {
            let _ = post_lower_imported_namespace_symbol_evidence_id(
                il,
                interner,
                receiver_node,
                module,
            );
        }
        _ => {}
    }
}
