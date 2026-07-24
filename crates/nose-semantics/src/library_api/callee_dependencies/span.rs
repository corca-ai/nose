use super::java::*;
use super::*;

pub(in crate::library_api) fn library_api_dependencies_match_callee_at_span(
    il: &Il,
    interner: &Interner,
    call_span: Span,
    callee_span: Option<Span>,
    receiver_span: Option<Span>,
    callee: LibraryApiCalleeContract,
    record: &EvidenceRecord,
) -> bool {
    match callee {
        LibraryApiCalleeContract::FreeName { .. }
        | LibraryApiCalleeContract::LabeledFreeName { .. }
        | LibraryApiCalleeContract::RustMacro { .. }
        | LibraryApiCalleeContract::JsGlobalConstructor { .. }
        | LibraryApiCalleeContract::ImportedBinding { .. } => {
            library_api_dependencies_match_named_callee_at_span(
                il,
                interner,
                call_span,
                callee_span,
                receiver_span,
                callee,
                record,
            )
        }
        LibraryApiCalleeContract::JavaUtilStaticMember { .. }
        | LibraryApiCalleeContract::JavaStaticMember { .. }
        | LibraryApiCalleeContract::JavaUtilConstructor { .. }
        | LibraryApiCalleeContract::RubyRequireStaticMember { .. } => {
            library_api_dependencies_match_static_import_callee_at_span(
                il,
                interner,
                call_span,
                callee_span,
                receiver_span,
                callee,
                record,
            )
        }
        LibraryApiCalleeContract::RegexLiteralMethod { .. }
        | LibraryApiCalleeContract::Property { .. }
        | LibraryApiCalleeContract::StaticIndexMembershipMethod { .. }
        | LibraryApiCalleeContract::ImportedNamespaceFunction { .. }
        | LibraryApiCalleeContract::StaticGlobalMethod { .. }
        | LibraryApiCalleeContract::StaticGlobalFunction { .. } => {
            library_api_dependencies_match_static_member_callee_at_span(
                il,
                interner,
                callee_span,
                receiver_span,
                callee,
                record,
            )
        }
        LibraryApiCalleeContract::Method { .. }
        | LibraryApiCalleeContract::IteratorAdapterMethod { .. }
        | LibraryApiCalleeContract::AsyncMethod { .. } => {
            library_api_dependencies_match_method_callee_at_span(
                il,
                interner,
                call_span,
                callee_span,
                receiver_span,
                callee,
                record,
            )
        }
    }
}

pub(in crate::library_api) fn library_api_dependencies_match_named_callee_at_span(
    il: &Il,
    interner: &Interner,
    call_span: Span,
    callee_span: Option<Span>,
    receiver_span: Option<Span>,
    callee: LibraryApiCalleeContract,
    record: &EvidenceRecord,
) -> bool {
    match callee {
        LibraryApiCalleeContract::FreeName { name, shadow } => {
            free_name_dependency_safe_at_span(il, interner, record, callee_span, name, shadow)
        }
        LibraryApiCalleeContract::LabeledFreeName {
            name,
            first_label,
            shadow,
        } => {
            free_name_dependency_safe_at_span(il, interner, record, callee_span, name, shadow)
                && call_first_arg_label_matches_at_span(il, interner, call_span, first_label)
        }
        LibraryApiCalleeContract::RustMacro { name, shadow } => {
            dependency_has_source_call(il, record, call_span, SourceCallKind::MacroInvocation)
                && free_name_dependency_safe_at_span(
                    il,
                    interner,
                    record,
                    callee_span,
                    name,
                    shadow,
                )
        }
        LibraryApiCalleeContract::JsGlobalConstructor {
            receiver,
            requires_unshadowed_global,
        } => {
            dependency_has_source_call(il, record, call_span, SourceCallKind::Construct)
                && (!requires_unshadowed_global
                    || callee_span.is_some_and(|span| {
                        dependency_has_unshadowed_global_anchor(
                            il,
                            record,
                            span,
                            NodeKind::Var,
                            receiver,
                        )
                    }))
        }
        LibraryApiCalleeContract::ImportedBinding { module, exported } => {
            if let Some(span) = receiver_span {
                callee_span.is_some_and(|callee_span| {
                    field_method_receiver_matches_span(il, interner, callee_span, exported, span)
                }) && dependency_has_imported_namespace_anchor(
                    il,
                    interner,
                    record,
                    span,
                    NodeKind::Var,
                    module,
                )
            } else if let Some(span) = callee_span {
                dependency_has_imported_binding_anchor(
                    il,
                    interner,
                    record,
                    span,
                    NodeKind::Var,
                    module,
                    exported,
                )
            } else {
                dependency_has_imported_binding_dependency(il, interner, record, module, exported)
            }
        }
        _ => false,
    }
}

fn free_name_dependency_safe_at_span(
    il: &Il,
    interner: &Interner,
    record: &EvidenceRecord,
    callee_span: Option<Span>,
    name: &str,
    shadow: LibraryApiShadowPolicy,
) -> bool {
    let Some(span) = callee_span else {
        return false;
    };
    dependency_has_unshadowed_global_anchor(il, record, span, NodeKind::Var, name)
        && library_api_free_name_shadow_safe(il.meta.lang, name, shadow, |candidate| {
            file_defines_name_visible_at(il, interner, candidate, span)
        })
}

fn call_first_arg_label_matches_at_span(
    il: &Il,
    interner: &Interner,
    call_span: Span,
    expected: &str,
) -> bool {
    let Some(call) = node_at_span_with_kind(il, call_span, NodeKind::Call) else {
        return false;
    };
    call_first_arg_label_matches(il, interner, call, expected)
}

fn field_method_receiver_matches_span(
    il: &Il,
    interner: &Interner,
    callee_span: Span,
    method: &str,
    receiver_span: Span,
) -> bool {
    let Some(callee) = node_at_span_with_kind(il, callee_span, NodeKind::Field) else {
        return false;
    };
    field_method_at_span(il, interner, callee_span, method)
        && il
            .children(callee)
            .first()
            .is_some_and(|&receiver| il.node(receiver).span == receiver_span)
}

fn static_receiver_import_proven_at_span(
    il: &Il,
    interner: &Interner,
    record: &EvidenceRecord,
    receiver_span: Option<Span>,
    module: &str,
    receiver: &str,
) -> bool {
    if let Some(span) = receiver_span {
        dependency_has_imported_binding_anchor(
            il,
            interner,
            record,
            span,
            NodeKind::Var,
            module,
            receiver,
        )
    } else {
        dependency_has_imported_binding_dependency(il, interner, record, module, receiver)
    }
}

fn static_receiver_shadow_safe_at_span(
    il: &Il,
    interner: &Interner,
    receiver_span: Option<Span>,
    receiver: &str,
) -> bool {
    if let Some(span) = receiver_span {
        !unit_defines_hash_visible_at(il, interner, stable_symbol_hash(receiver), span)
    } else {
        !unit_defines_hash(il, interner, stable_symbol_hash(receiver))
    }
}

pub(in crate::library_api) fn library_api_dependencies_match_static_import_callee_at_span(
    il: &Il,
    interner: &Interner,
    call_span: Span,
    callee_span: Option<Span>,
    receiver_span: Option<Span>,
    callee: LibraryApiCalleeContract,
    record: &EvidenceRecord,
) -> bool {
    match callee {
        LibraryApiCalleeContract::JavaUtilStaticMember { owner, .. } => {
            static_receiver_dependency_safe_at_span(
                il,
                interner,
                record,
                receiver_span,
                StaticReceiverDependency { owner },
            )
        }
        LibraryApiCalleeContract::JavaStaticMember { owner, .. } => {
            static_receiver_dependency_safe_at_span(
                il,
                interner,
                record,
                receiver_span,
                StaticReceiverDependency { owner },
            )
        }
        LibraryApiCalleeContract::JavaUtilConstructor { type_ref } => {
            dependency_has_source_call(il, record, call_span, SourceCallKind::Construct)
                && callee_span.is_some_and(|span| {
                    java_constructor_dependencies_match_at_span(
                        il, interner, record, span, call_span, type_ref,
                    )
                })
        }
        LibraryApiCalleeContract::RubyRequireStaticMember {
            receiver,
            required_module,
            shadow_root,
            ..
        } => {
            receiver_span.is_some_and(|span| {
                dependency_has_unshadowed_global_anchor(il, record, span, NodeKind::Var, receiver)
            }) && dependency_has_required_module_before(
                record,
                il,
                interner,
                required_module,
                call_span,
            ) && receiver_span
                .is_some_and(|span| !file_defines_name_visible_at(il, interner, shadow_root, span))
        }
        _ => false,
    }
}

#[derive(Clone, Copy)]
struct StaticReceiverDependency {
    owner: JavaTypeReference,
}

fn static_receiver_dependency_safe_at_span(
    il: &Il,
    interner: &Interner,
    record: &EvidenceRecord,
    receiver_span: Option<Span>,
    dependency: StaticReceiverDependency,
) -> bool {
    static_receiver_import_proven_at_span(
        il,
        interner,
        record,
        receiver_span,
        dependency.owner.module(),
        dependency.owner.simple_type(),
    ) && static_receiver_shadow_safe_at_span(
        il,
        interner,
        receiver_span,
        dependency.owner.simple_type(),
    )
}

pub(in crate::library_api) fn library_api_dependencies_match_static_member_callee_at_span(
    il: &Il,
    interner: &Interner,
    callee_span: Option<Span>,
    receiver_span: Option<Span>,
    callee: LibraryApiCalleeContract,
    record: &EvidenceRecord,
) -> bool {
    match callee {
        LibraryApiCalleeContract::RegexLiteralMethod {
            required_receiver_fact,
            ..
        } => receiver_span.is_some_and(|span| {
            dependency_has_source_fact_anchor(il, record, span, required_receiver_fact)
        }),
        LibraryApiCalleeContract::Property { .. } => false,
        LibraryApiCalleeContract::StaticIndexMembershipMethod { method, receiver } => {
            callee_span.is_some_and(|span| field_method_at_span(il, interner, span, method))
                && receiver_span.is_some_and(|span| {
                    static_index_membership_receiver_dependency_id_at_span(
                        il, interner, span, receiver,
                    )
                    .is_some_and(|dependency| dependency_ids_are_present(record, &[dependency]))
                })
        }
        LibraryApiCalleeContract::ImportedNamespaceFunction { module, .. } => {
            if let Some(span) = receiver_span {
                dependency_has_imported_namespace_anchor(
                    il,
                    interner,
                    record,
                    span,
                    NodeKind::Var,
                    module,
                )
            } else {
                dependency_has_imported_namespace_dependency(il, interner, record, module)
            }
        }
        LibraryApiCalleeContract::StaticGlobalMethod {
            receiver,
            qualified_path,
            requires_unshadowed_receiver,
            ..
        } => {
            callee_span.is_some_and(|span| {
                dependency_has_qualified_global_anchor(
                    il,
                    record,
                    span,
                    NodeKind::Field,
                    qualified_path,
                )
            }) && (!requires_unshadowed_receiver
                || receiver_span.is_some_and(|span| {
                    dependency_has_unshadowed_global_anchor(
                        il,
                        record,
                        span,
                        NodeKind::Var,
                        receiver,
                    )
                }))
                && static_global_method_extra_dependencies_match_at_span(
                    il,
                    interner,
                    record,
                    qualified_path,
                )
        }
        LibraryApiCalleeContract::StaticGlobalFunction {
            function,
            requires_unshadowed_function,
        } => {
            !requires_unshadowed_function
                || callee_span.is_some_and(|span| {
                    dependency_has_unshadowed_global_anchor(
                        il,
                        record,
                        span,
                        NodeKind::Var,
                        function,
                    )
                })
        }
        _ => false,
    }
}

fn static_global_method_extra_dependencies_match_at_span(
    il: &Il,
    interner: &Interner,
    record: &EvidenceRecord,
    qualified_path: &str,
) -> bool {
    if qualified_path != "Object.keys" {
        return true;
    }
    let Some(call) = node_at_span_with_kind(il, record.anchor.span(), NodeKind::Call) else {
        return false;
    };
    js_object_key_view_argument_dependency_ids_for_call(il, interner, call)
        .is_some_and(|dependencies| dependency_ids_are_present(record, &dependencies))
}

pub(in crate::library_api) fn library_api_dependencies_match_method_callee_at_span(
    il: &Il,
    interner: &Interner,
    call_span: Span,
    callee_span: Option<Span>,
    receiver_span: Option<Span>,
    callee: LibraryApiCalleeContract,
    record: &EvidenceRecord,
) -> bool {
    match callee {
        LibraryApiCalleeContract::Method { method, receiver } => {
            if !callee_span.is_some_and(|span| field_method_at_span(il, interner, span, method)) {
                return false;
            }
            if let Some(matches) = call_anchored_method_dependencies_match(
                il,
                interner,
                call_span,
                callee_span,
                receiver_span,
                callee,
                record,
            ) {
                return matches;
            }
            if receiver == MethodReceiverContract::ExactProtocolPairArgument
                || receiver == MethodReceiverContract::UnshadowedGlobal("Math")
            {
                return false;
            }
            receiver_span.is_some_and(|span| {
                method_receiver_dependencies_at_span(il, interner, span, receiver)
                    .is_some_and(|dependencies| dependency_ids_are_present(record, &dependencies))
            })
        }
        LibraryApiCalleeContract::IteratorAdapterMethod { method, receiver } => {
            callee_span.is_some_and(|span| field_method_at_span(il, interner, span, method))
                && receiver_span.is_some_and(|span| {
                    iterator_adapter_receiver_dependencies_at_span(il, interner, span, receiver)
                        .is_some_and(|dependencies| {
                            dependency_ids_are_present(record, &dependencies)
                        })
                })
        }
        LibraryApiCalleeContract::AsyncMethod { method, receiver } => {
            callee_span.is_some_and(|span| field_method_at_span(il, interner, span, method))
                && receiver_span.is_some_and(|span| {
                    async_receiver_dependencies_at_span(il, interner, span, receiver).is_some_and(
                        |dependencies| dependency_ids_are_present(record, &dependencies),
                    )
                })
        }
        _ => false,
    }
}

fn call_anchored_method_dependencies_match(
    il: &Il,
    interner: &Interner,
    call_span: Span,
    callee_span: Option<Span>,
    receiver_span: Option<Span>,
    callee: LibraryApiCalleeContract,
    record: &EvidenceRecord,
) -> Option<bool> {
    let source_call = node_at_span_with_kind(il, call_span, NodeKind::Call)?;
    if !source_call_spans_match_span_query(il, source_call, callee_span, receiver_span) {
        return None;
    }
    Some(
        library_api_receiver_dependencies_for_call(il, interner, source_call, callee)
            .is_some_and(|dependencies| dependency_ids_are_present(record, &dependencies)),
    )
}

fn source_call_spans_match_span_query(
    il: &Il,
    source_call: NodeId,
    callee_span: Option<Span>,
    receiver_span: Option<Span>,
) -> bool {
    let Some(&callee) = il.children(source_call).first() else {
        return false;
    };
    if callee_span.is_some_and(|span| il.node(callee).span != span) {
        return false;
    }
    if let Some(span) = receiver_span {
        let Some(&receiver) = il.children(callee).first() else {
            return false;
        };
        if il.node(receiver).span != span {
            return false;
        }
    }
    true
}
