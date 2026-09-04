//! JavaScript / TypeScript -> raw IL lowering.
//!
//! JS and TS share one walk; TypeScript type syntax is erased (`type_annotation`,
//! `as`/`satisfies`/`!` are stripped). Convergence-friendly lowering:
//! `a as T` reduces to `a`; `i++` and `x += 1` desugar to assignments;
//! C-style `for`, `for...of`, `do/while` map to the unified `Loop`; `switch`
//! becomes an `if`/`else if` chain; ternary lowers to an expression `If`.

mod array_parameters;
mod control;
mod declarations;
mod expressions;
mod globals;
mod imports;
mod jsx;
mod operators;
mod record_guard;
mod syntax;
mod types;

use crate::lower::Lowering;
use control::{
    lower_aug_assignment, lower_for_c, lower_for_in, lower_if, lower_switch, lower_try,
    lower_update, lower_while,
};
use declarations::{
    lower_class, lower_class_static_block, lower_decorated_definition, lower_decorator,
    lower_field, lower_func, lower_own_decorated_definition, lower_var_decl,
};
use expressions::lower_expr;
use imports::{is_exportable_decl, lower_static_import};
use nose_il::{FileId, Il, Interner, Lang, NodeId, NodeKind, Payload, Span};
use tree_sitter::Node as TsNode;
use types::lower_type_decl;

pub(crate) fn lower(
    file: FileId,
    path: &str,
    src: &[u8],
    lang: Lang,
    interner: &Interner,
) -> anyhow::Result<Il> {
    use crate::lower::grammar;
    // JS, TS, and TSX share a parse path but use distinct grammars; .tsx needs the
    // TSX dialect (JSX-aware), other TS files the plain TypeScript grammar.
    let key = if lang == Lang::JavaScript {
        grammar::JAVASCRIPT
    } else if path.ends_with(".tsx") {
        grammar::TSX
    } else {
        grammar::TYPESCRIPT
    };
    crate::lower::lower_file(
        file,
        path,
        src,
        interner,
        key,
        || match key {
            grammar::JAVASCRIPT => tree_sitter_javascript::LANGUAGE.into(),
            grammar::TSX => tree_sitter_typescript::LANGUAGE_TSX.into(),
            _ => tree_sitter_typescript::LANGUAGE_TYPESCRIPT.into(),
        },
        lang,
        lower_program,
    )
}

fn lower_program(lo: &mut Lowering, node: TsNode) -> NodeId {
    lower_stmt_list(lo, node, NodeKind::Module, false)
}

pub(super) fn lower_block(lo: &mut Lowering, node: TsNode) -> NodeId {
    lower_stmt_list(lo, node, NodeKind::Block, false)
}

pub(super) fn lower_stmt_list(
    lo: &mut Lowering,
    node: TsNode,
    kind: NodeKind,
    in_class: bool,
) -> NodeId {
    let span = lo.span(node);
    let mut stmts = Vec::new();
    let mut pending_decorators = Vec::new();
    let mut decorator_span: Option<Span> = None;
    for child in Lowering::named_children(node) {
        if child.kind() == "decorator" {
            decorator_span = Some(match decorator_span {
                Some(existing) => existing.merge(lo.span(child)),
                None => lo.span(child),
            });
            pending_decorators.push(lower_decorator(lo, child));
            continue;
        }
        if let Some(stmt) = lower_stmt(lo, child, in_class) {
            if pending_decorators.is_empty() {
                stmts.push(stmt);
            } else {
                let kids = std::mem::take(&mut pending_decorators);
                let decorated_span = decorator_span.take().unwrap_or(span).merge(lo.span(child));
                stmts.push(lower_decorated_definition(
                    lo,
                    kids,
                    lo.span(child),
                    decorated_span,
                    stmt,
                ));
            }
        } else if !pending_decorators.is_empty() {
            stmts.extend(std::mem::take(&mut pending_decorators));
            decorator_span = None;
        }
    }
    stmts.extend(pending_decorators);
    lo.add(kind, Payload::None, span, &stmts)
}

pub(super) fn lower_stmt(lo: &mut Lowering, node: TsNode, in_class: bool) -> Option<NodeId> {
    let span = lo.span(node);
    match node.kind() {
        "function_declaration"
        | "generator_function_declaration"
        | "function_expression"
        | "generator_function" => {
            let out = lower_func(lo, node, in_class);
            Some(lower_own_decorated_definition(lo, node, out))
        }
        // ambient declarations without bodies - no behavior to model
        "function_signature" => None,
        "method_definition" => {
            let out = lower_func(lo, node, true);
            Some(lower_own_decorated_definition(lo, node, out))
        }
        "class_declaration" | "class" | "abstract_class_declaration" => Some(lower_class(lo, node)),
        "lexical_declaration" | "variable_declaration" => Some(lower_var_decl(lo, node)),
        "if_statement" => Some(lower_if(lo, node)),
        "for_statement" => Some(lower_for_c(lo, node)),
        "for_in_statement" => Some(lower_for_in(lo, node)),
        "while_statement" | "do_statement" => Some(lower_while(lo, node)),
        "switch_statement" => Some(lower_switch(lo, node)),
        "return_statement" => {
            let mut kids = Vec::new();
            if let Some(v) = node.named_child(0) {
                kids.push(lower_expr(lo, v));
            }
            Some(lo.add(NodeKind::Return, Payload::None, span, &kids))
        }
        "throw_statement" => {
            let mut kids = Vec::new();
            if let Some(v) = node.named_child(0) {
                kids.push(lower_expr(lo, v));
            }
            Some(lo.add(NodeKind::Throw, Payload::None, span, &kids))
        }
        "try_statement" => Some(lower_try(lo, node)),
        "break_statement" => Some(lo.add(NodeKind::Break, Payload::None, span, &[])),
        "continue_statement" => Some(lo.add(NodeKind::Continue, Payload::None, span, &[])),
        "statement_block" => Some(lower_block(lo, node)),
        "class_static_block" => Some(lower_class_static_block(lo, node)),
        "decorator" => Some(lower_decorator(lo, node)),
        // `label: stmt` (loop/block label) - lower the inner statement, drop the label.
        "labeled_statement" => Lowering::named_children(node)
            .into_iter()
            .next_back()
            .and_then(|s| lower_stmt(lo, s, in_class)),
        "expression_statement" => {
            let child = node.named_child(0)?;
            Some(match child.kind() {
                "assignment_expression" => crate::lower::assignment(lo, child, lower_expr, lower_expr),
                "augmented_assignment_expression" => lower_aug_assignment(lo, child),
                "update_expression" => lower_update(lo, child),
                _ => {
                    let e = lower_expr(lo, child);
                    lo.add(NodeKind::ExprStmt, Payload::None, span, &[e])
                }
            })
        }
        "empty_statement" => None,
        "import_statement" => Some(
            lower_static_import(lo, node).unwrap_or_else(|| crate::lower::import_tokens(lo, node)),
        ),
        "export_statement" => {
            // Only an `export <decl>` carries behavior; re-exports
            // (`export {...} from "..."`, `export * ...`) are erased.
            match node.named_child(0) {
                Some(d) if is_exportable_decl(d.kind()) => lower_stmt(lo, d, in_class),
                _ => None,
            }
        }
        // A type *declaration* is content worth deduplicating: copy-pasted type/
        // interface/enum definitions (e.g. generated `.gen.ts` files) are real
        // duplication. Lower to a structural unit (names + literals + shape). This is
        // distinct from erasing type *annotations* in code (behavioral convergence).
        "type_alias_declaration" | "interface_declaration" | "enum_declaration" => {
            Some(lower_type_decl(lo, node))
        }
        // TypeScript type-only / ambient constructs and bare specifiers: erase.
        "ambient_declaration"
        | "method_signature"
        | "abstract_method_signature"
        | "import_alias"
        | "export_specifier"
        | "export_clause"
        | "comment"
        // legacy `<!-- -->` HTML comments are valid JS tokens (common in <script> blocks)
        | "html_comment" => None,
        // class fields when walking a class body
        "field_definition" | "public_field_definition" => {
            let out = lower_field(lo, node);
            out.map(|id| lower_own_decorated_definition(lo, node, id))
        }
        // Anything else in statement position: treat as an expression statement
        // (lower_expr has its own Raw fallback for genuinely unknown nodes).
        _ => {
            let e = lower_expr(lo, node);
            Some(lo.add(NodeKind::ExprStmt, Payload::None, span, &[e]))
        }
    }
}

#[cfg(test)]
mod tests;
