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
            callee_span.is_some_and(|span| {
                dependency_has_unshadowed_global_anchor(il, record, span, NodeKind::Var, name)
            }) && library_api_free_name_shadow_safe(il.meta.lang, name, shadow, |candidate| {
                callee_span
                    .is_some_and(|span| file_defines_name_visible_at(il, interner, candidate, span))
            })
        }
        LibraryApiCalleeContract::LabeledFreeName {
            name,
            first_label,
            shadow,
        } => {
            callee_span.is_some_and(|span| {
                dependency_has_unshadowed_global_anchor(il, record, span, NodeKind::Var, name)
            }) && call_first_arg_label_matches_at_span(il, interner, call_span, first_label)
                && library_api_free_name_shadow_safe(il.meta.lang, name, shadow, |candidate| {
                    callee_span.is_some_and(|span| {
                        file_defines_name_visible_at(il, interner, candidate, span)
                    })
                })
        }
        LibraryApiCalleeContract::RustMacro { name, shadow } => {
            dependency_has_source_call(il, record, call_span, SourceCallKind::MacroInvocation)
                && callee_span.is_some_and(|span| {
                    dependency_has_unshadowed_global_anchor(il, record, span, NodeKind::Var, name)
                })
                && library_api_free_name_shadow_safe(il.meta.lang, name, shadow, |candidate| {
                    callee_span.is_some_and(|span| {
                        file_defines_name_visible_at(il, interner, candidate, span)
                    })
                })
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
    required: bool,
) -> bool {
    if !required {
        return true;
    }
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
    required: bool,
) -> bool {
    if !required {
        return true;
    }
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
        LibraryApiCalleeContract::JavaUtilStaticMember { receiver, .. } => {
            static_receiver_import_proven_at_span(
                il,
                interner,
                record,
                receiver_span,
                "java.util",
                receiver,
                true,
            ) && static_receiver_shadow_safe_at_span(il, interner, receiver_span, receiver, true)
        }
        LibraryApiCalleeContract::JavaStaticMember {
            module,
            receiver,
            requires_import_for_simple_receiver,
            requires_no_local_type_shadow,
            ..
        } => {
            static_receiver_import_proven_at_span(
                il,
                interner,
                record,
                receiver_span,
                module,
                receiver,
                requires_import_for_simple_receiver,
            ) && static_receiver_shadow_safe_at_span(
                il,
                interner,
                receiver_span,
                receiver,
                requires_no_local_type_shadow,
            )
        }
        LibraryApiCalleeContract::JavaUtilConstructor {
            simple_type,
            qualified_type,
            module,
            requires_import_for_simple_type,
            requires_no_local_type_shadow,
        } => {
            dependency_has_source_call(il, record, call_span, SourceCallKind::Construct)
                && callee_span.is_some_and(|span| {
                    java_constructor_dependencies_match_at_span(
                        il,
                        interner,
                        record,
                        span,
                        call_span,
                        simple_type,
                        qualified_type,
                        module,
                        requires_import_for_simple_type,
                        requires_no_local_type_shadow,
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
            if receiver == MethodReceiverContract::UnshadowedGlobal("Math") {
                let Some(source_call) = node_at_span_with_kind(il, call_span, NodeKind::Call)
                else {
                    return false;
                };
                return library_api_receiver_dependencies_for_call(
                    il,
                    interner,
                    source_call,
                    callee,
                )
                .is_some_and(|dependencies| dependency_ids_are_present(record, &dependencies));
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
