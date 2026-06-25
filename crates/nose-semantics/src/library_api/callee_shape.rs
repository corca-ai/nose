use super::*;

pub(in crate::library_api) fn library_api_callee_shape_matches(
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
        LibraryApiCalleeContract::LabeledFreeName {
            name, first_label, ..
        } => {
            var_name_matches(il, interner, callee_node, name)
                && call_first_arg_label_matches(il, interner, node, first_label)
        }
        LibraryApiCalleeContract::JsGlobalConstructor { receiver, .. } => {
            var_name_matches(il, interner, callee_node, receiver)
        }
        LibraryApiCalleeContract::ImportedBinding { exported, .. } => {
            imported_member_callee_shape_matches(il, interner, callee_node, exported)
        }
        LibraryApiCalleeContract::JavaUtilStaticMember { receiver, method }
        | LibraryApiCalleeContract::JavaStaticMember {
            receiver, method, ..
        } => {
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

pub(in crate::library_api) fn library_api_node_callee_shape_matches(
    il: &Il,
    interner: &Interner,
    node: NodeId,
    callee: LibraryApiCalleeContract,
) -> bool {
    match callee {
        LibraryApiCalleeContract::FreeName { name, .. } => {
            var_name_matches(il, interner, node, name)
        }
        LibraryApiCalleeContract::LabeledFreeName { .. } => false,
        LibraryApiCalleeContract::Property { property, .. } => {
            field_method_matches(il, interner, node, property)
        }
        _ => false,
    }
}

pub(in crate::library_api) fn call_first_arg_label_matches(
    il: &Il,
    interner: &Interner,
    call: NodeId,
    expected: &str,
) -> bool {
    let Some(first_arg) = il.children(call).get(1).copied() else {
        return false;
    };
    matches!(
        (il.kind(first_arg), il.node(first_arg).payload),
        (NodeKind::KwArg, Payload::Name(name)) if interner.resolve(name) == expected
    )
}

pub(in crate::library_api) fn method_callee_receiver(
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

pub(in crate::library_api) fn field_method_at_span(
    il: &Il,
    interner: &Interner,
    span: Span,
    expected: &str,
) -> bool {
    il.nodes_spanning(span).any(|id| {
        let node = il.node(id);
        node.span == span
            && node.kind == NodeKind::Field
            && matches!(node.payload, Payload::Name(method) if interner.resolve(method) == expected)
    })
}

pub(in crate::library_api) fn node_at_span(il: &Il, span: Span) -> Option<NodeId> {
    let mut found = None;
    for id in il.nodes_spanning(span) {
        let node = il.node(id);
        if node.span != span {
            continue;
        }
        match found {
            None => found = Some(id),
            Some(existing)
                if il.kind(existing) == node.kind && il.node(existing).payload == node.payload => {}
            Some(_) => return None,
        }
    }
    found
}

pub(in crate::library_api) fn node_at_span_with_kind(
    il: &Il,
    span: Span,
    kind: NodeKind,
) -> Option<NodeId> {
    let mut found = None;
    for id in il.nodes_spanning(span) {
        let node = il.node(id);
        if node.span != span || node.kind != kind {
            continue;
        }
        match found {
            None => found = Some(id),
            Some(existing) if il.node(existing).payload == node.payload => {}
            Some(_) => return None,
        }
    }
    found
}

pub(in crate::library_api) fn var_name_matches(
    il: &Il,
    interner: &Interner,
    node: NodeId,
    expected: &str,
) -> bool {
    matches!(
        (il.kind(node), il.node(node).payload),
        (NodeKind::Var, Payload::Name(name)) if interner.resolve(name) == expected
    )
}

pub(in crate::library_api) fn static_member_callee_parts<'a>(
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

pub(in crate::library_api) fn imported_member_callee_shape_matches(
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

pub(in crate::library_api) fn field_method_matches(
    il: &Il,
    interner: &Interner,
    node: NodeId,
    expected: &str,
) -> bool {
    matches!(
        (il.kind(node), il.node(node).payload),
        (NodeKind::Field, Payload::Name(method)) if interner.resolve(method) == expected
    )
}
