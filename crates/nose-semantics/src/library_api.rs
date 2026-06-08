//! Library and standard-library API contracts plus occurrence-evidence admission.
//!
//! Contract rows describe first-party API semantics. Admission remains separate:
//! consumers only rely on a contract after matching `LibraryApi` evidence and its
//! source/import/symbol/domain dependencies.

use super::*;
use crate::evidence::span_contains;

mod contracts;
pub use contracts::*;

pub fn library_api_contract_evidence_for_call(
    il: &Il,
    interner: &Interner,
    node: NodeId,
    id: LibraryApiContractId,
    callee: LibraryApiCalleeContract,
    arg_count: usize,
) -> LibraryApiEvidenceStatus {
    if il.kind(node) != NodeKind::Call || arg_count > u16::MAX as usize {
        return LibraryApiEvidenceStatus::Rejected;
    }
    let expected = LibraryApiEvidenceKind::Contract {
        contract_hash: library_api_contract_id_hash(id),
        callee_hash: library_api_callee_contract_hash(callee),
        arity: arg_count as u16,
    };
    let span = il.node(node).span;
    let mut saw_library_api_evidence = false;
    let mut admitted = false;
    for record in &il.evidence {
        if record.anchor != EvidenceAnchor::node(span, NodeKind::Call) {
            continue;
        }
        let EvidenceKind::LibraryApi(api) = record.kind else {
            continue;
        };
        saw_library_api_evidence = true;
        if record.status != EvidenceStatus::Asserted
            || api != expected
            || !il.evidence_dependencies_asserted(record)
            || !library_api_callee_shape_matches(il, interner, node, callee)
            || !library_api_dependencies_match_callee(il, interner, node, callee, record)
        {
            return LibraryApiEvidenceStatus::Rejected;
        }
        admitted = true;
    }
    if admitted {
        LibraryApiEvidenceStatus::Admitted
    } else if saw_library_api_evidence {
        LibraryApiEvidenceStatus::Rejected
    } else {
        LibraryApiEvidenceStatus::Missing
    }
}

pub fn library_api_contract_evidence_for_node(
    il: &Il,
    interner: &Interner,
    node: NodeId,
    id: LibraryApiContractId,
    callee: LibraryApiCalleeContract,
    arg_count: usize,
) -> LibraryApiEvidenceStatus {
    if arg_count > u16::MAX as usize {
        return LibraryApiEvidenceStatus::Rejected;
    }
    let expected = LibraryApiEvidenceKind::Contract {
        contract_hash: library_api_contract_id_hash(id),
        callee_hash: library_api_callee_contract_hash(callee),
        arity: arg_count as u16,
    };
    let anchor = EvidenceAnchor::node(il.node(node).span, il.kind(node));
    let mut saw_library_api_evidence = false;
    let mut admitted = false;
    for record in &il.evidence {
        if record.anchor != anchor {
            continue;
        }
        let EvidenceKind::LibraryApi(api) = record.kind else {
            continue;
        };
        saw_library_api_evidence = true;
        if record.status != EvidenceStatus::Asserted
            || api != expected
            || !il.evidence_dependencies_asserted(record)
            || !library_api_node_callee_shape_matches(il, interner, node, callee)
            || !library_api_dependencies_match_callee_node(il, interner, node, callee, record)
        {
            return LibraryApiEvidenceStatus::Rejected;
        }
        admitted = true;
    }
    if admitted {
        LibraryApiEvidenceStatus::Admitted
    } else if saw_library_api_evidence {
        LibraryApiEvidenceStatus::Rejected
    } else {
        LibraryApiEvidenceStatus::Missing
    }
}

pub fn library_api_contract_evidence_at_call_span(
    il: &Il,
    interner: &Interner,
    query: LibraryApiSpanEvidenceQuery,
) -> LibraryApiEvidenceStatus {
    let Some(span) = query.call_span else {
        return LibraryApiEvidenceStatus::Missing;
    };
    if query.arg_count > u16::MAX as usize {
        return LibraryApiEvidenceStatus::Rejected;
    }
    let expected = LibraryApiEvidenceKind::Contract {
        contract_hash: library_api_contract_id_hash(query.id),
        callee_hash: library_api_callee_contract_hash(query.callee),
        arity: query.arg_count as u16,
    };
    let source_call = node_at_span_with_kind(il, span, NodeKind::Call);
    let mut saw_library_api_evidence = false;
    let mut admitted = false;
    for record in &il.evidence {
        if record.anchor != EvidenceAnchor::node(span, NodeKind::Call) {
            continue;
        }
        let EvidenceKind::LibraryApi(api) = record.kind else {
            continue;
        };
        saw_library_api_evidence = true;
        let source_call_matches = source_call.is_some_and(|node| {
            library_api_source_call_spans_match_query(
                il,
                node,
                query.callee_span,
                query.receiver_span,
            ) && library_api_callee_shape_matches(il, interner, node, query.callee)
                && library_api_dependencies_match_callee(il, interner, node, query.callee, record)
        });
        let span_query_matches = library_api_dependencies_match_callee_at_span(
            il,
            interner,
            span,
            query.callee_span,
            query.receiver_span,
            query.callee,
            record,
        );
        if record.status != EvidenceStatus::Asserted
            || api != expected
            || !il.evidence_dependencies_asserted(record)
            || (!source_call_matches && !span_query_matches)
        {
            return LibraryApiEvidenceStatus::Rejected;
        }
        admitted = true;
    }
    if admitted {
        LibraryApiEvidenceStatus::Admitted
    } else if saw_library_api_evidence {
        LibraryApiEvidenceStatus::Rejected
    } else {
        LibraryApiEvidenceStatus::Missing
    }
}

fn library_api_source_call_spans_match_query(
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

pub fn library_api_receiver_dependencies_for_call(
    il: &Il,
    interner: &Interner,
    call: NodeId,
    callee: LibraryApiCalleeContract,
) -> Option<Vec<EvidenceId>> {
    let mut cache = LibraryApiDependencyCache::default();
    library_api_receiver_dependencies_for_call_with_cache(il, interner, call, callee, &mut cache)
}

#[derive(Default)]
pub struct LibraryApiDependencyCache {
    nearest_scope_by_node: FxHashMap<NodeId, Option<NodeId>>,
    binding_lhs_by_reference: FxHashMap<NodeId, EvidenceResolution<NodeId>>,
    receiver_param_span_by_reference: FxHashMap<NodeId, Option<Span>>,
    name_assigned_in_scope: FxHashMap<(NodeId, Symbol), bool>,
}

pub fn library_api_receiver_dependencies_for_call_with_cache(
    il: &Il,
    interner: &Interner,
    call: NodeId,
    callee: LibraryApiCalleeContract,
    cache: &mut LibraryApiDependencyCache,
) -> Option<Vec<EvidenceId>> {
    let (&callee_node, args) = il.children(call).split_first()?;
    match callee {
        LibraryApiCalleeContract::Method { method, receiver } => {
            let receiver_node = method_callee_receiver(il, interner, callee_node, method)?;
            method_receiver_dependency_ids(il, interner, receiver_node, receiver, args, cache)
        }
        LibraryApiCalleeContract::IteratorAdapterMethod { method, receiver } => {
            let receiver_node = method_callee_receiver(il, interner, callee_node, method)?;
            iterator_adapter_receiver_dependency_ids(il, interner, receiver_node, receiver, cache)
        }
        LibraryApiCalleeContract::AsyncMethod { .. } => None,
        LibraryApiCalleeContract::StaticIndexMembershipMethod { method, receiver } => {
            let receiver_node = method_callee_receiver(il, interner, callee_node, method)?;
            static_index_membership_receiver_dependency_id(il, interner, receiver_node, receiver)
                .map(|dependency| vec![dependency])
        }
        _ => Some(Vec::new()),
    }
}

pub fn library_api_property_dependencies_for_field_with_cache(
    il: &Il,
    interner: &Interner,
    field: NodeId,
    callee: LibraryApiCalleeContract,
    cache: &mut LibraryApiDependencyCache,
) -> Option<Vec<EvidenceId>> {
    let LibraryApiCalleeContract::Property { property, receiver } = callee else {
        return None;
    };
    if !field_method_matches(il, interner, field, property) {
        return None;
    }
    let receiver_node = il.children(field).first().copied()?;
    method_receiver_dependency_ids(il, interner, receiver_node, receiver, &[], cache)
}

fn library_api_callee_shape_matches(
    il: &Il,
    interner: &Interner,
    node: NodeId,
    callee: LibraryApiCalleeContract,
) -> bool {
    let Some(&callee_node) = il.children(node).first() else {
        return false;
    };
    match callee {
        LibraryApiCalleeContract::FreeName { .. } | LibraryApiCalleeContract::RustMacro { .. } => {
            il.kind(callee_node) == NodeKind::Var
        }
        LibraryApiCalleeContract::JsGlobalConstructor { receiver, .. } => {
            var_name_matches(il, interner, callee_node, receiver)
        }
        LibraryApiCalleeContract::ImportedBinding { exported, .. } => {
            imported_member_callee_shape_matches(il, interner, callee_node, exported)
        }
        LibraryApiCalleeContract::JavaUtilStaticMember { receiver, method } => {
            let Some((actual_receiver, actual_method)) =
                static_member_callee_parts(il, interner, callee_node)
            else {
                return false;
            };
            actual_receiver == receiver && actual_method == method
        }
        LibraryApiCalleeContract::JavaUtilConstructor {
            simple_type,
            qualified_type,
            ..
        } => {
            var_name_matches(il, interner, callee_node, simple_type)
                || var_name_matches(il, interner, callee_node, qualified_type)
        }
        LibraryApiCalleeContract::RubyRequireStaticMember { method, .. } => {
            if il.kind(callee_node) != NodeKind::Field {
                return false;
            }
            let Some(&receiver) = il.children(callee_node).first() else {
                return false;
            };
            il.kind(receiver) == NodeKind::Var
                && field_method_matches(il, interner, callee_node, method)
        }
        LibraryApiCalleeContract::RegexLiteralMethod { method, .. } => {
            field_method_matches(il, interner, callee_node, method)
        }
        LibraryApiCalleeContract::Property { .. } => false,
        LibraryApiCalleeContract::StaticIndexMembershipMethod { method, .. } => {
            method_callee_receiver(il, interner, callee_node, method).is_some()
        }
        LibraryApiCalleeContract::ImportedNamespaceFunction { function, .. } => {
            field_method_matches(il, interner, callee_node, function)
        }
        LibraryApiCalleeContract::StaticGlobalMethod {
            receiver, method, ..
        } => {
            let Some((actual_receiver, actual_method)) =
                static_member_callee_parts(il, interner, callee_node)
            else {
                return false;
            };
            actual_receiver == receiver && actual_method == method
        }
        LibraryApiCalleeContract::StaticGlobalFunction { function, .. } => {
            var_name_matches(il, interner, callee_node, function)
        }
        LibraryApiCalleeContract::Method { method, .. }
        | LibraryApiCalleeContract::AsyncMethod { method, .. }
        | LibraryApiCalleeContract::IteratorAdapterMethod { method, .. } => {
            method_callee_receiver(il, interner, callee_node, method).is_some()
        }
    }
}

fn library_api_dependencies_match_callee(
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
        LibraryApiCalleeContract::FreeName { name, shadow } => {
            dependency_has_unshadowed_global_node(il, record, callee_node, name)
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
        }
        LibraryApiCalleeContract::StaticGlobalFunction {
            function,
            requires_unshadowed_function,
        } => {
            !requires_unshadowed_function
                || dependency_has_unshadowed_global_node(il, record, callee_node, function)
        }
        LibraryApiCalleeContract::Method { .. }
        | LibraryApiCalleeContract::IteratorAdapterMethod { .. } => {
            library_api_receiver_dependencies_for_call(il, interner, node, callee)
                .is_some_and(|dependencies| dependency_ids_are_present(record, &dependencies))
        }
        LibraryApiCalleeContract::AsyncMethod { .. } => false,
    }
}

fn library_api_node_callee_shape_matches(
    il: &Il,
    interner: &Interner,
    node: NodeId,
    callee: LibraryApiCalleeContract,
) -> bool {
    match callee {
        LibraryApiCalleeContract::FreeName { name, .. } => {
            var_name_matches(il, interner, node, name)
        }
        LibraryApiCalleeContract::Property { property, .. } => {
            field_method_matches(il, interner, node, property)
        }
        _ => false,
    }
}

fn library_api_dependencies_match_callee_node(
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

#[allow(clippy::too_many_arguments)]
fn java_constructor_dependencies_match(
    il: &Il,
    interner: &Interner,
    record: &EvidenceRecord,
    callee_node: NodeId,
    call_span: Span,
    simple_type: &str,
    qualified_type: &str,
    module: &str,
    requires_import_for_simple_type: bool,
    requires_no_local_type_shadow: bool,
) -> bool {
    let Some(actual) = node_name(il, interner, callee_node) else {
        return false;
    };
    java_constructor_dependencies_match_for_name(
        il,
        interner,
        record,
        actual,
        Some(callee_node),
        il.node(callee_node).span,
        call_span,
        simple_type,
        qualified_type,
        module,
        requires_import_for_simple_type,
        requires_no_local_type_shadow,
    )
}

#[allow(clippy::too_many_arguments)]
fn java_constructor_dependencies_match_at_span(
    il: &Il,
    interner: &Interner,
    record: &EvidenceRecord,
    callee_span: Span,
    call_span: Span,
    simple_type: &str,
    qualified_type: &str,
    module: &str,
    requires_import_for_simple_type: bool,
    requires_no_local_type_shadow: bool,
) -> bool {
    let Some(callee_node) = node_at_span_with_kind(il, callee_span, NodeKind::Var) else {
        return false;
    };
    java_constructor_dependencies_match(
        il,
        interner,
        record,
        callee_node,
        call_span,
        simple_type,
        qualified_type,
        module,
        requires_import_for_simple_type,
        requires_no_local_type_shadow,
    )
}

#[allow(clippy::too_many_arguments)]
fn java_constructor_dependencies_match_for_name(
    il: &Il,
    interner: &Interner,
    record: &EvidenceRecord,
    actual: &str,
    callee_node: Option<NodeId>,
    callee_span: Span,
    call_span: Span,
    simple_type: &str,
    qualified_type: &str,
    module: &str,
    requires_import_for_simple_type: bool,
    requires_no_local_type_shadow: bool,
) -> bool {
    if actual == qualified_type {
        return true;
    }
    if actual != simple_type {
        return false;
    }
    if requires_no_local_type_shadow
        && unit_defines_hash_visible_at(il, interner, stable_symbol_hash(simple_type), callee_span)
    {
        return false;
    }
    if !requires_import_for_simple_type {
        return true;
    }
    let explicit_import = callee_node.is_some_and(|node| {
        dependency_has_imported_binding_node(il, interner, record, node, module, simple_type)
    });
    explicit_import
        || dependency_has_java_wildcard_import_before(
            il,
            interner,
            record,
            module,
            simple_type,
            call_span,
        )
}

fn dependency_has_java_wildcard_import_before(
    il: &Il,
    interner: &Interner,
    record: &EvidenceRecord,
    module: &str,
    simple_type: &str,
    call_span: Span,
) -> bool {
    let expected = EvidenceKind::Import(ImportEvidenceKind::Wildcard {
        module_hash: stable_symbol_hash(module),
    });
    record.dependencies.iter().any(|&id| {
        let Some(dependency) = il.evidence_record_by_id(id) else {
            return false;
        };
        dependency.status == EvidenceStatus::Asserted
            && dependency.kind == expected
            && matches!(
                dependency.anchor,
                EvidenceAnchor::SourceSpan(span)
                    if span.file == call_span.file && span.end_byte <= call_span.start_byte
            )
            && !java_explicit_import_conflicts(il, interner, module, simple_type)
    })
}

fn java_explicit_import_conflicts(
    il: &Il,
    _interner: &Interner,
    module: &str,
    simple_type: &str,
) -> bool {
    let local_hash = stable_symbol_hash(simple_type);
    let expected = SymbolEvidenceKind::ImportedBinding {
        module_hash: stable_symbol_hash(module),
        exported_hash: stable_symbol_hash(simple_type),
    };
    il.evidence.iter().any(|record| {
        matches!(
            record.anchor,
            EvidenceAnchor::Binding {
                local_hash: anchor_hash,
                ..
            } if anchor_hash == local_hash
        ) && matches!(record.kind, EvidenceKind::Symbol(actual) if actual != expected)
            && record.status == EvidenceStatus::Asserted
    })
}

fn library_api_dependencies_match_callee_at_span(
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
                dependency_has_imported_namespace_anchor(
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
                ) || dependency_has_imported_namespace_dependency(il, interner, record, module)
            } else {
                dependency_has_imported_binding_dependency(il, interner, record, module, exported)
                    || dependency_has_imported_namespace_dependency(il, interner, record, module)
            }
        }
        LibraryApiCalleeContract::JavaUtilStaticMember { receiver, .. } => {
            let receiver_proven = if let Some(span) = receiver_span {
                dependency_has_imported_binding_anchor(
                    il,
                    interner,
                    record,
                    span,
                    NodeKind::Var,
                    "java.util",
                    receiver,
                )
            } else {
                dependency_has_imported_binding_dependency(
                    il,
                    interner,
                    record,
                    "java.util",
                    receiver,
                )
            };
            receiver_proven
                && if let Some(span) = receiver_span {
                    !unit_defines_hash_visible_at(il, interner, stable_symbol_hash(receiver), span)
                } else {
                    !unit_defines_hash(il, interner, stable_symbol_hash(receiver))
                }
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
        LibraryApiCalleeContract::Method { method, receiver } => {
            callee_span.is_some_and(|span| field_method_at_span(il, interner, span, method))
                && receiver_span.is_some_and(|span| {
                    method_receiver_dependencies_at_span(il, interner, span, receiver).is_some_and(
                        |dependencies| dependency_ids_are_present(record, &dependencies),
                    )
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
        LibraryApiCalleeContract::AsyncMethod { .. } => false,
    }
}

fn method_callee_receiver(
    il: &Il,
    interner: &Interner,
    callee: NodeId,
    expected_method: &str,
) -> Option<NodeId> {
    if !field_method_matches(il, interner, callee, expected_method) {
        return None;
    }
    il.children(callee).first().copied()
}

fn field_method_at_span(il: &Il, interner: &Interner, span: Span, expected: &str) -> bool {
    il.nodes.iter().any(|node| {
        node.span == span
            && node.kind == NodeKind::Field
            && matches!(node.payload, Payload::Name(method) if interner.resolve(method) == expected)
    })
}

fn method_receiver_dependency_ids(
    il: &Il,
    interner: &Interner,
    receiver: NodeId,
    contract: MethodReceiverContract,
    args: &[NodeId],
    cache: &mut LibraryApiDependencyCache,
) -> Option<Vec<EvidenceId>> {
    let mut dependencies = receiver_dependency_ids(il, interner, receiver, contract, cache)?;
    if contract == MethodReceiverContract::ExactProtocolPairArgument {
        let pair = *args.first()?;
        dependencies.extend(receiver_dependency_ids(
            il,
            interner,
            pair,
            MethodReceiverContract::ExactProtocol,
            cache,
        )?);
    }
    Some(dependencies)
}

fn iterator_adapter_receiver_dependency_ids(
    il: &Il,
    interner: &Interner,
    receiver: NodeId,
    contract: IteratorAdapterReceiverContract,
    cache: &mut LibraryApiDependencyCache,
) -> Option<Vec<EvidenceId>> {
    match contract {
        IteratorAdapterReceiverContract::ExactIterableValue => receiver_dependency_ids(
            il,
            interner,
            receiver,
            MethodReceiverContract::ExactProtocol,
            cache,
        ),
    }
}

fn method_receiver_dependencies_at_span(
    il: &Il,
    interner: &Interner,
    receiver_span: Span,
    contract: MethodReceiverContract,
) -> Option<Vec<EvidenceId>> {
    let receiver = node_at_span(il, receiver_span)?;
    let mut cache = LibraryApiDependencyCache::default();
    receiver_dependency_ids(il, interner, receiver, contract, &mut cache)
}

fn iterator_adapter_receiver_dependencies_at_span(
    il: &Il,
    interner: &Interner,
    receiver_span: Span,
    contract: IteratorAdapterReceiverContract,
) -> Option<Vec<EvidenceId>> {
    let receiver = node_at_span(il, receiver_span)?;
    let mut cache = LibraryApiDependencyCache::default();
    iterator_adapter_receiver_dependency_ids(il, interner, receiver, contract, &mut cache)
}

fn node_at_span(il: &Il, span: Span) -> Option<NodeId> {
    let mut found = None;
    for (idx, node) in il.nodes.iter().enumerate() {
        if node.span != span {
            continue;
        }
        let id = NodeId(idx as u32);
        match found {
            None => found = Some(id),
            Some(existing)
                if il.kind(existing) == node.kind && il.node(existing).payload == node.payload => {}
            Some(_) => return None,
        }
    }
    found
}

fn node_at_span_with_kind(il: &Il, span: Span, kind: NodeKind) -> Option<NodeId> {
    let mut found = None;
    for (idx, node) in il.nodes.iter().enumerate() {
        if node.span != span || node.kind != kind {
            continue;
        }
        let id = NodeId(idx as u32);
        match found {
            None => found = Some(id),
            Some(existing) if il.node(existing).payload == node.payload => {}
            Some(_) => return None,
        }
    }
    found
}

fn receiver_dependency_ids(
    il: &Il,
    interner: &Interner,
    receiver: NodeId,
    contract: MethodReceiverContract,
    cache: &mut LibraryApiDependencyCache,
) -> Option<Vec<EvidenceId>> {
    match contract {
        MethodReceiverContract::LiteralString => {
            matches!(il.node(receiver).payload, Payload::LitStr(_)).then_some(Vec::new())
        }
        MethodReceiverContract::UnshadowedGlobal(global) => {
            Some(vec![symbol_dependency_id_for_node(
                il,
                receiver,
                SymbolEvidenceKind::UnshadowedGlobal {
                    name_hash: stable_symbol_hash(global),
                },
            )?])
        }
        MethodReceiverContract::ImportedNamespace(module) => {
            Some(vec![imported_symbol_dependency_id_for_node(
                il,
                interner,
                receiver,
                SymbolEvidenceKind::ImportedNamespace {
                    module_hash: stable_symbol_hash(module),
                },
            )?])
        }
        MethodReceiverContract::ExactMapLiteral => {
            Some(vec![sequence_surface_dependency_id_for_receiver(
                il, interner, receiver, contract,
            )?])
        }
        MethodReceiverContract::ExactCollectionOrMapLiteral => {
            domain_or_sequence_dependency_ids(il, interner, receiver, contract, cache)
        }
        MethodReceiverContract::ExactCollection | MethodReceiverContract::ExactCollectionOrMap => {
            if let Some(ids) =
                domain_or_sequence_dependency_ids(il, interner, receiver, contract, cache)
            {
                return Some(ids);
            }
            if let Some(id) =
                library_api_dependency_id_for_receiver_domain_call(il, interner, receiver, contract)
            {
                return Some(vec![id]);
            }
            library_api_dependency_id_for_map_key_view_call(
                il,
                interner,
                receiver,
                &[MapKeyViewKind::Collection],
            )
            .map(|id| vec![id])
        }
        MethodReceiverContract::RustMapGetOrExactOption => {
            if let Some(ids) =
                domain_or_sequence_dependency_ids(il, interner, receiver, contract, cache)
            {
                return Some(ids);
            }
            library_api_dependency_id_for_call(il, interner, receiver, LibraryApiContractId::MapGet)
                .map(|id| vec![id])
        }
        MethodReceiverContract::ExactCollectionOrJavaKeySet => {
            if let Some(ids) =
                domain_or_sequence_dependency_ids(il, interner, receiver, contract, cache)
            {
                return Some(ids);
            }
            if let Some(id) = library_api_dependency_id_for_call(
                il,
                interner,
                receiver,
                LibraryApiContractId::MapKeyView(MapKeyViewKind::Collection),
            ) {
                return Some(vec![id]);
            }
            library_api_dependency_id_for_receiver_domain_call(il, interner, receiver, contract)
                .map(|id| vec![id])
        }
        MethodReceiverContract::ExactProtocol => {
            if let Some(ids) =
                domain_or_sequence_dependency_ids(il, interner, receiver, contract, cache)
            {
                return Some(ids);
            }
            if let Some(id) = library_api_dependency_id_for_map_key_view_call(
                il,
                interner,
                receiver,
                &[MapKeyViewKind::Collection, MapKeyViewKind::Iterator],
            ) {
                return Some(vec![id]);
            }
            if let Some(id) =
                library_api_dependency_id_for_receiver_domain_call(il, interner, receiver, contract)
            {
                return Some(vec![id]);
            }
            if let Some(id) = library_api_dependency_id_for_normalized_hof(il, receiver) {
                return Some(vec![id]);
            }
            library_api_dependency_id_for_protocol_call(il, interner, receiver).map(|id| vec![id])
        }
        MethodReceiverContract::ExactProtocolPairArgument => domain_or_sequence_dependency_ids(
            il,
            interner,
            receiver,
            MethodReceiverContract::ExactProtocol,
            cache,
        )
        .or_else(|| {
            library_api_dependency_id_for_map_key_view_call(
                il,
                interner,
                receiver,
                &[MapKeyViewKind::Collection, MapKeyViewKind::Iterator],
            )
            .map(|id| vec![id])
        })
        .or_else(|| {
            library_api_dependency_id_for_receiver_domain_call(
                il,
                interner,
                receiver,
                MethodReceiverContract::ExactProtocol,
            )
            .map(|id| vec![id])
        })
        .or_else(|| library_api_dependency_id_for_normalized_hof(il, receiver).map(|id| vec![id]))
        .or_else(|| {
            library_api_dependency_id_for_protocol_call(il, interner, receiver).map(|id| vec![id])
        }),
        _ => domain_or_sequence_dependency_ids(il, interner, receiver, contract, cache).or_else(
            || {
                library_api_dependency_id_for_receiver_domain_call(il, interner, receiver, contract)
                    .map(|id| vec![id])
            },
        ),
    }
}

fn domain_or_sequence_dependency_ids(
    il: &Il,
    interner: &Interner,
    receiver: NodeId,
    contract: MethodReceiverContract,
    cache: &mut LibraryApiDependencyCache,
) -> Option<Vec<EvidenceId>> {
    if let Some(id) = domain_dependency_id_for_receiver(il, interner, receiver, contract, cache) {
        return Some(vec![id]);
    }
    sequence_surface_dependency_id_for_receiver(il, interner, receiver, contract).map(|id| vec![id])
}

fn domain_dependency_id_for_receiver(
    il: &Il,
    interner: &Interner,
    receiver: NodeId,
    contract: MethodReceiverContract,
    cache: &mut LibraryApiDependencyCache,
) -> Option<EvidenceId> {
    let requirement = method_receiver_domain_requirement(contract)?;
    let mut found = None;
    for record in &il.evidence {
        let EvidenceKind::Domain(domain) = record.kind else {
            continue;
        };
        if record.status != EvidenceStatus::Asserted
            || !il.evidence_dependencies_asserted(record)
            || !requirement.accepts(domain)
            || !domain_dependency_anchor_matches_receiver(
                il,
                interner,
                receiver,
                record.anchor,
                cache,
            )
        {
            continue;
        }
        match found {
            None => found = Some((domain, record.id)),
            Some((existing, _)) if existing == domain => {}
            Some(_) => return None,
        }
    }
    found.map(|(_, id)| id)
}

fn domain_dependency_anchor_matches_receiver(
    il: &Il,
    interner: &Interner,
    receiver: NodeId,
    anchor: EvidenceAnchor,
    cache: &mut LibraryApiDependencyCache,
) -> bool {
    match anchor {
        EvidenceAnchor::Node { span, kind } => {
            span == il.node(receiver).span && kind == il.kind(receiver)
        }
        EvidenceAnchor::Binding { span, local_hash } => {
            matches!(
                unique_binding_lhs_for_var_reference_cached(il, receiver, cache),
                EvidenceResolution::Found(lhs)
                    if il.node(lhs).span == span
                        && node_name_hash(il, interner, lhs) == Some(local_hash)
            )
        }
        EvidenceAnchor::Param { span } => {
            receiver_param_span_cached(il, receiver, cache) == Some(span)
        }
        _ => false,
    }
}

fn unique_binding_lhs_for_var_reference_cached(
    il: &Il,
    node: NodeId,
    cache: &mut LibraryApiDependencyCache,
) -> EvidenceResolution<NodeId> {
    if let Some(&cached) = cache.binding_lhs_by_reference.get(&node) {
        return cached;
    }
    let resolution = unique_binding_lhs_for_var_reference_with_cache(il, node, cache);
    cache.binding_lhs_by_reference.insert(node, resolution);
    resolution
}

fn unique_binding_lhs_for_var_reference_with_cache(
    il: &Il,
    node: NodeId,
    cache: &mut LibraryApiDependencyCache,
) -> EvidenceResolution<NodeId> {
    let scope = nearest_scope_cached(il, node, cache);
    let reference_is_free_name = matches!(il.node(node).payload, Payload::Name(_));
    let mut found = None;
    for (idx, candidate) in il.nodes.iter().enumerate() {
        if candidate.kind != NodeKind::Assign {
            continue;
        }
        let assign = NodeId(idx as u32);
        let assignment_scope = nearest_scope_cached(il, assign, cache);
        if assignment_scope != scope && !(reference_is_free_name && assignment_scope.is_none()) {
            continue;
        }
        if !assignment_is_visible_at_reference(il, assign, node) {
            continue;
        }
        let Some(&lhs) = il.children(assign).first() else {
            continue;
        };
        if !var_references_same_binding(il, lhs, node) {
            continue;
        }
        match found {
            None => found = Some(lhs),
            Some(existing) if existing == lhs => {}
            Some(_) => return EvidenceResolution::Ambiguous,
        }
    }
    found.map_or(EvidenceResolution::Missing, EvidenceResolution::Found)
}

fn nearest_scope_cached(
    il: &Il,
    node: NodeId,
    cache: &mut LibraryApiDependencyCache,
) -> Option<NodeId> {
    if let Some(cached) = cache.nearest_scope_by_node.get(&node).copied() {
        return cached;
    }
    let scope = nearest_scope(il, node);
    cache.nearest_scope_by_node.insert(node, scope);
    scope
}

fn receiver_param_span_cached(
    il: &Il,
    receiver: NodeId,
    cache: &mut LibraryApiDependencyCache,
) -> Option<Span> {
    if let Some(cached) = cache
        .receiver_param_span_by_reference
        .get(&receiver)
        .copied()
    {
        return cached;
    }
    let span = receiver_var_payload(il, receiver).and_then(|payload| match payload {
        Payload::Cid(cid) => receiver_cid_param_span_with_cache(il, receiver, cid, cache),
        Payload::Name(name) => receiver_named_param_span_with_cache(il, receiver, name, cache),
        _ => None,
    });
    cache
        .receiver_param_span_by_reference
        .insert(receiver, span);
    span
}

fn receiver_var_payload(il: &Il, receiver: NodeId) -> Option<Payload> {
    (il.kind(receiver) == NodeKind::Var).then_some(il.node(receiver).payload)
}

fn receiver_cid_param_span_with_cache(
    il: &Il,
    receiver: NodeId,
    cid: u32,
    cache: &mut LibraryApiDependencyCache,
) -> Option<Span> {
    let scope = nearest_scope_cached(il, receiver, cache);
    let mut found = None;
    for (idx, candidate) in il.nodes.iter().enumerate() {
        if candidate.kind != NodeKind::Param {
            continue;
        }
        let id = NodeId(idx as u32);
        if nearest_scope_cached(il, id, cache) != scope {
            continue;
        }
        if !matches!(candidate.payload, Payload::Cid(param_cid) if param_cid == cid) {
            continue;
        }
        match found {
            None => found = Some(candidate.span),
            Some(existing) if existing == candidate.span => {}
            Some(_) => return None,
        }
    }
    found
}

fn receiver_named_param_span_with_cache(
    il: &Il,
    receiver: NodeId,
    name: Symbol,
    cache: &mut LibraryApiDependencyCache,
) -> Option<Span> {
    let (scope, param) = nearest_named_param_scope(il, receiver, name)?;
    (!name_is_assigned_in_scope_cached(il, name, scope, cache)).then_some(il.node(param).span)
}

fn name_is_assigned_in_scope_cached(
    il: &Il,
    name: Symbol,
    scope: NodeId,
    cache: &mut LibraryApiDependencyCache,
) -> bool {
    if let Some(&assigned) = cache.name_assigned_in_scope.get(&(scope, name)) {
        return assigned;
    }
    let assigned = il.nodes.iter().enumerate().any(|(idx, node)| {
        if node.kind != NodeKind::Assign {
            return false;
        }
        let id = NodeId(idx as u32);
        if nearest_scope_cached(il, id, cache) != Some(scope) {
            return false;
        }
        let Some(&lhs) = il.children(id).first() else {
            return false;
        };
        il.kind(lhs) == NodeKind::Var && il.node(lhs).payload == Payload::Name(name)
    });
    cache.name_assigned_in_scope.insert((scope, name), assigned);
    assigned
}

fn sequence_surface_dependency_id_for_receiver(
    il: &Il,
    interner: &Interner,
    receiver: NodeId,
    contract: MethodReceiverContract,
) -> Option<EvidenceId> {
    if il.kind(receiver) != NodeKind::Seq {
        return None;
    }
    let surface = seq_surface_contract_for_node(il, interner, receiver)?;
    if !sequence_surface_satisfies_method_receiver(surface, contract) {
        return None;
    }
    let anchor = EvidenceAnchor::sequence(il.node(receiver).span);
    let mut found = None;
    for record in &il.evidence {
        let EvidenceKind::SequenceSurface(kind) = record.kind else {
            continue;
        };
        if record.anchor != anchor
            || record.status != EvidenceStatus::Asserted
            || !il.evidence_dependencies_asserted(record)
        {
            continue;
        }
        match found {
            None => found = Some((kind, record.id)),
            Some((existing, _)) if existing == kind => {}
            Some(_) => return None,
        }
    }
    found.map(|(_, id)| id)
}

fn static_index_membership_receiver_dependency_id(
    il: &Il,
    interner: &Interner,
    receiver: NodeId,
    contract: StaticIndexMembershipReceiverContract,
) -> Option<EvidenceId> {
    static_index_membership_receiver_dependency_id_at_span(
        il,
        interner,
        il.node(receiver).span,
        contract,
    )
    .filter(|_| static_index_membership_receiver_shape_matches(il, interner, receiver, contract))
}

fn static_index_membership_receiver_dependency_id_at_span(
    il: &Il,
    interner: &Interner,
    span: Span,
    contract: StaticIndexMembershipReceiverContract,
) -> Option<EvidenceId> {
    let receiver = node_at_span_with_kind(il, span, NodeKind::Seq)?;
    if !static_index_membership_receiver_shape_matches(il, interner, receiver, contract) {
        return None;
    }
    let anchor = EvidenceAnchor::sequence(span);
    let mut found = None;
    for record in &il.evidence {
        let EvidenceKind::SequenceSurface(kind) = record.kind else {
            continue;
        };
        if record.anchor != anchor
            || record.status != EvidenceStatus::Asserted
            || !il.evidence_dependencies_asserted(record)
        {
            continue;
        }
        match found {
            None => found = Some((kind, record.id)),
            Some((existing, _)) if existing == kind => {}
            Some(_) => return None,
        }
    }
    found.and_then(|(kind, id)| (kind == SequenceSurfaceKind::Collection).then_some(id))
}

fn static_index_membership_receiver_shape_matches(
    il: &Il,
    interner: &Interner,
    receiver: NodeId,
    contract: StaticIndexMembershipReceiverContract,
) -> bool {
    match contract {
        StaticIndexMembershipReceiverContract::StaticNonFloatLiteralCollection => {
            if il.kind(receiver) != NodeKind::Seq {
                return false;
            }
            if !seq_surface_contract_for_node(il, interner, receiver)
                .is_some_and(|surface| surface.membership_collection)
            {
                return false;
            }
            let kids = il.children(receiver);
            !kids.is_empty()
                && kids.iter().all(|&kid| {
                    il.kind(kid) == NodeKind::Lit
                        && matches!(
                            il.node(kid).payload,
                            Payload::LitInt(_)
                                | Payload::LitBool(_)
                                | Payload::LitStr(_)
                                | Payload::Lit(LitClass::Null)
                        )
                })
        }
    }
}

fn sequence_surface_satisfies_method_receiver(
    surface: SeqSurfaceContract,
    contract: MethodReceiverContract,
) -> bool {
    match contract {
        MethodReceiverContract::ExactCollection
        | MethodReceiverContract::ExactProtocol
        | MethodReceiverContract::ExactProtocolPairArgument
        | MethodReceiverContract::ExactCollectionOrJavaKeySet => surface.membership_collection,
        MethodReceiverContract::ExactMap | MethodReceiverContract::ExactMapLiteral => {
            surface.value_tag == SEQ_VALUE_MAP
        }
        MethodReceiverContract::ExactCollectionOrMap
        | MethodReceiverContract::ExactCollectionOrMapLiteral => {
            surface.membership_collection || surface.value_tag == SEQ_VALUE_MAP
        }
        MethodReceiverContract::ExactSetOrMap => surface.value_tag == SEQ_VALUE_MAP,
        _ => false,
    }
}

fn symbol_dependency_id_for_node(
    il: &Il,
    node: NodeId,
    expected: SymbolEvidenceKind,
) -> Option<EvidenceId> {
    let anchor = EvidenceAnchor::node(il.node(node).span, il.kind(node));
    il.evidence.iter().find_map(|record| {
        (record.anchor == anchor
            && record.status == EvidenceStatus::Asserted
            && record.kind == EvidenceKind::Symbol(expected)
            && il.evidence_dependencies_asserted(record))
        .then_some(record.id)
    })
}

fn imported_symbol_dependency_id_for_node(
    il: &Il,
    interner: &Interner,
    node: NodeId,
    expected: SymbolEvidenceKind,
) -> Option<EvidenceId> {
    let anchor = EvidenceAnchor::node(il.node(node).span, il.kind(node));
    il.evidence.iter().find_map(|record| {
        (record.anchor == anchor
            && record.status == EvidenceStatus::Asserted
            && record.kind == EvidenceKind::Symbol(expected)
            && imported_occurrence_symbol_dependencies_valid(il, interner, record, expected))
        .then_some(record.id)
    })
}

pub(crate) fn library_api_dependency_id_for_normalized_hof(
    il: &Il,
    receiver: NodeId,
) -> Option<EvidenceId> {
    let Payload::HoF(kind) = il.node(receiver).payload else {
        return None;
    };
    let expected_id = LibraryApiContractId::MethodCall(MethodSemanticContract::HoF(kind));
    let expected_contract_hash = library_api_contract_id_hash(expected_id);
    let anchor = EvidenceAnchor::node(il.node(receiver).span, NodeKind::Call);
    let mut found = None;
    for record in &il.evidence {
        if record.anchor != anchor
            || record.status != EvidenceStatus::Asserted
            || !il.evidence_dependencies_asserted(record)
        {
            continue;
        }
        let EvidenceKind::LibraryApi(LibraryApiEvidenceKind::Contract {
            contract_hash,
            callee_hash,
            ..
        }) = record.kind
        else {
            continue;
        };
        if contract_hash != expected_contract_hash {
            continue;
        }
        if library_api_callee_contract_for_hash(il.meta.lang, expected_id, callee_hash).is_none() {
            continue;
        }
        match found {
            None => found = Some(record.id),
            Some(existing) if existing == record.id => {}
            Some(_) => return None,
        }
    }
    found
}

fn library_api_dependency_id_for_protocol_call(
    il: &Il,
    interner: &Interner,
    call: NodeId,
) -> Option<EvidenceId> {
    if let Some(id) = library_api_dependency_id_for_call(
        il,
        interner,
        call,
        LibraryApiContractId::IteratorIdentityAdapter,
    ) {
        return Some(id);
    }
    if let Some(id) = library_api_dependency_id_for_call(
        il,
        interner,
        call,
        LibraryApiContractId::StaticCollectionAdapter,
    ) {
        return Some(id);
    }
    library_api_dependency_id_for_call_predicate(il, interner, call, |id| {
        matches!(
            id,
            LibraryApiContractId::MethodCall(
                MethodSemanticContract::HoF(_) | MethodSemanticContract::Builtin(Builtin::Zip)
            )
        )
    })
}

fn library_api_dependency_id_for_receiver_domain_call(
    il: &Il,
    interner: &Interner,
    call: NodeId,
    contract: MethodReceiverContract,
) -> Option<EvidenceId> {
    let requirement = method_receiver_domain_requirement(contract)?;
    library_api_dependency_id_for_call_contract(il, interner, call, |id, callee, arity| {
        library_api_contract_result_domain_for_arity(id, callee, arity)
            .is_some_and(|domain| requirement.accepts(domain))
    })
}

fn library_api_dependency_id_for_call(
    il: &Il,
    interner: &Interner,
    call: NodeId,
    id: LibraryApiContractId,
) -> Option<EvidenceId> {
    library_api_dependency_id_for_call_predicate(il, interner, call, |actual| actual == id)
}

pub(crate) fn language_core_builtin_at_call(il: &Il, call: NodeId, builtin: Builtin) -> bool {
    let arity = il.children(call).len();
    match (il.meta.lang, builtin, arity) {
        (Lang::Go, Builtin::Contains, 2) => true,
        (Lang::Go, Builtin::Enumerate, 1) => true,
        (Lang::Python, Builtin::DictEntry, 2) => true,
        (
            Lang::JavaScript | Lang::TypeScript | Lang::Vue | Lang::Svelte | Lang::Html,
            Builtin::Keys,
            1,
        ) => true,
        (Lang::C, Builtin::UnsignedCast32, 1) => {
            source_cast_at_node(il, call) == Some(SourceCastKind::CUnsigned32)
        }
        (_, Builtin::Append, 2) => {
            asserted_effect_at_node(il, call, EffectEvidenceKind::BuilderAppendCall)
        }
        _ => false,
    }
}

/// The asserted same-span `LibraryApi` evidence record that licenses a canonical builtin call.
///
/// Normalization may rewrite a source/library call to `Payload::Builtin`, but the payload is only
/// an operation shape. Producers of downstream evidence can use this helper to preserve the
/// original source/API proof as a dependency instead of treating the canonical payload as proof.
pub fn library_api_dependency_id_for_canonical_builtin_call(
    il: &Il,
    call: NodeId,
    builtin: Builtin,
) -> Option<EvidenceId> {
    if il.kind(call) != NodeKind::Call {
        return None;
    }
    let span = il.node(call).span;
    let mut found = None;
    for record in &il.evidence {
        if !matches!(
            record.anchor,
            EvidenceAnchor::Node {
                span: record_span,
                kind: NodeKind::Call | NodeKind::Field,
            } if record_span == span
        ) {
            continue;
        }
        let EvidenceKind::LibraryApi(LibraryApiEvidenceKind::Contract { contract_hash, .. }) =
            record.kind
        else {
            continue;
        };
        let Some(id) = library_api_contract_id_from_hash(contract_hash) else {
            continue;
        };
        if !library_api_record_models_canonical_builtin(il, record, id, builtin) {
            return None;
        }
        if record.status != EvidenceStatus::Asserted || !il.evidence_dependencies_asserted(record) {
            return None;
        }
        match found {
            None => found = Some(record.id),
            Some(existing) if existing == record.id => {}
            Some(_) => return None,
        }
    }
    found
}

fn library_api_record_models_canonical_builtin(
    il: &Il,
    record: &EvidenceRecord,
    id: LibraryApiContractId,
    builtin: Builtin,
) -> bool {
    if library_api_contract_id_builtin_result(id) == Some(builtin) {
        return true;
    }
    library_api_record_models_rust_map_get_default(il, record, id, builtin)
}

fn library_api_record_models_rust_map_get_default(
    il: &Il,
    record: &EvidenceRecord,
    id: LibraryApiContractId,
    builtin: Builtin,
) -> bool {
    if il.meta.lang != Lang::Rust || builtin != Builtin::GetOrDefault {
        return false;
    }
    let EvidenceKind::LibraryApi(LibraryApiEvidenceKind::Contract {
        callee_hash, arity, ..
    }) = record.kind
    else {
        return false;
    };
    if id
        != LibraryApiContractId::MethodCall(MethodSemanticContract::Builtin(
            Builtin::ValueOrDefault,
        ))
        || arity != 1
    {
        return false;
    }
    let Some(LibraryApiCalleeContract::Method {
        receiver: MethodReceiverContract::RustMapGetOrExactOption,
        ..
    }) = library_api_callee_contract_for_hash(il.meta.lang, id, callee_hash)
    else {
        return false;
    };
    evidence_depends_on_library_api_contract(il, record, LibraryApiContractId::MapGet)
}

fn evidence_depends_on_library_api_contract(
    il: &Il,
    record: &EvidenceRecord,
    expected_id: LibraryApiContractId,
) -> bool {
    record.dependencies.iter().any(|&id| {
        let Some(dependency) = il.evidence_record_by_id(id) else {
            return false;
        };
        if dependency.status != EvidenceStatus::Asserted
            || !il.evidence_dependencies_asserted(dependency)
        {
            return false;
        }
        let EvidenceKind::LibraryApi(LibraryApiEvidenceKind::Contract { contract_hash, .. }) =
            dependency.kind
        else {
            return false;
        };
        library_api_contract_id_from_hash(contract_hash) == Some(expected_id)
    })
}

fn library_api_contract_id_builtin_result(id: LibraryApiContractId) -> Option<Builtin> {
    match id {
        LibraryApiContractId::PropertyBuiltin(builtin)
        | LibraryApiContractId::FreeFunctionBuiltin(builtin) => Some(builtin),
        LibraryApiContractId::MethodCall(MethodSemanticContract::Builtin(builtin)) => Some(builtin),
        LibraryApiContractId::ScalarIntegerMethod(ScalarIntegerMethod::Abs) => Some(Builtin::Abs),
        LibraryApiContractId::ScalarIntegerMethod(ScalarIntegerMethod::Min) => Some(Builtin::Min),
        LibraryApiContractId::ScalarIntegerMethod(ScalarIntegerMethod::Max) => Some(Builtin::Max),
        _ => None,
    }
}

fn library_api_dependency_id_for_map_key_view_call(
    il: &Il,
    interner: &Interner,
    call: NodeId,
    allowed: &[MapKeyViewKind],
) -> Option<EvidenceId> {
    library_api_dependency_id_for_call_predicate(
        il,
        interner,
        call,
        |id| matches!(id, LibraryApiContractId::MapKeyView(kind) if allowed.contains(&kind)),
    )
}

fn library_api_dependency_id_for_call_predicate(
    il: &Il,
    interner: &Interner,
    call: NodeId,
    accepts: impl Fn(LibraryApiContractId) -> bool,
) -> Option<EvidenceId> {
    library_api_dependency_id_for_call_contract(il, interner, call, |id, _, _| accepts(id))
}

fn library_api_dependency_id_for_call_contract(
    il: &Il,
    interner: &Interner,
    call: NodeId,
    accepts: impl Fn(LibraryApiContractId, LibraryApiCalleeContract, u16) -> bool,
) -> Option<EvidenceId> {
    if il.kind(call) != NodeKind::Call {
        return None;
    }
    let anchor = EvidenceAnchor::node(il.node(call).span, NodeKind::Call);
    let mut found = None;
    for record in &il.evidence {
        if record.anchor != anchor
            || record.status != EvidenceStatus::Asserted
            || !il.evidence_dependencies_asserted(record)
        {
            continue;
        }
        let EvidenceKind::LibraryApi(LibraryApiEvidenceKind::Contract {
            contract_hash,
            callee_hash,
            arity,
        }) = record.kind
        else {
            continue;
        };
        let Some(id) = library_api_contract_id_from_hash(contract_hash) else {
            continue;
        };
        let Some(callee) = library_api_callee_contract_for_hash(il.meta.lang, id, callee_hash)
        else {
            continue;
        };
        if !accepts(id, callee, arity) {
            continue;
        }
        if !library_api_record_admitted_for_current_shape(il, interner, call, record) {
            continue;
        }
        match found {
            None => found = Some(record.id),
            Some(existing) if existing == record.id => {}
            Some(_) => return None,
        }
    }
    found
}

fn library_api_contract_result_domain_for_arity(
    id: LibraryApiContractId,
    callee: LibraryApiCalleeContract,
    arity: u16,
) -> Option<DomainEvidence> {
    match id {
        LibraryApiContractId::PythonBuiltinCollectionFactory
        | LibraryApiContractId::PythonImportedCollectionFactory
        | LibraryApiContractId::RustStdCollectionFactory
        | LibraryApiContractId::RustVecMacroFactory
        | LibraryApiContractId::RustVecNewFactory
        | LibraryApiContractId::JavaCollectionFactory(_)
        | LibraryApiContractId::JavaCollectionConstructor(_)
        | LibraryApiContractId::RubySetFactory
        | LibraryApiContractId::JsLikeSetConstructor => {
            library_collection_factory_result_domain_for_arity(
                LibraryCollectionFactoryContract {
                    id,
                    callee,
                    result: LibraryCollectionFactoryResult::SequenceArgument,
                },
                arity as usize,
            )
        }
        LibraryApiContractId::RustStdMapFactory
        | LibraryApiContractId::JavaMapFactory(_)
        | LibraryApiContractId::JsLikeMapConstructor => Some(library_map_factory_result_domain(
            LibraryMapFactoryContract {
                id,
                callee,
                result: LibraryMapFactoryResult::EntrySequence {
                    entry_seq_tag: SEQ_VALUE_COLLECTION,
                },
            },
        )),
        LibraryApiContractId::MapKeyViewWrapper => Some(
            library_map_key_view_wrapper_result_domain(LibraryMapKeyViewWrapperContract {
                id,
                callee,
                result: MapKeyViewWrapperContract {
                    receiver: "Array",
                    method: "from",
                    qualified_path: "Array.from",
                },
            }),
        ),
        LibraryApiContractId::RustOptionSomeConstructor => Some(DomainEvidence::Option),
        LibraryApiContractId::ScalarIntegerMethod(_) => Some(DomainEvidence::Integer),
        LibraryApiContractId::MethodCall(MethodSemanticContract::HoF(_)) => {
            Some(DomainEvidence::Collection)
        }
        _ => None,
    }
}

fn library_api_contract_id_from_hash(hash: u64) -> Option<LibraryApiContractId> {
    library_api_contract_ids()
        .into_iter()
        .find(|id| library_api_contract_id_hash(*id) == hash)
}

fn library_api_contract_ids() -> Vec<LibraryApiContractId> {
    let mut ids = vec![
        LibraryApiContractId::PropertyBuiltin(Builtin::Len),
        LibraryApiContractId::PythonBuiltinCollectionFactory,
        LibraryApiContractId::PythonImportedCollectionFactory,
        LibraryApiContractId::FreeFunctionBuiltin(Builtin::Len),
        LibraryApiContractId::FreeFunctionBuiltin(Builtin::Append),
        LibraryApiContractId::FreeFunctionBuiltin(Builtin::Print),
        LibraryApiContractId::FreeFunctionBuiltin(Builtin::Range),
        LibraryApiContractId::FreeFunctionBuiltin(Builtin::Sum),
        LibraryApiContractId::FreeFunctionBuiltin(Builtin::Min),
        LibraryApiContractId::FreeFunctionBuiltin(Builtin::Max),
        LibraryApiContractId::FreeFunctionBuiltin(Builtin::Abs),
        LibraryApiContractId::FreeFunctionBuiltin(Builtin::Zip),
        LibraryApiContractId::FreeFunctionBuiltin(Builtin::Enumerate),
        LibraryApiContractId::FreeFunctionBuiltin(Builtin::Any),
        LibraryApiContractId::FreeFunctionBuiltin(Builtin::All),
        LibraryApiContractId::RustOptionSomeConstructor,
        LibraryApiContractId::RustOptionNoneSentinel,
        LibraryApiContractId::RustOptionAndThen,
        LibraryApiContractId::RustStdCollectionFactory,
        LibraryApiContractId::RustStdMapFactory,
        LibraryApiContractId::RustVecMacroFactory,
        LibraryApiContractId::RustVecNewFactory,
        LibraryApiContractId::JavaMapEntryFactory,
        LibraryApiContractId::RubySetFactory,
        LibraryApiContractId::JsLikeSetConstructor,
        LibraryApiContractId::JsLikeMapConstructor,
        LibraryApiContractId::MapKeyViewWrapper,
        LibraryApiContractId::MapGet,
        LibraryApiContractId::JsArrayIsArray,
        LibraryApiContractId::JsBooleanCoercion,
        LibraryApiContractId::RegexTest,
        LibraryApiContractId::JsLikeStaticIndexMembership(StaticIndexMembershipKind::IndexOf),
        LibraryApiContractId::JsLikeStaticIndexMembership(StaticIndexMembershipKind::FindIndex),
        LibraryApiContractId::PromiseThen,
        LibraryApiContractId::IteratorIdentityAdapter,
        LibraryApiContractId::StaticCollectionAdapter,
    ];
    ids.extend(
        [
            ScalarIntegerMethod::Abs,
            ScalarIntegerMethod::Min,
            ScalarIntegerMethod::Max,
            ScalarIntegerMethod::Clamp,
        ]
        .into_iter()
        .map(LibraryApiContractId::ScalarIntegerMethod),
    );
    ids.extend(
        [
            JavaCollectionFactoryKind::ListOf,
            JavaCollectionFactoryKind::SetOf,
            JavaCollectionFactoryKind::ArraysAsList,
        ]
        .into_iter()
        .map(LibraryApiContractId::JavaCollectionFactory),
    );
    ids.push(LibraryApiContractId::JavaCollectionConstructor(
        JavaCollectionConstructorKind::EmptyList,
    ));
    ids.extend(
        [JavaMapFactoryKind::Of, JavaMapFactoryKind::OfEntries]
            .into_iter()
            .map(LibraryApiContractId::JavaMapFactory),
    );
    ids.extend(
        [MapKeyViewKind::Collection, MapKeyViewKind::Iterator]
            .into_iter()
            .map(LibraryApiContractId::MapKeyView),
    );
    ids.extend(
        [ImportedNamespaceFunctionSemantic::ProductReduction {
            op: Op::Mul,
            identity: 1,
        }]
        .into_iter()
        .map(LibraryApiContractId::ImportedNamespaceFunction),
    );
    ids.extend(
        [
            MethodSemanticContract::Builtin(Builtin::Append),
            MethodSemanticContract::Builtin(Builtin::Print),
            MethodSemanticContract::Builtin(Builtin::Len),
            MethodSemanticContract::Builtin(Builtin::IsEmpty),
            MethodSemanticContract::Builtin(Builtin::IsNull),
            MethodSemanticContract::Builtin(Builtin::IsNotNull),
            MethodSemanticContract::Builtin(Builtin::StartsWith),
            MethodSemanticContract::Builtin(Builtin::EndsWith),
            MethodSemanticContract::Builtin(Builtin::Contains),
            MethodSemanticContract::Builtin(Builtin::Join),
            MethodSemanticContract::Builtin(Builtin::GetOrDefault),
            MethodSemanticContract::Builtin(Builtin::ValueOrDefault),
            MethodSemanticContract::Builtin(Builtin::Reduce),
            MethodSemanticContract::Builtin(Builtin::Sum),
            MethodSemanticContract::Builtin(Builtin::Abs),
            MethodSemanticContract::Builtin(Builtin::Min),
            MethodSemanticContract::Builtin(Builtin::Max),
            MethodSemanticContract::Builtin(Builtin::Zip),
            MethodSemanticContract::Builtin(Builtin::Any),
            MethodSemanticContract::Builtin(Builtin::All),
            MethodSemanticContract::HoF(HoFKind::Map),
            MethodSemanticContract::HoF(HoFKind::Filter),
            MethodSemanticContract::HoF(HoFKind::FlatMap),
            MethodSemanticContract::HoF(HoFKind::FilterMap),
        ]
        .into_iter()
        .map(LibraryApiContractId::MethodCall),
    );
    ids
}

fn library_api_record_admitted_for_current_shape(
    il: &Il,
    interner: &Interner,
    call: NodeId,
    record: &EvidenceRecord,
) -> bool {
    let EvidenceKind::LibraryApi(LibraryApiEvidenceKind::Contract {
        contract_hash,
        callee_hash,
        arity,
    }) = record.kind
    else {
        return false;
    };
    let Some(id) = library_api_contract_id_from_hash(contract_hash) else {
        return false;
    };
    let Some(callee) = library_api_callee_contract_for_hash(il.meta.lang, id, callee_hash) else {
        return false;
    };
    matches!(
        library_api_contract_evidence_for_call(il, interner, call, id, callee, arity as usize),
        LibraryApiEvidenceStatus::Admitted
    )
}

fn library_api_callee_contract_for_hash(
    lang: Lang,
    id: LibraryApiContractId,
    hash: u64,
) -> Option<LibraryApiCalleeContract> {
    library_api_callee_contracts_for_id(lang, id)
        .into_iter()
        .find(|callee| library_api_callee_contract_hash(*callee) == hash)
}

fn library_api_callee_contracts_for_id(
    lang: Lang,
    id: LibraryApiContractId,
) -> Vec<LibraryApiCalleeContract> {
    match id {
        LibraryApiContractId::PropertyBuiltin(builtin) => ["length"]
            .into_iter()
            .filter_map(|property| library_property_builtin_contract(lang, property))
            .filter(|contract| contract.id == LibraryApiContractId::PropertyBuiltin(builtin))
            .map(|contract| contract.callee)
            .collect(),
        LibraryApiContractId::PythonBuiltinCollectionFactory
        | LibraryApiContractId::RustStdCollectionFactory => {
            library_free_name_collection_factory_contracts(lang)
                .filter(|contract| contract.id == id)
                .map(|contract| contract.callee)
                .collect()
        }
        LibraryApiContractId::PythonImportedCollectionFactory => {
            library_imported_collection_factory_contracts(lang)
                .filter(|contract| contract.id == id)
                .map(|contract| contract.callee)
                .collect()
        }
        LibraryApiContractId::FreeFunctionBuiltin(builtin) => {
            library_free_function_builtin_callee_contracts_for_id(lang, builtin)
        }
        LibraryApiContractId::RustOptionSomeConstructor => [
            "Some",
            "Option::Some",
            "std::option::Option::Some",
            "core::option::Option::Some",
        ]
        .into_iter()
        .filter_map(|name| library_rust_option_some_constructor_contract(lang, name, 1))
        .map(|contract| contract.callee)
        .collect(),
        LibraryApiContractId::RustOptionNoneSentinel => [
            "None",
            "Option::None",
            "std::option::Option::None",
            "core::option::Option::None",
        ]
        .into_iter()
        .filter_map(|name| library_rust_option_none_sentinel_contract(lang, name))
        .map(|contract| contract.callee)
        .collect(),
        LibraryApiContractId::RustOptionAndThen => {
            library_rust_option_and_then_contract(lang, "and_then", 1)
                .map(|contract| vec![contract.callee])
                .unwrap_or_default()
        }
        LibraryApiContractId::ScalarIntegerMethod(method) => ["abs", "min", "max", "clamp"]
            .into_iter()
            .filter_map(|name| library_scalar_integer_method_contract(lang, name, 0))
            .chain(
                ["abs", "min", "max", "clamp"]
                    .into_iter()
                    .filter_map(|name| library_scalar_integer_method_contract(lang, name, 1)),
            )
            .chain(
                ["abs", "min", "max", "clamp"]
                    .into_iter()
                    .filter_map(|name| library_scalar_integer_method_contract(lang, name, 2)),
            )
            .filter(|contract| contract.id == LibraryApiContractId::ScalarIntegerMethod(method))
            .map(|contract| contract.callee)
            .collect(),
        LibraryApiContractId::RustStdMapFactory => library_free_name_map_factory_contracts(lang)
            .filter(|contract| contract.id == id)
            .map(|contract| contract.callee)
            .collect(),
        LibraryApiContractId::RustVecMacroFactory => {
            library_rust_vec_macro_factory_contract(lang, "vec")
                .filter(|contract| contract.id == id)
                .map(|contract| vec![contract.callee])
                .unwrap_or_default()
        }
        LibraryApiContractId::RustVecNewFactory => {
            ["Vec::new", "std::vec::Vec::new", "alloc::vec::Vec::new"]
                .into_iter()
                .filter_map(|name| library_rust_vec_new_factory_contract(lang, name))
                .filter(|contract| contract.id == id)
                .map(|contract| contract.callee)
                .collect()
        }
        LibraryApiContractId::JavaCollectionFactory(kind) => {
            [("List", "of"), ("Set", "of"), ("Arrays", "asList")]
                .into_iter()
                .filter_map(|(receiver, method)| {
                    library_java_collection_factory_contract(lang, receiver, method)
                })
                .filter(|contract| contract.id == LibraryApiContractId::JavaCollectionFactory(kind))
                .map(|contract| contract.callee)
                .collect()
        }
        LibraryApiContractId::JavaCollectionConstructor(kind) => [
            "ArrayList",
            "java.util.ArrayList",
            "LinkedList",
            "java.util.LinkedList",
        ]
        .into_iter()
        .filter_map(|type_name| library_java_collection_constructor_contract(lang, type_name, 0))
        .filter(|contract| contract.id == LibraryApiContractId::JavaCollectionConstructor(kind))
        .map(|contract| contract.callee)
        .collect(),
        LibraryApiContractId::JavaMapFactory(kind) => ["of", "ofEntries"]
            .into_iter()
            .filter_map(|method| library_java_map_factory_contract(lang, "Map", method))
            .filter(|contract| contract.id == LibraryApiContractId::JavaMapFactory(kind))
            .map(|contract| contract.callee)
            .collect(),
        LibraryApiContractId::JavaMapEntryFactory => {
            library_java_map_entry_contract(lang, "Map", "entry")
                .map(|contract| vec![contract.callee])
                .unwrap_or_default()
        }
        LibraryApiContractId::RubySetFactory => {
            library_ruby_set_factory_contract(lang, "Set", "new", 1)
                .map(|contract| vec![contract.callee])
                .unwrap_or_default()
        }
        LibraryApiContractId::JsLikeSetConstructor => {
            library_js_like_set_constructor_contract(lang, "Set")
                .map(|contract| vec![contract.callee])
                .unwrap_or_default()
        }
        LibraryApiContractId::JsLikeMapConstructor => {
            library_js_like_map_constructor_contract(lang, "Map")
                .map(|contract| vec![contract.callee])
                .unwrap_or_default()
        }
        LibraryApiContractId::MapKeyViewWrapper => {
            library_map_key_view_wrapper_contract(lang, "Array", "from", 1)
                .map(|contract| vec![contract.callee])
                .unwrap_or_default()
        }
        LibraryApiContractId::JsLikeStaticIndexMembership(kind) => ["indexOf", "findIndex"]
            .into_iter()
            .filter_map(|method| library_static_index_membership_contract(lang, method, 1))
            .filter(|contract| {
                contract.id == LibraryApiContractId::JsLikeStaticIndexMembership(kind)
            })
            .map(|contract| contract.callee)
            .collect(),
        LibraryApiContractId::MapGet => ["get"]
            .into_iter()
            .filter_map(|method| {
                library_map_get_contract(lang, method, 1).map(|contract| contract.callee)
            })
            .collect(),
        LibraryApiContractId::MapKeyView(kind) => ["keys", "keySet"]
            .into_iter()
            .filter_map(|method| library_map_key_view_contract(lang, method, 0))
            .filter(|contract| contract.result.kind == kind)
            .map(|contract| contract.callee)
            .collect(),
        LibraryApiContractId::IteratorIdentityAdapter => {
            let methods = [
                "iter",
                "into_iter",
                "iter_mut",
                "collect",
                "to_vec",
                "copied",
                "cloned",
                "stream",
            ];
            methods
                .into_iter()
                .filter_map(|method| {
                    library_iterator_identity_adapter_contract(lang, method, 0)
                        .map(|contract| contract.callee)
                })
                .collect()
        }
        LibraryApiContractId::StaticCollectionAdapter => {
            library_static_collection_adapter_contract(lang, "Arrays", "stream", 1)
                .map(|contract| vec![contract.callee])
                .unwrap_or_default()
        }
        LibraryApiContractId::MethodCall(semantic) => {
            method_call_contract_callees_for_semantic(lang, semantic)
        }
        _ => Vec::new(),
    }
}

fn library_free_function_builtin_callee_contracts_for_id(
    lang: Lang,
    builtin: Builtin,
) -> Vec<LibraryApiCalleeContract> {
    let candidate = match (lang, builtin) {
        (Lang::Python, Builtin::Len) => Some(("len", 1)),
        (Lang::Go, Builtin::Len) => Some(("len", 1)),
        (Lang::Go, Builtin::Append) => Some(("append", 2)),
        (Lang::Python, Builtin::Print) => Some(("print", 0)),
        (Lang::Python, Builtin::Range) => Some(("range", 1)),
        (Lang::Python, Builtin::Sum) => Some(("sum", 1)),
        (Lang::Python, Builtin::Min) => Some(("min", 1)),
        (Lang::Python, Builtin::Max) => Some(("max", 1)),
        (Lang::Python, Builtin::Abs) => Some(("abs", 1)),
        (Lang::Python, Builtin::Zip) => Some(("zip", 2)),
        (Lang::Python, Builtin::Enumerate) => Some(("enumerate", 1)),
        (Lang::Python, Builtin::Any) => Some(("any", 1)),
        (Lang::Python, Builtin::All) => Some(("all", 1)),
        _ => None,
    };
    candidate
        .and_then(|(name, arg_count)| library_free_function_builtin_contract(lang, name, arg_count))
        .map(|contract| vec![contract.callee])
        .unwrap_or_default()
}

fn method_call_contract_callees_for_semantic(
    lang: Lang,
    semantic: MethodSemanticContract,
) -> Vec<LibraryApiCalleeContract> {
    let methods = [
        "append",
        "push",
        "log",
        "info",
        "debug",
        "Println",
        "Printf",
        "Print",
        "Abs",
        "HasPrefix",
        "HasSuffix",
        "Contains",
        "len",
        "size",
        "length",
        "is_empty",
        "isEmpty",
        "empty?",
        "nil?",
        "is_none",
        "is_some",
        "startsWith",
        "startswith",
        "starts_with",
        "start_with?",
        "endsWith",
        "endswith",
        "ends_with",
        "end_with?",
        "containsKey",
        "contains_key",
        "key?",
        "has_key?",
        "__contains__",
        "includes",
        "include?",
        "member?",
        "contains",
        "has",
        "join",
        "get",
        "fetch",
        "getOrDefault",
        "unwrap_or",
        "unwrap_or_else",
        "map_or",
        "reduce",
        "Min",
        "Max",
        "abs",
        "min",
        "max",
        "zip",
        "fold",
        "inject",
        "map",
        "collect",
        "filter",
        "select",
        "flatMap",
        "flat_map",
        "filter_map",
        "some",
        "every",
        "all",
        "any",
        "all?",
        "any?",
        "allMatch",
        "anyMatch",
        "sum",
        "count",
    ];
    methods
        .into_iter()
        .flat_map(|method| {
            (0..=3).filter_map(move |arity| library_method_call_contract(lang, method, arity))
        })
        .filter(|contract| contract.result.semantic == semantic)
        .map(|contract| contract.callee)
        .collect()
}

fn dependency_ids_are_present(record: &EvidenceRecord, dependencies: &[EvidenceId]) -> bool {
    dependencies
        .iter()
        .all(|dependency| record.dependencies.contains(dependency))
}

fn var_name_matches(il: &Il, interner: &Interner, node: NodeId, expected: &str) -> bool {
    matches!(
        (il.kind(node), il.node(node).payload),
        (NodeKind::Var, Payload::Name(name)) if interner.resolve(name) == expected
    )
}

fn static_member_callee_parts<'a>(
    il: &Il,
    interner: &'a Interner,
    node: NodeId,
) -> Option<(&'a str, &'a str)> {
    if il.kind(node) != NodeKind::Field {
        return None;
    }
    let Payload::Name(method) = il.node(node).payload else {
        return None;
    };
    let receiver = il.children(node).first().copied()?;
    if il.kind(receiver) != NodeKind::Var {
        return None;
    }
    let receiver_name = node_name(il, interner, receiver)?;
    Some((receiver_name, interner.resolve(method)))
}

fn imported_member_callee_shape_matches(
    il: &Il,
    interner: &Interner,
    node: NodeId,
    exported: &str,
) -> bool {
    match il.kind(node) {
        // Aliased imports are proven by the imported-binding dependency, not by
        // comparing the local callee spelling to the exported API name.
        NodeKind::Var => true,
        NodeKind::Field => field_method_matches(il, interner, node, exported),
        _ => false,
    }
}

fn field_method_matches(il: &Il, interner: &Interner, node: NodeId, expected: &str) -> bool {
    matches!(
        (il.kind(node), il.node(node).payload),
        (NodeKind::Field, Payload::Name(method)) if interner.resolve(method) == expected
    )
}

fn dependency_has_source_call(
    il: &Il,
    record: &EvidenceRecord,
    span: Span,
    expected: SourceCallKind,
) -> bool {
    let anchor = EvidenceAnchor::source_span(span);
    let kind = EvidenceKind::Source(SourceFactKind::Call(expected));
    matches!(
        unique_evidence_at(
            il,
            |candidate| candidate == anchor,
            |evidence| match evidence {
                EvidenceKind::Source(SourceFactKind::Call(call)) => Some(call),
                _ => None,
            },
        ),
        EvidenceResolution::Found(call) if call == expected
    ) && dependency_has_asserted_record(il, record, anchor, kind)
}

fn dependency_has_source_fact_node(
    il: &Il,
    record: &EvidenceRecord,
    node: NodeId,
    expected: SourceFactKind,
) -> bool {
    dependency_has_source_fact_anchor(il, record, il.node(node).span, expected)
}

fn dependency_has_source_fact_anchor(
    il: &Il,
    record: &EvidenceRecord,
    span: Span,
    expected: SourceFactKind,
) -> bool {
    let anchor = EvidenceAnchor::source_span(span);
    matches!(
        unique_evidence_at(
            il,
            |candidate| candidate == anchor,
            |evidence| match evidence {
                EvidenceKind::Source(fact) => Some(fact),
                _ => None,
            },
        ),
        EvidenceResolution::Found(fact) if fact == expected
    ) && dependency_has_asserted_record(il, record, anchor, EvidenceKind::Source(expected))
}

fn dependency_has_required_module_before(
    record: &EvidenceRecord,
    il: &Il,
    interner: &Interner,
    module: &str,
    call_span: Span,
) -> bool {
    let expected = EvidenceKind::Import(ImportEvidenceKind::Require {
        module_hash: stable_symbol_hash(module),
    });
    record.dependencies.iter().any(|id| {
        il.evidence.get(id.0 as usize).is_some_and(|dependency| {
            dependency.id == *id
                && dependency.status == EvidenceStatus::Asserted
                && dependency.kind == expected
                && require_dependency_is_before_call(dependency, call_span)
                && require_dependency_has_unshadowed_require(il, interner, dependency)
        })
    })
}

fn require_dependency_is_before_call(require_record: &EvidenceRecord, call_span: Span) -> bool {
    matches!(
        require_record.anchor,
        EvidenceAnchor::SourceSpan(span)
            if span.file == call_span.file && span.end_byte <= call_span.start_byte
    )
}

fn require_dependency_has_unshadowed_require(
    il: &Il,
    interner: &Interner,
    require_record: &EvidenceRecord,
) -> bool {
    let require_span = match require_record.anchor {
        EvidenceAnchor::SourceSpan(span) => span,
        _ => return false,
    };
    require_record.dependencies.iter().any(|id| {
        let Some(dependency) = il.evidence.get(id.0 as usize) else {
            return false;
        };
        let expected = SymbolEvidenceKind::UnshadowedGlobal {
            name_hash: stable_symbol_hash("require"),
        };
        let EvidenceAnchor::Node {
            span,
            kind: NodeKind::Var,
        } = dependency.anchor
        else {
            return false;
        };
        dependency.id == *id
            && dependency.status == EvidenceStatus::Asserted
            && dependency.kind == EvidenceKind::Symbol(expected)
            && span.file == require_span.file
            && span.start_byte >= require_span.start_byte
            && span.end_byte <= require_span.end_byte
            && !file_defines_name_visible_at(il, interner, "require", span)
            && matches!(
                symbol_evidence_at_node_anchor(il, span, NodeKind::Var),
                EvidenceResolution::Found(actual) if actual == expected
            )
    })
}

fn dependency_has_unshadowed_global_node(
    il: &Il,
    record: &EvidenceRecord,
    node: NodeId,
    expected: &str,
) -> bool {
    let span = il.node(node).span;
    let kind = il.kind(node);
    dependency_has_unshadowed_global_anchor(il, record, span, kind, expected)
}

fn dependency_has_unshadowed_global_anchor(
    il: &Il,
    record: &EvidenceRecord,
    span: Span,
    kind: NodeKind,
    expected: &str,
) -> bool {
    let expected_kind = SymbolEvidenceKind::UnshadowedGlobal {
        name_hash: stable_symbol_hash(expected),
    };
    if !matches!(
        symbol_evidence_at_node_anchor(il, span, kind),
        EvidenceResolution::Found(actual) if actual == expected_kind
    ) {
        return false;
    }
    dependency_has_asserted_record(
        il,
        record,
        EvidenceAnchor::node(span, kind),
        EvidenceKind::Symbol(expected_kind),
    )
}

fn dependency_has_qualified_global_node(
    il: &Il,
    record: &EvidenceRecord,
    node: NodeId,
    expected: &str,
) -> bool {
    let span = il.node(node).span;
    let kind = il.kind(node);
    dependency_has_qualified_global_anchor(il, record, span, kind, expected)
}

fn dependency_has_qualified_global_anchor(
    il: &Il,
    record: &EvidenceRecord,
    span: Span,
    kind: NodeKind,
    expected: &str,
) -> bool {
    let Some(contract) = qualified_global_symbol_contract(il.meta.lang, expected) else {
        return false;
    };
    let anchor = EvidenceAnchor::node(span, kind);
    if !matches!(
        qualified_global_symbol_at_evidence_anchor(il, anchor, contract),
        EvidenceResolution::Found(())
    ) {
        return false;
    }
    record.dependencies.iter().any(|&id| {
        il.evidence_record_by_id(id).is_some_and(|dependency| {
            dependency.anchor == anchor
                && qualified_global_symbol_record_valid(il, dependency, contract)
        })
    })
}

fn dependency_has_imported_member_node(
    il: &Il,
    interner: &Interner,
    record: &EvidenceRecord,
    node: NodeId,
    module: &str,
    exported: &str,
) -> bool {
    match il.kind(node) {
        NodeKind::Var => {
            dependency_has_imported_binding_node(il, interner, record, node, module, exported)
        }
        NodeKind::Field => {
            let Some(receiver) = il.children(node).first().copied() else {
                return false;
            };
            dependency_has_imported_namespace_node(il, interner, record, receiver, module)
        }
        _ => false,
    }
}

fn dependency_has_imported_binding_node(
    il: &Il,
    interner: &Interner,
    record: &EvidenceRecord,
    node: NodeId,
    module: &str,
    exported: &str,
) -> bool {
    dependency_has_imported_binding_anchor(
        il,
        interner,
        record,
        il.node(node).span,
        il.kind(node),
        module,
        exported,
    )
}

fn dependency_has_imported_binding_anchor(
    il: &Il,
    interner: &Interner,
    record: &EvidenceRecord,
    span: Span,
    kind: NodeKind,
    module: &str,
    exported: &str,
) -> bool {
    let expected = SymbolEvidenceKind::ImportedBinding {
        module_hash: stable_symbol_hash(module),
        exported_hash: stable_symbol_hash(exported),
    };
    dependency_has_imported_symbol_anchor(il, interner, record, span, kind, expected)
}

fn dependency_has_imported_namespace_node(
    il: &Il,
    interner: &Interner,
    record: &EvidenceRecord,
    node: NodeId,
    module: &str,
) -> bool {
    dependency_has_imported_namespace_anchor(
        il,
        interner,
        record,
        il.node(node).span,
        il.kind(node),
        module,
    )
}

fn dependency_has_imported_namespace_anchor(
    il: &Il,
    interner: &Interner,
    record: &EvidenceRecord,
    span: Span,
    kind: NodeKind,
    module: &str,
) -> bool {
    let expected = SymbolEvidenceKind::ImportedNamespace {
        module_hash: stable_symbol_hash(module),
    };
    dependency_has_imported_symbol_anchor(il, interner, record, span, kind, expected)
}

fn dependency_has_imported_binding_dependency(
    il: &Il,
    interner: &Interner,
    record: &EvidenceRecord,
    module: &str,
    exported: &str,
) -> bool {
    let expected = SymbolEvidenceKind::ImportedBinding {
        module_hash: stable_symbol_hash(module),
        exported_hash: stable_symbol_hash(exported),
    };
    dependency_has_imported_symbol_dependency(il, interner, record, expected)
}

fn dependency_has_imported_namespace_dependency(
    il: &Il,
    interner: &Interner,
    record: &EvidenceRecord,
    module: &str,
) -> bool {
    let expected = SymbolEvidenceKind::ImportedNamespace {
        module_hash: stable_symbol_hash(module),
    };
    dependency_has_imported_symbol_dependency(il, interner, record, expected)
}

fn dependency_has_imported_symbol_dependency(
    il: &Il,
    interner: &Interner,
    record: &EvidenceRecord,
    expected: SymbolEvidenceKind,
) -> bool {
    record.dependencies.iter().any(|&id| {
        let Some(dependency) = il.evidence_record_by_id(id) else {
            return false;
        };
        dependency.status == EvidenceStatus::Asserted
            && dependency.kind == EvidenceKind::Symbol(expected)
            && matches!(
                dependency.anchor,
                EvidenceAnchor::Node {
                    kind: NodeKind::Var,
                    ..
                }
            )
            && imported_occurrence_symbol_dependencies_valid(il, interner, dependency, expected)
    })
}

fn dependency_has_imported_symbol_anchor(
    il: &Il,
    interner: &Interner,
    record: &EvidenceRecord,
    span: Span,
    kind: NodeKind,
    expected: SymbolEvidenceKind,
) -> bool {
    if kind != NodeKind::Var {
        return false;
    }
    if !matches!(
        symbol_evidence_at_node_anchor(il, span, kind),
        EvidenceResolution::Found(actual) if actual == expected
    ) {
        return false;
    }
    let Some(symbol_record) = record.dependencies.iter().find_map(|&id| {
        let dependency = il.evidence_record_by_id(id)?;
        (dependency.anchor == EvidenceAnchor::node(span, kind)
            && dependency.status == EvidenceStatus::Asserted
            && dependency.kind == EvidenceKind::Symbol(expected))
        .then_some(dependency)
    }) else {
        return false;
    };
    imported_occurrence_symbol_dependencies_valid(il, interner, symbol_record, expected)
}

pub(crate) fn imported_occurrence_symbol_dependencies_valid(
    il: &Il,
    interner: &Interner,
    symbol_record: &EvidenceRecord,
    expected: SymbolEvidenceKind,
) -> bool {
    let EvidenceAnchor::Node {
        span: occurrence_span,
        kind: NodeKind::Var,
    } = symbol_record.anchor
    else {
        return false;
    };
    let Some(binding_record) = symbol_record.dependencies.iter().find_map(|&id| {
        let dependency = il.evidence_record_by_id(id)?;
        (dependency.status == EvidenceStatus::Asserted
            && dependency.kind == EvidenceKind::Symbol(expected)
            && matches!(dependency.anchor, EvidenceAnchor::Binding { .. }))
        .then_some(dependency)
    }) else {
        return false;
    };
    let EvidenceAnchor::Binding {
        span: binding_span,
        local_hash,
    } = binding_record.anchor
    else {
        return false;
    };
    if unit_defines_hash_visible_at(il, interner, local_hash, occurrence_span) {
        return false;
    }
    if !matches!(
        binding_identity_matches(il, local_hash, binding_span, expected),
        EvidenceResolution::Found(true)
    ) {
        return false;
    }
    if !binding_has_no_visible_conflicting_assignment(il, interner, local_hash, binding_span) {
        return false;
    }
    if !binding_has_no_visible_local_shadow(il, interner, local_hash, binding_span, occurrence_span)
    {
        return false;
    }
    binding_symbol_evidence_consistent_for_local(il, local_hash, expected)
}

fn binding_has_no_visible_conflicting_assignment(
    il: &Il,
    interner: &Interner,
    local_hash: u64,
    binding_span: Span,
) -> bool {
    top_level_statements(il)
        .into_iter()
        .filter(|&stmt| assignment_alias_hash(il, interner, stmt) == Some(local_hash))
        .all(|stmt| il.node(stmt).span == binding_span)
}

fn binding_has_no_visible_local_shadow(
    il: &Il,
    interner: &Interner,
    local_hash: u64,
    binding_span: Span,
    occurrence_span: Span,
) -> bool {
    let Some(function_span) = innermost_enclosing_function_span(il, occurrence_span) else {
        return true;
    };
    let occurrence_cid = var_cid_at_span(il, occurrence_span);
    !il.nodes.iter().enumerate().any(|(idx, node)| {
        let node_id = NodeId(idx as u32);
        if !span_contains(function_span, node.span)
            || node.span == binding_span
            || node.span.start_byte > occurrence_span.start_byte
            || innermost_enclosing_function_span(il, node.span) != Some(function_span)
        {
            return false;
        }
        match node.kind {
            NodeKind::Param => node_cid(il, node_id)
                .zip(occurrence_cid)
                .is_some_and(|(param_cid, occurrence_cid)| param_cid == occurrence_cid),
            NodeKind::Assign => {
                assignment_lhs_cid(il, node_id)
                    .zip(occurrence_cid)
                    .is_some_and(|(lhs_cid, occurrence_cid)| lhs_cid == occurrence_cid)
                    || assignment_lhs_raw_name_hash(il, interner, node_id) == Some(local_hash)
            }
            _ => false,
        }
    })
}

fn innermost_enclosing_function_span(il: &Il, span: Span) -> Option<Span> {
    il.nodes
        .iter()
        .filter_map(|node| {
            (node.kind == NodeKind::Func && span_contains(node.span, span)).then_some(node.span)
        })
        .min_by_key(|span| span.end_byte.saturating_sub(span.start_byte))
}

fn var_cid_at_span(il: &Il, span: Span) -> Option<u32> {
    il.nodes
        .iter()
        .enumerate()
        .find_map(|(idx, node)| {
            (node.kind == NodeKind::Var && node.span == span).then_some(NodeId(idx as u32))
        })
        .and_then(|node| node_cid(il, node))
}

fn node_cid(il: &Il, node: NodeId) -> Option<u32> {
    match il.node(node).payload {
        Payload::Cid(cid) => Some(cid),
        _ => None,
    }
}

fn assignment_lhs_cid(il: &Il, stmt: NodeId) -> Option<u32> {
    let (lhs, _) = assignment_parts(il, stmt)?;
    (il.kind(lhs) == NodeKind::Var)
        .then(|| node_cid(il, lhs))
        .flatten()
}

fn assignment_lhs_raw_name_hash(il: &Il, interner: &Interner, stmt: NodeId) -> Option<u64> {
    let (lhs, _) = assignment_parts(il, stmt)?;
    match il.node(lhs).payload {
        Payload::Name(symbol) => Some(stable_symbol_hash(interner.resolve(symbol))),
        _ => None,
    }
}

fn binding_symbol_evidence_consistent_for_local(
    il: &Il,
    local_hash: u64,
    expected: SymbolEvidenceKind,
) -> bool {
    let mut saw_symbol = false;
    for record in &il.evidence {
        let EvidenceAnchor::Binding {
            local_hash: anchor_hash,
            ..
        } = record.anchor
        else {
            continue;
        };
        if anchor_hash != local_hash {
            continue;
        }
        let EvidenceKind::Symbol(symbol) = record.kind else {
            continue;
        };
        if record.status != EvidenceStatus::Asserted || symbol != expected {
            return false;
        }
        saw_symbol = true;
    }
    saw_symbol
}

fn dependency_has_asserted_record(
    il: &Il,
    record: &EvidenceRecord,
    anchor: EvidenceAnchor,
    kind: EvidenceKind,
) -> bool {
    record.dependencies.iter().any(|&id| {
        il.evidence_record_by_id(id).is_some_and(|dependency| {
            dependency.anchor == anchor
                && dependency.status == EvidenceStatus::Asserted
                && dependency.kind == kind
        })
    })
}

pub fn library_free_name_collection_factory_contract(
    lang: Lang,
    name: &str,
) -> Option<LibraryCollectionFactoryContract> {
    FREE_NAME_COLLECTION_FACTORIES
        .iter()
        .find(|row| row.lang.is_none_or(|row_lang| row_lang == lang) && row.names.contains(&name))
        .and_then(|row| {
            let matched_name = row
                .names
                .iter()
                .copied()
                .find(|candidate| *candidate == name)?;
            let id = match lang {
                Lang::Python => LibraryApiContractId::PythonBuiltinCollectionFactory,
                Lang::Rust => LibraryApiContractId::RustStdCollectionFactory,
                _ => return None,
            };
            Some(LibraryCollectionFactoryContract {
                id,
                callee: LibraryApiCalleeContract::FreeName {
                    name: matched_name,
                    shadow: library_free_name_shadow_policy(lang, row.shadow_guard),
                },
                result: LibraryCollectionFactoryResult::SequenceArgument,
            })
        })
}

pub fn library_free_name_collection_factory_contracts(
    lang: Lang,
) -> impl Iterator<Item = LibraryCollectionFactoryContract> {
    FREE_NAME_COLLECTION_FACTORIES
        .iter()
        .filter(move |row| row.lang.is_none_or(|row_lang| row_lang == lang))
        .flat_map(move |row| {
            row.names
                .iter()
                .filter_map(move |name| library_free_name_collection_factory_contract(lang, name))
        })
}

pub fn library_free_function_builtin_contract(
    lang: Lang,
    name: &str,
    arg_count: usize,
) -> Option<LibraryFreeFunctionBuiltinContract> {
    let result = free_function_builtin_contract(lang, name, arg_count)?;
    Some(LibraryFreeFunctionBuiltinContract {
        id: LibraryApiContractId::FreeFunctionBuiltin(result.builtin),
        callee: LibraryApiCalleeContract::FreeName {
            name: result.name,
            shadow: library_free_name_shadow_policy(lang, result.requires_unshadowed),
        },
        result,
    })
}

pub fn library_imported_collection_factory_contract(
    lang: Lang,
    module: &str,
    exported: &str,
) -> Option<LibraryCollectionFactoryContract> {
    IMPORTED_COLLECTION_FACTORIES
        .iter()
        .find(|row| {
            row.lang.is_none_or(|row_lang| row_lang == lang)
                && row.module == module
                && row.exported == exported
        })
        .map(|row| LibraryCollectionFactoryContract {
            id: LibraryApiContractId::PythonImportedCollectionFactory,
            callee: LibraryApiCalleeContract::ImportedBinding {
                module: row.module,
                exported: row.exported,
            },
            result: LibraryCollectionFactoryResult::SequenceArgument,
        })
}

pub fn library_imported_collection_factory_contracts(
    lang: Lang,
) -> impl Iterator<Item = LibraryCollectionFactoryContract> {
    IMPORTED_COLLECTION_FACTORIES
        .iter()
        .filter(move |row| row.lang.is_none_or(|row_lang| row_lang == lang))
        .filter_map(move |row| {
            library_imported_collection_factory_contract(lang, row.module, row.exported)
        })
}

pub fn library_free_name_map_factory_contract(
    lang: Lang,
    name: &str,
) -> Option<LibraryMapFactoryContract> {
    FREE_NAME_MAP_FACTORIES
        .iter()
        .find(|row| row.lang.is_none_or(|row_lang| row_lang == lang) && row.names.contains(&name))
        .and_then(|row| {
            let matched_name = row
                .names
                .iter()
                .copied()
                .find(|candidate| *candidate == name)?;
            let id = match lang {
                Lang::Rust => LibraryApiContractId::RustStdMapFactory,
                _ => return None,
            };
            Some(LibraryMapFactoryContract {
                id,
                callee: LibraryApiCalleeContract::FreeName {
                    name: matched_name,
                    shadow: library_free_name_shadow_policy(lang, false),
                },
                result: LibraryMapFactoryResult::EntrySequence {
                    entry_seq_tag: row.entry_seq_tag,
                },
            })
        })
}

pub fn library_free_name_map_factory_contracts(
    lang: Lang,
) -> impl Iterator<Item = LibraryMapFactoryContract> {
    FREE_NAME_MAP_FACTORIES
        .iter()
        .filter(move |row| row.lang.is_none_or(|row_lang| row_lang == lang))
        .flat_map(move |row| {
            row.names
                .iter()
                .filter_map(move |name| library_free_name_map_factory_contract(lang, name))
        })
}

pub fn library_java_collection_factory_contract(
    lang: Lang,
    receiver: &str,
    method: &str,
) -> Option<LibraryCollectionFactoryContract> {
    let contract = java_collection_factory_contract(lang, receiver, method)?;
    Some(LibraryCollectionFactoryContract {
        id: LibraryApiContractId::JavaCollectionFactory(contract.kind),
        callee: LibraryApiCalleeContract::JavaUtilStaticMember {
            receiver: contract.receiver,
            method: contract.method,
        },
        result: LibraryCollectionFactoryResult::VariadicElements {
            single_arg_spreads_array: contract.single_arg_spreads_array,
        },
    })
}

pub fn library_java_collection_factory_contract_by_hash(
    lang: Lang,
    receiver: &str,
    method_hash: u64,
) -> Option<LibraryCollectionFactoryContract> {
    ["of", "asList"].into_iter().find_map(|method| {
        (stable_symbol_hash(method) == method_hash)
            .then(|| library_java_collection_factory_contract(lang, receiver, method))
            .flatten()
    })
}

pub fn library_java_collection_constructor_contract(
    lang: Lang,
    type_name: &str,
    arg_count: usize,
) -> Option<LibraryCollectionFactoryContract> {
    let contract = java_collection_constructor_contract(lang, type_name, arg_count)?;
    Some(LibraryCollectionFactoryContract {
        id: LibraryApiContractId::JavaCollectionConstructor(contract.kind),
        callee: LibraryApiCalleeContract::JavaUtilConstructor {
            simple_type: contract.simple_type,
            qualified_type: contract.qualified_type,
            module: contract.module,
            requires_import_for_simple_type: contract.requires_import_for_simple_type,
            requires_no_local_type_shadow: contract.requires_no_local_type_shadow,
        },
        result: LibraryCollectionFactoryResult::EmptySequence,
    })
}

pub fn library_java_map_factory_contract(
    lang: Lang,
    receiver: &str,
    method: &str,
) -> Option<LibraryMapFactoryContract> {
    let contract = java_map_factory_contract(lang, receiver, method)?;
    Some(LibraryMapFactoryContract {
        id: LibraryApiContractId::JavaMapFactory(contract.kind),
        callee: LibraryApiCalleeContract::JavaUtilStaticMember {
            receiver: contract.receiver,
            method: contract.method,
        },
        result: LibraryMapFactoryResult::JavaFactory {
            kind: contract.kind,
        },
    })
}

pub fn library_java_map_factory_contract_by_hash(
    lang: Lang,
    receiver: &str,
    method_hash: u64,
) -> Option<LibraryMapFactoryContract> {
    ["of", "ofEntries"].into_iter().find_map(|method| {
        (stable_symbol_hash(method) == method_hash)
            .then(|| library_java_map_factory_contract(lang, receiver, method))
            .flatten()
    })
}

pub fn library_java_map_entry_contract(
    lang: Lang,
    receiver: &str,
    method: &str,
) -> Option<LibraryMapEntryFactoryContract> {
    java_map_entry_contract(lang, receiver, method).then_some(LibraryMapEntryFactoryContract {
        id: LibraryApiContractId::JavaMapEntryFactory,
        callee: LibraryApiCalleeContract::JavaUtilStaticMember {
            receiver: "Map",
            method: "entry",
        },
    })
}

pub fn library_java_map_entry_contract_by_hash(
    lang: Lang,
    receiver: &str,
    method_hash: u64,
) -> Option<LibraryMapEntryFactoryContract> {
    (method_hash == stable_symbol_hash("entry"))
        .then(|| library_java_map_entry_contract(lang, receiver, "entry"))
        .flatten()
}

pub fn library_ruby_set_factory_contract(
    lang: Lang,
    receiver: &str,
    method: &str,
    arg_count: usize,
) -> Option<LibraryCollectionFactoryContract> {
    let contract = ruby_set_factory_contract(lang, receiver, method, arg_count)?;
    Some(LibraryCollectionFactoryContract {
        id: LibraryApiContractId::RubySetFactory,
        callee: LibraryApiCalleeContract::RubyRequireStaticMember {
            receiver: contract.receiver,
            method: contract.method,
            required_module: contract.required_module,
            shadow_root: contract.shadow_root,
        },
        result: LibraryCollectionFactoryResult::SequenceArgument,
    })
}

pub fn library_ruby_set_factory_contract_by_hash(
    lang: Lang,
    receiver: &str,
    method_hash: u64,
    arg_count: usize,
) -> Option<LibraryCollectionFactoryContract> {
    (method_hash == stable_symbol_hash("new"))
        .then(|| library_ruby_set_factory_contract(lang, receiver, "new", arg_count))
        .flatten()
}

pub fn library_js_like_set_constructor_contract(
    lang: Lang,
    receiver: &str,
) -> Option<LibraryCollectionFactoryContract> {
    let contract = js_like_set_constructor_contract(lang, receiver)?;
    Some(LibraryCollectionFactoryContract {
        id: LibraryApiContractId::JsLikeSetConstructor,
        callee: LibraryApiCalleeContract::JsGlobalConstructor {
            receiver: contract.receiver,
            requires_unshadowed_global: contract.requires_unshadowed_global,
        },
        result: LibraryCollectionFactoryResult::StaticNonFloatSequenceArgument,
    })
}

pub fn library_js_like_map_constructor_contract(
    lang: Lang,
    receiver: &str,
) -> Option<LibraryMapFactoryContract> {
    let contract = js_like_map_constructor_contract(lang, receiver)?;
    Some(LibraryMapFactoryContract {
        id: LibraryApiContractId::JsLikeMapConstructor,
        callee: LibraryApiCalleeContract::JsGlobalConstructor {
            receiver: contract.receiver,
            requires_unshadowed_global: contract.requires_unshadowed_global,
        },
        result: LibraryMapFactoryResult::EntrySequence {
            entry_seq_tag: contract.entry_seq_tag?,
        },
    })
}

pub fn library_rust_vec_macro_factory_contract(
    lang: Lang,
    name: &str,
) -> Option<LibraryCollectionFactoryContract> {
    (lang == Lang::Rust && name == "vec").then_some(LibraryCollectionFactoryContract {
        id: LibraryApiContractId::RustVecMacroFactory,
        callee: LibraryApiCalleeContract::RustMacro {
            name: "vec",
            shadow: LibraryApiShadowPolicy::SameName,
        },
        result: LibraryCollectionFactoryResult::VariadicElements {
            single_arg_spreads_array: false,
        },
    })
}

pub fn library_rust_vec_new_factory_contract(
    lang: Lang,
    name: &str,
) -> Option<LibraryCollectionFactoryContract> {
    let contract = rust_vec_new_factory_contract(lang, name)?;
    Some(LibraryCollectionFactoryContract {
        id: LibraryApiContractId::RustVecNewFactory,
        callee: LibraryApiCalleeContract::FreeName {
            name: match name {
                "Vec::new" => "Vec::new",
                "std::vec::Vec::new" => "std::vec::Vec::new",
                "alloc::vec::Vec::new" => "alloc::vec::Vec::new",
                _ => return None,
            },
            shadow: LibraryApiShadowPolicy::ExplicitRoot(contract.shadow_root),
        },
        result: LibraryCollectionFactoryResult::EmptySequence,
    })
}

pub fn library_map_key_view_contract(
    lang: Lang,
    method: &str,
    arg_count: usize,
) -> Option<LibraryMapKeyViewContract> {
    if arg_count != 0 {
        return None;
    }
    let result = match (lang, method) {
        (Lang::Python | Lang::Ruby, "keys") => MapKeyViewContract {
            method: "keys",
            kind: MapKeyViewKind::Collection,
        },
        (Lang::Java, "keySet") => MapKeyViewContract {
            method: "keySet",
            kind: MapKeyViewKind::Collection,
        },
        (Lang::JavaScript | Lang::TypeScript | Lang::Vue | Lang::Svelte | Lang::Html, "keys") => {
            MapKeyViewContract {
                method: "keys",
                kind: MapKeyViewKind::Iterator,
            }
        }
        _ => return None,
    };
    Some(LibraryMapKeyViewContract {
        id: LibraryApiContractId::MapKeyView(result.kind),
        callee: LibraryApiCalleeContract::Method {
            method: result.method,
            receiver: MethodReceiverContract::ExactMap,
        },
        result,
    })
}

pub fn library_map_key_view_contract_by_hash(
    lang: Lang,
    method_hash: u64,
    arg_count: usize,
) -> Option<LibraryMapKeyViewContract> {
    ["keys", "keySet"].into_iter().find_map(|method| {
        (stable_symbol_hash(method) == method_hash)
            .then(|| library_map_key_view_contract(lang, method, arg_count))
            .flatten()
    })
}

pub fn library_map_key_view_wrapper_contract(
    lang: Lang,
    receiver: &str,
    method: &str,
    arg_count: usize,
) -> Option<LibraryMapKeyViewWrapperContract> {
    if !js_like_lang(lang) || receiver != "Array" || method != "from" || arg_count != 1 {
        return None;
    }
    let result = MapKeyViewWrapperContract {
        receiver: "Array",
        method: "from",
        qualified_path: "Array.from",
    };
    Some(LibraryMapKeyViewWrapperContract {
        id: LibraryApiContractId::MapKeyViewWrapper,
        callee: LibraryApiCalleeContract::StaticGlobalMethod {
            receiver: result.receiver,
            method: result.method,
            qualified_path: result.qualified_path,
            requires_unshadowed_receiver: true,
        },
        result,
    })
}

pub fn library_map_key_view_wrapper_contract_by_hash(
    lang: Lang,
    receiver: &str,
    method_hash: u64,
    arg_count: usize,
) -> Option<LibraryMapKeyViewWrapperContract> {
    (method_hash == stable_symbol_hash("from"))
        .then(|| library_map_key_view_wrapper_contract(lang, receiver, "from", arg_count))
        .flatten()
}

pub fn library_map_get_contract(
    lang: Lang,
    method: &str,
    arg_count: usize,
) -> Option<LibraryMapGetContract> {
    if !matches!(
        lang,
        Lang::Java
            | Lang::Rust
            | Lang::JavaScript
            | Lang::TypeScript
            | Lang::Vue
            | Lang::Svelte
            | Lang::Html
    ) || method != "get"
        || arg_count != 1
    {
        return None;
    }
    let result = MapGetContract {
        method: "get",
        receiver: MethodReceiverContract::ExactMap,
    };
    Some(LibraryMapGetContract {
        id: LibraryApiContractId::MapGet,
        callee: LibraryApiCalleeContract::Method {
            method: result.method,
            receiver: result.receiver,
        },
        result,
    })
}

pub fn library_map_get_contract_by_hash(
    lang: Lang,
    method_hash: u64,
    arg_count: usize,
) -> Option<LibraryMapGetContract> {
    (method_hash == stable_symbol_hash("get"))
        .then(|| library_map_get_contract(lang, "get", arg_count))
        .flatten()
}

pub fn library_js_array_is_array_contract(
    lang: Lang,
    receiver: &str,
    method: &str,
    arg_count: usize,
) -> Option<LibraryStaticGlobalMethodContract> {
    if !js_like_lang(lang) || receiver != "Array" || method != "isArray" || arg_count != 1 {
        return None;
    }
    let result = StaticGlobalMethodContract {
        receiver: "Array",
        method: "isArray",
        qualified_path: "Array.isArray",
        requires_unshadowed_receiver: true,
    };
    Some(LibraryStaticGlobalMethodContract {
        id: LibraryApiContractId::JsArrayIsArray,
        callee: LibraryApiCalleeContract::StaticGlobalMethod {
            receiver: result.receiver,
            method: result.method,
            qualified_path: result.qualified_path,
            requires_unshadowed_receiver: result.requires_unshadowed_receiver,
        },
        result,
    })
}

pub fn library_js_boolean_coercion_contract(
    lang: Lang,
    function: &str,
    arg_count: usize,
) -> Option<LibraryStaticGlobalFunctionContract> {
    if !js_like_lang(lang) || function != "Boolean" || arg_count != 1 {
        return None;
    }
    let result = StaticGlobalFunctionContract {
        function: "Boolean",
        requires_unshadowed_function: true,
    };
    Some(LibraryStaticGlobalFunctionContract {
        id: LibraryApiContractId::JsBooleanCoercion,
        callee: LibraryApiCalleeContract::StaticGlobalFunction {
            function: result.function,
            requires_unshadowed_function: result.requires_unshadowed_function,
        },
        result,
    })
}

pub fn library_regex_test_contract(
    lang: Lang,
    method: &str,
    arg_count: usize,
) -> Option<LibraryRegexTestContract> {
    if !js_like_lang(lang) || method != "test" || arg_count != 1 {
        return None;
    }
    let result = RegexTestContract {
        method: "test",
        required_receiver_fact: SourceFactKind::Literal(SourceLiteralKind::Regex),
    };
    Some(LibraryRegexTestContract {
        id: LibraryApiContractId::RegexTest,
        callee: LibraryApiCalleeContract::RegexLiteralMethod {
            method: result.method,
            required_receiver_fact: result.required_receiver_fact,
        },
        result,
    })
}

pub fn library_static_index_membership_contract(
    lang: Lang,
    method: &str,
    arg_count: usize,
) -> Option<LibraryStaticIndexMembershipContract> {
    let result = static_index_membership_contract(lang, method, arg_count)?;
    Some(LibraryStaticIndexMembershipContract {
        id: LibraryApiContractId::JsLikeStaticIndexMembership(result.kind),
        callee: LibraryApiCalleeContract::StaticIndexMembershipMethod {
            method: result.method,
            receiver: result.receiver,
        },
        result,
    })
}

pub fn library_imported_namespace_function_contract(
    lang: Lang,
    function: &str,
    arg_count: usize,
) -> Option<LibraryImportedNamespaceFunctionContract> {
    let result = match (lang, function, arg_count) {
        (Lang::Python, "prod", 1 | 2) => ImportedNamespaceFunctionContract {
            module: "math",
            function: "prod",
            receiver: MethodReceiverContract::ImportedNamespace("math"),
            semantic: ImportedNamespaceFunctionSemantic::ProductReduction {
                op: Op::Mul,
                identity: 1,
            },
        },
        _ => return None,
    };
    Some(LibraryImportedNamespaceFunctionContract {
        id: LibraryApiContractId::ImportedNamespaceFunction(result.semantic),
        callee: LibraryApiCalleeContract::ImportedNamespaceFunction {
            module: result.module,
            function: result.function,
        },
        result,
    })
}

pub fn library_promise_then_contract(
    lang: Lang,
    method: &str,
    arg_count: usize,
) -> Option<LibraryPromiseThenContract> {
    if !js_like_lang(lang) || method != "then" || arg_count != 1 {
        return None;
    }
    let result = PromiseThenContract {
        receiver: AsyncReceiverContract::ExactPromiseLike,
    };
    Some(LibraryPromiseThenContract {
        id: LibraryApiContractId::PromiseThen,
        callee: LibraryApiCalleeContract::AsyncMethod {
            method: "then",
            receiver: result.receiver,
        },
        result,
    })
}

pub fn library_iterator_identity_adapter_contract(
    lang: Lang,
    method: &str,
    arg_count: usize,
) -> Option<LibraryIteratorIdentityAdapterContract> {
    let method = if lang == Lang::Rust && arg_count == 0 {
        match method {
            "iter" => "iter",
            "into_iter" => "into_iter",
            "iter_mut" => "iter_mut",
            "collect" => "collect",
            "to_vec" => "to_vec",
            "copied" => "copied",
            "cloned" => "cloned",
            _ => return None,
        }
    } else if lang == Lang::Java && method == "stream" && arg_count == 0 {
        "stream"
    } else {
        return None;
    };
    let result = IteratorIdentityAdapterContract {
        receiver: IteratorAdapterReceiverContract::ExactIterableValue,
    };
    Some(LibraryIteratorIdentityAdapterContract {
        id: LibraryApiContractId::IteratorIdentityAdapter,
        callee: LibraryApiCalleeContract::IteratorAdapterMethod {
            method,
            receiver: result.receiver,
        },
        result,
    })
}

pub fn library_static_collection_adapter_contract(
    lang: Lang,
    receiver: &str,
    method: &str,
    arg_count: usize,
) -> Option<LibraryStaticCollectionAdapterContract> {
    if lang != Lang::Java || receiver != "Arrays" || method != "stream" || arg_count != 1 {
        return None;
    }
    let result = StaticCollectionAdapterContract {
        module: "java.util",
        exported: "Arrays",
    };
    Some(LibraryStaticCollectionAdapterContract {
        id: LibraryApiContractId::StaticCollectionAdapter,
        callee: LibraryApiCalleeContract::JavaUtilStaticMember {
            receiver: result.exported,
            method: "stream",
        },
        result,
    })
}

pub fn library_method_call_contract(
    lang: Lang,
    name: &str,
    arg_count: usize,
) -> Option<LibraryMethodCallContract> {
    let result = method_call_contract_shape(lang, name, arg_count)?;
    let method = library_method_selector_name(name)?;
    Some(LibraryMethodCallContract {
        id: LibraryApiContractId::MethodCall(result.semantic),
        callee: LibraryApiCalleeContract::Method {
            method,
            receiver: result.receiver,
        },
        result,
    })
}

pub fn library_receiver_method_api_contract(
    lang: Lang,
    method: &str,
    arg_count: usize,
) -> Option<LibraryReceiverMethodApiContract> {
    library_map_get_contract(lang, method, arg_count)
        .map(|contract| LibraryReceiverMethodApiContract {
            id: contract.id,
            callee: contract.callee,
            rule: "library_api_map_get",
        })
        .or_else(|| {
            library_map_key_view_contract(lang, method, arg_count).map(|contract| {
                LibraryReceiverMethodApiContract {
                    id: contract.id,
                    callee: contract.callee,
                    rule: "library_api_map_key_view",
                }
            })
        })
        .or_else(|| {
            library_iterator_identity_adapter_contract(lang, method, arg_count).map(|contract| {
                LibraryReceiverMethodApiContract {
                    id: contract.id,
                    callee: contract.callee,
                    rule: "library_api_iterator_identity_adapter",
                }
            })
        })
        .or_else(|| {
            library_scalar_integer_method_contract(lang, method, arg_count).map(|contract| {
                LibraryReceiverMethodApiContract {
                    id: contract.id,
                    callee: contract.callee,
                    rule: "library_api_scalar_integer_method",
                }
            })
        })
        .or_else(|| {
            library_rust_option_and_then_contract(lang, method, arg_count).map(|contract| {
                LibraryReceiverMethodApiContract {
                    id: contract.id,
                    callee: contract.callee,
                    rule: "library_api_rust_option_and_then",
                }
            })
        })
        .or_else(|| {
            library_method_call_contract(lang, method, arg_count).map(|contract| {
                LibraryReceiverMethodApiContract {
                    id: contract.id,
                    callee: contract.callee,
                    rule: "library_api_method_call",
                }
            })
        })
}

pub(crate) fn library_method_selector_name(name: &str) -> Option<&'static str> {
    Some(match name {
        "__contains__" => "__contains__",
        "Abs" => "Abs",
        "Contains" => "Contains",
        "HasPrefix" => "HasPrefix",
        "HasSuffix" => "HasSuffix",
        "Max" => "Max",
        "Min" => "Min",
        "Print" => "Print",
        "Printf" => "Printf",
        "Println" => "Println",
        "abs" => "abs",
        "all" => "all",
        "all?" => "all?",
        "allMatch" => "allMatch",
        "any" => "any",
        "any?" => "any?",
        "anyMatch" => "anyMatch",
        "and_then" => "and_then",
        "append" => "append",
        "clamp" => "clamp",
        "collect" => "collect",
        "contains" => "contains",
        "containsKey" => "containsKey",
        "contains_key" => "contains_key",
        "count" => "count",
        "debug" => "debug",
        "empty?" => "empty?",
        "end_with?" => "end_with?",
        "endsWith" => "endsWith",
        "ends_with" => "ends_with",
        "endswith" => "endswith",
        "every" => "every",
        "fetch" => "fetch",
        "filter" => "filter",
        "filter_map" => "filter_map",
        "flatMap" => "flatMap",
        "flat_map" => "flat_map",
        "fold" => "fold",
        "get" => "get",
        "getOrDefault" => "getOrDefault",
        "has" => "has",
        "has_key?" => "has_key?",
        "include?" => "include?",
        "includes" => "includes",
        "info" => "info",
        "inject" => "inject",
        "isEmpty" => "isEmpty",
        "is_empty" => "is_empty",
        "is_none" => "is_none",
        "is_some" => "is_some",
        "join" => "join",
        "key?" => "key?",
        "len" => "len",
        "length" => "length",
        "log" => "log",
        "map" => "map",
        "map_or" => "map_or",
        "max" => "max",
        "member?" => "member?",
        "min" => "min",
        "nil?" => "nil?",
        "push" => "push",
        "reduce" => "reduce",
        "select" => "select",
        "size" => "size",
        "some" => "some",
        "start_with?" => "start_with?",
        "startsWith" => "startsWith",
        "starts_with" => "starts_with",
        "startswith" => "startswith",
        "sum" => "sum",
        "unwrap_or" => "unwrap_or",
        "unwrap_or_else" => "unwrap_or_else",
        "zip" => "zip",
        _ => return None,
    })
}

fn library_free_name_shadow_policy(lang: Lang, shadow_guard: bool) -> LibraryApiShadowPolicy {
    if shadow_guard {
        LibraryApiShadowPolicy::SameName
    } else if lang == Lang::Rust {
        LibraryApiShadowPolicy::RustStdRootForStdPath
    } else {
        LibraryApiShadowPolicy::None
    }
}
