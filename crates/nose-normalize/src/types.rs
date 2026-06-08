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
///
/// Runs to a FIXPOINT: each pass collects evidence using the *result type* of sibling
/// subexpressions (not just literals), so a chain like `(x + 1) * 2` proves `x : Num`
/// (the `*` makes `x + 1` numeric, which back-propagates to `x`), and `a + b*c` proves
/// `a : Num`. New param types learned in one pass let the next pass type more siblings.
/// Sound by construction: a type is recorded only when an operator *requires* it (the
/// code would error otherwise), exactly the assumption the single-op evidence already made.
pub(crate) fn infer_param_types(il: &Il, root: NodeId) -> Vec<Ty> {
    if il.kind(root) != NodeKind::Func {
        return Vec::new();
    }
    let mut params: Vec<u32> = Vec::new();
    for &k in il.children(root) {
        if il.kind(k) == NodeKind::Param {
            if let Payload::Cid(c) = il.node(k).payload {
                params.push(c);
            }
        }
    }
    let cid_of = |n: NodeId, il: &Il| -> Option<u32> {
        if il.kind(n) == NodeKind::Var {
            if let Payload::Cid(c) = il.node(n).payload {
                return Some(c);
            }
        }
        None
    };
    let mut ev: FxHashMap<u32, Ty> = FxHashMap::default();
    // Fixpoint: keep collecting evidence until no param type changes. The lattice only
    // moves up (toward `Unknown` via `join`) or assigns a fresh type, so this terminates
    // in at most `params + 1` passes; the bound also guards against any surprise.
    for _ in 0..params.len() + 1 {
        let mut next = ev.clone();
        let add = |cid: u32, t: Ty, ev: &mut FxHashMap<u32, Ty>| {
            ev.entry(cid).and_modify(|e| *e = e.join(t)).or_insert(t);
        };
        let mut stack = vec![root];
        while let Some(n) = stack.pop() {
            let kids = il.children(n).to_vec();
            match il.node(n).kind {
                NodeKind::BinOp => {
                    if let Payload::Op(op) = il.node(n).payload {
                        if is_strict_numeric(op) && kids.len() == 2 {
                            for &k in &kids {
                                if let Some(c) = cid_of(k, il) {
                                    add(c, Ty::Num, &mut next);
                                }
                            }
                        } else if op == Op::Add && kids.len() == 2 {
                            // `+` is numeric-add or concat; disambiguate from the sibling's
                            // *result* type (a `Num`/`Str` sibling forces the same on a Var).
                            let kt = [result_ty(il, kids[0], &ev), result_ty(il, kids[1], &ev)];
                            for i in 0..2 {
                                if let Some(c) = cid_of(kids[i], il) {
                                    if matches!(kt[1 - i], Ty::Num | Ty::Str) {
                                        add(c, kt[1 - i], &mut next);
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
                                add(c, Ty::Num, &mut next);
                            }
                        }
                    }
                }
                NodeKind::Index => {
                    // base[idx]: the index is numeric.
                    if let Some(c) = kids.get(1).and_then(|&k| cid_of(k, il)) {
                        add(c, Ty::Num, &mut next);
                    }
                }
                _ => {}
            }
            stack.extend(kids);
        }
        if next == ev {
            break;
        }
        ev = next;
    }
    params
        .iter()
        .map(|c| *ev.get(c).unwrap_or(&Ty::Unknown))
        .collect()
}

/// Conservative *result* type of an arbitrary subexpression under the current parameter
/// evidence `ev`. Returns a concrete type only when the operator pins it down; otherwise
/// `Unknown`. Sound: every arm reflects what the op *must* produce when it does not error.
/// Used both for `+` disambiguation above and as the bottom-up type of value-graph leaves.
pub(crate) fn result_ty(il: &Il, n: NodeId, ev: &FxHashMap<u32, Ty>) -> Ty {
    match il.node(n).kind {
        NodeKind::Lit => node_lit_ty(il, n).unwrap_or(Ty::Unknown),
        NodeKind::Var => match il.node(n).payload {
            Payload::Cid(c) => *ev.get(&c).unwrap_or(&Ty::Unknown),
            _ => Ty::Unknown,
        },
        NodeKind::Seq => Ty::List,
        NodeKind::UnOp => match il.node(n).payload {
            Payload::Op(Op::Neg) | Payload::Op(Op::Pos) | Payload::Op(Op::BitNot) => Ty::Num,
            Payload::Op(Op::Not) => Ty::Bool,
            _ => Ty::Unknown,
        },
        NodeKind::BinOp => {
            let kids = il.children(n);
            if let Payload::Op(op) = il.node(n).payload {
                if is_strict_numeric(op) {
                    Ty::Num
                } else if matches!(op, Op::Lt | Op::Le | Op::Gt | Op::Ge | Op::Eq | Op::Ne) {
                    Ty::Bool
                } else if op == Op::Add && kids.len() == 2 {
                    // numeric-add or concat: known only if a side's type is known.
                    let (a, b) = (result_ty(il, kids[0], ev), result_ty(il, kids[1], ev));
                    if a == Ty::Num && b == Ty::Num {
                        Ty::Num
                    } else if a == Ty::Str || b == Ty::Str {
                        Ty::Str
                    } else if a == Ty::List || b == Ty::List {
                        Ty::List
                    } else {
                        Ty::Unknown
                    }
                } else {
                    // And/Or short-circuit to an operand VALUE, not a Bool — leave Unknown.
                    Ty::Unknown
                }
            } else {
                Ty::Unknown
            }
        }
        NodeKind::Call => match il.node(n).payload {
            // `len(x)` is numeric; string/collection predicates are boolean.
            // Other builtins/calls are not pinned down here.
            Payload::Builtin(nose_il::Builtin::Len | nose_il::Builtin::UnsignedCast32) => Ty::Num,
            Payload::Builtin(
                nose_il::Builtin::IsEmpty
                | nose_il::Builtin::IsNull
                | nose_il::Builtin::IsNotNull
                | nose_il::Builtin::StartsWith
                | nose_il::Builtin::EndsWith
                | nose_il::Builtin::Contains,
            ) => Ty::Bool,
            Payload::Builtin(nose_il::Builtin::Join) => Ty::Str,
            _ => Ty::Unknown,
        },
        _ => Ty::Unknown,
    }
}

/// The type of a node IF it is a literal of known type, else `None` (used to type the
/// sibling of a `+`).
fn node_lit_ty(il: &Il, n: NodeId) -> Option<Ty> {
    if il.kind(n) != NodeKind::Lit {
        return None;
    }
    match il.node(n).payload {
        // A retained float (`LitFloat`) is as numeric as a `LitInt`; omitting it left
        // `x + 3.14` unable to type `x`.
        Payload::LitInt(_) | Payload::LitFloat(_) => Some(Ty::Num),
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

#[cfg(test)]
mod tests {
    use super::*;
    use nose_il::{FileId, FileMeta, IlBuilder, Lang, Span};

    /// Infer the parameter types of `fn(x) { return x + <lit> }`.
    fn infer_with_added_literal(lit: Payload) -> Vec<Ty> {
        let sp = Span::synthetic(FileId(0));
        let mut b = IlBuilder::new(FileId(0));
        let param = b.add(NodeKind::Param, Payload::Cid(0), sp, &[]);
        let varx = b.add(NodeKind::Var, Payload::Cid(0), sp, &[]);
        let lit = b.add(NodeKind::Lit, lit, sp, &[]);
        let add = b.add(NodeKind::BinOp, Payload::Op(Op::Add), sp, &[varx, lit]);
        let ret = b.add(NodeKind::Return, Payload::None, sp, &[add]);
        let func = b.add(NodeKind::Func, Payload::None, sp, &[param, ret]);
        let il = b.finish(
            func,
            FileMeta {
                path: "t".into(),
                lang: Lang::Python,
            },
            Vec::new(),
            Vec::new(),
        );
        infer_param_types(&il, func)
    }

    #[test]
    fn float_literal_sibling_of_add_types_param_as_num() {
        // `x + 3.14` (a retained-float `LitFloat`) must type `x` as `Num`, exactly like
        // `x + 3` (`LitInt`). `node_lit_ty` previously omitted `LitFloat`, leaving the
        // sibling `Unknown`.
        assert_eq!(infer_with_added_literal(Payload::LitInt(3)), vec![Ty::Num]);
        assert_eq!(
            infer_with_added_literal(Payload::LitFloat(0xBEEF)),
            vec![Ty::Num]
        );
    }
}
