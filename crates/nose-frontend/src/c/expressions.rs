use super::*;
use crate::tree_sitter_ext::last_named_child;

pub(super) fn lower_expr(lo: &mut Lowering, node: TsNode) -> NodeId {
    let span = lo.span(node);
    if let Some(lowered) = lower_expr_core(lo, node, span) {
        return lowered;
    }
    match node.kind() {
        "subscript_expression" => {
            let kids: Vec<NodeId> = Lowering::named_children(node)
                .into_iter()
                .map(|c| lower_expr(lo, c))
                .collect();
            lo.add(NodeKind::Index, Payload::None, span, &kids)
        }
        "conditional_expression" => {
            let kids: Vec<NodeId> = ["condition", "consequence", "alternative"]
                .iter()
                .filter_map(|f| node.child_by_field_name(f))
                .map(|c| lower_expr(lo, c))
                .collect();
            lo.add(NodeKind::If, Payload::None, span, &kids)
        }
        "initializer_list" => {
            let kids: Vec<NodeId> = Lowering::named_children(node)
                .into_iter()
                .map(|c| lower_expr(lo, c))
                .collect();
            lo.add(NodeKind::Seq, Payload::None, span, &kids)
        }
        "compound_literal_expression" => {
            let kids: Vec<NodeId> = Lowering::named_children(node)
                .into_iter()
                .filter(|child| !is_c_type_surface(child.kind()))
                .map(|c| lower_expr(lo, c))
                .collect();
            match kids.as_slice() {
                [only] => *only,
                _ => lo.add(
                    NodeKind::Seq,
                    Payload::Name(lo.sym("c_compound_literal")),
                    span,
                    &kids,
                ),
            }
        }
        // Designated initializer `.field = v` / `[i] = v` → the value (the designator
        // is a field/index name, not behavior).
        "initializer_pair" => node
            .child_by_field_name("value")
            .or_else(|| Lowering::named_children(node).into_iter().next_back())
            .map(|v| lower_expr(lo, v))
            .unwrap_or_else(|| lo.empty_block(span)),
        "field_designator" | "subscript_designator" => lo.var(lo.text(node), span),
        // `offsetof(T, m)` is a compile-time integer constant (like sizeof).
        "offsetof_expression" => lo.add(NodeKind::Lit, Payload::Lit(LitClass::Int), span, &[]),
        // These can appear under parser `ERROR` or macro-adjacent wrappers in large
        // real-world headers. Reuse statement/item lowering for the child structure while
        // leaving the enclosing `ERROR` Raw in place as the honest parse-boundary marker.
        "function_definition" => lower_func(lo, node),
        "declaration" => lower_decl(lo, node),
        "expression_statement"
        | "if_statement"
        | "for_statement"
        | "while_statement"
        | "do_statement"
        | "return_statement"
        | "break_statement"
        | "continue_statement"
        | "goto_statement"
        | "labeled_statement" => lower_stmt(lo, node).unwrap_or_else(|| lo.empty_block(span)),
        "preproc_if" | "preproc_ifdef" | "preproc_else" | "preproc_elif" | "preproc_elifdef" => {
            lower_preproc(lo, node)
        }
        "preproc_include" => crate::lower::import_tokens(lo, node),
        _ => lower_expr_tail(lo, node, span),
    }
}

fn lower_expr_tail(lo: &mut Lowering, node: TsNode, span: Span) -> NodeId {
    match node.kind() {
        "preproc_def"
        | "preproc_function_def"
        | "preproc_call"
        | "preproc_directive"
        | "comment"
        | "statement_identifier" => lo.empty_block(span),
        "case_statement"
        | "else_clause"
        | "init_declarator"
        | "declaration_list"
        | "gnu_asm_expression"
        | "gnu_asm_output_operand"
        | "gnu_asm_output_operand_list"
        | "gnu_asm_input_operand"
        | "gnu_asm_input_operand_list"
        | "gnu_asm_clobber_list"
        | "gnu_asm_qualifier"
        | "ms_declspec_modifier" => lower_c_opaque_seq(lo, node),
        "character" => lo.str_lit(lo.text(node), span),
        // `a, b` comma expression → a sequence of its operands.
        "comma_expression" => {
            let kids: Vec<NodeId> = Lowering::named_children(node)
                .into_iter()
                .map(|c| lower_expr(lo, c))
                .collect();
            lo.add(NodeKind::Seq, Payload::None, span, &kids)
        }
        // `NAME = value` enum constant → its value (or the name).
        "enumerator" => node
            .child_by_field_name("value")
            .map(|v| lower_expr(lo, v))
            .or_else(|| node.named_child(0).map(|n| lo.var(lo.text(n), span)))
            .unwrap_or_else(|| lo.empty_block(span)),
        // Type-level / declarator nodes reaching expression position (sizeof/casts/
        // compound literals, K&R decls, macro bodies) carry no behavior — erase.
        k if is_c_type_surface(k) => lo.empty_block(span),
        _ => {
            let kids: Vec<NodeId> = Lowering::named_children(node)
                .into_iter()
                .map(|c| lower_expr(lo, c))
                .collect();
            lo.raw(node.kind(), span, &kids)
        }
    }
}

fn lower_expr_core(lo: &mut Lowering, node: TsNode, span: Span) -> Option<NodeId> {
    let lowered = match node.kind() {
        // GCC statement-expression `({ stmt; ...; expr; })` reaches here via
        // `parenthesized_expression`; lower its body as a Block so the inner
        // statements route through `lower_stmt` instead of falling to Raw.
        "compound_statement" => lower_block(lo, node),
        // `sizeof x` / `sizeof(T)` is a compile-time integer constant; the operand is
        // often a type (which would itself be Raw), so lower to an int literal.
        "sizeof_expression" => lo.add(NodeKind::Lit, Payload::Lit(LitClass::Int), span, &[]),
        "identifier" | "field_identifier" | "type_identifier" => {
            if lo.text(node) == "NULL" {
                lo.add(NodeKind::Lit, Payload::Lit(LitClass::Null), span, &[])
            } else {
                lo.var(lo.text(node), span)
            }
        }
        "number_literal" => lower_number_literal(lo, node),
        "string_literal" | "concatenated_string" | "char_literal" => {
            let t = lo.text(node);
            lo.str_lit(t, span)
        }
        "true" => lo.add(NodeKind::Lit, Payload::LitBool(true), span, &[]),
        "false" => lo.add(NodeKind::Lit, Payload::LitBool(false), span, &[]),
        "null" => lo.add(NodeKind::Lit, Payload::Lit(LitClass::Null), span, &[]),
        "binary_expression" => lower_binary(lo, node),
        "unary_expression" => lower_unary(lo, node),
        // `*p`, `&x` pointer ops, and parentheses peel to the operand. Most casts keep
        // the historical behavior too, but explicit unsigned 32-bit casts are proof facts
        // for C byte-pack shifts such as `((u32)a[0]) << 24`.
        "cast_expression" => lower_cast(lo, node),
        "pointer_expression" | "parenthesized_expression" => node
            .child_by_field_name("argument")
            .or_else(|| node.child_by_field_name("value"))
            .or_else(|| last_named_child(node))
            .map(|c| lower_expr(lo, c))
            .unwrap_or_else(|| lo.empty_block(span)),
        "assignment_expression" => lower_assignment(lo, node),
        "update_expression" => lower_update(lo, node),
        "call_expression" => lower_call(lo, node),
        "field_expression" => lower_field_expr(lo, node),
        _ => return None,
    };
    Some(lowered)
}

pub(super) fn lower_c_opaque_seq(lo: &mut Lowering, node: TsNode) -> NodeId {
    let span = lo.span(node);
    let kids: Vec<NodeId> = Lowering::named_children(node)
        .into_iter()
        .filter(|child| !is_c_type_surface(child.kind()))
        .map(|child| lower_expr(lo, child))
        .collect();
    lo.add(
        NodeKind::Seq,
        Payload::Name(lo.sym(&format!("c_{}", node.kind()))),
        span,
        &kids,
    )
}
pub(super) fn is_c_type_surface(kind: &str) -> bool {
    matches!(
        kind,
        "primitive_type"
            | "sized_type_specifier"
            | "type_descriptor"
            | "parameter_declaration"
            | "parameter_list"
            | "abstract_pointer_declarator"
            | "function_declarator"
            | "storage_class_specifier"
            | "type_qualifier"
            | "ms_call_modifier"
            | "preproc_arg"
            | "preproc_defined"
            | "field_declaration"
            | "field_declaration_list"
            | "pointer_declarator"
            | "array_declarator"
            | "parenthesized_declarator"
            | "type_definition"
            | "struct_specifier"
            | "union_specifier"
            | "enum_specifier"
            | "enumerator_list"
            | "macro_type_specifier"
            | "preproc_params"
            | "system_lib_string"
    )
}
pub(super) fn lower_number_literal(lo: &mut Lowering, node: TsNode) -> NodeId {
    let span = lo.span(node);
    let t = lo.text(node);
    let lower = t.to_ascii_lowercase();
    // In a hex literal (`0x…`) the digits e/E are hex digits, not a float exponent;
    // a hex float instead uses a `.` or a binary `p`/`P` exponent. A decimal literal
    // is a float if it has a `.` or an `e`/`E` exponent.
    let is_float = if lower.starts_with("0x") {
        lower.contains('.') || lower.contains('p')
    } else {
        lower.contains('.') || lower.contains('e')
    };
    if is_float {
        lo.float_lit(t, span)
    } else {
        lo.int_lit(t.trim_end_matches(['u', 'U', 'l', 'L']), span)
    }
}
pub(super) fn lower_unary(lo: &mut Lowering, node: TsNode) -> NodeId {
    let span = lo.span(node);
    let operand = node
        .child_by_field_name("argument")
        .map(|o| lower_expr(lo, o))
        .unwrap_or_else(|| lo.empty_block(span));
    // Map by the operator token, not the whole node's text: `+` is `Pos`,
    // `-` is `Neg`, `!` is `Not`, `~` is `BitNot`. Reading only the leading
    // byte once collapsed `+x` and `~x` onto `Neg`.
    let op = match node.child_by_field_name("operator").map(|o| lo.text(o)) {
        Some("+") => Op::Pos,
        Some("!") => Op::Not,
        Some("~") => Op::BitNot,
        _ => Op::Neg,
    };
    lo.add(NodeKind::UnOp, Payload::Op(op), span, &[operand])
}
/// See [`crate::lower::deref_store_target`]: `(*nr)++` must keep the store place.
pub(super) fn lower_store_target(lo: &mut Lowering, node: TsNode) -> NodeId {
    crate::lower::deref_store_target(
        lo,
        node,
        |_, n| {
            (n.kind() == "pointer_expression" && crate::lower::has_direct_token(n, "*"))
                .then(|| {
                    n.child_by_field_name("argument")
                        .or_else(|| last_named_child(n))
                })
                .flatten()
        },
        lower_expr,
    )
}
pub(super) fn lower_assignment(lo: &mut Lowering, node: TsNode) -> NodeId {
    let span = lo.span(node);
    let l = node
        .child_by_field_name("left")
        .map(|x| lower_store_target(lo, x))
        .unwrap_or_else(|| lo.empty_block(span));
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
        // An unmapped compound operator keeps its own raw shape —
        // dropping the operator would merge it with `x = y`.
        let value = match common_bin_op(opt.trim_end_matches('=')) {
            Some(op) => lo.add(NodeKind::BinOp, Payload::Op(op), span, &[l2, r]),
            None => lo.raw(&format!("compound_assignment {opt}"), span, &[l2, r]),
        };
        return lo.add(NodeKind::Assign, Payload::None, span, &[l, value]);
    }
    lo.add(NodeKind::Assign, Payload::None, span, &[l, r])
}
pub(super) fn lower_update(lo: &mut Lowering, node: TsNode) -> NodeId {
    let span = lo.span(node);
    let arg = node.child_by_field_name("argument");
    let operand = arg
        .map(|o| lower_store_target(lo, o))
        .unwrap_or_else(|| lo.empty_block(span));
    let operand2 = arg
        .map(|o| lower_expr(lo, o))
        .unwrap_or_else(|| lo.empty_block(span));
    let one = lo.int_lit("1", span);
    // Decide by the operator TOKEN, scanning only this node's direct children:
    // a substring check on the whole text misreads a nested `--`/`++` in the
    // operand (e.g. `a[i--]++`, whose outer op is `++`).
    let op = if crate::lower::has_direct_token(node, "--") {
        Op::Sub
    } else {
        Op::Add
    };
    let bin = lo.add(NodeKind::BinOp, Payload::Op(op), span, &[operand2, one]);
    lo.add(NodeKind::Assign, Payload::None, span, &[operand, bin])
}
pub(super) fn lower_call(lo: &mut Lowering, node: TsNode) -> NodeId {
    let span = lo.span(node);
    let mut kids = Vec::new();
    if let Some(f) = node.child_by_field_name("function") {
        kids.push(lower_expr(lo, f));
    }
    if let Some(args) = node.child_by_field_name("arguments") {
        for a in Lowering::named_children(args) {
            kids.push(lower_expr(lo, a));
        }
    }
    lo.add(NodeKind::Call, Payload::None, span, &kids)
}
pub(super) fn lower_field_expr(lo: &mut Lowering, node: TsNode) -> NodeId {
    let span = lo.span(node);
    let base = node
        .child_by_field_name("argument")
        .map(|o| lower_expr(lo, o))
        .unwrap_or_else(|| lo.empty_block(span));
    let field = node
        .child_by_field_name("field")
        .map(|f| lo.sym(lo.text(f)));
    lo.add(
        NodeKind::Field,
        field.map(Payload::Name).unwrap_or(Payload::None),
        span,
        &[base],
    )
}
pub(super) fn lower_cast(lo: &mut Lowering, node: TsNode) -> NodeId {
    let span = lo.span(node);
    let value = node
        .child_by_field_name("argument")
        .or_else(|| node.child_by_field_name("value"))
        .or_else(|| last_named_child(node));
    let lowered = value
        .map(|c| lower_expr(lo, c))
        .unwrap_or_else(|| lo.empty_block(span));
    let cast_ty = node
        .child_by_field_name("type")
        .map(|ty| lo.text(ty))
        .unwrap_or("");
    if let Some(dependencies) = c_unsigned_32_cast_type_dependencies(lo, cast_ty) {
        if value.is_some_and(c_cast_operand_may_be_byte_lane) {
            lo.record_evidence_with_pack_dependencies(
                EvidenceAnchor::source_span(span),
                EvidenceKind::Source(SourceFactKind::Cast(SourceCastKind::CUnsigned32)),
                nose_semantics::C_LANGUAGE_PACK_ID,
                nose_semantics::C_UNSIGNED_32_CAST_SOURCE_PRODUCER_ID,
                dependencies,
            );
            return lo.add(
                NodeKind::Call,
                Payload::Builtin(Builtin::UnsignedCast32),
                span,
                &[lowered],
            );
        }
    }
    lowered
}
pub(super) fn c_unsigned_32_cast_type_dependencies(
    lo: &Lowering,
    text: &str,
) -> Option<Vec<EvidenceId>> {
    let tokens: Vec<String> = c_identifier_tokens(text)
        .into_iter()
        .filter(|token| !matches!(token.as_str(), "const" | "volatile" | "restrict"))
        .collect();
    if matches!(
        tokens.as_slice(),
        [token] if token == "unsigned" || token == "uint32_t"
    ) || matches!(tokens.as_slice(), [first, second] if first == "unsigned" && second == "int")
    {
        return Some(Vec::new());
    }
    let [alias] = tokens.as_slice() else {
        return None;
    };
    lo.unsigned_32_aliases
        .iter()
        .find_map(|known| (known.alias == *alias).then(|| known.evidence.into_iter().collect()))
}
pub(super) fn c_cast_operand_may_be_byte_lane(node: TsNode) -> bool {
    match node.kind() {
        "subscript_expression" => true,
        "parenthesized_expression" | "pointer_expression" => node
            .child_by_field_name("argument")
            .or_else(|| node.child_by_field_name("value"))
            .or_else(|| last_named_child(node))
            .is_some_and(c_cast_operand_may_be_byte_lane),
        _ => false,
    }
}
pub(super) fn lower_binary(lo: &mut Lowering, node: TsNode) -> NodeId {
    crate::lower::binary(lo, node, common_bin_op, lower_expr)
}
