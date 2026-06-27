use super::bindings::{assignment_name, import_namespace_proof, BindingUseIndex};
use super::exports::LiteralExports;
use super::snapshot::{snapshot_subtree, SubtreeSnapshot};
use super::FileImportContext;
use nose_il::{stable_symbol_hash, Il, Interner, NodeId, NodeKind, Payload, Symbol};
use nose_semantics::{
    binding_write_target, opaque_argument_escape_args, receiver_mutation_call_receiver, semantics,
};
use rustc_hash::FxHashSet;

pub(super) struct NamespaceMemberReplacement {
    pub(super) node: NodeId,
    pub(super) import_evidence: nose_il::EvidenceId,
    pub(super) module_hash: u64,
    pub(super) exported_hash: u64,
    pub(super) deps: Vec<SubtreeSnapshot>,
    pub(super) rhs_snapshot: SubtreeSnapshot,
}

pub(super) fn collect_namespace_member_replacements(
    files: &[Il],
    interner: &Interner,
    contexts: &[FileImportContext],
    exports: &LiteralExports,
) -> Vec<Vec<NamespaceMemberReplacement>> {
    files
        .iter()
        .enumerate()
        .map(|(file_idx, il)| {
            if !semantics(il.meta.lang)
                .modules()
                .go_import_namespace_facts()
            {
                return Vec::new();
            }
            let context = &contexts[file_idx];
            let Some(top_level) = context.top_level.as_deref() else {
                return Vec::new();
            };
            let Some(binding_uses) = context.binding_uses.as_ref() else {
                return Vec::new();
            };
            collect_file_namespace_replacements(
                files,
                interner,
                file_idx,
                il,
                top_level,
                binding_uses,
                exports,
            )
        })
        .collect()
}

fn collect_file_namespace_replacements(
    files: &[Il],
    interner: &Interner,
    file_idx: usize,
    il: &Il,
    top_level: &[NodeId],
    binding_uses: &BindingUseIndex,
    exports: &LiteralExports,
) -> Vec<NamespaceMemberReplacement> {
    let namespace_imports: Vec<NamespaceImport> = top_level
        .iter()
        .copied()
        .filter_map(|stmt| {
            let namespace = assignment_name(il, stmt)?;
            let proof = import_namespace_proof(il, stmt)?;
            Some(NamespaceImport {
                stmt,
                namespace,
                module_hash: proof.module_hash,
                evidence: proof.evidence,
            })
        })
        .collect();
    if namespace_imports.is_empty() {
        return Vec::new();
    }
    let imported_namespaces: FxHashSet<Symbol> = namespace_imports
        .iter()
        .map(|import| import.namespace)
        .collect();
    let shadowed_namespaces = namespace_params(il, &imported_namespaces);
    let unsafe_exports = unsafe_namespace_member_exports(il, interner, &imported_namespaces);
    let member_fields = namespace_member_fields(il, &imported_namespaces);

    let mut out = Vec::new();
    for import in namespace_imports {
        if binding_uses.binding_mutated(il, import.namespace, import.stmt)
            || shadowed_namespaces.contains(&import.namespace)
        {
            continue;
        }
        for &(field, namespace, exported) in &member_fields {
            if namespace != import.namespace {
                continue;
            }
            if unsafe_exports.contains(&(namespace, exported)) {
                continue;
            }
            let exported_hash = stable_symbol_hash(interner.resolve(exported));
            let Some(export) = exports.get_exact(import.module_hash, exported_hash) else {
                continue;
            };
            if export.file_idx == file_idx || files[export.file_idx].meta.lang != il.meta.lang {
                continue;
            }
            out.push(NamespaceMemberReplacement {
                node: field,
                import_evidence: import.evidence,
                module_hash: import.module_hash,
                exported_hash,
                deps: export.deps.clone(),
                rhs_snapshot: snapshot_subtree(&files[export.file_idx], export.rhs),
            });
        }
    }
    out
}

struct NamespaceImport {
    stmt: NodeId,
    namespace: Symbol,
    module_hash: u64,
    evidence: nose_il::EvidenceId,
}

fn namespace_member_fields(
    il: &Il,
    namespaces: &FxHashSet<Symbol>,
) -> Vec<(NodeId, Symbol, Symbol)> {
    (0..il.nodes.len())
        .map(|idx| NodeId(idx as u32))
        .filter_map(|node| {
            let (namespace, exported) = namespace_member_name(il, node)?;
            namespaces
                .contains(&namespace)
                .then_some((node, namespace, exported))
        })
        .collect()
}

fn namespace_member_name(il: &Il, node: NodeId) -> Option<(Symbol, Symbol)> {
    if il.kind(node) != NodeKind::Field {
        return None;
    }
    let Payload::Name(exported) = il.node(node).payload else {
        return None;
    };
    let [receiver] = il.children(node) else {
        return None;
    };
    if il.kind(*receiver) != NodeKind::Var {
        return None;
    }
    match il.node(*receiver).payload {
        Payload::Name(namespace) => Some((namespace, exported)),
        _ => None,
    }
}

fn namespace_params(il: &Il, namespaces: &FxHashSet<Symbol>) -> FxHashSet<Symbol> {
    il.nodes
        .iter()
        .filter_map(|node| match (node.kind, node.payload) {
            (NodeKind::Param, Payload::Name(name)) if namespaces.contains(&name) => Some(name),
            _ => None,
        })
        .collect()
}

fn unsafe_namespace_member_exports(
    il: &Il,
    interner: &Interner,
    namespaces: &FxHashSet<Symbol>,
) -> FxHashSet<(Symbol, Symbol)> {
    let mut unsafe_exports = FxHashSet::default();
    for idx in 0..il.nodes.len() {
        let node = NodeId(idx as u32);
        match il.kind(node) {
            NodeKind::Assign => {
                if let Some(target) = binding_write_target(il, node) {
                    collect_namespace_members(il, target, namespaces, &mut unsafe_exports);
                }
            }
            NodeKind::Call => {
                if let Some(receiver) = receiver_mutation_call_receiver(il, interner, node) {
                    collect_namespace_members(il, receiver, namespaces, &mut unsafe_exports);
                }
                if let Some(args) = opaque_argument_escape_args(il, node) {
                    for &arg in args {
                        collect_namespace_members(il, arg, namespaces, &mut unsafe_exports);
                    }
                }
            }
            _ => {}
        }
    }
    unsafe_exports
}

fn collect_namespace_members(
    il: &Il,
    node: NodeId,
    namespaces: &FxHashSet<Symbol>,
    out: &mut FxHashSet<(Symbol, Symbol)>,
) {
    if let Some((namespace, exported)) = namespace_member_name(il, node) {
        if namespaces.contains(&namespace) {
            out.insert((namespace, exported));
        }
    }
    for &child in il.children(node) {
        collect_namespace_members(il, child, namespaces, out);
    }
}
