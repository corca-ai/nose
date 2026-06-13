//! Falsification-driven distinguishing-input SEARCH for `nose verify` (#317).
//!
//! The fixed `verify_battery` is the ad-hoc half of the soundness perimeter: it only finds a
//! false merge when a HAND-CURATED row happens to feed the distinguishing input (e.g. two
//! distinct strings to two untyped params — the #283-C class). This module institutionalizes
//! that: given two fingerprint-equal units the battery found EQUAL, it SEARCHES a small,
//! value-kind-rich input domain (mixed-radix, seeded/deterministic, budget-bounded) for a row
//! on which they behave differently. A hit is a false merge the fixed battery missed.
//!
//! Scope: this runs only under `nose verify --falsify` (offline, opt-in) — the scan path and
//! the default gate are untouched. Determinism comes from the fixed pool + fixed enumeration
//! order (no RNG), so a nightly run is reproducible.

use nose_il::{Il, Interner, NodeId, NodeKind, Payload};
use nose_normalize::{run_unit, Value, F64};

/// The per-position value pool — deliberately spans every value KIND the oracle models, with
/// at least TWO distinct strings and TWO distinct lists (the combination the fixed battery
/// under-samples: it never binds two *different* strings to two params at once). Mined corpus
/// constants are appended so value-keyed branches (`x == 'ipc'`) are exercised too.
fn falsify_pool(probes: &[Value]) -> Vec<Value> {
    let mut pool = vec![
        Value::Int(0),
        Value::Int(1),
        Value::Int(-1),
        Value::Int(0xF_0000_0003), // > 2^32, exposes int32 wrap
        Value::Bool(true),
        Value::Null,
        Value::Str(vec![0x5EED_0001]),
        Value::Str(vec![0x5EED_0002]), // a SECOND, distinct string
        Value::List(vec![Value::Int(1), Value::Int(2)]),
        Value::List(vec![Value::Int(2), Value::Int(1)]), // a distinct list (same multiset)
        Value::Float(F64(1e16)),
        Value::Float(F64(-1e16)),
    ];
    pool.extend(probes.iter().cloned());
    pool
}

/// Number of `Param` children of a unit root (its arity); inputs beyond it are ignored by the
/// interpreter, so we only enumerate over this many positions.
fn arity(il: &Il, root: NodeId) -> usize {
    il.children(root)
        .iter()
        .filter(|&&c| {
            il.kind(c) == NodeKind::Param && matches!(il.node(c).payload, Payload::Cid(_))
        })
        .count()
}

/// Search for a distinguishing input for two units (their pre-canon CORE ILs + roots). Returns
/// the first row on which both interpret to DIFFERENT, non-`None` behaviors, or `None` if none
/// within `budget` rows. Mixed-radix enumeration over [`falsify_pool`] varies the lowest
/// positions fastest, so the high-value 1- and 2-arg distinguishers are covered first.
pub(crate) fn falsify_pair(
    il_a: &Il,
    root_a: NodeId,
    il_b: &Il,
    root_b: NodeId,
    interner: &Interner,
    probes: &[Value],
    budget: usize,
) -> Option<Vec<Value>> {
    let pool = falsify_pool(probes);
    let n = pool.len();
    let ar = arity(il_a, root_a).max(arity(il_b, root_b)).max(1);
    let total = n.checked_pow(ar as u32).unwrap_or(usize::MAX).min(budget);
    for e in 0..total {
        let row: Vec<Value> = (0..ar)
            .map(|j| {
                let radix = n.saturating_pow(j as u32).max(1);
                pool[(e / radix) % n].clone()
            })
            .collect();
        let (Some(ba), Some(bb)) = (
            run_unit(il_a, interner, root_a, &row),
            run_unit(il_b, interner, root_b, &row),
        ) else {
            continue;
        };
        // A symbolic disagreement is keyed on pre-canon syntax, not behavior — skip it (the
        // soundness report routes those to the advisory lane, never the hard gate).
        if ba != bb
            && !nose_normalize::behavior_has_sym(&ba)
            && !nose_normalize::behavior_has_sym(&bb)
        {
            return Some(row);
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;
    use nose_il::{FileId, FileMeta, IlBuilder, Lang, Op, Span};

    // Build `return <a op b>` (operands in `order`) as a 2-arg unit.
    fn two_arg_binop(op: Op, order: (u32, u32)) -> (Il, Interner, NodeId) {
        let interner = Interner::new();
        let sp = Span::synthetic(FileId(0));
        let mut b = IlBuilder::new(FileId(0));
        let pa = b.add(NodeKind::Param, Payload::Cid(0), sp, &[]);
        let pb = b.add(NodeKind::Param, Payload::Cid(1), sp, &[]);
        let l = b.add(NodeKind::Var, Payload::Cid(order.0), sp, &[]);
        let r = b.add(NodeKind::Var, Payload::Cid(order.1), sp, &[]);
        let bin = b.add(NodeKind::BinOp, Payload::Op(op), sp, &[l, r]);
        let ret = b.add(NodeKind::Return, Payload::None, sp, &[bin]);
        let func = b.add(NodeKind::Func, Payload::None, sp, &[pa, pb, ret]);
        let il = b.finish(
            func,
            FileMeta {
                path: "t".into(),
                lang: Lang::Python,
            },
            Vec::new(),
            Vec::new(),
        );
        (il, interner, func)
    }

    // #317 regression baseline: re-derive the #283-C distinguisher BY SEARCH. `a + b` and
    // `b + a` agree on every integer (the fixed battery's small-int rows), but DIFFER when both
    // params are distinct STRINGS (`s1·s2 != s2·s1`) — the input the fixed battery starves. The
    // search's pool carries two distinct strings, so it finds the row.
    #[test]
    fn search_finds_string_noncommutativity_distinguisher() {
        let (ia, na, ra) = two_arg_binop(Op::Add, (0, 1)); // a + b
        let (ib, _nb, rb) = two_arg_binop(Op::Add, (1, 0)); // b + a
        let row = falsify_pair(&ia, ra, &ib, rb, &na, &[], 4096)
            .expect("search must find a distinguishing input for a+b vs b+a");
        // The distinguisher is two DISTINCT non-int values (strings or lists).
        assert_ne!(row[0], row[1]);
    }

    // Genuinely-equal units (`a + b` vs `a + b`) have NO distinguisher — the search must report
    // none (no false positive), the soundness side of the engine.
    #[test]
    fn search_finds_no_distinguisher_for_identical_units() {
        let (ia, na, ra) = two_arg_binop(Op::Add, (0, 1));
        let (ib, _nb, rb) = two_arg_binop(Op::Add, (0, 1));
        assert!(falsify_pair(&ia, ra, &ib, rb, &na, &[], 4096).is_none());
    }
}
