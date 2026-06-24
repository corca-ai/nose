use super::super::*;

impl<'a> Builder<'a> {
    pub(in crate::value_graph) fn process_stmt(
        &mut self,
        stmt: NodeId,
        env: &mut FxHashMap<u32, ValueId>,
    ) {
        match self.il.kind(stmt) {
            NodeKind::Block => self.process_block(stmt, env),
            NodeKind::Assign => self.process_assign_stmt(stmt, env),
            NodeKind::Return => {
                // A bare `return;` (no value) is still behaviorally significant: as an
                // *early* exit inside a loop/branch it changes which later code runs, and
                // its path guard distinguishes a conditional early-return from an
                // unconditional one. Model it as a guarded void-return sink (previously a
                // valueless return pushed nothing, so two functions differing only in a
                // conditional early `return;` collapsed — cf. the break sink, §AS).
                let v = match self.il.children(stmt).first() {
                    Some(&e) => self.eval(e, env),
                    None => self.sentinel_const(sentinel::VOID_RETURN),
                };
                self.emit_return(v);
            }
            NodeKind::Throw => {
                let v = match self.il.children(stmt).first() {
                    Some(&e) => self.eval(e, env),
                    None => self.sentinel_const(sentinel::THROW),
                };
                self.emit_throw(v);
            }
            NodeKind::ExprStmt => {
                if let Some(&e) = self.il.children(stmt).first() {
                    if matches!(
                        self.il.kind(e),
                        NodeKind::Return | NodeKind::Throw | NodeKind::Break | NodeKind::Continue
                    ) {
                        self.process_stmt(e, env);
                        return;
                    }
                    // A `coll.append(x)` to an ACTIVE local builder var records the
                    // per-element contribution (so the loop becomes a `Map`) instead of an
                    // opaque effect. Anything irregular (2nd append, multi-arg) spoils it.
                    if self.try_record_append(e, env) {
                        return;
                    }
                    let v = self.eval(e, env);
                    // an expression statement is kept only if it has effect value
                    self.push_effect(v);
                }
            }
            NodeKind::If => self.process_if(stmt, env),
            NodeKind::Loop => self.process_loop(stmt, env),
            NodeKind::Try => {
                let kids = self.il.children(stmt).to_vec();
                if kids.len() == 2 && self.is_effect_free_static_err_body(kids[0], env) {
                    self.process_stmt(kids[1], env);
                    return;
                }
                if kids.len() == 2 && self.branch_returns(kids[0]) {
                    self.process_stmt(kids[0], env);
                    // Erasing the handler is sound ONLY when the body provably cannot
                    // raise (`try { return 1 }` — the pinned no-throw convention). A
                    // body that evaluates throw-capable expressions (`return x+1` can
                    // TypeError in Python) keeps its handler as the EXCEPTIONAL path:
                    // processed under a synthetic exception guard so its sinks join
                    // the multiset distinctly — `try {return x+1} except {return x}`
                    // no longer fingerprints as the bare `return x+1` (#210). The env
                    // clone keeps exceptional bindings out of the normal path.
                    if !self.branch_returns_throw_free(kids[0]) {
                        let exc = self.sentinel_const(sentinel::EXC);
                        self.path.push(exc);
                        let mut handler_env = env.clone();
                        self.process_stmt(kids[1], &mut handler_env);
                        self.path.pop();
                    }
                    return;
                }
                if kids.len() == 2 && self.is_effect_free_throw_body(kids[0]) {
                    self.process_stmt(kids[1], env);
                    return;
                }
                for c in kids {
                    self.process_stmt(c, env);
                }
            }
            NodeKind::Break => {
                // An early `break` truncates the loop to a prefix — the result is NOT
                // the full-iteration fold. Record the break's *path condition* as a sink
                // so an early-exit loop no longer fingerprints identically to one that
                // runs to completion, and two loops breaking on different conditions stay
                // distinct. (`continue` needs no handling: desugaring already hoists the
                // remainder of the body into the negated guard, so its filtering effect
                // is captured by the normal path-guard machinery.)
                let marker = self.sentinel_const(sentinel::BREAK);
                let g = self.guarded(marker);
                self.sinks.push(Sink::new(SinkKind::Break, g));
            }
            NodeKind::Continue => {}
            _ => {
                // unknown statement: evaluate as effect best-effort
                let v = self.eval(stmt, env);
                self.push_effect(v);
            }
        }
    }

    fn process_assign_stmt(&mut self, stmt: NodeId, env: &mut FxHashMap<u32, ValueId>) {
        let kids = self.il.children(stmt).to_vec();
        if kids.len() != 2 {
            return;
        }
        // Go-style functional append `r = append(r, item)` to an ACTIVE builder var
        // IS the per-element build (the reassignment is the append), exactly like
        // the effect-form `r.append(item)`. Record the contribution so the loop
        // becomes `Map(elem, contrib)` instead of an opaque reassign.
        if self.try_record_reassign_append(kids[0], kids[1], env) {
            return;
        }
        let rhs = self.eval(kids[1], env);
        if self.il.kind(kids[0]) == NodeKind::Var {
            if let Payload::Cid(c) = self.il.node(kids[0]).payload {
                let rhs = self.compact_coupled_recurrence(c, rhs);
                env.insert(c, rhs);
                return;
            }
        }
        // An evidence-backed exact field write updates per-place state
        // (last-write-wins), flushed as a (receiver, field, final-value)
        // sink later — order-insensitive across distinct places, correct
        // for same-place overwrites.
        if let Some(key) = self.exact_field_write_state_key(stmt, kids[0]) {
            // A field write is an observable effect a value-only inline
            // would drop — poison the active capture instead of recording.
            if let Some(frame) = self.inline_capture.last_mut() {
                frame.poisoned = true;
                return;
            }
            let g = self.guarded(rhs);
            self.field_env.insert(key, g);
            return;
        }
        // `d[k] = v` to an ACTIVE dict-builder records a `DictEntry` contribution
        // (so the loop becomes a `Map` of entries, converging with `{k:v for x}`).
        if self.try_record_index_assign(stmt, rhs, env) {
            return;
        }
        // store into an index / computed target → an ordered effect. Element WRITES stay
        // ORDERED effects (order-sensitive — sound even when two indices alias at runtime).
        // Additionally version the `(base, index)` place for READ-forwarding (#337): a later
        // `base[index]` read in the same straight-line run sees `rhs`, distinguishing `swap`
        // from `clobber`. The write TARGET is built directly from base+index (NOT via `eval`,
        // which would forward a prior write into the place itself).
        if self.il.kind(kids[0]) == NodeKind::Index {
            let ik = self.il.children(kids[0]).to_vec();
            if ik.len() == 2 {
                let base = self.eval(ik[0], env);
                let index = self.eval(ik[1], env);
                let place = self.mk(ValOp::Index, vec![base, index]);
                let st = self.mk(ValOp::Call(0), vec![place, rhs]);
                self.push_effect(st);
                self.record_index_write(base, index, rhs);
                return;
            }
        }
        let target = self.eval(kids[0], env);
        let st = self.mk(ValOp::Call(0), vec![target, rhs]);
        self.push_effect(st);
    }

    pub(in crate::value_graph) fn process_if(
        &mut self,
        stmt: NodeId,
        env: &mut FxHashMap<u32, ValueId>,
    ) {
        let kids = self.il.children(stmt).to_vec();
        if kids.is_empty() {
            return;
        }
        // The condition is *not* a standalone sink: it is captured where it matters —
        // in the `Phi` merge of any variable the branches update, and in the path-guard
        // of any return/effect they perform. Emitting it separately would make a
        // statement-`if` that updates a variable (`if c { x = a }`) diverge from the
        // equivalent ternary (`x = a if c else x`), which has no such sink. (`cond` is
        // still evaluated — for the path stack and so its sub-values are interned.)
        let cond = self.eval(kids[0], env);
        let effect_slot_base = self.effect_slot;

        let mut env_then = env.clone();
        let then_effect_slot = if kids.len() >= 2 {
            self.effect_slot = effect_slot_base;
            let bound_order_base = self.bound_order_facts.len();
            self.record_bound_order_fact(cond);
            self.path.push(cond);
            self.process_stmt(kids[1], &mut env_then);
            self.path.pop();
            self.bound_order_facts.truncate(bound_order_base);
            self.effect_slot
        } else {
            effect_slot_base
        };
        let mut env_else = env.clone();
        let else_effect_slot = if kids.len() >= 3 {
            self.effect_slot = effect_slot_base;
            let ncond = self.mk(ValOp::Un(Op::Not as u32), vec![cond]);
            let bound_order_base = self.bound_order_facts.len();
            self.record_bound_order_fact(ncond);
            self.path.push(ncond);
            self.process_stmt(kids[2], &mut env_else);
            self.path.pop();
            self.bound_order_facts.truncate(bound_order_base);
            self.effect_slot
        } else {
            effect_slot_base
        };
        self.effect_slot = then_effect_slot.max(else_effect_slot);

        // Merge: for each var that differs across branches, insert a Phi.
        let mut keys: Vec<u32> = env_then.keys().chain(env_else.keys()).copied().collect();
        keys.sort_unstable();
        keys.dedup();
        for cid in keys {
            let base = env.get(&cid).copied();
            let t = env_then.get(&cid).copied().or(base);
            let e = env_else.get(&cid).copied().or(base);
            match (t, e) {
                (Some(tv), Some(ev)) if tv == ev => {
                    env.insert(cid, tv);
                }
                (Some(tv), Some(ev)) => {
                    let phi = self.mk(ValOp::Phi, vec![cond, tv, ev]);
                    env.insert(cid, phi);
                }
                (Some(v), None) | (None, Some(v)) => {
                    env.insert(cid, v);
                }
                (None, None) => {}
            }
        }
    }
}
