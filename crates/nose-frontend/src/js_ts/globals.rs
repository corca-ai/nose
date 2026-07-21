use super::expressions::lower_expr;
use super::syntax::compact_js_expr;
use crate::lower::Lowering;
use nose_il::{
    contains_js_identifier, is_js_identifier_continue, stable_symbol_hash, EvidenceAnchor,
    EvidenceKind, GuardEvidenceKind, NodeId, NodeKind, Payload,
};
use nose_semantics::{qualified_global_symbol_contract, static_global_symbol_contract};
use tree_sitter::Node as TsNode;

pub(super) fn lower_own_property_guard_call(lo: &mut Lowering, node: TsNode) -> Option<NodeId> {
    let callee = node.child_by_field_name("function")?;
    let callee_text = compact_js_expr(lo.text(callee));
    let contract = qualified_global_symbol_contract(lo.lang, &callee_text)?;
    if !matches!(
        contract.path,
        "Object.hasOwn" | "Object.prototype.hasOwnProperty.call"
    ) {
        return None;
    }
    if contract.requires_unshadowed_root
        && (file_prefix_has_binding_ident(lo, node, contract.root)
            || enclosing_function_prefix_has_binding_ident(lo, node, contract.root))
    {
        return None;
    }
    let args = node.child_by_field_name("arguments")?;
    let args: Vec<TsNode> = Lowering::named_children(args);
    if args.len() != 2 || args.iter().any(|arg| arg.kind() == "spread_element") {
        return None;
    }
    let span = lo.span(node);
    let receiver = lower_expr(lo, args[0]);
    let key = lower_expr(lo, args[1]);
    let own = lo.str_lit("own", span);
    let present = lo.str_lit("present", span);
    let tag = lo.sym("own_property_guard");
    let guard = lo.add(
        NodeKind::Seq,
        Payload::Name(tag),
        span,
        &[receiver, key, own, present],
    );
    let api_dependency = lo.record_qualified_global_symbol(span, NodeKind::Seq, contract.path);
    lo.record_evidence_with_dependencies(
        EvidenceAnchor::sequence(span),
        EvidenceKind::Guard(GuardEvidenceKind::JsOwnProperty {
            api_path_hash: stable_symbol_hash(contract.path),
        }),
        "own_property_guard_js_api",
        vec![api_dependency],
    );
    Some(guard)
}

pub(super) fn file_prefix_has_binding_ident(lo: &Lowering, node: TsNode, ident: &str) -> bool {
    let end = node.start_byte();
    if end > lo.src.len() {
        return false;
    }
    // Most static-global names are never rebound in a given file. Establish
    // that once per name and turn every later source-position check into a
    // hash lookup. If the file does contain a binding, retain the existing
    // prefix scan so uses before and after that binding keep their semantics.
    if let Ok(text) = std::str::from_utf8(lo.src) {
        let symbol = lo.sym(ident);
        let cached = lo.js_file_binding_cache.borrow().get(&symbol).copied();
        let file_has_binding = cached.unwrap_or_else(|| {
            let found = contains_js_binding_ident(text, ident);
            lo.js_file_binding_cache.borrow_mut().insert(symbol, found);
            found
        });
        if !file_has_binding {
            return false;
        }
    }
    let prefix = std::str::from_utf8(&lo.src[..end]).unwrap_or("");
    contains_js_binding_ident(prefix, ident)
}

pub(super) fn enclosing_function_prefix_has_binding_ident(
    lo: &Lowering,
    node: TsNode,
    ident: &str,
) -> bool {
    let mut current = node;
    while let Some(parent) = current.parent() {
        if matches!(
            parent.kind(),
            "function_declaration" | "function" | "function_expression" | "arrow_function"
        ) {
            let start = parent.start_byte();
            let end = node.start_byte();
            if end <= lo.src.len() && start <= end {
                let prefix = std::str::from_utf8(&lo.src[start..end]).unwrap_or("");
                let header = prefix.find('{').map(|idx| &prefix[..idx]).unwrap_or(prefix);
                if contains_js_identifier(header, ident) || contains_js_binding_ident(prefix, ident)
                {
                    return true;
                }
            }
        }
        current = parent;
    }
    false
}

fn contains_js_binding_ident(text: &str, ident: &str) -> bool {
    ["const", "let", "var", "function", "class"]
        .iter()
        .any(|kw| contains_keyword_binding(text, kw, ident))
        || contains_import_binding(text, ident)
}

fn contains_keyword_binding(text: &str, keyword: &str, ident: &str) -> bool {
    text.match_indices(keyword).any(|(idx, _)| {
        let before = text[..idx].chars().next_back();
        if before.is_some_and(is_js_identifier_continue) {
            return false;
        }
        let mut rest = &text[idx + keyword.len()..];
        let Some(next) = rest.chars().next() else {
            return false;
        };
        if !next.is_whitespace() {
            return false;
        }
        rest = rest.trim_start();
        starts_with_js_ident(rest, ident) || destructuring_pattern_binds_ident(rest, ident)
    })
}

fn destructuring_pattern_binds_ident(text: &str, ident: &str) -> bool {
    if !matches!(text.chars().next(), Some('{') | Some('[')) {
        return false;
    }
    let pattern = text.split_once('=').map(|(lhs, _)| lhs).unwrap_or(text);
    contains_js_identifier(pattern, ident)
}

fn contains_import_binding(text: &str, ident: &str) -> bool {
    let mut earliest_rest = None;
    for (idx, _) in text.match_indices("import") {
        let before = text[..idx].chars().next_back();
        if before.is_some_and(is_js_identifier_continue) {
            continue;
        }
        let rest = text[idx + "import".len()..].trim_start();
        earliest_rest.get_or_insert(rest);
        if starts_with_js_ident(rest, ident) {
            return true;
        }
    }

    // The legacy prefix check deliberately treats these spellings anywhere
    // after an import keyword as a binding. Search the largest such suffix
    // once instead of allocating four needles and rescanning a shrinking file
    // suffix for every import declaration.
    earliest_rest.is_some_and(|rest| {
        rest.match_indices(ident).any(|(idx, _)| {
            let before = &rest[..idx];
            before.ends_with("{ ")
                || before.ends_with('{')
                || before.ends_with(", ")
                || before.ends_with(" as ")
        })
    })
}

fn starts_with_js_ident(text: &str, ident: &str) -> bool {
    text.starts_with(ident)
        && !text[ident.len()..]
            .chars()
            .next()
            .is_some_and(is_js_identifier_continue)
}

pub(super) fn lower_callee_expr(lo: &mut Lowering, node: TsNode) -> NodeId {
    match node.kind() {
        "identifier" | "undefined" => lower_js_static_global_or_var(lo, node),
        _ => lower_expr(lo, node),
    }
}

pub(super) fn lower_member_object(lo: &mut Lowering, node: TsNode) -> NodeId {
    match node.kind() {
        "identifier" | "undefined" => lower_js_static_global_or_var(lo, node),
        _ => lower_expr(lo, node),
    }
}

pub(super) fn lower_member_expr(lo: &mut Lowering, node: TsNode) -> NodeId {
    let span = lo.span(node);
    let obj = node
        .child_by_field_name("object")
        .map(|o| lower_member_object(lo, o))
        .unwrap_or_else(|| lo.empty_block(span));
    let prop = node
        .child_by_field_name("property")
        .map(|p| lo.text(p))
        .unwrap_or("");
    let sym = lo.sym(prop);
    let field = lo.add(NodeKind::Field, Payload::Name(sym), span, &[obj]);
    let path = compact_js_expr(lo.text(node));
    if let Some(contract) = qualified_global_symbol_contract(lo.lang, &path) {
        let root_unshadowed = !contract.requires_unshadowed_root
            || (!file_prefix_has_binding_ident(lo, node, contract.root)
                && !enclosing_function_prefix_has_binding_ident(lo, node, contract.root));
        if root_unshadowed {
            lo.record_qualified_global_symbol(span, NodeKind::Field, contract.path);
        }
    }
    field
}

pub(super) fn lower_constructor_expr(lo: &mut Lowering, node: TsNode) -> NodeId {
    match node.kind() {
        "identifier" | "undefined" => lower_js_static_global_or_var(lo, node),
        _ => lower_expr(lo, node),
    }
}

pub(super) fn lower_js_static_global_or_var(lo: &mut Lowering, node: TsNode) -> NodeId {
    let name = lo.text(node);
    let span = lo.span(node);
    if js_static_global_unshadowed_at(lo, node, name) {
        lo.unshadowed_global_var(name, span)
    } else {
        lo.var(name, span)
    }
}

fn js_static_global_unshadowed_at(lo: &Lowering, node: TsNode, name: &str) -> bool {
    let Some(contract) = static_global_symbol_contract(lo.lang, name) else {
        return false;
    };
    if !contract.requires_unshadowed {
        return true;
    }
    !file_prefix_has_binding_ident(lo, node, contract.name)
        && !enclosing_function_prefix_has_binding_ident(lo, node, contract.name)
}

#[cfg(test)]
mod tests {
    use super::contains_import_binding;

    #[test]
    fn import_binding_scan_preserves_supported_spellings() {
        for source in [
            "import Promise from 'pkg'",
            "import { Promise } from 'pkg'",
            "import {Promise} from 'pkg'",
            "import { value, Promise } from 'pkg'",
            "import { value as Promise } from 'pkg'",
        ] {
            assert!(contains_import_binding(source, "Promise"), "{source}");
        }
    }

    #[test]
    fn import_binding_scan_checks_later_default_imports_and_boundaries() {
        assert!(contains_import_binding(
            "import value from 'a'; import Promise from 'b'",
            "Promise"
        ));
        assert!(!contains_import_binding(
            "const reimport = true; import PromiseLike from 'b'",
            "Promise"
        ));
        assert!(!contains_import_binding("const Promise = 1", "Promise"));
    }
}
