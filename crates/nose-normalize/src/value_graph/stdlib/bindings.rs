use super::super::*;

impl<'a> Builder<'a> {
    pub(in crate::value_graph) fn proven_local_collection_binding_value(
        &mut self,
        expr: NodeId,
        env: &FxHashMap<u32, ValueId>,
    ) -> Option<ValueId> {
        let value = self.proven_local_binding_initializer_value(expr, env, |domain| {
            domain.is_collection_or_set()
        })?;
        self.proven_collection_value(value)
    }
    pub(super) fn proven_local_map_binding_value(
        &mut self,
        expr: NodeId,
        env: &FxHashMap<u32, ValueId>,
    ) -> Option<ValueId> {
        let value =
            self.proven_local_binding_initializer_value(expr, env, |domain| domain.is_map())?;
        self.proven_map_value(value)
    }
    fn proven_local_binding_initializer_value(
        &mut self,
        expr: NodeId,
        env: &FxHashMap<u32, ValueId>,
        accepts_domain: impl FnOnce(DomainEvidence) -> bool,
    ) -> Option<ValueId> {
        if self.il.kind(expr) != NodeKind::Var {
            return None;
        }
        let (lhs, rhs, self_referential) = match self.il.node(expr).payload {
            Payload::Cid(cid) => {
                let (lhs, rhs) = self.local_binding_initializer(cid, expr)?;
                (lhs, rhs, self.node_contains_cid(rhs, cid))
            }
            Payload::Name(name) => {
                if self.module_binding_mutated(name) {
                    return None;
                }
                let (lhs, rhs) = self.module_binding_initializer(name, expr)?;
                (lhs, rhs, self.node_contains_binding_name(rhs, name))
            }
            _ => return None,
        };
        if !nose_semantics::domain_evidence_for_binding_lhs(self.il, self.interner, lhs)
            .is_some_and(accepts_domain)
        {
            return None;
        }
        if self_referential {
            return None;
        }
        Some(self.eval(rhs, env))
    }
    fn local_binding_initializer(&self, cid: u32, use_node: NodeId) -> Option<(NodeId, NodeId)> {
        let mut rhs = None;
        for (idx, node) in self.il.nodes.iter().enumerate() {
            if node.kind != NodeKind::Assign {
                continue;
            }
            let assign = NodeId(idx as u32);
            let kids = self.il.children(assign);
            if kids.len() != 2 {
                continue;
            }
            if self.node_refers_to_cid(kids[0], cid) {
                if node.span.end_byte > self.il.node(use_node).span.start_byte {
                    continue;
                }
                if rhs.is_some() {
                    return None;
                }
                rhs = Some((kids[0], kids[1]));
            } else if self.node_contains_cid(kids[0], cid) {
                return None;
            }
        }
        rhs
    }
    fn module_binding_initializer(
        &self,
        name: Symbol,
        use_node: NodeId,
    ) -> Option<(NodeId, NodeId)> {
        let local_hash = stable_symbol_hash(self.interner.resolve(name));
        let mut found = None;
        for assign in top_level_statements_for(self.il) {
            let Some((lhs, rhs)) = self.il.assignment_var_parts(assign) else {
                continue;
            };
            if self.il.node(assign).span.end_byte > self.il.node(use_node).span.start_byte {
                continue;
            }
            if !self.binding_lhs_has_domain_hash(lhs, local_hash) {
                continue;
            }
            if found.is_some() {
                return None;
            }
            found = Some((lhs, rhs));
        }
        found
    }
    fn binding_lhs_has_domain_hash(&self, lhs: NodeId, local_hash: u64) -> bool {
        let span = self.il.node(lhs).span;
        self.il.evidence_anchored_at(span).any(|record| {
            matches!(
                record.anchor,
                EvidenceAnchor::Binding {
                    span: anchor_span,
                    local_hash: anchor_hash,
                } if anchor_span == span && anchor_hash == local_hash
            ) && matches!(record.kind, EvidenceKind::Domain(_))
                && record.status == EvidenceStatus::Asserted
                && self.il.evidence_dependencies_asserted(record)
        })
    }
    fn node_contains_binding_name(&self, node: NodeId, name: Symbol) -> bool {
        self.il.var_binding_name(node) == Some(name)
            || self
                .il
                .children(node)
                .iter()
                .any(|&child| self.node_contains_binding_name(child, name))
    }
}
