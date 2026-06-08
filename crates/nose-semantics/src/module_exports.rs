//! Provider-side literal export contracts for cross-file immutable import replacement.

use crate::{
    admitted_free_name_map_factory_at_call, admitted_java_map_entry_at_call,
    admitted_java_map_factory_at_call, import_fact_evidence_rhs, semantics,
    seq_surface_contract_evidence_for_node, ImportedMapFactoryContract, JavaMapFactoryKind,
    LibraryMapFactoryResult,
};
use nose_il::{Il, Interner, NodeId, NodeKind};

/// Whether `node` is a provider-owned literal value that can be snapshotted into
/// an importing file without treating raw import coordinates or API spellings as proof.
pub fn imported_literal_export_safe(il: &Il, interner: &Interner, node: NodeId) -> bool {
    match il.kind(node) {
        NodeKind::Seq => {
            seq_surface_contract_evidence_for_node(il, interner, node)
                .is_some_and(|contract| contract.imported_literal)
                && il
                    .children(node)
                    .iter()
                    .all(|&child| literal_export_value_safe(il, interner, child))
        }
        NodeKind::Call => imported_map_factory_call_safe(il, interner, node),
        _ => false,
    }
}

fn literal_export_value_safe(il: &Il, interner: &Interner, node: NodeId) -> bool {
    match il.kind(node) {
        NodeKind::Lit => true,
        NodeKind::Seq => {
            if import_fact_evidence_rhs(il, node).is_some() {
                return false;
            }
            if !seq_surface_contract_evidence_for_node(il, interner, node)
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
        JavaMapFactoryKind::Of => {
            args.len() % 2 == 0
                && args
                    .iter()
                    .all(|&arg| literal_export_value_safe(il, interner, arg))
        }
        JavaMapFactoryKind::OfEntries => args
            .iter()
            .all(|&arg| java_map_entry_call_safe(il, interner, arg)),
    }
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
