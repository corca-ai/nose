//! Track 2 — algebraic expression canonicalization.
//!
//! A deterministic, confluent, **value-independent** rewrite that puts
//! expressions into a canonical normal form (subsumes the old commutative
//! operand-sort):
//!
//! - **associativity + commutativity**: `+ * && || & | ^` chains are flattened,
//!   their operands sorted by structural hash, and rebuilt left-leaning, so any
//!   grouping/ordering of `a + b + c` converges.
//! - **comparison direction**: `a > b` → `b < a`, `a >= b` → `b <= a`, so only
//!   `<`/`<=`/`==`/`!=` remain.
//! - **negation / De Morgan**: `!(a && b)` → `!a || !b`, `!(a == b)` → `a != b`.
//!   Order-comparison duals such as `!(a < b)` → `a >= b`, plus value-DEPENDENT
//!   cancellations `!!x → x` (bool coercion) and `-(-x) → x` (numeric only), are
//!   NOT done here — this pass has no operand type, so they are deferred to the
//!   value graph's proof-gated canon.
//!
//! Value-dependent identities (`x + 0` → `x`) are deferred — they need literal
//! values, which are abstracted to classes. Full distribution (where the
//! canonical form is non-obvious) is also deferred; a confluent rewrite gives a
//! normal form without the cost/ambiguity of equality saturation.
//!
//! proof-obligation: normalize.value_graph.algebra
//! proof-obligation: normalize.value_graph.compare

use crate::combine;
use nose_il::{Il, IlBuilder, Interner, NodeId, NodeKind, Op, Payload, Span};
use nose_semantics::{semantics, ComparisonLaw};
use rustc_hash::{FxHashMap, FxHashSet};

pub(crate) fn run(old: &Il, interner: &Interner) -> Il {
    if !old
        .nodes
        .iter()
        .any(|node| matches!(node.kind, NodeKind::BinOp | NodeKind::UnOp))
    {
        return old.clone();
    }
    let unit_root_set: FxHashSet<u32> = old.units.iter().map(|u| u.root.0).collect();
    // Params with FLOAT type evidence (`double`/`f64`/`float64`/`: float`/…): a `+`/`*`
    // chain over them is non-associative (#283 C-float), so it must not be reassociated.
    // The value graph already proves these float (`proven_float` via the param domain); the
    // grouping is lost HERE, in the IL reassociation, unless this pass is float-aware too.
    let float_param_cids: FxHashSet<u32> = old
        .nodes
        .iter()
        .enumerate()
        .filter(|(_, node)| node.kind == NodeKind::Param)
        .filter_map(|(i, node)| match node.payload {
            Payload::Cid(c)
                if nose_semantics::domain_evidence_for_param(old, NodeId(i as u32))
                    .is_some_and(|d| d.is_float()) =>
            {
                Some(c)
            }
            _ => None,
        })
        .collect();
    let mut rw = Rewriter {
        old,
        b: IlBuilder::with_capacity(old.file, old.nodes.len(), old.edges.len()),
        hashes: Vec::with_capacity(old.nodes.len()),
        remap: FxHashMap::default(),
        unit_root_set,
        float_param_cids,
        interner,
    };
    let (new_root, _) = rw.rewrite(old.root);
    crate::finalize_rebuild(old, &rw.remap, rw.b, new_root, old.cid_names.clone())
}

fn is_assoc_comm(op: Op) -> bool {
    matches!(
        op,
        Op::Add | Op::Mul | Op::And | Op::Or | Op::BitAnd | Op::BitOr | Op::BitXor
    )
}

struct Rewriter<'a> {
    old: &'a Il,
    b: IlBuilder,
    /// Canonical hash per *new* node id, kept in lockstep with `b`'s arena.
    hashes: Vec<u64>,
    remap: FxHashMap<u32, NodeId>,
    unit_root_set: FxHashSet<u32>,
    /// Canonical ids of float-typed params (#283 C-float) — a `+`/`*` chain touching one
    /// is non-associative and must keep its source grouping.
    float_param_cids: FxHashSet<u32>,
    interner: &'a Interner,
}

impl Rewriter<'_> {
    /// The only node constructor: keeps `hashes` aligned with the arena.
    fn emit(
        &mut self,
        kind: NodeKind,
        payload: Payload,
        span: Span,
        kids: &[NodeId],
        khashes: &[u64],
    ) -> (NodeId, u64) {
        let id = self.b.add(kind, payload, span, kids);
        let mut h = crate::node_tag(kind, payload, self.interner);
        for &kh in khashes {
            h = combine(h, kh);
        }
        debug_assert_eq!(self.hashes.len(), id.0 as usize);
        self.hashes.push(h);
        (id, h)
    }

    fn rewrite(&mut self, old_id: NodeId) -> (NodeId, u64) {
        let node = *self.old.node(old_id);
        let res = match node.kind {
            NodeKind::BinOp => self.rewrite_binop(old_id, node.span),
            NodeKind::UnOp => match node.payload {
                Payload::Op(Op::Not) => {
                    let c = self.old.children(old_id)[0];
                    self.rewrite_negated(c)
                }
                Payload::Op(Op::Neg) => {
                    // `-(-x)` is `x` ONLY for numbers — on a list/string `-x` Errs, so
                    // `-(-x)` Errs while `x` does not. Cancelling it HERE is unsound: this
                    // pass is value-INDEPENDENT and has no operand type, so it would merge a
                    // `return -(-a)` with an identity `return a` for an untyped param (#283-B,
                    // exactly the `!!x → x` hazard handled just above). Preserve the double
                    // negation; the value graph cancels `-(-x) → x` ONLY when `x` is provably
                    // numeric (`proven_numeric`, genuine evidence — never the optimistic
                    // "param is Num because `-` was applied to it" inference).
                    self.generic(old_id, node.span)
                }
                _ => self.generic(old_id, node.span),
            },
            _ => self.generic(old_id, node.span),
        };
        if self.unit_root_set.contains(&old_id.0) {
            self.remap.insert(old_id.0, res.0);
        }
        res
    }

    fn generic(&mut self, old_id: NodeId, span: Span) -> (NodeId, u64) {
        let n = *self.old.node(old_id);
        let child_count = self.old.children(old_id).len();
        let mut kids = Vec::with_capacity(child_count);
        let mut khashes = Vec::with_capacity(child_count);
        for idx in 0..child_count {
            let child = self.old.children(old_id)[idx];
            let (id, h) = self.rewrite(child);
            kids.push(id);
            khashes.push(h);
        }
        self.emit(n.kind, n.payload, span, &kids, &khashes)
    }

    fn rewrite_binop(&mut self, old_id: NodeId, span: Span) -> (NodeId, u64) {
        let op = match self.old.node(old_id).payload {
            Payload::Op(o) => o,
            _ => return self.generic(old_id, span),
        };
        if is_assoc_comm(op) {
            // JS/TS/Java `+` can coerce mixed string/non-string operands, so grouping itself
            // is observable: `"a" + 2 + 3` is not `"a" + (2 + 3)`. This IL pass has no
            // evidence strong enough to prove a chain numeric; leave the tree intact and
            // let the value graph perform evidence-gated association/commutation.
            if op == Op::Add
                && semantics(self.old.meta.lang)
                    .operators()
                    .plus_coerces_strings()
            {
                return self.generic(old_id, span);
            }
            let mut leaves = Vec::new();
            self.collect_assoc_old(old_id, op, &mut leaves);
            // Float `+`/`*` is NON-ASSOCIATIVE (`(a+b)+c != a+(b+c)`, #283 C-float). If any
            // leaf is provably float — a float literal, a `/` true-division result, or a
            // float-TYPED param — leave the source tree intact (like the mixed-coercion `+`
            // above) so the grouping survives into the value graph, which keeps it (its
            // `proven_float` gate). Folding/reassociating here would erase the grouping the
            // float result depends on. (The fully-untyped chain with no float evidence still
            // flattens — that needs the full Float value kind, §3.3.)
            if matches!(op, Op::Add | Op::Mul) && self.chain_has_float(&leaves) {
                return self.generic(old_id, span);
            }
            // Constant folding + identity elimination (now that C retains literal values):
            // `2 + 3` → `5`, `x + 2 + 3` → `x + 5`, `x + 0` → `x`, `x * 1` → `x`. SOUND —
            // `x` is still evaluated either way. NOT `x * 0 → 0` (would drop `x`'s
            // side effects). Only the arithmetic ring ops; bitwise/logical left as-is.
            // EXCEPT Ruby `*`: it is string/array REPETITION and asymmetric (`3 * s` Errs
            // when `s` is a String but `s * 3` repeats), so folding a constant to the end —
            // a commute — would change behavior. Hold it ordered; the value graph commutes
            // it type-gated (series 9). Numeric Ruby `*` still converges there.
            let ruby_mul = op == Op::Mul
                && semantics(self.old.meta.lang)
                    .operators()
                    .mul_is_sequence_repetition();
            if matches!(op, Op::Add | Op::Mul) && !ruby_mul {
                return self.fold_arith(op, span, &leaves);
            }
            let operands: Vec<(NodeId, u64)> = leaves.iter().map(|&c| self.rewrite(c)).collect();
            return self.build_assoc(op, span, operands);
        }
        let kids = self.old.children(old_id).to_vec();
        if kids.len() != 2 {
            return self.generic(old_id, span);
        }
        let operators = semantics(self.old.meta.lang).operators();
        if let Some(contract) = operators.comparison_direction(op) {
            // Canonicalize comparison direction by reflecting the order: `a > b` → `b < a`
            // (`normalize.value_graph.compare`), `a >= b` → `b <= a`.
            // Both arms swap the operands and emit the mirror operator; only the target
            // opcode differs.
            let (r, rh) = self.rewrite(kids[1]);
            let (l, lh) = self.rewrite(kids[0]);
            return self.emit(
                NodeKind::BinOp,
                Payload::Op(contract.output),
                span,
                &[r, l],
                &[rh, lh],
            );
        }
        match op {
            // commutative but not associative: sort the two operands
            Op::Eq | Op::Ne
                if operators
                    .comparison_law(ComparisonLaw::EqualityCommutativity)
                    .is_some() =>
            {
                self.emit_commutative_cmp(op, span, kids[0], kids[1])
            }
            _ => {
                let (l, lh) = self.rewrite(kids[0]);
                let (r, rh) = self.rewrite(kids[1]);
                self.emit(NodeKind::BinOp, Payload::Op(op), span, &[l, r], &[lh, rh])
            }
        }
    }

    /// Emit a commutative-but-not-associative comparison (`==`/`!=`): rewrite both operands,
    /// then order them by structural hash (ties keep source order) so `a == b` and `b == a`
    /// converge. Shared by the direct `Eq`/`Ne` arm and the De-Morgan'd `!(a == b)` →
    /// `a != b` arm, which differ only in the target opcode.
    fn emit_commutative_cmp(
        &mut self,
        op: Op,
        span: Span,
        k0: NodeId,
        k1: NodeId,
    ) -> (NodeId, u64) {
        let (l, lh) = self.rewrite(k0);
        let (r, rh) = self.rewrite(k1);
        let (a, ah, bb, bh) = if lh <= rh {
            (l, lh, r, rh)
        } else {
            (r, rh, l, lh)
        };
        self.emit(NodeKind::BinOp, Payload::Op(op), span, &[a, bb], &[ah, bh])
    }

    /// Fold the integer-constant leaves of an `Add`/`Mul` chain into one constant. `*0` is
    /// deliberately NOT collapsed (it would drop a side effect). The identity element
    /// (`+0`, `*1`) is NOT dropped here: `x + 0` and `x * 1` equal `x` only when `x` is
    /// NUMERIC (`"a" + 0` is a TypeError; `self * 1` on a non-number need not be `self`), so
    /// dropping it untyped silently merged `return self*1` with an identity `return self`.
    /// The value graph removes the identity ONLY when the surviving operand is proven Num.
    fn fold_arith(&mut self, op: Op, span: Span, leaves: &[NodeId]) -> (NodeId, u64) {
        let identity: i64 = if op == Op::Add { 0 } else { 1 };
        let mut konst: Option<i64> = None;
        let mut rest: Vec<(NodeId, u64)> = Vec::new();
        for &c in leaves {
            if let Payload::LitInt(v) = self.old.node(c).payload {
                konst = Some(match konst {
                    None => v,
                    Some(a) if op == Op::Add => a.wrapping_add(v),
                    Some(a) => a.wrapping_mul(v),
                });
            } else {
                rest.push(self.rewrite(c));
            }
        }
        match konst {
            // Fold pure-constant identity away only when NO real operand remains (`3*1`→3);
            // with a surviving operand keep it (`x*1` stays `x*1`) for the typed value graph.
            Some(k) if k == identity && !rest.is_empty() => {
                rest.push(self.emit_lit_int(k, span));
            }
            Some(k) if k == identity => {}
            Some(k) => rest.push(self.emit_lit_int(k, span)),
            None => {}
        }
        match rest.len() {
            0 => self.emit_lit_int(konst.unwrap_or(identity), span),
            1 => rest.into_iter().next().expect("len 1"),
            _ => self.build_assoc(op, span, rest),
        }
    }

    fn emit_lit_int(&mut self, v: i64, span: Span) -> (NodeId, u64) {
        self.emit(NodeKind::Lit, Payload::LitInt(v), span, &[], &[])
    }

    /// Flatten an assoc chain to a canonical left-leaning shape, sorting operands by hash
    /// ONLY for the genuinely-commutative ops. NOT sorted here:
    ///   • `+` — may be string/list CONCATENATION, which is non-commutative
    ///     (`"a"+"b"` ≠ `"b"+"a"`). This pass has no types, so it cannot tell concat from
    ///     numeric add; sorting reordered the pieces of every string-building expression
    ///     (`"+" + value + "\r\n"`), silently changing behavior. The value graph sorts `+`
    ///     itself, type-GATED on concat, so numeric `a+b ≡ b+a` still converges.
    ///   • logical `and`/`or` — short-circuit value-and/or is associative but NOT
    ///     commutative (`1 or 2` = 1 ≠ `2 or 1`); kept in source order, canonicalized to
    ///     the positional `Phi` by the value graph.
    ///   • Ruby `*` — string/array repetition is non-commutative in Ruby (`"ab" * 3` →
    ///     "ababab" but `3 * "ab"` raises); kept ordered, commuted type-gated by the value
    ///     graph (series 9). Python `*` repetition IS commutative and JS/Java/Go/C `*` is
    ///     numeric, so only Ruby is held.
    /// Bitwise `&`/`|`/`^` and non-Ruby `*` stay sorted (associativity is always applied).
    fn build_assoc(
        &mut self,
        op: Op,
        span: Span,
        mut operands: Vec<(NodeId, u64)>,
    ) -> (NodeId, u64) {
        let ruby_mul = op == Op::Mul
            && semantics(self.old.meta.lang)
                .operators()
                .mul_is_sequence_repetition();
        if !matches!(op, Op::And | Op::Or | Op::Add) && !ruby_mul {
            operands.sort_by_key(|&(_, h)| h);
        }
        let mut iter = operands.into_iter();
        let mut acc = iter.next().expect("binop has operands");
        for (id, h) in iter {
            acc = self.emit(
                NodeKind::BinOp,
                Payload::Op(op),
                span,
                &[acc.0, id],
                &[acc.1, h],
            );
        }
        acc
    }

    /// Return the rewritten form of `!old` (pushes negation toward the leaves).
    fn rewrite_negated(&mut self, old: NodeId) -> (NodeId, u64) {
        let node = *self.old.node(old);
        let span = node.span;
        match node.kind {
            NodeKind::UnOp if node.payload == Payload::Op(Op::Not) => {
                // `!!x` is `bool(x)` (truthiness coercion), NOT `x` — cancelling it here is
                // unsound for non-bool `x` (`!!5` = true ≠ 5; this silently merged a
                // `return !!x` with an identity `return x`). Preserve the double negation;
                // the value graph cancels `!(!x) → x` ONLY when `x` is provably Bool.
                self.negate_wrap(old, span)
            }
            NodeKind::BinOp => {
                let op = match node.payload {
                    Payload::Op(o) => o,
                    _ => return self.negate_wrap(old, span),
                };
                let kids = self.old.children(old).to_vec();
                let operators = semantics(self.old.meta.lang).operators();
                match op {
                    Op::And | Op::Or => {
                        // De Morgan: negate each (flattened) operand, flip the op.
                        let flip = if op == Op::And { Op::Or } else { Op::And };
                        let mut olds = Vec::new();
                        self.collect_assoc_old(old, op, &mut olds);
                        let negated: Vec<(NodeId, u64)> =
                            olds.into_iter().map(|o| self.rewrite_negated(o)).collect();
                        self.build_assoc(flip, span, negated)
                    }
                    Op::Eq | Op::Ne | Op::Lt | Op::Le | Op::Gt | Op::Ge => {
                        // Equality negation is value-independent. Order-comparison duals
                        // need total-order proof because NaN makes `!(a < b)` differ from
                        // `a >= b`; the value graph handles those when operands prove integer.
                        if matches!(op, Op::Lt | Op::Le | Op::Gt | Op::Ge) {
                            return self.negate_wrap(old, span);
                        }
                        let Some(contract) = operators.canonical_negated_comparison(op) else {
                            return self.negate_wrap(old, span);
                        };
                        if matches!(contract.output, Op::Eq | Op::Ne)
                            && operators
                                .comparison_law(ComparisonLaw::EqualityCommutativity)
                                .is_some()
                        {
                            return self.emit_commutative_cmp(
                                contract.output,
                                span,
                                kids[0],
                                kids[1],
                            );
                        }
                        let (first, second) = if contract.swap_operands {
                            (kids[1], kids[0])
                        } else {
                            (kids[0], kids[1])
                        };
                        let (a, ah) = self.rewrite(first);
                        let (b, bh) = self.rewrite(second);
                        self.emit(
                            NodeKind::BinOp,
                            Payload::Op(contract.output),
                            span,
                            &[a, b],
                            &[ah, bh],
                        )
                    }
                    _ => self.negate_wrap(old, span),
                }
            }
            _ => self.negate_wrap(old, span),
        }
    }

    /// Fallback: build `Not(rewrite(old))`.
    fn negate_wrap(&mut self, old: NodeId, span: Span) -> (NodeId, u64) {
        let (c, ch) = self.rewrite(old);
        self.emit(NodeKind::UnOp, Payload::Op(Op::Not), span, &[c], &[ch])
    }

    /// Collect the *old* operand node ids of an assoc-comm chain (no rewrite).
    fn collect_assoc_old(&self, old_id: NodeId, op: Op, out: &mut Vec<NodeId>) {
        for c in self.old.children(old_id) {
            let same = self.old.kind(*c) == NodeKind::BinOp
                && self.old.node(*c).payload == Payload::Op(op);
            if same {
                self.collect_assoc_old(*c, op, out);
            } else {
                out.push(*c);
            }
        }
    }

    /// Whether any leaf of an `Add`/`Mul` chain is FLOAT — a float literal, a `/`
    /// (true-division) result, a FLOAT-TYPED param (`double`/`f64`/`float64`/`: float`/…), or
    /// a sign-flip of one — so the chain must not be reassociated (#283 C-float; float
    /// `+`/`*` is non-associative). Mirrors the value graph's `proven_float` at the IL level
    /// so the two layers agree. The fully-untyped case (no float evidence anywhere) is
    /// undecidable here and still flattens (the residual Float value kind, §3.3).
    fn chain_has_float(&self, leaves: &[NodeId]) -> bool {
        leaves.iter().any(|&n| self.il_node_is_float(n))
    }

    fn il_node_is_float(&self, n: NodeId) -> bool {
        match self.old.node(n).payload {
            Payload::LitFloat(_) => true,
            Payload::Op(Op::TrueDiv) => true,
            Payload::Cid(c) if self.old.kind(n) == NodeKind::Var => {
                self.float_param_cids.contains(&c)
            }
            // `-x` / `+x` is float iff `x` is (mirrors `proven_float`'s unary recursion).
            Payload::Op(Op::Neg | Op::Pos) => self
                .old
                .children(n)
                .first()
                .is_some_and(|&child| self.il_node_is_float(child)),
            _ => false,
        }
    }
}
