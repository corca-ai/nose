use super::*;

pub(super) fn lower_expr(lo: &mut Lowering, node: TsNode) -> NodeId {
    lower_expr_with_iota(lo, node, None)
}
pub(super) fn lower_expr_with_iota(
    lo: &mut Lowering,
    node: TsNode,
    iota_value: Option<usize>,
) -> NodeId {
    let span = lo.span(node);
    match node.kind() {
        "iota" => iota_value
            .map(|value| lo.int_lit(&value.to_string(), span))
            .unwrap_or_else(|| lo.raw("iota", span, &[])),
        "identifier" | "field_identifier" | "package_identifier" | "type_identifier" => {
            match lo.text(node) {
                "true" => lo.add(NodeKind::Lit, Payload::LitBool(true), span, &[]),
                "false" => lo.add(NodeKind::Lit, Payload::LitBool(false), span, &[]),
                "nil" => lo.add(NodeKind::Lit, Payload::Lit(LitClass::Null), span, &[]),
                other => lo.var(other, span),
            }
        }
        "int_literal" => {
            let t = lo.text(node);
            lo.int_lit(t, span)
        }
        "float_literal" | "imaginary_literal" => lo.float_lit(lo.text(node), span),
        "interpreted_string_literal" | "raw_string_literal" | "rune_literal" => {
            let t = lo.text(node);
            lo.str_lit(t, span)
        }
        "true" => lo.add(NodeKind::Lit, Payload::LitBool(true), span, &[]),
        "false" => lo.add(NodeKind::Lit, Payload::LitBool(false), span, &[]),
        "nil" => lo.add(NodeKind::Lit, Payload::Lit(LitClass::Null), span, &[]),
        "call_expression" if iota_value.is_some() => lower_call_with_iota(lo, node, iota_value),
        "call_expression" => lower_call(lo, node),
        "binary_expression" if iota_value.is_some() => lower_binary_with_iota(lo, node, iota_value),
        "binary_expression" => lower_binary(lo, node),
        "unary_expression" if iota_value.is_some() => lower_unary_with_iota(lo, node, iota_value),
        "unary_expression" => lower_unary(lo, node),
        "selector_expression" => lower_selector(lo, node),
        "type_instantiation_expression" => lower_type_instantiation(lo, node),
        "index_expression" => {
            let base = node
                .child_by_field_name("operand")
                .map(|o| lower_expr_with_iota(lo, o, iota_value))
                .unwrap_or_else(|| lo.empty_block(span));
            let idx = node
                .child_by_field_name("index")
                .map(|i| lower_expr_with_iota(lo, i, iota_value))
                .unwrap_or_else(|| lo.empty_block(span));
            lo.add(NodeKind::Index, Payload::None, span, &[base, idx])
        }
        "func_literal" => lower_func_literal(lo, node),
        "composite_literal" => lower_composite_literal(lo, node),
        "literal_element" | "keyed_element" => {
            let kids: Vec<NodeId> = Lowering::named_children(node)
                .into_iter()
                .map(|c| lower_expr_with_iota(lo, c, iota_value))
                .collect();
            if kids.len() == 1 {
                kids[0]
            } else {
                let tag = lo.sym(node.kind());
                lo.add(NodeKind::Seq, Payload::Name(tag), span, &kids)
            }
        }
        "parenthesized_expression" => node
            .named_child(0)
            .map(|c| lower_expr_with_iota(lo, c, iota_value))
            .unwrap_or_else(|| lo.empty_block(span)),
        "type_conversion_expression" => lower_type_conversion_expression(lo, node, iota_value),
        // A bare `{ … }` initializer (a nested composite, or a body reached
        // directly) → a `Seq` of its elements.
        "literal_value" => {
            let kids: Vec<NodeId> = Lowering::named_children(node)
                .into_iter()
                .map(|c| lower_expr_with_iota(lo, c, iota_value))
                .collect();
            lo.add(NodeKind::Seq, Payload::None, span, &kids)
        }
        // `a[lo:hi:cap]` → indexing shape: base plus whatever bounds are present.
        "slice_expression" => lower_slice_expr(lo, node),
        // `x.(T)` is a type-level check; keep the operand `x`, drop the assertion.
        "type_assertion_expression" => node
            .child_by_field_name("operand")
            .or_else(|| node.named_child(0))
            .map(|o| lower_expr_with_iota(lo, o, iota_value))
            .unwrap_or_else(|| lo.empty_block(span)),
        // `args...` → the spread operand.
        "variadic_argument" => node
            .named_child(0)
            .map(|c| lower_expr_with_iota(lo, c, iota_value))
            .unwrap_or_else(|| lo.empty_block(span)),
        // Type expressions in value position (`[]int`, `map[K]V`, `pkg.T`, …) carry
        // no behavior — collapse to an abstract literal so they aren't `Raw` noise.
        k if is_type_surface_kind(k) => {
            lo.add(NodeKind::Lit, Payload::Lit(LitClass::Other), span, &[])
        }
        _ => {
            let kids: Vec<NodeId> = Lowering::named_children(node)
                .into_iter()
                .map(|c| lower_expr_with_iota(lo, c, iota_value))
                .collect();
            lo.raw(node.kind(), span, &kids)
        }
    }
}
pub(super) fn lower_type_instantiation(lo: &mut Lowering, node: TsNode) -> NodeId {
    let span = lo.span(node);
    let Some(operand) = node
        .child_by_field_name("type")
        .or_else(|| Lowering::named_children(node).into_iter().next())
    else {
        return lo.empty_block(span);
    };
    let mut acc = lower_type_surface_as_value(lo, operand);
    for arg in Lowering::named_children(node)
        .into_iter()
        .filter(|arg| arg.id() != operand.id())
        .flat_map(type_argument_nodes)
    {
        let index = lower_type_surface_as_value(lo, arg);
        acc = lo.add(NodeKind::Index, Payload::None, span, &[acc, index]);
    }
    acc
}
pub(super) fn lower_type_conversion_expression(
    lo: &mut Lowering,
    node: TsNode,
    iota_value: Option<usize>,
) -> NodeId {
    let span = lo.span(node);
    let operand = node
        .child_by_field_name("operand")
        .map(|o| lower_expr_with_iota(lo, o, iota_value))
        .unwrap_or_else(|| lo.empty_block(span));
    let Some(type_node) = node.child_by_field_name("type") else {
        return operand;
    };
    if is_ambiguous_type_conversion_callee(type_node) {
        let callee = lower_type_surface_as_value(lo, type_node);
        return lo.add(
            NodeKind::Seq,
            Payload::Name(lo.sym("go_type_conversion_or_index_call")),
            span,
            &[callee, operand],
        );
    }
    operand
}
fn is_ambiguous_type_conversion_callee(node: TsNode) -> bool {
    matches!(
        node.kind(),
        "generic_type" | "type_instantiation_expression"
    )
}
pub(super) fn lower_type_surface_as_value(lo: &mut Lowering, node: TsNode) -> NodeId {
    let span = lo.span(node);
    match node.kind() {
        "qualified_type" => lower_qualified_type_as_value(lo, node),
        "generic_type" => lower_generic_type_as_value(lo, node),
        "type_arguments" | "type_elem" => lower_type_argument_container_as_value(lo, node),
        "parenthesized_type" => Lowering::named_children(node)
            .into_iter()
            .next()
            .map(|child| lower_type_surface_as_value(lo, child))
            .unwrap_or_else(|| lo.empty_block(span)),
        "type_instantiation_expression" => lower_type_instantiation(lo, node),
        "type_identifier" | "identifier" | "package_identifier" => lo.var(lo.text(node), span),
        _ if is_type_surface_kind(node.kind()) => lo.str_lit(lo.text(node), span),
        _ => lower_expr(lo, node),
    }
}
pub(super) fn lower_generic_type_as_value(lo: &mut Lowering, node: TsNode) -> NodeId {
    let span = lo.span(node);
    let Some(operand) = node
        .child_by_field_name("type")
        .or_else(|| Lowering::named_children(node).into_iter().next())
    else {
        return lo.empty_block(span);
    };
    let mut acc = lower_type_surface_as_value(lo, operand);
    let arg_nodes: Vec<TsNode> = node
        .child_by_field_name("type_arguments")
        .map(type_argument_nodes)
        .unwrap_or_else(|| {
            Lowering::named_children(node)
                .into_iter()
                .filter(|arg| arg.id() != operand.id())
                .flat_map(type_argument_nodes)
                .collect()
        });
    for arg in arg_nodes {
        let index = lower_type_surface_as_value(lo, arg);
        acc = lo.add(NodeKind::Index, Payload::None, span, &[acc, index]);
    }
    acc
}
fn lower_type_argument_container_as_value(lo: &mut Lowering, node: TsNode) -> NodeId {
    let span = lo.span(node);
    let args = type_argument_nodes(node);
    if args.is_empty() {
        return lo.empty_block(span);
    }
    if args.len() == 1 && args[0].id() != node.id() {
        return lower_type_surface_as_value(lo, args[0]);
    }
    let kids: Vec<NodeId> = args
        .into_iter()
        .map(|arg| {
            if arg.id() == node.id() {
                lo.str_lit(lo.text(arg), lo.span(arg))
            } else {
                lower_type_surface_as_value(lo, arg)
            }
        })
        .collect();
    lo.add(NodeKind::Seq, Payload::None, span, &kids)
}
fn type_argument_nodes(node: TsNode) -> Vec<TsNode> {
    match node.kind() {
        "type_arguments" => Lowering::named_children(node)
            .into_iter()
            .flat_map(type_argument_nodes)
            .collect(),
        "type_elem" => {
            let children = Lowering::named_children(node);
            if children.is_empty() {
                vec![node]
            } else {
                children.into_iter().flat_map(type_argument_nodes).collect()
            }
        }
        _ => vec![node],
    }
}
pub(super) fn lower_qualified_type_as_value(lo: &mut Lowering, node: TsNode) -> NodeId {
    let span = lo.span(node);
    let mut parts = Lowering::named_children(node).into_iter();
    let Some(first) = parts.next() else {
        return lo.var(lo.text(node), span);
    };
    let mut acc = lo.var(lo.text(first), lo.span(first));
    for part in parts {
        acc = lo.add(
            NodeKind::Field,
            Payload::Name(lo.sym(lo.text(part))),
            lo.span(part),
            &[acc],
        );
    }
    acc
}
pub(super) fn lower_binary_with_iota(
    lo: &mut Lowering,
    node: TsNode,
    iota_value: Option<usize>,
) -> NodeId {
    if node.child_by_field_name("operator").map(|o| lo.text(o)) == Some("&^") {
        let span = lo.span(node);
        let l = node
            .child_by_field_name("left")
            .map(|x| lower_expr_with_iota(lo, x, iota_value))
            .unwrap_or_else(|| lo.empty_block(span));
        let r = node
            .child_by_field_name("right")
            .map(|x| lower_expr_with_iota(lo, x, iota_value))
            .unwrap_or_else(|| lo.empty_block(span));
        return go_bitclear(lo, span, l, r);
    }
    crate::lower::binary(lo, node, go_bin_op, |lo, n| {
        lower_expr_with_iota(lo, n, iota_value)
    })
}
pub(super) fn lower_unary_with_iota(
    lo: &mut Lowering,
    node: TsNode,
    iota_value: Option<usize>,
) -> NodeId {
    let span = lo.span(node);
    let op_text = node
        .child_by_field_name("operator")
        .map(|o| lo.text(o))
        .unwrap_or("-");
    let operand = node.child_by_field_name("operand");
    match op_text {
        "-" | "+" | "!" | "^" => {
            let op = match op_text {
                "-" => Op::Neg,
                "+" => Op::Pos,
                "!" => Op::Not,
                _ => Op::BitNot,
            };
            let arg = operand
                .map(|a| lower_expr_with_iota(lo, a, iota_value))
                .unwrap_or_else(|| lo.empty_block(span));
            lo.add(NodeKind::UnOp, Payload::Op(op), span, &[arg])
        }
        _ => operand
            .map(|a| lower_expr_with_iota(lo, a, iota_value))
            .unwrap_or_else(|| lo.empty_block(span)),
    }
}
pub(super) fn lower_selector(lo: &mut Lowering, node: TsNode) -> NodeId {
    let span = lo.span(node);
    let obj = node
        .child_by_field_name("operand")
        .map(|o| lower_expr(lo, o))
        .unwrap_or_else(|| lo.empty_block(span));
    let field = node
        .child_by_field_name("field")
        .map(|f| lo.text(f))
        .unwrap_or("");
    let sym = lo.sym(field);
    lo.add(NodeKind::Field, Payload::Name(sym), span, &[obj])
}
pub(super) fn lower_func_literal(lo: &mut Lowering, node: TsNode) -> NodeId {
    let span = lo.span(node);
    let mut kids = Vec::new();
    if let Some(params) = node.child_by_field_name("parameters") {
        lower_params(lo, params, &mut kids);
    }
    let body = node
        .child_by_field_name("body")
        .map(|b| lower_block(lo, b))
        .unwrap_or_else(|| lo.empty_block(span));
    kids.push(body);
    lo.add(NodeKind::Lambda, Payload::None, span, &kids)
}
pub(super) fn lower_composite_literal(lo: &mut Lowering, node: TsNode) -> NodeId {
    let span = lo.span(node);
    let body = node.child_by_field_name("body");
    let kids: Vec<NodeId> = match body {
        Some(b) => Lowering::named_children(b)
            .into_iter()
            .map(|c| lower_expr(lo, c))
            .collect(),
        None => Vec::new(),
    };
    // Tag the literal by its TYPE so the kinds stay semantically distinct (the old
    // blanket `composite_literal` tag collapsed slice ≡ map ≡ struct to one value):
    //   • slice/array  → `array`  — an ordered sequence; converges with `[1,2]` / JS.
    //   • map          → `composite_literal` — preserves go-map handling
    //                    (`proven_go_literal_zero_map_seq`, which keys on this tag).
    //   • struct/named → `go_struct` — a record, NOT a collection; a distinct value so
    //                    `Point{1,2}` no longer value-merges with `[1,2]`.
    // Named-type aliases default to `go_struct` (conservative: distinct, never a false
    // sequence merge — at worst a missed convergence for a named slice alias).
    let tag_str = match node.child_by_field_name("type").map(|t| t.kind()) {
        Some("slice_type" | "array_type") => "array",
        Some("map_type") => "composite_literal",
        _ => "go_struct",
    };
    let tag = lo.sym(tag_str);
    lo.add(NodeKind::Seq, Payload::Name(tag), span, &kids)
}
pub(super) fn lower_slice_expr(lo: &mut Lowering, node: TsNode) -> NodeId {
    let span = lo.span(node);
    let base = node
        .child_by_field_name("operand")
        .map(|o| lower_expr(lo, o))
        .unwrap_or_else(|| lo.empty_block(span));
    // Preserve slot POSITIONS: `a[1:]` (start) and `a[:1]` (end) are different
    // slices. A missing bound emits an explicit `None` placeholder so the
    // operands stay positional rather than both collapsing to `Index(a, 1)`.
    let mut kids = vec![base];
    for field in ["start", "end", "capacity"] {
        let v = node
            .child_by_field_name(field)
            .map(|n| lower_expr(lo, n))
            .unwrap_or_else(|| lo.add(NodeKind::Lit, Payload::Lit(LitClass::Null), span, &[]));
        kids.push(v);
    }
    lo.add(NodeKind::Index, Payload::None, span, &kids)
}
pub(super) fn lower_call(lo: &mut Lowering, node: TsNode) -> NodeId {
    let span = lo.span(node);
    let mut kids = Vec::new();
    kids.push(lower_call_function(lo, node, None));
    kids.extend(crate::lower::call_arguments(lo, node, lower_expr));
    lo.add(NodeKind::Call, Payload::None, span, &kids)
}
pub(super) fn lower_call_with_iota(
    lo: &mut Lowering,
    node: TsNode,
    iota_value: Option<usize>,
) -> NodeId {
    let span = lo.span(node);
    let mut kids = Vec::new();
    kids.push(lower_call_function(lo, node, iota_value));
    kids.extend(crate::lower::call_arguments(lo, node, |lo, argument| {
        lower_expr_with_iota(lo, argument, iota_value)
    }));
    lo.add(NodeKind::Call, Payload::None, span, &kids)
}
pub(super) fn lower_call_function(
    lo: &mut Lowering,
    node: TsNode,
    iota_value: Option<usize>,
) -> NodeId {
    let span = lo.span(node);
    let mut callee = match node.child_by_field_name("function") {
        Some(f) if iota_value.is_none() => lower_call_callee(lo, f),
        Some(f) => lower_expr_with_iota(lo, f, iota_value),
        None => lo.empty_block(span),
    };
    if let Some(type_args) = node.child_by_field_name("type_arguments") {
        for arg in type_argument_nodes(type_args) {
            let index = lower_type_surface_as_value(lo, arg);
            callee = lo.add(NodeKind::Index, Payload::None, span, &[callee, index]);
        }
    }
    callee
}
pub(super) fn lower_call_callee(lo: &mut Lowering, node: TsNode) -> NodeId {
    lower_expr(lo, node)
}
pub(super) fn lower_binary(lo: &mut Lowering, node: TsNode) -> NodeId {
    // `a &^ b` (bit-clear / AND-NOT) desugars to `a & ^b`; the generic op-map can only
    // yield a single BinOp, so it is built here rather than in `go_bin_op`.
    if node.child_by_field_name("operator").map(|o| lo.text(o)) == Some("&^") {
        let span = lo.span(node);
        let l = node
            .child_by_field_name("left")
            .map(|x| lower_expr(lo, x))
            .unwrap_or_else(|| lo.empty_block(span));
        let r = node
            .child_by_field_name("right")
            .map(|x| lower_expr(lo, x))
            .unwrap_or_else(|| lo.empty_block(span));
        return go_bitclear(lo, span, l, r);
    }
    crate::lower::binary(lo, node, go_bin_op, lower_expr)
}
/// Go's bit-clear `a &^ b` ≡ `a & ^b` (AND-NOT) — it clears the bits of `a` that are set
/// in `b`, which is NOT the same as `a & b`. Desugar to that two-node form so the two
/// operators don't collapse to one fingerprint.
pub(super) fn go_bitclear(lo: &mut Lowering, span: Span, l: NodeId, r: NodeId) -> NodeId {
    let not_r = lo.add(NodeKind::UnOp, Payload::Op(Op::BitNot), span, &[r]);
    lo.add(NodeKind::BinOp, Payload::Op(Op::BitAnd), span, &[l, not_r])
}
pub(super) fn lower_unary(lo: &mut Lowering, node: TsNode) -> NodeId {
    let span = lo.span(node);
    let op_text = node
        .child_by_field_name("operator")
        .map(|o| lo.text(o))
        .unwrap_or("-");
    let operand = node.child_by_field_name("operand");
    match op_text {
        "-" | "+" | "!" | "^" => {
            let op = match op_text {
                "-" => Op::Neg,
                "+" => Op::Pos,
                "!" => Op::Not,
                _ => Op::BitNot,
            };
            let arg = operand
                .map(|a| lower_expr(lo, a))
                .unwrap_or_else(|| lo.empty_block(span));
            lo.add(NodeKind::UnOp, Payload::Op(op), span, &[arg])
        }
        "<-" => lower_channel_receive_expr(lo, node),
        // `*p`, `&x`: strip to operand. Pointer/place semantics need a separate
        // place-evidence slice; channel receive is preserved above because it has
        // observable synchronization behavior.
        _ => operand
            .map(|a| lower_expr(lo, a))
            .unwrap_or_else(|| lo.empty_block(span)),
    }
}
pub(super) fn go_bin_op(text: &str) -> Option<Op> {
    // `&^` (bit-clear) is handled by desugaring in `lower_binary` / the compound path,
    // not here, since it expands to two nodes rather than a single BinOp.
    crate::lower::common_bin_op(text)
}
pub(super) fn js_like_compound_op(text: &str) -> Option<Op> {
    go_bin_op(text.trim_end_matches('='))
}
