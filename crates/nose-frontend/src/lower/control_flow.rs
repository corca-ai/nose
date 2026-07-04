use super::*;
use nose_il::LoopKind;

/// Lower a C-family `switch` (scrutinee in the `condition` field, case groups in
/// `body`) to an `if`/else-if chain. Case labels become `scrutinee == label`
/// conditions; a default label becomes the final `else`. Frontends supply only
/// which child nodes are case groups (`is_case`) and how to lower expressions and
/// statements.
pub(crate) fn switch_to_if_chain(
    lo: &mut Lowering,
    node: TsNode,
    is_case: impl Fn(&str) -> bool,
    mut lower_expr: impl FnMut(&mut Lowering, TsNode) -> NodeId,
    mut lower_stmt: impl FnMut(&mut Lowering, TsNode) -> Option<NodeId>,
) -> NodeId {
    let span = lo.span(node);
    let scrutinee = node
        .child_by_field_name("condition")
        .map(|c| lower_expr(lo, c))
        .unwrap_or_else(|| lo.empty_block(span));
    let cases: Vec<TsNode> = node
        .child_by_field_name("body")
        .map(|b| {
            Lowering::named_children(b)
                .into_iter()
                .filter(|c| is_case(c.kind()))
                .collect()
        })
        .unwrap_or_default();

    let mut branches = Vec::new();
    let mut default_block = None;
    for case in cases {
        let (labels, block) = lower_switch_case(lo, case, span, &mut lower_expr, &mut lower_stmt);
        match fold_switch_labels(lo, span, scrutinee, labels) {
            Some(cond) => branches.push((cond, block)),
            None => default_block = Some(block),
        }
    }

    let mut acc = default_block.unwrap_or_else(|| lo.empty_block(span));
    for (cond, block) in branches.into_iter().rev() {
        acc = lo.add(NodeKind::If, Payload::None, span, &[cond, block, acc]);
    }
    acc
}

fn lower_switch_case<E, S>(
    lo: &mut Lowering,
    case: TsNode,
    span: Span,
    lower_expr: &mut E,
    lower_stmt: &mut S,
) -> (Vec<NodeId>, NodeId)
where
    E: FnMut(&mut Lowering, TsNode) -> NodeId,
    S: FnMut(&mut Lowering, TsNode) -> Option<NodeId>,
{
    let mut labels = Vec::new();
    let mut stmts = Vec::new();
    let mut label_phase = true;
    let mut saw_explicit_label = false;

    for child in Lowering::named_children(case) {
        if label_phase && child.kind() == "switch_label" {
            saw_explicit_label = true;
            for label in Lowering::named_children(child) {
                labels.push(lower_expr(lo, label));
            }
            continue;
        }
        if label_phase && !saw_explicit_label && !is_switch_body_child(child.kind()) {
            labels.push(lower_expr(lo, child));
            continue;
        }

        label_phase = false;
        if let Some(id) = lower_stmt(lo, child) {
            stmts.push(id);
        }
    }

    let block = lo.add(NodeKind::Block, Payload::None, span, &stmts);
    (labels, block)
}

pub(crate) fn fold_switch_labels(
    lo: &mut Lowering,
    span: Span,
    scrutinee: NodeId,
    labels: Vec<NodeId>,
) -> Option<NodeId> {
    let mut acc = None;
    for label in labels {
        let cond = lo.add(
            NodeKind::BinOp,
            Payload::Op(Op::Eq),
            span,
            &[scrutinee, label],
        );
        acc = Some(match acc {
            None => cond,
            Some(prev) => lo.add(NodeKind::BinOp, Payload::Op(Op::Or), span, &[prev, cond]),
        });
    }
    acc
}

fn is_switch_body_child(kind: &str) -> bool {
    matches!(
        kind,
        "assert_statement"
            | "block"
            | "break_statement"
            | "compound_statement"
            | "continue_statement"
            | "declaration"
            | "do_statement"
            | "expression_statement"
            | "for_statement"
            | "if_statement"
            | "labeled_statement"
            | "local_variable_declaration"
            | "return_statement"
            | "switch_statement"
            | "synchronized_statement"
            | "throw_statement"
            | "try_statement"
            | "try_with_resources_statement"
            | "while_statement"
            | "yield_statement"
    )
}

/// Lower a `condition`/`body`-shaped CST node into a canonical `While` [`Loop`].
/// Every C-family `while` lowers identically apart from *how* its condition and
/// body sub-nodes are lowered, so each frontend supplies those two as closures
/// and shares the field-extraction, empty-fallback, and node-construction here.
pub(crate) fn while_loop(
    lo: &mut Lowering,
    node: TsNode,
    lower_cond: impl FnOnce(&mut Lowering, TsNode) -> NodeId,
    lower_body: impl FnOnce(&mut Lowering, TsNode) -> NodeId,
) -> NodeId {
    let span = lo.span(node);
    let cond = node
        .child_by_field_name("condition")
        .map(|c| lower_cond(lo, c))
        .unwrap_or_else(|| lo.empty_block(span));
    let body = node
        .child_by_field_name("body")
        .map(|b| lower_body(lo, b))
        .unwrap_or_else(|| lo.empty_block(span));
    lo.add(
        NodeKind::Loop,
        Payload::Loop(LoopKind::While),
        span,
        &[cond, body],
    )
}

/// Lower a statement that may *already* be a block into a canonical [`Block`].
/// Frontends name their block node differently (`block`, `compound_statement`,
/// `statement_block`) and lower a bare statement their own way, so the block kind
/// and the two lowerings vary as params; the "already a block? lower it : wrap the
/// single statement" decision is shared.
pub(crate) fn stmt_as_block(
    lo: &mut Lowering,
    node: TsNode,
    block_kind: &str,
    lower_block: impl FnOnce(&mut Lowering, TsNode) -> NodeId,
    lower_stmt: impl FnOnce(&mut Lowering, TsNode) -> Option<NodeId>,
) -> NodeId {
    if node.kind() == block_kind {
        lower_block(lo, node)
    } else {
        let span = lo.span(node);
        let s = lower_stmt(lo, node);
        lo.block_of_stmt(span, s)
    }
}

/// Lower a `condition`/`consequence`/`alternative`-shaped CST node into a canonical
/// [`If`]. Those three field names are *shared* across the C-family grammars, so no
/// grammar wart leaks in; only how each sub-node is lowered (and how the optional
/// else branch is resolved — a bare block, or an else-if recursion) varies, so each
/// frontend supplies those as closures and shares the field-extraction,
/// empty-fallback, and `[cond, then, else?]` construction here.
pub(crate) fn if_stmt(
    lo: &mut Lowering,
    node: TsNode,
    lower_cond: impl FnOnce(&mut Lowering, TsNode) -> NodeId,
    lower_then: impl FnOnce(&mut Lowering, TsNode) -> NodeId,
    lower_else: impl FnOnce(&mut Lowering, TsNode) -> NodeId,
) -> NodeId {
    let span = lo.span(node);
    let cond = node
        .child_by_field_name("condition")
        .map(|c| lower_cond(lo, c))
        .unwrap_or_else(|| lo.empty_block(span));
    let then = node
        .child_by_field_name("consequence")
        .map(|c| lower_then(lo, c))
        .unwrap_or_else(|| lo.empty_block(span));
    let mut kids = vec![cond, then];
    if let Some(alt) = node.child_by_field_name("alternative") {
        kids.push(lower_else(lo, alt));
    }
    lo.add(NodeKind::If, Payload::None, span, &kids)
}

/// Lower an `init`/`cond`/`update`/`body` C-style `for` into a canonical `CStyle`
/// [`Loop`]. The four sub-node lowerings are closures; the two clause field names
/// that differ across grammars (`initializer` vs `init`, `update` vs `increment`)
/// are params. Every absent or empty clause falls back to an empty block so the
/// `[init, cond, update, body]` arity is fixed.
#[allow(clippy::too_many_arguments)]
pub(crate) fn c_style_for(
    lo: &mut Lowering,
    node: TsNode,
    init_field: &str,
    update_field: &str,
    lower_init: impl FnOnce(&mut Lowering, TsNode) -> Option<NodeId>,
    lower_cond: impl FnOnce(&mut Lowering, TsNode) -> NodeId,
    lower_update: impl FnOnce(&mut Lowering, TsNode) -> NodeId,
    lower_body: impl FnOnce(&mut Lowering, TsNode) -> NodeId,
) -> NodeId {
    let span = lo.span(node);
    let init = node
        .child_by_field_name(init_field)
        .and_then(|n| lower_init(lo, n))
        .unwrap_or_else(|| lo.empty_block(span));
    let cond = node
        .child_by_field_name("condition")
        .map(|c| lower_cond(lo, c))
        .unwrap_or_else(|| lo.empty_block(span));
    let update = node
        .child_by_field_name(update_field)
        .map(|u| lower_update(lo, u))
        .unwrap_or_else(|| lo.empty_block(span));
    let body = node
        .child_by_field_name("body")
        .map(|b| lower_body(lo, b))
        .unwrap_or_else(|| lo.empty_block(span));
    lo.add(
        NodeKind::Loop,
        Payload::Loop(LoopKind::CStyle),
        span,
        &[init, cond, update, body],
    )
}
