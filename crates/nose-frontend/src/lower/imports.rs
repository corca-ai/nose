use super::*;

/// Does `node` have a *direct* child token of the given `kind`? Used to read an
/// operator token (`--`, `++`) off the node it belongs to without being fooled by a
/// nested occurrence in the operand (e.g. the inner `i--` of `a[i--]++`), which a
/// substring search over the node's whole text would wrongly match.
pub(crate) fn has_direct_token(node: TsNode, kind: &str) -> bool {
    let mut cur = node.walk();
    let found = node.children(&mut cur).any(|c| c.kind() == kind);
    found
}

/// Lower an import / `#include` / `use` statement to a `Seq` of its identifier and
/// string leaves. Imports carry no behavior, but a *duplicated import block* is real
/// copy-paste (jscpd flags it); emitting its tokens lets the contiguous copy-paste
/// channel — nose's Type-1/2 floor — cover it. These form no unit (the structural and
/// behavioral channels ignore them) and rank near-zero, so users never see import
/// noise; only the copy-paste floor does.
pub(crate) fn import_tokens(lo: &mut Lowering, node: TsNode) -> NodeId {
    let span = lo.span(node);
    let mut kids = Vec::new();
    collect_leaf_tokens(lo, node, &mut kids);
    lo.add(NodeKind::Seq, Payload::None, span, &kids)
}

/// A strict semantic proof fact for a static import binding:
/// local name → `(module coordinate, exported symbol)`.
///
/// Frontends only call this for import forms whose module/export identity is fully static.
/// Ambiguous forms fall back to [`import_tokens`], remaining visible to syntax/near but
/// unavailable to strict exact semantic mode.
pub(crate) fn import_binding(
    lo: &mut Lowering,
    span: Span,
    local: &str,
    module: &str,
    exported: &str,
) -> NodeId {
    import_fact_with_symbol_evidence(
        lo,
        span,
        local,
        ImportFactKind::Binding,
        &[module, exported],
    )
    .0
}

pub(crate) fn import_binding_with_symbol_evidence(
    lo: &mut Lowering,
    span: Span,
    local: &str,
    module: &str,
    exported: &str,
) -> (NodeId, Option<EvidenceId>) {
    import_fact_with_symbol_evidence(
        lo,
        span,
        local,
        ImportFactKind::Binding,
        &[module, exported],
    )
}

pub(crate) fn import_binding_evidence_only(
    lo: &mut Lowering,
    span: Span,
    local: &str,
    module: &str,
    exported: &str,
) -> Option<EvidenceId> {
    record_import_fact_evidence(
        lo,
        span,
        local,
        ImportFactKind::Binding,
        &[module, exported],
    )
}

pub(crate) fn re_export_binding_evidence_only(
    lo: &mut Lowering,
    span: Span,
    local: &str,
    target_module: &str,
    target_exported: &str,
) -> EvidenceId {
    lo.record_evidence(
        EvidenceAnchor::binding(span, stable_symbol_hash(local)),
        EvidenceKind::Import(ImportEvidenceKind::ReExportBinding {
            target_module_hash: stable_symbol_hash(target_module),
            target_exported_hash: stable_symbol_hash(target_exported),
        }),
        "import_re_export_binding",
    )
}

/// A strict semantic proof fact for a static namespace import:
/// local namespace → module coordinate.
pub(crate) fn import_namespace(lo: &mut Lowering, span: Span, local: &str, module: &str) -> NodeId {
    import_fact_with_symbol_evidence(lo, span, local, ImportFactKind::Namespace, &[module]).0
}

pub(crate) fn import_namespace_evidence_only(
    lo: &mut Lowering,
    span: Span,
    local: &str,
    module: &str,
) -> Option<EvidenceId> {
    record_import_fact_evidence(lo, span, local, ImportFactKind::Namespace, &[module])
}

/// Shared shape of static-import proof facts. The assignment remains in IL so
/// import text participates in the syntax/near floor, but the `Seq` payload is
/// deliberately untagged: semantic proof lives only in the evidence records.
fn import_fact_with_symbol_evidence(
    lo: &mut Lowering,
    span: Span,
    local: &str,
    kind: ImportFactKind,
    coords: &[&str],
) -> (NodeId, Option<EvidenceId>) {
    let lhs = lo.var(local, span);
    let strs: Vec<NodeId> = coords.iter().map(|c| lo.str_lit(c, span)).collect();
    let rhs = lo.add(NodeKind::Seq, Payload::None, span, &strs);
    let symbol_evidence = record_import_fact_evidence(lo, span, local, kind, coords);
    (
        lo.add(NodeKind::Assign, Payload::None, span, &[lhs, rhs]),
        symbol_evidence,
    )
}

fn record_import_fact_evidence(
    lo: &mut Lowering,
    span: Span,
    local: &str,
    kind: ImportFactKind,
    coords: &[&str],
) -> Option<EvidenceId> {
    let evidence_kind = match kind {
        ImportFactKind::Binding if coords.len() == 2 => {
            EvidenceKind::Import(ImportEvidenceKind::Binding {
                module_hash: stable_symbol_hash(coords[0]),
                exported_hash: stable_symbol_hash(coords[1]),
            })
        }
        ImportFactKind::Namespace if coords.len() == 1 => {
            EvidenceKind::Import(ImportEvidenceKind::Namespace {
                module_hash: stable_symbol_hash(coords[0]),
            })
        }
        _ => return None,
    };
    let symbol_kind = match kind {
        ImportFactKind::Binding if coords.len() == 2 => {
            EvidenceKind::Symbol(SymbolEvidenceKind::ImportedBinding {
                module_hash: stable_symbol_hash(coords[0]),
                exported_hash: stable_symbol_hash(coords[1]),
            })
        }
        ImportFactKind::Namespace if coords.len() == 1 => {
            EvidenceKind::Symbol(SymbolEvidenceKind::ImportedNamespace {
                module_hash: stable_symbol_hash(coords[0]),
            })
        }
        _ => return None,
    };
    lo.record_evidence(EvidenceAnchor::sequence(span), evidence_kind, "import_fact");
    lo.record_evidence(
        EvidenceAnchor::binding(span, stable_symbol_hash(local)),
        evidence_kind,
        "import_binding_subject",
    );
    let symbol_evidence = lo.record_evidence(
        EvidenceAnchor::binding(span, stable_symbol_hash(local)),
        symbol_kind,
        "symbol_import_identity",
    );
    Some(symbol_evidence)
}

/// Emit a `Var` token for every named leaf (identifier, string fragment, path
/// component) in `node`'s subtree — the textual identity of an import.
fn collect_leaf_tokens(lo: &mut Lowering, node: TsNode, out: &mut Vec<NodeId>) {
    let named = Lowering::named_children(node);
    if named.is_empty() {
        let t = lo.text(node);
        if !t.is_empty() {
            let span = lo.span(node);
            out.push(lo.var(t, span));
        }
    } else {
        for c in named {
            collect_leaf_tokens(lo, c, out);
        }
    }
}
