//! Cross-language idiom canonicalization: collapse equivalent builtins from
//! different languages to one [`Builtin`] op so that, e.g., Python `len(xs)`,
//! JS `xs.length`, and Go `len(xs)` all converge. Detection on `xs.length` (a
//! field access) is handled by the caller; this module only inspects `Call`s.
//!
//! proof-obligation: normalize.value_graph.bool_reduce
//! proof-obligation: normalize.value_graph.functor
//! proof-obligation: normalize.value_graph.min_max

use nose_il::{stable_symbol_hash, Builtin, HoFKind, Il, Interner, NodeId, NodeKind, Payload};
use nose_semantics::{
    domain_evidence_from_param_semantic, free_function_builtin_contract,
    iterator_identity_adapter_contract, map_get_contract, map_key_view_contract,
    method_call_contract, method_hof_contract, rust_option_some_constructor_contract,
    BuiltinArgContract, DomainEvidence, MapKeyViewKind, MethodBuiltinArgs, MethodCallContract,
    MethodReceiverContract, MethodSemanticContract,
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
                    return apply_method_contract(old, interner, fname, contract, proven, args);
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
        MethodReceiverContract::UnshadowedGlobal(global) => (name_of(old, interner, base)
            == Some(global)
            && !file_defines_name(old, interner, global))
        .then_some(ProvenReceiver::Direct(base)),
        MethodReceiverContract::ImportedNamespace(module) => {
            import_namespace_expr(old, interner, base, module)
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
    method: &str,
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
            let fallback = if method == "fetch" && old.kind(args[1]) == NodeKind::Lambda {
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

/// Peel a first-party Rust identity iterator adapter to the underlying collection, so
/// `xs.iter().fold(..)` iterates over `xs` (matching `for v in xs`). NOT `.keys()`/`.values()`,
/// which change *what* is iterated.
fn unwrap_iter(old: &Il, interner: &Interner, node: NodeId) -> NodeId {
    if old.kind(node) == NodeKind::Call {
        let call_kids = old.children(node);
        if let Some(&callee) = call_kids.first() {
            if old.kind(callee) == NodeKind::Field {
                if let Payload::Name(s) = old.node(callee).payload {
                    let method = interner.resolve(s);
                    if method == "stream" {
                        if let Some(&base) = old.children(callee).first() {
                            if name_of(old, interner, base) == Some("Arrays") {
                                if let Some(&arg) = old.children(node).get(1) {
                                    return arg;
                                }
                            } else {
                                return base;
                            }
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
    if matches!(method, "iter" | "into_iter" | "iter_mut") {
        return exact_collection_receiver(old, interner, receiver);
    }
    if method == "stream" && name_of(old, interner, receiver) == Some("Arrays") {
        return kids
            .get(1)
            .is_some_and(|&arg| exact_collection_receiver(old, interner, arg));
    }
    if method == "zip" && kids.len() == 2 {
        return exact_protocol_receiver(old, interner, receiver)
            && exact_protocol_receiver(old, interner, kids[1]);
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
        old.param_type_facts
            .iter()
            .find(|fact| fact.span == span)
            .map(|fact| domain_evidence_from_param_semantic(fact.semantic))
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
    match old.node(node).payload {
        Payload::None => true,
        Payload::Name(name) => matches!(
            interner.resolve(name),
            "array" | "array_expression" | "list" | "tuple" | "tuple_expression"
        ),
        _ => false,
    }
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
        .any(|factory| factory.names.iter().any(|name| *name == callee))
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
        || old.nodes.iter().any(|node| match node.payload {
            Payload::Cid(cid) => old
                .cid_names
                .get(cid as usize)
                .is_some_and(|symbol| interner.resolve(*symbol) == name),
            Payload::Name(symbol) if matches!(node.kind, NodeKind::Module | NodeKind::Block) => {
                interner.resolve(symbol) == name
            }
            _ => false,
        })
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

fn import_namespace_expr(old: &Il, interner: &Interner, id: NodeId, module: &str) -> bool {
    let Some(alias) = name_of(old, interner, id) else {
        return false;
    };
    old.children(old.root).iter().any(|&stmt| {
        import_namespace_assignment(old, interner, stmt, alias, module)
            || (old.kind(stmt) == NodeKind::Block
                && old
                    .children(stmt)
                    .iter()
                    .any(|&child| import_namespace_assignment(old, interner, child, alias, module)))
    })
}

fn import_namespace_assignment(
    old: &Il,
    interner: &Interner,
    stmt: NodeId,
    alias: &str,
    module: &str,
) -> bool {
    if old.kind(stmt) != NodeKind::Assign {
        return false;
    }
    let kids = old.children(stmt);
    if kids.len() != 2 || assignment_alias_name(old, interner, kids[0]) != Some(alias) {
        return false;
    }
    if old.kind(kids[1]) != NodeKind::Seq {
        return false;
    }
    let Payload::Name(seq_name) = old.node(kids[1]).payload else {
        return false;
    };
    if interner.resolve(seq_name) != "import_namespace" {
        return false;
    }
    let Some(&module_node) = old.children(kids[1]).first() else {
        return false;
    };
    matches!(
        old.node(module_node).payload,
        Payload::LitStr(hash) if hash == stable_symbol_hash(module)
    )
}

fn assignment_alias_name<'a>(old: &Il, interner: &'a Interner, id: NodeId) -> Option<&'a str> {
    if old.kind(id) != NodeKind::Var {
        return None;
    }
    match old.node(id).payload {
        Payload::Name(symbol) => Some(interner.resolve(symbol)),
        Payload::Cid(cid) => old
            .cid_names
            .get(cid as usize)
            .map(|&symbol| interner.resolve(symbol)),
        _ => None,
    }
}

fn map_like_literal(old: &Il, interner: &Interner, id: NodeId) -> bool {
    if old.kind(id) != NodeKind::Seq {
        return false;
    }
    match old.node(id).payload {
        Payload::Name(s) => matches!(interner.resolve(s), "dictionary" | "hash" | "object"),
        _ => false,
    }
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
    use nose_il::{FileId, FileMeta, IlBuilder, Lang, ParamSemantic, ParamTypeFact, Span};

    fn sp() -> Span {
        Span::new(FileId(0), 1, 1, 1, 1)
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
            b.add(NodeKind::Seq, Payload::None, sp(), &[])
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
            let tag = interner.intern("import_namespace");
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
