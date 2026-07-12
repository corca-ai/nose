use super::*;
use nose_il::UnitKind;

mod swift_flat_map;

use swift_flat_map::swift_flat_map_coordinates_proven;

#[derive(Clone, Copy)]
pub(super) enum CallbackCoordinate {
    EffectClosed,
    UnaryValue,
}

#[derive(Clone, Copy)]
pub(super) enum CallbackObligation {
    Transform(CallbackCoordinate),
    Predicate(CallbackCoordinate),
    FilterMap(CallbackCoordinate),
    FlatMap(CallbackCoordinate),
}

impl CallbackObligation {
    fn coordinate(self) -> CallbackCoordinate {
        match self {
            Self::Transform(coordinate)
            | Self::Predicate(coordinate)
            | Self::FilterMap(coordinate)
            | Self::FlatMap(coordinate) => coordinate,
        }
    }

    fn requires_proven_operator_dispatch(self) -> bool {
        matches!(self, Self::Transform(_) | Self::FlatMap(_))
    }

    fn is_pure_transform(self) -> bool {
        matches!(self, Self::Transform(_) | Self::FlatMap(_))
    }

    fn is_pure_filter_map(self) -> bool {
        matches!(self, Self::FilterMap(_))
    }

    fn is_pure_flat_map(self) -> bool {
        matches!(self, Self::FlatMap(_))
    }

    /// Whether a successful callback check under `self` already proves every
    /// node-level effect condition imposed by `required`.
    ///
    /// Pure transforms add operator-dispatch and Swift value-surface gates on
    /// top of the predicate perimeter; coordinate arity is checked by the
    /// nested contract admission itself. FlatMap source/depth coordinates are
    /// also checked by that admission before this effect-only relation is used.
    /// Avoiding an identical/weaker second tree walk keeps deeply nested HOF
    /// chains from recursively rechecking the same callback subtree.
    fn subsumes(self, required: Self) -> bool {
        matches!(
            (self, required),
            (
                Self::Transform(_),
                Self::Transform(_) | Self::FlatMap(_) | Self::Predicate(_)
            ) | (
                Self::FlatMap(_),
                Self::Transform(_) | Self::FlatMap(_) | Self::Predicate(_)
            ) | (Self::Predicate(_), Self::Predicate(_))
                | (Self::FilterMap(_), Self::FilterMap(_) | Self::Predicate(_),)
        )
    }
}

pub(super) fn library_api_callback_obligation(
    lang: Lang,
    id: LibraryApiContractId,
) -> Option<CallbackObligation> {
    let LibraryApiContractId::MethodCall(semantic) = id else {
        return None;
    };
    match semantic {
        MethodSemanticContract::HoF(kind) => transform_callback_obligation(lang, kind),
        MethodSemanticContract::Builtin(builtin) => {
            predicate_callback_coordinate(lang, builtin).map(CallbackObligation::Predicate)
        }
    }
}

fn transform_callback_coordinate(lang: Lang, kind: HoFKind) -> Option<CallbackCoordinate> {
    let supported = match lang {
        Lang::Ruby => matches!(kind, HoFKind::Map | HoFKind::Filter | HoFKind::Reject),
        Lang::Swift => matches!(
            kind,
            HoFKind::Map | HoFKind::Filter | HoFKind::FilterMap | HoFKind::FlatMap
        ),
        _ if js_like_lang(lang) => {
            matches!(kind, HoFKind::Map | HoFKind::Filter | HoFKind::FlatMap)
        }
        _ => false,
    };
    supported.then_some(CallbackCoordinate::UnaryValue)
}

fn transform_callback_obligation(lang: Lang, kind: HoFKind) -> Option<CallbackObligation> {
    let coordinate = transform_callback_coordinate(lang, kind)?;
    Some(match kind {
        HoFKind::FlatMap if lang == Lang::Swift => CallbackObligation::FlatMap(coordinate),
        HoFKind::Map | HoFKind::FlatMap => CallbackObligation::Transform(coordinate),
        HoFKind::Filter | HoFKind::Reject => CallbackObligation::Predicate(coordinate),
        HoFKind::FilterMap if lang == Lang::Swift => CallbackObligation::FilterMap(coordinate),
        HoFKind::Reduce | HoFKind::FilterMap => return None,
    })
}

fn predicate_callback_coordinate(lang: Lang, builtin: Builtin) -> Option<CallbackCoordinate> {
    match lang {
        Lang::Ruby if matches!(builtin, Builtin::Any | Builtin::All) => {
            Some(CallbackCoordinate::UnaryValue)
        }
        Lang::Swift if builtin == Builtin::All => Some(CallbackCoordinate::UnaryValue),
        Lang::Rust if matches!(builtin, Builtin::Any | Builtin::All) => {
            Some(CallbackCoordinate::EffectClosed)
        }
        _ if js_like_lang(lang) && matches!(builtin, Builtin::Any | Builtin::All) => {
            Some(CallbackCoordinate::UnaryValue)
        }
        _ => None,
    }
}

pub(super) fn library_api_callback_obligation_matches_node(
    il: &Il,
    interner: Option<&Interner>,
    node: NodeId,
    obligation: CallbackObligation,
) -> bool {
    let Some(&callback) = il.children(node).get(1) else {
        return false;
    };
    if matches!(obligation.coordinate(), CallbackCoordinate::UnaryValue)
        && !callback_has_single_value_param(il, callback)
    {
        return false;
    }
    if obligation.is_pure_filter_map() {
        let Some(interner) = interner else {
            return false;
        };
        if !swift_compact_map_has_direct_parameter_source(il, node)
            || swift_compact_map_dispatch_ambiguous_in_file(il, interner)
            || swift_nil_literal_conformance_in_file(il, interner)
            || !callback_filter_map_coordinates_proven(il, callback)
        {
            return false;
        }
    }
    if obligation.is_pure_flat_map() {
        let Some(interner) = interner else {
            return false;
        };
        if !swift_flat_map_coordinates_proven(il, interner, node, callback) {
            return false;
        }
    }
    callback_effect_closed(il, interner, callback, obligation)
}

fn callback_has_single_value_param(il: &Il, callback: NodeId) -> bool {
    callback_single_value_param(il, callback).is_some()
}

fn callback_single_value_param(il: &Il, callback: NodeId) -> Option<NodeId> {
    if !matches!(il.kind(callback), NodeKind::Func | NodeKind::Lambda) {
        return None;
    }
    let mut params = il
        .children(callback)
        .iter()
        .copied()
        .filter(|&child| il.kind(child) == NodeKind::Param);
    let param = params.next()?;
    (params.next().is_none() && il.children(param).is_empty()).then_some(param)
}

fn swift_compact_map_has_direct_parameter_source(il: &Il, node: NodeId) -> bool {
    let source = match il.kind(node) {
        NodeKind::Call => {
            let callee = il.children(node).first().copied();
            callee
                .filter(|&callee| il.kind(callee) == NodeKind::Field)
                .and_then(|callee| il.children(callee).first().copied())
        }
        NodeKind::HoF => il.children(node).first().copied(),
        _ => None,
    };
    source
        .and_then(|source| swift_compact_map_direct_function_param(il, source))
        .is_some_and(|param| swift_bracket_array_parameter_proven(il, param))
}

fn swift_compact_map_direct_function_param(il: &Il, source: NodeId) -> Option<NodeId> {
    swift_direct_parameter_source(il, source)
        .filter(|(scope, _)| il.kind(*scope) == NodeKind::Func)
        .map(|(_, param)| param)
}

fn swift_direct_parameter_source(il: &Il, source: NodeId) -> Option<(NodeId, NodeId)> {
    if il.kind(source) != NodeKind::Var {
        return None;
    }
    match il.node(source).payload {
        Payload::Name(name) => nearest_named_param_scope(il, source, name),
        Payload::Cid(cid) => {
            let span = receiver_cid_param_span(il, source, cid)?;
            let mut params = il.nodes_spanning(span).filter(|&param| {
                il.node(param).span == span
                    && il.kind(param) == NodeKind::Param
                    && matches!(il.node(param).payload, Payload::Cid(param_cid) if param_cid == cid)
            });
            let param = params.next()?;
            if params.next().is_some() {
                return None;
            }
            il.nearest_scope(param).map(|scope| (scope, param))
        }
        _ => None,
    }
}

fn swift_compact_map_dispatch_ambiguous_in_file(il: &Il, interner: &Interner) -> bool {
    il.units.iter().any(|unit| {
        matches!(unit.kind, UnitKind::Function | UnitKind::Method)
            && unit.name.is_some_and(|name| {
                ["compactMap", "filter", "map"]
                    .into_iter()
                    .any(|expected| swift_identifier_matches(interner.resolve(name), expected))
            })
    }) || il.nodes.iter().any(|node| {
        matches!(node.payload, Payload::Name(name) if interner.resolve(name)
            == SWIFT_COMPACT_MAP_DISPATCH_BARRIER_MARKER)
    }) || swift_has_unproven_collection_parameter(il)
}

fn swift_nil_literal_conformance_in_file(il: &Il, interner: &Interner) -> bool {
    il.nodes.iter().any(|node| {
        matches!(node.payload, Payload::Name(name) if matches!(
            interner.resolve(name),
            SWIFT_NIL_LITERAL_CONFORMANCE_MARKER | SWIFT_NIL_LITERAL_PROOF_BARRIER_MARKER
        ))
    })
}

/// Prove the controlled Swift `compactMap` callback perimeter: one branch is
/// exactly Optional absence and the other re-emits the same callback binding
/// that controls the condition. Swift requires that condition binding to be
/// `Bool`, which makes the contextual `nil` Optional absence without trusting
/// nominal type text. Captured/other Vars stay closed because they may be
/// Optional or conform to `ExpressibleByNilLiteral`.
fn callback_filter_map_coordinates_proven(il: &Il, callback: NodeId) -> bool {
    let Some(param) = callback_single_value_param(il, callback) else {
        return false;
    };
    let Some(output) = callback_single_output(il, callback) else {
        return false;
    };
    let [condition, then_branch, else_branch] = il.children(output) else {
        return false;
    };
    let Some(condition) = callback_single_output(il, *condition) else {
        return false;
    };
    if il.kind(output) != NodeKind::If || il.kind(condition) != NodeKind::Var {
        return false;
    }
    let emitted = match (
        callback_filter_map_branch(il, *then_branch),
        callback_filter_map_branch(il, *else_branch),
    ) {
        (Some(FilterMapBranch::Emit(emitted)), Some(FilterMapBranch::Drop))
        | (Some(FilterMapBranch::Drop), Some(FilterMapBranch::Emit(emitted))) => emitted,
        _ => return false,
    };
    var_references_same_binding(il, condition, emitted)
        && var_references_param_binding(il, condition, param)
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum FilterMapBranch {
    Emit(NodeId),
    Drop,
}

fn callback_filter_map_branch(il: &Il, branch: NodeId) -> Option<FilterMapBranch> {
    let output = callback_single_output(il, branch)?;
    match (il.kind(output), il.node(output).payload) {
        (NodeKind::Lit, Payload::Lit(LitClass::Null)) => Some(FilterMapBranch::Drop),
        (NodeKind::Var, _) => Some(FilterMapBranch::Emit(output)),
        _ => None,
    }
}

fn callback_single_output(il: &Il, root: NodeId) -> Option<NodeId> {
    let mut node = root;
    loop {
        match il.kind(node) {
            NodeKind::Func | NodeKind::Lambda => {
                let mut outputs = il
                    .children(node)
                    .iter()
                    .copied()
                    .filter(|&child| il.kind(child) != NodeKind::Param);
                node = outputs.next()?;
                if outputs.next().is_some() {
                    return None;
                }
            }
            NodeKind::Block => {
                let [only] = il.children(node) else {
                    return None;
                };
                node = *only;
            }
            NodeKind::Return | NodeKind::ExprStmt => {
                let [value] = il.children(node) else {
                    return None;
                };
                node = *value;
            }
            _ => return Some(node),
        }
    }
}

fn callback_effect_closed(
    il: &Il,
    interner: Option<&Interner>,
    callback: NodeId,
    obligation: CallbackObligation,
) -> bool {
    if !matches!(il.kind(callback), NodeKind::Func | NodeKind::Lambda) {
        return false;
    }
    let mut stack = vec![callback];
    while let Some(node) = stack.pop() {
        if il.kind(node) == NodeKind::Call {
            let Some(nested) = nested_callback_call_eager_children(il, interner, node) else {
                return false;
            };
            stack.push(nested.receiver);
            if !nested.obligation.subsumes(obligation) {
                stack.push(nested.callback);
            }
            continue;
        }
        if il.kind(node) == NodeKind::HoF {
            if library_api_dependency_id_for_normalized_hof(il, interner, node).is_none() {
                return false;
            }
            let Some(&source) = il.children(node).first() else {
                return false;
            };
            stack.push(source);
            let Some(&nested_callback) = il.children(node).get(1) else {
                return false;
            };
            let own_obligation = match il.node(node).payload {
                Payload::HoF(kind) => transform_callback_obligation(il.meta.lang, kind),
                _ => None,
            };
            if !own_obligation.is_some_and(|own| own.subsumes(obligation)) {
                stack.push(nested_callback);
            }
            continue;
        }
        if !callback_node_effect_closed(il, interner, node, obligation) {
            return false;
        }
        stack.extend(il.children(node).iter().copied());
    }
    true
}

fn callback_node_effect_closed(
    il: &Il,
    interner: Option<&Interner>,
    node: NodeId,
    obligation: CallbackObligation,
) -> bool {
    let kind = il.kind(node);
    let runtime_definition = il
        .units
        .iter()
        .any(|unit| unit.root == node && matches!(unit.kind, UnitKind::Method | UnitKind::Class));
    if runtime_definition || (il.meta.lang == Lang::Ruby && kind == NodeKind::Return) {
        return false;
    }
    if kind == NodeKind::Var {
        if js_like_lang(il.meta.lang)
            && matches!(il.node(node).payload, Payload::Name(name) if interner
                .is_some_and(|interner| matches!(interner.resolve(name), "arguments" | "this" | "super")))
        {
            return false;
        }
        if !effect_closed_local_var_reference(il, node) {
            return false;
        }
    }
    if obligation.is_pure_transform() && il.meta.lang == Lang::Swift && kind == NodeKind::Lit {
        return false;
    }
    if obligation.requires_proven_operator_dispatch()
        && matches!(kind, NodeKind::BinOp | NodeKind::UnOp)
        && !semantics(il.meta.lang)
            .operators()
            .pure_transform_operator_effect_closed(il, interner, node)
    {
        return false;
    }
    if kind == NodeKind::Seq {
        let literal_kind = admitted_sequence_surface_kind_at_node(il, node);
        let admitted_literal = match literal_kind {
            Some(SequenceSurfaceKind::Tuple) => true,
            Some(SequenceSurfaceKind::Collection) => {
                !(obligation.is_pure_transform() && il.meta.lang == Lang::Swift)
            }
            _ => false,
        };
        if !admitted_literal
            || interner
                .is_some_and(|interner| seq_surface_contract_for_node(il, interner, node).is_none())
        {
            return false;
        }
    }
    matches!(
        kind,
        NodeKind::Func
            | NodeKind::Lambda
            | NodeKind::Param
            | NodeKind::Block
            | NodeKind::ExprStmt
            | NodeKind::Return
            | NodeKind::If
            | NodeKind::Var
            | NodeKind::Lit
            | NodeKind::BinOp
            | NodeKind::UnOp
            | NodeKind::Seq
    )
}

struct NestedCallbackCall {
    receiver: NodeId,
    callback: NodeId,
    obligation: CallbackObligation,
}

fn nested_callback_call_eager_children(
    il: &Il,
    interner: Option<&Interner>,
    call: NodeId,
) -> Option<NestedCallbackCall> {
    let interner = interner?;
    let &callee = il.children(call).first()?;
    let NodeKind::Field = il.kind(callee) else {
        return None;
    };
    let Payload::Name(method) = il.node(callee).payload else {
        return None;
    };
    let &receiver = il.children(callee).first()?;
    let &nested_callback = il.children(call).get(1)?;
    let arg_count = il.children(call).len().saturating_sub(1);
    let obligation =
        library_method_call_contracts(il.meta.lang, interner.resolve(method), arg_count)
            .into_iter()
            .filter(|contract| nested_callback_hof_method_call(il.meta.lang, contract.result))
            .find_map(|contract| {
                matches!(
                    library_api_contract_evidence_for_call(
                        il,
                        interner,
                        call,
                        contract.id,
                        contract.callee,
                        arg_count,
                    ),
                    LibraryApiEvidenceStatus::Admitted
                )
                .then(|| library_api_callback_obligation(il.meta.lang, contract.id))
                .flatten()
            });
    Some(NestedCallbackCall {
        receiver,
        callback: nested_callback,
        obligation: obligation?,
    })
}

fn nested_callback_hof_method_call(lang: Lang, contract: MethodCallContract) -> bool {
    js_like_array_hof_method_call(lang, contract)
        || swift_sequence_hof_method_call(lang, contract)
        || ruby_sequence_hof_method_call(lang, contract)
}
