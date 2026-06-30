use super::*;

pub(super) fn lower_expr(lo: &mut Lowering, node: TsNode) -> NodeId {
    let span = lo.span(node);
    if let Some(lowered) = lower_expr_atom(lo, node, span) {
        return lowered;
    }
    match node.kind() {
        "binary_expression" => lower_binary(lo, node),
        "unary_expression" => lower_unary(lo, node),
        "assignment_expression" => {
            crate::lower::assignment(lo, node, lower_store_target, lower_expr)
        }
        "compound_assignment_expr" => lower_compound_assign(lo, node),
        "try_expression" => {
            let value = node
                .named_child(0)
                .map(|c| lower_expr(lo, c))
                .unwrap_or_else(|| lo.empty_block(span));
            lo.protocol_boundary(span, SourceProtocolKind::TryPropagation, "try", &[value])
        }
        "await_expression" => {
            let value = node
                .named_child(0)
                .map(|c| lower_expr(lo, c))
                .unwrap_or_else(|| lo.empty_block(span));
            lo.await_boundary(span, value)
        }
        // Wrappers that peel to their single child: `(x)`, `&pat`, turbofish
        // `foo::<T>` / `Vec::<T>::new` (drop the turbofish), and `unsafe { … }`
        // (just its block).
        "parenthesized_expression" | "reference_pattern" | "generic_function" | "unsafe_block" => {
            node.named_child(0)
                .map(|c| lower_expr(lo, c))
                .unwrap_or_else(|| lo.empty_block(span))
        }
        // `&x` / `&mut x` → the referenced value (skip the mutable_specifier);
        // `x as T` → `x` (a cast is type-level; erase it, like TS `as`)
        "reference_expression" | "type_cast_expression" => node
            .child_by_field_name("value")
            .map(|c| lower_expr(lo, c))
            .unwrap_or_else(|| lo.empty_block(span)),
        // `a..b` / `a..=b` / `a..` → a sequence of its endpoints
        "range_expression" => lower_range_expr(lo, node),
        "call_expression" => lower_call(lo, node),
        "macro_invocation" => lower_macro(lo, node),
        "method_call_expression" => lower_method_call(lo, node),
        "field_expression" => lower_field(lo, node),
        "index_expression" => lower_index(lo, node),
        "closure_expression" => lower_closure(lo, node),
        "if_expression" => lower_if(lo, node),
        "match_expression" => lower_match(lo, node),
        "for_expression" => lower_for(lo, node),
        "while_expression" => lower_while(lo, node),
        "loop_expression" => lower_loop(lo, node),
        "return_expression" => {
            let mut kids = Vec::new();
            if let Some(v) = node.named_child(0) {
                kids.push(lower_expr(lo, v));
            }
            lo.add(NodeKind::Return, Payload::None, span, &kids)
        }
        "break_expression" => lo.add(NodeKind::Break, Payload::None, span, &[]),
        "continue_expression" => lo.add(NodeKind::Continue, Payload::None, span, &[]),
        // Patterns share the tag of the expression form they destructure so
        // `(a, b)` and `[a, b]` shapes converge across binding/value position.
        "tuple_pattern" => lower_seq_tagged(lo, node, "tuple_expression"),
        "slice_pattern" => lower_seq_tagged(lo, node, "array_expression"),
        "tuple_struct_pattern"
        | "struct_pattern"
        | "field_pattern"
        | "remaining_field_pattern"
        | "captured_pattern"
        | "or_pattern"
        | "wildcard_pattern"
        | "shorthand_field_identifier_pattern" => lower_pattern_surface(lo, node),
        "array_expression" | "tuple_expression" => lower_seq_tagged(lo, node, node.kind()),
        "struct_expression" => lower_struct_expr(lo, node),
        "async_block" => {
            let body = node
                .named_child(0)
                .map(|c| lower_expr(lo, c))
                .unwrap_or_else(|| lo.empty_block(span));
            lo.protocol_boundary(span, SourceProtocolKind::AsyncBlock, "async_block", &[body])
        }
        // Type-level nodes carry no runtime behavior — erase (don't Raw). These reach
        // expression position via turbofish, casts, and closure/fn param subtrees.
        k if is_type_level(k) => lo.empty_block(span),
        "macro_definition" => lower_macro_definition_shadow(lo, node),
        "line_comment" | "block_comment" | "attribute_item" | "mutable_specifier" => {
            lo.empty_block(span)
        }
        _ => {
            let kids: Vec<NodeId> = Lowering::named_children(node)
                .into_iter()
                .map(|c| lower_expr(lo, c))
                .collect();
            lo.raw(node.kind(), span, &kids)
        }
    }
}

fn lower_expr_atom(lo: &mut Lowering, node: TsNode, span: Span) -> Option<NodeId> {
    let lowered = match node.kind() {
        "identifier"
        | "type_identifier"
        | "field_identifier"
        | "scoped_identifier"
        | "crate"
        | "super"
        | "shorthand_field_identifier" => lo.var(lo.text(node), span),
        "self" => lo.var("self", span),
        "integer_literal" => {
            let t = lo.text(node);
            lo.int_lit(strip_rust_decimal_int_suffix(t), span)
        }
        "float_literal" => lo.float_lit(lo.text(node), span),
        "negative_literal" => lower_negative_literal(lo, node),
        "string_literal" | "raw_string_literal" | "char_literal" => {
            let t = lo.text(node);
            lo.str_lit(t, span)
        }
        "boolean_literal" => {
            let b = lo.text(node) == "true";
            lo.add(NodeKind::Lit, Payload::LitBool(b), span, &[])
        }
        "unit_expression" => lo.add(NodeKind::Lit, Payload::Lit(LitClass::Null), span, &[]),
        "block" => lower_block(lo, node),
        "let_condition" => lower_let_condition(lo, node),
        "let_chain" => lower_let_chain(lo, node),
        _ => return None,
    };
    Some(lowered)
}

pub(super) fn is_type_level(k: &str) -> bool {
    matches!(
        k,
        "type_arguments"
            | "type_parameters"
            | "reference_type"
            | "primitive_type"
            | "generic_type"
            | "scoped_type_identifier"
            | "array_type"
            | "tuple_type"
            | "pointer_type"
            | "dynamic_type"
            | "lifetime"
            | "parameters"
            | "parameter"
            | "self_parameter"
            | "function_signature_item"
            | "where_clause"
            | "type_arguments_list"
            | "trait_bounds"
            | "type_binding"
            | "constrained_type_parameter"
    )
}
pub(super) fn lower_range_expr(lo: &mut Lowering, node: TsNode) -> NodeId {
    let span = lo.span(node);
    // Preserve start/end POSITIONS and inclusivity: `1..`, `..1`, `1..2`,
    // `1..=2` are all different. tree-sitter omits empty bounds and the
    // `..`/`..=` operator is anonymous, so collecting named children collapsed
    // `1..` and `..1` to `Seq(1)`. Split on the operator; emit a `None`
    // placeholder for each empty slot and a trailing `0`/`1` inclusivity flag.
    let (start, end, inclusive) = lower_range_bounds(lo, node);
    let none = |lo: &mut Lowering| lo.add(NodeKind::Lit, Payload::Lit(LitClass::Null), span, &[]);
    let s = start.unwrap_or_else(|| none(lo));
    let e = end.unwrap_or_else(|| none(lo));
    let flag = lo.int_lit(if inclusive { "1" } else { "0" }, span);
    let range = if inclusive {
        SourceRangeKind::RustInclusiveRangeExpression
    } else {
        SourceRangeKind::RustHalfOpenRangeExpression
    };
    lo.record_source_fact(span, SourceFactKind::Range(range));
    lo.add(NodeKind::Seq, Payload::None, span, &[s, e, flag])
}
pub(super) fn lower_seq_tagged(lo: &mut Lowering, node: TsNode, tag_str: &str) -> NodeId {
    let span = lo.span(node);
    let kids: Vec<NodeId> = Lowering::named_children(node)
        .into_iter()
        .map(|c| lower_expr(lo, c))
        .collect();
    let tag = lo.sym(tag_str);
    lo.add(NodeKind::Seq, Payload::Name(tag), span, &kids)
}
pub(super) fn lower_pattern_surface(lo: &mut Lowering, node: TsNode) -> NodeId {
    let span = lo.span(node);
    let constructor_path = matches!(node.kind(), "tuple_struct_pattern" | "struct_pattern")
        .then(|| constructor_path_node(node))
        .flatten();
    let constructor_id = constructor_path.map(|path| path.id());
    let mut kids = Vec::new();
    if let Some(path) = constructor_path {
        kids.push(lower_expr(lo, path));
    }
    kids.extend(
        Lowering::named_children(node)
            .into_iter()
            .filter(|child| Some(child.id()) != constructor_id)
            .filter(|child| !is_type_level(child.kind()))
            .filter(|child| child.kind() != "mutable_specifier")
            .map(|child| lower_expr(lo, child)),
    );
    lo.add(
        NodeKind::Seq,
        Payload::Name(lo.sym(rust_pattern_tag(node.kind()))),
        span,
        &kids,
    )
}
pub(super) fn rust_pattern_tag(kind: &str) -> &'static str {
    match kind {
        "tuple_struct_pattern" => "rust_tuple_struct_pattern",
        "struct_pattern" => "rust_struct_pattern",
        "field_pattern" => "rust_field_pattern",
        "remaining_field_pattern" => "rust_remaining_field_pattern",
        "captured_pattern" => "rust_captured_pattern",
        "or_pattern" => "rust_or_pattern",
        "wildcard_pattern" => "rust_wildcard_pattern",
        "shorthand_field_identifier_pattern" => "rust_shorthand_field_identifier_pattern",
        _ => "rust_pattern",
    }
}
pub(super) fn lower_binary(lo: &mut Lowering, node: TsNode) -> NodeId {
    crate::lower::binary(lo, node, rust_bin_op, lower_expr)
}
pub(super) fn lower_unary(lo: &mut Lowering, node: TsNode) -> NodeId {
    let span = lo.span(node);
    let operand = node
        .named_child(0)
        .map(|c| lower_expr(lo, c))
        .unwrap_or_else(|| lo.empty_block(span));
    // tree-sitter-rust unary_expression: the operator is an anonymous child
    // (`-`, `!`, or `*`). A dereference `*x` is reference-level — other languages
    // don't have it — so peel it to its operand, like `&x` (reference_expression),
    // so `*x > 0` converges with a plain `x > 0`.
    let txt = lo.text(node);
    if txt.starts_with('*') {
        return operand;
    }
    let op = if txt.starts_with('!') {
        Op::Not
    } else {
        Op::Neg
    };
    lo.add(NodeKind::UnOp, Payload::Op(op), span, &[operand])
}
/// See [`crate::lower::deref_store_target`]: `*x = v` must keep the store place.
pub(super) fn lower_store_target(lo: &mut Lowering, node: TsNode) -> NodeId {
    crate::lower::deref_store_target(
        lo,
        node,
        |lo, n| {
            (n.kind() == "unary_expression" && lo.text(n).starts_with('*'))
                .then(|| n.named_child(n.named_child_count().saturating_sub(1)))
                .flatten()
        },
        lower_expr,
    )
}
pub(super) fn lower_compound_assign(lo: &mut Lowering, node: TsNode) -> NodeId {
    crate::lower::compound_assignment(lo, node, rust_bin_op, lower_store_target, lower_expr)
}
pub(super) fn lower_call(lo: &mut Lowering, node: TsNode) -> NodeId {
    let span = lo.span(node);
    let mut kids = Vec::new();
    match node.child_by_field_name("function") {
        Some(f) => kids.push(lower_expr(lo, f)),
        None => kids.push(lo.empty_block(span)),
    }
    if let Some(args) = node.child_by_field_name("arguments") {
        for a in Lowering::named_children(args) {
            kids.push(lower_expr(lo, a));
        }
    }
    lo.add(NodeKind::Call, Payload::None, span, &kids)
}
/// `recv.method(args)` → `Call(Field(method, recv), args...)`, matching how the
/// JS/Python frontends model method calls (so `.map`/`.filter` etc. canonicalize).
pub(super) fn lower_method_call(lo: &mut Lowering, node: TsNode) -> NodeId {
    let span = lo.span(node);
    let recv = node
        .child_by_field_name("receiver")
        .map(|r| lower_expr(lo, r))
        .unwrap_or_else(|| lo.empty_block(span));
    let method = node
        .child_by_field_name("method")
        .map(|m| lo.sym(lo.text(m)));
    let callee = lo.add(
        NodeKind::Field,
        method.map(Payload::Name).unwrap_or(Payload::None),
        span,
        &[recv],
    );
    let mut kids = vec![callee];
    if let Some(args) = node.child_by_field_name("arguments") {
        for a in Lowering::named_children(args) {
            kids.push(lower_expr(lo, a));
        }
    }
    lo.add(NodeKind::Call, Payload::None, span, &kids)
}
pub(super) fn lower_field(lo: &mut Lowering, node: TsNode) -> NodeId {
    let span = lo.span(node);
    let base = node
        .child_by_field_name("value")
        .map(|v| lower_expr(lo, v))
        .unwrap_or_else(|| lo.empty_block(span));
    let field = node
        .child_by_field_name("field")
        .map(|f| lo.sym(lo.text(f)));
    let field_id = lo.add(
        NodeKind::Field,
        field.map(Payload::Name).unwrap_or(Payload::None),
        span,
        &[base],
    );
    record_rust_self_field_runtime_domain(lo, node, field_id);
    field_id
}
fn record_rust_self_field_runtime_domain(lo: &mut Lowering, node: TsNode, field_id: NodeId) {
    if lo.b.kind(field_id) != NodeKind::Field || !rust_field_expression_base_is_self(node) {
        return;
    }
    if !rust_enclosing_function_has_self_parameter(node) {
        return;
    }
    let Some(field_name) = node
        .child_by_field_name("field")
        .map(|field| lo.text(field))
    else {
        return;
    };
    let Some(impl_type) = rust_enclosing_impl_type_name(lo, node) else {
        return;
    };
    let Some(type_node) = rust_struct_field_type_in_same_scope(lo, node, &impl_type, field_name)
    else {
        return;
    };
    let Some((domain, dependencies)) =
        rust_tokio_runtime_nominal_type_domain(lo, type_node, lo.text(type_node))
    else {
        return;
    };
    lo.record_node_domain_with_dependencies(
        lo.b.node(field_id).span,
        NodeKind::Field,
        domain,
        dependencies,
    );
}
fn rust_field_expression_base_is_self(node: TsNode) -> bool {
    node.child_by_field_name("value")
        .is_some_and(|base| base.kind() == "self")
}
fn rust_enclosing_function_has_self_parameter(mut node: TsNode) -> bool {
    while let Some(parent) = node.parent() {
        if parent.kind() == "function_item" {
            return parent
                .child_by_field_name("parameters")
                .is_some_and(rust_params_include_self);
        }
        node = parent;
    }
    false
}
fn rust_params_include_self(params: TsNode) -> bool {
    Lowering::named_children(params)
        .into_iter()
        .any(|child| child.kind() == "self_parameter")
}
fn rust_enclosing_impl_type_name(lo: &Lowering, mut node: TsNode) -> Option<String> {
    while let Some(parent) = node.parent() {
        if parent.kind() == "impl_item" {
            let type_node = parent.child_by_field_name("type")?;
            let head = rust_type_head_preserving_case(lo.text(type_node))?;
            return (!head.contains("::")).then_some(head);
        }
        node = parent;
    }
    None
}
fn rust_struct_field_type_in_same_scope<'tree>(
    lo: &Lowering,
    node: TsNode<'tree>,
    struct_name: &str,
    field_name: &str,
) -> Option<TsNode<'tree>> {
    let scope = rust_enclosing_module_scope(node)?;
    let mut matches = Vec::new();
    for item in Lowering::named_children(scope) {
        if item.kind() != "struct_item" || rust_named_item_text(lo, item) != Some(struct_name) {
            continue;
        }
        if let Some(field_type) = rust_struct_field_type(lo, item, field_name) {
            matches.push(field_type);
        }
    }
    let [field_type] = matches.as_slice() else {
        return None;
    };
    Some(*field_type)
}
fn rust_named_item_text<'a>(lo: &'a Lowering, node: TsNode) -> Option<&'a str> {
    node.child_by_field_name("name").map(|name| lo.text(name))
}
fn rust_struct_field_type<'tree>(
    lo: &Lowering,
    struct_item: TsNode<'tree>,
    field_name: &str,
) -> Option<TsNode<'tree>> {
    let body = struct_item.child_by_field_name("body")?;
    let mut matches = Vec::new();
    for field in Lowering::named_children(body) {
        if field.kind() != "field_declaration" {
            continue;
        }
        let Some(name) = field.child_by_field_name("name") else {
            continue;
        };
        if lo.text(name) == field_name {
            matches.push(field.child_by_field_name("type")?);
        }
    }
    let [field_type] = matches.as_slice() else {
        return None;
    };
    Some(*field_type)
}
pub(super) fn lower_index(lo: &mut Lowering, node: TsNode) -> NodeId {
    let span = lo.span(node);
    let kids: Vec<NodeId> = Lowering::named_children(node)
        .into_iter()
        .map(|c| lower_expr(lo, c))
        .collect();
    lo.add(NodeKind::Index, Payload::None, span, &kids)
}
pub(super) fn lower_closure(lo: &mut Lowering, node: TsNode) -> NodeId {
    let span = lo.span(node);
    let mut kids = Vec::new();
    if let Some(params) = node.child_by_field_name("parameters") {
        lower_params(lo, params, &mut kids);
    }
    let body = node
        .child_by_field_name("body")
        .map(|b| lower_expr(lo, b))
        .unwrap_or_else(|| lo.empty_block(span));
    kids.push(body);
    lo.add(NodeKind::Lambda, Payload::None, span, &kids)
}
pub(super) fn lower_negative_literal(lo: &mut Lowering, node: TsNode) -> NodeId {
    let span = lo.span(node);
    let Some(child) = node.named_child(0) else {
        return lo.raw(node.kind(), span, &[]);
    };
    match child.kind() {
        "integer_literal" => {
            let text = lo.text(child);
            let magnitude = strip_rust_decimal_int_suffix(text);
            let signed = format!("-{magnitude}");
            lo.int_lit(&signed, span)
        }
        "float_literal" => {
            let signed = format!("-{}", lo.text(child));
            lo.float_lit(&signed, span)
        }
        _ => lo.raw(node.kind(), span, &[]),
    }
}
pub(super) fn strip_rust_decimal_int_suffix(text: &str) -> &str {
    let trimmed = text.trim();
    if matches!(
        trimmed.get(..2),
        Some("0x" | "0X" | "0b" | "0B" | "0o" | "0O")
    ) {
        return trimmed;
    }
    let end = trimmed
        .char_indices()
        .find(|&(_, ch)| ch.is_ascii_alphabetic())
        .map(|(idx, _)| idx)
        .unwrap_or(trimmed.len());
    trimmed[..end].trim_end_matches('_')
}
pub(super) fn lower_struct_expr(lo: &mut Lowering, node: TsNode) -> NodeId {
    let span = lo.span(node);
    let kids: Vec<NodeId> = node
        .child_by_field_name("body")
        .map(|b| {
            Lowering::named_children(b)
                .into_iter()
                .filter_map(|f| match f.kind() {
                    "shorthand_field_identifier" | "field_identifier" | "identifier" => {
                        Some(lower_expr(lo, f))
                    }
                    _ => match f.child_by_field_name("value") {
                        Some(value) => Some(lower_expr(lo, value)),
                        None => {
                            let text = lo.text(f).trim();
                            simple_rust_ident(text).then(|| lo.var(text, lo.span(f)))
                        }
                    },
                })
                .collect()
        })
        .unwrap_or_default();
    lo.add(
        NodeKind::Seq,
        Payload::Name(lo.sym("rust_struct_expression")),
        span,
        &kids,
    )
}
pub(super) fn rust_bin_op(text: &str) -> Option<Op> {
    // Rust's binary operators are exactly the shared C-family set.
    crate::lower::common_bin_op(text)
}
