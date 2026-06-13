//! Low-level value-graph op codes and tree scans.
//!
//! proof-obligation: normalize.value_graph.compare

use super::{ConstKind, ValOp};
use crate::combine;
use nose_il::{stable_symbol_hash, Il, Lang, NodeId, NodeKind, Op, Payload};
use nose_semantics::{semantics, ValueDomain};
use rustc_hash::FxHashSet;

pub(super) const PROMISE_RESOLVED_CODE: u32 = 0x5052_4F4D;

pub(super) fn collect_assigned(il: &Il, node: NodeId, out: &mut FxHashSet<u32>) {
    if il.kind(node) == NodeKind::Assign {
        if let Some(&lhs) = il.children(node).first() {
            if il.kind(lhs) == NodeKind::Var {
                if let Payload::Cid(c) = il.node(lhs).payload {
                    out.insert(c);
                }
            }
        }
    }
    for &c in il.children(node) {
        collect_assigned(il, c, out);
    }
}

pub(super) fn op_code(p: Payload) -> u32 {
    match p {
        Payload::Op(op) => op as u32,
        _ => 0,
    }
}

/// All canonical variable ids referenced anywhere in `node`'s subtree.
pub(super) fn mentioned_cids(il: &Il, node: NodeId) -> FxHashSet<u32> {
    let mut out = FxHashSet::default();
    mentioned_scan(il, node, &mut out);
    out
}

fn mentioned_scan(il: &Il, node: NodeId, out: &mut FxHashSet<u32>) {
    if il.kind(node) == NodeKind::Var {
        if let Payload::Cid(c) = il.node(node).payload {
            out.insert(c);
        }
    }
    for &c in il.children(node) {
        mentioned_scan(il, c, out);
    }
}

/// Loop induction variables: those updated by `i = i ± constant` in the body.
pub(super) fn induction_vars(il: &Il, body: NodeId) -> FxHashSet<u32> {
    let mut out = FxHashSet::default();
    induction_scan(il, body, &mut out);
    out
}

fn induction_scan(il: &Il, node: NodeId, out: &mut FxHashSet<u32>) {
    if il.kind(node) == NodeKind::Assign {
        let kids = il.children(node);
        if kids.len() == 2 && il.kind(kids[0]) == NodeKind::Var {
            if let Payload::Cid(c) = il.node(kids[0]).payload {
                if is_increment(il, kids[1], c) {
                    out.insert(c);
                }
            }
        }
    }
    for &c in il.children(node) {
        induction_scan(il, c, out);
    }
}

/// The constant step of induction variable `cid` if the body updates it *exactly once*
/// as `i = i + k` / `i = k + i` / `i = i - k` (k an int literal); else `None`. `k - i`
/// is a reflection (not a step) and is rejected, as is a variable updated 0 or >=2 times.
pub(super) fn induction_step(il: &Il, body: NodeId, cid: u32) -> Option<i64> {
    let mut step = None;
    let mut count = 0u32;
    induction_step_scan(il, body, cid, &mut step, &mut count);
    if count == 1 {
        step
    } else {
        None
    }
}

fn induction_step_scan(il: &Il, node: NodeId, cid: u32, step: &mut Option<i64>, count: &mut u32) {
    if il.kind(node) == NodeKind::Assign {
        let kids = il.children(node);
        if kids.len() == 2 && il.kind(kids[0]) == NodeKind::Var {
            if let Payload::Cid(c) = il.node(kids[0]).payload {
                if c == cid {
                    *count += 1;
                    *step = increment_amount(il, kids[1], cid);
                }
            }
        }
    }
    for &c in il.children(node) {
        induction_step_scan(il, c, cid, step, count);
    }
}

/// The signed step if `expr` is `cid + k`, `k + cid`, or `cid - k` (k an int literal);
/// `k - cid` and anything else -> `None`.
fn increment_amount(il: &Il, expr: NodeId, cid: u32) -> Option<i64> {
    if il.kind(expr) != NodeKind::BinOp {
        return None;
    }
    let kids = il.children(expr);
    if kids.len() != 2 {
        return None;
    }
    let is_self = |n: NodeId| {
        matches!(
            (il.kind(n), il.node(n).payload),
            (NodeKind::Var, Payload::Cid(c)) if c == cid
        )
    };
    let lit = |n: NodeId| match il.node(n).payload {
        Payload::LitInt(v) => Some(v),
        _ => None,
    };
    match il.node(expr).payload {
        Payload::Op(Op::Add) => {
            if is_self(kids[0]) {
                lit(kids[1])
            } else if is_self(kids[1]) {
                lit(kids[0])
            } else {
                None
            }
        }
        // Only `i - k` is a step; `k - i` reflects.
        Payload::Op(Op::Sub) if is_self(kids[0]) => lit(kids[1]).map(|v| -v),
        _ => None,
    }
}

/// Whether `expr` is `cid ± literal` — a step of the induction variable `cid`.
fn is_increment(il: &Il, expr: NodeId, cid: u32) -> bool {
    if il.kind(expr) != NodeKind::BinOp
        || !matches!(
            il.node(expr).payload,
            Payload::Op(Op::Add) | Payload::Op(Op::Sub)
        )
    {
        return false;
    }
    let mut refs_self = false;
    let mut others_literal = true;
    for &k in il.children(expr) {
        match (il.kind(k), il.node(k).payload) {
            (NodeKind::Var, Payload::Cid(c)) if c == cid => refs_self = true,
            (NodeKind::Lit, _) => {}
            _ => others_literal = false,
        }
    }
    refs_self && others_literal
}

/// The complementary comparison op code, if `opc` is a comparison; else `None`.
pub(super) fn negate_cmp_code(lang: Lang, opc: u32) -> Option<u32> {
    let op = op_from_code(opc)?;
    semantics(lang)
        .operators()
        .comparison_complement(op)
        .map(|contract| contract.output as u32)
}

/// The same comparison with operands swapped: `a < b` becomes `b > a`.
pub(super) fn reverse_cmp_code(lang: Lang, opc: u32) -> Option<u32> {
    let op = op_from_code(opc)?;
    semantics(lang)
        .operators()
        .comparison_reverse(op)
        .map(|contract| contract.output as u32)
}

pub(super) fn op_from_code(opc: u32) -> Option<Op> {
    const OPS: &[Op] = &[
        Op::Add,
        Op::Sub,
        Op::Mul,
        Op::Div,
        Op::FloorDiv,
        Op::TrueDiv,
        Op::Mod,
        Op::FloorMod,
        Op::Pow,
        Op::Eq,
        Op::Ne,
        Op::Lt,
        Op::Le,
        Op::Gt,
        Op::Ge,
        Op::In,
        Op::And,
        Op::Or,
        Op::Not,
        Op::BitAnd,
        Op::BitOr,
        Op::BitXor,
        Op::Shl,
        Op::Shr,
        Op::BitNot,
        Op::Neg,
        Op::Pos,
    ];
    OPS.iter().copied().find(|op| *op as u32 == opc)
}

pub(super) fn is_commutative(opc: u32) -> bool {
    is_assoc_comm_code(opc) || opc == Op::Eq as u32 || opc == Op::Ne as u32
}

/// Coarse value domain of a `Const`, read directly from its explicit [`ConstKind`] (no
/// numeric-range inference — that packing was the source of the #308/series-8 kind
/// misclassifications). `Null` and sentinels are behaviorally `Unknown`.
pub(super) fn const_value_domain(kind: ConstKind) -> ValueDomain {
    match kind {
        ConstKind::Int | ConstKind::Float => ValueDomain::Number,
        ConstKind::Str => ValueDomain::String,
        ConstKind::Bool => ValueDomain::Boolean,
        ConstKind::Null | ConstKind::Sentinel => ValueDomain::Unknown,
    }
}

/// Associative *and* commutative operators (flatten-eligible).
pub(super) fn is_assoc_comm_code(opc: u32) -> bool {
    // NOTE: logical `And`/`Or` are deliberately ABSENT — short-circuit value-and/or is
    // associative but NOT commutative (`1 or 2` != `2 or 1`; it returns the deciding
    // operand's value). Treating them as commutative here swapped their operands by hash
    // and silently merged `a or b` with `b or a`.
    opc == Op::Add as u32
        || opc == Op::Mul as u32
        || opc == Op::BitAnd as u32
        || opc == Op::BitOr as u32
        || opc == Op::BitXor as u32
        // MIN/MAX (synthesized from ternaries by `minmax_pattern`) are
        // associative AND commutative on the ternary semantics — for ALL inputs,
        // including floats/NaN, because `x if x<y else y` is total (coevo §CE /
        // #284). Flattening nested min/max converges `max(max(a,b),c)` with
        // `max(a,max(b,c))`. They were already in `is_commutative` (per-level
        // operand sorting); this lets the chain flatten across levels too.
        || opc == MIN_CODE
        || opc == MAX_CODE
}

/// `Reduce` op codes for the selection reductions (min/max). Kept clear of the small
/// `Op` discriminants (used for `+`/`*` folds) and of the `Const` int range.
pub(super) const REDUCE_MAX: u32 = 0xFF00;
pub(super) const REDUCE_MIN: u32 = 0xFF01;
/// `Reduce` op codes for the boolean short-circuit reductions: `any`/`some` (existential
/// OR) and `all`/`every` (universal AND). `REDUCE_ALL == REDUCE_ANY + 1`.
pub(super) const REDUCE_ANY: u32 = 0xFF02;
pub(super) const REDUCE_ALL: u32 = 0xFF03;
pub(super) const ORDERED_STRING_JOIN: u32 = 0xFF04;

/// `Un` op code for absolute value — `abs(x)` and the `x if x>=0 else -x` idiom both
/// canonicalize to `Un(ABS_CODE, [x])`. Clear of the small `Op` discriminants.
pub(super) const ABS_CODE: u32 = 0xAB5;
/// Pseudo-ops for the 2-way min/max idiom (`x if x<y else y` == `min(x,y)`), clear of the
/// `Op` discriminants and `ABS_CODE`. Commutative (min/max are symmetric).
pub(super) const MIN_CODE: u32 = 0x319;
pub(super) const MAX_CODE: u32 = 0x32A;
pub(super) const JS_PROTOTYPE_IN_CODE: u32 = 0x4A53_494E;
/// `Un` op code for the JS `ToInt32` coercion that every JS bitwise operator applies to
/// its operands (`a & b` is `ToInt32(a) & ToInt32(b)`, the result an int32). Wrapping the
/// LEAF operands of a JS-family bitwise expression in this gives JS bitwise a fingerprint
/// distinct from arbitrary-precision (`Python`/`Ruby`) bitwise — closing the cross-language
/// false merge (`2^40 & 2^40` is `0` in JS, `2^40` in Python; #283-D). The bitwise op
/// STRUCTURE is preserved (only leaves are wrapped, intermediate results are already
/// int32), so the De Morgan / idempotence / byte-pack canons keep matching. Clear of the
/// `Op` discriminants and the other synthesized `Un` codes.
pub(super) const TO_INT32_CODE: u32 = 0x132;
/// Salt for a strict (`===`/`!==`) comparison against a null-ish operand — a shape
/// the null/undefined-conflating value model cannot express as `Bin(Eq)` without
/// merging it with the loose check (see `eval`'s strict-null handling).
pub(super) const JS_STRICT_NULL_CMP_TAG: u64 = 0x4A53_534E_4351;
/// Salt for JS loose equality outside the nullish special case. `x == null` is modeled
/// by the null/undefined-conflating `Eq` because that is exactly the nullish-default
/// check; other loose equality uses JS coercions and must not merge with `===`.
pub(super) const JS_LOOSE_EQ_CMP_TAG: u64 = 0x4A53_4C45_4351;
pub(super) const JS_LOOSE_NE_CMP_TAG: u64 = 0x4A53_4C4E_4351;
/// Salt for JS `instanceof`. The frontend lowers it through an equality-shaped
/// BinOp so structural consumers can still see a binary predicate, but the
/// value graph must keep prototype-chain type membership distinct from equality.
pub(super) const JS_INSTANCEOF_CMP_TAG: u64 = 0x4A53_494F_4351;
/// Salt for JS-family relational comparison (`< <= > >=`) when operands are not
/// proven numeric. JS compares two strings lexicographically but otherwise applies
/// numeric coercion, so untyped relational predicates cannot share the primitive
/// numeric comparison node safely.
pub(super) const JS_RELATIONAL_CMP_TAG: u64 = 0x4A53_5243_4D50;
pub(super) const C_U16_BE_BYTE_PACK_CODE: u32 = 0x4331_3642;
pub(super) const C_U32_BE_BYTE_PACK_CODE: u32 = 0x4333_3242;
pub(super) const EFFECT_ORDINAL_SINK_TAG: u64 = 0xEFFE_C701;

/// A selection reduction (min/max) keeps no additive/multiplicative identity, so its
/// `Reduce` carries only the per-element contribution (no init).
pub(super) fn is_selection_code(op: u32) -> bool {
    op == REDUCE_MAX || op == REDUCE_MIN || op == REDUCE_ANY || op == REDUCE_ALL
}

/// The identity element of a reduction operator (`acc op identity = acc`), used to
/// neutralize a filtered-out element in a guarded reduction.
pub(super) fn identity_of(opc: u32) -> Option<u32> {
    if opc == Op::Add as u32 {
        Some(0)
    } else if opc == Op::Mul as u32 {
        Some(1)
    } else {
        None
    }
}

pub(super) fn op_tag(op: &ValOp) -> u64 {
    let (k, p): (u64, u64) = match op {
        ValOp::Input(c) => (1, *c as u64),
        // Fold the kind into the discriminant so two Consts of different kinds with the
        // same `bits` (e.g. Int 1 vs Bool true) never share a hash.
        ValOp::Const { kind, bits } => (2 ^ ((*kind as u64) << 8), *bits),
        ValOp::Bin(o) => (3, *o as u64),
        ValOp::Un(o) => (4, *o as u64),
        ValOp::Field(n) => (5, *n),
        ValOp::Index => (6, 0),
        ValOp::Call(t) => (7, *t as u64),
        ValOp::KwArg(n) => (23, *n),
        ValOp::Hof(h) => (8, *h as u64),
        ValOp::Clamp => (20, 0),
        ValOp::Seq(t) => (9, *t),
        ValOp::ImportNamespace { module_hash } => (21, *module_hash),
        ValOp::ImportBinding {
            module_hash,
            exported_hash,
        } => (22, combine(*module_hash, *exported_hash)),
        ValOp::CollectionParam => (17, 0),
        ValOp::ArrayParam => (18, 0),
        ValOp::StringParam => (19, 0),
        ValOp::Phi => (10, 0),
        ValOp::Lambda(h) => (11, *h),
        ValOp::Loop(c) => (12, *c as u64),
        ValOp::Elem(h) => (14, *h),
        ValOp::Reduce(o) => (15, *o as u64),
        ValOp::Idx(h) => (16, *h),
        ValOp::Formula(h) => (19, *h),
        ValOp::Recurrence(h) => (18, *h),
        ValOp::Opaque(c) => (13, *c),
    };
    combine(k.wrapping_mul(0xF00D), p)
}

/// The low-bit mask for a literal's content hash inside its class-tagged `Const` key
/// The `Const` `bits` for a string literal: the FULL content hash. The kind
/// (`ConstKind::Str`) is carried separately, so unlike the old packed key there is no
/// range to escape and no truncation — the #308 mask and its 28-bit collision are gone
/// (coevo series 8). The frontend's `LitStr` and the synthesized empty string both go
/// through `stable_symbol_hash`, so they agree.
pub(super) fn stable_string_const_bits(value: &str) -> u64 {
    stable_symbol_hash(value)
}

pub(super) fn stable_float_const_bits(value: &str) -> u64 {
    let normalized = value.trim().trim_end_matches(['f', 'F', 'd', 'D']);
    stable_symbol_hash(normalized)
}
