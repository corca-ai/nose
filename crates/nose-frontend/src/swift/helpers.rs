use super::*;

pub(super) fn lower_store_target(lo: &mut Lowering, node: TsNode) -> NodeId {
    lower_expr(lo, node)
}
pub(super) fn binding_var(lo: &mut Lowering, node: TsNode, fallback_span: Span) -> NodeId {
    if let Some(name) = binding_name(lo, node) {
        lo.var(&name, lo.span(node))
    } else {
        lo.empty_block(fallback_span)
    }
}
pub(super) fn binding_name(lo: &Lowering, node: TsNode) -> Option<String> {
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
pub(super) fn peel_value_child(lo: &mut Lowering, node: TsNode) -> NodeId {
    first_expr_child(node)
        .map(|child| lower_expr(lo, child))
        .unwrap_or_else(|| lo.empty_block(lo.span(node)))
}
pub(super) fn first_expr_child<'a>(node: TsNode<'a>) -> Option<TsNode<'a>> {
    Lowering::named_children(node)
        .into_iter()
        .find(|child| is_expr_kind(child.kind()) && child.kind() != "bang")
}
pub(super) fn first_statements_child<'a>(node: TsNode<'a>) -> Option<TsNode<'a>> {
    Lowering::named_children(node)
        .into_iter()
        .find(|child| child.kind() == "statements" || child.kind() == "function_body")
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub(super) enum SwiftForAsyncIterationKind {
    Sync,
    Plain,
    Throwing,
}

pub(super) fn swift_for_async_iteration_kind(text: &str) -> SwiftForAsyncIterationKind {
    let Some(rest) = consume_swift_keyword(text.trim_start(), "for") else {
        return SwiftForAsyncIterationKind::Sync;
    };
    let rest = rest.trim_start();
    if consume_swift_keyword(rest, "await").is_some() {
        return SwiftForAsyncIterationKind::Plain;
    }
    if let Some(after_try) = consume_swift_try_token(rest) {
        if after_try.chars().next().is_some_and(char::is_whitespace)
            && consume_swift_keyword(after_try.trim_start(), "await").is_some()
        {
            return SwiftForAsyncIterationKind::Throwing;
        }
    }
    SwiftForAsyncIterationKind::Sync
}

pub(super) fn swift_for_try_keyword_span(text: &str, span: Span) -> Span {
    let Some(start) = swift_for_try_keyword_offset(text) else {
        return span;
    };
    let end = start
        + if text[start..].starts_with("try?") || text[start..].starts_with("try!") {
            4
        } else {
            3
        };
    let start_line =
        span.start_line + text[..start].bytes().filter(|byte| *byte == b'\n').count() as u32;
    let end_line = start_line
        + text[start..end]
            .bytes()
            .filter(|byte| *byte == b'\n')
            .count() as u32;
    Span::new(
        span.file,
        span.start_byte + start as u32,
        span.start_byte + end as u32,
        start_line,
        end_line,
    )
}

fn swift_for_try_keyword_offset(text: &str) -> Option<usize> {
    let for_offset = text.find("for")?;
    let rest = consume_swift_keyword(&text[for_offset..], "for")?;
    let rest_offset = text.len() - rest.len();
    let trimmed = rest.trim_start();
    let trim_offset = rest.len() - trimmed.len();
    let try_offset = rest_offset + trim_offset;
    consume_swift_try_token(trimmed).map(|_| try_offset)
}

fn consume_swift_try_token(text: &str) -> Option<&str> {
    text.strip_prefix("try?")
        .or_else(|| text.strip_prefix("try!"))
        .or_else(|| consume_swift_keyword(text, "try"))
}

pub(super) fn consume_swift_keyword<'a>(text: &'a str, keyword: &str) -> Option<&'a str> {
    let rest = text.strip_prefix(keyword)?;
    if rest
        .chars()
        .next()
        .is_some_and(is_swift_identifier_continue)
    {
        return None;
    }
    Some(rest)
}

pub(super) fn is_swift_identifier_continue(ch: char) -> bool {
    ch == '_' || ch.is_alphanumeric()
}

pub(super) fn field_children<'a>(node: TsNode<'a>, field: &str) -> Vec<TsNode<'a>> {
    let mut cursor = node.walk();
    node.children_by_field_name(field, &mut cursor).collect()
}
pub(super) fn expr_list_children<'a>(node: TsNode<'a>) -> Vec<TsNode<'a>> {
    if node.kind() == "value_argument" {
        return node
            .child_by_field_name("value")
            .into_iter()
            .collect::<Vec<_>>();
    }
    vec![node]
}
pub(super) fn is_expr_kind(kind: &str) -> bool {
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
            | "diagnostic"
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
            | "macro_invocation"
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
    ) || kind == "custom_operator"
        || is_swift_operator_token_kind(kind)
}
pub(super) fn is_type_level(kind: &str) -> bool {
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
