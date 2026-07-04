use nose_il::{
    stable_symbol_hash, DomainEvidence, EvidenceAnchor, EvidenceEmitter, EvidenceId, EvidenceKind,
    EvidenceProvenance, EvidenceRecord, EvidenceStatus, Il, Interner, NodeId, NodeKind, Payload,
    SequenceSurfaceKind, Symbol,
};
use nose_semantics::{
    binding_write_target, language_core_evidence_provenance, language_core_record_has_provenance,
    opaque_argument_escape_args, receiver_mutation_call_receiver,
};
use rustc_hash::{FxHashMap, FxHashSet};

#[derive(Clone, Copy)]
struct BindingAssignment {
    assign: NodeId,
    lhs: NodeId,
    rhs: NodeId,
    name: Symbol,
    control_gated: bool,
}

/// Emit builtin language-core domain evidence for immutable local/module bindings.
///
/// This runs after desugaring and before alpha-renaming: the IL has canonical
/// surface shapes and evidence records, but binding names are still available
/// for stable `EvidenceAnchor::Binding` local hashes.
pub(crate) fn run(il: &mut Il, interner: &Interner) {
    let root = il.root;
    record_scope_bindings(il, interner, root);
}

fn record_scope_bindings(il: &mut Il, interner: &Interner, scope: NodeId) {
    let mut assignments = Vec::new();
    let mut nested_scopes = Vec::new();
    collect_scope_assignments(
        il,
        scope,
        scope,
        false,
        &mut assignments,
        &mut nested_scopes,
    );
    record_assignments_in_scope(il, interner, scope, &assignments);
    for nested in nested_scopes {
        record_scope_bindings(il, interner, nested);
    }
}

fn collect_scope_assignments(
    il: &Il,
    scope: NodeId,
    node: NodeId,
    control_gated: bool,
    assignments: &mut Vec<BindingAssignment>,
    nested_scopes: &mut Vec<NodeId>,
) {
    if node != scope && matches!(il.kind(node), NodeKind::Func | NodeKind::Lambda) {
        nested_scopes.push(node);
        return;
    }

    let now_control_gated =
        control_gated || matches!(il.kind(node), NodeKind::If | NodeKind::Loop | NodeKind::Try);
    if il.kind(node) == NodeKind::Assign {
        let kids = il.children(node);
        if kids.len() == 2 {
            if let Some(name) = binding_lhs_name(il, kids[0]) {
                assignments.push(BindingAssignment {
                    assign: node,
                    lhs: kids[0],
                    rhs: kids[1],
                    name,
                    control_gated,
                });
            }
        }
    }

    for &child in il.children(node) {
        collect_scope_assignments(
            il,
            scope,
            child,
            now_control_gated,
            assignments,
            nested_scopes,
        );
    }
}

fn record_assignments_in_scope(
    il: &mut Il,
    interner: &Interner,
    scope: NodeId,
    assignments: &[BindingAssignment],
) {
    let mut counts: FxHashMap<Symbol, usize> = FxHashMap::default();
    for assignment in assignments {
        *counts.entry(assignment.name).or_insert(0) += 1;
    }

    let unit_names: FxHashSet<Symbol> = if il.kind(scope) == NodeKind::Module {
        il.units.iter().filter_map(|unit| unit.name).collect()
    } else {
        FxHashSet::default()
    };

    let mut env: FxHashMap<Symbol, (DomainEvidence, EvidenceId)> = FxHashMap::default();
    let mut ordered = assignments.to_vec();
    ordered.sort_by_key(|assignment| {
        (
            il.node(assignment.assign).span.start_byte,
            il.node(assignment.assign).span.end_byte,
        )
    });

    let mutation_facts = ScopeMutationFacts::collect(il, interner, scope);
    let provenance = binding_domain_provenance(il);
    for assignment in ordered {
        if assignment.control_gated {
            continue;
        }
        if counts.get(&assignment.name).copied().unwrap_or(0) != 1 {
            continue;
        }
        if unit_names.contains(&assignment.name) {
            continue;
        }
        if node_contains_name(il, assignment.rhs, assignment.name) {
            continue;
        }
        if mutation_facts.binding_mutated(assignment) {
            continue;
        }

        let Some((domain, dependencies)) = rhs_domain_evidence(il, assignment.rhs, &env) else {
            continue;
        };
        let local_hash = stable_symbol_hash(interner.resolve(assignment.name));
        let Some(id) = find_or_push_evidence(
            il,
            EvidenceAnchor::binding(il.node(assignment.lhs).span, local_hash),
            EvidenceKind::Domain(domain),
            provenance,
            dependencies,
        ) else {
            continue;
        };
        env.insert(assignment.name, (domain, id));
    }
}

fn binding_lhs_name(il: &Il, lhs: NodeId) -> Option<Symbol> {
    if il.kind(lhs) != NodeKind::Var {
        return None;
    }
    match il.node(lhs).payload {
        Payload::Name(name) => Some(name),
        Payload::Cid(cid) => il.cid_names.get(cid as usize).copied(),
        _ => None,
    }
}

fn rhs_domain_evidence(
    il: &Il,
    rhs: NodeId,
    env: &FxHashMap<Symbol, (DomainEvidence, EvidenceId)>,
) -> Option<(DomainEvidence, Vec<EvidenceId>)> {
    if let Some((domain, id)) = domain_evidence_record_for_node(il, rhs) {
        return Some((domain, vec![id]));
    }
    if let Some((domain, id)) = sequence_domain_evidence_record_for_node(il, rhs) {
        return Some((domain, vec![id]));
    }
    if il.kind(rhs) == NodeKind::Var {
        if let Some(name) = binding_lhs_name(il, rhs) {
            if let Some(&(domain, id)) = env.get(&name) {
                return Some((domain, vec![id]));
            }
        }
    }
    None
}

fn domain_evidence_record_for_node(il: &Il, node: NodeId) -> Option<(DomainEvidence, EvidenceId)> {
    let expected = EvidenceAnchor::node(il.node(node).span, il.kind(node));
    let mut found = None;
    for record in il.evidence_anchored_at(expected.span()) {
        if record.anchor != expected {
            continue;
        }
        let EvidenceKind::Domain(domain) = record.kind else {
            continue;
        };
        if !record_is_live(il, record) {
            return None;
        }
        match found {
            None => found = Some((domain, record.id)),
            Some((existing, _)) if existing == domain => {}
            Some(_) => return None,
        }
    }
    found
}

fn sequence_domain_evidence_record_for_node(
    il: &Il,
    node: NodeId,
) -> Option<(DomainEvidence, EvidenceId)> {
    if il.kind(node) != NodeKind::Seq {
        return None;
    }
    let span = il.node(node).span;
    let mut found = None;
    for record in il.evidence_anchored_at(span) {
        if !matches!(record.anchor, EvidenceAnchor::Sequence { span: anchor_span } if anchor_span == span)
        {
            continue;
        }
        let EvidenceKind::SequenceSurface(surface) = record.kind else {
            continue;
        };
        if !language_core_record_has_provenance(il, record) {
            continue;
        }
        if !record_is_live(il, record) {
            return None;
        }
        match found {
            None => found = Some((surface, record.id)),
            Some((existing, _)) if existing == surface => {}
            Some(_) => return None,
        }
    }
    let (surface, id) = found?;
    let domain = match surface {
        SequenceSurfaceKind::Collection => DomainEvidence::Collection,
        SequenceSurfaceKind::Map => DomainEvidence::Map,
        _ => return None,
    };
    Some((domain, id))
}

fn record_is_live(il: &Il, record: &EvidenceRecord) -> bool {
    record.status == EvidenceStatus::Asserted && il.evidence_dependencies_asserted(record)
}

/// Per-scope mutation facts, collected in ONE walk and queried per binding.
///
/// Replaces the per-binding `visit_scope_nodes` walk (which was
/// O(assignments × scope size) — quadratic on module scopes full of
/// assignments, e.g. data-table files). Semantics are identical to the old
/// walk: a site only counts against a name when no nested `Func`/`Lambda`
/// between the scope root and the site *binds* that name (shadowing), and a
/// write-target site never counts against the binding assignment itself. The
/// old code's separate first pass over the scope's own assignment list was
/// subsumed by the walk (same `binding_write_target`/`node_contains_name`
/// check on the same nodes), so it has no counterpart here.
struct ScopeMutationFacts {
    /// Names mutated independent of any assignment: a mutating method call's
    /// `Var` receiver, or any `Var` inside an opaque-escape argument.
    mutated: FxHashSet<Symbol>,
    /// Names contained in an `Assign` write target → the assign sites that
    /// contain them (so the binding's own assignment can be excluded).
    write_sites: FxHashMap<Symbol, Vec<NodeId>>,
}

impl ScopeMutationFacts {
    fn collect(il: &Il, interner: &Interner, scope: NodeId) -> Self {
        let mut facts = ScopeMutationFacts {
            mutated: FxHashSet::default(),
            write_sites: FxHashMap::default(),
        };
        // Bound-name sets of the nested scopes currently on the walk path; a
        // harvested name shadowed by any of them does not reach the facts.
        let mut shadow_stack: Vec<FxHashSet<Symbol>> = Vec::new();
        facts.go(il, interner, scope, scope, &mut shadow_stack);
        facts
    }

    fn go(
        &mut self,
        il: &Il,
        interner: &Interner,
        scope: NodeId,
        node: NodeId,
        shadow_stack: &mut Vec<FxHashSet<Symbol>>,
    ) {
        match il.kind(node) {
            NodeKind::Call => {
                if let Some(receiver) = receiver_mutation_call_receiver(il, interner, node) {
                    if let Some(name) = var_name(il, receiver) {
                        self.record_mutated(name, shadow_stack);
                    }
                }
                if let Some(args) = opaque_argument_escape_args(il, node) {
                    for &arg in args {
                        self.record_subtree_vars(il, arg, shadow_stack, None);
                    }
                }
            }
            NodeKind::Assign => {
                if let Some(lhs) = binding_write_target(il, node) {
                    self.record_subtree_vars(il, lhs, shadow_stack, Some(node));
                }
            }
            _ => {}
        }
        if node != scope && matches!(il.kind(node), NodeKind::Func | NodeKind::Lambda) {
            shadow_stack.push(scope_bound_names(il, node));
            for &child in il.children(node) {
                self.go(il, interner, scope, child, shadow_stack);
            }
            shadow_stack.pop();
        } else {
            for &child in il.children(node) {
                self.go(il, interner, scope, child, shadow_stack);
            }
        }
    }

    fn record_mutated(&mut self, name: Symbol, shadow_stack: &[FxHashSet<Symbol>]) {
        if !shadow_stack.iter().any(|bound| bound.contains(&name)) {
            self.mutated.insert(name);
        }
    }

    /// Harvest every `Var` name in `node`'s subtree — the inverted form of
    /// `node_contains_name`. With `write_site`, names go to `write_sites`
    /// (assignment-excludable); without, straight to `mutated`.
    fn record_subtree_vars(
        &mut self,
        il: &Il,
        node: NodeId,
        shadow_stack: &[FxHashSet<Symbol>],
        write_site: Option<NodeId>,
    ) {
        if let Some(name) = var_name(il, node) {
            if !shadow_stack.iter().any(|bound| bound.contains(&name)) {
                match write_site {
                    Some(site) => self.write_sites.entry(name).or_default().push(site),
                    None => {
                        self.mutated.insert(name);
                    }
                }
            }
        }
        for &child in il.children(node) {
            self.record_subtree_vars(il, child, shadow_stack, write_site);
        }
    }

    fn binding_mutated(&self, binding: BindingAssignment) -> bool {
        self.mutated.contains(&binding.name)
            || self
                .write_sites
                .get(&binding.name)
                .is_some_and(|sites| sites.iter().any(|&site| site != binding.assign))
    }
}

fn var_name(il: &Il, node: NodeId) -> Option<Symbol> {
    (il.kind(node) == NodeKind::Var)
        .then(|| binding_node_name(il, node))
        .flatten()
}

/// Every name `scope` binds at its own level: params, assignment targets, and
/// loop patterns, not descending into deeper nested scopes — the same walk the
/// old per-name `scope_binds_name` did, collected once.
fn scope_bound_names(il: &Il, scope: NodeId) -> FxHashSet<Symbol> {
    fn go(il: &Il, scope: NodeId, node: NodeId, out: &mut FxHashSet<Symbol>) {
        if node != scope && matches!(il.kind(node), NodeKind::Func | NodeKind::Lambda) {
            return;
        }
        match il.kind(node) {
            NodeKind::Param => {
                if let Some(name) = binding_node_name(il, node) {
                    out.insert(name);
                }
            }
            NodeKind::Assign => {
                if let Some(&lhs) = il.children(node).first() {
                    collect_target_names(il, lhs, out);
                }
            }
            NodeKind::Loop => {
                if let Some(&pattern) = il.children(node).first() {
                    collect_target_names(il, pattern, out);
                }
            }
            _ => {}
        }
        for &child in il.children(node) {
            go(il, scope, child, out);
        }
    }
    let mut out = FxHashSet::default();
    go(il, scope, scope, &mut out);
    out
}

fn collect_target_names(il: &Il, node: NodeId, out: &mut FxHashSet<Symbol>) {
    match il.kind(node) {
        NodeKind::Var => {
            if let Some(name) = binding_node_name(il, node) {
                out.insert(name);
            }
        }
        NodeKind::Seq => {
            for &child in il.children(node) {
                collect_target_names(il, child, out);
            }
        }
        _ => {}
    }
}

fn node_refers_to_name(il: &Il, node: NodeId, name: Symbol) -> bool {
    il.kind(node) == NodeKind::Var && binding_node_name(il, node) == Some(name)
}

fn node_contains_name(il: &Il, node: NodeId, name: Symbol) -> bool {
    node_refers_to_name(il, node, name)
        || il
            .children(node)
            .iter()
            .any(|&child| node_contains_name(il, child, name))
}

fn find_or_push_evidence(
    il: &mut Il,
    anchor: EvidenceAnchor,
    kind: EvidenceKind,
    provenance: EvidenceProvenance,
    dependencies: Vec<EvidenceId>,
) -> Option<EvidenceId> {
    Some(il.find_or_push_builtin_evidence_with_provenance(anchor, kind, provenance, dependencies))
}

fn binding_domain_provenance(il: &Il) -> EvidenceProvenance {
    let (pack_id, producer_id) = language_core_evidence_provenance(il.meta.lang);
    EvidenceProvenance {
        emitter: EvidenceEmitter::Builtin,
        pack_hash: Some(stable_symbol_hash(pack_id)),
        rule_hash: Some(stable_symbol_hash(producer_id)),
    }
}

fn binding_node_name(il: &Il, node: NodeId) -> Option<Symbol> {
    match (il.kind(node), il.node(node).payload) {
        (NodeKind::Var | NodeKind::Param, Payload::Name(name)) => Some(name),
        (NodeKind::Var | NodeKind::Param, Payload::Cid(cid)) => {
            il.cid_names.get(cid as usize).copied()
        }
        _ => None,
    }
}

#[cfg(test)]
mod tests;
