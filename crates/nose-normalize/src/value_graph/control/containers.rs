use super::super::*;

impl<'a> Builder<'a> {
    /// Walk a container (class/module body), folding each contained method's behavior
    /// into the current sinks. A `Func` is processed in its own parameter scope (its
    /// returns/effects become the container's), so the container's fingerprint is the
    /// aggregate of its methods; `Block` wrappers are descended; anything else (field
    /// initializers, attribute assigns) is processed as a normal statement.
    pub(in crate::value_graph) fn process_container(
        &mut self,
        node: NodeId,
        env: &mut FxHashMap<u32, ValueId>,
    ) {
        for c in self.il.children(node).to_vec() {
            match self.il.kind(c) {
                NodeKind::Func => {
                    let kids = self.il.children(c).to_vec();
                    let mut menv = env.clone();
                    // The method shadows the container's param scope. MOVE the container maps
                    // out (leaving them empty, which is what the seed below expects) and move
                    // them back after — a behavior-identical save/restore that avoids cloning
                    // both maps per method. `seed_param_value_domains` reassigns `param_ty`
                    // wholesale, so its emptied start is equivalent to the prior un-cleared one.
                    let saved_param_domain = std::mem::take(&mut self.param_domain);
                    let saved_param_ty = std::mem::take(&mut self.param_ty);
                    self.seed_param_domains(c);
                    self.seed_param_value_domains(c);
                    let mut pos = 0u32;
                    for &k in &kids {
                        if self.il.kind(k) == NodeKind::Param {
                            if let Payload::Cid(cid) = self.il.node(k).payload {
                                let v = self.mk(ValOp::Input(pos), vec![]);
                                menv.insert(cid, v);
                                pos += 1;
                            }
                        }
                    }
                    if let Some(&body) = kids.last() {
                        self.process_stmt(body, &mut menv);
                    }
                    self.param_domain = saved_param_domain;
                    self.param_ty = saved_param_ty;
                }
                NodeKind::Block => self.process_container(c, env),
                NodeKind::Assign => self.process_container_assignment(c, env),
                _ => self.process_stmt(c, env),
            }
        }
    }

    pub(in crate::value_graph) fn process_container_assignment(
        &mut self,
        stmt: NodeId,
        env: &mut FxHashMap<u32, ValueId>,
    ) {
        let Some((lhs, rhs)) = self.il.assignment_var_parts(stmt) else {
            self.process_stmt(stmt, env);
            return;
        };
        let binding = match self.il.node(lhs).payload {
            Payload::Cid(cid) => self.il.cid_names.get(cid as usize).copied().map(|name| {
                self.container_binding_value(lhs, rhs, name, env)
                    .map(|value| ContainerBinding::Name(name, value))
                    .unwrap_or(ContainerBinding::Cid(cid, name))
            }),
            Payload::Name(name) => self
                .container_binding_value(lhs, rhs, name, env)
                .map(|value| ContainerBinding::Name(name, value)),
            _ => None,
        };
        match binding {
            Some(ContainerBinding::Name(name, value)) => {
                self.global_env.insert(name, value);
                return;
            }
            Some(ContainerBinding::Cid(cid, name)) => {
                self.process_stmt(stmt, env);
                if let Some(&value) = env.get(&cid) {
                    self.global_env.insert(name, value);
                }
                return;
            }
            None => {}
        }
        self.process_stmt(stmt, env);
    }

    fn container_binding_value(
        &mut self,
        lhs: NodeId,
        rhs: NodeId,
        name: Symbol,
        env: &FxHashMap<u32, ValueId>,
    ) -> Option<ValueId> {
        if self.module_binding_mutated(name) {
            return None;
        }
        let binding_domain =
            nose_semantics::domain_evidence_for_binding_lhs(self.il, self.interner, lhs)?;
        let value = self.eval(rhs, env);
        if self.immutable_binding_safe(rhs, env) {
            return Some(value);
        }
        if binding_domain.is_map() {
            return self.proven_map_value(value);
        }
        if binding_domain.is_collection_or_set() {
            return self.proven_collection_value(value);
        }
        None
    }

    pub(in crate::value_graph) fn compact_coupled_recurrence(
        &mut self,
        cid: u32,
        value: ValueId,
    ) -> ValueId {
        let should_compact = self.loop_recurrence.as_ref().is_some_and(|scope| {
            scope.loop_values.contains_key(&cid)
                && self.references_nonself_loop_dependency(value, cid, scope)
        });
        if !should_compact {
            return value;
        }

        let key = self
            .loop_recurrence
            .as_ref()
            .and_then(|scope| scope.loop_keys.get(&cid))
            .copied()
            .unwrap_or(cid);
        let h = combine(combine(0xC0AD_D1EC, key as u64), self.vhash[value as usize]);
        self.mk(ValOp::Recurrence(h), vec![])
    }

    pub(in crate::value_graph) fn references_nonself_loop_dependency(
        &self,
        value: ValueId,
        self_cid: u32,
        scope: &LoopRecurrenceScope,
    ) -> bool {
        let self_key = scope.loop_keys.get(&self_cid).copied();
        let mut stack = vec![value];
        let mut seen = FxHashSet::default();
        while let Some(v) = stack.pop() {
            if !seen.insert(v) {
                continue;
            }
            match &self.nodes[v as usize].op {
                ValOp::Loop(key) if Some(*key) != self_key && scope.loop_key_set.contains(key) => {
                    return true;
                }
                ValOp::Recurrence(_) => return true,
                _ => {}
            }
            stack.extend(self.nodes[v as usize].args.iter().copied());
        }
        false
    }
}

enum ContainerBinding {
    Cid(u32, Symbol),
    Name(Symbol, ValueId),
}
