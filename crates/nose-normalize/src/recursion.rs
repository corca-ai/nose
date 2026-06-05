//! Recursion → iteration canonicalization: rewrite the two recursion schemes that
//! have a *behavior-preserving* iterative form into that form, so a recursive function
//! converges with the loop a programmer would have written instead (and with other
//! recursions of the same shape — cross-language included, since this runs on the shared
//! IL). Everything outside the two proven templates is left untouched.
//!
//! ## Tail recursion → `while`
//!
//! ```text
//! f(p…):  if c₀: return v₀ ; … ; return f(a…)
//! ```
//! becomes `while not(c₀ or …) { p… := a… } ; if c₀: return v₀ ; … ; return vₖ₋₁`.
//! Each loop turn performs the next call's argument bindings (in a hazard-safe order — a
//! cyclic binding like a swap bails), and on exit exactly one guard holds, so the
//! post-loop guard chain returns the same base value the final call would have.
//!
//! ## Structural (linear) recursion → accumulator fold
//!
//! ```text
//! f(p…):  if base: return e ; return  HEAD ⊕ f(a…)      (or f(a…) ⊕ HEAD)
//! ```
//! becomes `acc = e ; while not(base) { acc = acc ⊕ HEAD ; p… := a… } ; return acc`.
//! `f(a…) = HEAD₀ ⊕ (HEAD₁ ⊕ (… ⊕ e))` (a right fold) equals the left fold the loop
//! computes **iff ⊕ is an associative monoid with identity `e`**. This pass therefore
//! fires ONLY for `⊕ ∈ {+, ·}` proven `Num` (commutative *and* associative; identities
//! `0`/`1`), with the base case returning exactly that identity literal. Short-circuit
//! `and`/`or` are excluded: their early-exit skips later `HEAD`s the accumulator loop
//! would still evaluate, so the forms diverge on an erroring/​effectful `HEAD`.
//!
//! Soundness is not taken on faith: the rewrite runs in the SEMANTIC phase (after the
//! oracle's structural cutoff), so `nose verify` interprets the original recursion — which
//! the interpreter now executes as a real call (see [`crate::interp`]) — and the rewritten
//! loop, and flags any behavioral difference.

use crate::types::{infer_param_types, result_ty, Ty};
use nose_il::{FileMeta, Il, IlBuilder, NodeId, NodeKind, Op, Payload, Symbol, Unit, UnitKind};
use rustc_hash::{FxHashMap, FxHashSet};

pub(crate) fn run(old: &Il) -> Il {
    // A same-named call inside a standalone function is its self-call. Methods are excluded:
    // `self.m()` lowers through a `Field` callee (so it never matches the bare-name test
    // below anyway), and a bare-name call inside a method is more likely a free function.
    let func_name: FxHashMap<u32, Symbol> = old
        .units
        .iter()
        .filter(|u| u.kind == UnitKind::Function)
        .filter_map(|u| u.name.map(|n| (u.root.0, n)))
        .collect();
    let unit_root_set: FxHashSet<u32> = old.units.iter().map(|u| u.root.0).collect();
    let mut rb = Rebuilder {
        old,
        b: IlBuilder::new(old.file),
        func_name,
        unit_root_set,
        remap: FxHashMap::default(),
    };
    let new_root = rb.go(old.root);

    let units: Vec<Unit> = old
        .units
        .iter()
        .filter_map(|u| {
            rb.remap.get(&u.root.0).map(|&root| Unit {
                root,
                kind: u.kind,
                name: u.name,
            })
        })
        .collect();
    let meta = FileMeta {
        path: old.meta.path.clone(),
        lang: old.meta.lang,
    };
    let mut out = rb.b.finish(new_root, meta, units, old.cid_names.clone());
    out.param_type_facts = old.param_type_facts.clone();
    out
}

struct Rebuilder<'a> {
    old: &'a Il,
    b: IlBuilder,
    func_name: FxHashMap<u32, Symbol>,
    unit_root_set: FxHashSet<u32>,
    remap: FxHashMap<u32, NodeId>,
}

/// A recognized recursion ready to be emitted as a loop. Node ids are in the OLD arena
/// (copied into the new one during emission). `param_cids` is the function's parameters in
/// order; `args` are the next-call arguments, positionally matched to them.
enum Plan {
    Tail {
        param_cids: Vec<u32>,
        guards: Vec<(NodeId, NodeId)>, // (cond, returned value)
        args: Vec<NodeId>,
    },
    Structural {
        param_cids: Vec<u32>,
        base_cond: NodeId,
        op: Op,
        head: NodeId,
        args: Vec<NodeId>,
        identity: NodeId,
    },
}

impl Rebuilder<'_> {
    fn go(&mut self, old_id: NodeId) -> NodeId {
        let new_id = if self.old.kind(old_id) == NodeKind::Func {
            self.func(old_id)
        } else {
            self.generic(old_id)
        };
        if self.unit_root_set.contains(&old_id.0) {
            self.remap.insert(old_id.0, new_id);
        }
        new_id
    }

    crate::rebuild_generic!();

    fn func(&mut self, fid: NodeId) -> NodeId {
        if let Some(&name) = self.func_name.get(&fid.0) {
            if let Some(plan) = self.recognize(fid, name) {
                if let Some(rewritten) = self.build(fid, &plan) {
                    return rewritten;
                }
            }
        }
        self.generic(fid)
    }

    // ----- recognition (read-only over the old arena) -----

    fn param_cids(&self, fid: NodeId) -> Vec<u32> {
        self.old
            .children(fid)
            .iter()
            .filter(|&&c| self.old.kind(c) == NodeKind::Param)
            .filter_map(|&c| match self.old.node(c).payload {
                Payload::Cid(cid) => Some(cid),
                _ => None,
            })
            .collect()
    }

    /// Is `node` a direct call to the enclosing function `name`? If so, its argument nodes.
    fn as_self_call(&self, node: NodeId, name: Symbol) -> Option<Vec<NodeId>> {
        if self.old.kind(node) != NodeKind::Call {
            return None;
        }
        // A canonicalized builtin carries `Builtin` and drops its callee child — never a
        // user self-call.
        if !matches!(self.old.node(node).payload, Payload::None) {
            return None;
        }
        let kids = self.old.children(node);
        let callee = *kids.first()?;
        match self.old.node(callee).payload {
            Payload::Name(s) if s == name => Some(kids[1..].to_vec()),
            _ => None,
        }
    }

    fn count_self_calls(&self, node: NodeId, name: Symbol) -> usize {
        let here = usize::from(self.as_self_call(node, name).is_some());
        here + self
            .old
            .children(node)
            .iter()
            .map(|&c| self.count_self_calls(c, name))
            .sum::<usize>()
    }

    /// Parse a base-case guard `if cond: return val` (no `else`, single returned value).
    fn parse_guard(&self, g: NodeId) -> Option<(NodeId, NodeId)> {
        if self.old.kind(g) != NodeKind::If {
            return None;
        }
        let kids = self.old.children(g);
        if kids.len() != 2 {
            return None; // an `else` arm is not a guard shape we rewrite
        }
        let (cond, then) = (kids[0], kids[1]);
        let ret = match self.old.kind(then) {
            NodeKind::Return => then,
            NodeKind::Block => match self.old.children(then) {
                [only] if self.old.kind(*only) == NodeKind::Return => *only,
                _ => return None,
            },
            _ => return None,
        };
        match self.old.children(ret) {
            [val] => Some((cond, *val)),
            _ => None,
        }
    }

    fn recognize(&self, fid: NodeId, name: Symbol) -> Option<Plan> {
        let kids = self.old.children(fid);
        let body = *kids.last()?;
        if self.old.kind(body) != NodeKind::Block {
            return None;
        }
        let stmts = self.old.children(body);
        if stmts.len() < 2 {
            return None;
        }
        let (guard_stmts, last) = stmts.split_at(stmts.len() - 1);
        let last = last[0];
        if self.old.kind(last) != NodeKind::Return {
            return None;
        }
        let guards: Vec<(NodeId, NodeId)> = guard_stmts
            .iter()
            .map(|&g| self.parse_guard(g))
            .collect::<Option<_>>()?;
        if guards.is_empty() {
            return None;
        }
        // Exactly one self-call in the whole body — the recursive case below. This also
        // guarantees the guards (and every argument/operand) are self-call-free.
        if self.count_self_calls(body, name) != 1 {
            return None;
        }
        let rexpr = match self.old.children(last) {
            [e] => *e,
            _ => return None,
        };
        let param_cids = self.param_cids(fid);

        // Tail recursion: the recursive case IS the self-call.
        if let Some(args) = self.as_self_call(rexpr, name) {
            if args.len() != param_cids.len() {
                return None;
            }
            return Some(Plan::Tail {
                param_cids,
                guards,
                args,
            });
        }

        // Structural recursion: `HEAD ⊕ f(a…)` / `f(a…) ⊕ HEAD`, gated to a numeric monoid.
        if self.old.kind(rexpr) == NodeKind::BinOp {
            let op = match self.old.node(rexpr).payload {
                Payload::Op(o) => o,
                _ => return None,
            };
            if !matches!(op, Op::Add | Op::Mul) {
                return None;
            }
            let operands = self.old.children(rexpr);
            if operands.len() != 2 {
                return None;
            }
            let (a, b) = (operands[0], operands[1]);
            let (head, args) = match (self.as_self_call(a, name), self.as_self_call(b, name)) {
                (Some(args), None) => (b, args),
                (None, Some(args)) => (a, args),
                _ => return None, // both or neither — not a linear fold
            };
            if args.len() != param_cids.len() || guards.len() != 1 {
                return None;
            }
            let (base_cond, identity) = guards[0];
            // Numeric monoid gate: ⊕ on proven-`Num` operands (so commutative + associative)
            // and the base case returning ⊕'s identity literal (`0` for `+`, `1` for `·`).
            let ev = self.param_type_env(fid, &param_cids);
            if result_ty(self.old, head, &ev) != Ty::Num {
                return None;
            }
            let want_identity = match op {
                Op::Add => 0,
                Op::Mul => 1,
                _ => return None,
            };
            if !self.is_int_literal(identity, want_identity) {
                return None;
            }
            return Some(Plan::Structural {
                param_cids,
                base_cond,
                op,
                head,
                args,
                identity,
            });
        }
        None
    }

    fn param_type_env(&self, fid: NodeId, param_cids: &[u32]) -> FxHashMap<u32, Ty> {
        infer_param_types(self.old, fid)
            .into_iter()
            .enumerate()
            .filter_map(|(i, ty)| param_cids.get(i).map(|&c| (c, ty)))
            .collect()
    }

    fn is_int_literal(&self, node: NodeId, want: i64) -> bool {
        self.old.kind(node) == NodeKind::Lit
            && matches!(self.old.node(node).payload, Payload::LitInt(v) if v == want)
    }

    // ----- emission (into the new arena) -----

    fn build(&mut self, fid: NodeId, plan: &Plan) -> Option<NodeId> {
        let span = self.old.node(fid).span;
        let params: Vec<NodeId> = self
            .old
            .children(fid)
            .iter()
            .take_while(|&&c| self.old.kind(c) == NodeKind::Param)
            .map(|&c| self.generic(c))
            .collect();

        let body = match plan {
            Plan::Tail {
                param_cids,
                guards,
                args,
            } => {
                let updates = self.ordered_updates(param_cids, args)?;
                let cond = self.not_any(guards.iter().map(|&(c, _)| c).collect());
                let loop_body = self.b.add(NodeKind::Block, Payload::None, span, &updates);
                let wl = self.while_loop(cond, loop_body, span);
                // Post-loop guard chain: exactly one guard holds on exit, so the final one
                // is an unconditional return.
                let mut stmts = vec![wl];
                for (i, &(cond, val)) in guards.iter().enumerate() {
                    let v = self.go_val(val);
                    let ret = self.ret(v, span);
                    if i + 1 == guards.len() {
                        stmts.push(ret);
                    } else {
                        let c = self.go_val(cond);
                        let then = self.b.add(NodeKind::Block, Payload::None, span, &[ret]);
                        stmts.push(self.b.add(NodeKind::If, Payload::None, span, &[c, then]));
                    }
                }
                self.b.add(NodeKind::Block, Payload::None, span, &stmts)
            }
            Plan::Structural {
                param_cids,
                base_cond,
                op,
                head,
                args,
                identity,
            } => {
                let acc = self.fresh_cid(fid);
                let init = {
                    let lhs = self.var(acc, span);
                    let rhs = self.go_val(*identity);
                    self.b
                        .add(NodeKind::Assign, Payload::None, span, &[lhs, rhs])
                };
                // `acc = acc ⊕ HEAD` runs first (reads the current params/acc), then the
                // params advance to the next call's arguments.
                let acc_update = {
                    let lhs = self.var(acc, span);
                    let cur = self.var(acc, span);
                    let h = self.go_val(*head);
                    let combined = self
                        .b
                        .add(NodeKind::BinOp, Payload::Op(*op), span, &[cur, h]);
                    self.b
                        .add(NodeKind::Assign, Payload::None, span, &[lhs, combined])
                };
                let mut loop_stmts = vec![acc_update];
                loop_stmts.extend(self.ordered_updates(param_cids, args)?);
                let cond = self.not_any(vec![*base_cond]);
                let loop_body = self
                    .b
                    .add(NodeKind::Block, Payload::None, span, &loop_stmts);
                let wl = self.while_loop(cond, loop_body, span);
                let ret = {
                    let v = self.var(acc, span);
                    self.ret(v, span)
                };
                self.b
                    .add(NodeKind::Block, Payload::None, span, &[init, wl, ret])
            }
        };

        let mut children = params;
        children.push(body);
        Some(
            self.b
                .add(NodeKind::Func, self.old.node(fid).payload, span, &children),
        )
    }

    /// Copy an old-arena expression subtree into the new arena.
    fn go_val(&mut self, old_id: NodeId) -> NodeId {
        self.go(old_id)
    }

    fn var(&mut self, cid: u32, span: nose_il::Span) -> NodeId {
        self.b.add(NodeKind::Var, Payload::Cid(cid), span, &[])
    }

    fn ret(&mut self, val: NodeId, span: nose_il::Span) -> NodeId {
        self.b.add(NodeKind::Return, Payload::None, span, &[val])
    }

    fn while_loop(&mut self, cond: NodeId, body: NodeId, span: nose_il::Span) -> NodeId {
        self.b.add(
            NodeKind::Loop,
            Payload::Loop(nose_il::LoopKind::While),
            span,
            &[cond, body],
        )
    }

    /// `not(c₀ or c₁ or …)` — the loop continues while no base case has been reached.
    fn not_any(&mut self, conds: Vec<NodeId>) -> NodeId {
        let span = self.old.node(conds[0]).span;
        let mut it = conds.into_iter();
        let mut acc = self.go_val(it.next().unwrap());
        for c in it {
            let cc = self.go_val(c);
            acc = self
                .b
                .add(NodeKind::BinOp, Payload::Op(Op::Or), span, &[acc, cc]);
        }
        self.b
            .add(NodeKind::UnOp, Payload::Op(Op::Not), span, &[acc])
    }

    /// A fresh canonical id for an introduced variable: one past the max id used in the
    /// function. `cid_names` is read with `.get()` everywhere, so leaving it unextended is
    /// safe (the new id simply has no source name).
    fn fresh_cid(&self, fid: NodeId) -> u32 {
        fn max_cid(il: &Il, n: NodeId, m: &mut u32) {
            if let Payload::Cid(c) = il.node(n).payload {
                *m = (*m).max(c);
            }
            for &c in il.children(n) {
                max_cid(il, c, m);
            }
        }
        let mut m = 0;
        max_cid(self.old, fid, &mut m);
        m + 1
    }

    /// Emit `pᵢ := argᵢ` as parallel bindings: each argument reads the *old* parameter
    /// values. Identity bindings (`pᵢ := pᵢ`) are dropped; the rest are ordered so no
    /// binding clobbers a parameter a later one still reads. A cyclic dependency (e.g. a
    /// swap `f(b, a)`) has no such order — return `None` so the caller keeps the recursion.
    fn ordered_updates(&mut self, param_cids: &[u32], args: &[NodeId]) -> Option<Vec<NodeId>> {
        let param_set: FxHashSet<u32> = param_cids.iter().copied().collect();
        // Non-identity updates: (param cid, arg old id, params it reads).
        let mut updates: Vec<(u32, NodeId, FxHashSet<u32>)> = Vec::new();
        for (i, &arg) in args.iter().enumerate() {
            let p = param_cids[i];
            if self.is_var_cid(arg, p) {
                continue; // pᵢ := pᵢ is a no-op
            }
            let mut reads = FxHashSet::default();
            collect_reads(self.old, arg, &param_set, &mut reads);
            reads.remove(&p); // a self-read keeps the param's own old value — not a hazard
            updates.push((p, arg, reads));
        }
        let order = toposort_updates(&updates)?;
        Some(
            order
                .into_iter()
                .map(|idx| {
                    let (p, arg, _) = &updates[idx];
                    let span = self.old.node(*arg).span;
                    let lhs = self.var(*p, span);
                    let rhs = self.go_val(*arg);
                    self.b
                        .add(NodeKind::Assign, Payload::None, span, &[lhs, rhs])
                })
                .collect(),
        )
    }

    fn is_var_cid(&self, node: NodeId, cid: u32) -> bool {
        self.old.kind(node) == NodeKind::Var
            && matches!(self.old.node(node).payload, Payload::Cid(c) if c == cid)
    }
}

/// Collect the parameter cids referenced anywhere in `node`.
fn collect_reads(il: &Il, node: NodeId, params: &FxHashSet<u32>, out: &mut FxHashSet<u32>) {
    if il.kind(node) == NodeKind::Var {
        if let Payload::Cid(c) = il.node(node).payload {
            if params.contains(&c) {
                out.insert(c);
            }
        }
    }
    for &c in il.children(node) {
        collect_reads(il, c, params, out);
    }
}

/// Order updates so that any update writing `p` runs AFTER every update that reads `p`
/// (which needs `p`'s old value). Returns indices into `updates`, or `None` on a cycle.
fn toposort_updates(updates: &[(u32, NodeId, FxHashSet<u32>)]) -> Option<Vec<usize>> {
    let n = updates.len();
    // edge j -> i  (j before i)  when update j reads the param that update i writes.
    let writes: Vec<u32> = updates.iter().map(|(p, _, _)| *p).collect();
    let mut indeg = vec![0usize; n];
    let mut adj: Vec<Vec<usize>> = vec![Vec::new(); n];
    for (j, (_, _, reads)) in updates.iter().enumerate() {
        for (i, &wp) in writes.iter().enumerate() {
            if i != j && reads.contains(&wp) {
                adj[j].push(i);
                indeg[i] += 1;
            }
        }
    }
    // Kahn, picking the lowest index among ready nodes for determinism.
    let mut order = Vec::with_capacity(n);
    let mut ready: Vec<usize> = (0..n).filter(|&i| indeg[i] == 0).collect();
    while let Some(pos) = ready
        .iter()
        .enumerate()
        .min_by_key(|&(_, &v)| v)
        .map(|(k, _)| k)
    {
        let u = ready.remove(pos);
        order.push(u);
        for &v in &adj[u] {
            indeg[v] -= 1;
            if indeg[v] == 0 {
                ready.push(v);
            }
        }
    }
    (order.len() == n).then_some(order)
}
