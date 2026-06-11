//! Structural hashing utilities. Operand canonicalization for commutative
//! operators now lives in [`crate::algebra`] (which subsumes it); this module
//! retains [`subtree_hashes`] (a post-order structural fingerprint reused by
//! `cfg_norm` and the value graph) and [`node_tag`]. The arena is built
//! post-order (children precede parents), so a single forward pass suffices.

use crate::combine;
use nose_il::{Il, Interner, NodeId, NodeKind, Payload};

/// Structural hash of every node, indexed by `NodeId`. Identifier *names* are
/// hashed by their string content (via the interner) so the result is
/// reproducible across runs despite parallel interning; canonical ids are
/// alpha-invariant after the `alpha` pass has run.
pub fn subtree_hashes(il: &Il, interner: &Interner) -> Vec<u64> {
    per_node_hashes(il, interner, node_tag)
}

/// An interner-INDEPENDENT content hash of `il`'s whole tree that RETAINS literal
/// values (via [`node_tag_valued`], unlike [`subtree_hashes`] whose `node_tag`
/// erases them). The detection-unit cache keys on this: it is stable across runs
/// (no interner-relative symbol ids leak in — names/literals resolve to content
/// hashes) yet distinguishes `{…: 1}` from `{…: 9}`, so a file whose post-resolve
/// IL changed (an inlined imported literal differs) gets a different key.
pub fn valued_tree_hash(il: &Il, interner: &Interner) -> u64 {
    per_node_hashes(il, interner, node_tag_valued)
        .into_iter()
        .fold(crate::SEED, combine)
}

/// Per-node structural hashes under a chosen `(kind, payload)` tagger. The arena
/// is post-order (children precede parents), so one forward pass interns each
/// node's hash from its already-computed children — the shared core of
/// [`subtree_hashes`] (shape, value-blind) and [`valued_tree_hash`] (value-keeping).
fn per_node_hashes(
    il: &Il,
    interner: &Interner,
    tag: fn(NodeKind, Payload, &Interner) -> u64,
) -> Vec<u64> {
    let n = il.nodes.len();
    let mut hashes = vec![0u64; n];
    for i in 0..n {
        let node = il.node(NodeId(i as u32));
        let mut h = tag(node.kind, node.payload, interner);
        for &c in il.children(NodeId(i as u32)) {
            h = combine(h, hashes[c.0 as usize]);
        }
        hashes[i] = h;
    }
    hashes
}

/// A discriminant for `(kind, payload)`. Canonical ids (`Cid`) contribute their
/// value, so variable *identity* is preserved (alpha-invariantly): `x - y` and a
/// swapped-parameter `b - a` stay distinct, while two genuine clones converge.
/// Field names contribute a content hash (stable across runs), keeping
/// `obj.foo` ≠ `obj.bar`.
pub fn node_tag(kind: NodeKind, payload: Payload, interner: &Interner) -> u64 {
    let k = kind as u64;
    let p = match payload {
        Payload::None => 0,
        Payload::Op(op) => 1 + op as u64,
        Payload::Lit(c) => 100 + c as u64,
        // Retained literal values: keep the *structural* tag identical to the
        // abstract class so shape similarity is unaffected — the concrete value
        // only discriminates inside the value-graph (the behavioral signal).
        Payload::LitInt(_) => 100 + nose_il::LitClass::Int as u64,
        Payload::LitBool(_) => 100 + nose_il::LitClass::Bool as u64,
        Payload::LitStr(_) => 100 + nose_il::LitClass::Str as u64,
        Payload::LitFloat(_) => 100 + nose_il::LitClass::Float as u64,
        Payload::Builtin(b) => 400 + b as u64,
        Payload::HoF(h) => 500 + h as u64,
        Payload::Loop(l) => 600 + l as u64,
        Payload::Cid(c) => 1_000_000 + c as u64,
        Payload::Name(s) => 2_000_000 ^ interner.symbol_hash(s),
    };
    combine(k.wrapping_mul(0xF00D), p)
}

/// Like [`node_tag`] but **value-sensitive** for literals: a retained int/bool/string
/// literal contributes its concrete value, not just its abstract class. The contiguous
/// copy-paste channel uses this so two *different* data tables (e.g. distinct HTML-entity
/// or locale maps — hundreds of distinct string constants) don't collapse into one long
/// "clone" the way they do under value-abstracted tags. Identifiers still go through the
/// alpha-renamed `Cid`/content-hashed `Name` path, so genuine identifier-renamed (Type-2)
/// copies still match; only literal *data* is now discriminated, matching how a raw-token
/// copy-paste detector (jscpd) behaves.
pub fn node_tag_valued(kind: NodeKind, payload: Payload, interner: &Interner) -> u64 {
    let p = match payload {
        Payload::LitInt(v) => combine(100 + nose_il::LitClass::Int as u64, v as u64),
        Payload::LitBool(b) => combine(100 + nose_il::LitClass::Bool as u64, b as u64),
        Payload::LitStr(h) => combine(100 + nose_il::LitClass::Str as u64, h),
        Payload::LitFloat(h) => combine(100 + nose_il::LitClass::Float as u64, h),
        other => return node_tag(kind, other, interner),
    };
    combine((kind as u64).wrapping_mul(0xF00D), p)
}
