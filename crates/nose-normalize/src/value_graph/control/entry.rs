use super::super::*;

impl<'a> Builder<'a> {
    /// Build the value graph for a `Func`/`Method`/class unit. The unit root may
    /// be a `Func` (params + body) or a `Block` (class body of methods); for a
    /// `Block` we process its statements directly.
    pub(in crate::value_graph) fn build_unit(&mut self, root: NodeId) {
        self.build_unit_with_context(root, None);
    }

    pub(in crate::value_graph) fn build_unit_with_context(
        &mut self,
        root: NodeId,
        context: Option<&'a ValueFingerprintContext>,
    ) {
        let mut timer = ValueGraphBuildTimer::new(self.il, root);
        self.param_domain.clear();
        self.seed_param_domains(root);
        self.seed_param_value_domains(root);
        timer.mark_seed();
        self.seed_immutable_bindings(root, context);
        timer.mark_immutable();
        match context {
            Some(context) => {
                self.adopt_inline_candidates(
                    root,
                    Cow::Borrowed(context.inline_candidates(
                        self.il,
                        self.interner,
                        self.await_transparent,
                    )),
                );
            }
            None => {
                let candidates = self.collect_inline_candidates();
                self.adopt_inline_candidates(root, Cow::Owned(candidates));
            }
        }
        timer.mark_inline();
        let mut env: FxHashMap<u32, ValueId> = FxHashMap::default();
        match self.il.kind(root) {
            NodeKind::Func => {
                // Seed parameters as inputs *by position*, so duplicate-named params
                // (which alpha-rename collapses to one cid) stay distinct values — the
                // accessible one wins, as at runtime. For well-formed code param cid ==
                // position, so this is identical to keying by cid.
                let kids = self.il.children(root).to_vec();
                let mut pos = 0u32;
                for &k in &kids {
                    if self.il.kind(k) == NodeKind::Param {
                        if let Payload::Cid(c) = self.il.node(k).payload {
                            let v = self.mk(ValOp::Input(pos), vec![]);
                            env.insert(c, v);
                            pos += 1;
                        }
                    }
                }
                if let Some(&body) = kids.last() {
                    self.process_stmt(body, &mut env);
                }
                self.recognize_value_default_returns();
                self.recognize_existence_reduction();
            }
            NodeKind::Module | NodeKind::Block => {
                // Class/other container unit. Two things make its data visible:
                //  (1) attribute assignments (`name = value`) land in `env` but reach no
                //      sink — a class's attributes ARE its data, so expose them (two
                //      locale-table classes that differ only in values must differ);
                //  (2) a container's *behavior* is the aggregate of its methods. Plain
                //      `process_stmt` has no `Func` case, so a method definition fell to
                //      the opaque-effect branch and the class collapsed to a near-empty
                //      structural shell — a one-operator change deep inside a method left
                //      the class fingerprint identical, so two classes were "behavioral
                //      clones" on structure alone. Descend into each contained method and
                //      fold its returns/effects into the container, so the class differs
                //      exactly when its methods do.
                self.process_container(root, &mut env);
                let mut vals: Vec<ValueId> = env.values().copied().collect();
                vals.sort_unstable();
                vals.dedup();
                for v in vals {
                    self.sinks.push(Sink::new(SinkKind::Effect, v));
                }
            }
            _ => {
                self.process_stmt(root, &mut env);
            }
        }
        timer.mark_process();
        self.flush_fields();
        self.index_env.clear();
        timer.mark_finish(self.nodes.len(), self.sinks.len());
    }
}
