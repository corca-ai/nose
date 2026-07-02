use super::*;

pub(crate) struct StrictFacts<'a> {
    immutable_names: FxHashSet<Symbol>,
    function_roots: FxHashSet<NodeId>,
    function_spans: FxHashSet<Span>,
    decorated_definition_spans: FxHashSet<Span>,
    receiver_domains: ReceiverDomainEvidenceIndex<'a>,
}

impl<'a> StrictFacts<'a> {
    pub(crate) fn collect(il: &'a Il, interner: &'a Interner) -> Self {
        let mut facts = StrictFacts {
            immutable_names: FxHashSet::default(),
            function_roots: FxHashSet::default(),
            function_spans: FxHashSet::default(),
            decorated_definition_spans: FxHashSet::default(),
            receiver_domains: ReceiverDomainEvidenceIndex::new(il, interner),
        };
        facts.collect_decorated_definition_spans(il);
        facts.collect_immutable_bindings(il, interner);
        facts.collect_function_bindings(il, interner);
        facts
    }

    pub(super) fn exact_value_name(&self, name: Symbol) -> bool {
        self.immutable_names.contains(&name)
    }

    pub(super) fn direct_function_target_at_call(
        &self,
        il: &Il,
        interner: &Interner,
        call: NodeId,
    ) -> bool {
        direct_function_call_target_span_at_call(il, interner, call)
            .is_some_and(|span| self.function_spans.contains(&span))
    }

    pub(super) fn direct_method_target_at_call(
        &self,
        il: &Il,
        interner: &Interner,
        call: NodeId,
    ) -> bool {
        direct_method_call_target_span_at_call(il, interner, call)
            .is_some_and(|span| self.function_spans.contains(&span))
    }

    pub(super) fn receiver_satisfies_domain(
        &self,
        receiver: NodeId,
        requirement: DomainRequirement,
    ) -> bool {
        self.receiver_domains
            .receiver_satisfies_domain(receiver, requirement)
    }

    pub(super) fn decorated_definition_span(&self, span: Span) -> bool {
        self.decorated_definition_spans.contains(&span)
    }

    fn collect_decorated_definition_spans(&mut self, il: &Il) {
        for record in &il.evidence {
            if !matches!(
                record.kind,
                EvidenceKind::Source(SourceFactKind::Binding(
                    SourceBindingKind::DecoratedDefinition
                ))
            ) {
                continue;
            }
            if let EvidenceAnchor::SourceSpan(span) = record.anchor {
                self.decorated_definition_spans.insert(span);
            }
        }
    }

    fn collect_immutable_bindings(&mut self, il: &Il, interner: &Interner) {
        let top_level = top_level_statements(il);
        let mut is_top_level = vec![false; il.nodes.len()];
        for &stmt in &top_level {
            if let Some(slot) = is_top_level.get_mut(stmt.0 as usize) {
                *slot = true;
            }
        }

        let mut counts: FxHashMap<Symbol, usize> = FxHashMap::default();
        for &stmt in &top_level {
            let Some(name) = assignment_name(il, stmt) else {
                continue;
            };
            *counts.entry(name).or_insert(0) += 1;
        }
        let candidate_names: FxHashSet<Symbol> = counts
            .iter()
            .filter_map(|(&name, &count)| (count == 1).then_some(name))
            .collect();
        let mutated_bindings =
            collect_module_mutations(il, interner, &candidate_names, &is_top_level);

        let mut env: FxHashSet<u32> = FxHashSet::default();
        for &stmt in &top_level {
            let kids = il.children(stmt);
            if kids.len() != 2 {
                continue;
            }
            let Some(name) = assignment_name(il, stmt) else {
                continue;
            };
            if counts.get(&name).copied().unwrap_or(0) != 1 {
                continue;
            }
            if mutated_bindings.contains(&name) {
                continue;
            }
            let safe_literal = immutable_binding_safe(il, &env, &self.immutable_names, kids[1]);
            if safe_literal {
                self.immutable_names.insert(name);
                if let Payload::Cid(cid) = il.node(kids[0]).payload {
                    env.insert(cid);
                }
            }
        }
    }

    fn collect_function_bindings(&mut self, il: &Il, interner: &Interner) {
        for unit in &il.units {
            if il.kind(unit.root) != NodeKind::Func {
                continue;
            }
            if function_binding_safe(il, interner, self, unit.root, unit.root) {
                self.function_roots.insert(unit.root);
                self.function_spans.insert(il.node(unit.root).span);
            }
        }
    }
}

fn top_level_statements(il: &Il) -> Vec<NodeId> {
    let mut out = Vec::new();
    for &stmt in il.children(il.root) {
        if il.kind(stmt) == NodeKind::Block {
            out.extend(il.children(stmt).iter().copied());
        } else {
            out.push(stmt);
        }
    }
    out
}

fn assignment_name(il: &Il, stmt: NodeId) -> Option<Symbol> {
    let (lhs, _) = il.assignment_var_parts(stmt)?;
    let cid = il.var_cid(lhs)?;
    il.cid_names.get(cid as usize).copied()
}

fn immutable_binding_safe(
    il: &Il,
    env: &FxHashSet<u32>,
    immutable_names: &FxHashSet<Symbol>,
    node: NodeId,
) -> bool {
    match il.kind(node) {
        NodeKind::Raw
        | NodeKind::Call
        | NodeKind::HoF
        | NodeKind::Func
        | NodeKind::Lambda
        | NodeKind::Loop
        | NodeKind::Try
        | NodeKind::Throw
        | NodeKind::Assign => false,
        NodeKind::Var => match il.node(node).payload {
            Payload::Cid(c) => env.contains(&c),
            Payload::Name(s) => immutable_names.contains(&s),
            _ => false,
        },
        NodeKind::Lit => exact_literal_safe(il, node),
        _ => il
            .children(node)
            .iter()
            .all(|&c| immutable_binding_safe(il, env, immutable_names, c)),
    }
}
