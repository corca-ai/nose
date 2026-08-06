use super::*;
use crate::tree_sitter_ext::last_named_child;

pub(super) fn lower_expr(lo: &mut Lowering, node: TsNode) -> NodeId {
    let span = lo.span(node);
    match node.kind() {
        "identifier" | "type_identifier" | "scoped_identifier" => lo.var(lo.text(node), span),
        "this" => lo.var("this", span),
        "decimal_integer_literal"
        | "hex_integer_literal"
        | "octal_integer_literal"
        | "binary_integer_literal" => {
            let t = lo.text(node);
            lo.int_lit(t.trim_end_matches(['L', 'l']), span)
        }
        "decimal_floating_point_literal" | "hex_floating_point_literal" => {
            lo.float_lit(lo.text(node), span)
        }
        "string_literal" | "character_literal" | "text_block" => {
            let t = lo.text(node);
            lo.str_lit(t, span)
        }
        "true" => lo.add(NodeKind::Lit, Payload::LitBool(true), span, &[]),
        "false" => lo.add(NodeKind::Lit, Payload::LitBool(false), span, &[]),
        "null_literal" => lo.add(NodeKind::Lit, Payload::Lit(LitClass::Null), span, &[]),
        "binary_expression" => lower_binary(lo, node),
        "unary_expression" => lower_unary(lo, node),
        "assignment_expression" => {
            let l = node
                .child_by_field_name("left")
                .map(|x| lower_expr(lo, x))
                .unwrap_or_else(|| lo.empty_block(span));
            // compound `x += y` → `x = x op y`
            let opt = node
                .child_by_field_name("operator")
                .map(|o| lo.text(o))
                .unwrap_or("=");
            let r = node
                .child_by_field_name("right")
                .map(|x| lower_expr(lo, x))
                .unwrap_or_else(|| lo.empty_block(span));
            if opt.len() > 1 {
                let l2 = node
                    .child_by_field_name("left")
                    .map(|x| lower_expr(lo, x))
                    .unwrap_or_else(|| lo.empty_block(span));
                // An unmapped compound operator (e.g. `>>>=`) keeps its own raw
                // shape — dropping the operator would merge it with `x = y`.
                let value = match common_bin_op(opt.trim_end_matches('=')) {
                    Some(op) => lo.add(NodeKind::BinOp, Payload::Op(op), span, &[l2, r]),
                    None if opt.trim_end_matches('=') == ">>>" => {
                        lower_unsigned_shift_right(lo, span, l2, r)
                    }
                    None => lo.raw(&format!("compound_assignment {opt}"), span, &[l2, r]),
                };
                return lo.add(NodeKind::Assign, Payload::None, span, &[l, value]);
            }
            lo.add(NodeKind::Assign, Payload::None, span, &[l, r])
        }
        "update_expression" => lower_update(lo, node),
        "method_invocation" => lower_call(lo, node),
        "switch_expression" => lower_switch_expr(lo, node),
        "object_creation_expression" => lower_object_creation(lo, node),
        "field_access" => {
            let base = node
                .child_by_field_name("object")
                .map(|o| lower_expr(lo, o))
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
            record_java_this_field_receiver_domain(lo, node, field_id);
            field_id
        }
        "array_access" => {
            let kids: Vec<NodeId> = Lowering::named_children(node)
                .into_iter()
                .map(|c| lower_expr(lo, c))
                .collect();
            lo.add(NodeKind::Index, Payload::None, span, &kids)
        }
        "lambda_expression" => lower_lambda(lo, node),
        _ => lower_expr_rest(lo, node),
    }
}

fn lower_object_creation(lo: &mut Lowering, node: TsNode) -> NodeId {
    let span = lo.span(node);
    let mut kids = crate::lower::call_arguments(lo, node, lower_expr);
    // Locally-bound anonymous objects are reusable behavior objects (visitors/recorders).
    // Expanding inline callback arguments would instead dominate their enclosing call shape.
    if anonymous_object_is_locally_bound(node) {
        let body = node.child_by_field_name("body").or_else(|| {
            Lowering::named_children(node)
                .into_iter()
                .find(|child| child.kind() == "class_body")
        });
        if let Some(body) = body.filter(|&body| anonymous_body_is_substantive(body)) {
            kids.push(lower_anonymous_body_declarations(lo, body));
        }
    }
    if let Some(list) = lower_empty_java_collection_constructor(lo, node, &kids, span) {
        return list;
    }
    if let Some(future) = lower_java_completable_future_constructor(lo, node, &kids, span) {
        return future;
    }
    lo.record_source_fact(span, SourceFactKind::Call(SourceCallKind::Construct));
    lo.add(NodeKind::Call, Payload::None, span, &kids)
}

fn anonymous_object_is_locally_bound(node: TsNode) -> bool {
    node.parent().is_some_and(|parent| {
        matches!(
            parent.kind(),
            "variable_declarator" | "assignment_expression"
        )
    })
}

/// Keep multi-step behavior objects visible to connected-witness matching while leaving
/// small defensive overrides (for example throw-only equality guards) constructor-shaped.
/// Four executable statements is enough to admit visitor/recorder implementations without
/// perturbing existing method similarities for incidental anonymous classes.
fn anonymous_body_is_substantive(body: TsNode) -> bool {
    const MIN_EXECUTABLE_STATEMENTS: usize = 4;

    let mut executable_statements = 0;
    let mut stack = vec![body];
    while let Some(node) = stack.pop() {
        if matches!(
            node.kind(),
            "expression_statement"
                | "local_variable_declaration"
                | "return_statement"
                | "throw_statement"
        ) {
            executable_statements += 1;
            if executable_statements >= MIN_EXECUTABLE_STATEMENTS {
                return true;
            }
        }
        stack.extend(Lowering::named_children(node));
    }
    false
}
/// Tail of [`lower_expr`]'s dispatch: grouping/cast wrappers, aggregate
/// initializers, constructor delegation, label/constant kinds, and type-level
/// nodes that reach expression position.
pub(super) fn lower_expr_rest(lo: &mut Lowering, node: TsNode) -> NodeId {
    let span = lo.span(node);
    match node.kind() {
        "parenthesized_expression" | "cast_expression" => last_named_child(node)
            .map(|c| lower_expr(lo, c))
            .unwrap_or_else(|| lo.empty_block(span)),
        "ternary_expression" => crate::lower::conditional_expression(lo, node, lower_expr),
        "array_initializer" | "array_creation_expression" | "argument_list" => {
            let kids: Vec<NodeId> = Lowering::named_children(node)
                .into_iter()
                .map(|c| lower_expr(lo, c))
                .collect();
            lo.add(NodeKind::Seq, Payload::None, span, &kids)
        }
        // `Foo.class` → a field access named `class` over the (erased) type.
        "class_literal" => {
            let base = node
                .named_child(0)
                .map(|c| lower_expr(lo, c))
                .unwrap_or_else(|| lo.empty_block(span));
            let f = lo.sym("class");
            lo.add(NodeKind::Field, Payload::Name(f), span, &[base])
        }
        "super" => lo.var("super", span),
        // `x instanceof T` → the runtime value being tested (type erased).
        "instanceof_expression" => node
            .named_child(0)
            .map(|c| lower_expr(lo, c))
            .unwrap_or_else(|| lo.empty_block(span)),
        // `this(...)` / `super(...)` constructor delegation → a Call over its args.
        "explicit_constructor_invocation" => {
            let kids = crate::lower::call_arguments(lo, node, lower_expr);
            lo.add(NodeKind::Call, Payload::None, span, &kids)
        }
        "method_reference" => lo.var(lo.text(node), span),
        // `case V:` label in an expression-position group → the matched value.
        "switch_label" => node
            .named_child(0)
            .map(|c| lower_expr(lo, c))
            .unwrap_or_else(|| lo.empty_block(span)),
        // `new int[n]` size carries behavior — keep the inner expression.
        "dimensions_expr" => node
            .named_child(0)
            .map(|c| lower_expr(lo, c))
            .unwrap_or_else(|| lo.empty_block(span)),
        "enum_constant" => lower_enum_constant(lo, node),
        "module_body" => lower_module_body(lo, node),
        "requires_module_directive"
        | "exports_module_directive"
        | "opens_module_directive"
        | "uses_module_directive"
        | "provides_module_directive" => lower_module_directive(lo, node),
        // Statement nodes reaching expression position (switch-rule bodies, etc.).
        "block" => lower_block(lo, node),
        "expression_statement" => node
            .named_child(0)
            .map(|c| lower_expr(lo, c))
            .unwrap_or_else(|| lo.empty_block(span)),
        // Type-level nodes carry no behavior — erase rather than Raw.
        "integral_type"
        | "floating_point_type"
        | "boolean_type"
        | "void_type"
        | "generic_type"
        | "array_type"
        | "scoped_type_identifier"
        | "dimensions"
        | "type_arguments"
        | "wildcard"
        | "type_parameters"
        | "type_parameter"
        | "type_bound"
        | "annotation"
        | "marker_annotation"
        | "annotation_argument_list"
        | "annotation_type_element_declaration"
        | "annotation_type_body"
        | "super_interfaces"
        | "extends_interfaces"
        | "type_list"
        | "throws"
        | "modifiers"
        | "requires_modifier" => lo.empty_block(span),
        _ => {
            let kids: Vec<NodeId> = Lowering::named_children(node)
                .into_iter()
                .map(|c| lower_expr(lo, c))
                .collect();
            lo.raw(node.kind(), span, &kids)
        }
    }
}
pub(super) fn lower_enum_constant(lo: &mut Lowering, node: TsNode) -> NodeId {
    let span = lo.span(node);
    let mut kids = Vec::new();
    if let Some(name) = node.child_by_field_name("name") {
        kids.push(lo.str_lit(lo.text(name), lo.span(name)));
    }
    kids.extend(crate::lower::call_arguments(lo, node, lower_expr));
    if let Some(body) = node.child_by_field_name("body") {
        kids.push(lower_body_declarations(lo, body));
    }
    lo.add(
        NodeKind::Seq,
        Payload::Name(lo.sym("java_enum_constant")),
        span,
        &kids,
    )
}
pub(super) fn lower_unary(lo: &mut Lowering, node: TsNode) -> NodeId {
    let span = lo.span(node);
    let operand = node
        .child_by_field_name("operand")
        .map(|o| lower_expr(lo, o))
        .unwrap_or_else(|| lo.empty_block(span));
    // Map by the operator token, not the leading byte: `+`→Pos, `-`→Neg,
    // `~`→BitNot, `!`→Not. Reading only the first byte collapsed `+x` and `~x`
    // onto `Neg` (same class of bug as the C/Ruby frontends).
    let op = match node.child_by_field_name("operator").map(|o| lo.text(o)) {
        Some("+") => Op::Pos,
        Some("~") => Op::BitNot,
        Some("!") => Op::Not,
        _ => Op::Neg,
    };
    lo.add(NodeKind::UnOp, Payload::Op(op), span, &[operand])
}
pub(super) fn lower_update(lo: &mut Lowering, node: TsNode) -> NodeId {
    // x++ / ++x → x = x + 1
    let span = lo.span(node);
    let operand = node
        .named_child(0)
        .map(|o| lower_expr(lo, o))
        .unwrap_or_else(|| lo.empty_block(span));
    let operand2 = node
        .named_child(0)
        .map(|o| lower_expr(lo, o))
        .unwrap_or_else(|| lo.empty_block(span));
    let one = lo.int_lit("1", span);
    // Decide by the operator TOKEN among this node's direct children: a substring
    // check on the whole text misreads a nested `--`/`++` in the operand (e.g.
    // `a[i--]++`, whose outer op is `++`).
    let op = if crate::lower::has_direct_token(node, "--") {
        Op::Sub
    } else {
        Op::Add
    };
    let bin = lo.add(NodeKind::BinOp, Payload::Op(op), span, &[operand2, one]);
    lo.add(NodeKind::Assign, Payload::None, span, &[operand, bin])
}
pub(super) fn lower_lambda(lo: &mut Lowering, node: TsNode) -> NodeId {
    let span = lo.span(node);
    let mut kids = Vec::new();
    let body_node = node.child_by_field_name("body");
    if let Some(params) = node.child_by_field_name("parameters") {
        for p in Lowering::named_children(params) {
            let psym = if p.kind() == "identifier" {
                Some(lo.sym(lo.text(p)))
            } else {
                p.child_by_field_name("name").map(|n| lo.sym(lo.text(n)))
            };
            let pspan = lo.span(p);
            kids.push(lo.add(
                NodeKind::Param,
                psym.map(Payload::Name).unwrap_or(Payload::None),
                pspan,
                &[],
            ));
        }
    } else if let Some(p) = node.child_by_field_name("parameter") {
        let psym = if p.kind() == "identifier" {
            Some(lo.sym(lo.text(p)))
        } else {
            p.child_by_field_name("name").map(|n| lo.sym(lo.text(n)))
        };
        kids.push(lo.add(
            NodeKind::Param,
            psym.map(Payload::Name).unwrap_or(Payload::None),
            lo.span(p),
            &[],
        ));
    } else if let Some(p) = node.named_child(0) {
        let body_start = body_node.map(|b| b.start_byte());
        if p.kind() == "identifier" && Some(p.start_byte()) != body_start {
            kids.push(lo.add(
                NodeKind::Param,
                Payload::Name(lo.sym(lo.text(p))),
                lo.span(p),
                &[],
            ));
        }
    }
    if kids.is_empty() {
        if let Some(name) = lambda_single_param_from_text(lo.text(node)) {
            kids.push(lo.add(NodeKind::Param, Payload::Name(lo.sym(name)), span, &[]));
        }
    }
    let body = body_node
        .map(|b| {
            if b.kind() == "block" {
                lower_block(lo, b)
            } else {
                lower_expr(lo, b)
            }
        })
        .unwrap_or_else(|| lo.empty_block(span));
    kids.push(body);
    lo.add(NodeKind::Lambda, Payload::None, span, &kids)
}
pub(super) fn lambda_single_param_from_text(text: &str) -> Option<&str> {
    let (head, _) = text.split_once("->")?;
    let head = head
        .trim()
        .trim_start_matches('(')
        .trim_end_matches(')')
        .trim();
    if head.is_empty() || head.contains(',') {
        return None;
    }
    let name = head.split_whitespace().last()?;
    if is_java_identifier(name) {
        Some(name)
    } else {
        None
    }
}
pub(super) fn is_java_identifier(name: &str) -> bool {
    let mut chars = name.chars();
    let Some(first) = chars.next() else {
        return false;
    };
    (first == '_' || first == '$' || first.is_ascii_alphabetic())
        && chars.all(|c| c == '_' || c == '$' || c.is_ascii_alphanumeric())
}

fn record_java_this_field_receiver_domain(lo: &mut Lowering, node: TsNode, field_id: NodeId) {
    if lo.b.kind(field_id) != NodeKind::Field || !java_field_access_base_is_this(node) {
        return;
    }
    let Some(field_name) = node
        .child_by_field_name("field")
        .map(|field| lo.text(field))
    else {
        return;
    };
    let Some(class_node) = java_enclosing_type_decl(node) else {
        return;
    };
    let Some(field_decl) = java_unique_field_declaration_in_type(lo, class_node, field_name) else {
        return;
    };
    let Some(domain) = java_receiver_declaration_domain(lo, field_decl) else {
        return;
    };
    lo.record_node_domain_with_dependencies(
        lo.b.node(field_id).span,
        NodeKind::Field,
        domain.domain,
        domain.dependencies,
    );
}

fn java_field_access_base_is_this(node: TsNode) -> bool {
    node.child_by_field_name("object")
        .is_some_and(|base| base.kind() == "this")
}

fn java_enclosing_type_decl<'tree>(mut node: TsNode<'tree>) -> Option<TsNode<'tree>> {
    while let Some(parent) = node.parent() {
        if java_is_nested_type_decl(parent.kind()) {
            return Some(parent);
        }
        node = parent;
    }
    None
}

fn java_unique_field_declaration_in_type<'tree>(
    lo: &Lowering<'_>,
    type_node: TsNode<'tree>,
    field_name: &str,
) -> Option<TsNode<'tree>> {
    let body = type_node.child_by_field_name("body")?;
    let mut found = None;
    for child in Lowering::named_children(body) {
        if !matches!(child.kind(), "field_declaration" | "constant_declaration") {
            continue;
        }
        if !java_field_declaration_defines(lo, child, field_name) {
            continue;
        }
        if found.replace(child).is_some() {
            return None;
        }
    }
    found
}

fn java_field_declaration_defines(lo: &Lowering<'_>, decl: TsNode, field_name: &str) -> bool {
    Lowering::named_children(decl)
        .into_iter()
        .filter(|child| child.kind() == "variable_declarator")
        .any(|declarator| {
            declarator
                .child_by_field_name("name")
                .is_some_and(|name| lo.text(name) == field_name)
        })
}
pub(super) fn lower_binary(lo: &mut Lowering, node: TsNode) -> NodeId {
    if node
        .child_by_field_name("operator")
        .is_some_and(|op| lo.text(op) == ">>>")
    {
        let span = lo.span(node);
        let left = node
            .child_by_field_name("left")
            .map(|child| lower_expr(lo, child))
            .unwrap_or_else(|| lo.empty_block(span));
        let right = node
            .child_by_field_name("right")
            .map(|child| lower_expr(lo, child))
            .unwrap_or_else(|| lo.empty_block(span));
        return lower_unsigned_shift_right(lo, span, left, right);
    }
    crate::lower::binary(lo, node, common_bin_op, lower_expr)
}
pub(super) fn lower_unsigned_shift_right(
    lo: &mut Lowering,
    span: Span,
    left: NodeId,
    right: NodeId,
) -> NodeId {
    let tag = lo.sym("java_unsigned_shift_right");
    lo.add(NodeKind::Seq, Payload::Name(tag), span, &[left, right])
}
/// `recv.method(args)` → `Call(Field(method, recv), args...)`.
pub(super) fn lower_call(lo: &mut Lowering, node: TsNode) -> NodeId {
    let span = lo.span(node);
    let name_node = node.child_by_field_name("name");
    let name = name_node.map(|n| lo.sym(lo.text(n)));
    let callee = match node.child_by_field_name("object") {
        Some(o) => {
            let recv = lower_expr(lo, o);
            lo.add(
                NodeKind::Field,
                name.map(Payload::Name).unwrap_or(Payload::None),
                span,
                &[recv],
            )
        }
        None => lo.add(
            NodeKind::Var,
            name.map(Payload::Name).unwrap_or(Payload::None),
            span,
            &[],
        ),
    };
    let mut kids = vec![callee];
    kids.extend(crate::lower::call_arguments(lo, node, lower_expr));
    lo.add(NodeKind::Call, Payload::None, span, &kids)
}
