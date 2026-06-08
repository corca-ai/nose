//! Provider-side literal export contracts for cross-file immutable import replacement.

use crate::{
    import_fact_evidence_rhs, library_api_contract_evidence_for_call,
    library_free_name_map_factory_contract, library_java_map_entry_contract,
    library_java_map_factory_contract, semantics, seq_surface_contract_evidence_for_node,
    ImportedMapFactoryContract, JavaMapFactoryKind, LibraryApiEvidenceStatus,
    LibraryMapFactoryResult,
};
use nose_il::{Il, Interner, NodeId, NodeKind, Payload};

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
    let Some((&callee, args)) = kids.split_first() else {
        return false;
    };
    let Some((_receiver_node, method)) = field_method_on_var(il, interner, callee, "Map") else {
        return false;
    };
    let Some(contract) = library_java_map_factory_contract(il.meta.lang, "Map", method) else {
        return false;
    };
    if !library_api_evidence_required(il, interner, call, contract.id, contract.callee) {
        return false;
    }
    let LibraryMapFactoryResult::JavaFactory { kind } = contract.result else {
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
    let Some((_receiver_node, method)) = field_method_on_var(il, interner, kids[0], "Map") else {
        return false;
    };
    let Some(contract) = library_java_map_entry_contract(il.meta.lang, "Map", method) else {
        return false;
    };
    if !library_api_evidence_required(il, interner, call, contract.id, contract.callee) {
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
    let Some(name) = var_text(il, interner, kids[0]) else {
        return false;
    };
    let Some(contract) = library_free_name_map_factory_contract(il.meta.lang, name) else {
        return false;
    };
    if !library_api_evidence_required(il, interner, call, contract.id, contract.callee) {
        return false;
    }
    let LibraryMapFactoryResult::EntrySequence { .. } = contract.result else {
        return false;
    };
    literal_export_value_safe(il, interner, kids[1])
}

fn library_api_evidence_required(
    il: &Il,
    interner: &Interner,
    call: NodeId,
    contract_id: crate::LibraryApiContractId,
    callee: crate::LibraryApiCalleeContract,
) -> bool {
    matches!(
        library_api_contract_evidence_for_call(
            il,
            interner,
            call,
            contract_id,
            callee,
            il.children(call).len().saturating_sub(1),
        ),
        LibraryApiEvidenceStatus::Admitted
    )
}

fn field_method_on_var<'a>(
    il: &Il,
    interner: &'a Interner,
    node: NodeId,
    receiver: &str,
) -> Option<(NodeId, &'a str)> {
    if il.kind(node) != NodeKind::Field {
        return None;
    }
    let Payload::Name(method) = il.node(node).payload else {
        return None;
    };
    let receiver_node = il.children(node).first().copied()?;
    var_named(il, interner, receiver_node, receiver)
        .then(|| (receiver_node, interner.resolve(method)))
}

fn var_named(il: &Il, interner: &Interner, node: NodeId, expected: &str) -> bool {
    var_text(il, interner, node).is_some_and(|name| name == expected)
}

fn var_text<'a>(il: &Il, interner: &'a Interner, node: NodeId) -> Option<&'a str> {
    if il.kind(node) != NodeKind::Var {
        return None;
    }
    let Payload::Name(name) = il.node(node).payload else {
        return None;
    };
    Some(interner.resolve(name))
}
