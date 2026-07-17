use super::*;

impl<'a> Interp<'a> {
    /// Execute a statement (or block), threading control flow.
    pub(super) fn exec(&mut self, node: NodeId, env: &mut FxHashMap<u32, Value>) -> R<Flow> {
        self.exec_inner(node, env)
            .map_err(|blocker| blocker.with_frame(self.il, node, "exec"))
    }

    fn exec_inner(&mut self, node: NodeId, env: &mut FxHashMap<u32, Value>) -> R<Flow> {
        self.tick()?;
        match self.il.kind(node) {
            NodeKind::Block => {
                for s in self.il.children(node).to_vec() {
                    match self.exec(s, env)? {
                        Flow::Normal => {}
                        other => return Ok(other),
                    }
                }
                Ok(Flow::Normal)
            }
            NodeKind::Assign => {
                let kids = self.il.children(node).to_vec();
                if kids.len() != 2 {
                    return Err(Unsupported::il("il.assignment-shape"));
                }
                let rhs = self.eval(kids[1], env)?;
                if matches!(rhs, Value::Err) {
                    return Ok(Flow::Err);
                }
                if self.bind(kids[0], rhs, env, Some(node))? {
                    return Ok(Flow::Err);
                }
                Ok(Flow::Normal)
            }
            NodeKind::ExprStmt => {
                if let Some(&e) = self.il.children(node).first() {
                    if let Some(flow) = self.exec_stmt_append(e, env)? {
                        return Ok(flow);
                    }
                    if matches!(self.eval(e, env)?, Value::Err) {
                        return Ok(Flow::Err);
                    }
                }
                Ok(Flow::Normal)
            }
            NodeKind::Return => {
                let v = match self.il.children(node).first() {
                    Some(&e) => self.eval(e, env)?,
                    None => Value::Null,
                };
                if matches!(v, Value::Err) {
                    return Ok(Flow::Err);
                }
                Ok(Flow::Ret(v))
            }
            NodeKind::Throw => {
                if let Some(&e) = self.il.children(node).first() {
                    self.eval(e, env)?;
                }
                Ok(Flow::Throw)
            }
            NodeKind::If => {
                let kids = self.il.children(node).to_vec();
                if kids.is_empty() {
                    return Ok(Flow::Normal);
                }
                let cond = self.eval(kids[0], env)?;
                if matches!(cond, Value::Err) {
                    return Ok(Flow::Err);
                }
                if self.cond_truthy(&cond)? {
                    if let Some(&t) = kids.get(1) {
                        return self.exec(t, env);
                    }
                } else if let Some(&e) = kids.get(2) {
                    return self.exec(e, env);
                }
                Ok(Flow::Normal)
            }
            NodeKind::Loop => self.exec_loop(node, env),
            NodeKind::Try => self.exec_try(node, env),
            NodeKind::Break => Ok(Flow::Break),
            NodeKind::Continue => Ok(Flow::Continue),
            // Empty block / no-op pass lowers to an empty Block (handled above) or a
            // Seq with no children; anything else as a statement we don't model.
            NodeKind::Seq if self.il.children(node).is_empty() => Ok(Flow::Normal),
            _ => Err(Unsupported::il("il.statement-node-unsupported")),
        }
    }

    pub(super) fn exec_loop(&mut self, node: NodeId, env: &mut FxHashMap<u32, Value>) -> R<Flow> {
        let kind = match self.il.node(node).payload {
            Payload::Loop(k) => k,
            _ => LoopKind::While,
        };
        let kids = self.il.children(node).to_vec();
        match kind {
            LoopKind::While if kids.len() == 2 => {
                loop {
                    self.tick()?;
                    let c = self.eval(kids[0], env)?;
                    if matches!(c, Value::Err) {
                        return Ok(Flow::Err); // type error in the loop test → Err behavior
                    }
                    if !self
                        .value_truthy(&c)
                        .ok_or_else(|| Unsupported::value("value.loop-condition-truthiness"))?
                    {
                        break;
                    }
                    match self.exec(kids[1], env)? {
                        Flow::Normal | Flow::Continue => {}
                        Flow::Break => break,
                        other => return Ok(other), // Ret / Err propagate
                    }
                }
                Ok(Flow::Normal)
            }
            LoopKind::ForEach if kids.len() == 3 => {
                let seq = match self.eval(kids[1], env)? {
                    Value::List(xs) => xs,
                    Value::Err => return Ok(Flow::Err),
                    // Iterating a non-iterable (int/bool/null/string) is a runtime TYPE ERROR in
                    // every modeled language (Python/JS `TypeError`, …), not an unmodelable
                    // construct — so it is `Err` behavior, NOT `Unsupported`. This keeps a
                    // foreach-accumulator (the headline cross-language Type-4 pattern) interpretable
                    // even on the battery's scalar rows: it `Err`s there and computes on the list
                    // rows, so the oracle CHECKS it instead of excluding the whole unit.
                    _ => return Ok(Flow::Err),
                };
                for item in seq {
                    self.tick()?;
                    if self.bind(kids[0], item, env, None)? {
                        return Ok(Flow::Err);
                    }
                    match self.exec(kids[2], env)? {
                        Flow::Normal | Flow::Continue => {}
                        Flow::Break => break,
                        other => return Ok(other), // Ret / Err propagate
                    }
                }
                Ok(Flow::Normal)
            }
            LoopKind::CStyle if kids.len() == 4 => {
                // [init, cond, update, body] — desugar normally rewrites this away.
                match self.exec(kids[0], env)? {
                    Flow::Normal => {}
                    other => return Ok(other),
                }
                loop {
                    self.tick()?;
                    let c = self.eval(kids[1], env)?;
                    if matches!(c, Value::Err) {
                        return Ok(Flow::Err);
                    }
                    if !self
                        .value_truthy(&c)
                        .ok_or_else(|| Unsupported::value("value.loop-condition-truthiness"))?
                    {
                        break;
                    }
                    match self.exec(kids[3], env)? {
                        Flow::Normal | Flow::Continue => {}
                        Flow::Break => break,
                        other => return Ok(other), // Ret / Err propagate
                    }
                    match self.exec(kids[2], env)? {
                        Flow::Normal => {}
                        other => return Ok(other), // Ret / Break / Continue / Err propagate
                    }
                }
                Ok(Flow::Normal)
            }
            _ => Err(Unsupported::protocol("protocol.loop-shape")),
        }
    }

    pub(super) fn exec_try(&mut self, node: NodeId, env: &mut FxHashMap<u32, Value>) -> R<Flow> {
        let kids = self.il.children(node).to_vec();
        if kids.len() != 2 || self.il.children(kids[1]).is_empty() {
            return Err(Unsupported::protocol("protocol.exception-handler-shape"));
        }
        match self.exec(kids[0], env)? {
            Flow::Throw | Flow::Err => self.exec(kids[1], env),
            other => Ok(other),
        }
    }

    /// Bind a target (Var / tuple `Seq` / `Index` store) to a value.
    /// Returns true when evaluating the target itself raised a runtime `Err`.
    pub(super) fn bind(
        &mut self,
        target: NodeId,
        val: Value,
        env: &mut FxHashMap<u32, Value>,
        assignment: Option<NodeId>,
    ) -> R<bool> {
        match self.il.kind(target) {
            NodeKind::Var => {
                if let Payload::Cid(c) = self.il.node(target).payload {
                    env.insert(c, val);
                    Ok(false)
                } else {
                    Err(Unsupported::il("il.assignment-target-identity"))
                }
            }
            NodeKind::Seq => {
                // tuple unpack: `a, b = pair`
                let names = self.il.children(target).to_vec();
                let vals = match val {
                    Value::List(vs) if vs.len() == names.len() => vs,
                    _ => {
                        return Err(Unsupported::value("value.destructure-shape"));
                    }
                };
                for (t, v) in names.into_iter().zip(vals) {
                    if self.bind(t, v, env, None)? {
                        return Ok(true);
                    }
                }
                Ok(false)
            }
            // A field store updates per-place object state (last-write-wins), keyed by
            // receiver identity plus field name. Writing distinct receiver+field places
            // commutes; same-place overwrites keep the last value.
            NodeKind::Field => {
                let Some(&receiver) = self.il.children(target).first() else {
                    return Err(Unsupported::il("il.field-receiver-missing"));
                };
                if self.field_receiver_errored(receiver, env)? {
                    return Ok(true);
                }
                if let Some(assign) = assignment {
                    let Some(key) = self.exact_field_write_key(assign, target) else {
                        return Err(Unsupported::protocol("protocol.field-write-proof"));
                    };
                    self.fields.insert(key, val);
                    Ok(false)
                } else {
                    Err(Unsupported::protocol("protocol.field-write-context"))
                }
            }
            NodeKind::Index => {
                let kids = self.il.children(target).to_vec();
                let Some(&base) = kids.first() else {
                    return Err(Unsupported::il("il.index-base-missing"));
                };
                let base_value = self.eval(base, env)?;
                if matches!(base_value, Value::Err) {
                    return Ok(true);
                }
                let mut iv = None;
                if let Some(&ix) = kids.get(1) {
                    let v = self.eval(ix, env)?;
                    if matches!(v, Value::Err) {
                        return Ok(true);
                    }
                    self.effects.push(v.clone());
                    iv = Some(v);
                }
                self.effects.push(val.clone());
                // In-place element mutation (#337): when the base is a simple var holding a
                // `List` and the index is an in-bounds `Int`, update the element so a LATER
                // `base[i]` read observes the write — this is what distinguishes `swap`
                // (`t=a[i]; a[i]=a[j]; a[j]=t`) from `clobber` (`a[i]=a[j]; a[j]=a[i]`). The
                // ordered-effect push above still records the write itself; mutation only
                // changes what subsequent reads see. Any non-clean shape (computed/aliased
                // base, out-of-bounds, non-List) leaves the list untouched — the conservative
                // no-mutation fall-back, matching the value graph's clear-on-write forwarding.
                if let (NodeKind::Var, Some(Value::Int(i))) = (self.il.kind(base), &iv) {
                    if let Payload::Cid(c) = self.il.node(base).payload {
                        if let Some(Value::List(xs)) = env.get_mut(&c) {
                            let idx = if *i < 0 { *i + xs.len() as i64 } else { *i };
                            if idx >= 0 && (idx as usize) < xs.len() {
                                xs[idx as usize] = val;
                            }
                        }
                    }
                }
                Ok(false)
            }
            _ => Err(Unsupported::protocol("protocol.assignment-target")),
        }
    }
}
