use super::*;
use crate::tree_sitter_ext::child_at;

/// Lower each named child of `node` with `lower_one`, keeping the `Some` results,
/// and wrap them in a `kind` node (`Module` for a file root, `Block` for a body).
/// Every frontend's module/block builders are this same iterate-lower-collect loop
/// differing only in the node kind and per-language statement lowering.
pub(crate) fn collect_into(
    lo: &mut Lowering,
    node: TsNode,
    kind: NodeKind,
    mut lower_one: impl FnMut(&mut Lowering, TsNode) -> Option<NodeId>,
) -> NodeId {
    let span = lo.span(node);
    let mut stmts = Vec::new();
    for child in Lowering::named_children(node) {
        if let Some(id) = lower_one(lo, child) {
            stmts.push(id);
        }
    }
    lo.add(kind, Payload::None, span, &stmts)
}

pub(crate) fn node_has_child_kind(node: TsNode, kind: &str) -> bool {
    (0..node.child_count())
        .any(|index| child_at(node, index).is_some_and(|child| child.kind() == kind))
}

/// Lower the named children of a grammar's `arguments` field.
///
/// Callee identity, missing-callee policy, and special argument surfaces stay
/// in each language adapter; this helper owns only the shared CST field shape.
pub(crate) fn call_arguments(
    lo: &mut Lowering,
    node: TsNode,
    mut lower_argument: impl FnMut(&mut Lowering, TsNode) -> NodeId,
) -> Vec<NodeId> {
    node.child_by_field_name("arguments")
        .map(Lowering::named_children)
        .unwrap_or_default()
        .into_iter()
        .map(|argument| lower_argument(lo, argument))
        .collect()
}

/// Lower a `condition`/`consequence`/`alternative` expression into `If`.
///
/// These field names and the present-child policy are shared by the C, Java,
/// and Ruby grammars. Languages with different ordering or missing-arm policy
/// keep their own adapter.
pub(crate) fn conditional_expression(
    lo: &mut Lowering,
    node: TsNode,
    mut lower_branch: impl FnMut(&mut Lowering, TsNode) -> NodeId,
) -> NodeId {
    let span = lo.span(node);
    let children = ["condition", "consequence", "alternative"]
        .into_iter()
        .filter_map(|field| node.child_by_field_name(field))
        .map(|branch| lower_branch(lo, branch))
        .collect::<Vec<_>>();
    lo.add(NodeKind::If, Payload::None, span, &children)
}

/// Build a `Func` unit from a `name`/`parameters`/`body`-shaped node and register
/// it for detection (a `Method` when `method`, else a `Function`). Every frontend
/// shares this skeleton — extract the name, lower the parameters, lower the body,
/// push the unit; `lower_params` and `lower_body` are the only language-specific
/// pieces (param node shapes and body/return conventions differ per grammar).
pub(crate) fn function_unit(
    lo: &mut Lowering,
    node: TsNode,
    method: bool,
    lower_params: impl FnOnce(&mut Lowering, TsNode, &mut Vec<NodeId>),
    lower_body: impl FnOnce(&mut Lowering, TsNode) -> NodeId,
) -> NodeId {
    let span = lo.span(node);
    let name = node.child_by_field_name("name").map(|n| lo.sym(lo.text(n)));
    let mut kids = Vec::new();
    if let Some(params) = node.child_by_field_name("parameters") {
        lower_params(lo, params, &mut kids);
    }
    let body_node = node.child_by_field_name("body");
    let body = body_node
        .map(|b| lower_body(lo, b))
        .unwrap_or_else(|| lo.empty_block(span));
    kids.push(body);
    let func = lo.add(NodeKind::Func, Payload::None, span, &kids);
    let kind = if method {
        UnitKind::Method
    } else {
        UnitKind::Function
    };
    let origin = imperative_callable_origin(
        if method {
            UnitSubkind::Method
        } else {
            UnitSubkind::Function
        },
        body_node.is_some(),
    );
    lo.push_unit_with_origin(func, kind, name, origin);
    func
}

pub(crate) fn imperative_callable_origin(subkind: UnitSubkind, has_body: bool) -> UnitOrigin {
    UnitOrigin::new(
        UnitDomains::of(UnitDomain::Imperative),
        subkind,
        if has_body {
            UnitBodyKind::Implementation
        } else {
            UnitBodyKind::DeclarationOnly
        },
        SourceGranularity::WholeUnit,
        RegionKind::Code,
    )
    .with_evidence(if has_body {
        UnitEvidenceFlag::HasReusableBody
    } else {
        UnitEvidenceFlag::DeclarationOnly
    })
}

/// Lower a `left`/`operator`/`right` binary-expression node into a `BinOp`. Every
/// supported grammar names those fields identically; each frontend supplies its
/// dialect's operator resolution and its expression lowering. An operator the
/// dialect doesn't recognise (or a missing operand) becomes a `Raw` node that
/// preserves the children — never a silently-wrong default operator.
pub(crate) fn binary(
    lo: &mut Lowering,
    node: TsNode,
    op_of: impl FnOnce(&str) -> Option<Op>,
    mut lower_operand: impl FnMut(&mut Lowering, TsNode) -> NodeId,
) -> NodeId {
    let span = lo.span(node);
    let l = node
        .child_by_field_name("left")
        .map(|x| lower_operand(lo, x));
    let r = node
        .child_by_field_name("right")
        .map(|x| lower_operand(lo, x));
    let op_text = node.child_by_field_name("operator").map(|o| lo.text(o));
    let op = op_text.and_then(op_of);
    match (l, r, op) {
        (Some(l), Some(r), Some(op)) => lo.add(NodeKind::BinOp, Payload::Op(op), span, &[l, r]),
        _ => {
            let kids: Vec<NodeId> = Lowering::named_children(node)
                .into_iter()
                .map(|c| lower_operand(lo, c))
                .collect();
            // Key the raw node by the operator spelling, not just the CST kind:
            // two different unmapped operators over the same operands must not
            // share a fingerprint (`a >>> b` is not `a @ b`).
            match op_text {
                Some(text) => lo.raw(&format!("{} {text}", node.kind()), span, &kids),
                None => lo.raw(node.kind(), span, &kids),
            }
        }
    }
}

/// `a OP= b`  →  `a = a OP b` for grammars with `left`/`operator`/`right` fields
/// (Python/JS/Rust). The lhs is lowered twice (two faithful subtrees). The
/// operator is looked up by its compound spelling minus the trailing `=`; an
/// unmapped operator keeps its own raw shape keyed by the spelling — defaulting
/// it to `Add` (or dropping it) would merge `a @= b` / `a >>>= b` with
/// `a += b` / `a = b`.
pub(crate) fn compound_assignment(
    lo: &mut Lowering,
    node: TsNode,
    op_of: impl FnOnce(&str) -> Option<Op>,
    mut lower_target: impl FnMut(&mut Lowering, TsNode) -> NodeId,
    mut lower_operand: impl FnMut(&mut Lowering, TsNode) -> NodeId,
) -> NodeId {
    let span = lo.span(node);
    let left = node.child_by_field_name("left");
    let right = node.child_by_field_name("right");
    let op_text = node.child_by_field_name("operator").map(|o| lo.text(o));
    let op = op_text.and_then(|t| op_of(t.trim_end_matches('=')));
    let lhs1 = left
        .map(|l| lower_target(lo, l))
        .unwrap_or_else(|| lo.empty_block(span));
    let lhs2 = left
        .map(|l| lower_operand(lo, l))
        .unwrap_or_else(|| lo.empty_block(span));
    let rhs = right
        .map(|r| lower_operand(lo, r))
        .unwrap_or_else(|| lo.empty_block(span));
    let value = match op {
        Some(op) => lo.add(NodeKind::BinOp, Payload::Op(op), span, &[lhs2, rhs]),
        None => {
            let kind = match op_text {
                Some(text) => format!("{} {text}", node.kind()),
                None => node.kind().to_string(),
            };
            lo.raw(&kind, span, &[lhs2, rhs])
        }
    };
    lo.add(NodeKind::Assign, Payload::None, span, &[lhs1, value])
}

/// Lower a `left`/`right` assignment-expression node into an `Assign`.
/// JS/TS and Rust grammars use the same field names for simple assignment; compound
/// assignment remains frontend-specific because operator spelling and rewrites differ.
/// Lower an ASSIGNMENT TARGET, keeping a pointer/reference dereference a computed
/// PLACE: `*p = v` stores through `p` — exactly `p[0] = v` — so the target lowers
/// to `Index(p, 0)` and the store stays an ordered effect in the value graph.
/// Dereference READS keep peeling to the operand (each frontend's read convention
/// lets `*x > 0` converge with `x > 0`); only the STORE position must keep the
/// place, or a unit that writes through a pointer fingerprints identically to a
/// bare stub (#210). `deref_operand` is the frontend's "is this node a deref, and
/// of what" test; parentheses peel recursively.
pub(crate) fn deref_store_target<'a>(
    lo: &mut Lowering,
    node: TsNode<'a>,
    deref_operand: impl Fn(&Lowering, TsNode<'a>) -> Option<TsNode<'a>> + Copy,
    mut lower_expr: impl FnMut(&mut Lowering, TsNode<'a>) -> NodeId,
) -> NodeId {
    let span = lo.span(node);
    if node.kind() == "parenthesized_expression" {
        return match node.named_child(0) {
            Some(inner) => deref_store_target(lo, inner, deref_operand, lower_expr),
            None => lo.empty_block(span),
        };
    }
    match deref_operand(lo, node) {
        Some(operand) => {
            let p = lower_expr(lo, operand);
            let zero = lo.int_lit("0", span);
            lo.add(NodeKind::Index, Payload::None, span, &[p, zero])
        }
        None => lower_expr(lo, node),
    }
}

pub(crate) fn assignment(
    lo: &mut Lowering,
    node: TsNode,
    mut lower_target: impl FnMut(&mut Lowering, TsNode) -> NodeId,
    mut lower_expr: impl FnMut(&mut Lowering, TsNode) -> NodeId,
) -> NodeId {
    let span = lo.span(node);
    let lhs = node
        .child_by_field_name("left")
        .map(|l| lower_target(lo, l))
        .unwrap_or_else(|| lo.empty_block(span));
    let rhs = node
        .child_by_field_name("right")
        .map(|r| lower_expr(lo, r))
        .unwrap_or_else(|| lo.empty_block(span));
    lo.add(NodeKind::Assign, Payload::None, span, &[lhs, rhs])
}
