//! Cross-language idiom canonicalization: collapse equivalent builtins from
//! different languages to one [`Builtin`] op so that, e.g., Python `len(xs)`,
//! JS `xs.length`, and Go `len(xs)` all converge. Detection on `xs.length` (a
//! field access) is handled by the caller; this module only inspects `Call`s.
//!
//! proof-obligation: normalize.value_graph.bool_reduce
//! proof-obligation: normalize.value_graph.functor
//! proof-obligation: normalize.value_graph.min_max

use nose_il::{contains_js_identifier, Builtin, HoFKind, Il, Interner, NodeId, NodeKind, Payload};
use nose_semantics::{
    domain_evidence_for_param, free_function_builtin_contract, imported_binding_symbol,
    imported_namespace_symbol, iterator_identity_adapter_contract, map_get_contract,
    map_key_view_contract, method_call_contract, method_hof_contract,
    rust_option_some_constructor_contract, seq_surface_contract_for_node,
    static_collection_adapter_contract, unshadowed_global_symbol, BuiltinArgContract,
    DomainEvidence, MapKeyViewKind, MethodBuiltinArgs, MethodCallContract, MethodReceiverContract,
    MethodSemanticContract, SEQ_VALUE_MAP,
};

/// The result of inspecting a `Call`: it canonicalizes to a builtin, to a
/// higher-order op (`HoF`), or stays an ordinary call. The carried `NodeId`s are
/// *old* ids to be rebuilt as the new node's children.
pub(crate) enum CallCanon {
    Builtin {
        op: Builtin,
        arg_olds: Vec<NodeId>,
    },
    /// `xs.map(f)` / `.flatMap(f)` / `.filter(f)` / `.reduce(f)` → `HoF[xs, f]`,
    /// converging with Python comprehensions.
    HoF {
        kind: HoFKind,
        collection_old: NodeId,
        fn_old: NodeId,
    },
    None,
}

pub(crate) fn canon_call(old: &Il, interner: &Interner, call_id: NodeId) -> CallCanon {
    let kids = old.children(call_id);
    if kids.is_empty() {
        return CallCanon::None;
    }
    let callee = kids[0];
    let args = &kids[1..];
    // Every plain builtin recognition returns the same shape — a `CallCanon::Builtin`
    // carrying either the single first argument or all arguments. `builtin!(Op, first)` and
    // `builtin!(Op, all)` name those two cases so the recognition arms stay one line each.
    macro_rules! builtin {
        ($op:expr, first) => {
            return CallCanon::Builtin {
                op: $op,
                arg_olds: vec![args[0]],
            }
        };
        ($op:expr, all) => {
            return CallCanon::Builtin {
                op: $op,
                arg_olds: args.to_vec(),
            }
        };
    }
    let cn = old.node(callee);
    match cn.kind {
        NodeKind::Var => {
            if let Payload::Name(s) = cn.payload {
                if let Some(contract) =
                    free_function_builtin_contract(old.meta.lang, interner.resolve(s), args.len())
                {
                    if contract.requires_unshadowed
                        && file_defines_name(old, interner, interner.resolve(s))
                    {
                        return CallCanon::None;
                    }
                    match contract.args {
                        BuiltinArgContract::First => builtin!(contract.builtin, first),
                        BuiltinArgContract::All => builtin!(contract.builtin, all),
                    }
                }
            }
        }
        NodeKind::Field => {
            if let Payload::Name(s) = cn.payload {
                let fname = interner.resolve(s);
                let base = old.children(callee).first().copied();
                if let Some(contract) = method_call_contract(old.meta.lang, fname, args.len()) {
                    let Some(base) = base else {
                        return CallCanon::None;
                    };
                    let Some(proven) =
                        prove_method_receiver(old, interner, contract.receiver, base, args)
                    else {
                        return CallCanon::None;
                    };
                    return apply_method_contract(old, interner, contract, proven, args);
                }
            }
        }
        _ => {}
    }
    CallCanon::None
}

#[derive(Clone, Copy)]
enum ProvenReceiver {
    Direct(NodeId),
    MapGet { map: NodeId, key: NodeId },
}

fn prove_method_receiver(
    old: &Il,
    interner: &Interner,
    contract: MethodReceiverContract,
    base: NodeId,
    args: &[NodeId],
) -> Option<ProvenReceiver> {
    match contract {
        MethodReceiverContract::ExactCollection => {
            exact_collection_receiver(old, interner, base).then_some(ProvenReceiver::Direct(base))
        }
        MethodReceiverContract::ExactProtocol => {
            exact_protocol_receiver(old, interner, base).then_some(ProvenReceiver::Direct(base))
        }
        MethodReceiverContract::ExactProtocolPairArgument => {
            (exact_protocol_receiver(old, interner, base)
                && args
                    .first()
                    .is_some_and(|&arg| exact_protocol_receiver(old, interner, arg)))
            .then_some(ProvenReceiver::Direct(base))
        }
        MethodReceiverContract::ExactOption => {
            exact_option_receiver(old, interner, base).then_some(ProvenReceiver::Direct(base))
        }
        MethodReceiverContract::ExactString => {
            exact_string_receiver(old, base).then_some(ProvenReceiver::Direct(base))
        }
        MethodReceiverContract::ExactInteger => {
            exact_integer_receiver(old, base).then_some(ProvenReceiver::Direct(base))
        }
        MethodReceiverContract::ExactMap => {
            exact_map_receiver(old, interner, base).then_some(ProvenReceiver::Direct(base))
        }
        MethodReceiverContract::ExactMapLiteral => {
            map_like_literal(old, interner, base).then_some(ProvenReceiver::Direct(base))
        }
        MethodReceiverContract::ExactCollectionOrMap => {
            (exact_collection_receiver(old, interner, base)
                || exact_map_receiver(old, interner, base))
            .then_some(ProvenReceiver::Direct(base))
        }
        MethodReceiverContract::ExactCollectionOrMapLiteral => {
            (exact_collection_receiver(old, interner, base)
                || map_like_literal(old, interner, base))
            .then_some(ProvenReceiver::Direct(base))
        }
        MethodReceiverContract::ExactCollectionOrJavaKeySet => {
            if exact_collection_receiver(old, interner, base) {
                return Some(ProvenReceiver::Direct(base));
            }
            if old.meta.lang == nose_il::Lang::Java {
                let map = key_set_receiver(old, interner, base)?;
                return map_like_literal(old, interner, map).then_some(ProvenReceiver::Direct(map));
            }
            None
        }
        MethodReceiverContract::ExactSetOrMap => (exact_set_param(old, base)
            || exact_map_param(old, base))
        .then_some(ProvenReceiver::Direct(base)),
        MethodReceiverContract::LiteralString => {
            literal_string_receiver(old, base).then_some(ProvenReceiver::Direct(base))
        }
        MethodReceiverContract::UnshadowedGlobal(global) => {
            unshadowed_global_symbol(old, interner, base, global)
                .then_some(ProvenReceiver::Direct(base))
        }
        MethodReceiverContract::ImportedNamespace(module) => {
            imported_namespace_symbol(old, interner, base, module)
                .then_some(ProvenReceiver::Direct(base))
        }
        MethodReceiverContract::RustMapGetOrExactOption => {
            if let Some((map, key)) = proven_map_get_call_parts(old, interner, base) {
                Some(ProvenReceiver::MapGet { map, key })
            } else {
                exact_option_receiver(old, interner, base).then_some(ProvenReceiver::Direct(base))
            }
        }
    }
}

fn apply_method_contract(
    old: &Il,
    interner: &Interner,
    contract: MethodCallContract,
    receiver: ProvenReceiver,
    args: &[NodeId],
) -> CallCanon {
    let op = match contract.semantic {
        MethodSemanticContract::Builtin(op) => op,
        MethodSemanticContract::HoF(kind) => {
            let ProvenReceiver::Direct(base) = receiver else {
                return CallCanon::None;
            };
            return CallCanon::HoF {
                kind,
                collection_old: unwrap_iter(old, interner, base),
                fn_old: args[0],
            };
        }
    };

    let direct = match receiver {
        ProvenReceiver::Direct(node) => Some(node),
        ProvenReceiver::MapGet { .. } => None,
    };

    match contract.args {
        MethodBuiltinArgs::All => CallCanon::Builtin {
            op,
            arg_olds: args.to_vec(),
        },
        MethodBuiltinArgs::First => CallCanon::Builtin {
            op,
            arg_olds: vec![args[0]],
        },
        MethodBuiltinArgs::ReceiverOnly => {
            if let ProvenReceiver::MapGet { map, key } = receiver {
                if op == Builtin::IsNotNull {
                    return CallCanon::Builtin {
                        op: Builtin::Contains,
                        arg_olds: vec![key, map],
                    };
                }
                return CallCanon::None;
            }
            CallCanon::Builtin {
                op,
                arg_olds: vec![direct.unwrap()],
            }
        }
        MethodBuiltinArgs::ReceiverThenAll => {
            let Some(base) = direct else {
                return CallCanon::None;
            };
            let mut a = Vec::with_capacity(args.len() + 1);
            a.push(base);
            a.extend_from_slice(args);
            CallCanon::Builtin { op, arg_olds: a }
        }
        MethodBuiltinArgs::ReceiverAndFirst => {
            let Some(base) = direct else {
                return CallCanon::None;
            };
            CallCanon::Builtin {
                op,
                arg_olds: vec![base, args[0]],
            }
        }
        MethodBuiltinArgs::FirstThenReceiver => {
            let Some(base) = direct else {
                return CallCanon::None;
            };
            CallCanon::Builtin {
                op,
                arg_olds: vec![args[0], base],
            }
        }
        MethodBuiltinArgs::GoSliceContains => CallCanon::Builtin {
            op,
            arg_olds: vec![args[1], args[0]],
        },
        MethodBuiltinArgs::MapGetDefault => {
            let Some(base) = direct else {
                return CallCanon::None;
            };
            CallCanon::Builtin {
                op,
                arg_olds: vec![base, args[0], args[1]],
            }
        }
        MethodBuiltinArgs::MapGetDefaultOrZeroArgLambda => {
            let Some(base) = direct else {
                return CallCanon::None;
            };
            let fallback = if old.kind(args[1]) == NodeKind::Lambda {
                let Some(fallback) = zero_arg_lambda_body_value(old, args[1]) else {
                    return CallCanon::None;
                };
                fallback
            } else {
                args[1]
            };
            CallCanon::Builtin {
                op,
                arg_olds: vec![base, args[0], fallback],
            }
        }
        MethodBuiltinArgs::RustMapGetOrOptionDefault => match receiver {
            ProvenReceiver::MapGet { map, key } => CallCanon::Builtin {
                op: Builtin::GetOrDefault,
                arg_olds: vec![map, key, args[0]],
            },
            ProvenReceiver::Direct(base) => CallCanon::Builtin {
                op,
                arg_olds: vec![base, args[0]],
            },
        },
        MethodBuiltinArgs::RustOptionDefaultLambda => {
            let Some(base) = direct else {
                return CallCanon::None;
            };
            if let Some(fallback) = zero_arg_lambda_body(old, args[0]) {
                return CallCanon::Builtin {
                    op,
                    arg_olds: vec![base, fallback],
                };
            }
            CallCanon::None
        }
        MethodBuiltinArgs::RustOptionMapOrIdentity => {
            let Some(base) = direct else {
                return CallCanon::None;
            };
            if identity_lambda(old, args[1]) {
                return CallCanon::Builtin {
                    op,
                    arg_olds: vec![base, args[0]],
                };
            }
            CallCanon::None
        }
        MethodBuiltinArgs::RustZip => {
            let Some(base) = direct else {
                return CallCanon::None;
            };
            CallCanon::Builtin {
                op,
                arg_olds: vec![
                    unwrap_iter(old, interner, base),
                    unwrap_iter(old, interner, args[0]),
                ],
            }
        }
        MethodBuiltinArgs::Fold => {
            let Some(base) = direct else {
                return CallCanon::None;
            };
            let (fn_old, init_old) = fold_fn_init(old, args);
            let Some(fn_old) = fn_old else {
                return CallCanon::None;
            };
            let coll = unwrap_iter(old, interner, base);
            let mut a = vec![fn_old, coll];
            if let Some(init) = init_old {
                a.push(init);
            }
            CallCanon::Builtin { op, arg_olds: a }
        }
        MethodBuiltinArgs::BoolReduction => {
            let Some(base) = direct else {
                return CallCanon::None;
            };
            CallCanon::Builtin {
                op,
                arg_olds: vec![unwrap_iter(old, interner, base), args[0]],
            }
        }
        MethodBuiltinArgs::Hof => CallCanon::None,
        MethodBuiltinArgs::CollectionReduction => {
            let Some(base) = direct else {
                return CallCanon::None;
            };
            if matches!(op, Builtin::Min | Builtin::Max) && old.kind(base) == NodeKind::Seq {
                let items = old.children(base);
                if items.len() == 2 {
                    return CallCanon::Builtin {
                        op,
                        arg_olds: items.to_vec(),
                    };
                }
            }
            CallCanon::Builtin {
                op,
                arg_olds: vec![unwrap_iter(old, interner, base)],
            }
        }
    }
}

/// Classify a fold's args into `(fn, init)`: the lambda is the function (whatever its
/// position — JS puts it first, Ruby/Rust last), the other arg (if any) is the seed.
fn fold_fn_init(old: &Il, args: &[NodeId]) -> (Option<NodeId>, Option<NodeId>) {
    let mut fn_old = None;
    let mut init = None;
    for &a in args {
        if old.kind(a) == NodeKind::Lambda {
            fn_old = Some(a);
        } else {
            init = Some(a);
        }
    }
    (fn_old, init)
}

/// Peel a first-party identity iterator adapter to the underlying collection, so
/// `xs.iter().fold(..)` / `xs.stream().map(..)` iterate over `xs`. NOT `.keys()`/`.values()`,
/// which change *what* is iterated.
fn unwrap_iter(old: &Il, interner: &Interner, node: NodeId) -> NodeId {
    if old.kind(node) == NodeKind::Call {
        let call_kids = old.children(node);
        if let Some(&callee) = call_kids.first() {
            if old.kind(callee) == NodeKind::Field {
                if let Payload::Name(s) = old.node(callee).payload {
                    let method = interner.resolve(s);
                    if let Some(&base) = old.children(callee).first() {
                        if let Some(arg) =
                            static_collection_adapter_arg(old, interner, base, method, call_kids)
                        {
                            return arg;
                        }
                    }
                    if iterator_identity_adapter_contract(
                        old.meta.lang,
                        method,
                        call_kids.len() - 1,
                    )
                    .is_some()
                    {
                        if let Some(&base) = old.children(callee).first() {
                            return unwrap_iter(old, interner, base);
                        }
                    }
                }
            }
        }
    }
    node
}

fn exact_collection_receiver(old: &Il, interner: &Interner, node: NodeId) -> bool {
    exact_protocol_receiver(old, interner, node)
}

fn exact_protocol_receiver(old: &Il, interner: &Interner, node: NodeId) -> bool {
    if exact_collection_literal(old, interner, node) || exact_collection_param(old, node) {
        return true;
    }
    if old.kind(node) != NodeKind::Call {
        return false;
    }
    let kids = old.children(node);
    let Some(&callee) = kids.first() else {
        return false;
    };
    if old.kind(callee) != NodeKind::Field {
        return false;
    }
    let Payload::Name(method) = old.node(callee).payload else {
        return false;
    };
    let method = interner.resolve(method);
    let Some(&receiver) = old.children(callee).first() else {
        return false;
    };
    if let Some(arg) = static_collection_adapter_arg(old, interner, receiver, method, kids) {
        return exact_collection_receiver(old, interner, arg);
    }
    if let Some(contract) = method_call_contract(old.meta.lang, method, kids.len() - 1) {
        if contract.semantic == MethodSemanticContract::Builtin(Builtin::Zip)
            && contract.receiver == MethodReceiverContract::ExactProtocolPairArgument
            && contract.args == MethodBuiltinArgs::RustZip
        {
            return exact_protocol_receiver(old, interner, receiver)
                && exact_protocol_receiver(old, interner, kids[1]);
        }
    }
    if iterator_identity_adapter_contract(old.meta.lang, method, kids.len() - 1).is_some() {
        return exact_protocol_receiver(old, interner, receiver);
    }
    if method_hof_contract(old.meta.lang, method).is_some() && kids.len() >= 2 {
        return exact_protocol_receiver(old, interner, receiver);
    }
    false
}

fn exact_collection_param(old: &Il, node: NodeId) -> bool {
    domain_evidence_for_var(old, node).is_some_and(DomainEvidence::is_array_collection_or_set)
}

fn static_collection_adapter_arg(
    old: &Il,
    interner: &Interner,
    receiver: NodeId,
    method: &str,
    call_kids: &[NodeId],
) -> Option<NodeId> {
    let receiver_name = name_of(old, interner, receiver)?;
    let contract = static_collection_adapter_contract(
        old.meta.lang,
        receiver_name,
        method,
        call_kids.len().saturating_sub(1),
    )?;
    if file_defines_type_name(old, interner, receiver_name)
        || !imported_binding_symbol(old, interner, receiver, contract.module, contract.exported)
    {
        return None;
    }
    call_kids.get(1).copied()
}

fn exact_set_param(old: &Il, node: NodeId) -> bool {
    domain_evidence_for_var(old, node).is_some_and(DomainEvidence::is_set)
}

fn exact_map_param(old: &Il, node: NodeId) -> bool {
    domain_evidence_for_var(old, node).is_some_and(DomainEvidence::is_map)
}

fn exact_map_receiver(old: &Il, interner: &Interner, node: NodeId) -> bool {
    exact_map_param(old, node) || map_like_literal(old, interner, node)
}

fn exact_option_param(old: &Il, node: NodeId) -> bool {
    domain_evidence_for_var(old, node).is_some_and(DomainEvidence::is_option)
}

fn domain_evidence_for_var(old: &Il, node: NodeId) -> Option<DomainEvidence> {
    if old.kind(node) != NodeKind::Var {
        return None;
    }
    let param_span = match old.node(node).payload {
        Payload::Cid(cid) => old.nodes.iter().find_map(|candidate| {
            (candidate.kind == NodeKind::Param && candidate.payload == Payload::Cid(cid))
                .then_some(candidate.span)
        }),
        Payload::Name(name) => {
            let (scope, span) = nearest_named_param_scope(old, node, name)?;
            if name_is_assigned_in_scope(old, name, scope) {
                return None;
            }
            Some(span)
        }
        _ => None,
    };
    param_span.and_then(|span| {
        old.nodes.iter().enumerate().find_map(|(idx, candidate)| {
            (candidate.kind == NodeKind::Param && candidate.span == span)
                .then(|| domain_evidence_for_param(old, NodeId(idx as u32)))
                .flatten()
        })
    })
}

fn nearest_named_param_scope(
    old: &Il,
    node: NodeId,
    name: nose_il::Symbol,
) -> Option<(NodeId, nose_il::Span)> {
    let target = old.node(node).span;
    let mut best: Option<(u32, NodeId, nose_il::Span)> = None;
    for (idx, candidate) in old.nodes.iter().enumerate() {
        if !matches!(candidate.kind, NodeKind::Func | NodeKind::Lambda) {
            continue;
        }
        if !span_contains(candidate.span, target) {
            continue;
        }
        let scope = NodeId(idx as u32);
        let Some(param_span) = old.children(scope).iter().find_map(|&child| {
            (old.kind(child) == NodeKind::Param && old.node(child).payload == Payload::Name(name))
                .then_some(old.node(child).span)
        }) else {
            continue;
        };
        let width = candidate
            .span
            .end_byte
            .saturating_sub(candidate.span.start_byte);
        if best.is_none_or(|(best_width, _, _)| width < best_width) {
            best = Some((width, scope, param_span));
        }
    }
    best.map(|(_, scope, span)| (scope, span))
}

fn name_is_assigned_in_scope(old: &Il, name: nose_il::Symbol, scope: NodeId) -> bool {
    old.nodes.iter().enumerate().any(|(idx, node)| {
        if node.kind != NodeKind::Assign {
            return false;
        }
        let id = NodeId(idx as u32);
        if nearest_scope(old, id) != Some(scope) {
            return false;
        }
        let Some(&lhs) = old.children(id).first() else {
            return false;
        };
        old.kind(lhs) == NodeKind::Var && old.node(lhs).payload == Payload::Name(name)
    })
}

fn nearest_scope(old: &Il, node: NodeId) -> Option<NodeId> {
    let target = old.node(node).span;
    let mut best: Option<(u32, NodeId)> = None;
    for (idx, candidate) in old.nodes.iter().enumerate() {
        if !matches!(candidate.kind, NodeKind::Func | NodeKind::Lambda) {
            continue;
        }
        if !span_contains(candidate.span, target) {
            continue;
        }
        let width = candidate
            .span
            .end_byte
            .saturating_sub(candidate.span.start_byte);
        if best.is_none_or(|(best_width, _)| width < best_width) {
            best = Some((width, NodeId(idx as u32)));
        }
    }
    best.map(|(_, scope)| scope)
}

fn span_contains(outer: nose_il::Span, inner: nose_il::Span) -> bool {
    outer.file == inner.file
        && outer.start_byte <= inner.start_byte
        && outer.end_byte >= inner.end_byte
}

fn exact_collection_literal(old: &Il, interner: &Interner, node: NodeId) -> bool {
    if old.kind(node) != NodeKind::Seq {
        return false;
    }
    seq_surface_contract_for_node(old, interner, node)
        .is_some_and(|contract| contract.membership_collection)
}

fn exact_option_receiver(old: &Il, interner: &Interner, node: NodeId) -> bool {
    if exact_option_param(old, node) {
        return true;
    }
    if matches!(
        old.node(node).payload,
        Payload::Lit(nose_il::LitClass::Null)
    ) {
        return true;
    }
    if old.kind(node) != NodeKind::Call {
        return false;
    }
    let kids = old.children(node);
    if kids.len() != 2 || old.kind(kids[0]) != NodeKind::Var {
        return false;
    }
    matches!(
        old.node(kids[0]).payload,
        Payload::Name(name) if rust_option_some_name(old, interner, interner.resolve(name))
    )
}

fn exact_string_receiver(old: &Il, node: NodeId) -> bool {
    matches!(
        old.node(node).payload,
        Payload::LitStr(_) | Payload::Lit(nose_il::LitClass::Str)
    ) || matches!(
        domain_evidence_for_var(old, node),
        Some(domain) if domain.is_string()
    )
}

fn literal_string_receiver(old: &Il, node: NodeId) -> bool {
    exact_string_receiver(old, node)
}

fn exact_integer_receiver(old: &Il, node: NodeId) -> bool {
    matches!(
        old.node(node).payload,
        Payload::LitInt(_) | Payload::Lit(nose_il::LitClass::Int)
    ) || matches!(
        domain_evidence_for_var(old, node),
        Some(domain) if domain.is_integer()
    )
}

fn rust_option_some_name(old: &Il, interner: &Interner, text: &str) -> bool {
    rust_option_some_constructor_contract(old.meta.lang, text)
        .is_some_and(|contract| !file_defines_name(old, interner, contract.shadow_root))
}

fn proven_map_get_call_parts(
    old: &Il,
    interner: &Interner,
    node: NodeId,
) -> Option<(NodeId, NodeId)> {
    let (map, key) = map_get_call_parts(old, interner, node)?;
    source_map_expr(old, interner, map).then_some((map, key))
}

fn source_map_expr(old: &Il, interner: &Interner, node: NodeId) -> bool {
    map_like_literal(old, interner, node) || rust_std_map_factory_call(old, interner, node)
}

fn rust_std_map_factory_call(old: &Il, interner: &Interner, node: NodeId) -> bool {
    if old.kind(node) != NodeKind::Call {
        return false;
    }
    let kids = old.children(node);
    if kids.len() != 2 || old.kind(kids[0]) != NodeKind::Var || old.kind(kids[1]) != NodeKind::Seq {
        return false;
    }
    let Payload::Name(callee) = old.node(kids[0]).payload else {
        return false;
    };
    let callee = interner.resolve(callee);
    nose_semantics::semantics(old.meta.lang)
        .collections()
        .free_name_map_factories()
        .any(|factory| {
            factory.names.contains(&callee)
                && free_name_factory_shadow_safe(old, interner, callee, false)
                && map_factory_entries_match_surface(old, interner, kids[1], factory.entry_seq_tag)
        })
}

fn free_name_factory_shadow_safe(
    old: &Il,
    interner: &Interner,
    name: &str,
    shadow_guard: bool,
) -> bool {
    if shadow_guard {
        return !file_defines_name(old, interner, name);
    }
    if old.meta.lang == nose_il::Lang::Rust && name.starts_with("std::") {
        return !file_defines_name(old, interner, "std");
    }
    true
}

fn map_factory_entries_match_surface(
    old: &Il,
    interner: &Interner,
    entries: NodeId,
    entry_seq_tag: u64,
) -> bool {
    old.children(entries).iter().all(|&entry| {
        old.kind(entry) == NodeKind::Seq
            && seq_surface_contract_for_node(old, interner, entry)
                .is_some_and(|contract| contract.value_tag == entry_seq_tag)
    })
}

/// Canonical sync name for a known async counterpart, so behaviorally-equivalent
/// sync/async twins (a frequent real Type-4 pattern, e.g. `__exit__`/`__aexit__`,
/// `read`/`aread`) converge. Curated to high-confidence pairs only — generic
/// `a`-prefixed names (`add`, `append`, `get`) are deliberately excluded.
pub(crate) fn async_to_sync(lang: nose_il::Lang, name: &str) -> Option<&'static str> {
    nose_semantics::async_to_sync_name(lang, name)
}

fn file_defines_name(old: &Il, interner: &Interner, name: &str) -> bool {
    old.units
        .iter()
        .filter_map(|unit| unit.name)
        .any(|symbol| interner.resolve(symbol) == name)
        || old
            .nodes
            .iter()
            .enumerate()
            .any(|(idx, node)| match node.payload {
                Payload::Cid(cid) => old
                    .cid_names
                    .get(cid as usize)
                    .is_some_and(|symbol| symbol_defines_name(old, interner, *symbol, name)),
                Payload::Name(symbol)
                    if matches!(
                        node.kind,
                        NodeKind::Module | NodeKind::Block | NodeKind::Param
                    ) =>
                {
                    symbol_defines_name(old, interner, symbol, name)
                }
                _ if node.kind == NodeKind::Assign => old
                    .children(NodeId(idx as u32))
                    .first()
                    .is_some_and(|&lhs| node_defines_name(old, interner, lhs, name)),
                _ => false,
            })
}

fn node_defines_name(old: &Il, interner: &Interner, node: NodeId, name: &str) -> bool {
    match old.node(node).payload {
        Payload::Name(symbol) => symbol_defines_name(old, interner, symbol, name),
        Payload::Cid(cid) => old
            .cid_names
            .get(cid as usize)
            .is_some_and(|symbol| symbol_defines_name(old, interner, *symbol, name)),
        _ => false,
    }
}

fn symbol_defines_name(old: &Il, interner: &Interner, symbol: nose_il::Symbol, name: &str) -> bool {
    let text = interner.resolve(symbol);
    text == name
        || (nose_semantics::semantics(old.meta.lang)
            .modules()
            .js_like_shadowed_module_bindings()
            && contains_js_identifier(text, name))
}

fn file_defines_type_name(old: &Il, interner: &Interner, name: &str) -> bool {
    old.units
        .iter()
        .filter(|unit| matches!(unit.kind, nose_il::UnitKind::Class))
        .filter_map(|unit| unit.name)
        .any(|symbol| interner.resolve(symbol) == name)
}

fn name_of<'a>(old: &Il, interner: &'a Interner, id: NodeId) -> Option<&'a str> {
    let n = old.node(id);
    if n.kind == NodeKind::Var {
        if let Payload::Name(s) = n.payload {
            return Some(interner.resolve(s));
        }
    }
    None
}

fn map_like_literal(old: &Il, interner: &Interner, id: NodeId) -> bool {
    if old.kind(id) != NodeKind::Seq {
        return false;
    }
    seq_surface_contract_for_node(old, interner, id)
        .is_some_and(|contract| contract.value_tag == SEQ_VALUE_MAP)
}

fn map_get_call_parts(old: &Il, interner: &Interner, id: NodeId) -> Option<(NodeId, NodeId)> {
    if old.kind(id) != NodeKind::Call {
        return None;
    }
    let kids = old.children(id);
    if kids.len() != 2 || old.kind(kids[0]) != NodeKind::Field {
        return None;
    }
    let Payload::Name(method) = old.node(kids[0]).payload else {
        return None;
    };
    map_get_contract(old.meta.lang, interner.resolve(method), 1)?;
    let receiver = *old.children(kids[0]).first()?;
    Some((receiver, kids[1]))
}

fn key_set_receiver(old: &Il, interner: &Interner, id: NodeId) -> Option<NodeId> {
    if old.kind(id) != NodeKind::Call {
        return None;
    }
    let kids = old.children(id);
    if kids.len() != 1 || old.kind(kids[0]) != NodeKind::Field {
        return None;
    }
    let Payload::Name(method) = old.node(kids[0]).payload else {
        return None;
    };
    let contract = map_key_view_contract(old.meta.lang, interner.resolve(method), 0)?;
    if contract.kind != MapKeyViewKind::Collection {
        return None;
    }
    old.children(kids[0]).first().copied()
}

fn zero_arg_lambda_body(old: &Il, lambda: NodeId) -> Option<NodeId> {
    if old.kind(lambda) != NodeKind::Lambda {
        return None;
    }
    let kids = old.children(lambda);
    if kids.len() == 1 {
        Some(kids[0])
    } else {
        None
    }
}

fn zero_arg_lambda_body_value(old: &Il, lambda: NodeId) -> Option<NodeId> {
    let body = zero_arg_lambda_body(old, lambda)?;
    implicit_block_value(old, body).or(Some(body))
}

fn implicit_block_value(old: &Il, block: NodeId) -> Option<NodeId> {
    if old.kind(block) != NodeKind::Block {
        return None;
    }
    let kids = old.children(block);
    let &[stmt] = kids else {
        return None;
    };
    match old.kind(stmt) {
        NodeKind::ExprStmt | NodeKind::Return => {
            let stmt_kids = old.children(stmt);
            let &[expr] = stmt_kids else {
                return None;
            };
            Some(expr)
        }
        _ => None,
    }
}

fn identity_lambda(old: &Il, lambda: NodeId) -> bool {
    if old.kind(lambda) != NodeKind::Lambda {
        return false;
    }
    let kids = old.children(lambda);
    if kids.len() != 2 || old.kind(kids[0]) != NodeKind::Param || old.kind(kids[1]) != NodeKind::Var
    {
        return false;
    }
    match (old.node(kids[0]).payload, old.node(kids[1]).payload) {
        (Payload::Cid(a), Payload::Cid(b)) => a == b,
        (Payload::Name(a), Payload::Name(b)) => a == b,
        _ => false,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use nose_il::stable_symbol_hash;
    use nose_il::{
        EvidenceAnchor, EvidenceEmitter, EvidenceId, EvidenceKind, EvidenceProvenance,
        EvidenceRecord, EvidenceStatus, FileId, FileMeta, IlBuilder, Lang, ParamSemantic,
        ParamTypeFact, SequenceSurfaceKind, Span, Unit, UnitKind,
    };
    use nose_semantics::{import_fact_tag, ImportFactKind};

    fn sp() -> Span {
        Span::new(FileId(0), 1, 1, 1, 1)
    }

    fn evidence(
        id: u32,
        anchor: EvidenceAnchor,
        kind: EvidenceKind,
        status: EvidenceStatus,
    ) -> EvidenceRecord {
        EvidenceRecord {
            id: EvidenceId(id),
            anchor,
            kind,
            provenance: EvidenceProvenance {
                emitter: EvidenceEmitter::FirstParty,
                pack_hash: Some(stable_symbol_hash(nose_semantics::FIRST_PARTY_PACK_ID)),
                rule_hash: Some(stable_symbol_hash("test")),
            },
            dependencies: Vec::new(),
            status,
        }
    }

    fn free_call_il(lang: Lang, name: &str, shadow_name: bool) -> (Il, Interner, NodeId) {
        let interner = Interner::new();
        let mut b = IlBuilder::new(FileId(0));
        let mut module_kids = Vec::new();
        let mut cid_names = Vec::new();
        if shadow_name {
            let sym = interner.intern(name);
            cid_names.push(sym);
            module_kids.push(b.add(NodeKind::Param, Payload::Cid(0), sp(), &[]));
        }
        let callee = b.add(
            NodeKind::Var,
            Payload::Name(interner.intern(name)),
            sp(),
            &[],
        );
        let arg = b.add(
            NodeKind::Var,
            Payload::Name(interner.intern("x")),
            sp(),
            &[],
        );
        let call = b.add(NodeKind::Call, Payload::None, sp(), &[callee, arg]);
        module_kids.push(call);
        let root = b.add(NodeKind::Module, Payload::None, sp(), &module_kids);
        let il = b.finish(
            root,
            FileMeta {
                path: "t".to_string(),
                lang,
            },
            Vec::new(),
            cid_names,
        );
        (il, interner, call)
    }

    fn method_call_il(lang: Lang, method: &str, literal_receiver: bool) -> (Il, Interner, NodeId) {
        let interner = Interner::new();
        let mut b = IlBuilder::new(FileId(0));
        let receiver = if literal_receiver {
            b.add(
                NodeKind::Seq,
                Payload::Name(interner.intern("array")),
                sp(),
                &[],
            )
        } else {
            b.add(
                NodeKind::Var,
                Payload::Name(interner.intern("xs")),
                sp(),
                &[],
            )
        };
        let field = b.add(
            NodeKind::Field,
            Payload::Name(interner.intern(method)),
            sp(),
            &[receiver],
        );
        let func = b.add(
            NodeKind::Var,
            Payload::Name(interner.intern("f")),
            sp(),
            &[],
        );
        let call = b.add(NodeKind::Call, Payload::None, sp(), &[field, func]);
        let root = b.add(NodeKind::Module, Payload::None, sp(), &[call]);
        let il = b.finish(
            root,
            FileMeta {
                path: "t".to_string(),
                lang,
            },
            Vec::new(),
            Vec::new(),
        );
        (il, interner, call)
    }

    fn method_call_no_arg_il(
        lang: Lang,
        method: &str,
        literal_receiver: bool,
    ) -> (Il, Interner, NodeId) {
        let interner = Interner::new();
        let mut b = IlBuilder::new(FileId(0));
        let receiver = if literal_receiver {
            b.add(
                NodeKind::Seq,
                Payload::Name(interner.intern("array")),
                sp(),
                &[],
            )
        } else {
            b.add(
                NodeKind::Var,
                Payload::Name(interner.intern("xs")),
                sp(),
                &[],
            )
        };
        let field = b.add(
            NodeKind::Field,
            Payload::Name(interner.intern(method)),
            sp(),
            &[receiver],
        );
        let call = b.add(NodeKind::Call, Payload::None, sp(), &[field]);
        let root = b.add(NodeKind::Module, Payload::None, sp(), &[call]);
        let il = b.finish(
            root,
            FileMeta {
                path: "t".to_string(),
                lang,
            },
            Vec::new(),
            Vec::new(),
        );
        (il, interner, call)
    }

    fn method_call_with_arg_il(
        lang: Lang,
        method: &str,
        literal_receiver: bool,
        literal_arg: bool,
    ) -> (Il, Interner, NodeId) {
        let interner = Interner::new();
        let mut b = IlBuilder::new(FileId(0));
        let receiver = if literal_receiver {
            b.add(
                NodeKind::Seq,
                Payload::Name(interner.intern("array")),
                sp(),
                &[],
            )
        } else {
            b.add(
                NodeKind::Var,
                Payload::Name(interner.intern("xs")),
                sp(),
                &[],
            )
        };
        let field = b.add(
            NodeKind::Field,
            Payload::Name(interner.intern(method)),
            sp(),
            &[receiver],
        );
        let arg = if literal_arg {
            b.add(
                NodeKind::Seq,
                Payload::Name(interner.intern("array")),
                sp(),
                &[],
            )
        } else {
            b.add(
                NodeKind::Var,
                Payload::Name(interner.intern("ys")),
                sp(),
                &[],
            )
        };
        let call = b.add(NodeKind::Call, Payload::None, sp(), &[field, arg]);
        let root = b.add(NodeKind::Module, Payload::None, sp(), &[call]);
        let il = b.finish(
            root,
            FileMeta {
                path: "t".to_string(),
                lang,
            },
            Vec::new(),
            Vec::new(),
        );
        (il, interner, call)
    }

    fn map_get_default_call_il(
        lang: Lang,
        method: &str,
        default_lambda: bool,
        lambda_param: bool,
    ) -> (Il, Interner, NodeId, NodeId) {
        let interner = Interner::new();
        let mut b = IlBuilder::new(FileId(0));
        let receiver = b.add(
            NodeKind::Seq,
            Payload::Name(interner.intern("hash")),
            sp(),
            &[],
        );
        let field = b.add(
            NodeKind::Field,
            Payload::Name(interner.intern(method)),
            sp(),
            &[receiver],
        );
        let key = b.add(NodeKind::Lit, Payload::LitStr(1), sp(), &[]);
        let fallback_value = b.add(NodeKind::Lit, Payload::LitInt(0), sp(), &[]);
        let fallback = if default_lambda {
            if lambda_param {
                let param = b.add(NodeKind::Param, Payload::Cid(0), sp(), &[]);
                b.add(
                    NodeKind::Lambda,
                    Payload::None,
                    sp(),
                    &[param, fallback_value],
                )
            } else {
                b.add(NodeKind::Lambda, Payload::None, sp(), &[fallback_value])
            }
        } else {
            fallback_value
        };
        let call = b.add(NodeKind::Call, Payload::None, sp(), &[field, key, fallback]);
        let root = b.add(NodeKind::Module, Payload::None, sp(), &[call]);
        let il = b.finish(
            root,
            FileMeta {
                path: "t".to_string(),
                lang,
            },
            Vec::new(),
            Vec::new(),
        );
        (il, interner, call, fallback_value)
    }

    fn typed_method_call_il(
        lang: Lang,
        method: &str,
        semantic: ParamSemantic,
        duplicate_param_name: bool,
    ) -> (Il, Interner, NodeId) {
        let interner = Interner::new();
        let mut b = IlBuilder::new(FileId(0));
        let mut functions = Vec::new();
        let param_span = sp();
        let xs = interner.intern("xs");
        let param = b.add(NodeKind::Param, Payload::Name(xs), param_span, &[]);
        let receiver = b.add(NodeKind::Var, Payload::Name(xs), param_span, &[]);
        let field = b.add(
            NodeKind::Field,
            Payload::Name(interner.intern(method)),
            sp(),
            &[receiver],
        );
        let func = b.add(
            NodeKind::Var,
            Payload::Name(interner.intern("f")),
            sp(),
            &[],
        );
        let call = b.add(NodeKind::Call, Payload::None, sp(), &[field, func]);
        let body = b.add(NodeKind::Block, Payload::None, sp(), &[call]);
        let function = b.add(NodeKind::Func, Payload::None, sp(), &[param, body]);
        functions.push(function);
        let duplicate_span = Span::new(FileId(0), 10, 11, 2, 2);
        if duplicate_param_name {
            let other_param = b.add(NodeKind::Param, Payload::Name(xs), duplicate_span, &[]);
            let other_body = b.add(NodeKind::Block, Payload::None, duplicate_span, &[]);
            let other_function = b.add(
                NodeKind::Func,
                Payload::None,
                duplicate_span,
                &[other_param, other_body],
            );
            functions.push(other_function);
        }
        let root = b.add(NodeKind::Module, Payload::None, sp(), &functions);
        let mut il = b.finish(
            root,
            FileMeta {
                path: "t".to_string(),
                lang,
            },
            Vec::new(),
            Vec::new(),
        );
        il.param_type_facts.push(ParamTypeFact {
            span: param_span,
            semantic,
        });
        if duplicate_param_name {
            il.param_type_facts.push(ParamTypeFact {
                span: duplicate_span,
                semantic,
            });
        }
        (il, interner, call)
    }

    fn console_log_il(shadow_console: bool) -> (Il, Interner, NodeId) {
        let interner = Interner::new();
        let mut b = IlBuilder::new(FileId(0));
        let mut module_kids = Vec::new();
        let mut cid_names = Vec::new();
        if shadow_console {
            cid_names.push(interner.intern("console"));
            module_kids.push(b.add(NodeKind::Param, Payload::Cid(0), sp(), &[]));
        }
        let receiver = b.add(
            NodeKind::Var,
            Payload::Name(interner.intern("console")),
            sp(),
            &[],
        );
        let field = b.add(
            NodeKind::Field,
            Payload::Name(interner.intern("log")),
            sp(),
            &[receiver],
        );
        let arg = b.add(
            NodeKind::Var,
            Payload::Name(interner.intern("x")),
            sp(),
            &[],
        );
        let call = b.add(NodeKind::Call, Payload::None, sp(), &[field, arg]);
        module_kids.push(call);
        let root = b.add(NodeKind::Module, Payload::None, sp(), &module_kids);
        let il = b.finish(
            root,
            FileMeta {
                path: "t".to_string(),
                lang: Lang::JavaScript,
            },
            Vec::new(),
            cid_names,
        );
        (il, interner, call)
    }

    fn go_math_abs_il(with_import: bool) -> (Il, Interner, NodeId) {
        let interner = Interner::new();
        let mut b = IlBuilder::new(FileId(0));
        let mut module_kids = Vec::new();
        if with_import {
            let lhs = b.add(
                NodeKind::Var,
                Payload::Name(interner.intern("math")),
                sp(),
                &[],
            );
            let module = b.add(
                NodeKind::Lit,
                Payload::LitStr(stable_symbol_hash("math")),
                sp(),
                &[],
            );
            let tag = interner.intern(import_fact_tag(ImportFactKind::Namespace));
            let rhs = b.add(NodeKind::Seq, Payload::Name(tag), sp(), &[module]);
            module_kids.push(b.add(NodeKind::Assign, Payload::None, sp(), &[lhs, rhs]));
        }
        let receiver = b.add(
            NodeKind::Var,
            Payload::Name(interner.intern("math")),
            sp(),
            &[],
        );
        let field = b.add(
            NodeKind::Field,
            Payload::Name(interner.intern("Abs")),
            sp(),
            &[receiver],
        );
        let arg = b.add(
            NodeKind::Var,
            Payload::Name(interner.intern("x")),
            sp(),
            &[],
        );
        let call = b.add(NodeKind::Call, Payload::None, sp(), &[field, arg]);
        module_kids.push(call);
        let root = b.add(NodeKind::Module, Payload::None, sp(), &module_kids);
        let il = b.finish(
            root,
            FileMeta {
                path: "t".to_string(),
                lang: Lang::Go,
            },
            Vec::new(),
            Vec::new(),
        );
        (il, interner, call)
    }

    #[test]
    fn free_name_builtin_requires_language_contract() {
        let (il, interner, call) = free_call_il(Lang::JavaScript, "len", false);
        assert!(matches!(canon_call(&il, &interner, call), CallCanon::None));
    }

    #[test]
    fn free_name_builtin_requires_no_shadowing() {
        let (il, interner, call) = free_call_il(Lang::Python, "len", true);
        assert!(matches!(canon_call(&il, &interner, call), CallCanon::None));
    }

    #[test]
    fn python_unshadowed_builtin_is_admitted() {
        let (il, interner, call) = free_call_il(Lang::Python, "len", false);
        assert!(matches!(
            canon_call(&il, &interner, call),
            CallCanon::Builtin {
                op: Builtin::Len,
                arg_olds
            } if arg_olds.len() == 1
        ));
    }

    #[test]
    fn method_hof_requires_exact_receiver() {
        let (il, interner, call) = method_call_il(Lang::JavaScript, "map", false);
        assert!(matches!(canon_call(&il, &interner, call), CallCanon::None));
    }

    #[test]
    fn method_hof_allows_literal_sequence_receiver() {
        let (il, interner, call) = method_call_il(Lang::JavaScript, "map", true);
        assert!(matches!(
            canon_call(&il, &interner, call),
            CallCanon::HoF {
                kind: HoFKind::Map,
                ..
            }
        ));
    }

    #[test]
    fn map_get_default_lambda_fallback_is_contract_controlled() {
        let (ruby, ruby_interner, ruby_call, ruby_fallback_value) =
            map_get_default_call_il(Lang::Ruby, "fetch", true, false);
        assert!(matches!(
            canon_call(&ruby, &ruby_interner, ruby_call),
            CallCanon::Builtin {
                op: Builtin::GetOrDefault,
                arg_olds
            } if arg_olds.len() == 3 && arg_olds[2] == ruby_fallback_value
        ));

        let (ruby_param, ruby_param_interner, ruby_param_call, _) =
            map_get_default_call_il(Lang::Ruby, "fetch", true, true);
        assert!(
            matches!(
                canon_call(&ruby_param, &ruby_param_interner, ruby_param_call),
                CallCanon::None
            ),
            "Ruby fetch block fallback must be zero-arg before exact canonicalization"
        );

        let (python, python_interner, python_call, python_fallback_value) =
            map_get_default_call_il(Lang::Python, "get", true, false);
        assert!(matches!(
            canon_call(&python, &python_interner, python_call),
            CallCanon::Builtin {
                op: Builtin::GetOrDefault,
                arg_olds
            } if arg_olds.len() == 3 && arg_olds[2] != python_fallback_value
        ));
    }

    #[test]
    fn iterator_identity_adapter_requires_kernel_contract() {
        let (js, js_interner, js_iter) = method_call_no_arg_il(Lang::JavaScript, "iter", true);
        assert!(
            !exact_protocol_receiver(&js, &js_interner, js_iter),
            "a JS method named iter is not a Rust iterator adapter"
        );

        let (rust_bad, rust_bad_interner, rust_bad_iter) = method_call_il(Lang::Rust, "iter", true);
        assert!(
            !exact_protocol_receiver(&rust_bad, &rust_bad_interner, rust_bad_iter),
            "Rust iter with unexpected arguments must not bypass the arity contract"
        );

        let (rust, rust_interner, rust_iter) = method_call_no_arg_il(Lang::Rust, "iter", true);
        assert!(
            exact_protocol_receiver(&rust, &rust_interner, rust_iter),
            "Rust iter stays admitted through iterator_identity_adapter_contract"
        );
    }

    #[test]
    fn zip_protocol_pair_requires_kernel_contract() {
        let (js, js_interner, js_zip) =
            method_call_with_arg_il(Lang::JavaScript, "zip", true, true);
        assert!(
            !exact_protocol_receiver(&js, &js_interner, js_zip),
            "a JS method named zip is not a Rust zip protocol contract"
        );

        let (rust, rust_interner, rust_zip) =
            method_call_with_arg_il(Lang::Rust, "zip", true, true);
        assert!(
            exact_protocol_receiver(&rust, &rust_interner, rust_zip),
            "Rust zip stays admitted through method_call_contract"
        );
    }

    #[test]
    fn method_bool_reduction_allows_typed_collection_receiver() {
        let (il, interner, call) =
            typed_method_call_il(Lang::TypeScript, "some", ParamSemantic::Collection, false);
        assert!(matches!(
            canon_call(&il, &interner, call),
            CallCanon::Builtin {
                op: Builtin::Any,
                arg_olds
            } if arg_olds.len() == 2
        ));
    }

    #[test]
    fn map_like_literal_respects_sequence_surface_evidence() {
        let interner = Interner::new();
        let mut b = IlBuilder::new(FileId(0));
        let map = b.add(
            NodeKind::Seq,
            Payload::Name(interner.intern("dictionary")),
            sp(),
            &[],
        );
        let root = b.add(NodeKind::Module, Payload::None, sp(), &[map]);
        let mut il = b.finish(
            root,
            FileMeta {
                path: "t".to_string(),
                lang: Lang::Python,
            },
            Vec::new(),
            Vec::new(),
        );

        assert!(map_like_literal(&il, &interner, map));

        il.evidence.push(evidence(
            0,
            EvidenceAnchor::sequence(sp()),
            EvidenceKind::SequenceSurface(SequenceSurfaceKind::Collection),
            EvidenceStatus::Asserted,
        ));
        assert!(
            !map_like_literal(&il, &interner, map),
            "conflicting sequence-surface evidence must block raw map tag fallback"
        );
    }

    fn rust_hash_map_from_call(entry_surface: &str, shadow_std: bool) -> (Il, Interner, NodeId) {
        let interner = Interner::new();
        let mut b = IlBuilder::new(FileId(0));
        let key = b.add(NodeKind::Lit, Payload::LitStr(1), sp(), &[]);
        let value = b.add(NodeKind::Lit, Payload::LitInt(1), sp(), &[]);
        let entry = b.add(
            NodeKind::Seq,
            Payload::Name(interner.intern(entry_surface)),
            sp(),
            &[key, value],
        );
        let entries = b.add(
            NodeKind::Seq,
            Payload::Name(interner.intern("array")),
            sp(),
            &[entry],
        );
        let callee = b.add(
            NodeKind::Var,
            Payload::Name(interner.intern("std::collections::HashMap::from")),
            sp(),
            &[],
        );
        let call = b.add(NodeKind::Call, Payload::None, sp(), &[callee, entries]);
        let root = b.add(NodeKind::Module, Payload::None, sp(), &[call]);
        let units = if shadow_std {
            vec![Unit {
                root,
                kind: UnitKind::Class,
                name: Some(interner.intern("std")),
            }]
        } else {
            Vec::new()
        };
        let il = b.finish(
            root,
            FileMeta {
                path: "t".to_string(),
                lang: Lang::Rust,
            },
            units,
            Vec::new(),
        );
        (il, interner, call)
    }

    #[test]
    fn rust_std_map_factory_requires_entry_surface_and_shadow_proof() {
        let (il, interner, call) = rust_hash_map_from_call("tuple", false);
        assert!(rust_std_map_factory_call(&il, &interner, call));

        let (il, interner, call) = rust_hash_map_from_call("array", false);
        assert!(
            !rust_std_map_factory_call(&il, &interner, call),
            "HashMap::from exact map proof requires tuple-shaped entries"
        );

        let (il, interner, call) = rust_hash_map_from_call("tuple", true);
        assert!(
            !rust_std_map_factory_call(&il, &interner, call),
            "a local std binding must close the Rust stdlib factory path"
        );
    }

    #[test]
    fn method_bool_reduction_uses_lexical_param_scope() {
        let (il, interner, call) =
            typed_method_call_il(Lang::TypeScript, "some", ParamSemantic::Collection, true);
        assert!(matches!(
            canon_call(&il, &interner, call),
            CallCanon::Builtin {
                op: Builtin::Any,
                arg_olds
            } if arg_olds.len() == 2
        ));
    }

    #[test]
    fn js_console_print_requires_no_shadowing() {
        let (il, interner, call) = console_log_il(true);
        assert!(matches!(canon_call(&il, &interner, call), CallCanon::None));

        let (il, interner, call) = console_log_il(false);
        assert!(matches!(
            canon_call(&il, &interner, call),
            CallCanon::Builtin {
                op: Builtin::Print,
                arg_olds
            } if arg_olds.len() == 1
        ));
    }

    #[test]
    fn go_stdlib_method_requires_import_namespace_proof() {
        let (il, interner, call) = go_math_abs_il(false);
        assert!(matches!(canon_call(&il, &interner, call), CallCanon::None));

        let (il, interner, call) = go_math_abs_il(true);
        assert!(matches!(
            canon_call(&il, &interner, call),
            CallCanon::Builtin {
                op: Builtin::Abs,
                arg_olds
            } if arg_olds.len() == 1
        ));
    }
}
