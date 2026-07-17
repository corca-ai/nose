use super::*;

/// `(receiver, value)` of a single-item append-like builder call admitted by first-party
/// language/library contracts.
///
/// Raw method selectors such as `push`, `append`, or `add` are not proof by themselves;
/// callers that see those selectors must first prove the receiver/builder contract, lower
/// the call to the canonical builtin, and attach append-effect evidence.
pub fn builder_append_call_args(
    il: &Il,
    _interner: &Interner,
    node: NodeId,
) -> Option<(NodeId, NodeId)> {
    match exact_effect_evidence_for_node(il, node) {
        EvidenceResolution::Found(EffectEvidenceKind::BuilderAppendCall) => {
            syntactic_append_call_args(il, node)
        }
        EvidenceResolution::Found(_) | EvidenceResolution::Ambiguous => None,
        EvidenceResolution::Missing => None,
    }
}

/// `(receiver, value)` for any append occurrence whose semantics are independently admitted.
pub fn admitted_builder_append_call_args(
    il: &Il,
    interner: &Interner,
    node: NodeId,
) -> Option<(NodeId, NodeId)> {
    if let Some(parts) = builder_append_call_args(il, interner, node) {
        return Some(parts);
    }
    if crate::admitted_builtin_semantics_at_call(il, node, Builtin::Append) {
        return canonical_append_call_args(il, node);
    }
    admitted_builder_append_method_call_args(il, interner, node)
}

/// `(receiver, value)` of Ruby's shovel form `recv << item`.
pub fn ruby_shovel_append_parts(il: &Il, node: NodeId) -> Option<(NodeId, NodeId)> {
    if il.meta.lang != Lang::Ruby
        || il.kind(node) != NodeKind::BinOp
        || !matches!(il.node(node).payload, Payload::Op(Op::Shl))
    {
        return None;
    }
    let [recv, item] = il.children(node) else {
        return None;
    };
    Some((*recv, *item))
}

/// `(receiver, value)` of a source method call whose append meaning is proven by
/// same-span `LibraryApi(MethodCall(Builtin(Append)))` occurrence evidence.
pub fn admitted_builder_append_method_call_args(
    il: &Il,
    interner: &Interner,
    node: NodeId,
) -> Option<(NodeId, NodeId)> {
    if il.kind(node) != NodeKind::Call || !matches!(il.node(node).payload, Payload::None) {
        return None;
    }
    let [_callee, item] = il.children(node) else {
        return None;
    };
    let admitted = admitted_library_method_call_at_call(il, interner, node)?;
    let receiver = admitted.receiver?;
    let LibraryApiCalleeContract::Method { method, .. } = admitted.contract.callee else {
        return None;
    };
    let effect = builder_append_method_contract(il.meta.lang, method, admitted.arg_count)?;
    if effect.effect != EffectEvidenceKind::BuilderAppendCall
        || effect.receiver != MethodEffectReceiverContract::ActiveCollectionBuilder
    {
        return None;
    }
    if admitted.contract.result.semantic != MethodSemanticContract::Builtin(Builtin::Append)
        || admitted.contract.result.args != MethodBuiltinArgs::ReceiverThenAll
    {
        return None;
    }
    Some((receiver, *item))
}

fn canonical_append_call_args(il: &Il, node: NodeId) -> Option<(NodeId, NodeId)> {
    if il.kind(node) != NodeKind::Call {
        return None;
    }
    let kids = il.children(node);
    if matches!(il.node(node).payload, Payload::Builtin(Builtin::Append)) {
        return (kids.len() == 2).then(|| (kids[0], kids[1]));
    }
    None
}

fn syntactic_append_call_args(il: &Il, node: NodeId) -> Option<(NodeId, NodeId)> {
    if let Some(parts) = canonical_append_call_args(il, node) {
        return Some(parts);
    }
    if il.kind(node) != NodeKind::Call {
        return None;
    }
    let kids = il.children(node);
    if kids.len() != 2 || il.kind(kids[0]) != NodeKind::Field {
        return None;
    }
    let receiver = il.children(kids[0]).first().copied()?;
    Some((receiver, kids[1]))
}

pub fn builder_append_call(il: &Il, interner: &Interner, node: NodeId) -> bool {
    builder_append_call_args(il, interner, node).is_some()
}
