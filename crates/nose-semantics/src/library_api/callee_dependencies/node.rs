use super::*;

pub(in crate::library_api) fn library_api_dependencies_match_callee(
    il: &Il,
    interner: &Interner,
    node: NodeId,
    callee: LibraryApiCalleeContract,
    record: &EvidenceRecord,
) -> bool {
    if !library_api_callee_shape_matches(il, interner, node, callee) {
        return false;
    }
    if matches!(
        callee,
        LibraryApiCalleeContract::Method {
            receiver: MethodReceiverContract::RubyCoreNilPredicate,
            ..
        }
    ) {
        // This receiver contract has no node-local dependencies. Its completed
        // file-wide redefinition proof is what licensed the first-party
        // occurrence record in the first place; admission has already checked
        // that record's exact builtin provenance and callee shape.
        return record.dependencies.is_empty();
    }
    if matches!(
        callee,
        LibraryApiCalleeContract::Method { .. }
            | LibraryApiCalleeContract::IteratorAdapterMethod { .. }
            | LibraryApiCalleeContract::AsyncMethod { .. }
    ) {
        return library_api_receiver_dependencies_for_call(il, interner, node, callee)
            .is_some_and(|dependencies| dependency_ids_are_present(record, &dependencies));
    }
    let Some((call_span, callee_span, receiver_span)) = call_dependency_spans(il, node) else {
        return false;
    };
    library_api_dependencies_match_callee_at_span(
        il,
        interner,
        call_span,
        Some(callee_span),
        receiver_span,
        callee,
        record,
    )
}

fn call_dependency_spans(il: &Il, call: NodeId) -> Option<(Span, Span, Option<Span>)> {
    let callee = il.children(call).first().copied()?;
    let receiver_span = il
        .children(callee)
        .first()
        .map(|&receiver| il.node(receiver).span);
    Some((il.node(call).span, il.node(callee).span, receiver_span))
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
