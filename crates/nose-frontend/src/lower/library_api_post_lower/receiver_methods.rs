use super::*;

pub(super) fn record_post_lower_receiver_method_library_api(
    il: &mut Il,
    interner: &Interner,
    call: NodeId,
    dependency_cache: &mut LibraryApiDependencyCache,
) -> bool {
    proven_receiver_method_api_contract_for_call_with_cache(
        il,
        interner,
        call,
        dependency_cache,
        |il, interner, callee, callee_contract| {
            seed_post_lower_receiver_method_dependencies(il, interner, callee, callee_contract);
        },
    )
    .is_some_and(|(arg_count, contract, dependencies)| {
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
    })
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
