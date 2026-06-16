//! CSS → declarative IL lowering.
//!
//! CSS is *declarative*: a rule's meaning is its **computed style**, not imperative
//! behavior. So CSS rules are NOT lowered through the imperative value graph (GVN).
//! Each `CssRule` becomes a detection unit; the exact `semantic` fingerprint for a
//! CSS unit is produced later by the CSS canonicalizer plus a domain-namespaced hash
//! (`nose-normalize::css_canon`), dispatched in `value_graph::api` by the unit-root
//! kind. Here we build only a faithful, span-accurate declarative tree.
//!
//! Shape: `stylesheet` → a `Module` of `CssRule`s; each rule is
//! `CssRule[ CssSelector*, CssDecl(prop)[ Lit(value-token)... ]... ]`. At-rules
//! (`@media`, `@supports`, `@keyframes`) wrap their nested rules in a `CssRule` whose
//! `CssSelector` carries the at-rule prelude, and the inner rules are also their own
//! units. Anything unrecognized becomes `Raw`, so `nose stats` keeps an honest
//! Raw-node ratio (no panics, ever).

use crate::lower::Lowering;
use nose_il::{FileId, Il, Interner, Lang, NodeId, NodeKind, Payload, UnitKind};
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
        crate::lower::grammar::CSS,
        || tree_sitter_css::LANGUAGE.into(),
        Lang::Css,
        lower_stylesheet,
    )
}

/// The stylesheet root → a `Module` whose children are the top-level rules.
fn lower_stylesheet(lo: &mut Lowering, root: TsNode) -> NodeId {
    let span = lo.span(root);
    let mut rules = Vec::new();
    collect_rules(lo, root, &mut rules, true);
    lo.add(NodeKind::Module, Payload::None, span, &rules)
}

/// Walk a container (`stylesheet` or an at-rule `block`) collecting rule nodes into
/// `out`. Only TOP-LEVEL rules register as detection units (`register`): an at-rule's
/// inner rules and CSS-nested rules roll into their enclosing rule's fingerprint
/// instead, so a `@media`-scoped rule never false-merges with an identical
/// unconditional one (the enclosing context is part of the fingerprint).
fn collect_rules(lo: &mut Lowering, node: TsNode, out: &mut Vec<NodeId>, register: bool) {
    for child in Lowering::named_children(node) {
        match child.kind() {
            "rule_set" => {
                if let Some(rule) = lower_rule_set(lo, child, register) {
                    out.push(rule);
                }
            }
            "media_statement"
            | "supports_statement"
            | "keyframes_statement"
            | "keyframe_block_list"
            | "at_rule" => {
                lower_at_rule(lo, child, out, register);
            }
            // Declaration-level statements with no clone value (and handled, not Raw).
            "import_statement" | "charset_statement" | "namespace_statement" => {}
            other => {
                let span = lo.span(child);
                out.push(lo.raw(other, span, &[]));
            }
        }
    }
}

/// `selectors { block }` → a `CssRule`. Selectors become `CssSelector` children;
/// declarations become `CssDecl` children. Registered as a unit only when `register`
/// (top level); the first selector names the unit (cosmetic, for human reports).
fn lower_rule_set(lo: &mut Lowering, node: TsNode, register: bool) -> Option<NodeId> {
    let span = lo.span(node);
    let mut children = Vec::new();
    let mut name = None;
    for c in Lowering::named_children(node) {
        match c.kind() {
            "selectors" => {
                for sel in Lowering::named_children(c) {
                    let sym = lower_selector(lo, sel);
                    if name.is_none() {
                        name = Some(sym);
                    }
                    let sspan = lo.span(sel);
                    children.push(lo.add(NodeKind::CssSelector, Payload::Name(sym), sspan, &[]));
                }
            }
            "block" => collect_block(lo, c, &mut children),
            // A single bare selector (some grammars omit the `selectors` wrapper).
            k if is_selector_kind(k) => {
                let sym = lower_selector(lo, c);
                if name.is_none() {
                    name = Some(sym);
                }
                let sspan = lo.span(c);
                children.push(lo.add(NodeKind::CssSelector, Payload::Name(sym), sspan, &[]));
            }
            _ => {}
        }
    }
    if children.is_empty() {
        return None;
    }
    let rule = lo.add(NodeKind::CssRule, Payload::None, span, &children);
    if register {
        lo.push_unit(rule, UnitKind::Block, name);
    }
    Some(rule)
}

/// The interned (raw) text of a selector. Canonicalization (whitespace, list order)
/// happens later in `css_canon`; here we keep the source spelling for the report.
fn lower_selector(lo: &mut Lowering, sel: TsNode) -> nose_il::Symbol {
    let text = normalize_ws(lo.text(sel));
    lo.sym(&text)
}

/// Collect a `{ ... }` block's declarations (and any nested rule sets) into `out`.
fn collect_block(lo: &mut Lowering, block: TsNode, out: &mut Vec<NodeId>) {
    for c in Lowering::named_children(block) {
        match c.kind() {
            "declaration" => {
                if let Some(decl) = lower_declaration(lo, c) {
                    out.push(decl);
                }
            }
            // CSS nesting: a rule set inside a block. Never a top-level unit — it rolls
            // into the enclosing rule's fingerprint (which carries the parent selector
            // as context).
            "rule_set" => {
                if let Some(rule) = lower_rule_set(lo, c, false) {
                    out.push(rule);
                }
            }
            // A nested at-rule (e.g. `@media` inside a rule, CSS nesting) — roll it in.
            "media_statement" | "supports_statement" | "keyframes_statement" | "at_rule" => {
                lower_at_rule(lo, c, out, false);
            }
            other => {
                let span = lo.span(c);
                out.push(lo.raw(other, span, &[]));
            }
        }
    }
}

/// `property: value...;` → `CssDecl(property)[ Lit(Name=raw-value-token)... ]`. The
/// property name is lowercased (CSS property names are case-insensitive — a lossless
/// fact). Value tokens keep their RAW (whitespace-normalized, case-preserved) source
/// text as a `Lit(Name)` so the fingerprint and the computed-style oracle can each
/// normalize them INDEPENDENTLY (the moat: a value-normalization bug can't hide if the
/// oracle re-derives computed style from the raw text by a different route). Lit nodes
/// are used (not `Var`) so they carry no binding semantics; `strict_exact` treats a
/// CSS declaration's value tokens as constant data (see `strict_exact_safe_tree`).
fn lower_declaration(lo: &mut Lowering, node: TsNode) -> Option<NodeId> {
    let span = lo.span(node);
    let kids = Lowering::named_children(node);
    let (prop_node, value_nodes) = kids.split_first()?;
    let prop = lo.text(*prop_node).trim().to_ascii_lowercase();
    let prop_sym = lo.sym(&prop);
    let value_children: Vec<NodeId> = value_nodes
        .iter()
        .map(|v| {
            let vspan = lo.span(*v);
            let sym = lo.sym(&normalize_ws(lo.text(*v)));
            lo.add(NodeKind::Lit, Payload::Name(sym), vspan, &[])
        })
        .collect();
    Some(lo.add(
        NodeKind::CssDecl,
        Payload::Name(prop_sym),
        span,
        &value_children,
    ))
}

/// An at-rule (`@media`/`@supports`/`@keyframes`) → a `CssRule` whose `CssSelector`
/// carries the prelude, wrapping the nested rules. The wrapper is the matchable unit
/// (registered only at top level); its inner rules are NOT separate units — they roll
/// into the wrapper's fingerprint, which includes the prelude as context, so a
/// `@media`-scoped rule cannot merge with an identical unconditional one.
fn lower_at_rule(lo: &mut Lowering, node: TsNode, out: &mut Vec<NodeId>, register: bool) {
    let span = lo.span(node);
    let mut inner = Vec::new();
    // The prelude is the ENTIRE at-rule head (every non-block child), not just the
    // keyword — `@container foo (max-width: 768px)` and `@container foo (min-width:
    // 769px)` are different CONDITIONS and must not merge. Capturing only the first
    // child (`@container`) was a false merge surfaced on the bulma corpus.
    let mut prelude_parts: Vec<String> = Vec::new();
    for c in Lowering::named_children(node) {
        match c.kind() {
            "block" | "keyframe_block_list" => collect_at_rule_body(lo, c, &mut inner),
            _ => prelude_parts.push(normalize_ws(lo.text(c))),
        }
    }
    if inner.is_empty() {
        return;
    }
    let prelude = (!prelude_parts.is_empty()).then(|| lo.sym(&prelude_parts.join(" ")));
    let mut children = Vec::new();
    if let Some(sym) = prelude {
        children.push(lo.add(NodeKind::CssSelector, Payload::Name(sym), span, &[]));
    }
    children.extend(inner);
    let wrapper = lo.add(NodeKind::CssRule, Payload::None, span, &children);
    if register {
        lo.push_unit(wrapper, UnitKind::Block, prelude);
    }
    out.push(wrapper);
}

/// An at-rule body holds a MIX depending on the at-rule: declarations directly
/// (`@font-face`, `@page`), nested rule sets (`@media`, `@supports`), or keyframe
/// blocks (`@keyframes`). Lower each into the wrapper's children so none fall to `Raw`.
fn collect_at_rule_body(lo: &mut Lowering, body: TsNode, out: &mut Vec<NodeId>) {
    for c in Lowering::named_children(body) {
        match c.kind() {
            "declaration" => {
                if let Some(d) = lower_declaration(lo, c) {
                    out.push(d);
                }
            }
            "rule_set" => {
                if let Some(r) = lower_rule_set(lo, c, false) {
                    out.push(r);
                }
            }
            "keyframe_block" => {
                if let Some(r) = lower_keyframe_block(lo, c) {
                    out.push(r);
                }
            }
            other => {
                let s = lo.span(c);
                out.push(lo.raw(other, s, &[]));
            }
        }
    }
}

/// A `@keyframes` step `0% { … }` / `from { … }` → a `CssRule`. The offset is SEMANTIC
/// (a `0%` step ≠ a `100%` step), so it is lowered as a synthetic significant
/// declaration (`@keyframe-offset: 0%`) rather than an excluded selector.
fn lower_keyframe_block(lo: &mut Lowering, node: TsNode) -> Option<NodeId> {
    let span = lo.span(node);
    let mut decls = Vec::new();
    let mut offset = None;
    for c in Lowering::named_children(node) {
        if c.kind() == "block" {
            collect_block(lo, c, &mut decls);
        } else if offset.is_none() {
            offset = Some(normalize_ws(lo.text(c)));
        }
    }
    let mut children = Vec::new();
    if let Some(off) = offset {
        let psym = lo.sym("@keyframe-offset");
        let vsym = lo.sym(&off);
        let vlit = lo.add(NodeKind::Lit, Payload::Name(vsym), span, &[]);
        children.push(lo.add(NodeKind::CssDecl, Payload::Name(psym), span, &[vlit]));
    }
    children.extend(decls);
    if children.is_empty() {
        return None;
    }
    Some(lo.add(NodeKind::CssRule, Payload::None, span, &children))
}

/// True for tree-sitter-css selector node kinds (used only for the rare bare-selector
/// shape; the common case goes through the `selectors` wrapper).
fn is_selector_kind(kind: &str) -> bool {
    matches!(
        kind,
        "class_selector"
            | "id_selector"
            | "tag_name"
            | "universal_selector"
            | "attribute_selector"
            | "pseudo_class_selector"
            | "pseudo_element_selector"
            | "descendant_selector"
            | "child_selector"
            | "sibling_selector"
            | "adjacent_sibling_selector"
            | "nesting_selector"
    )
}

/// Collapse internal whitespace runs to single spaces and trim — so source
/// formatting differences (newlines/indentation inside a selector or value) do not
/// by themselves split a clone family.
fn normalize_ws(s: &str) -> String {
    s.split_whitespace().collect::<Vec<_>>().join(" ")
}
