use super::*;
use nose_il::UnitKind;

#[derive(Clone, Copy)]
pub(super) enum CallbackCoordinate {
    EffectClosed,
    UnaryValue,
}

#[derive(Clone, Copy)]
pub(super) enum CallbackObligation {
    PureTransform(CallbackCoordinate),
    PurePredicate(CallbackCoordinate),
}

impl CallbackObligation {
    fn coordinate(self) -> CallbackCoordinate {
        match self {
            Self::PureTransform(coordinate) | Self::PurePredicate(coordinate) => coordinate,
        }
    }

    fn requires_proven_operator_dispatch(self) -> bool {
        matches!(self, Self::PureTransform(_))
    }

    fn is_pure_transform(self) -> bool {
        matches!(self, Self::PureTransform(_))
    }

    /// Whether a successful callback check under `self` already proves every
    /// node-level effect condition imposed by `required`.
    ///
    /// Pure transforms add operator-dispatch and Swift value-surface gates on
    /// top of the predicate perimeter; coordinate arity is checked by the
    /// nested contract admission itself. Avoiding an identical/weaker second
    /// tree walk keeps deeply nested HOF chains from recursively rechecking the
    /// same callback subtree.
    fn subsumes(self, required: Self) -> bool {
        self.is_pure_transform() || !required.is_pure_transform()
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
            predicate_callback_coordinate(lang, builtin).map(CallbackObligation::PurePredicate)
        }
    }
}

fn transform_callback_coordinate(lang: Lang, kind: HoFKind) -> Option<CallbackCoordinate> {
    let supported = match lang {
        Lang::Ruby => matches!(kind, HoFKind::Map | HoFKind::Filter | HoFKind::Reject),
        Lang::Swift => matches!(kind, HoFKind::Map | HoFKind::Filter | HoFKind::FlatMap),
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
        HoFKind::Map | HoFKind::FlatMap => CallbackObligation::PureTransform(coordinate),
        HoFKind::Filter | HoFKind::Reject => CallbackObligation::PurePredicate(coordinate),
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
    callback_effect_closed(il, interner, callback, obligation)
}

fn callback_has_single_value_param(il: &Il, callback: NodeId) -> bool {
    if !matches!(il.kind(callback), NodeKind::Func | NodeKind::Lambda) {
        return false;
    }
    let mut params = il
        .children(callback)
        .iter()
        .copied()
        .filter(|&child| il.kind(child) == NodeKind::Param);
    let Some(param) = params.next() else {
        return false;
    };
    params.next().is_none() && il.children(param).is_empty()
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
