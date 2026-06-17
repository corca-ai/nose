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
        // SPIKE(markup-arch): drop <script>/<style> element shells from the markup region
        // entirely — they are analyzed as their own regions and are pure cross-dialect
        // noise (Svelte/Vue SFCs carry them inline). Was: lowered as shells.
        "script_element" | "style_element" => None,
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
                let lower = lo.text(c).to_ascii_lowercase();
                tag = Some(lo.sym(canonical_tag_name(&lower)));
            }
            "attribute" => {
                if let Some(a) = lower_attr(lo, c) {
                    attrs.push(a);
                }
            }
            _ => {}
        }
    }
    (tag, attrs)
}

/// `name="value"` → `HtmlAttr(name)[Lit(Name=raw value)]`; a boolean attribute has no
/// value child. The name is lowercased (HTML attribute names are case-insensitive); the
/// value keeps its raw text so the DOM fingerprint and a checker can normalize
/// independently.
fn lower_attr(lo: &mut Lowering, node: TsNode) -> Option<NodeId> {
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
    // SPIKE(markup-arch): classify the (possibly dialect-specific) attribute.
    let (name, dynamic) = match classify_attr(&name) {
        AttrKind::Drop => return None,
        // A dynamic binding of a REAL DOM attribute (`:src`→`v-bind:src`, Svelte
        // `bind:value`, Vue `v-model`): keep the rendered attribute name, value is a hole.
        // This is what makes `:src="x"` (Vue) ≡ `src={x}` (Svelte/JSX) ≡ `src="..."` (HTML).
        AttrKind::Bound(real) => (canonical_rendered_attr(&real).to_string(), true),
        AttrKind::Plain => (canonical_rendered_attr(&name).to_string(), false),
    };
    // Inline `style="…"` is a CSS declaration block — lower it as a (selector-less)
    // `CssRule` child so the markup fingerprint reuses the full CSS computed-style
    // canonicalization (color/shorthand/unit/cascade) for it.
    if name == "style" && !dynamic {
        let rule = lower_inline_style(lo, value.as_deref().unwrap_or(""), span);
        let nsym = lo.sym(&name);
        return Some(lo.add(NodeKind::HtmlAttr, Payload::Name(nsym), span, &[rule]));
    }
    let nsym = lo.sym(&name);
    let children: Vec<NodeId> = if dynamic {
        let vsym = lo.sym("{}");
        vec![lo.add(NodeKind::Lit, Payload::Name(vsym), span, &[])]
    } else {
        match value {
            Some(v) => {
                // A dynamic value (`{...}` / mustache) is a hole — canonicalize so a
                // bound and a static attribute of the same name converge structurally.
                let vsym = lo.sym(&normalize_dynamic(&normalize_ws(&v)));
                vec![lo.add(NodeKind::Lit, Payload::Name(vsym), span, &[])]
            }
            None => Vec::new(),
        }
    };
    Some(lo.add(NodeKind::HtmlAttr, Payload::Name(nsym), span, &children))
}

/// SPIKE(markup-arch): map framework routing components that render a plain anchor to
/// `a`, so a Vue `<router-link>` / Nuxt `<nuxt-link>` / SvelteKit usage converges with a
/// hand-written `<a>`. Other tags pass through (already lowercased).
fn canonical_tag_name(name: &str) -> &str {
    match name {
        "router-link" | "nuxt-link" | "routerlink" => "a",
        other => other,
    }
}

/// SPIKE(markup-arch): map a framework component's prop to the DOM attribute it renders,
/// so `<router-link :to="x">` (→ `<a href>`) converges with a hand-written `<a href>`.
fn canonical_rendered_attr(name: &str) -> &str {
    match name {
        "to" => "href",
        other => other,
    }
}

/// SPIKE(markup-arch): how an attribute maps onto rendered DOM.
enum AttrKind {
    /// Framework control/event/bookkeeping — not a rendered attribute. Dropped.
    Drop,
    /// A dynamic binding of a real DOM attribute; the `String` is the rendered name and
    /// the value becomes a hole (`v-bind:src`→`src`, `bind:value`→`value`, `v-model`→`value`).
    Bound(String),
    /// An ordinary rendered attribute (static or `{…}`-valued).
    Plain,
}

fn classify_attr(name: &str) -> AttrKind {
    // Event handlers, lifecycle/animation, and per-dialect bookkeeping render nothing.
    if name.starts_with("v-on:")
        || name.starts_with("on:")
        || name.starts_with("use:")
        || name.starts_with("transition:")
        || name.starts_with("in:")
        || name.starts_with("out:")
        || name.starts_with("animate:")
        || name.starts_with("class:")
        || name.starts_with("style:")
        || name.starts_with('#')
        || matches!(name, "key" | "slot" | "ref" | "is" | "bind:this")
        || matches!(
            name,
            "v-for" | "v-if" | "v-else" | "v-else-if" | "v-show" | "v-pre" | "v-cloak"
                | "v-once" | "v-html" | "v-text" | "v-slot" | "v-bind:key" | "v-bind:ref"
                | "v-bind:is"
        )
    {
        return AttrKind::Drop;
    }
    if name == "v-model" {
        return AttrKind::Bound("value".to_string());
    }
    if let Some(real) = name.strip_prefix("v-bind:").or_else(|| name.strip_prefix("bind:")) {
        return AttrKind::Bound(real.to_string());
    }
    AttrKind::Plain
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
    // SPIKE(markup-arch): a Svelte block marker (`{#each}`, `{/each}`, `{:else}`, `{#if}`,
    // `{/if}`, `{#await}`…) is control flow, not rendered text — drop it so the templated
    // child becomes a direct child of its container (matching Vue's `v-for`-on-element and
    // React's `.map()`-wrapped child after their own normalization).
    let trimmed = text.trim_start();
    if trimmed.starts_with("{#") || trimmed.starts_with("{/") || trimmed.starts_with("{:") {
        return None;
    }
    let text = normalize_dynamic(&text);
    if text.is_empty() {
        return None;
    }
    let sym = lo.sym(&text);
    Some(lo.add(NodeKind::HtmlText, Payload::Name(sym), span, &[]))
}

/// SPIKE(markup-arch): collapse every `{ expression }` interpolation (Svelte/JSX `{x}`,
/// Vue `{{ x }}`) to a single canonical hole token `{}`, so two components with the same
/// markup skeleton but different dynamic content converge structurally. Static text is
/// returned unchanged. This is intentionally crude (brace-run replacement) for the spike.
fn normalize_dynamic(s: &str) -> String {
    if !s.contains('{') {
        return s.to_string();
    }
    let mut out = String::new();
    let mut depth = 0usize;
    let mut emitted_hole = false;
    for ch in s.chars() {
        match ch {
            '{' => {
                if depth == 0 && !emitted_hole {
                    out.push_str("{}");
                    emitted_hole = true;
                }
                depth += 1;
            }
            '}' => {
                depth = depth.saturating_sub(1);
                if depth == 0 {
                    emitted_hole = false;
                }
            }
            _ if depth == 0 => out.push(ch),
            _ => {}
        }
    }
    normalize_ws(&out)
}

/// Collapse internal whitespace runs to single spaces and trim — DOM-insignificant
/// formatting differences must not split a clone family.
fn normalize_ws(s: &str) -> String {
    s.split_whitespace().collect::<Vec<_>>().join(" ")
}
