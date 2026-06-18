//! HTML markup → declarative IL lowering.
//!
//! HTML is *declarative*: an element's meaning is the **rendered DOM** it produces, not
//! imperative behavior. So markup is NOT lowered through the imperative value graph —
//! each `HtmlElement` subtree is a detection unit whose exact `semantic` fingerprint is
//! the canonical DOM of that subtree (`nose-normalize::html`, dispatched in
//! `value_graph::api` by the unit-root kind). `<script>`/`<style>` elements are dropped
//! (analyzed as their own JS/CSS regions, see `embedded.rs`).
//!
//! Shape: `document` → a `Module` of `HtmlElement`s; each element is
//! `HtmlElement(tag)[ HtmlAttr(name)[Lit(value)?]..., (child element | HtmlText | HtmlControl)... ]`.
//! A `.vue`/`.svelte` file parses as HTML too, so its `<template>` markup is lowered the
//! same way. To make the markup family converge cross-dialect, dialect idioms are
//! normalized into the common IL: framework directives are classified (events/bookkeeping
//! dropped; `:src`/`bind:value`/`v-model` → the rendered attribute with its expression as
//! value); routing components/props are mapped (`router-link`→`a`, `to`→`href`); and
//! control flow (Vue `v-for`/`v-if`, Svelte `{#each}`/`{#if}`) becomes an `HtmlControl`
//! wrapper — distinct from a single element so a loop never exact-merges with one element.
//! Anything unrecognized becomes `Raw` (no panics).

use crate::lower::Lowering;
use nose_il::{
    FileId, Il, Interner, Lang, NodeId, NodeKind, Payload, RegionKind, SourceGranularity, Span,
    Symbol, UnitBodyKind, UnitContainerKind, UnitDomain, UnitDomains, UnitEvidenceFlag, UnitKind,
    UnitOrigin, UnitSubkind,
};
use tree_sitter::Node as TsNode;

pub(crate) fn lower(
    file: FileId,
    path: &str,
    src: &[u8],
    interner: &Interner,
) -> anyhow::Result<Il> {
    lower_with_container(file, path, src, UnitContainerKind::HtmlDocument, interner)
}

pub(crate) fn lower_with_container(
    file: FileId,
    path: &str,
    src: &[u8],
    container_kind: UnitContainerKind,
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
        |lo, root| lower_document(lo, root, container_kind),
    )
}

fn lower_document(lo: &mut Lowering, root: TsNode, container_kind: UnitContainerKind) -> NodeId {
    let span = lo.span(root);
    let kids = lower_children(lo, &Lowering::named_children(root), false, container_kind);
    lo.add(NodeKind::Module, Payload::None, span, &kids)
}

/// A Svelte block marker — which tree-sitter-html parses as plain `text`.
#[derive(Clone, Copy)]
enum SvelteMarker {
    /// `{#each}` (repeat) / `{#if}`,`{#await}`,`{#key}` (conditional) — opens a control frame.
    Open(&'static str),
    /// `{/each}`,`{/if}`,… — closes the current control frame.
    Close,
    /// `{:else}`,`{:then}`,`{:catch}` — an in-block boundary; kept flat (no extra nesting).
    Boundary,
    None,
}

fn svelte_marker(text: &str) -> SvelteMarker {
    let t = text.trim_start();
    if t.starts_with("{#each") {
        SvelteMarker::Open("repeat")
    } else if t.starts_with("{#if") || t.starts_with("{#await") || t.starts_with("{#key") {
        SvelteMarker::Open("if")
    } else if t.starts_with("{/") {
        SvelteMarker::Close
    } else if t.starts_with("{:") {
        SvelteMarker::Boundary
    } else {
        SvelteMarker::None
    }
}

/// Lower a sibling sequence, grouping Svelte block markers (`{#each}`…`{/each}`,
/// `{#if}`…`{/if}`) into [`NodeKind::HtmlControl`] wrappers around the enclosed template.
/// Vue `v-for`/`v-if` (an attribute on the element) and JSX `.map`/`&&` are wrapped at
/// their own sites; this handles only Svelte's text-marker form. A control wrapper keeps a
/// loop/conditional structurally distinct from a single element (exact-channel soundness),
/// while `node_tag` abstracts it so the three dialects' control idioms converge in `near`.
fn lower_children(
    lo: &mut Lowering,
    nodes: &[TsNode],
    pre: bool,
    container_kind: UnitContainerKind,
) -> Vec<NodeId> {
    // Stack of open control frames; frame 0 is the output.
    let mut stack: Vec<(&'static str, Vec<NodeId>)> = vec![("", Vec::new())];
    for &c in nodes {
        if c.kind() == "text" {
            match svelte_marker(lo.text(c)) {
                SvelteMarker::Open(kind) => {
                    stack.push((kind, Vec::new()));
                    continue;
                }
                SvelteMarker::Close if stack.len() > 1 => {
                    let span = lo.span(c);
                    let (kind, kids) = stack.pop().unwrap();
                    let ksym = lo.sym(kind);
                    let ctl = lo.add(NodeKind::HtmlControl, Payload::Name(ksym), span, &kids);
                    stack.last_mut().unwrap().1.push(ctl);
                    continue;
                }
                SvelteMarker::Close | SvelteMarker::Boundary => continue,
                SvelteMarker::None => {}
            }
        }
        if let Some(id) = lower_node(lo, c, pre, container_kind) {
            stack.last_mut().unwrap().1.push(id);
        }
    }
    // Defensive: fold any still-open frame (malformed markup) so nothing is dropped.
    while stack.len() > 1 {
        let span = lo.span(nodes[0]);
        let (kind, kids) = stack.pop().unwrap();
        let ksym = lo.sym(kind);
        let ctl = lo.add(NodeKind::HtmlControl, Payload::Name(ksym), span, &kids);
        stack.last_mut().unwrap().1.push(ctl);
    }
    stack.pop().unwrap().1
}

/// `pre`: are we inside a whitespace-PRESERVING element (`<pre>`/`<textarea>`)? There
/// the renderer keeps whitespace verbatim, so collapsing it would merge DOM-distinct
/// blocks — a false merge. Elsewhere flow whitespace is insignificant and collapsed.
fn lower_node(
    lo: &mut Lowering,
    node: TsNode,
    pre: bool,
    container_kind: UnitContainerKind,
) -> Option<NodeId> {
    match node.kind() {
        "element" => Some(lower_element(lo, node, false, pre, container_kind)),
        // Drop <script>/<style> element shells from the markup region entirely — they are
        // analyzed as their own JS/CSS regions and are pure cross-dialect noise (Svelte/Vue
        // SFCs carry them inline).
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
fn lower_element(
    lo: &mut Lowering,
    node: TsNode,
    raw_content: bool,
    pre: bool,
    container_kind: UnitContainerKind,
) -> NodeId {
    let span = lo.span(node);
    let mut attrs = Vec::new();
    let mut tag = None;
    // A Vue control directive (`v-for`/`v-if`) on this element wraps it in an HtmlControl.
    let mut control: Option<&'static str> = None;
    // Whitespace is significant inside this element if we are already in a preformatted
    // context OR this element is one (`<pre>`/`<textarea>`). The start tag is the first
    // child, so `child_pre` is set before any text/child element is lowered.
    let mut child_pre = pre;
    let mut content = Vec::new();
    for c in Lowering::named_children(node) {
        match c.kind() {
            "start_tag" | "self_closing_tag" => {
                let (t, a) = lower_tag(lo, c);
                if tag.is_none() {
                    tag = t;
                    if let Some(sym) = tag {
                        child_pre = pre || matches!(lo.interner.resolve(sym), "pre" | "textarea");
                    }
                    control = tag_control_kind(lo, c);
                }
                attrs.extend(a);
            }
            "end_tag" | "raw_text" => {}
            _ if !raw_content => content.push(c),
            _ => {}
        }
    }
    let mut children = attrs;
    children.extend(lower_children(lo, &content, child_pre, container_kind));
    let tag_sym = tag.unwrap_or_else(|| lo.sym(""));
    let el = lo.add(
        NodeKind::HtmlElement,
        Payload::Name(tag_sym),
        span,
        &children,
    );
    lo.push_unit_with_origin(
        el,
        UnitKind::Block,
        tag,
        html_element_origin(container_kind, control),
    );
    match control {
        Some(kind) => {
            let ksym = lo.sym(kind);
            lo.add(NodeKind::HtmlControl, Payload::Name(ksym), span, &[el])
        }
        None => el,
    }
}

fn html_element_origin(
    container_kind: UnitContainerKind,
    control: Option<&'static str>,
) -> UnitOrigin {
    let mut origin = UnitOrigin::new(
        UnitDomains::of(UnitDomain::Markup),
        UnitSubkind::HtmlElement,
        UnitBodyKind::DeclarativeDenotation,
        SourceGranularity::Element,
        RegionKind::Markup,
    )
    .with_container(container_kind);
    if control.is_some() {
        origin = origin
            .with_evidence(UnitEvidenceFlag::ContainsMarkupControl)
            .with_evidence(match control {
                Some("repeat") => UnitEvidenceFlag::RepeatControl,
                Some("if") => UnitEvidenceFlag::ConditionalControl,
                _ => UnitEvidenceFlag::ControlFlowTemplate,
            });
    } else {
        origin = origin.with_evidence(UnitEvidenceFlag::StaticAttrsOnly);
    }
    origin
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

/// The Vue control directive on a start tag, if any: `v-for` → a `repeat`, `v-if`/
/// `v-else-if`/`v-else`/`v-show` → an `if`. Used to wrap the element in `HtmlControl`.
fn tag_control_kind(lo: &Lowering, tag_node: TsNode) -> Option<&'static str> {
    for c in Lowering::named_children(tag_node) {
        if c.kind() != "attribute" {
            continue;
        }
        let Some(n) = Lowering::named_children(c)
            .into_iter()
            .find(|x| x.kind() == "attribute_name")
        else {
            continue;
        };
        match canonical_attr_name(&lo.text(n).to_ascii_lowercase()).as_str() {
            "v-for" => return Some("repeat"),
            "v-if" | "v-else-if" | "v-else" | "v-show" => return Some("if"),
            _ => {}
        }
    }
    None
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
    // Classify the (possibly dialect-specific) attribute. A directive binding of a real
    // DOM attribute (`:src`→`src`, Svelte `bind:value`→`value`, `v-model`→`value`) keeps
    // the rendered attribute name; its value is the binding EXPRESSION text, kept verbatim
    // (NOT collapsed to a hole) so the exact fingerprint stays faithful — two attributes
    // with different bound expressions must not be claimed equal. The `near` channel
    // abstracts the value (node_tag) so cross-dialect shells still converge structurally.
    let (name, bound) = match classify_attr(&name) {
        AttrKind::Drop => return None,
        AttrKind::Bound(real) => (canonical_rendered_attr(&real).to_string(), true),
        AttrKind::Plain => (canonical_rendered_attr(&name).to_string(), false),
    };
    // Inline `style="…"` is a CSS declaration block — lower it as a (selector-less)
    // `CssRule` child so the markup fingerprint reuses the full CSS computed-style
    // canonicalization (color/shorthand/unit/cascade) for it. A *bound* `:style` is a
    // dynamic expression, not a literal block — keep it as a plain attribute.
    if name == "style" && !bound {
        let rule = lower_inline_style(lo, value.as_deref().unwrap_or(""), span);
        let nsym = lo.sym(&name);
        return Some(lo.add(NodeKind::HtmlAttr, Payload::Name(nsym), span, &[rule]));
    }
    let nsym = lo.sym(&name);
    let children: Vec<NodeId> = match value {
        Some(v) => {
            let vsym = lo.sym(&normalize_ws(&v));
            vec![lo.add(NodeKind::Lit, Payload::Name(vsym), span, &[])]
        }
        None => Vec::new(),
    };
    Some(lo.add(NodeKind::HtmlAttr, Payload::Name(nsym), span, &children))
}

/// Map framework routing components that render a plain anchor to
/// `a`, so a Vue `<router-link>` / Nuxt `<nuxt-link>` / SvelteKit usage converges with a
/// hand-written `<a>`. Other tags pass through (already lowercased).
fn canonical_tag_name(name: &str) -> &str {
    match name {
        "router-link" | "nuxt-link" | "routerlink" => "a",
        other => other,
    }
}

/// Map a framework component's prop to the DOM attribute it renders,
/// so `<router-link :to="x">` (→ `<a href>`) converges with a hand-written `<a href>`.
fn canonical_rendered_attr(name: &str) -> &str {
    match name {
        "to" => "href",
        other => other,
    }
}

/// How an attribute maps onto rendered DOM.
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
            "v-for"
                | "v-if"
                | "v-else"
                | "v-else-if"
                | "v-show"
                | "v-pre"
                | "v-cloak"
                | "v-once"
                | "v-html"
                | "v-text"
                | "v-slot"
                | "v-bind:key"
                | "v-bind:ref"
                | "v-bind:is"
        )
    {
        return AttrKind::Drop;
    }
    if name == "v-model" {
        return AttrKind::Bound("value".to_string());
    }
    if let Some(real) = name
        .strip_prefix("v-bind:")
        .or_else(|| name.strip_prefix("bind:"))
    {
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
    // Interpolation text (`{x}`, `{{ x }}`) keeps its verbatim source: the exact
    // fingerprint distinguishes different expressions; the `near` channel abstracts the
    // text node (node_tag) so same-shell components still converge across dialects. Svelte
    // block markers are restructured into `HtmlControl` upstream (see `collect_nodes`), so
    // any `{#…}`/`{/…}` reaching here is left as text.
    let sym = lo.sym(&text);
    Some(lo.add(NodeKind::HtmlText, Payload::Name(sym), span, &[]))
}

/// Collapse internal whitespace runs to single spaces and trim — DOM-insignificant
/// formatting differences must not split a clone family.
fn normalize_ws(s: &str) -> String {
    s.split_whitespace().collect::<Vec<_>>().join(" ")
}
