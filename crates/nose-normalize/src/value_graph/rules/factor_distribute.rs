//! Factor a common multiplicand out of a sum of two products.
//!
//! Rule:
//! `x*f + y*f -> (x+y)*f`
//!
//! The rule fires only when both operands are 2-ary `Mul` nodes sharing exactly one factor
//! and every leaf is a proven `Num`. Distribution is unsound for the string/list
//! repetition monoid, so the type gate is part of the proof obligation.

use super::super::{Builder, ValOp, ValueId};
use crate::types::Ty;
use nose_il::Op;

pub(in super::super) fn apply(
    builder: &mut Builder<'_>,
    a: ValueId,
    b: ValueId,
) -> Option<ValueId> {
    let mul = Op::Mul as u32;
    let (a0, a1) = as_mul(builder, a)?;
    let (b0, b1) = as_mul(builder, b)?;
    // (distributed-from-a, distributed-from-b, shared factor)
    let (x, y, f) = if a0 == b0 {
        (a1, b1, a0)
    } else if a0 == b1 {
        (a1, b0, a0)
    } else if a1 == b0 {
        (a0, b1, a1)
    } else if a1 == b1 {
        (a0, b0, a1)
    } else {
        return None;
    };
    if builder.vty(x) != Ty::Num || builder.vty(y) != Ty::Num || builder.vty(f) != Ty::Num {
        return None;
    }
    let sum = builder.mk(ValOp::Bin(Op::Add as u32), vec![x, y]);
    Some(builder.mk(ValOp::Bin(mul), vec![sum, f]))
}

fn as_mul(builder: &Builder<'_>, value: ValueId) -> Option<(ValueId, ValueId)> {
    let mul = Op::Mul as u32;
    if let ValOp::Bin(op) = builder.nodes[value as usize].op {
        if op == mul && builder.nodes[value as usize].args.len() == 2 {
            let args = &builder.nodes[value as usize].args;
            return Some((args[0], args[1]));
        }
    }
    None
}
