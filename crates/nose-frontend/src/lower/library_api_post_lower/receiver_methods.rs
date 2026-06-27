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
        if ruby_string_affix_redefined_in_file(il, interner, contract) {
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
    ruby_string_class_unit_redefines_method(il, interner, expected_method)
        || ruby_string_class_eval_redefines_method(il, interner, expected_method)
        || ruby_string_direct_define_method_redefines_method(il, interner, expected_method)
}

fn ruby_string_class_unit_redefines_method(
    il: &Il,
    interner: &Interner,
    expected_method: &str,
) -> bool {
    il.units.iter().any(|class_unit| {
        class_unit.kind == UnitKind::Class
            && class_unit
                .name
                .is_some_and(|name| ruby_string_class_name(interner.resolve(name)))
            && {
                let class_span = il.node(class_unit.root).span;
                il.units.iter().any(|method_unit| {
                    let method_span = il.node(method_unit.root).span;
                    method_unit.kind == UnitKind::Method
                        && method_unit
                            .name
                            .is_some_and(|name| interner.resolve(name) == expected_method)
                        && class_span.file == method_span.file
                        && class_span.start_byte <= method_span.start_byte
                        && method_span.end_byte <= class_span.end_byte
                }) || il.nodes.iter().enumerate().any(|(idx, node)| {
                    let call = NodeId(idx as u32);
                    node.kind == NodeKind::Call
                        && ruby_define_method_call_redefines_method(
                            il,
                            interner,
                            call,
                            expected_method,
                        )
                        && class_span.file == node.span.file
                        && class_span.start_byte <= node.span.start_byte
                        && node.span.end_byte <= class_span.end_byte
                })
            }
    })
}

fn ruby_string_class_eval_redefines_method(
    il: &Il,
    interner: &Interner,
    expected_method: &str,
) -> bool {
    let method_spans: Vec<Span> = il
        .units
        .iter()
        .filter(|unit| {
            unit.kind == UnitKind::Method
                && unit
                    .name
                    .is_some_and(|name| interner.resolve(name) == expected_method)
        })
        .map(|unit| il.node(unit.root).span)
        .collect();
    let define_method_spans: Vec<Span> = il
        .nodes
        .iter()
        .enumerate()
        .filter(|(idx, node)| {
            node.kind == NodeKind::Call
                && ruby_define_method_call_redefines_method(
                    il,
                    interner,
                    NodeId(*idx as u32),
                    expected_method,
                )
        })
        .map(|(_, node)| node.span)
        .collect();
    il.nodes.iter().enumerate().any(|(idx, node)| {
        let call = NodeId(idx as u32);
        node.kind == NodeKind::Call
            && ruby_string_class_eval_call(il, interner, call)
            && (method_spans.iter().any(|&method_span| {
                node.span.file == method_span.file
                    && node.span.start_byte <= method_span.start_byte
                    && method_span.end_byte <= node.span.end_byte
            }) || define_method_spans.iter().any(|&define_method_span| {
                node.span.file == define_method_span.file
                    && node.span.start_byte <= define_method_span.start_byte
                    && define_method_span.end_byte <= node.span.end_byte
            }))
    })
}

fn ruby_string_class_eval_call(il: &Il, interner: &Interner, call: NodeId) -> bool {
    let Some(&callee) = il.children(call).first() else {
        return false;
    };
    field_name(il, interner, callee) == Some("class_eval")
        && il
            .children(callee)
            .first()
            .copied()
            .and_then(|receiver| post_lower_var_name(il, interner, receiver))
            .is_some_and(ruby_string_class_name)
}

fn ruby_string_direct_define_method_redefines_method(
    il: &Il,
    interner: &Interner,
    expected_method: &str,
) -> bool {
    il.nodes.iter().enumerate().any(|(idx, node)| {
        let call = NodeId(idx as u32);
        node.kind == NodeKind::Call
            && ruby_define_method_call_redefines_method(il, interner, call, expected_method)
            && il.children(call).first().copied().is_some_and(|callee| {
                field_name(il, interner, callee) == Some("define_method")
                    && il
                        .children(callee)
                        .first()
                        .copied()
                        .and_then(|receiver| post_lower_var_name(il, interner, receiver))
                        .is_some_and(ruby_string_class_name)
            })
    })
}

fn ruby_define_method_call_redefines_method(
    il: &Il,
    interner: &Interner,
    call: NodeId,
    expected_method: &str,
) -> bool {
    let [callee, method, ..] = il.children(call) else {
        return false;
    };
    (post_lower_var_name(il, interner, *callee) == Some("define_method")
        || field_name(il, interner, *callee) == Some("define_method"))
        && string_literal(il, *method, expected_method)
}

fn ruby_string_class_name(name: &str) -> bool {
    matches!(name, "String" | "::String")
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
