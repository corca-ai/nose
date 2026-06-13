//! Control-flow normalization.
//!
//! - [`structure`] (a rebuild, run before `algebra`): **conjoined-guard** merging
//!   (`if a { if b { X } }` → `if a && b { X }`) and **continue-guard** unwrapping
//!   inside loop bodies (`if c { continue } S` → `if !c { S }`). The `&&`/`!` it
//!   produces are then canonicalized by `algebra` (flatten, De Morgan), so nested
//!   and flattened control styles converge.
//! - [`run`] (in-place, run last): **branch orientation** — when an `if`'s two
//!   branches could be swapped, orient equality comparisons canonically by
//!   inverting the condition. Order-comparison branch orientation is value-domain
//!   dependent (`NaN` breaks `!(a < b)` ≡ `a >= b`), so the value graph performs
//!   that rewrite only after integer-domain proof.
//!
//! proof-obligation: normalize.control_flow.guard_returns

use crate::commutative::subtree_hashes;
use nose_il::{Il, IlBuilder, Interner, NodeId, NodeKind, Op, Payload};
use nose_semantics::semantics;
use rustc_hash::FxHashMap;

// ----------------------------------------------------------------------------
// structure: conjoined-guard merge + continue-guard unwrap (a rebuild)
// ----------------------------------------------------------------------------

pub(crate) fn structure(old: &Il) -> Il {
    let unit_root_set = old.units.iter().map(|u| u.root.0).collect();
    let mut rb = SRebuilder {
        old,
        b: IlBuilder::with_capacity(old.file, old.nodes.len(), old.edges.len()),
        remap: FxHashMap::default(),
        unit_root_set,
    };
    let new_root = rb.go(old.root);
    crate::finalize_rebuild(old, &rb.remap, rb.b, new_root, old.cid_names.clone())
}

struct SRebuilder<'a> {
    old: &'a Il,
    b: IlBuilder,
    remap: FxHashMap<u32, NodeId>,
    unit_root_set: rustc_hash::FxHashSet<u32>,
}

impl SRebuilder<'_> {
    fn go(&mut self, old_id: NodeId) -> NodeId {
        let new_id = match self.old.kind(old_id) {
            NodeKind::If => self.rewrite_if(old_id),
            NodeKind::Loop => self.rewrite_loop(old_id),
            _ => self.generic(old_id),
        };
        if self.unit_root_set.contains(&old_id.0) {
            self.remap.insert(old_id.0, new_id);
        }
        new_id
    }

    crate::rebuild_generic!();

    /// `if a { if b { X } }` (no elses) → `if a && b { X }`, chained.
    fn rewrite_if(&mut self, old_id: NodeId) -> NodeId {
        if self.old.children(old_id).len() >= 3 {
            return self.generic(old_id); // has an else: don't conjoin
        }
        let mut conds_old = Vec::new();
        let mut cur = old_id;
        let final_then;
        loop {
            let k = self.old.children(cur);
            conds_old.push(k[0]);
            let then_blk = k[1];
            match self.single_if_no_else(then_blk) {
                Some(inner) => cur = inner,
                None => {
                    final_then = then_blk;
                    break;
                }
            }
        }
        if conds_old.len() == 1 {
            return self.generic(old_id); // nothing to merge
        }
        let span = self.old.node(old_id).span;
        let rconds: Vec<NodeId> = conds_old.iter().map(|&c| self.go(c)).collect();
        let cond = self.and_chain(&rconds, span);
        let then = self.go(final_then);
        self.b.add(NodeKind::If, Payload::None, span, &[cond, then])
    }

    /// If `block` is exactly `{ if … { … } }` with no else, return that inner if.
    fn single_if_no_else(&self, block: NodeId) -> Option<NodeId> {
        if self.old.kind(block) != NodeKind::Block {
            return None;
        }
        let kids = self.old.children(block);
        if kids.len() != 1 {
            return None;
        }
        let inner = kids[0];
        if self.old.kind(inner) == NodeKind::If && self.old.children(inner).len() == 2 {
            Some(inner)
        } else {
            None
        }
    }

    fn and_chain(&mut self, conds: &[NodeId], span: nose_il::Span) -> NodeId {
        let mut acc = conds[0];
        for &c in &conds[1..] {
            acc = self
                .b
                .add(NodeKind::BinOp, Payload::Op(Op::And), span, &[acc, c]);
        }
        acc
    }

    fn rewrite_loop(&mut self, old_id: NodeId) -> NodeId {
        let n = *self.old.node(old_id);
        let child_count = self.old.children(old_id).len();
        let mut new_kids = Vec::with_capacity(child_count);
        for idx in 0..child_count {
            let k = self.old.children(old_id)[idx];
            if idx + 1 == child_count && self.old.kind(k) == NodeKind::Block {
                new_kids.push(self.wrap_body(k));
            } else {
                new_kids.push(self.go(k));
            }
        }
        self.b.add(NodeKind::Loop, n.payload, n.span, &new_kids)
    }

    fn wrap_body(&mut self, block: NodeId) -> NodeId {
        let stmts = self.old.children(block).to_vec();
        let new_stmts = self.wrap_continues(&stmts);
        let span = self.old.node(block).span;
        self.b.add(NodeKind::Block, Payload::None, span, &new_stmts)
    }

    /// Within a loop body, `… if c { continue } S` → `… if !c { S }`.
    fn wrap_continues(&mut self, stmts: &[NodeId]) -> Vec<NodeId> {
        for i in 0..stmts.len() {
            if let Some(c) = self.continue_guard_cond(stmts[i]) {
                let mut out: Vec<NodeId> = stmts[..i].iter().map(|&s| self.go(s)).collect();
                let tail = self.wrap_continues(&stmts[i + 1..]);
                if !tail.is_empty() {
                    let span = self.old.node(stmts[i]).span;
                    let rc = self.go(c);
                    let notc = self
                        .b
                        .add(NodeKind::UnOp, Payload::Op(Op::Not), span, &[rc]);
                    let blk = self.b.add(NodeKind::Block, Payload::None, span, &tail);
                    let ifnode = self.b.add(NodeKind::If, Payload::None, span, &[notc, blk]);
                    out.push(ifnode);
                }
                // else: a trailing `if c { continue }` is a no-op — dropped.
                return out;
            }
        }
        stmts.iter().map(|&s| self.go(s)).collect()
    }

    /// If `stmt` is `if c { continue }` (no else), return the old condition node.
    fn continue_guard_cond(&self, stmt: NodeId) -> Option<NodeId> {
        if self.old.kind(stmt) != NodeKind::If {
            return None;
        }
        let k = self.old.children(stmt);
        if k.len() != 2 {
            return None;
        }
        let then = k[1];
        if self.old.kind(then) == NodeKind::Block {
            let tk = self.old.children(then);
            if tk.len() == 1 && self.old.kind(tk[0]) == NodeKind::Continue {
                return Some(k[0]);
            }
        }
        None
    }
}

// ----------------------------------------------------------------------------
// run: branch orientation (in-place)
// ----------------------------------------------------------------------------

pub(crate) fn run(il: &mut Il, interner: &Interner) {
    if !il
        .nodes
        .iter()
        .any(|node| node.kind == NodeKind::If && node.child_len == 3)
    {
        return;
    }
    let hashes = subtree_hashes(il, interner);
    for i in 0..il.nodes.len() {
        let node = il.nodes[i];
        if node.kind != NodeKind::If || node.child_len != 3 {
            continue;
        }
        let cs = node.child_start as usize;
        let cond = il.edges[cs];
        let then = il.edges[cs + 1];
        let els = il.edges[cs + 2];
        if hashes[then.0 as usize] > hashes[els.0 as usize] {
            if let Some((inv, swap_operands)) = invert_comparison(il, cond) {
                il.nodes[cond.0 as usize].payload = Payload::Op(inv);
                if swap_operands {
                    // Keep the comparison in canonical operand order (algebra maps
                    // `>`/`>=` to `<`/`<=` with swapped operands; the inversion must
                    // land in that same `Lt`/`Le`/`Eq`/`Ne` set, else `if a<b{…}else{…}`
                    // orients to `Ge(a,b)` while `if a>=b` canonicalizes to `Le(b,a)`
                    // and the two never converge).
                    let ccs = il.node(cond).child_start as usize;
                    il.edges.swap(ccs, ccs + 1);
                }
                il.edges.swap(cs + 1, cs + 2);
            }
        }
    }
}

/// If `cond` is an equality comparison `BinOp`, return its canonical negation as
/// `(operator, swap_operands)`. Order comparisons are deliberately excluded here:
/// they need total-order proof, which is only available in the value graph.
fn invert_comparison(il: &Il, cond: NodeId) -> Option<(Op, bool)> {
    let n = il.node(cond);
    if n.kind != NodeKind::BinOp {
        return None;
    }
    match n.payload {
        Payload::Op(op) if matches!(op, Op::Eq | Op::Ne) => semantics(il.meta.lang)
            .operators()
            .canonical_negated_comparison(op)
            .map(|contract| (contract.output, contract.swap_operands)),
        Payload::Op(_) => None,
        _ => None,
    }
}
