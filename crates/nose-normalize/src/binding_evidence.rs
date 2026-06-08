use nose_il::{
    stable_symbol_hash, Builtin, DomainEvidence, EvidenceAnchor, EvidenceId, EvidenceKind,
    EvidenceRecord, EvidenceStatus, Il, Interner, NodeId, NodeKind, Payload, SequenceSurfaceKind,
    Symbol,
};
use nose_semantics::{module_binding_mutating_method_name, FIRST_PARTY_PACK_ID};
use rustc_hash::{FxHashMap, FxHashSet};

#[derive(Clone, Copy)]
struct BindingAssignment {
    assign: NodeId,
    lhs: NodeId,
    rhs: NodeId,
    name: Symbol,
    control_gated: bool,
}

/// Emit first-party domain evidence for immutable local/module bindings.
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
        if binding_mutated_in_scope(il, interner, scope, assignments, assignment) {
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
            "immutable_binding_domain",
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
    for record in &il.evidence {
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
    for record in &il.evidence {
        if !matches!(record.anchor, EvidenceAnchor::Sequence { span: anchor_span } if anchor_span == span)
        {
            continue;
        }
        let EvidenceKind::SequenceSurface(surface) = record.kind else {
            continue;
        };
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

fn binding_mutated_in_scope(
    il: &Il,
    interner: &Interner,
    scope: NodeId,
    assignments: &[BindingAssignment],
    binding: BindingAssignment,
) -> bool {
    for assignment in assignments {
        if assignment.assign != binding.assign
            && node_contains_name(il, assignment.lhs, binding.name)
        {
            return true;
        }
    }

    let mut mutated = false;
    visit_scope_nodes(il, scope, binding.name, |node| {
        if mutated {
            return;
        }
        match il.kind(node) {
            NodeKind::Call
                if matches!(il.node(node).payload, Payload::Builtin(Builtin::Append)) =>
            {
                if let Some(&receiver) = il.children(node).first() {
                    mutated = node_refers_to_name(il, receiver, binding.name);
                }
            }
            NodeKind::Field => {
                let Payload::Name(method) = il.node(node).payload else {
                    return;
                };
                if !module_binding_mutating_method_name(interner.resolve(method)) {
                    return;
                }
                if let Some(&receiver) = il.children(node).first() {
                    mutated = node_refers_to_name(il, receiver, binding.name);
                }
            }
            NodeKind::Assign if node != binding.assign => {
                if let Some(&lhs) = il.children(node).first() {
                    mutated = node_contains_name(il, lhs, binding.name);
                }
            }
            _ => {}
        }
    });
    mutated
}

fn visit_scope_nodes(il: &Il, scope: NodeId, binding_name: Symbol, mut visit: impl FnMut(NodeId)) {
    fn go(
        il: &Il,
        scope: NodeId,
        binding_name: Symbol,
        node: NodeId,
        visit: &mut impl FnMut(NodeId),
    ) {
        visit(node);
        if node != scope
            && matches!(il.kind(node), NodeKind::Func | NodeKind::Lambda)
            && scope_binds_name(il, node, binding_name)
        {
            return;
        }
        for &child in il.children(node) {
            go(il, scope, binding_name, child, visit);
        }
    }
    go(il, scope, binding_name, scope, &mut visit);
}

fn scope_binds_name(il: &Il, scope: NodeId, name: Symbol) -> bool {
    fn go(il: &Il, scope: NodeId, node: NodeId, name: Symbol) -> bool {
        if node != scope && matches!(il.kind(node), NodeKind::Func | NodeKind::Lambda) {
            return false;
        }
        match il.kind(node) {
            NodeKind::Param if binding_node_name(il, node) == Some(name) => return true,
            NodeKind::Assign => {
                if let Some(&lhs) = il.children(node).first() {
                    if target_binds_name(il, lhs, name) {
                        return true;
                    }
                }
            }
            NodeKind::Loop => {
                if let Some(&pattern) = il.children(node).first() {
                    if target_binds_name(il, pattern, name) {
                        return true;
                    }
                }
            }
            _ => {}
        }
        il.children(node)
            .iter()
            .any(|&child| go(il, scope, child, name))
    }
    go(il, scope, scope, name)
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

fn target_binds_name(il: &Il, node: NodeId, name: Symbol) -> bool {
    match il.kind(node) {
        NodeKind::Var => binding_node_name(il, node) == Some(name),
        NodeKind::Seq => il
            .children(node)
            .iter()
            .any(|&child| target_binds_name(il, child, name)),
        _ => false,
    }
}

fn find_or_push_evidence(
    il: &mut Il,
    anchor: EvidenceAnchor,
    kind: EvidenceKind,
    rule: &str,
    dependencies: Vec<EvidenceId>,
) -> Option<EvidenceId> {
    Some(il.find_or_push_first_party_evidence(
        anchor,
        kind,
        FIRST_PARTY_PACK_ID,
        rule,
        dependencies,
    ))
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
mod tests {
    use super::*;
    use nose_il::{
        EvidenceEmitter, EvidenceProvenance, FileId, FileMeta, IlBuilder, Lang,
        SequenceSurfaceKind, Span, Unit, UnitKind,
    };

    fn sp(line: u32) -> Span {
        Span::new(FileId(0), line, line, line, line)
    }

    fn finish(builder: IlBuilder, root: NodeId, lang: Lang) -> Il {
        builder.finish(
            root,
            FileMeta {
                path: "t".into(),
                lang,
            },
            vec![Unit {
                root,
                kind: UnitKind::Function,
                name: None,
            }],
            Vec::new(),
        )
    }

    fn sequence_evidence(id: u32, span: Span, kind: SequenceSurfaceKind) -> EvidenceRecord {
        EvidenceRecord {
            id: EvidenceId(id),
            anchor: EvidenceAnchor::sequence(span),
            kind: EvidenceKind::SequenceSurface(kind),
            provenance: EvidenceProvenance {
                emitter: EvidenceEmitter::FirstParty,
                pack_hash: Some(stable_symbol_hash(FIRST_PARTY_PACK_ID)),
                rule_hash: Some(stable_symbol_hash("test")),
            },
            dependencies: Vec::new(),
            status: EvidenceStatus::Asserted,
        }
    }

    fn binding_domain_record<'a>(il: &'a Il, name: &str) -> Option<&'a EvidenceRecord> {
        let local_hash = stable_symbol_hash(name);
        il.evidence.iter().find(|record| {
            matches!(
                record.anchor,
                EvidenceAnchor::Binding {
                    local_hash: anchor_hash,
                    ..
                } if anchor_hash == local_hash
            ) && matches!(record.kind, EvidenceKind::Domain(_))
        })
    }

    fn array_assignment(
        b: &mut IlBuilder,
        interner: &Interner,
        name: Symbol,
        assign_span: Span,
        seq_span: Span,
    ) -> (NodeId, NodeId) {
        let lhs = b.add(NodeKind::Var, Payload::Name(name), assign_span, &[]);
        let seq = b.add(
            NodeKind::Seq,
            Payload::Name(interner.intern("array")),
            seq_span,
            &[],
        );
        let assign = b.add(NodeKind::Assign, Payload::None, assign_span, &[lhs, seq]);
        (assign, seq)
    }

    fn append_call(b: &mut IlBuilder, name: Symbol, span: Span) -> NodeId {
        let receiver = b.add(NodeKind::Var, Payload::Name(name), span, &[]);
        let item = b.add(NodeKind::Lit, Payload::LitInt(1), span, &[]);
        b.add(
            NodeKind::Call,
            Payload::Builtin(Builtin::Append),
            span,
            &[receiver, item],
        )
    }

    fn finish_with_sequence_evidence(b: IlBuilder, root: NodeId) -> Il {
        let mut il = finish(b, root, Lang::TypeScript);
        il.evidence
            .push(sequence_evidence(0, sp(2), SequenceSurfaceKind::Collection));
        il
    }

    #[derive(Clone, Copy)]
    enum MutationCase {
        Direct,
        NestedModule,
        NestedLocal,
    }

    fn mutation_case_il(interner: &Interner, case: MutationCase) -> Il {
        let xs = interner.intern("xs");
        let mut b = IlBuilder::new(FileId(0));
        let (assign, _) = array_assignment(&mut b, interner, xs, sp(1), sp(2));
        let append_span = match case {
            MutationCase::Direct => sp(3),
            MutationCase::NestedModule => sp(4),
            MutationCase::NestedLocal => sp(5),
        };
        let append = append_call(&mut b, xs, append_span);
        let root = match case {
            MutationCase::Direct => b.add(NodeKind::Func, Payload::None, sp(1), &[assign, append]),
            MutationCase::NestedModule => {
                let body = b.add(NodeKind::Block, Payload::None, sp(4), &[append]);
                let nested = b.add(NodeKind::Func, Payload::None, sp(3), &[body]);
                b.add(NodeKind::Module, Payload::None, sp(1), &[assign, nested])
            }
            MutationCase::NestedLocal => {
                let nested_body = b.add(NodeKind::Block, Payload::None, sp(5), &[append]);
                let nested = b.add(NodeKind::Func, Payload::None, sp(4), &[nested_body]);
                b.add(NodeKind::Func, Payload::None, sp(1), &[assign, nested])
            }
        };
        finish_with_sequence_evidence(b, root)
    }

    #[test]
    fn records_binding_domain_from_sequence_surface_evidence() {
        let interner = Interner::new();
        let xs = interner.intern("xs");
        let mut b = IlBuilder::new(FileId(0));
        let (assign, _) = array_assignment(&mut b, &interner, xs, sp(1), sp(2));
        let root = b.add(NodeKind::Func, Payload::None, sp(1), &[assign]);
        let mut il = finish_with_sequence_evidence(b, root);

        run(&mut il, &interner);

        let record = binding_domain_record(&il, "xs").expect("binding domain evidence");
        assert!(matches!(
            record.kind,
            EvidenceKind::Domain(DomainEvidence::Collection)
        ));
        assert_eq!(record.dependencies, vec![EvidenceId(0)]);
    }

    #[test]
    fn binding_domain_chains_through_prior_immutable_binding() {
        let interner = Interner::new();
        let xs = interner.intern("xs");
        let ys = interner.intern("ys");
        let mut b = IlBuilder::new(FileId(0));
        let (xs_assign, _) = array_assignment(&mut b, &interner, xs, sp(1), sp(2));
        let ys_lhs = b.add(NodeKind::Var, Payload::Name(ys), sp(3), &[]);
        let xs_ref = b.add(NodeKind::Var, Payload::Name(xs), sp(4), &[]);
        let ys_assign = b.add(NodeKind::Assign, Payload::None, sp(3), &[ys_lhs, xs_ref]);
        let root = b.add(
            NodeKind::Func,
            Payload::None,
            sp(1),
            &[xs_assign, ys_assign],
        );
        let mut il = finish_with_sequence_evidence(b, root);

        run(&mut il, &interner);

        let xs_record = binding_domain_record(&il, "xs").expect("xs binding evidence");
        let ys_record = binding_domain_record(&il, "ys").expect("ys binding evidence");
        assert!(matches!(
            ys_record.kind,
            EvidenceKind::Domain(DomainEvidence::Collection)
        ));
        assert_eq!(ys_record.dependencies, vec![xs_record.id]);
    }

    #[test]
    fn mutations_block_binding_domain_evidence() {
        let interner = Interner::new();
        for case in [
            MutationCase::Direct,
            MutationCase::NestedModule,
            MutationCase::NestedLocal,
        ] {
            let mut il = mutation_case_il(&interner, case);
            run(&mut il, &interner);
            assert!(binding_domain_record(&il, "xs").is_none());
        }
    }
}
