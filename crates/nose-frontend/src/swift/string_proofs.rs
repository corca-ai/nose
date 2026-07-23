use super::*;
use crate::tree_sitter_ext::child_at;

#[derive(Clone, Copy)]
pub(super) enum SwiftStringSpelling {
    Unqualified,
    Qualified,
}

pub(super) fn swift_string_spelling(text: &str) -> Option<SwiftStringSpelling> {
    let matches = |expected: &str| {
        text.chars()
            .filter(|ch| !ch.is_whitespace())
            .eq(expected.chars())
    };
    if matches("String") {
        Some(SwiftStringSpelling::Unqualified)
    } else if matches("Swift.String") {
        Some(SwiftStringSpelling::Qualified)
    } else {
        None
    }
}

#[cold]
#[inline(never)]
pub(super) fn record_selective_import_shadow(lo: &mut Lowering, node: TsNode) {
    let selective = (0..node.child_count()).any(|index| {
        child_at(node, index).is_some_and(|child| {
            matches!(
                child.kind(),
                "typealias" | "struct" | "class" | "enum" | "protocol" | "let" | "var" | "func"
            )
        })
    });
    if !selective {
        // A plain module import exposes all of its public declarations as unqualified names.
        // Without the imported module graph, either stdlib-looking spelling may be shadowed.
        for name in ["String", "Swift"] {
            record_import_shadow(lo, node, name);
        }
        return;
    }
    let Some(path) = Lowering::named_children(node)
        .into_iter()
        .find(|child| child.kind() == "identifier")
    else {
        return;
    };
    let Some(name) = Lowering::named_children(path)
        .into_iter()
        .rev()
        .find(|child| child.kind() == "simple_identifier")
    else {
        return;
    };
    // Selective imports bind the final path component in the consumer's lexical namespace.
    // Preserve that fact as the same empty declaration marker used for local typealiases/types;
    // the cross-file closer can then tombstone an otherwise spelling-only stdlib proof.
    let name = lo.text(name);
    let name = name
        .strip_prefix('`')
        .and_then(|name| name.strip_suffix('`'))
        .unwrap_or(name);
    let symbol = lo.sym(name);
    lo.add(NodeKind::Block, Payload::Name(symbol), lo.span(node), &[]);
}

fn record_import_shadow(lo: &mut Lowering, node: TsNode, name: &str) {
    let symbol = lo.sym(name);
    lo.add(NodeKind::Block, Payload::Name(symbol), lo.span(node), &[]);
}

pub(super) fn swift_string_parameter_evidence(
    lo: &Lowering,
    param: TsNode,
    type_node: TsNode,
) -> Option<TypeEvidenceKind> {
    let (kind, shadow_name) = match swift_string_spelling(lo.text(type_node))? {
        SwiftStringSpelling::Unqualified => {
            (TypeEvidenceKind::SwiftUnqualifiedStringParameter, "String")
        }
        SwiftStringSpelling::Qualified => {
            (TypeEvidenceKind::SwiftQualifiedStringParameter, "Swift")
        }
    };
    if swift_lexical_type_shadow(param, lo, shadow_name) {
        None
    } else {
        Some(kind)
    }
}

fn swift_lexical_type_shadow(mut node: TsNode, lo: &Lowering, name: &str) -> bool {
    while let Some(parent) = node.parent() {
        if swift_generic_header_declares(parent, lo, name)
            || swift_associated_type_shadows(parent, lo, name)
        {
            return true;
        }
        node = parent;
    }
    false
}

fn swift_generic_header_declares(node: TsNode, lo: &Lowering, name: &str) -> bool {
    if !matches!(
        node.kind(),
        "function_declaration"
            | "protocol_function_declaration"
            | "actor_declaration"
            | "class_declaration"
            | "struct_declaration"
            | "enum_declaration"
            | "protocol_declaration"
            | "extension_declaration"
    ) {
        return false;
    }
    let header = lo.text(node).split_once('{').map_or(lo.text(node), |x| x.0);
    let end = header.find('(').unwrap_or(header.len());
    let header = &header[..end];
    let Some(start) = header.find('<') else {
        return false;
    };
    let Some(end) = header.rfind('>') else {
        return false;
    };
    if end <= start {
        return false;
    }
    header[start + 1..end].split(',').any(|parameter| {
        let parameter = parameter
            .trim()
            .strip_prefix("each ")
            .unwrap_or(parameter.trim())
            .trim_start_matches('`');
        let mut declared = parameter
            .chars()
            .take_while(|ch| *ch == '_' || ch.is_alphanumeric());
        declared.by_ref().eq(name.chars())
    })
}

fn swift_associated_type_shadows(node: TsNode, lo: &Lowering, name: &str) -> bool {
    if !matches!(
        node.kind(),
        "actor_declaration"
            | "class_declaration"
            | "struct_declaration"
            | "enum_declaration"
            | "protocol_declaration"
            | "extension_declaration"
    ) {
        return false;
    }
    let Some(body) = node.child_by_field_name("body") else {
        return false;
    };
    Lowering::named_children(body).into_iter().any(|member| {
        member.kind() == "associatedtype_declaration"
            && member
                .child_by_field_name("name")
                .or_else(|| {
                    Lowering::named_children(member)
                        .into_iter()
                        .find(|child| matches!(child.kind(), "simple_identifier" | "identifier"))
                })
                .is_some_and(|declared| lo.text(declared).trim_matches('`') == name)
    })
}
