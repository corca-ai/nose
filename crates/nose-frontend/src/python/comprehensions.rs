use super::*;

pub(super) fn lower_comprehension_pair(lo: &mut Lowering, node: TsNode) -> NodeId {
    let span = lo.span(node);
    let kids: Vec<NodeId> = semantic_named_children(node)
        .into_iter()
        .map(|c| lower_expr(lo, c))
        .collect();
    lo.add(
        NodeKind::Seq,
        Payload::Builtin(Builtin::DictEntry),
        span,
        &kids,
    )
}
/// Build a lambda `λ<pattern>. <Block[Return[body]]>` over a comprehension's
/// iteration pattern (the `for x in …` target), so the body converges with a JS
/// `x => body` arrow.
pub(super) fn comp_lambda(
    lo: &mut Lowering,
    pattern: Option<TsNode>,
    body: NodeId,
    bspan: Span,
) -> NodeId {
    let mut kids = Vec::new();
    if let Some(p) = pattern {
        push_pattern_params(lo, p, &mut kids);
    }
    let ret = lo.add(NodeKind::Return, Payload::None, bspan, &[body]);
    let block = lo.add(NodeKind::Block, Payload::None, bspan, &[ret]);
    kids.push(block);
    lo.add(NodeKind::Lambda, Payload::None, bspan, &kids)
}
/// A comprehension `[body for x in xs]` lowers to `HoF(Map)[xs, λx. body]`, with
/// the body wrapped as `Block[Return[body]]` so it converges with a JS
/// `xs.map(x => body)` arrow (whose expression body lowers the same way). A filter
/// `… if cond` wraps the collection in `HoF(Filter)[xs, λx. cond]`, so a filtered
/// comprehension converges with a guarded loop (`if cond: …`) — see §AI.
///
/// A *multi-clause* comprehension (`[body for a in A for b in B]`) lowers to a
/// first-class flat-map nesting — see [`lower_multi_clause_comprehension`].
pub(super) fn lower_comprehension(lo: &mut Lowering, node: TsNode) -> NodeId {
    let span = lo.span(node);
    if let Some(kind) = python_comprehension_kind(node.kind()) {
        lo.record_source_fact(span, SourceFactKind::Comprehension(kind));
    }
    let body_node = node.named_child(0);

    let for_clauses = semantic_named_children(node)
        .into_iter()
        .filter(|c| c.kind() == "for_in_clause")
        .count();
    if for_clauses >= 2 {
        return lower_multi_clause_comprehension(lo, node, span, body_node);
    }

    let clause = semantic_named_children(node)
        .into_iter()
        .find(|c| c.kind() == "for_in_clause");
    let pattern = clause.and_then(|c| c.child_by_field_name("left").or_else(|| c.named_child(0)));
    let mut collection = clause
        .and_then(|c| c.child_by_field_name("right").or_else(|| c.named_child(1)))
        .map(|r| lower_expr(lo, r))
        .unwrap_or_else(|| lo.empty_block(span));

    // Each `if cond` clause wraps the collection in a `HoF(Filter)`.
    for f in semantic_named_children(node) {
        if f.kind() != "if_clause" {
            continue;
        }
        if let Some(cn) = first_semantic_named_child(f) {
            let fspan = lo.span(f);
            let cond = lower_expr(lo, cn);
            let flam = comp_lambda(lo, pattern, cond, fspan);
            collection = lo.add(
                NodeKind::HoF,
                Payload::HoF(HoFKind::Filter),
                fspan,
                &[collection, flam],
            );
        }
    }

    let body = body_node
        .map(|b| lower_expr(lo, b))
        .unwrap_or_else(|| lo.empty_block(span));
    let map_lam = comp_lambda(lo, pattern, body, span);
    let mapped = lo.add(
        NodeKind::HoF,
        Payload::HoF(HoFKind::Map),
        span,
        &[collection, map_lam],
    );
    if let Some(clause) = clause {
        wrap_async_comprehension_clause(lo, clause, mapped)
    } else {
        mapped
    }
}
pub(super) fn python_comprehension_kind(kind: &str) -> Option<SourceComprehensionKind> {
    Some(match kind {
        "list_comprehension" => SourceComprehensionKind::PythonListComprehension,
        "set_comprehension" => SourceComprehensionKind::PythonSetComprehension,
        "dictionary_comprehension" => SourceComprehensionKind::PythonDictComprehension,
        "generator_expression" => SourceComprehensionKind::PythonGeneratorExpression,
        _ => return None,
    })
}
/// Lower a comprehension with more than one `for` clause. `[body for a in A for
/// b in B]` is Python sugar for nested iteration that *flattens* — equivalent to
/// `A.flatMap(a => B.map(b => body))`. The innermost clause maps to produced
/// elements; each outer clause flat-maps the list produced by the next inner
/// layer. This stays distinct from the genuinely different nested comprehension
/// `[[body for b in B] for a in A]`, which lowers to `Map[A, λa. Map[B, ...]]`.
pub(super) fn lower_multi_clause_comprehension(
    lo: &mut Lowering,
    node: TsNode,
    span: Span,
    body_node: Option<TsNode>,
) -> NodeId {
    // Group each `for` clause with the `if` clauses that follow it, in source order.
    let mut groups: Vec<(TsNode, Vec<TsNode>)> = Vec::new();
    for c in semantic_named_children(node) {
        match c.kind() {
            "for_in_clause" => groups.push((c, Vec::new())),
            "if_clause" => {
                if let Some(last) = groups.last_mut() {
                    last.1.push(c);
                }
            }
            _ => {}
        }
    }

    // Build inside-out: the body is the innermost produced element. The innermost
    // `for` maps to elements; every outer `for` flat-maps the list produced by the
    // inner layer.
    let mut inner = body_node
        .map(|b| lower_expr(lo, b))
        .unwrap_or_else(|| lo.empty_block(span));
    for (idx, (forc, ifs)) in groups.iter().rev().enumerate() {
        let pattern = forc
            .child_by_field_name("left")
            .or_else(|| forc.named_child(0));
        let mut collection = forc
            .child_by_field_name("right")
            .or_else(|| forc.named_child(1))
            .map(|r| lower_expr(lo, r))
            .unwrap_or_else(|| lo.empty_block(span));
        for ifc in ifs {
            if let Some(cn) = first_semantic_named_child(*ifc) {
                let fspan = lo.span(*ifc);
                let cond = lower_expr(lo, cn);
                let flam = comp_lambda(lo, pattern, cond, fspan);
                collection = lo.add(
                    NodeKind::HoF,
                    Payload::HoF(HoFKind::Filter),
                    fspan,
                    &[collection, flam],
                );
            }
        }
        let lam = comp_lambda(lo, pattern, inner, span);
        let hof_kind = if idx == 0 {
            HoFKind::Map
        } else {
            HoFKind::FlatMap
        };
        let layer = lo.add(
            NodeKind::HoF,
            Payload::HoF(hof_kind),
            span,
            &[collection, lam],
        );
        inner = wrap_async_comprehension_clause(lo, *forc, layer);
    }

    inner
}

fn wrap_async_comprehension_clause(lo: &mut Lowering, clause: TsNode, node: NodeId) -> NodeId {
    if crate::lower::node_has_child_kind(clause, "async") {
        lo.protocol_boundary(
            lo.span(clause),
            nose_il::SourceProtocolKind::AsyncIteration,
            "async_for",
            &[node],
        )
    } else {
        node
    }
}
/// Emit `Param` nodes for a comprehension/loop target (identifier or tuple).
pub(super) fn push_pattern_params(lo: &mut Lowering, node: TsNode, out: &mut Vec<NodeId>) {
    match node.kind() {
        "tuple_pattern" | "pattern_list" | "tuple" | "list_pattern" => {
            for c in semantic_named_children(node) {
                push_pattern_params(lo, c, out);
            }
        }
        _ => {
            let span = lo.span(node);
            let sym = lo.sym(lo.text(node));
            out.push(lo.add(NodeKind::Param, Payload::Name(sym), span, &[]));
        }
    }
}
