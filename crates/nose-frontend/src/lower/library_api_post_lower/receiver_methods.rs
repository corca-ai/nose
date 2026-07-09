use super::*;
use nose_il::HoFKind;

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
        if nose_semantics::js_ts_builtin_prototype_mutated_in_file(
            il,
            interner,
            contract.id,
            contract.callee,
            post_lower_top_level_statements(il),
            post_lower_module_unshadowed_var_name,
        ) {
            return false;
        }
        if ruby_string_affix_redefined_in_file(il, interner, contract) {
            return false;
        }
        if ruby_sequence_hof_method_redefined_in_file(il, interner, contract) {
            return false;
        }
        record_post_lower_library_api_contract(
            il,
            interner,
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

fn ruby_string_affix_redefined_in_file(
    il: &Il,
    interner: &Interner,
    contract: LibraryReceiverMethodApiContract,
) -> bool {
    if il.meta.lang != Lang::Ruby {
        return false;
    }
    let LibraryApiContractId::MethodCall(MethodSemanticContract::Builtin(
        Builtin::StartsWith | Builtin::EndsWith,
    )) = contract.id
    else {
        return false;
    };
    let LibraryApiCalleeContract::Method { method, .. } = contract.callee else {
        return false;
    };
    ruby_string_instance_method_redefined_in_file(il, interner, method)
}

fn ruby_string_instance_method_redefined_in_file(
    il: &Il,
    interner: &Interner,
    expected_method: &str,
) -> bool {
    ruby_class_instance_method_redefined_in_file(
        il,
        interner,
        &["String", "::String"],
        expected_method,
    )
}

fn ruby_sequence_hof_method_redefined_in_file(
    il: &Il,
    interner: &Interner,
    contract: LibraryReceiverMethodApiContract,
) -> bool {
    if il.meta.lang != Lang::Ruby {
        return false;
    }
    let LibraryApiContractId::MethodCall(
        MethodSemanticContract::HoF(HoFKind::Map | HoFKind::Filter | HoFKind::Reject)
        | MethodSemanticContract::Builtin(Builtin::Any | Builtin::All),
    ) = contract.id
    else {
        return false;
    };
    let LibraryApiCalleeContract::Method {
        method,
        receiver: MethodReceiverContract::ExactArrayOrCollection,
    } = contract.callee
    else {
        return false;
    };
    ruby_class_instance_method_redefined_in_file(
        il,
        interner,
        &["Array", "::Array", "Enumerable", "::Enumerable"],
        method,
    )
}

fn ruby_class_instance_method_redefined_in_file(
    il: &Il,
    interner: &Interner,
    class_names: &[&str],
    expected_method: &str,
) -> bool {
    nose_semantics::ruby_class_instance_method_redefined_in_file(
        il,
        interner,
        class_names,
        expected_method,
        post_lower_var_name,
    )
}

fn post_lower_module_unshadowed_var_name(
    il: &Il,
    interner: &Interner,
    node: NodeId,
    expected: &str,
) -> bool {
    post_lower_var_name(il, interner, node) == Some(expected)
        && !post_lower_module_scope_defines_name(il, interner, expected)
}

fn post_lower_module_scope_defines_name(il: &Il, interner: &Interner, expected: &str) -> bool {
    post_lower_top_level_statements(il)
        .into_iter()
        .any(|stmt| post_lower_module_scope_statement_defines_name(il, interner, stmt, expected))
}

fn post_lower_module_scope_statement_defines_name(
    il: &Il,
    interner: &Interner,
    node: NodeId,
    expected: &str,
) -> bool {
    match il.kind(node) {
        NodeKind::Assign => il
            .children(node)
            .first()
            .copied()
            .is_some_and(|lhs| post_lower_var_name(il, interner, lhs) == Some(expected)),
        NodeKind::Func => il.units.iter().any(|unit| {
            unit.root == node
                && unit
                    .name
                    .is_some_and(|symbol| interner.resolve(symbol) == expected)
        }),
        _ => false,
    }
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
        MethodReceiverContract::ExactString => {
            if matches!(il.node(receiver_node).payload, Payload::LitStr(_)) {
                let _ = post_lower_find_or_push_evidence(
                    il,
                    EvidenceAnchor::node(il.node(receiver_node).span, il.kind(receiver_node)),
                    EvidenceKind::Domain(DomainEvidence::String),
                    "string_literal_receiver_domain",
                    Vec::new(),
                );
            }
        }
        _ => {}
    }
}
