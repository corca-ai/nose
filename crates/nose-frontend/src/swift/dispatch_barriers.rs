use super::*;

pub(super) fn record_nil_literal_conformance(
    lo: &mut Lowering,
    node: TsNode,
    kids: &mut Vec<NodeId>,
) {
    let header = lo.text(node).split('{').next().unwrap_or_default();
    if !swift_source_mentions_identifier(header, "ExpressibleByNilLiteral") {
        return;
    }
    let span = lo.span(node);
    let marker = lo.sym(SWIFT_NIL_LITERAL_CONFORMANCE_MARKER);
    kids.push(lo.add(NodeKind::Block, Payload::Name(marker), span, &[]));
}

pub(super) fn swift_source_mentions_identifier(source: &str, expected: &str) -> bool {
    source
        .split(|ch: char| !(ch == '_' || ch.is_ascii_alphanumeric()))
        .any(|token| token == expected)
}

pub(super) fn record_compact_map_dispatch_barrier(lo: &mut Lowering, node: TsNode) {
    if !["compactMap", "filter", "map"]
        .into_iter()
        .any(|name| swift_source_mentions_identifier(lo.text(node), name))
    {
        return;
    }
    let marker = lo.sym(SWIFT_COMPACT_MAP_DISPATCH_BARRIER_MARKER);
    lo.add(NodeKind::Block, Payload::Name(marker), lo.span(node), &[]);
}

pub(super) fn record_flat_map_dispatch_barrier(lo: &mut Lowering, node: TsNode) {
    if !["flatMap", "filter", "map"]
        .into_iter()
        .any(|name| swift_source_mentions_identifier(lo.text(node), name))
    {
        return;
    }
    record_flat_map_dispatch_proof_barrier(lo, lo.span(node));
}

pub(super) fn record_all_satisfy_dispatch_barrier(lo: &mut Lowering, node: TsNode) {
    if !swift_all_satisfy_dispatch_may_overlap(lo, node) {
        return;
    }
    record_all_satisfy_dispatch_proof_barrier(lo, lo.span(node));
}

fn swift_all_satisfy_dispatch_may_overlap(lo: &Lowering, root: TsNode) -> bool {
    let mut stack = vec![root];
    while let Some(node) = stack.pop() {
        match node.kind() {
            "function_declaration" | "protocol_function_declaration" => {
                if swift_declaration_name_matches(lo, node, "allSatisfy")
                    && swift_all_satisfy_function_accepts_unary_predicate(node)
                {
                    return true;
                }
                // Calls inside a function body do not declare an overload on
                // its enclosing type. Keep scanning sibling declarations when
                // this declaration is proven callback-arity-disjoint.
                continue;
            }
            "property_declaration"
            | "protocol_property_declaration"
            | "protocol_property_requirements" => {
                if swift_declaration_name_matches(lo, node, "allSatisfy") {
                    return true;
                }
                continue;
            }
            "ERROR" if swift_source_mentions_identifier(lo.text(node), "allSatisfy") => {
                return true;
            }
            _ => {}
        }
        stack.extend(Lowering::named_children(node));
    }
    false
}

fn swift_declaration_name_matches(lo: &Lowering, node: TsNode, expected: &str) -> bool {
    let mut cursor = node.walk();
    let matches = node
        .children_by_field_name("name", &mut cursor)
        .any(|name| swift_source_mentions_identifier(lo.text(name), expected));
    matches
}

fn swift_all_satisfy_function_accepts_unary_predicate(function: TsNode) -> bool {
    let parameters: Vec<_> = Lowering::named_children(function)
        .into_iter()
        .filter(|child| child.kind() == "parameter")
        .collect();
    let [parameter] = parameters.as_slice() else {
        // Default arguments and recovered signatures are not modeled here.
        return true;
    };
    let Some(callback_type) = parameter.child_by_field_name("type") else {
        return true;
    };
    if callback_type.kind() != "function_type" {
        return true;
    }
    let Some(callback_parameters) = callback_type.child_by_field_name("params") else {
        return true;
    };
    if callback_parameters.kind() != "tuple_type" {
        // A parenthesized single type is represented as a tuple_type_item;
        // an unparenthesized function input is likewise unary.
        return true;
    }
    let mut cursor = callback_parameters.walk();
    callback_parameters
        .children_by_field_name("element", &mut cursor)
        .count()
        == 1
}

pub(super) fn record_all_satisfy_dispatch_proof_barrier(lo: &mut Lowering, span: Span) {
    let marker = lo.sym(SWIFT_ALL_SATISFY_DISPATCH_BARRIER_MARKER);
    lo.add(NodeKind::Block, Payload::Name(marker), span, &[]);
}

pub(super) fn record_flat_map_dispatch_proof_barrier(lo: &mut Lowering, span: Span) {
    let marker = lo.sym(SWIFT_FLAT_MAP_DISPATCH_BARRIER_MARKER);
    lo.add(NodeKind::Block, Payload::Name(marker), span, &[]);
}

pub(super) fn record_dictionary_default_subscript_extension_barrier(
    lo: &mut Lowering,
    node: TsNode,
) {
    let source = lo.text(node);
    // Alias targets and escaped identifiers can still extend Dictionary, and
    // imported aliases are not resolvable from one source file. Keep the
    // controlled slice fail-closed for every visible default-subscript
    // extension instead of using target spelling as type-identity proof.
    if swift_source_mentions_identifier(source, "subscript") && source.contains("default") {
        record_dictionary_default_subscript_proof_barrier(lo, lo.span(node));
    }
}

pub(super) fn record_dictionary_default_subscript_proof_barrier(lo: &mut Lowering, span: Span) {
    let marker = lo.sym(SWIFT_DICTIONARY_DEFAULT_SUBSCRIPT_BARRIER_MARKER);
    lo.add(NodeKind::Block, Payload::Name(marker), span, &[]);
}

pub(super) fn record_nil_literal_proof_barrier(lo: &mut Lowering, span: Span) {
    let marker = lo.sym(SWIFT_NIL_LITERAL_PROOF_BARRIER_MARKER);
    lo.add(NodeKind::Block, Payload::Name(marker), span, &[]);
}
