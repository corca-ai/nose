use super::*;

/// `match e { p1 => b1, p2 => b2, … }` → nested `if`/`else` chain, each arm's
/// pattern lowered as a comparison-ish condition (approximate, but converges with
/// equivalent switch/if-chains in other languages).
pub(super) fn lower_match(lo: &mut Lowering, node: TsNode) -> NodeId {
    let span = lo.span(node);
    let scrutinee = node
        .child_by_field_name("value")
        .map(|v| lower_expr(lo, v))
        .unwrap_or_else(|| lo.empty_block(span));
    let arms: Vec<TsNode> = node
        .child_by_field_name("body")
        .map(|b| {
            Lowering::named_children(b)
                .into_iter()
                .filter(|c| c.kind() == "match_arm")
                .collect()
        })
        .unwrap_or_default();
    let mut branches = Vec::new();
    let mut default_body = None;
    for arm in arms {
        let body_node = arm.child_by_field_name("value");
        let body_node_id = body_node.map(|v| v.id());
        let pattern = arm.child_by_field_name("pattern").or_else(|| {
            Lowering::named_children(arm)
                .into_iter()
                .find(|child| Some(child.id()) != body_node_id && !is_match_guard(*child))
        });
        let mut body = body_node
            .map(|v| {
                if v.kind() == "block" {
                    lower_block(lo, v)
                } else {
                    lower_expr(lo, v)
                }
            })
            .unwrap_or_else(|| lo.empty_block(span));
        // Bind the pattern's payload locals (`Some(v)` → `v = scrutinee.0`) ahead of the arm
        // body so its uses of them alpha-canonicalize — converging two copies that differ only
        // in the binding name (`Some(a) => f(a)` with `Some(b) => f(b)`), the #390 follow-up.
        if let Some(mut binds) =
            pattern.and_then(|pattern| rust_bind_arm_pattern(lo, pattern, scrutinee, span))
        {
            binds.push(body);
            body = lo.add(NodeKind::Block, Payload::None, span, &binds);
        }
        let pattern_cond =
            pattern.and_then(|pattern| lower_match_pattern_condition(lo, scrutinee, pattern, span));
        let guard_cond = arm
            .child_by_field_name("condition")
            .or_else(|| arm.child_by_field_name("guard"))
            .or_else(|| {
                Lowering::named_children(arm)
                    .into_iter()
                    .find(|child| is_match_guard(*child))
                    .and_then(|guard| guard.named_child(0))
            })
            .map(|guard| lower_expr(lo, guard));
        let Some(cond) =
            crate::lower::combine_optional_conditions(lo, span, pattern_cond, guard_cond)
        else {
            default_body = Some(body);
            continue;
        };
        branches.push((cond, body));
    }
    let mut acc = default_body.unwrap_or_else(|| lo.empty_block(span));
    for (cond, body) in branches.into_iter().rev() {
        acc = lo.add(NodeKind::If, Payload::None, span, &[cond, body, acc]);
    }
    acc
}
pub(super) fn is_match_guard(node: TsNode) -> bool {
    matches!(node.kind(), "match_arm_guard" | "match_guard")
}
pub(super) fn lower_match_pattern_condition(
    lo: &mut Lowering,
    scrutinee: NodeId,
    pattern: TsNode,
    span: Span,
) -> Option<NodeId> {
    if lo.text(pattern).trim() == "_" || pattern.kind() == "wildcard_pattern" {
        return None;
    }
    if pattern.kind() == "match_pattern" {
        let guard = pattern.child_by_field_name("condition");
        let guard_id = guard.map(|guard| guard.id());
        let pattern_cond = pattern
            .child_by_field_name("pattern")
            .or_else(|| {
                Lowering::named_children(pattern)
                    .into_iter()
                    .find(|child| Some(child.id()) != guard_id)
            })
            .and_then(|child| lower_match_pattern_condition(lo, scrutinee, child, span));
        let guard_cond = guard.map(|guard| lower_expr(lo, guard));
        return crate::lower::combine_optional_conditions(lo, span, pattern_cond, guard_cond);
    }
    if pattern.kind() == "or_pattern" {
        let mut conditions = Vec::new();
        for child in Lowering::named_children(pattern) {
            let cond = lower_match_pattern_condition(lo, scrutinee, child, span)?;
            conditions.push(cond);
        }
        return crate::lower::fold_or_conditions(lo, span, conditions);
    }
    if pattern.kind() == "range_pattern" {
        return lower_range_pattern_condition(lo, scrutinee, pattern, span);
    }
    // `Some(_)`-style single-wildcard patterns are a recognized *presence* test: the source
    // fact lets a downstream idiom converge `if let Some(_)` with `.is_some()`. Keep the
    // pattern node itself in the comparison so the source fact remains anchored to the evaluated
    // selector node.
    let presence_wildcard = rust_tuple_struct_wildcard_pattern(lo, pattern);
    if presence_wildcard {
        lo.record_source_fact(
            lo.span(pattern),
            SourceFactKind::Pattern(SourcePatternKind::RustTupleStructSingleWildcardPattern),
        );
    }
    // A *binding* constructor pattern (`Some(v)`, `Ok(_)` with payload, `Point { x, y }`) tests
    // the scrutinee's VARIANT; the inner bindings bind in the arm body and are not part of the
    // discriminant. Lower the constructor PATH as the comparison value — parallel to a unit
    // variant like `None` — rather than the whole pattern as an expression, which (a) emits an
    // opaque `Raw` node (the dominant Rust pattern coverage loss, #390 §CP) and (b) keys the
    // test on the binding name's subtree hash, splitting two copies that differ only in
    // `Some(x)` vs `Some(y)`. Lowering the path makes the variant test binding-name-invariant.
    let pat = if presence_wildcard {
        lower_expr(lo, pattern)
    } else if matches!(pattern.kind(), "tuple_struct_pattern" | "struct_pattern") {
        let target = constructor_path_node(pattern).unwrap_or(pattern);
        lower_expr(lo, target)
    } else {
        lower_expr(lo, pattern)
    };
    Some(lo.add(
        NodeKind::BinOp,
        Payload::Op(Op::Eq),
        span,
        &[scrutinee, pat],
    ))
}
/// Bind a constructor pattern's payload locals as assignment targets so the arm body's uses of
/// them alpha-canonicalize (`alpha.rs` renames a `Var` only when it is *bound* in scope) —
/// converging copies that differ only in the binding name. Returns the binding assignments to
/// prepend to the arm body, or `None` to leave the body unchanged.
///
/// Deliberately conservative — it must never emit a *wrong* projection (an unsound binding could
/// seed a false merge):
/// - `struct_pattern` (`Point { x, y }`): reuse the field-named projection (position-independent).
/// - `tuple_struct_pattern` with exactly ONE payload element that is a simple binding (`Some(v)`,
///   `Ok(v)`, `Err(e)`): bind it to field `0`, an unambiguous position. Multi-element tuple
///   structs (where a `_` wildcard could shift positions) and nested/complex payloads are left
///   unbound — no convergence, but no soundness risk.
pub(super) fn rust_bind_arm_pattern(
    lo: &mut Lowering,
    pattern: TsNode,
    scrutinee: NodeId,
    span: Span,
) -> Option<Vec<NodeId>> {
    match pattern.kind() {
        // A match arm's pattern is wrapped in `match_pattern` (with an optional `if` guard);
        // unwrap to the inner pattern, like `lower_match_pattern_condition`. The guard adds no
        // bindings we project, so binding from the inner pattern alone is correct.
        "match_pattern" => {
            let inner = pattern
                .child_by_field_name("pattern")
                .or_else(|| Lowering::named_children(pattern).into_iter().next())?;
            rust_bind_arm_pattern(lo, inner, scrutinee, span)
        }
        "struct_pattern" => lower_static_projection_pattern(lo, pattern, scrutinee, span),
        "tuple_struct_pattern" => {
            let path_id = constructor_path_node(pattern).map(|p| p.id());
            let payload: Vec<TsNode> = Lowering::named_children(pattern)
                .into_iter()
                .filter(|child| Some(child.id()) != path_id)
                .collect();
            let [only] = payload[..] else {
                return None;
            };
            let name = rust_binding_name(lo, only)?;
            Some(vec![rust_projection_assign(
                lo, scrutinee, "0", &name, span,
            )])
        }
        _ => None,
    }
}
/// The constructor path of a tuple-struct / struct pattern (`Some`, `Ok`, `mod::Point`) — the
/// discriminant a variant test keys on, with the inner bindings dropped. `None` if no path
/// child is present (the caller then falls back to lowering the whole pattern).
pub(super) fn constructor_path_node(pattern: TsNode) -> Option<TsNode> {
    pattern.child_by_field_name("type").or_else(|| {
        Lowering::named_children(pattern).into_iter().find(|c| {
            matches!(
                c.kind(),
                "identifier"
                    | "scoped_identifier"
                    | "type_identifier"
                    | "scoped_type_identifier"
                    | "qualified_type"
            )
        })
    })
}
pub(super) fn lower_range_bounds(
    lo: &mut Lowering,
    node: TsNode,
) -> (Option<NodeId>, Option<NodeId>, bool) {
    let mut start: Option<NodeId> = None;
    let mut end: Option<NodeId> = None;
    let mut inclusive = false;
    let mut seen_op = false;
    let mut cursor = node.walk();
    for child in node.children(&mut cursor) {
        match child.kind() {
            ".." => seen_op = true,
            "..=" | "..." => {
                seen_op = true;
                inclusive = true;
            }
            _ if child.is_named() => {
                let v = lower_expr(lo, child);
                if seen_op {
                    end = Some(v);
                } else {
                    start = Some(v);
                }
            }
            _ => {}
        }
    }
    (start, end, inclusive)
}
pub(super) fn rust_tuple_struct_wildcard_pattern(lo: &Lowering, pattern: TsNode) -> bool {
    if pattern.kind() != "tuple_struct_pattern" {
        return false;
    }
    let text = lo.text(pattern).trim();
    let Some(open) = text.find('(') else {
        return false;
    };
    let Some(close) = text.rfind(')') else {
        return false;
    };
    if close <= open {
        return false;
    }
    let args = text[open + 1..close].trim().trim_end_matches(',').trim();
    args == "_"
}
pub(super) fn lower_range_pattern_condition(
    lo: &mut Lowering,
    scrutinee: NodeId,
    pattern: TsNode,
    span: Span,
) -> Option<NodeId> {
    let (start, end, inclusive) = lower_range_bounds(lo, pattern);
    let lower = start.map(|start| {
        lo.add(
            NodeKind::BinOp,
            Payload::Op(Op::Ge),
            span,
            &[scrutinee, start],
        )
    });
    let upper = end.map(|end| {
        let op = if inclusive { Op::Le } else { Op::Lt };
        lo.add(NodeKind::BinOp, Payload::Op(op), span, &[scrutinee, end])
    });
    crate::lower::combine_optional_conditions(lo, span, lower, upper)
}
