use super::*;

const JAVA_CONCURRENT_MODULE: &str = "java.util.concurrent";
const COMPLETABLE_FUTURE_TYPE: &str = "CompletableFuture";
const COMPLETABLE_FUTURE_QUALIFIED: &str = "java.util.concurrent.CompletableFuture";

pub(super) fn lower_empty_java_collection_constructor(
    lo: &mut Lowering,
    node: TsNode,
    args: &[NodeId],
    span: Span,
) -> Option<NodeId> {
    let ty = node.child_by_field_name("type")?;
    let type_span = lo.span(ty);
    let type_name = java_constructor_type_name(lo.text(ty));
    let contract = library_java_collection_constructor_contract(lo.lang, &type_name, args.len())?;
    let LibraryApiCalleeContract::JavaUtilConstructor { type_ref } = contract.callee else {
        return None;
    };
    let uses_simple_type = type_name == type_ref.simple_type();
    if uses_simple_type {
        let root = java_root(node);
        if !java_tree_resolves_simple_type(lo, root, type_ref.module(), type_ref.simple_type()) {
            return None;
        }
        if java_tree_declares_type(lo, root, type_ref.simple_type()) {
            return None;
        }
    }
    match contract.result {
        LibraryCollectionFactoryResult::EmptySequence => Some(lower_java_construct_call(
            lo, &type_name, type_span, args, span,
        )),
        _ => None,
    }
}

pub(super) fn lower_java_completable_future_constructor(
    lo: &mut Lowering,
    node: TsNode,
    args: &[NodeId],
    span: Span,
) -> Option<NodeId> {
    let ty = node.child_by_field_name("type")?;
    let type_span = lo.span(ty);
    let type_name = java_constructor_type_name(lo.text(ty));
    match type_name.as_str() {
        COMPLETABLE_FUTURE_QUALIFIED => {}
        COMPLETABLE_FUTURE_TYPE => {
            let root = java_root(node);
            let mut saw_wildcard = false;
            let mut saw_exact = false;
            let mut saw_conflict = false;
            java_tree_import_resolution(
                lo,
                root,
                JAVA_CONCURRENT_MODULE,
                COMPLETABLE_FUTURE_TYPE,
                &mut saw_wildcard,
                &mut saw_exact,
                &mut saw_conflict,
            );
            if saw_conflict
                || !(saw_exact || saw_wildcard)
                || java_tree_declares_type(lo, root, COMPLETABLE_FUTURE_TYPE)
            {
                return None;
            }
            if saw_exact {
                lo.record_evidence(
                    EvidenceAnchor::binding(type_span, stable_symbol_hash(COMPLETABLE_FUTURE_TYPE)),
                    EvidenceKind::Symbol(SymbolEvidenceKind::ImportedBinding {
                        module_hash: stable_symbol_hash(JAVA_CONCURRENT_MODULE),
                        exported_hash: stable_symbol_hash(COMPLETABLE_FUTURE_TYPE),
                    }),
                    "java_completable_future_constructor_import",
                );
            }
        }
        _ => return None,
    }

    Some(lower_java_construct_call(
        lo, &type_name, type_span, args, span,
    ))
}

fn lower_java_construct_call(
    lo: &mut Lowering,
    type_name: &str,
    type_span: Span,
    args: &[NodeId],
    span: Span,
) -> NodeId {
    let callee = lo.add(
        NodeKind::Var,
        Payload::Name(lo.sym(type_name)),
        type_span,
        &[],
    );
    let mut kids = Vec::with_capacity(args.len() + 1);
    kids.push(callee);
    kids.extend_from_slice(args);
    lo.record_source_fact(span, SourceFactKind::Call(SourceCallKind::Construct));
    lo.add(NodeKind::Call, Payload::None, span, &kids)
}

pub(super) fn java_constructor_type_name(text: &str) -> String {
    text.split('<')
        .next()
        .unwrap_or(text)
        .chars()
        .filter(|c| !c.is_whitespace())
        .collect()
}
pub(super) fn java_root(mut node: TsNode) -> TsNode {
    while let Some(parent) = node.parent() {
        node = parent;
    }
    node
}
pub(super) fn java_tree_resolves_simple_type(
    lo: &Lowering,
    node: TsNode,
    module: &str,
    simple_type: &str,
) -> bool {
    let mut saw_wildcard = false;
    let mut saw_exact = false;
    let mut saw_conflict = false;
    java_tree_import_resolution(
        lo,
        node,
        module,
        simple_type,
        &mut saw_wildcard,
        &mut saw_exact,
        &mut saw_conflict,
    );
    (saw_exact || saw_wildcard) && !saw_conflict
}
pub(super) fn java_tree_import_resolution(
    lo: &Lowering,
    node: TsNode,
    module: &str,
    simple_type: &str,
    saw_wildcard: &mut bool,
    saw_exact: &mut bool,
    saw_conflict: &mut bool,
) {
    if node.kind() == "import_declaration" {
        let compact: String = lo
            .text(node)
            .chars()
            .filter(|c| !c.is_whitespace())
            .collect();
        let exact = format!("import{module}.{simple_type};");
        let wildcard = format!("import{module}.*;");
        if compact == exact {
            *saw_exact = true;
        } else if compact == wildcard {
            *saw_wildcard = true;
        } else if compact.starts_with("import") && compact.ends_with(&format!(".{simple_type};")) {
            *saw_conflict = true;
        }
        return;
    }
    for child in Lowering::named_children(node) {
        java_tree_import_resolution(
            lo,
            child,
            module,
            simple_type,
            saw_wildcard,
            saw_exact,
            saw_conflict,
        );
    }
}
pub(super) fn java_tree_declares_type(lo: &Lowering, node: TsNode, simple_type: &str) -> bool {
    if matches!(
        node.kind(),
        "class_declaration"
            | "interface_declaration"
            | "enum_declaration"
            | "record_declaration"
            | "annotation_type_declaration"
    ) && node
        .child_by_field_name("name")
        .is_some_and(|name| lo.text(name) == simple_type)
    {
        return true;
    }
    Lowering::named_children(node)
        .into_iter()
        .any(|child| java_tree_declares_type(lo, child, simple_type))
}
