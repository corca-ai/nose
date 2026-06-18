//! Swift → raw IL lowering.
//!
//! Swift is lowered as a statically-typed C-family frontend: functions and methods
//! become units, declarations and assignments lower to `Assign`, `for`/`while` /
//! `repeat while` map to unified loops, expression `if`/`switch` become canonical
//! conditionals, and calls / member / index expressions use the shared call shape.
//! `try` and `await` stay source-backed protocol boundaries until semantic
//! contracts can prove those effects are erasable.

use crate::lower::{common_bin_op, Lowering};
use nose_il::{
    stable_symbol_hash, Builtin, EvidenceAnchor, EvidenceKind, FileId, Il, Interner, Lang,
    LitClass, LoopKind, NodeId, NodeKind, Op, Payload, SourceProtocolKind, Span, Symbol, UnitKind,
};
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
        crate::lower::grammar::SWIFT,
        || tree_sitter_swift::LANGUAGE.into(),
        Lang::Swift,
        lower_items,
    )
}

fn lower_items(lo: &mut Lowering, node: TsNode) -> NodeId {
    crate::lower::collect_into(lo, node, NodeKind::Module, lower_item)
}

fn lower_item(lo: &mut Lowering, node: TsNode) -> Option<NodeId> {
    match node.kind() {
        "function_declaration" => Some(lower_function(lo, node, false)),
        "protocol_function_declaration" => Some(lower_function(lo, node, true)),
        "init_declaration" | "deinit_declaration" | "subscript_declaration" => {
            Some(lower_function(lo, node, true))
        }
        "class_declaration"
        | "struct_declaration"
        | "enum_declaration"
        | "protocol_declaration" => Some(lower_type(lo, node)),
        "extension_declaration" => Some(lower_extension(lo, node)),
        "property_declaration"
        | "protocol_property_declaration"
        | "protocol_property_requirements" => Some(lower_property(lo, node)),
        "import_declaration" => Some(lower_import(lo, node)),
        "typealias_declaration"
        | "associatedtype_declaration"
        | "operator_declaration"
        | "precedence_group_declaration"
        | "macro_declaration"
        | "line_comment"
        | "multiline_comment" => None,
        _ => lower_stmt(lo, node),
    }
}

fn lower_import(lo: &mut Lowering, node: TsNode) -> NodeId {
    let span = lo.span(node);
    let module = Lowering::named_children(node)
        .into_iter()
        .filter(|child| matches!(child.kind(), "identifier" | "simple_identifier"))
        .map(|child| lo.text(child))
        .collect::<Vec<_>>()
        .join(".");
    if module.is_empty() {
        crate::lower::import_tokens(lo, node)
    } else {
        crate::lower::import_namespace(lo, span, &module, &module)
    }
}

fn lower_type(lo: &mut Lowering, node: TsNode) -> NodeId {
    let span = lo.span(node);
    let name = node.child_by_field_name("name").map(|n| lo.sym(lo.text(n)));
    let body = node.child_by_field_name("body");
    let mut kids = Vec::new();
    if let Some(body) = body {
        for child in Lowering::named_children(body) {
            if let Some(id) = lower_item(lo, child) {
                kids.push(id);
            }
        }
    }
    let block = lo.add(NodeKind::Block, Payload::None, span, &kids);
    lo.push_unit(block, UnitKind::Class, name);
    block
}

fn lower_extension(lo: &mut Lowering, node: TsNode) -> NodeId {
    let span = lo.span(node);
    let mut kids = Vec::new();
    for child in Lowering::named_children(node) {
        match child.kind() {
            "class_body" | "enum_class_body" => {
                for item in Lowering::named_children(child) {
                    if let Some(id) = lower_item(lo, item) {
                        kids.push(id);
                    }
                }
            }
            _ => {}
        }
    }
    let block = lo.add(NodeKind::Block, Payload::None, span, &kids);
    lo.push_unit(block, UnitKind::Class, None);
    block
}

fn lower_function(lo: &mut Lowering, node: TsNode, method: bool) -> NodeId {
    let span = lo.span(node);
    let name = swift_decl_name(lo, node);
    let mut kids = Vec::new();
    for param in Lowering::named_children(node)
        .into_iter()
        .filter(|child| child.kind() == "parameter")
    {
        lower_param(lo, param, &mut kids);
    }
    let body = node
        .child_by_field_name("body")
        .map(|body| lower_function_body(lo, body))
        .unwrap_or_else(|| lo.empty_block(span));
    kids.push(body);
    let func = lo.add(NodeKind::Func, Payload::None, span, &kids);
    lo.push_unit(
        func,
        if method {
            UnitKind::Method
        } else {
            UnitKind::Function
        },
        name,
    );
    func
}

fn swift_decl_name(lo: &mut Lowering, node: TsNode) -> Option<Symbol> {
    node.child_by_field_name("name")
        .or_else(|| {
            Lowering::named_children(node)
                .into_iter()
                .find(|child| matches!(child.kind(), "simple_identifier" | "identifier"))
        })
        .map(|name| lo.sym(lo.text(name)))
}

fn lower_param(lo: &mut Lowering, param: TsNode, out: &mut Vec<NodeId>) {
    let span = lo.span(param);
    let name = parameter_binding_name(param);
    let payload = name
        .filter(|n| lo.text(*n) != "_")
        .map(|n| Payload::Name(lo.sym(lo.text(n))))
        .unwrap_or(Payload::None);
    if let Some(domain) = param
        .child_by_field_name("type")
        .and_then(|ty| lo.type_domain_from_text_with_dependencies(lo.text(ty)))
        .or_else(|| lo.type_domain_from_text_with_dependencies(lo.text(param)))
    {
        lo.record_param_domain_resolution(span, domain);
    }
    out.push(lo.add(NodeKind::Param, payload, span, &[]));
}

fn parameter_binding_name(param: TsNode) -> Option<TsNode> {
    let mut cursor = param.walk();
    let named: Vec<TsNode> = param
        .children_by_field_name("name", &mut cursor)
        .filter(|child| matches!(child.kind(), "simple_identifier" | "self_expression"))
        .collect();
    named.last().copied().or_else(|| {
        Lowering::named_children(param)
            .into_iter()
            .rfind(|child| matches!(child.kind(), "simple_identifier" | "self_expression"))
    })
}

fn lower_function_body(lo: &mut Lowering, node: TsNode) -> NodeId {
    let span = lo.span(node);
    let statements = Lowering::named_children(node)
        .into_iter()
        .find(|child| child.kind() == "statements")
        .unwrap_or(node);
    let children = Lowering::named_children(statements);
    let last_index = children.len().saturating_sub(1);
    let mut stmts = Vec::new();
    for (idx, child) in children.into_iter().enumerate() {
        if idx == last_index && is_tail_expr(child.kind()) {
            let expr = lower_expr(lo, child);
            stmts.push(lo.add(NodeKind::Return, Payload::None, lo.span(child), &[expr]));
        } else if let Some(id) = lower_stmt(lo, child) {
            stmts.push(id);
        }
    }
    lo.add(NodeKind::Block, Payload::None, span, &stmts)
}

fn is_tail_expr(kind: &str) -> bool {
    is_expr_kind(kind)
        && !matches!(
            kind,
            "assignment"
                | "if_statement"
                | "switch_statement"
                | "for_statement"
                | "while_statement"
        )
}

fn lower_block(lo: &mut Lowering, node: TsNode) -> NodeId {
    let block = Lowering::named_children(node)
        .into_iter()
        .find(|child| child.kind() == "statements")
        .unwrap_or(node);
    crate::lower::collect_into(lo, block, NodeKind::Block, lower_stmt)
}

fn lower_stmt(lo: &mut Lowering, node: TsNode) -> Option<NodeId> {
    let span = lo.span(node);
    match node.kind() {
        "statements" | "function_body" => Some(lower_block(lo, node)),
        "function_declaration" => Some(lower_function(lo, node, false)),
        "protocol_function_declaration" => Some(lower_function(lo, node, true)),
        "class_declaration"
        | "struct_declaration"
        | "enum_declaration"
        | "protocol_declaration" => Some(lower_type(lo, node)),
        "extension_declaration" => Some(lower_extension(lo, node)),
        "property_declaration"
        | "protocol_property_declaration"
        | "protocol_property_requirements" => Some(lower_property(lo, node)),
        "assignment" => Some(lower_assignment(lo, node)),
        "control_transfer_statement" => lower_control_transfer(lo, node),
        "if_statement" | "guard_statement" => Some(lower_if(lo, node)),
        "switch_statement" => Some(lower_switch(lo, node)),
        "for_statement" => Some(lower_for(lo, node)),
        "while_statement" => Some(lower_while(lo, node)),
        "repeat_while_statement" => Some(lower_repeat_while(lo, node)),
        "do_statement" => Some(lower_do(lo, node)),
        "discard_statement" => None,
        "line_comment" | "multiline_comment" => None,
        k if is_expr_kind(k) => {
            let expr = lower_expr(lo, node);
            Some(lo.add(NodeKind::ExprStmt, Payload::None, span, &[expr]))
        }
        _ => {
            let kids: Vec<NodeId> = Lowering::named_children(node)
                .into_iter()
                .map(|child| lower_expr(lo, child))
                .collect();
            Some(lo.raw(node.kind(), span, &kids))
        }
    }
}

fn lower_control_transfer(lo: &mut Lowering, node: TsNode) -> Option<NodeId> {
    let span = lo.span(node);
    let text = lo.text(node).trim_start();
    if text.starts_with("return") {
        let kids: Vec<NodeId> = node
            .child_by_field_name("result")
            .into_iter()
            .map(|value| lower_expr(lo, value))
            .collect();
        return Some(lo.add(NodeKind::Return, Payload::None, span, &kids));
    }
    if text.starts_with("throw") {
        let kids: Vec<NodeId> = node
            .child_by_field_name("result")
            .into_iter()
            .map(|value| lower_expr(lo, value))
            .collect();
        return Some(lo.add(NodeKind::Throw, Payload::None, span, &kids));
    }
    if text.starts_with("break") {
        return Some(lo.add(NodeKind::Break, Payload::None, span, &[]));
    }
    if text.starts_with("continue") {
        return Some(lo.add(NodeKind::Continue, Payload::None, span, &[]));
    }
    None
}

fn lower_property(lo: &mut Lowering, node: TsNode) -> NodeId {
    let span = lo.span(node);
    let mut assigns = Vec::new();
    let mut cursor = node.walk();
    let names: Vec<TsNode> = node.children_by_field_name("name", &mut cursor).collect();
    let values = field_children(node, "value");
    let types = field_children(node, "type");
    for (idx, name_node) in names.iter().enumerate() {
        if let Some(ty) = types.get(idx).or_else(|| types.first()) {
            record_property_binding_domain(lo, *name_node, *ty);
        } else {
            record_property_binding_domain_from_decl_text(lo, *name_node, node);
        }
        let lhs = binding_var(lo, *name_node, span);
        let rhs = values
            .get(idx)
            .or_else(|| values.first())
            .map(|value| lower_expr(lo, *value))
            .or_else(|| lower_computed_property(lo, node))
            .unwrap_or_else(|| lo.add(NodeKind::Lit, Payload::Lit(LitClass::Null), span, &[]));
        assigns.push(lo.add(NodeKind::Assign, Payload::None, span, &[lhs, rhs]));
    }
    if assigns.is_empty() {
        lower_computed_property(lo, node).unwrap_or_else(|| lo.empty_block(span))
    } else if assigns.len() == 1 {
        assigns[0]
    } else {
        lo.add(NodeKind::Block, Payload::None, span, &assigns)
    }
}

fn record_property_binding_domain(lo: &mut Lowering, name_node: TsNode, type_node: TsNode) {
    let Some(name) = binding_name(lo, name_node) else {
        return;
    };
    if name == "_" {
        return;
    }
    let Some(domain) = lo.type_domain_from_text_with_dependencies(lo.text(type_node)) else {
        return;
    };
    lo.record_evidence_with_pack_dependencies(
        EvidenceAnchor::binding(lo.span(name_node), stable_symbol_hash(&name)),
        EvidenceKind::Domain(domain.domain),
        domain.provenance.pack_id,
        domain.provenance.rule,
        domain.dependencies,
    );
}

fn record_property_binding_domain_from_decl_text(
    lo: &mut Lowering,
    name_node: TsNode,
    property_node: TsNode,
) {
    let Some(name) = binding_name(lo, name_node) else {
        return;
    };
    if name == "_" {
        return;
    }
    let decl = lo.text(property_node);
    let Some(name_start) = decl.find(&name) else {
        return;
    };
    let after_name = &decl[name_start + name.len()..];
    let Some((_, after_colon)) = after_name.split_once(':') else {
        return;
    };
    let ty = after_colon
        .split(['=', ','])
        .next()
        .unwrap_or(after_colon)
        .trim();
    if ty.is_empty() {
        return;
    }
    let annotated = format!("{name}: {ty}");
    let Some(domain) = lo.type_domain_from_text_with_dependencies(&annotated) else {
        return;
    };
    lo.record_evidence_with_pack_dependencies(
        EvidenceAnchor::binding(lo.span(name_node), stable_symbol_hash(&name)),
        EvidenceKind::Domain(domain.domain),
        domain.provenance.pack_id,
        domain.provenance.rule,
        domain.dependencies,
    );
}

fn lower_computed_property(lo: &mut Lowering, node: TsNode) -> Option<NodeId> {
    node.child_by_field_name("computed_value")
        .or_else(|| {
            Lowering::named_children(node)
                .into_iter()
                .find(|child| child.kind() == "computed_property")
        })
        .map(|computed| lower_block(lo, computed))
}

fn lower_assignment(lo: &mut Lowering, node: TsNode) -> NodeId {
    let span = lo.span(node);
    let lhs_node = node.child_by_field_name("target");
    let rhs_node = node.child_by_field_name("result");
    let op = node
        .child_by_field_name("operator")
        .map(|op| lo.text(op).trim().to_string())
        .unwrap_or_else(|| "=".to_string());
    let lhs = lhs_node
        .map(|target| lower_store_target(lo, target))
        .unwrap_or_else(|| lo.empty_block(span));
    let rhs = rhs_node
        .map(|value| lower_expr(lo, value))
        .unwrap_or_else(|| lo.empty_block(span));
    if op == "=" {
        return lo.add(NodeKind::Assign, Payload::None, span, &[lhs, rhs]);
    }
    let read_lhs = lhs_node
        .map(|target| lower_expr(lo, target))
        .unwrap_or_else(|| lo.empty_block(span));
    let value = op
        .strip_suffix('=')
        .and_then(common_bin_op)
        .map(|op| lo.add(NodeKind::BinOp, Payload::Op(op), span, &[read_lhs, rhs]))
        .unwrap_or_else(|| lo.raw(&format!("assignment {op}"), span, &[read_lhs, rhs]));
    lo.add(NodeKind::Assign, Payload::None, span, &[lhs, value])
}

fn lower_if(lo: &mut Lowering, node: TsNode) -> NodeId {
    let span = lo.span(node);
    let cond = node
        .child_by_field_name("condition")
        .map(|condition| lower_condition(lo, condition))
        .unwrap_or_else(|| lo.empty_block(span));
    let then = first_statements_child(node)
        .map(|body| lower_block(lo, body))
        .unwrap_or_else(|| lo.empty_block(span));
    let mut kids = vec![cond, then];
    if let Some(else_node) = Lowering::named_children(node)
        .into_iter()
        .find(|child| child.kind() == "else")
    {
        let alt = Lowering::named_children(else_node)
            .into_iter()
            .find(|child| !matches!(child.kind(), "line_comment" | "multiline_comment"));
        if let Some(alt) = alt {
            let lowered = if alt.kind() == "if_statement" {
                lower_if(lo, alt)
            } else {
                lower_block(lo, alt)
            };
            kids.push(lowered);
        }
    }
    lo.add(NodeKind::If, Payload::None, span, &kids)
}

fn lower_condition(lo: &mut Lowering, node: TsNode) -> NodeId {
    match node.kind() {
        "condition" | "pattern" => {
            let exprs: Vec<NodeId> = Lowering::named_children(node)
                .into_iter()
                .filter(|child| !is_type_level(child.kind()))
                .map(|child| lower_expr(lo, child))
                .collect();
            fold_and(lo, lo.span(node), exprs)
        }
        _ => lower_expr(lo, node),
    }
}

fn fold_and(lo: &mut Lowering, span: Span, mut values: Vec<NodeId>) -> NodeId {
    if values.is_empty() {
        return lo.empty_block(span);
    }
    let mut acc = values.remove(0);
    for value in values {
        acc = lo.add(NodeKind::BinOp, Payload::Op(Op::And), span, &[acc, value]);
    }
    acc
}

fn lower_while(lo: &mut Lowering, node: TsNode) -> NodeId {
    let span = lo.span(node);
    let cond = node
        .child_by_field_name("condition")
        .map(|condition| lower_condition(lo, condition))
        .unwrap_or_else(|| lo.empty_block(span));
    let body = first_statements_child(node)
        .map(|body| lower_block(lo, body))
        .unwrap_or_else(|| lo.empty_block(span));
    lo.add(
        NodeKind::Loop,
        Payload::Loop(LoopKind::While),
        span,
        &[cond, body],
    )
}

fn lower_repeat_while(lo: &mut Lowering, node: TsNode) -> NodeId {
    let span = lo.span(node);
    let cond = node
        .child_by_field_name("condition")
        .map(|condition| lower_condition(lo, condition))
        .unwrap_or_else(|| lo.empty_block(span));
    let body = first_statements_child(node)
        .map(|body| lower_block(lo, body))
        .unwrap_or_else(|| lo.empty_block(span));
    lo.add(
        NodeKind::Loop,
        Payload::Loop(LoopKind::While),
        span,
        &[cond, body],
    )
}

fn lower_for(lo: &mut Lowering, node: TsNode) -> NodeId {
    let span = lo.span(node);
    let pattern = node
        .child_by_field_name("item")
        .map(|item| binding_var(lo, item, lo.span(item)))
        .unwrap_or_else(|| lo.empty_block(span));
    let iterable = node
        .child_by_field_name("collection")
        .map(|collection| lower_expr(lo, collection))
        .unwrap_or_else(|| lo.empty_block(span));
    let body = first_statements_child(node)
        .map(|body| lower_block(lo, body))
        .unwrap_or_else(|| lo.empty_block(span));
    lo.add(
        NodeKind::Loop,
        Payload::Loop(LoopKind::ForEach),
        span,
        &[pattern, iterable, body],
    )
}

fn lower_do(lo: &mut Lowering, node: TsNode) -> NodeId {
    let span = lo.span(node);
    let body = first_statements_child(node)
        .map(|body| lower_block(lo, body))
        .unwrap_or_else(|| lo.empty_block(span));
    let mut kids = vec![body];
    for catch in Lowering::named_children(node)
        .into_iter()
        .filter(|child| child.kind() == "catch_block")
    {
        kids.push(lower_block(lo, catch));
    }
    lo.add(NodeKind::Try, Payload::None, span, &kids)
}

fn lower_switch(lo: &mut Lowering, node: TsNode) -> NodeId {
    let span = lo.span(node);
    let scrutinee = node
        .child_by_field_name("expr")
        .map(|expr| lower_expr(lo, expr));
    let mut arms = Vec::new();
    for entry in Lowering::named_children(node)
        .into_iter()
        .filter(|child| child.kind() == "switch_entry")
    {
        let (test, body) = lower_switch_entry(lo, entry, scrutinee, span);
        arms.push((test, body));
    }
    let mut acc: Option<NodeId> = None;
    for (test, body) in arms.into_iter().rev() {
        match test {
            None => acc = Some(body),
            Some(test) => {
                let mut kids = vec![test, body];
                if let Some(else_node) = acc {
                    kids.push(else_node);
                }
                acc = Some(lo.add(NodeKind::If, Payload::None, span, &kids));
            }
        }
    }
    acc.unwrap_or_else(|| lo.empty_block(span))
}

fn lower_switch_entry(
    lo: &mut Lowering,
    entry: TsNode,
    scrutinee: Option<NodeId>,
    switch_span: Span,
) -> (Option<NodeId>, NodeId) {
    let span = lo.span(entry);
    let text = lo.text(entry).trim_start();
    let mut labels = Vec::new();
    let mut stmts = Vec::new();
    for child in Lowering::named_children(entry) {
        match child.kind() {
            "switch_pattern" | "pattern" if !text.starts_with("default") => {
                labels.push(lower_switch_label(lo, child, scrutinee, span));
            }
            "statements" => {
                for stmt in Lowering::named_children(child) {
                    if let Some(id) = lower_stmt(lo, stmt) {
                        stmts.push(id);
                    }
                }
            }
            _ if is_expr_kind(child.kind()) && !text.starts_with("default") => {
                labels.push(lower_switch_label(lo, child, scrutinee, span));
            }
            _ => {}
        }
    }
    let body = lo.add(NodeKind::Block, Payload::None, span, &stmts);
    if text.starts_with("default") {
        return (None, body);
    }
    let test = if labels.is_empty() {
        Some(lo.raw("switch_case", switch_span, &[]))
    } else {
        fold_or(lo, span, labels)
    };
    (test, body)
}

fn lower_switch_label(
    lo: &mut Lowering,
    label: TsNode,
    scrutinee: Option<NodeId>,
    span: Span,
) -> NodeId {
    let value = match label.kind() {
        "switch_pattern" | "pattern" => Lowering::named_children(label)
            .into_iter()
            .find(|child| is_expr_kind(child.kind()))
            .map(|child| lower_expr(lo, child))
            .unwrap_or_else(|| lo.raw("switch_pattern", span, &[])),
        _ => lower_expr(lo, label),
    };
    match scrutinee {
        Some(subject) => lo.add(
            NodeKind::BinOp,
            Payload::Op(Op::Eq),
            span,
            &[subject, value],
        ),
        None => value,
    }
}

fn fold_or(lo: &mut Lowering, span: Span, mut values: Vec<NodeId>) -> Option<NodeId> {
    if values.is_empty() {
        return None;
    }
    let mut acc = values.remove(0);
    for value in values {
        acc = lo.add(NodeKind::BinOp, Payload::Op(Op::Or), span, &[acc, value]);
    }
    Some(acc)
}

fn lower_expr(lo: &mut Lowering, node: TsNode) -> NodeId {
    let span = lo.span(node);
    match node.kind() {
        "identifier" | "simple_identifier" | "type_identifier" => match lo.text(node) {
            "true" => lo.add(NodeKind::Lit, Payload::LitBool(true), span, &[]),
            "false" => lo.add(NodeKind::Lit, Payload::LitBool(false), span, &[]),
            other => lo.var(other, span),
        },
        "self_expression" => lo.var("self", span),
        "super_expression" => lo.var("super", span),
        "integer_literal" | "hex_literal" | "oct_literal" | "bin_literal" => {
            lo.int_lit(lo.text(node), span)
        }
        "real_literal" => lo.float_lit(lo.text(node), span),
        "line_string_literal"
        | "multi_line_string_literal"
        | "raw_string_literal"
        | "regex_literal" => lower_string(lo, node),
        "boolean_literal" => {
            let text = lo.text(node);
            lo.add(NodeKind::Lit, Payload::LitBool(text == "true"), span, &[])
        }
        "nil" => lo.add(NodeKind::Lit, Payload::Lit(LitClass::Null), span, &[]),
        "array_literal" => lower_seq(lo, node, "array"),
        "dictionary_literal" => lower_dictionary(lo, node),
        "tuple_expression" => lower_tuple(lo, node),
        "assignment" => lower_assignment(lo, node),
        "directly_assignable_expression" => peel_value_child(lo, node),
        "additive_expression"
        | "multiplicative_expression"
        | "comparison_expression"
        | "equality_expression"
        | "conjunction_expression"
        | "disjunction_expression"
        | "nil_coalescing_expression"
        | "infix_expression"
        | "range_expression"
        | "open_start_range_expression"
        | "open_end_range_expression"
        | "fully_open_range"
        | "bitwise_operation" => lower_binary_like(lo, node),
        "prefix_expression" => lower_prefix(lo, node),
        "postfix_expression" => lower_postfix(lo, node),
        "ternary_expression" => lower_ternary(lo, node),
        "if_statement" | "guard_statement" => lower_if(lo, node),
        "switch_statement" => lower_switch(lo, node),
        "call_expression" | "constructor_expression" => lower_call(lo, node),
        "navigation_expression" | "selector_expression" => lower_navigation(lo, node),
        "lambda_literal" => lower_lambda(lo, node),
        "as_expression" | "check_expression" | "consume_expression" | "value_pack_expansion" => {
            peel_value_child(lo, node)
        }
        "try_expression" => {
            let value = first_expr_child(node)
                .map(|child| lower_expr(lo, child))
                .unwrap_or_else(|| lo.empty_block(span));
            lo.protocol_boundary(span, SourceProtocolKind::TryPropagation, "try", &[value])
        }
        "await_expression" => {
            let value = first_expr_child(node)
                .map(|child| lower_expr(lo, child))
                .unwrap_or_else(|| lo.empty_block(span));
            lo.await_boundary(span, value)
        }
        "control_transfer_statement" => lower_control_transfer(lo, node)
            .unwrap_or_else(|| lo.raw("control_transfer_statement", span, &[])),
        k if is_type_level(k) => lo.empty_block(span),
        _ => {
            let kids: Vec<NodeId> = Lowering::named_children(node)
                .into_iter()
                .map(|child| lower_expr(lo, child))
                .collect();
            lo.raw(node.kind(), span, &kids)
        }
    }
}

fn lower_string(lo: &mut Lowering, node: TsNode) -> NodeId {
    let span = lo.span(node);
    let interpolations: Vec<NodeId> = field_children(node, "interpolation")
        .into_iter()
        .map(|interp| {
            let kids: Vec<NodeId> = Lowering::named_children(interp)
                .into_iter()
                .filter(|child| is_expr_kind(child.kind()))
                .map(|child| lower_expr(lo, child))
                .collect();
            if kids.len() == 1 {
                kids[0]
            } else {
                lo.raw("string_interpolation", lo.span(interp), &kids)
            }
        })
        .collect();
    if interpolations.is_empty() {
        lo.str_lit(lo.text(node), span)
    } else {
        let mut kids = Vec::with_capacity(interpolations.len() + 1);
        kids.push(lo.str_lit(lo.text(node), span));
        kids.extend(interpolations);
        lo.add(
            NodeKind::Seq,
            Payload::Name(lo.sym("interpolated_string")),
            span,
            &kids,
        )
    }
}

fn lower_seq(lo: &mut Lowering, node: TsNode, tag: &str) -> NodeId {
    let span = lo.span(node);
    let kids: Vec<NodeId> = field_children(node, "element")
        .into_iter()
        .flat_map(|child| expr_list_children(child).into_iter())
        .map(|child| lower_expr(lo, child))
        .collect();
    lo.add(NodeKind::Seq, Payload::Name(lo.sym(tag)), span, &kids)
}

fn lower_dictionary(lo: &mut Lowering, node: TsNode) -> NodeId {
    let span = lo.span(node);
    let keys = field_children(node, "key");
    let values = field_children(node, "value");
    let mut entries = Vec::new();
    for (key, value) in keys.iter().zip(values.iter()) {
        let k = lower_expr(lo, *key);
        let v = lower_expr(lo, *value);
        entries.push(lo.add(NodeKind::Seq, Payload::Name(lo.sym("pair")), span, &[k, v]));
    }
    lo.add(NodeKind::Seq, Payload::Name(lo.sym("map")), span, &entries)
}

fn lower_tuple(lo: &mut Lowering, node: TsNode) -> NodeId {
    let span = lo.span(node);
    let kids: Vec<NodeId> = Lowering::named_children(node)
        .into_iter()
        .filter(|child| is_expr_kind(child.kind()) || child.kind() == "value_argument")
        .map(|child| {
            if child.kind() == "value_argument" {
                lower_value_argument(lo, child)
            } else {
                lower_expr(lo, child)
            }
        })
        .collect();
    if kids.len() == 1 {
        return kids[0];
    }
    lo.add(NodeKind::Seq, Payload::Name(lo.sym("tuple")), span, &kids)
}

fn lower_binary_like(lo: &mut Lowering, node: TsNode) -> NodeId {
    let span = lo.span(node);
    let lhs = node
        .child_by_field_name("lhs")
        .or_else(|| node.child_by_field_name("left"))
        .or_else(|| node.child_by_field_name("value"));
    let rhs = node
        .child_by_field_name("rhs")
        .or_else(|| node.child_by_field_name("right"))
        .or_else(|| node.child_by_field_name("if_nil"));
    let op_text = node
        .child_by_field_name("op")
        .or_else(|| node.child_by_field_name("operator"))
        .map(|op| lo.text(op));
    if node.kind() == "nil_coalescing_expression" {
        let left = lhs
            .map(|lhs| lower_expr(lo, lhs))
            .unwrap_or_else(|| lo.empty_block(span));
        let right = rhs
            .map(|rhs| lower_expr(lo, rhs))
            .unwrap_or_else(|| lo.empty_block(span));
        return lo.add(
            NodeKind::Call,
            Payload::Builtin(Builtin::ValueOrDefault),
            span,
            &[left, right],
        );
    }
    if let (Some(lhs), Some(rhs), Some(op_text)) = (lhs, rhs, op_text) {
        let left = lower_expr(lo, lhs);
        let right = lower_expr(lo, rhs);
        if op_text == "??" {
            return lo.add(
                NodeKind::Call,
                Payload::Builtin(Builtin::ValueOrDefault),
                span,
                &[left, right],
            );
        }
        if let Some(rewritten) = lower_misnested_swift_boolean_rhs(lo, span, op_text, left, right) {
            return rewritten;
        }
        if let Some(op) = swift_bin_op(op_text) {
            return lo.add(NodeKind::BinOp, Payload::Op(op), span, &[left, right]);
        }
        return lo.raw(&format!("{} {op_text}", node.kind()), span, &[left, right]);
    }
    let kids: Vec<NodeId> = Lowering::named_children(node)
        .into_iter()
        .filter(|child| is_expr_kind(child.kind()))
        .map(|child| lower_expr(lo, child))
        .collect();
    if kids.len() == 1 {
        kids[0]
    } else if let Some(op_text) = op_text {
        lo.raw(&format!("{} {op_text}", node.kind()), span, &kids)
    } else {
        lo.raw(node.kind(), span, &kids)
    }
}

fn lower_misnested_swift_boolean_rhs(
    lo: &mut Lowering,
    span: Span,
    op_text: &str,
    left: NodeId,
    right: NodeId,
) -> Option<NodeId> {
    let cmp_op = swift_bin_op(op_text)?;
    if !matches!(cmp_op, Op::Lt | Op::Le | Op::Gt | Op::Ge | Op::Eq | Op::Ne) {
        return None;
    }
    if lo.b.kind(right) != NodeKind::BinOp {
        return None;
    }
    let Payload::Op(bool_op @ (Op::And | Op::Or)) = lo.b.payload(right) else {
        return None;
    };
    let rhs_children = lo.b.children(right).to_vec();
    let [rhs_left, rhs_right] = rhs_children.as_slice() else {
        return None;
    };
    let fixed_left = lo.add(
        NodeKind::BinOp,
        Payload::Op(cmp_op),
        span,
        &[left, *rhs_left],
    );
    Some(lo.add(
        NodeKind::BinOp,
        Payload::Op(bool_op),
        span,
        &[fixed_left, *rhs_right],
    ))
}

fn swift_bin_op(text: &str) -> Option<Op> {
    match text {
        "&&" => Some(Op::And),
        "||" => Some(Op::Or),
        "..<" | "..." => None,
        other => common_bin_op(other),
    }
}

fn lower_prefix(lo: &mut Lowering, node: TsNode) -> NodeId {
    let span = lo.span(node);
    let op_text = node
        .child_by_field_name("operation")
        .map(|op| lo.text(op))
        .unwrap_or_else(|| lo.text(node).trim_start());
    let operand = node
        .child_by_field_name("target")
        .or_else(|| first_expr_child(node))
        .map(|child| lower_expr(lo, child))
        .unwrap_or_else(|| lo.empty_block(span));
    if op_text.starts_with('!') {
        lo.add(NodeKind::UnOp, Payload::Op(Op::Not), span, &[operand])
    } else if op_text.starts_with('-') {
        lo.add(NodeKind::UnOp, Payload::Op(Op::Neg), span, &[operand])
    } else if op_text.starts_with('+') || op_text == "&" {
        operand
    } else if op_text.starts_with('.') {
        lower_implicit_member(lo, span, operand)
    } else {
        lo.raw("prefix_expression", span, &[operand])
    }
}

fn lower_implicit_member(lo: &mut Lowering, span: Span, member: NodeId) -> NodeId {
    if lo.b.kind(member) == NodeKind::Var {
        if let Payload::Name(field) = lo.b.payload(member) {
            let base = lo.var("swift_implicit_member", span);
            return lo.add(NodeKind::Field, Payload::Name(field), span, &[base]);
        }
    }
    lo.add(
        NodeKind::Seq,
        Payload::Name(lo.sym("swift_implicit_member")),
        span,
        &[member],
    )
}

fn lower_postfix(lo: &mut Lowering, node: TsNode) -> NodeId {
    let span = lo.span(node);
    let op_text = node
        .child_by_field_name("operation")
        .map(|op| lo.text(op))
        .unwrap_or_else(|| lo.text(node).trim_end());
    let operand_node = node
        .child_by_field_name("target")
        .or_else(|| first_expr_child(node));
    let operand = operand_node
        .map(|child| lower_expr(lo, child))
        .unwrap_or_else(|| lo.empty_block(span));
    if op_text.ends_with('?') || op_text.ends_with('!') {
        operand
    } else {
        lo.raw("postfix_expression", span, &[operand])
    }
}

fn lower_ternary(lo: &mut Lowering, node: TsNode) -> NodeId {
    let span = lo.span(node);
    let kids: Vec<NodeId> = Lowering::named_children(node)
        .into_iter()
        .filter(|child| is_expr_kind(child.kind()))
        .map(|child| lower_expr(lo, child))
        .collect();
    match kids.as_slice() {
        [cond, yes, no] => {
            let then = lo.add(NodeKind::Block, Payload::None, span, &[*yes]);
            let els = lo.add(NodeKind::Block, Payload::None, span, &[*no]);
            lo.add(NodeKind::If, Payload::None, span, &[*cond, then, els])
        }
        _ => lo.raw("ternary_expression", span, &kids),
    }
}

fn lower_call(lo: &mut Lowering, node: TsNode) -> NodeId {
    let span = lo.span(node);
    let callee = lower_callee(lo, node).unwrap_or_else(|| lo.empty_block(span));
    let mut kids = vec![callee];
    for suffix in Lowering::named_children(node)
        .into_iter()
        .filter(|child| matches!(child.kind(), "call_suffix" | "constructor_suffix"))
    {
        for child in Lowering::named_children(suffix) {
            match child.kind() {
                "value_arguments" => {
                    let mut args = Vec::new();
                    for arg in Lowering::named_children(child) {
                        if arg.kind() == "value_argument" {
                            args.push(lower_value_argument(lo, arg));
                        }
                    }
                    if lo.text(child).trim_start().starts_with('[') {
                        let index = match args.as_slice() {
                            [] => lo.empty_block(lo.span(child)),
                            [only] => *only,
                            [key, default] if kwarg_name(lo, *default) == Some("default") => {
                                let default_value =
                                    lo.b.children(*default).first().copied().unwrap_or(*default);
                                lo.add(
                                    NodeKind::Seq,
                                    Payload::Name(lo.sym("swift_subscript_default")),
                                    lo.span(child),
                                    &[*key, default_value],
                                )
                            }
                            _ => lo.add(
                                NodeKind::Seq,
                                Payload::Name(lo.sym("tuple")),
                                lo.span(child),
                                &args,
                            ),
                        };
                        return lo.add(NodeKind::Index, Payload::None, span, &[kids[0], index]);
                    }
                    kids.extend(args);
                }
                "lambda_literal" => kids.push(lower_lambda(lo, child)),
                _ => {}
            }
        }
    }
    lo.add(NodeKind::Call, Payload::None, span, &kids)
}

fn kwarg_name<'a>(lo: &'a Lowering, node: NodeId) -> Option<&'a str> {
    if lo.b.kind(node) != NodeKind::KwArg {
        return None;
    }
    let Payload::Name(name) = lo.b.payload(node) else {
        return None;
    };
    Some(lo.interner.resolve(name))
}

fn lower_callee(lo: &mut Lowering, node: TsNode) -> Option<NodeId> {
    if node.kind() == "constructor_expression" {
        let ty = node.child_by_field_name("constructed_type")?;
        let name = type_surface_name(lo, ty).unwrap_or_else(|| lo.text(ty).to_string());
        return Some(lo.var(&name, lo.span(ty)));
    }
    Lowering::named_children(node)
        .into_iter()
        .find(|child| child.kind() != "call_suffix")
        .map(|child| lower_expr(lo, child))
}

fn lower_value_argument(lo: &mut Lowering, node: TsNode) -> NodeId {
    let span = lo.span(node);
    let value = node
        .child_by_field_name("value")
        .or_else(|| first_expr_child(node))
        .map(|value| lower_expr(lo, value))
        .unwrap_or_else(|| lo.empty_block(span));
    if let Some(name) = node.child_by_field_name("name") {
        lo.add(
            NodeKind::KwArg,
            Payload::Name(lo.sym(lo.text(name).trim_end_matches(':'))),
            span,
            &[value],
        )
    } else {
        value
    }
}

fn lower_navigation(lo: &mut Lowering, node: TsNode) -> NodeId {
    let span = lo.span(node);
    let Some(target) = node.child_by_field_name("target") else {
        return lo.raw(node.kind(), span, &[]);
    };
    let mut base = lower_expr(lo, target);
    let Some(suffix) = node.child_by_field_name("suffix") else {
        return base;
    };
    let suffix_value = suffix
        .child_by_field_name("suffix")
        .or_else(|| Lowering::named_children(suffix).into_iter().next());
    if let Some(value) = suffix_value {
        match value.kind() {
            "simple_identifier" | "identifier" => {
                return lo.add(
                    NodeKind::Field,
                    Payload::Name(lo.sym(lo.text(value))),
                    span,
                    &[base],
                );
            }
            "integer_literal" => {
                let index = lower_expr(lo, value);
                base = lo.add(NodeKind::Index, Payload::None, span, &[base, index]);
            }
            _ if is_expr_kind(value.kind()) => {
                let index = lower_expr(lo, value);
                base = lo.add(NodeKind::Index, Payload::None, span, &[base, index]);
            }
            _ => {}
        }
    }
    base
}

fn lower_lambda(lo: &mut Lowering, node: TsNode) -> NodeId {
    let span = lo.span(node);
    let mut kids = Vec::new();
    if let Some(lambda_type) = node.child_by_field_name("type") {
        lower_lambda_type_params(lo, lambda_type, &mut kids);
    }
    for child in Lowering::named_children(node)
        .into_iter()
        .filter(|child| child.kind() == "lambda_function_type")
    {
        lower_lambda_type_params(lo, child, &mut kids);
    }
    if kids.is_empty() {
        for name in lambda_parameter_names_from_text(lo.text(node)) {
            kids.push(lo.add(NodeKind::Param, Payload::Name(lo.sym(&name)), span, &[]));
        }
    }
    let body = first_statements_child(node)
        .map(|body| lower_function_body(lo, body))
        .unwrap_or_else(|| lo.empty_block(span));
    dedupe_lambda_params(lo, &mut kids);
    kids.push(body);
    lo.add(NodeKind::Lambda, Payload::None, span, &kids)
}

fn dedupe_lambda_params(lo: &Lowering, kids: &mut Vec<NodeId>) {
    let mut seen = Vec::new();
    kids.retain(|&kid| {
        if lo.b.kind(kid) != NodeKind::Param {
            return true;
        }
        let Payload::Name(name) = lo.b.payload(kid) else {
            return true;
        };
        if seen.contains(&name) {
            false
        } else {
            seen.push(name);
            true
        }
    });
}

fn lower_lambda_type_params(lo: &mut Lowering, node: TsNode, out: &mut Vec<NodeId>) {
    for child in Lowering::named_children(node) {
        if child.kind() == "lambda_parameter" {
            lower_param(lo, child, out);
        } else if matches!(
            child.kind(),
            "lambda_function_type" | "lambda_function_type_parameters"
        ) {
            lower_lambda_type_params(lo, child, out);
        }
    }
}

fn lambda_parameter_names_from_text(text: &str) -> Vec<String> {
    let Some(inner) = text
        .trim()
        .strip_prefix('{')
        .and_then(|text| text.strip_suffix('}'))
    else {
        return Vec::new();
    };
    let inner = inner.trim();
    if let Some((header, _body)) = inner.split_once(" in ") {
        return header
            .trim()
            .trim_start_matches('(')
            .trim_end_matches(')')
            .split(',')
            .filter_map(lambda_parameter_name_from_header_part)
            .collect();
    }
    if inner.contains("$0") {
        return vec!["$0".to_string()];
    }
    Vec::new()
}

fn lambda_parameter_name_from_header_part(part: &str) -> Option<String> {
    let before_type = part.trim().split(':').next()?.trim();
    let name = before_type
        .split_whitespace()
        .last()
        .unwrap_or(before_type)
        .trim();
    if name.is_empty() || name == "_" {
        None
    } else {
        Some(name.to_string())
    }
}

fn lower_store_target(lo: &mut Lowering, node: TsNode) -> NodeId {
    lower_expr(lo, node)
}

fn binding_var(lo: &mut Lowering, node: TsNode, fallback_span: Span) -> NodeId {
    if let Some(name) = binding_name(lo, node) {
        lo.var(&name, lo.span(node))
    } else {
        lo.empty_block(fallback_span)
    }
}

fn binding_name(lo: &Lowering, node: TsNode) -> Option<String> {
    match node.kind() {
        "simple_identifier" | "identifier" => Some(lo.text(node).to_string()),
        "self_expression" => Some("self".to_string()),
        "pattern" => node
            .child_by_field_name("bound_identifier")
            .or_else(|| {
                Lowering::named_children(node)
                    .into_iter()
                    .find(|child| matches!(child.kind(), "simple_identifier" | "identifier"))
            })
            .map(|child| lo.text(child).to_string()),
        "value_binding_pattern" => Lowering::named_children(node)
            .into_iter()
            .find_map(|child| binding_name(lo, child)),
        _ => Lowering::named_children(node)
            .into_iter()
            .find_map(|child| binding_name(lo, child)),
    }
}

fn peel_value_child(lo: &mut Lowering, node: TsNode) -> NodeId {
    first_expr_child(node)
        .map(|child| lower_expr(lo, child))
        .unwrap_or_else(|| lo.empty_block(lo.span(node)))
}

fn first_expr_child<'a>(node: TsNode<'a>) -> Option<TsNode<'a>> {
    Lowering::named_children(node)
        .into_iter()
        .find(|child| is_expr_kind(child.kind()))
}

fn first_statements_child<'a>(node: TsNode<'a>) -> Option<TsNode<'a>> {
    Lowering::named_children(node)
        .into_iter()
        .find(|child| child.kind() == "statements" || child.kind() == "function_body")
}

fn field_children<'a>(node: TsNode<'a>, field: &str) -> Vec<TsNode<'a>> {
    let mut cursor = node.walk();
    node.children_by_field_name(field, &mut cursor).collect()
}

fn expr_list_children<'a>(node: TsNode<'a>) -> Vec<TsNode<'a>> {
    if node.kind() == "value_argument" {
        return node
            .child_by_field_name("value")
            .into_iter()
            .collect::<Vec<_>>();
    }
    vec![node]
}

fn type_surface_name(lo: &Lowering, node: TsNode) -> Option<String> {
    match node.kind() {
        "user_type" | "optional_type" | "array_type" | "dictionary_type" => {
            Lowering::named_children(node)
                .into_iter()
                .find_map(|child| type_surface_name(lo, child))
        }
        "type_identifier" | "simple_identifier" | "identifier" => Some(lo.text(node).to_string()),
        _ => None,
    }
}

fn is_expr_kind(kind: &str) -> bool {
    matches!(
        kind,
        "additive_expression"
            | "array_literal"
            | "as_expression"
            | "assignment"
            | "await_expression"
            | "bang"
            | "bin_literal"
            | "bitwise_operation"
            | "boolean_literal"
            | "call_expression"
            | "check_expression"
            | "comparison_expression"
            | "conjunction_expression"
            | "constructor_expression"
            | "consume_expression"
            | "dictionary_literal"
            | "directly_assignable_expression"
            | "disjunction_expression"
            | "equality_expression"
            | "fully_open_range"
            | "guard_statement"
            | "hex_literal"
            | "if_statement"
            | "infix_expression"
            | "integer_literal"
            | "key_path_expression"
            | "key_path_string_expression"
            | "lambda_literal"
            | "line_string_literal"
            | "multi_line_string_literal"
            | "multiplicative_expression"
            | "navigation_expression"
            | "nil_coalescing_expression"
            | "oct_literal"
            | "open_end_range_expression"
            | "open_start_range_expression"
            | "playground_literal"
            | "postfix_expression"
            | "prefix_expression"
            | "range_expression"
            | "raw_string_literal"
            | "real_literal"
            | "regex_literal"
            | "selector_expression"
            | "self_expression"
            | "simple_identifier"
            | "special_literal"
            | "super_expression"
            | "switch_statement"
            | "ternary_expression"
            | "try_expression"
            | "tuple_expression"
            | "value_argument"
            | "value_pack_expansion"
            | "value_parameter_pack"
            | "identifier"
            | "type_identifier"
            | "nil"
    )
}

fn is_type_level(kind: &str) -> bool {
    matches!(
        kind,
        "array_type"
            | "bracket_qualified_type"
            | "dictionary_type"
            | "existential_type"
            | "function_type"
            | "lambda_function_type"
            | "lambda_function_type_parameters"
            | "metatype"
            | "modifiers"
            | "opaque_type"
            | "optional_type"
            | "parameter_modifiers"
            | "protocol_composition_type"
            | "suppressed_constraint"
            | "tuple_type"
            | "type_annotation"
            | "type_arguments"
            | "type_constraints"
            | "type_identifier"
            | "type_modifiers"
            | "type_pack_expansion"
            | "type_parameter"
            | "type_parameter_pack"
            | "type_parameters"
            | "user_type"
            | "where_clause"
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    fn il_with_interner(src: &str) -> (Il, Interner) {
        let interner = Interner::default();
        let il = lower(FileId(0), "t.swift", src.as_bytes(), &interner).expect("lower swift");
        (il, interner)
    }

    fn il(src: &str) -> Il {
        il_with_interner(src).0
    }

    fn raw_names(il: &Il, interner: &Interner) -> Vec<String> {
        il.nodes
            .iter()
            .filter_map(|node| {
                if node.kind != NodeKind::Raw {
                    return None;
                }
                let Payload::Name(name) = node.payload else {
                    return None;
                };
                Some(interner.resolve(name).to_string())
            })
            .collect()
    }

    #[test]
    fn function_lowers_to_unit() {
        let il = il(r#"
func add(_ x: Int, _ y: Int) -> Int {
    return x + y
}
"#);
        assert_eq!(il.units.len(), 1);
        assert_eq!(il.meta.lang, Lang::Swift);
    }

    #[test]
    fn foreach_lowers_to_loop() {
        let il = il(r#"
func sumPositive(_ xs: [Int]) -> Int {
    var total = 0
    for x in xs {
        if x > 0 {
            total += x
        }
    }
    return total
}
"#);
        assert!(il.nodes.iter().any(|node| {
            node.kind == NodeKind::Loop && node.payload == Payload::Loop(LoopKind::ForEach)
        }));
    }

    #[test]
    fn subscript_lowers_to_index() {
        let il = il(r#"
func get(_ xs: [Int], _ i: Int) -> Int {
    return xs[i]
}
"#);
        assert!(il.nodes.iter().any(|node| node.kind == NodeKind::Index));
    }

    #[test]
    fn closure_header_lowers_to_lambda_param() {
        let il = il(r#"
func mapped(_ xs: [Int]) -> [Int] {
    return xs.map { x in x + 1 }
}
"#);
        let lambda = il
            .nodes
            .iter()
            .position(|node| node.kind == NodeKind::Lambda)
            .map(|idx| NodeId(idx as u32))
            .expect("lambda");
        let first = il.children(lambda).first().copied().expect("lambda child");
        assert_eq!(il.kind(first), NodeKind::Param);
    }

    #[test]
    fn closure_type_header_dedupes_lambda_params() {
        let il = il(r#"
func mapped(_ xs: [Int]) -> [Int] {
    return xs.map { (x: Int) -> Int in x + 1 }
}
"#);
        let lambda = il
            .nodes
            .iter()
            .position(|node| node.kind == NodeKind::Lambda)
            .map(|idx| NodeId(idx as u32))
            .expect("lambda");
        let params = il
            .children(lambda)
            .iter()
            .filter(|&&child| il.kind(child) == NodeKind::Param)
            .count();
        assert_eq!(params, 1);
    }

    #[test]
    fn unparenthesized_comparison_conjunction_lowers_as_boolean_and() {
        let il = il(r#"
func ordered(_ x: Int, _ y: Int) -> Bool {
    return x < y && x <= y
}
"#);
        assert!(il.nodes.iter().enumerate().any(|(idx, node)| {
            if node.kind != NodeKind::BinOp || node.payload != Payload::Op(Op::And) {
                return false;
            }
            let kids = il.children(NodeId(idx as u32));
            matches!(
                kids,
                [left, right]
                    if il.kind(*left) == NodeKind::BinOp
                        && il.node(*left).payload == Payload::Op(Op::Lt)
                        && il.kind(*right) == NodeKind::BinOp
                        && il.node(*right).payload == Payload::Op(Op::Le)
            )
        }));
    }

    #[test]
    fn dictionary_default_subscript_lowers_with_marker() {
        let (il, interner) = il_with_interner(
            r#"
func lookup(_ dict: Dictionary<String, Int>, _ key: String, _ fallback: Int) -> Int {
    return dict[key, default: fallback]
}
"#,
        );
        let marker = interner.intern("swift_subscript_default");
        assert!(il
            .nodes
            .iter()
            .any(|node| { node.kind == NodeKind::Seq && node.payload == Payload::Name(marker) }));
    }

    #[test]
    fn parameter_type_annotation_records_domain() {
        let il = il(r#"
func lookup(_ dict: Dictionary<String, Int>, _ value: Any) -> Int {
    return dict["red", default: 0]
}
"#);
        assert_eq!(
            il.evidence
                .iter()
                .filter(|record| record.kind == EvidenceKind::Domain(nose_il::DomainEvidence::Map))
                .count(),
            1,
            "only Dictionary parameters should record a Map domain"
        );
    }

    #[test]
    fn property_type_annotation_records_binding_domain() {
        let il = il(r#"
func build(_ xs: [Int]) -> [Int] {
    var out: [Int] = []
    for x in xs {
        out.append(x)
    }
    return out
}
"#);
        assert!(il.evidence.iter().any(|record| {
            matches!(
                record.anchor,
                EvidenceAnchor::Binding { local_hash, .. }
                    if local_hash == stable_symbol_hash("out")
            ) && record.kind == EvidenceKind::Domain(nose_il::DomainEvidence::Collection)
        }));
    }

    #[test]
    fn parenthesized_single_expression_does_not_become_tuple() {
        let il = il(r#"
func mapped(_ xs: [Int]) -> [Int] {
    return xs.map { x in (x + 1) * 2 }
}
"#);
        assert!(!il.nodes.iter().any(|node| {
            node.kind == NodeKind::Seq && matches!(node.payload, Payload::Name(_))
        }));
    }

    #[test]
    fn implicit_member_shorthand_lowers_without_raw_prefix() {
        let (il, interner) = il_with_interner(
            r#"
func axis() -> Any {
    return .vertical
}

func space() -> Any {
    return .named("scroll")
}
"#,
        );
        assert!(
            !raw_names(&il, &interner)
                .iter()
                .any(|name| name == "prefix_expression"),
            "implicit member syntax should not stay as a generic Raw prefix"
        );
        let implicit = interner.intern("swift_implicit_member");
        assert!(il.nodes.iter().enumerate().any(|(idx, node)| {
            if node.kind != NodeKind::Field {
                return false;
            }
            let children = il.children(NodeId(idx as u32));
            matches!(
                children,
                [receiver]
                    if il.kind(*receiver) == NodeKind::Var
                        && il.node(*receiver).payload == Payload::Name(implicit)
            )
        }));
    }

    #[test]
    fn protocol_requirements_lower_as_signature_units() {
        let (il, interner) = il_with_interner(
            r#"
protocol Store {
    var count: Int { get }
    func fetch(_ key: String) async throws -> Int
}
"#,
        );
        let raw = raw_names(&il, &interner);
        assert!(
            !raw.iter().any(|name| matches!(
                name.as_str(),
                "protocol_function_declaration"
                    | "protocol_property_declaration"
                    | "protocol_property_requirements"
            )),
            "protocol requirements should lower as declaration/signature structure, got {raw:?}"
        );
        assert!(
            il.units
                .iter()
                .any(|unit| unit.kind == UnitKind::Method
                    && unit.name == Some(interner.intern("fetch"))),
            "protocol function requirement should be a method-like unit"
        );
        assert!(il.nodes.iter().any(|node| node.kind == NodeKind::Param));
    }
}
