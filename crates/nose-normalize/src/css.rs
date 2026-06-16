//! Declarative (CSS) fingerprint: the value-graph bypass for CSS units.
//!
//! CSS is declarative — a rule's meaning is its **computed style**, not imperative
//! behavior — so CSS units are NOT lowered through the imperative value graph (GVN).
//! Instead the exact `semantic` fingerprint of a `CssRule` is the canonical multiset
//! of its declarations, computed here and dispatched from
//! [`crate::value_graph::api`] by the unit-root kind.
//!
//! Soundness (the moat's *equal fingerprint ⟹ equal behavior* contract, specialized
//! to CSS as *equal fingerprint ⟹ equal computed style*) rests on three properties of
//! this multiset:
//!
//! 1. **Domain namespacing.** Every hash is mixed with [`CSS_DOMAIN`], so a CSS
//!    fingerprint can never equal an imperative (or future HTML) one — the
//!    cross-domain false-merge guard for the otherwise language-blind exact channel.
//! 2. **Property→value binding.** Each declaration contributes a hash binding its
//!    property to its *ordered* value tokens (`margin: 1px 2px` ≠ `margin: 2px 1px`),
//!    so two rules merge only when they declare the same property/value pairs. The
//!    multiset is order-INdependent *across* declarations (`{a;b}` ≡ `{b;a}`), which
//!    is correct: declaration order does not affect computed style — EXCEPT for
//!    repeated properties, handled next.
//! 3. **Cascade (last-wins) dedup.** Within a rule, a repeated property keeps only its
//!    last declaration, so `{color:red; color:blue}` ≢ `{color:blue; color:red}`.
//!
//! Selectors are deliberately EXCLUDED from a normal rule's fingerprint: a duplicated
//! declaration block under different selectors (`.btn` vs `.cta`) is the canonical CSS
//! clone. At-rule context (`@media (...)`) IS included, so a `@media`-scoped rule does
//! not merge with an unconditional one (their computed styles differ by condition).
//!
//! Computed-equivalence: each declaration's value tokens are canonicalized by
//! [`crate::css_value`] (color → canonical hex, number/length → canonical spelling,
//! box shorthands collapsed) before hashing, so `#fff` ≡ `#ffffff` ≡ `white` and
//! `margin: 0 0 0 0` ≡ `margin: 0`. The frontend stores RAW value text (`Lit(Name)`),
//! so the computed-style oracle can re-derive computed style independently.

use nose_il::{stable_symbol_hash, Il, NodeId, NodeKind, Payload};
use rustc_hash::FxHashMap;

/// Mixed into every CSS value hash so the CSS fingerprint space is disjoint from the
/// imperative value graph's (and from any other declarative domain's).
const CSS_DOMAIN: u64 = 0xC55_0000_0000_0001;

// Distinct sub-tags keep per-declaration, property, value-token, and context hashes in
// disjoint spaces so they never alias each other inside the multiset.
const TAG_DECL: u64 = 1;
const TAG_PROP: u64 = 2;
const TAG_VAL: u64 = 3;
const TAG_CTX: u64 = 4;
const TAG_SEQ: u64 = 5;

/// Deterministic 64-bit mix (no random seed — determinism is a hard invariant).
fn combine(a: u64, b: u64) -> u64 {
    (a.rotate_left(5) ^ b).wrapping_mul(0x5851_f42d_4c95_7f2d)
}

fn tagged(tag: u64, h: u64) -> u64 {
    combine(CSS_DOMAIN ^ combine(tag, 0x9E37_79B9_7F4A_7C15), h)
}

/// The declarative fingerprint of a CSS unit: `(value, lits, returns)` sorted hash
/// multisets, or `None` when `root` is not a declarative (CSS) unit (caller then falls
/// back to the imperative value graph). `value` is the exact-channel multiset, `lits`
/// the value-token constants (for the data-table gate), `returns` the per-declaration
/// "what it computes" multiset (for the return-signature gate).
pub(crate) fn declarative_fingerprint(
    il: &Il,
    root: NodeId,
    interner: &nose_il::Interner,
) -> Option<(Vec<u64>, Vec<u64>, Vec<u64>)> {
    if il.kind(root) != NodeKind::CssRule {
        return None;
    }
    let mut value = Vec::new();
    let mut lits = Vec::new();
    let mut returns = Vec::new();
    collect_rule(il, interner, root, &mut value, &mut lits, &mut returns);
    value.sort_unstable();
    lits.sort_unstable();
    returns.sort_unstable();
    Some((value, lits, returns))
}

/// Accumulate one rule's declaration hashes. Recurses into nested rules (CSS nesting /
/// at-rule blocks) so a wrapper rule carries its nested declarations.
fn collect_rule(
    il: &Il,
    interner: &nose_il::Interner,
    rule: NodeId,
    value: &mut Vec<u64>,
    lits: &mut Vec<u64>,
    returns: &mut Vec<u64>,
) {
    let kids = il.children(rule);
    let has_nested = kids.iter().any(|&c| il.kind(c) == NodeKind::CssRule);

    // Cascade: collect declarations in source order, then keep the LAST per property
    // (last-wins). Preserve first-seen order for determinism; the multiset is sorted
    // at the end regardless. `seq` keeps EVERY declaration in source order (no dedup)
    // for the cascade-ambiguity guard below.
    let mut order: Vec<u64> = Vec::new();
    let mut last: FxHashMap<u64, DeclHashes> = FxHashMap::default();
    let mut prop_names: Vec<String> = Vec::new();
    let mut seq: Vec<u64> = Vec::new();

    for &c in kids {
        match il.kind(c) {
            NodeKind::CssDecl => {
                let prop_text = match il.node(c).payload {
                    Payload::Name(p) => interner.resolve(p),
                    _ => "",
                };
                let prop = stable_symbol_hash(prop_text);
                // Resolve the raw value tokens and canonicalize the value toward
                // computed equivalence (color/number/shorthand) before hashing.
                let raw: Vec<&str> = il
                    .children(c)
                    .iter()
                    .filter_map(|&v| match il.node(v).payload {
                        Payload::Name(s) => Some(interner.resolve(s)),
                        _ => None,
                    })
                    .collect();
                let mut decl = prop;
                let mut tokens = Vec::new();
                for tok in crate::css_value::canonicalize_value(prop_text, &raw) {
                    let th = stable_symbol_hash(&tok);
                    decl = combine(decl, th); // ordered within a declaration
                    tokens.push(th);
                }
                seq.push(decl);
                prop_names.push(prop_text.to_string());
                if last.insert(prop, DeclHashes { decl, tokens }).is_none() {
                    order.push(prop);
                }
            }
            NodeKind::CssSelector if has_nested => {
                // At-rule prelude / nesting context — keep it so a `@media`-scoped rule
                // does not merge with an unconditional one.
                if let Payload::Name(s) = il.node(c).payload {
                    let ch = tagged(TAG_CTX, stable_symbol_hash(interner.resolve(s)));
                    value.push(ch);
                    returns.push(ch);
                }
            }
            NodeKind::CssRule => collect_rule(il, interner, c, value, lits, returns),
            _ => {}
        }
    }

    for prop in order {
        let DeclHashes { decl, tokens } = &last[&prop];
        value.push(tagged(TAG_DECL, *decl));
        returns.push(tagged(TAG_DECL, *decl));
        value.push(tagged(TAG_PROP, prop));
        for &t in tokens {
            value.push(tagged(TAG_VAL, t));
            lits.push(tagged(TAG_VAL, t));
        }
    }

    // Cascade-ambiguity guard (soundness). The per-property multiset above is order-
    // INdependent, which is correct EXCEPT when a shorthand and one of its longhands
    // co-occur (`margin` + `margin-top`, `border` + `border-color`): there the cascade
    // is order-SENSITIVE (`margin:0; margin-top:5px` ≠ the reverse). Detect that purely
    // structurally — some property is a `prefix-` of another — and, when present, add an
    // order-sensitive hash of the full declaration sequence so a reorder cannot merge.
    if has_cascade_ambiguity(&prop_names) {
        let mut s = 0u64;
        for d in seq {
            s = combine(s, d);
        }
        value.push(tagged(TAG_SEQ, s));
        returns.push(tagged(TAG_SEQ, s));
    }
}

/// True if some property is a shorthand whose longhand also appears (`margin` &
/// `margin-top`, `border` & `border-color`) — i.e. their relative order can change the
/// computed style. Detected structurally (`q` starts with `p` + `-`), so it needs no
/// shorthand table and fails safe (a false positive only costs order-independence).
fn has_cascade_ambiguity(props: &[String]) -> bool {
    props.iter().any(|p| {
        props.iter().any(|q| {
            q.len() > p.len() && q.as_bytes()[p.len()] == b'-' && q.starts_with(p.as_str())
        })
    })
}

struct DeclHashes {
    decl: u64,
    tokens: Vec<u64>,
}
