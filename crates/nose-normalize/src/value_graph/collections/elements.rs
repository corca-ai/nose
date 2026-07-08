use super::super::*;

impl<'a> Builder<'a> {
    pub(in crate::value_graph) fn elem(&mut self, coll: ValueId) -> ValueId {
        if let Some(value) = self.hof_emitted_elem(coll) {
            return value;
        }
        self.raw_elem(coll)
    }

    fn hof_emitted_elem(&mut self, coll: ValueId) -> Option<ValueId> {
        let (emitted, predicate) = self.hof_emitted_elem_with_pred(coll)?;
        predicate.is_none().then_some(emitted)
    }

    fn raw_elem(&mut self, coll: ValueId) -> ValueId {
        self.mk(ValOp::Elem(self.vhash[coll as usize]), vec![coll])
    }

    pub(in crate::value_graph) fn collection_elem_with_pred(
        &mut self,
        coll: ValueId,
    ) -> (ValueId, Option<ValueId>) {
        if let Some(parts) = self.hof_emitted_elem_with_pred(coll) {
            return parts;
        }
        (self.raw_elem(coll), None)
    }

    pub(in crate::value_graph) fn js_like_array_param_universal_closed(
        &self,
        coll: ValueId,
    ) -> bool {
        self.is_js_like_lang() && self.is_array_param_value(coll)
    }

    pub(in crate::value_graph) fn refs_js_like_array_param_elem(&self, v: ValueId) -> bool {
        if !self.is_js_like_lang() {
            return false;
        }
        let mut seen = FxHashSet::default();
        let mut stack = vec![v];
        while let Some(n) = stack.pop() {
            if !seen.insert(n) {
                continue;
            }
            if matches!(self.nodes[n as usize].op, ValOp::Elem(_))
                && self.nodes[n as usize]
                    .args
                    .first()
                    .is_some_and(|&coll| self.is_array_param_value(coll))
            {
                return true;
            }
            stack.extend(self.nodes[n as usize].args.iter().copied());
        }
        false
    }

    fn hof_emitted_elem_with_pred(&mut self, coll: ValueId) -> Option<(ValueId, Option<ValueId>)> {
        // FUNCTOR LAW / map fusion: an element drawn from `map(f, c)` is `f` applied to an
        // element of `c`, and a pure Map node's `contrib` (args[0]) already *is* that
        // per-element value. So `Elem(Map(f, c)) -> contrib`, which fuses nested maps:
        // `map(g, map(f, xs))` and `map(g o f, xs)` converge to one fingerprint. Sound
        // (functor composition law: map g o map f = map (g o f)). A *filtered* map carries a
        // predicate (args.len() == 2) and is NOT peeled (the filter changes which elements).
        //
        // FlatMap emits the elements produced by its inner stream. When the modeled inner
        // stream is a pure Map, `Elem(FlatMap(xs, map(f, ys)))` is the same emitted `f(y)`
        // value. Keep this separate from the filtered-Map two-argument layout so aggregate
        // consumers do not confuse `FlatMap[outer, inner]` with `Map[contrib, pred]`.
        let (op, args) = {
            let n = &self.nodes[coll as usize];
            (n.op.clone(), n.args.clone())
        };
        match op {
            ValOp::Hof(k) if k == HoFKind::Map as u32 && args.len() == 1 => Some((args[0], None)),
            ValOp::Hof(k) if k == HoFKind::Map as u32 && args.len() == 2 => {
                Some((args[0], Some(args[1])))
            }
            ValOp::Hof(k)
                if k == HoFKind::FlatMap as u32 && (args.len() == 2 || args.len() == 3) =>
            {
                let outer = args[0];
                let inner = args[1];
                let outer_predicate = args.get(2).copied();
                let (emitted, inner_predicate) = self.hof_emitted_elem_with_pred(inner)?;
                if !self.references(emitted, outer) {
                    return None;
                }
                let predicate = self.and_preds(outer_predicate, inner_predicate);
                Some((emitted, predicate))
            }
            _ => None,
        }
    }

    /// A canonical "iteration index into `coll`". `for i in range(len(xs))`, an indexed
    /// `while`, and `for i, _ in enumerate(xs)` all bind their index variable to this,
    /// so they converge — and `C[Idx(C)]` rewrites to `Elem(C)`.
    pub(in crate::value_graph) fn idx(&mut self, coll: ValueId) -> ValueId {
        self.mk(ValOp::Idx(self.vhash[coll as usize]), vec![coll])
    }

    /// The canonical-id variables of a loop pattern: a single `Var`, or the elements
    /// of a tuple pattern `(i, x)` (lowered as a `Seq` of `Var`s).
    pub(in crate::value_graph) fn pattern_cids(&self, pat: NodeId) -> Vec<u32> {
        let mut out = Vec::new();
        let push = |n: NodeId, out: &mut Vec<u32>| {
            if let (NodeKind::Var, Payload::Cid(c)) = (self.il.kind(n), self.il.node(n).payload) {
                out.push(c);
            }
        };
        if self.il.kind(pat) == NodeKind::Seq {
            for c in self.il.children(pat) {
                push(*c, &mut out);
            }
        } else {
            push(pat, &mut out);
        }
        out
    }

    /// If `node` is a *full* index range over `C` — `range(len(C))` or `range(0,
    /// len(C))` — return `C`: the loop visits every index of `C`, so `C[i]` is the
    /// canonical `Elem(C)`. A non-zero start (`range(1, len(C))`), an explicit step
    /// (`range(_, _, k)`), or any other form iterates a *subset*, so its element is NOT
    /// `Elem(C)` — abstracting `C[i]` to `Elem(C)` there drops the start/step bound and
    /// merges behaviorally-different loops (a soundness bug). Such forms return `None`.
    pub(in crate::value_graph) fn range_len_collection(&self, node: NodeId) -> Option<NodeId> {
        let len_arg = if self.il.kind(node) == NodeKind::Call
            && matches!(self.il.node(node).payload, Payload::Builtin(Builtin::Range))
            && self.admitted_builtin_call(node, Builtin::Range)
        {
            let kids = self.il.children(node);
            match kids.len() {
                1 => kids[0],
                // `range(start, stop)` is a full iteration only when `start` is literally 0.
                2 if matches!(self.il.node(kids[0]).payload, Payload::LitInt(0)) => kids[1],
                _ => return None,
            }
        } else if self.il.kind(node) == NodeKind::Seq
            && source_range_at_node(self.il, node)
                == Some(SourceRangeKind::RustHalfOpenRangeExpression)
        {
            let kids = self.il.children(node);
            match kids {
                // Rust `0..len(C)` lowers as `Seq(0, Len(C), inclusive=0)`.
                // The source range fact proves this is a Rust half-open range expression;
                // raw `Seq(0, Len(C), 0)` shape alone is not an iteration contract.
                [start, stop, inclusive]
                    if matches!(self.il.node(*start).payload, Payload::LitInt(0))
                        && matches!(self.il.node(*inclusive).payload, Payload::LitInt(0)) =>
                {
                    *stop
                }
                _ => return None,
            }
        } else {
            return None;
        };
        if self.il.kind(len_arg) == NodeKind::Call
            && matches!(
                self.il.node(len_arg).payload,
                Payload::Builtin(Builtin::Len)
            )
            && self.admitted_builtin_call(len_arg, Builtin::Len)
        {
            return self.il.children(len_arg).first().copied();
        }
        None
    }
}
