//! HTML markup → declarative IL lowering.
//!
//! HTML is *declarative*: an element's meaning is the **rendered DOM** it produces, not
//! imperative behavior. So markup is NOT lowered through the imperative value graph —
//! each `HtmlElement` subtree is a detection unit whose exact `semantic` fingerprint is
//! the canonical DOM of that subtree (`nose-normalize::html`, dispatched in
//! `value_graph::api` by the unit-root kind). The `<script>`/`<style>` *internals* are
//! NOT lowered here — they are analyzed as their own JS/CSS regions (see `embedded.rs`);
//! this frontend keeps only their element shells (tag + attributes).
//!
//! Shape: `document` → a `Module` of `HtmlElement`s; each element is
//! `HtmlElement(tag)[ HtmlAttr(name)[Lit(value)?]..., (child element | HtmlText)... ]`.
//! A `.vue`/`.svelte` file parses as HTML too, so its `<template>` markup is lowered the
//! same way. Anything unrecognized becomes `Raw` (no panics).

use crate::lower::Lowering;
use nose_il::{FileId, Il, Interner, Lang, NodeId, NodeKind, Payload, Span, Symbol, UnitKind};
use tree_sitter::Node as TsNode;

pub(crate) fn lower(
    file: FileId,
    path: &str,
    src: &[u8],
    interner: &Interner,
) -> anyhow::Result<Il> {
    crate::lower::lower_file(
        file,
        path,
        src,
        interner,
        crate::lower::grammar::HTML,
        || tree_sitter_html::LANGUAGE.into(),
        Lang::Html,
        lower_document,
    )
}

fn lower_document(lo: &mut Lowering, root: TsNode) -> NodeId {
    let span = lo.span(root);
    let mut kids = Vec::new();
    collect_nodes(lo, root, &mut kids, false);
    lo.add(NodeKind::Module, Payload::None, span, &kids)
}

fn collect_nodes(lo: &mut Lowering, node: TsNode, out: &mut Vec<NodeId>, pre: bool) {
    for c in Lowering::named_children(node) {
        if let Some(id) = lower_node(lo, c, pre) {
            out.push(id);
        }
    }
}

/// `pre`: are we inside a whitespace-PRESERVING element (`<pre>`/`<textarea>`)? There
/// the renderer keeps whitespace verbatim, so collapsing it would merge DOM-distinct
/// blocks — a false merge. Elsewhere flow whitespace is insignificant and collapsed.
fn lower_node(lo: &mut Lowering, node: TsNode, pre: bool) -> Option<NodeId> {
    match node.kind() {
        "element" => Some(lower_element(lo, node, false, pre)),
        // Script/style elements: keep the shell (tag + attrs), drop the raw_text body
        // (the JS/CSS is analyzed separately as its own region).
        "script_element" | "style_element" => Some(lower_element(lo, node, true, pre)),
        // Text and entities (`&amp;`, `&nbsp;`) are both content — fold to HtmlText.
        "text" | "entity" => lower_text(lo, node, pre),
        "doctype" | "comment" | "erroneous_end_tag" => None,
        other => {
            let s = lo.span(node);
            Some(lo.raw(other, s, &[]))
        }
    }
}

/// Lower an element subtree → `HtmlElement(tag)`. When `raw_content`, child content is
/// dropped (script/style bodies). Every element registers as a detection unit; the size
/// gate keeps trivial single elements from matching.
fn lower_element(lo: &mut Lowering, node: TsNode, raw_content: bool, pre: bool) -> NodeId {
    let span = lo.span(node);
    let mut children = Vec::new();
    let mut tag = None;
    // Whitespace is significant inside this element if we are already in a preformatted
    // context OR this element is one (`<pre>`/`<textarea>`). The start tag is the first
    // child, so `child_pre` is set before any text/child element is lowered.
    let mut child_pre = pre;
    for c in Lowering::named_children(node) {
        match c.kind() {
            "start_tag" | "self_closing_tag" => {
                let (t, attrs) = lower_tag(lo, c);
                if tag.is_none() {
                    tag = t;
                    if let Some(sym) = tag {
                        child_pre = pre || matches!(lo.interner.resolve(sym), "pre" | "textarea");
                    }
                }
                children.extend(attrs);
            }
            "end_tag" | "raw_text" => {}
            _ if !raw_content => {
                if let Some(id) = lower_node(lo, c, child_pre) {
                    children.push(id);
                }
            }
            _ => {}
        }
    }
    let tag_sym = tag.unwrap_or_else(|| lo.sym(""));
    let el = lo.add(
        NodeKind::HtmlElement,
        Payload::Name(tag_sym),
        span,
        &children,
    );
    lo.push_unit(el, UnitKind::Block, tag);
    el
}

/// Extract `(tag, attributes)` from a `start_tag` / `self_closing_tag`.
fn lower_tag(lo: &mut Lowering, tag_node: TsNode) -> (Option<Symbol>, Vec<NodeId>) {
    let mut tag = None;
    let mut attrs = Vec::new();
    for c in Lowering::named_children(tag_node) {
        match c.kind() {
            "tag_name" if tag.is_none() => {
                let name = lo.text(c).to_ascii_lowercase();
                tag = Some(lo.sym(&name));
            }
            "attribute" => attrs.push(lower_attr(lo, c)),
            _ => {}
        }
    }
    (tag, attrs)
}

/// `name="value"` → `HtmlAttr(name)[Lit(Name=raw value)]`; a boolean attribute has no
/// value child. The name is lowercased (HTML attribute names are case-insensitive); the
/// value keeps its raw text so the DOM fingerprint and a checker can normalize
/// independently.
fn lower_attr(lo: &mut Lowering, node: TsNode) -> NodeId {
    let span = lo.span(node);
    let mut name = None;
    let mut value = None;
    for c in Lowering::named_children(node) {
        match c.kind() {
            "attribute_name" if name.is_none() => name = Some(lo.text(c).to_ascii_lowercase()),
            "quoted_attribute_value" => {
                let inner = Lowering::named_children(c)
                    .into_iter()
                    .find(|x| x.kind() == "attribute_value")
                    .map(|x| lo.text(x))
                    .unwrap_or("");
                value = Some(inner.to_string());
            }
            "attribute_value" if value.is_none() => value = Some(lo.text(c).to_string()),
            _ => {}
        }
    }
    let name = canonical_attr_name(&name.unwrap_or_default());
    // Inline `style="…"` is a CSS declaration block — lower it as a (selector-less)
    // `CssRule` child so the markup fingerprint reuses the full CSS computed-style
    // canonicalization (color/shorthand/unit/cascade) for it.
    if name == "style" {
        let rule = lower_inline_style(lo, value.as_deref().unwrap_or(""), span);
        let nsym = lo.sym(&name);
        return lo.add(NodeKind::HtmlAttr, Payload::Name(nsym), span, &[rule]);
    }
    let nsym = lo.sym(&name);
    let children: Vec<NodeId> = match value {
        Some(v) => {
            let vsym = lo.sym(&normalize_ws(&v));
            vec![lo.add(NodeKind::Lit, Payload::Name(vsym), span, &[])]
        }
        None => Vec::new(),
    };
    lo.add(NodeKind::HtmlAttr, Payload::Name(nsym), span, &children)
}

/// Canonicalize Vue/Svelte directive shorthands so the two spellings of one binding
/// match: `:x` ≡ `v-bind:x`, `@x` ≡ `v-on:x`. Other names pass through (already
/// lowercased). Svelte's explicit `bind:`/`on:` are left as-is.
fn canonical_attr_name(name: &str) -> String {
    if let Some(rest) = name.strip_prefix(':') {
        format!("v-bind:{rest}")
    } else if let Some(rest) = name.strip_prefix('@') {
        format!("v-on:{rest}")
    } else {
        name.to_string()
    }
}

/// Parse an inline-style value (`color: red; margin: 0`) into a selector-less `CssRule`
/// of `CssDecl(prop)[Lit(Name=token)…]`, mirroring the CSS frontend so value tokens keep
/// their RAW text and the CSS fingerprint can canonicalize them.
fn lower_inline_style(lo: &mut Lowering, value: &str, span: Span) -> NodeId {
    let mut decls = Vec::new();
    for part in value.split(';') {
        let Some((prop, val)) = part.split_once(':') else {
            continue;
        };
        let prop = prop.trim().to_ascii_lowercase();
        if prop.is_empty() {
            continue;
        }
        let psym = lo.sym(&prop);
        let tokens: Vec<NodeId> = val
            .split_whitespace()
            .map(|t| {
                let tsym = lo.sym(t);
                lo.add(NodeKind::Lit, Payload::Name(tsym), span, &[])
            })
            .collect();
        decls.push(lo.add(NodeKind::CssDecl, Payload::Name(psym), span, &tokens));
    }
    lo.add(NodeKind::CssRule, Payload::None, span, &decls)
}

fn lower_text(lo: &mut Lowering, node: TsNode, pre: bool) -> Option<NodeId> {
    let span = lo.span(node);
    // In a preformatted element keep whitespace VERBATIM (it is significant — collapsing
    // it would merge DOM-distinct `<pre>`/`<textarea>` blocks); otherwise collapse it
    // (flow whitespace is insignificant).
    let raw = lo.text(node);
    let text = if pre {
        raw.to_string()
    } else {
        normalize_ws(raw)
    };
    if text.is_empty() {
        return None;
    }
    let sym = lo.sym(&text);
    Some(lo.add(NodeKind::HtmlText, Payload::Name(sym), span, &[]))
}

/// Collapse internal whitespace runs to single spaces and trim — DOM-insignificant
/// formatting differences must not split a clone family.
fn normalize_ws(s: &str) -> String {
    s.split_whitespace().collect::<Vec<_>>().join(" ")
}
