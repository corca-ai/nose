//! Declarative (HTML) fingerprint: the value-graph bypass for markup units.
//!
//! HTML is declarative — an element's meaning is its **rendered DOM** — so markup units
//! are NOT lowered through the imperative value graph. The exact `semantic` fingerprint
//! of an `HtmlElement` is the canonical DOM of its subtree, computed here and dispatched
//! from [`crate::value_graph::api`] by the unit-root kind.
//!
//! Soundness (*equal fingerprint ⟹ equal rendered DOM*) rests on:
//!
//! 1. **Domain namespacing** ([`HTML_DOMAIN`]) — an HTML fingerprint can never equal a
//!    CSS or imperative one.
//! 2. **A collision-resistant recursive subtree hash** combining the (lowercased) tag,
//!    the attribute SET (order-independent — DOM attribute order is insignificant), and
//!    the ORDERED child sequence (child order IS significant). The root subtree hash is
//!    tagged distinctly ([`TAG_ROOT`]), so two equal fingerprints must share a root hash
//!    and hence an identical DOM.
//! 3. Sound value canonicalization: a boolean attribute (`disabled`) ≡ its empty-valued
//!    form (`disabled=""`); a `class` attribute is a token SET (order-insensitive);
//!    whitespace is collapsed (done in the frontend). Anything else keeps its exact
//!    value/text, so a content difference never merges.
//!
//! The multiset also carries every descendant's subtree hash (tagged [`TAG_NODE`]) so
//! the structural `near` channel can score partial markup similarity.

use nose_il::{stable_symbol_hash, Il, NodeId, NodeKind, Payload};

/// Mixed into every HTML value hash — disjoint from CSS and the imperative value graph.
const HTML_DOMAIN: u64 = 0x4854_4d4c_0000_0001; // "HTML" bytes

const TAG_ROOT: u64 = 1;
const TAG_NODE: u64 = 2;
const TAG_TEXT: u64 = 3;

fn combine(a: u64, b: u64) -> u64 {
    (a.rotate_left(7) ^ b).wrapping_mul(0x100_0000_01b3)
}

fn tagged(tag: u64, h: u64) -> u64 {
    combine(HTML_DOMAIN ^ combine(tag, 0x9E37_79B9_7F4A_7C15), h)
}

/// The declarative fingerprint of an HTML markup unit: `(value, lits, returns)` sorted
/// multisets, or `None` when `root` is not an `HtmlElement`.
pub(crate) fn declarative_fingerprint(
    il: &Il,
    root: NodeId,
    interner: &nose_il::Interner,
) -> Option<(Vec<u64>, Vec<u64>, Vec<u64>)> {
    if il.kind(root) != NodeKind::HtmlElement {
        return None;
    }
    let mut value = Vec::new();
    let mut lits = Vec::new();
    let root_hash = subtree_hash(il, interner, root, &mut value, &mut lits);
    value.push(tagged(TAG_ROOT, root_hash));
    let returns = vec![tagged(TAG_ROOT, root_hash)];
    value.sort_unstable();
    lits.sort_unstable();
    Some((value, lits, returns))
}

/// The collision-resistant DOM hash of the subtree at `node`. Side effect: records each
/// element/text descendant's subtree hash into `value` (tagged [`TAG_NODE`], for the
/// near channel) and each text/attribute-value hash into `lits`.
fn subtree_hash(
    il: &Il,
    interner: &nose_il::Interner,
    node: NodeId,
    value: &mut Vec<u64>,
    lits: &mut Vec<u64>,
) -> u64 {
    match il.kind(node) {
        NodeKind::HtmlElement => {
            let tag = match il.node(node).payload {
                Payload::Name(s) => stable_symbol_hash(interner.resolve(s)),
                _ => 0,
            };
            // Attributes: order-independent → collect and sort. Child nodes: ordered.
            let mut attr_hashes = Vec::new();
            let mut child_fold = 0u64;
            for &c in il.children(node) {
                match il.kind(c) {
                    NodeKind::HtmlAttr => attr_hashes.push(attr_hash(il, interner, c, lits)),
                    NodeKind::HtmlElement | NodeKind::HtmlText | NodeKind::HtmlControl => {
                        let h = subtree_hash(il, interner, c, value, lits);
                        child_fold = combine(child_fold, h); // ordered
                    }
                    _ => {}
                }
            }
            attr_hashes.sort_unstable();
            let mut h = combine(0xE1E_0000 ^ tag, child_fold);
            for a in attr_hashes {
                h = combine(h, a);
            }
            value.push(tagged(TAG_NODE, h));
            h
        }
        NodeKind::HtmlText => {
            let t = match il.node(node).payload {
                Payload::Name(s) => stable_symbol_hash(interner.resolve(s)),
                _ => 0,
            };
            let h = combine(0x7E47, t);
            value.push(tagged(TAG_TEXT, h));
            lits.push(tagged(TAG_TEXT, h));
            h
        }
        // A control wrapper (repeat / conditional). Its hash mixes the control KIND with
        // its template children, so a `{#each}<li>` (repeat of li) can NEVER collide with a
        // single static `<li>` — the exact channel keeps a loop distinct from one element.
        NodeKind::HtmlControl => {
            let kind = match il.node(node).payload {
                Payload::Name(s) => stable_symbol_hash(interner.resolve(s)),
                _ => 0,
            };
            let mut child_fold = 0u64;
            for &c in il.children(node) {
                if matches!(
                    il.kind(c),
                    NodeKind::HtmlElement | NodeKind::HtmlText | NodeKind::HtmlControl
                ) {
                    child_fold = combine(child_fold, subtree_hash(il, interner, c, value, lits));
                }
            }
            let h = combine(0xC02_0000 ^ kind, child_fold);
            value.push(tagged(TAG_NODE, h));
            h
        }
        _ => 0,
    }
}

/// Hash of one attribute. Name is already lowercased; the value is canonicalized: a
/// boolean attribute ≡ empty value, and `class` is a token SET (order-insensitive).
fn attr_hash(il: &Il, interner: &nose_il::Interner, attr: NodeId, lits: &mut Vec<u64>) -> u64 {
    let name = match il.node(attr).payload {
        Payload::Name(s) => interner.resolve(s),
        _ => "",
    };
    // Inline `style="…"` is lowered to a CssRule child — fold in the FULL CSS
    // computed-style fingerprint so inline styles get color/shorthand/cascade canon
    // (`style="margin:0 0 0 0;color:#fff"` ≡ `style="color:white;margin:0"`).
    if let Some(&css) = il
        .children(attr)
        .iter()
        .find(|&&c| il.kind(c) == NodeKind::CssRule)
    {
        let mut h = stable_symbol_hash(name);
        if let Some((value, _, _)) = crate::css::declarative_fingerprint(il, css, interner) {
            for v in value {
                h = combine(h, v);
            }
        }
        return h;
    }
    let raw = il
        .children(attr)
        .iter()
        .find_map(|&c| match il.node(c).payload {
            Payload::Name(s) => Some(interner.resolve(s)),
            _ => None,
        })
        .unwrap_or(""); // boolean attribute → empty value
    let value = if name == "class" {
        let mut toks: Vec<&str> = raw.split_whitespace().collect();
        toks.sort_unstable();
        toks.join(" ")
    } else {
        raw.to_string()
    };
    let vh = stable_symbol_hash(&value);
    lits.push(tagged(TAG_TEXT, vh));
    combine(stable_symbol_hash(name), vh)
}
