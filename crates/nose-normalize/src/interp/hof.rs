use super::*;

impl<'a> Interp<'a> {
    pub(super) fn eval_hof(&mut self, node: NodeId, env: &mut FxHashMap<u32, Value>) -> R<Value> {
        let kind = match self.il.node(node).payload {
            Payload::HoF(h) => h,
            _ => return Err(Unsupported),
        };
        let kids = self.il.children(node).to_vec();
        if kids.len() < 2 {
            return Err(Unsupported);
        }
        let coll = match self.eval(kids[0], env)? {
            Value::List(xs) => xs,
            v if contains_sym(&v) => {
                let mut parts = vec![hashed(&kind), vhash(&v)];
                parts.extend(
                    kids.iter()
                        .skip(1)
                        .map(|&k| subtree_sig(self.il, self.interner, k)),
                );
                return Ok(Value::Sym(sym_id(0x040F_5E90, &parts)));
            }
            _ => return Ok(Value::Err),
        };
        let f = kids[1];
        match hof_contract(kind).demand {
            HofDemandProfile::Map { .. } => {
                let mut out = Vec::new();
                for x in coll {
                    let value = self.apply(f, &[x], env)?;
                    if matches!(value, Value::Err) {
                        return Ok(Value::Err);
                    }
                    out.push(value);
                }
                Ok(Value::List(out))
            }
            HofDemandProfile::FlatMap { .. } => {
                let mut out = Vec::new();
                for x in coll {
                    match self.apply(f, &[x], env)? {
                        Value::Err => return Ok(Value::Err),
                        Value::List(items) => out.extend(items),
                        _ => return Ok(Value::Err),
                    }
                }
                Ok(Value::List(out))
            }
            HofDemandProfile::FilterMap { .. } => {
                let mut out = Vec::new();
                for x in coll {
                    match self.apply(f, &[x], env)? {
                        Value::Err => return Ok(Value::Err),
                        Value::Null => {}
                        value => out.push(value),
                    }
                }
                Ok(Value::List(out))
            }
            HofDemandProfile::Filter { .. } => {
                let mut out = Vec::new();
                for x in coll {
                    let keep = self.apply(f, std::slice::from_ref(&x), env)?;
                    if matches!(keep, Value::Err) {
                        return Ok(Value::Err);
                    }
                    let keep = truthy(&keep).ok_or(Unsupported)?;
                    let keep = if kind == HoFKind::Reject { !keep } else { keep };
                    if keep {
                        out.push(x);
                    }
                }
                Ok(Value::List(out))
            }
            HofDemandProfile::Reduce { .. } => {
                let mut it = coll.into_iter();
                let mut acc = match it.next() {
                    Some(v) => v,
                    None => return Ok(Value::Err),
                };
                for x in it {
                    acc = self.apply(f, &[acc, x], env)?;
                    if matches!(acc, Value::Err) {
                        return Ok(Value::Err);
                    }
                }
                Ok(acc)
            }
        }
    }

    /// Apply a `Lambda` node to positional `args`, returning its body's value. The
    /// lambda's single tuple parameter destructures a pair element (zip/enumerate).
    pub(super) fn apply(
        &mut self,
        lambda: NodeId,
        args: &[Value],
        env: &mut FxHashMap<u32, Value>,
    ) -> R<Value> {
        if self.il.kind(lambda) != NodeKind::Lambda {
            return Err(Unsupported);
        }
        let kids = self.il.children(lambda).to_vec();
        let mut local = env.clone();
        let params: Vec<NodeId> = kids
            .iter()
            .copied()
            .filter(|&k| self.il.kind(k) == NodeKind::Param)
            .collect();
        // Bind params positionally; a single param receiving a pair stays a list.
        if params.len() == args.len() {
            for (p, a) in params.iter().zip(args) {
                if let Payload::Cid(c) = self.il.node(*p).payload {
                    local.insert(c, a.clone());
                }
            }
        } else if params.len() > 1 && args.len() == 1 {
            // tuple-destructured params over a pair element: `λ(x,y). …` applied to a
            // `[x, y]` element (a comprehension over zip/enumerate).
            if let Value::List(vs) = &args[0] {
                if vs.len() == params.len() {
                    for (p, v) in params.iter().zip(vs) {
                        if let Payload::Cid(c) = self.il.node(*p).payload {
                            local.insert(c, v.clone());
                        }
                    }
                } else {
                    return Err(Unsupported);
                }
            } else {
                return Err(Unsupported);
            }
        } else {
            return Err(Unsupported);
        }
        let body = *kids.last().ok_or(Unsupported)?;
        match self.il.kind(body) {
            NodeKind::Block
            | NodeKind::Assign
            | NodeKind::ExprStmt
            | NodeKind::Return
            | NodeKind::Throw
            | NodeKind::Loop
            | NodeKind::Try
            | NodeKind::Break
            | NodeKind::Continue => match self.exec(body, &mut local)? {
                Flow::Ret(v) => Ok(v),
                Flow::Err => Ok(Value::Err),
                Flow::Normal => Ok(Value::Null),
                Flow::Break | Flow::Continue => Err(Unsupported),
            },
            _ => self.eval(body, &mut local),
        }
    }
}
