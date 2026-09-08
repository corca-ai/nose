use crate::lower::Lowering;
use nose_il::{DomainEvidence, EvidenceAnchor, EvidenceKind, Lang, NodeKind, TypeEvidenceKind};
use tree_sitter::Node;

pub(super) fn record(lo: &mut Lowering, param: Node) {
    if lo.lang != Lang::TypeScript {
        return;
    }
    let Some(annotation) = param.child_by_field_name("type") else {
        return;
    };
    if annotation
        .named_child(0)
        .is_none_or(|ty| ty.kind() != "generic_type" || ty.has_error())
    {
        return;
    }
    let text = lo
        .text(annotation)
        .chars()
        .filter(|c| !c.is_whitespace())
        .collect::<String>();
    let Some((name, key)) = key_annotation(&text) else {
        return;
    };
    if shadowed(lo, param, name) {
        return;
    }
    lo.record_evidence(
        EvidenceAnchor::node(lo.span(param), NodeKind::Param),
        EvidenceKind::Type(TypeEvidenceKind::KeyedCollectionKey { key }),
        "typescript_primitive_key_parameter",
    );
}

fn key_annotation(text: &str) -> Option<(&str, DomainEvidence)> {
    let (name, args) = text.strip_prefix(':')?.strip_suffix('>')?.split_once('<')?;
    let key = match name {
        "Map" | "ReadonlyMap" => args.split_once(',')?.0,
        "Set" | "ReadonlySet" => args,
        _ => return None,
    };
    Some((
        name,
        match key {
            "boolean" => DomainEvidence::Boolean,
            "number" => DomainEvidence::Number,
            "string" => DomainEvidence::String,
            _ => return None,
        },
    ))
}

// Type declarations, imports and generic parameters can shadow standard names,
// including declarations after the function. Check the whole source tree.
fn shadowed(lo: &Lowering, mut node: Node, name: &str) -> bool {
    while let Some(parent) = node.parent() {
        node = parent;
    }
    let mut cursor = node.walk();
    loop {
        let node = cursor.node();
        if matches!(node.kind(), "identifier" | "type_identifier") && lo.text(node) == name {
            let parent = node.parent().map(|p| p.kind()).unwrap_or("");
            if !matches!(parent, "generic_type" | "new_expression") {
                return true;
            }
        }
        if cursor.goto_first_child() {
            continue;
        }
        loop {
            if cursor.goto_next_sibling() {
                break;
            }
            if !cursor.goto_parent() {
                return false;
            }
        }
    }
}
