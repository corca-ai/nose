//! Provider-side literal export contracts for cross-file immutable import replacement.

use crate::{
    admitted_free_name_collection_factory_at_call, admitted_free_name_map_factory_at_call,
    admitted_imported_collection_factory_at_call, admitted_java_collection_factory_at_call,
    admitted_java_map_entry_at_call, admitted_java_map_factory_at_call,
    admitted_js_like_map_constructor_at_call, admitted_js_like_set_constructor_at_call,
    admitted_ruby_set_factory_at_call, go_zero_map_default_kind,
    go_zero_map_entry_contract_for_node, go_zero_map_literal_contract_for_node,
    import_fact_evidence_rhs, java_collection_factory_rejects_null_literal,
    java_map_factory_positional_arg_count_supported, java_map_factory_uses_positional_entries,
    nodes_contain_duplicate_static_literal_keys, nodes_contain_static_null_literal, semantics,
    seq_surface_contract_for_node, ImportedMapFactoryContract, JavaMapFactoryKind,
    LibraryApiContractId, LibraryCollectionFactoryResult, LibraryMapFactoryResult,
};
use nose_il::{Il, Interner, NodeId, NodeKind, Payload};

/// Whether `node` is a provider-owned literal value that can be snapshotted into
/// an importing file without treating raw import coordinates or API spellings as proof.
pub fn imported_literal_export_safe(il: &Il, interner: &Interner, node: NodeId) -> bool {
    match il.kind(node) {
        NodeKind::Lit => true,
        NodeKind::Seq => imported_literal_seq_safe(il, interner, node),
        NodeKind::Call => {
            imported_map_factory_call_safe(il, interner, node)
                || imported_collection_factory_call_safe(il, interner, node)
        }
        _ => false,
    }
}

/// Coarse diagnostic reason for a provider-owned value that cannot currently be
/// snapshotted across an immutable import boundary.
pub fn imported_literal_export_rejection_reason(
    il: &Il,
    interner: &Interner,
    node: NodeId,
) -> Option<&'static str> {
    if imported_literal_export_safe(il, interner, node) {
        return None;
    }
    Some(match il.kind(node) {
        NodeKind::Seq => imported_literal_seq_rejection_reason(il, interner, node),
        NodeKind::Call => imported_factory_call_rejection_reason(il, interner, node),
        _ => "unsupported-provider-rhs-shape",
    })
}

fn imported_literal_seq_safe(il: &Il, interner: &Interner, seq: NodeId) -> bool {
    if go_zero_map_literal_export_safe(il, interner, seq) {
        return true;
    }
    seq_surface_contract_for_node(il, interner, seq)
        .is_some_and(|contract| contract.imported_literal)
        && il
            .children(seq)
            .iter()
            .all(|&child| literal_export_value_safe(il, interner, child))
}

fn imported_literal_seq_rejection_reason(
    il: &Il,
    interner: &Interner,
    seq: NodeId,
) -> &'static str {
    if go_zero_map_literal_contract_for_node(il, interner, seq).is_some() {
        return literal_export_children_rejection_reason(il, interner, il.children(seq));
    }
    let Some(contract) = seq_surface_contract_for_node(il, interner, seq) else {
        return "provider-sequence-surface-proof-missing";
    };
    if !contract.imported_literal {
        return "provider-sequence-surface-not-import-literal-safe";
    }
    literal_export_children_rejection_reason(il, interner, il.children(seq))
}

fn go_zero_map_literal_export_safe(il: &Il, interner: &Interner, seq: NodeId) -> bool {
    go_zero_map_literal_contract_for_node(il, interner, seq).is_some()
        && !il.children(seq).is_empty()
        && go_zero_map_entries_export_safe(il, interner, il.children(seq))
}

fn go_zero_map_entries_export_safe(il: &Il, interner: &Interner, entries: &[NodeId]) -> bool {
    let mut value_kind = None;
    for &entry in entries {
        if go_zero_map_entry_contract_for_node(il, interner, entry).is_none() {
            return false;
        }
        let kids = il.children(entry);
        if kids.len() != 2 || !matches!(il.node(kids[0]).payload, Payload::LitStr(_)) {
            return false;
        }
        let Some(kind) = go_zero_map_default_kind(il.meta.lang, il.node(kids[1]).payload) else {
            return false;
        };
        match value_kind {
            Some(existing) if existing != kind => return false,
            Some(_) => {}
            None => value_kind = Some(kind),
        }
        if !literal_export_value_safe(il, interner, kids[0])
            || !literal_export_value_safe(il, interner, kids[1])
        {
            return false;
        }
    }
    true
}

fn literal_export_value_safe(il: &Il, interner: &Interner, node: NodeId) -> bool {
    match il.kind(node) {
        NodeKind::Lit => true,
        NodeKind::Seq => {
            if import_fact_evidence_rhs(il, node).is_some() {
                return false;
            }
            if !seq_surface_contract_for_node(il, interner, node)
                .is_some_and(|contract| contract.exact_tree_safe)
            {
                return false;
            }
            il.children(node)
                .iter()
                .all(|&child| literal_export_value_safe(il, interner, child))
        }
        NodeKind::UnOp => il
            .children(node)
            .iter()
            .all(|&child| literal_export_value_safe(il, interner, child)),
        NodeKind::Call => java_map_entry_call_safe(il, interner, node),
        _ => false,
    }
}

fn literal_export_children_rejection_reason(
    il: &Il,
    interner: &Interner,
    children: &[NodeId],
) -> &'static str {
    children
        .iter()
        .find_map(|&child| literal_export_value_rejection_reason(il, interner, child))
        .unwrap_or("provider-aggregate-children-not-exact-safe")
}

fn literal_export_value_rejection_reason(
    il: &Il,
    interner: &Interner,
    node: NodeId,
) -> Option<&'static str> {
    match il.kind(node) {
        NodeKind::Lit => None,
        NodeKind::Seq => {
            if import_fact_evidence_rhs(il, node).is_some() {
                return Some("provider-aggregate-child-import-coordinate-boundary");
            }
            let contract = seq_surface_contract_for_node(il, interner, node)?;
            if !contract.exact_tree_safe {
                return Some("provider-aggregate-child-surface-not-exact-safe");
            }
            il.children(node)
                .iter()
                .find_map(|&child| literal_export_value_rejection_reason(il, interner, child))
        }
        NodeKind::UnOp => il
            .children(node)
            .iter()
            .find_map(|&child| literal_export_value_rejection_reason(il, interner, child)),
        NodeKind::Var | NodeKind::Field | NodeKind::Index => {
            Some("provider-aggregate-child-reference-boundary")
        }
        NodeKind::Call => {
            if java_map_entry_call_safe(il, interner, node) {
                None
            } else {
                Some("provider-aggregate-child-call-boundary")
            }
        }
        _ => Some("provider-aggregate-children-not-exact-safe"),
    }
}

fn imported_map_factory_call_safe(il: &Il, interner: &Interner, call: NodeId) -> bool {
    if js_like_map_constructor_call_safe(il, interner, call) {
        return true;
    }
    match semantics(il.meta.lang).stdlib().imported_map_factory() {
        Some(ImportedMapFactoryContract::JavaMap) => java_map_factory_call_safe(il, interner, call),
        Some(ImportedMapFactoryContract::RustStdMap) => {
            rust_std_map_factory_call_safe(il, interner, call)
        }
        None => false,
    }
}

fn imported_factory_call_rejection_reason(
    il: &Il,
    interner: &Interner,
    call: NodeId,
) -> &'static str {
    if importable_factory_api_occurrence_present(il, interner, call) {
        "provider-factory-arguments-not-exact-safe"
    } else {
        "provider-library-api-proof-missing"
    }
}

fn importable_factory_api_occurrence_present(il: &Il, interner: &Interner, call: NodeId) -> bool {
    admitted_js_like_map_constructor_at_call(il, interner, call).is_some()
        || admitted_js_like_set_constructor_at_call(il, interner, call).is_some()
        || admitted_free_name_collection_factory_at_call(il, interner, call).is_some()
        || admitted_imported_collection_factory_at_call(il, interner, call).is_some()
        || admitted_java_collection_factory_at_call(il, interner, call).is_some()
        || admitted_ruby_set_factory_at_call(il, interner, call).is_some()
        || match semantics(il.meta.lang).stdlib().imported_map_factory() {
            Some(ImportedMapFactoryContract::JavaMap) => {
                admitted_java_map_factory_at_call(il, interner, call).is_some()
            }
            Some(ImportedMapFactoryContract::RustStdMap) => {
                admitted_free_name_map_factory_at_call(il, interner, call).is_some()
            }
            None => false,
        }
}

fn imported_collection_factory_call_safe(il: &Il, interner: &Interner, call: NodeId) -> bool {
    free_name_collection_factory_call_safe(il, interner, call)
        || imported_binding_collection_factory_call_safe(il, interner, call)
        || java_collection_factory_call_safe(il, interner, call)
        || ruby_set_factory_call_safe(il, interner, call)
        || js_like_set_constructor_call_safe(il, interner, call)
}

fn free_name_collection_factory_call_safe(il: &Il, interner: &Interner, call: NodeId) -> bool {
    let kids = il.children(call);
    let Some((_callee, args)) = kids.split_first() else {
        return false;
    };
    let Some(occurrence) = admitted_free_name_collection_factory_at_call(il, interner, call) else {
        return false;
    };
    collection_factory_args_export_safe(il, interner, occurrence.contract.result, args)
}

fn imported_binding_collection_factory_call_safe(
    il: &Il,
    interner: &Interner,
    call: NodeId,
) -> bool {
    let kids = il.children(call);
    let Some((_callee, args)) = kids.split_first() else {
        return false;
    };
    let Some(occurrence) = admitted_imported_collection_factory_at_call(il, interner, call) else {
        return false;
    };
    collection_factory_args_export_safe(il, interner, occurrence.contract.result, args)
}

fn java_collection_factory_call_safe(il: &Il, interner: &Interner, call: NodeId) -> bool {
    let kids = il.children(call);
    let Some((_callee, args)) = kids.split_first() else {
        return false;
    };
    let Some(occurrence) = admitted_java_collection_factory_at_call(il, interner, call) else {
        return false;
    };
    if matches!(
        occurrence.contract.id,
        LibraryApiContractId::JavaCollectionFactory(kind)
            if java_collection_factory_rejects_null_literal(kind)
    ) && nodes_contain_static_null_literal(il, args.iter().copied())
    {
        return false;
    }
    collection_factory_args_export_safe(il, interner, occurrence.contract.result, args)
}

fn ruby_set_factory_call_safe(il: &Il, interner: &Interner, call: NodeId) -> bool {
    let kids = il.children(call);
    let Some((_callee, args)) = kids.split_first() else {
        return false;
    };
    let Some(occurrence) = admitted_ruby_set_factory_at_call(il, interner, call) else {
        return false;
    };
    collection_factory_args_export_safe(il, interner, occurrence.contract.result, args)
}

fn collection_factory_args_export_safe(
    il: &Il,
    interner: &Interner,
    result: LibraryCollectionFactoryResult,
    args: &[NodeId],
) -> bool {
    match result {
        LibraryCollectionFactoryResult::SequenceArgument
        | LibraryCollectionFactoryResult::StaticNonFloatSequenceArgument => {
            args.len() == 1 && literal_export_value_safe(il, interner, args[0])
        }
        LibraryCollectionFactoryResult::EmptySequence => args.is_empty(),
        LibraryCollectionFactoryResult::ElementArguments => {
            args.len() == 1 && literal_export_value_safe(il, interner, args[0])
        }
        LibraryCollectionFactoryResult::VariadicElements {
            single_arg_spreads_array,
        } => {
            if args.len() == 1 && single_arg_spreads_array {
                return false;
            }
            args.iter()
                .all(|&arg| literal_export_value_safe(il, interner, arg))
        }
    }
}

fn js_like_map_constructor_call_safe(il: &Il, interner: &Interner, call: NodeId) -> bool {
    let kids = il.children(call);
    if kids.len() != 2 {
        return false;
    }
    let Some(occurrence) = admitted_js_like_map_constructor_at_call(il, interner, call) else {
        return false;
    };
    if occurrence.arg_count != 1 {
        return false;
    }
    let LibraryMapFactoryResult::EntrySequence { entry_seq_tag } = occurrence.contract.result
    else {
        return false;
    };
    if entry_seq_tag != crate::SEQ_VALUE_COLLECTION {
        return false;
    }
    js_like_map_entry_sequence_export_safe(il, interner, kids[1])
}

fn js_like_set_constructor_call_safe(il: &Il, interner: &Interner, call: NodeId) -> bool {
    let kids = il.children(call);
    if kids.len() != 2 {
        return false;
    }
    let Some(occurrence) = admitted_js_like_set_constructor_at_call(il, interner, call) else {
        return false;
    };
    if occurrence.arg_count != 1 {
        return false;
    }
    if occurrence.contract.result != LibraryCollectionFactoryResult::StaticNonFloatSequenceArgument
    {
        return false;
    }
    literal_export_value_safe(il, interner, kids[1])
}

fn js_like_map_entry_sequence_export_safe(il: &Il, interner: &Interner, entries: NodeId) -> bool {
    if !literal_export_value_safe(il, interner, entries) {
        return false;
    }
    if !seq_surface_contract_for_node(il, interner, entries)
        .is_some_and(|contract| contract.exact_tree_safe)
    {
        return false;
    }
    il.children(entries).iter().all(|&entry| {
        il.kind(entry) == NodeKind::Seq
            && il.children(entry).len() == 2
            && seq_surface_contract_for_node(il, interner, entry)
                .is_some_and(|contract| contract.exact_tree_safe)
    })
}

fn java_map_factory_call_safe(il: &Il, interner: &Interner, call: NodeId) -> bool {
    let kids = il.children(call);
    let Some((_callee, args)) = kids.split_first() else {
        return false;
    };
    let Some(occurrence) = admitted_java_map_factory_at_call(il, interner, call) else {
        return false;
    };
    let LibraryMapFactoryResult::JavaFactory { kind } = occurrence.contract.result else {
        return false;
    };
    match kind {
        kind if java_map_factory_uses_positional_entries(kind) => {
            java_map_factory_positional_arg_count_supported(kind, args.len())
                && java_map_positional_args_export_safe(il, kind, args)
                && args
                    .iter()
                    .all(|&arg| literal_export_value_safe(il, interner, arg))
        }
        JavaMapFactoryKind::OfEntries => args
            .iter()
            .all(|&arg| java_map_entry_call_safe(il, interner, arg)),
        _ => false,
    }
}

fn java_map_positional_args_export_safe(
    il: &Il,
    kind: JavaMapFactoryKind,
    args: &[NodeId],
) -> bool {
    if kind != JavaMapFactoryKind::GuavaImmutableMapOf {
        return true;
    }
    !nodes_contain_static_null_literal(il, args.iter().copied())
        && !nodes_contain_duplicate_static_literal_keys(il, args.iter().step_by(2).copied())
}

fn java_map_entry_call_safe(il: &Il, interner: &Interner, call: NodeId) -> bool {
    if il.kind(call) != NodeKind::Call {
        return false;
    }
    let kids = il.children(call);
    if kids.len() != 3 {
        return false;
    }
    let Some(occurrence) = admitted_java_map_entry_at_call(il, interner, call) else {
        return false;
    };
    if occurrence.arg_count != 2 {
        return false;
    }
    literal_export_value_safe(il, interner, kids[1])
        && literal_export_value_safe(il, interner, kids[2])
}

fn rust_std_map_factory_call_safe(il: &Il, interner: &Interner, call: NodeId) -> bool {
    let kids = il.children(call);
    if kids.len() != 2 {
        return false;
    }
    let Some(occurrence) = admitted_free_name_map_factory_at_call(il, interner, call) else {
        return false;
    };
    if occurrence.arg_count != 1 {
        return false;
    }
    let LibraryMapFactoryResult::EntrySequence { .. } = occurrence.contract.result else {
        return false;
    };
    literal_export_value_safe(il, interner, kids[1])
}
