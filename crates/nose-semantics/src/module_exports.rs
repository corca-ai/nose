//! Provider-side literal export contracts for cross-file immutable import replacement.

use crate::{
    admitted_free_name_map_factory_at_call, admitted_java_map_entry_at_call,
    admitted_java_map_factory_at_call, go_zero_map_default_kind,
    go_zero_map_entry_contract_for_node, go_zero_map_literal_contract_for_node,
    import_fact_evidence_rhs, java_map_factory_positional_arg_count_supported,
    java_map_factory_uses_positional_entries, nodes_contain_duplicate_static_literal_keys,
    nodes_contain_static_null_literal, semantics, seq_surface_contract_for_node,
    ImportedMapFactoryContract, JavaMapFactoryKind, LibraryMapFactoryResult,
};
use nose_il::{Il, Interner, NodeId, NodeKind, Payload};

/// Whether `node` is a provider-owned literal value that can be snapshotted into
/// an importing file without treating raw import coordinates or API spellings as proof.
pub fn imported_literal_export_safe(il: &Il, interner: &Interner, node: NodeId) -> bool {
    match il.kind(node) {
        NodeKind::Lit => true,
        NodeKind::Seq => imported_literal_seq_safe(il, interner, node),
        NodeKind::Call => imported_map_factory_call_safe(il, interner, node),
        _ => false,
    }
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

fn imported_map_factory_call_safe(il: &Il, interner: &Interner, call: NodeId) -> bool {
    match semantics(il.meta.lang).stdlib().imported_map_factory() {
        Some(ImportedMapFactoryContract::JavaMap) => java_map_factory_call_safe(il, interner, call),
        Some(ImportedMapFactoryContract::RustStdMap) => {
            rust_std_map_factory_call_safe(il, interner, call)
        }
        None => false,
    }
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
