//! Lightweight, conservative type inference for the value graph — *just enough* to make
//! type-dependent canonicalizations SOUND. The untyped value graph cannot tell numeric
//! `+` (commutative) from string/list concat (non-commutative), nor whether `-(-x)` may
//! drop an observable type error; both led to false merges (§ value_graph `mk`). This
//! module recovers the minimum type information to gate those rewrites.
//!
//! The lattice is coarse — `Num | Bool | Str | List | Unknown` — and `Unknown` is the
//! safe TOP: a canonicalization fires only when the type is *proven*, never on `Unknown`.
//! Parameter types are inferred from *strictly-typed uses* (e.g. `x * 2`, `-x`, `x % p`
//! force `Num` — strings/lists don't support those ops); ambiguous or conflicting
//! evidence stays `Unknown`. Sound by construction: we never assign a type we can't justify.

use nose_il::{Il, NodeId, NodeKind, Op, Payload};
use rustc_hash::FxHashMap;

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub(crate) enum Ty {
    Num,
    Bool,
    Str,
    List,
    Unknown,
}

impl Ty {
    /// Least upper bound: equal types stay; anything else widens to `Unknown`.
    pub(crate) fn join(self, other: Ty) -> Ty {
        if self == other {
            self
        } else {
            Ty::Unknown
        }
    }
}

/// Ops that *require* numeric operands (strings/lists/bools don't support them), so a
/// variable used as their operand is provably `Num`.
fn is_strict_numeric(op: Op) -> bool {
    matches!(
        op,
        Op::Sub
            | Op::Mul
            | Op::Div
            | Op::Mod
            | Op::Pow
            | Op::BitAnd
            | Op::BitOr
            | Op::BitXor
            | Op::Shl
            | Op::Shr
    )
}

/// Infer each parameter's type from how it is USED in the body. Conservative: a param is
/// typed only when its uses give consistent evidence; otherwise `Unknown`. Returned by
/// parameter position (matching the value graph's `Input(pos)` seeding).
pub(crate) fn infer_param_types(il: &Il, root: NodeId) -> Vec<Ty> {
    let mut params: Vec<u32> = Vec::new();
    for &k in il.children(root) {
        if il.kind(k) == NodeKind::Param {
            if let Payload::Cid(c) = il.node(k).payload {
                params.push(c);
            }
        }
    }
    let mut ev: FxHashMap<u32, Ty> = FxHashMap::default();
    let add = |cid: u32, t: Ty, ev: &mut FxHashMap<u32, Ty>| {
        ev.entry(cid).and_modify(|e| *e = e.join(t)).or_insert(t);
    };
    let cid_of = |n: NodeId, il: &Il| -> Option<u32> {
        if il.kind(n) == NodeKind::Var {
            if let Payload::Cid(c) = il.node(n).payload {
                return Some(c);
            }
        }
        None
    };
    // Walk every node; record type evidence for any Var child in a strictly-typed slot.
    let mut stack = vec![root];
    while let Some(n) = stack.pop() {
        let kids = il.children(n).to_vec();
        match il.node(n).kind {
            NodeKind::BinOp => {
                if let Payload::Op(op) = il.node(n).payload {
                    if is_strict_numeric(op) && kids.len() == 2 {
                        for &k in &kids {
                            if let Some(c) = cid_of(k, il) {
                                add(c, Ty::Num, &mut ev);
                            }
                        }
                    } else if op == Op::Add && kids.len() == 2 {
                        // `+` is numeric-add or concat; disambiguate from a typed sibling.
                        let kt = [node_lit_ty(il, kids[0]), node_lit_ty(il, kids[1])];
                        for i in 0..2 {
                            if let (Some(c), Some(t)) = (cid_of(kids[i], il), kt[1 - i]) {
                                if matches!(t, Ty::Num | Ty::Str) {
                                    add(c, t, &mut ev);
                                }
                            }
                        }
                    }
                }
            }
            NodeKind::UnOp => {
                if let Payload::Op(op) = il.node(n).payload {
                    if matches!(op, Op::Neg | Op::Pos | Op::BitNot) {
                        if let Some(c) = kids.first().and_then(|&k| cid_of(k, il)) {
                            add(c, Ty::Num, &mut ev);
                        }
                    }
                }
            }
            NodeKind::Index => {
                // base[idx]: the index is numeric.
                if let Some(c) = kids.get(1).and_then(|&k| cid_of(k, il)) {
                    add(c, Ty::Num, &mut ev);
                }
            }
            _ => {}
        }
        stack.extend(kids);
    }
    params
        .iter()
        .map(|c| *ev.get(c).unwrap_or(&Ty::Unknown))
        .collect()
}

/// The type of a node IF it is a literal of known type, else `None` (used to type the
/// sibling of a `+`).
fn node_lit_ty(il: &Il, n: NodeId) -> Option<Ty> {
    if il.kind(n) != NodeKind::Lit {
        return None;
    }
    match il.node(n).payload {
        Payload::LitInt(_) => Some(Ty::Num),
        Payload::LitStr(_) => Some(Ty::Str),
        Payload::LitBool(_) => Some(Ty::Bool),
        Payload::Lit(nose_il::LitClass::Int) | Payload::Lit(nose_il::LitClass::Float) => {
            Some(Ty::Num)
        }
        Payload::Lit(nose_il::LitClass::Str) => Some(Ty::Str),
        Payload::Lit(nose_il::LitClass::Bool) => Some(Ty::Bool),
        _ => None,
    }
}
