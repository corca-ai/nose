use super::snapshot::{snapshot_subtree, SubtreeSnapshot};
use nose_il::{EvidenceId, Il, Interner, NodeId, NodeKind, Payload, Symbol, UnitKind};
use nose_semantics::{import_fact_proof_rhs, ImportFactKind};
use rustc_hash::{FxHashMap, FxHashSet};

pub(super) struct ImportBindingProof {
    pub(super) module_hash: u64,
    pub(super) exported_hash: u64,
    pub(super) evidence: EvidenceId,
}

pub(super) struct ImportNamespaceProof {
    pub(super) module_hash: u64,
    pub(super) evidence: EvidenceId,
}

pub(super) fn import_dependency_snapshots(
    il: &Il,
    rhs: NodeId,
    top_level: &[NodeId],
) -> Vec<SubtreeSnapshot> {
    top_level
        .iter()
        .copied()
        .filter(|&stmt| {
            assignment_rhs(il, stmt).is_some_and(|dep_rhs| {
                import_binding_key(il, stmt).is_some() && il.kind(dep_rhs) == NodeKind::Seq
            })
        })
        .filter(|&stmt| {
            assignment_name(il, stmt).is_some_and(|name| node_contains_symbol(il, rhs, name))
        })
        .map(|stmt| snapshot_subtree(il, stmt))
        .collect()
}

pub(super) fn collect_top_level_statements(il: &Il) -> Vec<NodeId> {
    let class_roots: FxHashSet<NodeId> = il
        .units
        .iter()
        .filter_map(|unit| (unit.kind == UnitKind::Class).then_some(unit.root))
        .collect();
    collect_statements_for_root_except(il, il.root, &class_roots)
}

pub(super) fn collect_statements_for_root(il: &Il, root: NodeId) -> Vec<NodeId> {
    collect_statements_for_root_except(il, root, &FxHashSet::default())
}

fn collect_statements_for_root_except(
    il: &Il,
    root: NodeId,
    non_flattened_blocks: &FxHashSet<NodeId>,
) -> Vec<NodeId> {
    il.children(root)
        .iter()
        .copied()
        .fold(Vec::new(), |mut statements, node| {
            match il.kind(node) {
                NodeKind::Block if non_flattened_blocks.contains(&node) => statements.push(node),
                NodeKind::Block => statements.extend_from_slice(il.children(node)),
                _ => statements.push(node),
            }
            statements
        })
}

pub(super) fn assignment_name(il: &Il, stmt: NodeId) -> Option<Symbol> {
    let (lhs, _) = il.assignment_var_parts(stmt)?;
    il.var_name(lhs)
}

pub(super) fn assignment_rhs(il: &Il, stmt: NodeId) -> Option<NodeId> {
    il.assignment_parts(stmt).map(|(_, rhs)| rhs)
}

pub(super) fn import_binding_key(il: &Il, stmt: NodeId) -> Option<(u64, u64)> {
    let proof = import_binding_proof(il, stmt)?;
    Some((proof.module_hash, proof.exported_hash))
}

pub(super) fn import_binding_proof(il: &Il, stmt: NodeId) -> Option<ImportBindingProof> {
    let rhs = assignment_rhs(il, stmt)?;
    let proof = import_fact_proof_rhs(il, rhs)?;
    let fact = proof.fact;
    if fact.kind != ImportFactKind::Binding {
        return None;
    }
    let exported_hash = fact.exported_hash?;
    Some(ImportBindingProof {
        module_hash: fact.module_hash,
        exported_hash,
        evidence: proof.evidence,
    })
}

pub(super) fn import_namespace_proof(il: &Il, stmt: NodeId) -> Option<ImportNamespaceProof> {
    let rhs = assignment_rhs(il, stmt)?;
    let proof = import_fact_proof_rhs(il, rhs)?;
    let fact = proof.fact;
    if fact.kind != ImportFactKind::Namespace {
        return None;
    }
    Some(ImportNamespaceProof {
        module_hash: fact.module_hash,
        evidence: proof.evidence,
    })
}

pub(super) struct BindingUseIndex {
    assignment_lhs_counts: FxHashMap<Symbol, usize>,
    receiver_mutation_symbols: FxHashSet<Symbol>,
    escaping_call_arg_symbols: FxHashSet<Symbol>,
}

impl BindingUseIndex {
    pub(super) fn new(il: &Il, interner: &Interner) -> Self {
        let mut out = Self {
            assignment_lhs_counts: FxHashMap::default(),
            receiver_mutation_symbols: FxHashSet::default(),
            escaping_call_arg_symbols: FxHashSet::default(),
        };
        for (idx, node) in il.nodes.iter().enumerate() {
            let node_id = NodeId(idx as u32);
            match node.kind {
                NodeKind::Assign => {
                    if let Some(lhs) = nose_semantics::binding_write_target(il, node_id) {
                        let mut lhs_symbols = FxHashSet::default();
                        collect_symbols_into_set(il, lhs, &mut lhs_symbols);
                        for symbol in lhs_symbols {
                            *out.assignment_lhs_counts.entry(symbol).or_insert(0) += 1;
                        }
                    }
                }
                NodeKind::Call => {
                    out.collect_receiver_mutation_symbol(il, interner, node_id);
                    out.collect_call_argument_escapes(il, node_id);
                }
                _ => {}
            }
        }
        out
    }

    pub(super) fn binding_mutated(&self, il: &Il, name: Symbol, defining_stmt: NodeId) -> bool {
        let defining_lhs_refs_name = il
            .children(defining_stmt)
            .first()
            .is_some_and(|&lhs| node_contains_symbol(il, lhs, name));
        let own_assignment = usize::from(defining_lhs_refs_name);
        self.assignment_lhs_counts.get(&name).copied().unwrap_or(0) > own_assignment
            || self.receiver_mutation_symbols.contains(&name)
    }

    pub(super) fn exported_binding_unsafe(
        &self,
        il: &Il,
        name: Symbol,
        defining_stmt: NodeId,
    ) -> bool {
        self.binding_mutated(il, name, defining_stmt)
            || self.escaping_call_arg_symbols.contains(&name)
    }

    fn collect_receiver_mutation_symbol(&mut self, il: &Il, interner: &Interner, call: NodeId) {
        let Some(receiver) = nose_semantics::receiver_mutation_call_receiver(il, interner, call)
        else {
            return;
        };
        if let Payload::Name(name) = il.node(receiver).payload {
            self.receiver_mutation_symbols.insert(name);
        }
    }

    fn collect_call_argument_escapes(&mut self, il: &Il, call: NodeId) {
        let Some(args) = nose_semantics::opaque_argument_escape_args(il, call) else {
            return;
        };
        for &arg in args {
            collect_symbols_into_set(il, arg, &mut self.escaping_call_arg_symbols);
        }
    }
}

fn node_refers_to_symbol(il: &Il, node: NodeId, name: Symbol) -> bool {
    match il.node(node).payload {
        Payload::Name(symbol) => symbol == name,
        _ => false,
    }
}

fn node_contains_symbol(il: &Il, node: NodeId, name: Symbol) -> bool {
    node_refers_to_symbol(il, node, name)
        || il
            .children(node)
            .iter()
            .any(|&child| node_contains_symbol(il, child, name))
}

fn collect_symbols_into_set(il: &Il, node: NodeId, out: &mut FxHashSet<Symbol>) {
    if let Payload::Name(symbol) = il.node(node).payload {
        out.insert(symbol);
    }
    for &child in il.children(node) {
        collect_symbols_into_set(il, child, out);
    }
}
