use super::java::*;
use super::*;

pub(in crate::library_api) fn library_api_dependencies_match_callee(
    il: &Il,
    interner: &Interner,
    node: NodeId,
    callee: LibraryApiCalleeContract,
    record: &EvidenceRecord,
) -> bool {
    let Some(&callee_node) = il.children(node).first() else {
        return false;
    };
    match callee {
        LibraryApiCalleeContract::FreeName { .. }
        | LibraryApiCalleeContract::LabeledFreeName { .. }
        | LibraryApiCalleeContract::RustMacro { .. }
        | LibraryApiCalleeContract::JsGlobalConstructor { .. }
        | LibraryApiCalleeContract::ImportedBinding { .. } => {
            library_api_dependencies_match_named_callee(
                il,
                interner,
                node,
                callee_node,
                callee,
                record,
            )
        }
        LibraryApiCalleeContract::JavaUtilStaticMember { .. }
        | LibraryApiCalleeContract::JavaStaticMember { .. }
        | LibraryApiCalleeContract::JavaUtilConstructor { .. }
        | LibraryApiCalleeContract::RubyRequireStaticMember { .. } => {
            library_api_dependencies_match_static_import_callee(
                il,
                interner,
                node,
                callee_node,
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
            library_api_dependencies_match_static_member_callee(
                il,
                interner,
                callee_node,
                callee,
                record,
            )
        }
        LibraryApiCalleeContract::Method { .. }
        | LibraryApiCalleeContract::IteratorAdapterMethod { .. }
        | LibraryApiCalleeContract::AsyncMethod { .. } => {
            library_api_dependencies_match_method_callee(il, interner, node, callee, record)
        }
    }
}

pub(in crate::library_api) fn library_api_dependencies_match_named_callee(
    il: &Il,
    interner: &Interner,
    node: NodeId,
    callee_node: NodeId,
    callee: LibraryApiCalleeContract,
    record: &EvidenceRecord,
) -> bool {
    match callee {
        LibraryApiCalleeContract::FreeName { name, shadow } => {
            dependency_has_unshadowed_global_node(il, record, callee_node, name)
                && library_api_free_name_shadow_safe(il.meta.lang, name, shadow, |candidate| {
                    file_defines_name_visible_at(il, interner, candidate, il.node(callee_node).span)
                })
        }
        LibraryApiCalleeContract::LabeledFreeName {
            name,
            first_label,
            shadow,
        } => {
            dependency_has_unshadowed_global_node(il, record, callee_node, name)
                && call_first_arg_label_matches(il, interner, node, first_label)
                && library_api_free_name_shadow_safe(il.meta.lang, name, shadow, |candidate| {
                    file_defines_name_visible_at(il, interner, candidate, il.node(callee_node).span)
                })
        }
        LibraryApiCalleeContract::RustMacro { name, shadow } => {
            dependency_has_source_call(
                il,
                record,
                il.node(node).span,
                SourceCallKind::MacroInvocation,
            ) && dependency_has_unshadowed_global_node(il, record, callee_node, name)
                && library_api_free_name_shadow_safe(il.meta.lang, name, shadow, |candidate| {
                    file_defines_name_visible_at(il, interner, candidate, il.node(callee_node).span)
                })
        }
        LibraryApiCalleeContract::JsGlobalConstructor {
            receiver,
            requires_unshadowed_global,
        } => {
            dependency_has_source_call(il, record, il.node(node).span, SourceCallKind::Construct)
                && (!requires_unshadowed_global
                    || dependency_has_unshadowed_global_node(il, record, callee_node, receiver))
        }
        LibraryApiCalleeContract::ImportedBinding { module, exported } => {
            dependency_has_imported_member_node(il, interner, record, callee_node, module, exported)
        }
        _ => false,
    }
}

pub(in crate::library_api) fn library_api_dependencies_match_static_import_callee(
    il: &Il,
    interner: &Interner,
    node: NodeId,
    callee_node: NodeId,
    callee: LibraryApiCalleeContract,
    record: &EvidenceRecord,
) -> bool {
    match callee {
        LibraryApiCalleeContract::JavaUtilStaticMember { receiver, .. } => {
            let Some(receiver_node) = il.children(callee_node).first().copied() else {
                return false;
            };
            dependency_has_imported_binding_node(
                il,
                interner,
                record,
                receiver_node,
                "java.util",
                receiver,
            ) && !unit_defines_hash_visible_at(
                il,
                interner,
                stable_symbol_hash(receiver),
                il.node(receiver_node).span,
            )
        }
        LibraryApiCalleeContract::JavaStaticMember {
            module,
            receiver,
            requires_import_for_simple_receiver,
            requires_no_local_type_shadow,
            ..
        } => {
            let Some(receiver_node) = il.children(callee_node).first().copied() else {
                return false;
            };
            (!requires_import_for_simple_receiver
                || dependency_has_imported_binding_node(
                    il,
                    interner,
                    record,
                    receiver_node,
                    module,
                    receiver,
                ))
                && (!requires_no_local_type_shadow
                    || !unit_defines_hash_visible_at(
                        il,
                        interner,
                        stable_symbol_hash(receiver),
                        il.node(receiver_node).span,
                    ))
        }
        LibraryApiCalleeContract::JavaUtilConstructor {
            simple_type,
            qualified_type,
            module,
            requires_import_for_simple_type,
            requires_no_local_type_shadow,
        } => {
            dependency_has_source_call(il, record, il.node(node).span, SourceCallKind::Construct)
                && java_constructor_dependencies_match(
                    il,
                    interner,
                    record,
                    callee_node,
                    il.node(node).span,
                    simple_type,
                    qualified_type,
                    module,
                    requires_import_for_simple_type,
                    requires_no_local_type_shadow,
                )
        }
        LibraryApiCalleeContract::RubyRequireStaticMember {
            receiver,
            required_module,
            shadow_root,
            ..
        } => {
            let Some(receiver_node) = il.children(callee_node).first().copied() else {
                return false;
            };
            dependency_has_unshadowed_global_node(il, record, receiver_node, receiver)
                && dependency_has_required_module_before(
                    record,
                    il,
                    interner,
                    required_module,
                    il.node(node).span,
                )
                && !file_defines_name_visible_at(
                    il,
                    interner,
                    shadow_root,
                    il.node(receiver_node).span,
                )
        }
        _ => false,
    }
}

pub(in crate::library_api) fn library_api_dependencies_match_static_member_callee(
    il: &Il,
    interner: &Interner,
    callee_node: NodeId,
    callee: LibraryApiCalleeContract,
    record: &EvidenceRecord,
) -> bool {
    match callee {
        LibraryApiCalleeContract::RegexLiteralMethod {
            required_receiver_fact,
            ..
        } => {
            let Some(receiver_node) = il.children(callee_node).first().copied() else {
                return false;
            };
            dependency_has_source_fact_node(il, record, receiver_node, required_receiver_fact)
        }
        LibraryApiCalleeContract::Property { .. } => false,
        LibraryApiCalleeContract::StaticIndexMembershipMethod { method, receiver } => {
            let Some(receiver_node) = method_callee_receiver(il, interner, callee_node, method)
            else {
                return false;
            };
            static_index_membership_receiver_dependency_id(il, interner, receiver_node, receiver)
                .is_some_and(|dependency| dependency_ids_are_present(record, &[dependency]))
        }
        LibraryApiCalleeContract::ImportedNamespaceFunction { module, .. } => {
            let Some(receiver_node) = il.children(callee_node).first().copied() else {
                return false;
            };
            dependency_has_imported_namespace_node(il, interner, record, receiver_node, module)
        }
        LibraryApiCalleeContract::StaticGlobalMethod {
            receiver,
            qualified_path,
            requires_unshadowed_receiver,
            ..
        } => {
            let Some(receiver_node) = il.children(callee_node).first().copied() else {
                return false;
            };
            dependency_has_qualified_global_node(il, record, callee_node, qualified_path)
                && (!requires_unshadowed_receiver
                    || dependency_has_unshadowed_global_node(il, record, receiver_node, receiver))
                && static_global_method_extra_dependencies_match(
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
                || dependency_has_unshadowed_global_node(il, record, callee_node, function)
        }
        _ => false,
    }
}

fn static_global_method_extra_dependencies_match(
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

pub(in crate::library_api) fn library_api_dependencies_match_method_callee(
    il: &Il,
    interner: &Interner,
    node: NodeId,
    callee: LibraryApiCalleeContract,
    record: &EvidenceRecord,
) -> bool {
    match callee {
        LibraryApiCalleeContract::Method { .. }
        | LibraryApiCalleeContract::IteratorAdapterMethod { .. } => {
            library_api_receiver_dependencies_for_call(il, interner, node, callee)
                .is_some_and(|dependencies| dependency_ids_are_present(record, &dependencies))
        }
        LibraryApiCalleeContract::AsyncMethod { .. } => {
            library_api_receiver_dependencies_for_call(il, interner, node, callee)
                .is_some_and(|dependencies| dependency_ids_are_present(record, &dependencies))
        }
        _ => false,
    }
}

pub(in crate::library_api) fn library_api_dependencies_match_callee_node(
    il: &Il,
    interner: &Interner,
    node: NodeId,
    callee: LibraryApiCalleeContract,
    record: &EvidenceRecord,
) -> bool {
    match callee {
        LibraryApiCalleeContract::FreeName { name, shadow } => {
            dependency_has_unshadowed_global_node(il, record, node, name)
                && library_api_free_name_shadow_safe(il.meta.lang, name, shadow, |candidate| {
                    file_defines_name_visible_at(il, interner, candidate, il.node(node).span)
                })
        }
        LibraryApiCalleeContract::Property { .. } => {
            let mut cache = LibraryApiDependencyCache::default();
            library_api_property_dependencies_for_field_with_cache(
                il, interner, node, callee, &mut cache,
            )
            .is_some_and(|dependencies| dependency_ids_are_present(record, &dependencies))
        }
        _ => false,
    }
}
