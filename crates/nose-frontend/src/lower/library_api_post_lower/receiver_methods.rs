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
        if js_ts_string_affix_prototype_mutated_in_file(il, interner, contract) {
            return false;
        }
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

fn js_ts_string_affix_prototype_mutated_in_file(
    il: &Il,
    interner: &Interner,
    contract: LibraryReceiverMethodApiContract,
) -> bool {
    if !matches!(il.meta.lang, Lang::JavaScript | Lang::TypeScript) {
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
    post_lower_top_level_statements(il)
        .into_iter()
        .any(|stmt| string_prototype_method_mutation_in_module_scope(il, interner, stmt, method))
}

fn string_prototype_method_mutation_in_module_scope(
    il: &Il,
    interner: &Interner,
    node: NodeId,
    expected_method: &str,
) -> bool {
    if matches!(il.kind(node), NodeKind::Func | NodeKind::Lambda) {
        return false;
    }
    string_prototype_method_write(il, interner, node, expected_method)
        || object_define_property_string_prototype_method(il, interner, node, expected_method)
        || il.children(node).iter().copied().any(|child| {
            string_prototype_method_mutation_in_module_scope(il, interner, child, expected_method)
        })
}

fn string_prototype_method_write(
    il: &Il,
    interner: &Interner,
    stmt: NodeId,
    expected_method: &str,
) -> bool {
    let assign = if il.kind(stmt) == NodeKind::ExprStmt {
        il.children(stmt).first().copied().unwrap_or(stmt)
    } else {
        stmt
    };
    if il.kind(assign) != NodeKind::Assign {
        return false;
    }
    let Some(&target) = il.children(assign).first() else {
        return false;
    };
    field_name(il, interner, target) == Some(expected_method)
        && il
            .children(target)
            .first()
            .copied()
            .is_some_and(|prototype| string_prototype_object(il, interner, prototype))
}

fn object_define_property_string_prototype_method(
    il: &Il,
    interner: &Interner,
    node: NodeId,
    expected_method: &str,
) -> bool {
    let call = if il.kind(node) == NodeKind::ExprStmt {
        il.children(node).first().copied().unwrap_or(node)
    } else {
        node
    };
    if il.kind(call) != NodeKind::Call {
        return false;
    }
    let [callee, target, property, ..] = il.children(call) else {
        return false;
    };
    field_name(il, interner, *callee) == Some("defineProperty")
        && il
            .children(*callee)
            .first()
            .copied()
            .is_some_and(|base| post_lower_module_unshadowed_var_name(il, interner, base, "Object"))
        && string_prototype_object(il, interner, *target)
        && string_literal(il, *property, expected_method)
}

fn string_prototype_object(il: &Il, interner: &Interner, node: NodeId) -> bool {
    field_name(il, interner, node) == Some("prototype")
        && il
            .children(node)
            .first()
            .copied()
            .is_some_and(|base| post_lower_module_unshadowed_var_name(il, interner, base, "String"))
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

fn string_literal(il: &Il, node: NodeId, expected: &str) -> bool {
    matches!(il.node(node).payload, Payload::LitStr(hash) if hash == stable_symbol_hash(expected))
}

fn field_name<'a>(il: &Il, interner: &'a Interner, node: NodeId) -> Option<&'a str> {
    if il.kind(node) != NodeKind::Field {
        return None;
    }
    let Payload::Name(symbol) = il.node(node).payload else {
        return None;
    };
    Some(interner.resolve(symbol))
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
