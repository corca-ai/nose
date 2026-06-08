//! Cross-language idiom canonicalization: collapse equivalent builtins from
//! different languages to one [`Builtin`] op so that, e.g., Python `len(xs)`,
//! JS `xs.length`, and Go `len(xs)` all converge. Detection on `xs.length` (a
//! field access) is handled by the caller; this module only inspects `Call`s.
//!
//! proof-obligation: normalize.value_graph.bool_reduce
//! proof-obligation: normalize.value_graph.functor
//! proof-obligation: normalize.value_graph.min_max

use nose_il::{
    Builtin, DomainEvidence, EvidenceAnchor, EvidenceKind, EvidenceStatus, HoFKind, Il, Interner,
    NodeId, NodeKind, Payload, Span, Symbol,
};
use nose_semantics::{
    domain_evidence_for_param, imported_namespace_symbol, library_api_contract_evidence_for_call,
    library_free_function_builtin_contract, library_free_name_map_factory_contract,
    library_iterator_identity_adapter_contract, library_map_get_contract,
    library_map_key_view_contract, library_method_call_contract,
    library_rust_option_some_constructor_contract, library_static_collection_adapter_contract,
    seq_surface_contract_for_node, unshadowed_global_symbol, BuiltinArgContract, DomainRequirement,
    LibraryApiCalleeContract, LibraryApiEvidenceStatus, LibraryMapFactoryResult, MapKeyViewKind,
    MethodBuiltinArgs, MethodCallContract, MethodReceiverContract, MethodSemanticContract,
    SEQ_VALUE_MAP,
};
use rustc_hash::{FxHashMap, FxHashSet};

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

pub(crate) struct ParamDomainIndex {
    global_cid_domains: FxHashMap<u32, DomainLookup>,
    node_domains: FxHashMap<(Span, NodeKind), DomainLookup>,
    scopes: Vec<ParamScope>,
}

struct ParamScope {
    span: Span,
    cid_domains: FxHashMap<u32, DomainLookup>,
    params: FxHashMap<Symbol, Option<DomainEvidence>>,
    assigned_names: FxHashSet<Symbol>,
}

#[derive(Clone, Copy)]
enum DomainLookup {
    Missing,
    Found(DomainEvidence),
    Ambiguous,
}

impl DomainLookup {
    fn option(self) -> Option<DomainEvidence> {
        match self {
            DomainLookup::Found(domain) => Some(domain),
            DomainLookup::Missing | DomainLookup::Ambiguous => None,
        }
    }

    fn insert(&mut self, domain: DomainEvidence) {
        match *self {
            DomainLookup::Missing => *self = DomainLookup::Found(domain),
            DomainLookup::Found(existing) if existing == domain => {}
            DomainLookup::Found(_) | DomainLookup::Ambiguous => *self = DomainLookup::Ambiguous,
        }
    }
}

impl ParamDomainIndex {
    pub(crate) fn new(old: &Il) -> Self {
        let mut global_cid_domains = FxHashMap::default();
        let node_domains = node_domain_index(old);
        let mut scopes = Vec::new();
        for (idx, node) in old.nodes.iter().enumerate() {
            if !matches!(node.kind, NodeKind::Func | NodeKind::Lambda) {
                continue;
            }
            let scope = NodeId(idx as u32);
            let mut cid_domains = FxHashMap::default();
            let mut params = FxHashMap::default();
            for &child in old.children(scope) {
                if old.kind(child) != NodeKind::Param {
                    continue;
                }
                let domain = domain_evidence_for_param(old, child);
                match old.node(child).payload {
                    Payload::Cid(cid) => {
                        merge_domain(&mut cid_domains, cid, domain);
                    }
                    Payload::Name(name) => {
                        params.entry(name).or_insert(domain);
                    }
                    _ => {}
                }
            }
            scopes.push(ParamScope {
                span: node.span,
                cid_domains,
                params,
                assigned_names: FxHashSet::default(),
            });
        }
        for (idx, node) in old.nodes.iter().enumerate() {
            if node.kind != NodeKind::Param {
                continue;
            }
            let Payload::Cid(cid) = node.payload else {
                continue;
            };
            merge_domain(
                &mut global_cid_domains,
                cid,
                domain_evidence_for_param(old, NodeId(idx as u32)),
            );
        }

        scopes.sort_by_key(|scope| scope.span.end_byte.saturating_sub(scope.span.start_byte));
        for (idx, node) in old.nodes.iter().enumerate() {
            if node.kind != NodeKind::Assign {
                continue;
            }
            let id = NodeId(idx as u32);
            let Some(&lhs) = old.children(id).first() else {
                continue;
            };
            if old.kind(lhs) != NodeKind::Var {
                continue;
            }
            let Payload::Name(name) = old.node(lhs).payload else {
                continue;
            };
            if let Some(scope) = scopes
                .iter_mut()
                .find(|scope| span_contains(scope.span, node.span))
            {
                scope.assigned_names.insert(name);
            }
        }

        Self {
            global_cid_domains,
            node_domains,
            scopes,
        }
    }

    fn domain_evidence_for_receiver(&self, old: &Il, node: NodeId) -> Option<DomainEvidence> {
        match self
            .node_domains
            .get(&(old.node(node).span, old.kind(node)))
            .copied()
            .unwrap_or(DomainLookup::Missing)
        {
            DomainLookup::Found(domain) => return Some(domain),
            DomainLookup::Ambiguous => return None,
            DomainLookup::Missing => {}
        }
        self.domain_evidence_for_var_reference(old, node)
    }

    fn receiver_satisfies_domain(
        &self,
        old: &Il,
        node: NodeId,
        requirement: DomainRequirement,
    ) -> bool {
        self.domain_evidence_for_receiver(old, node)
            .is_some_and(|domain| requirement.accepts(domain))
    }

    fn domain_evidence_for_var_reference(&self, old: &Il, node: NodeId) -> Option<DomainEvidence> {
        if old.kind(node) != NodeKind::Var {
            return None;
        }
        match old.node(node).payload {
            Payload::Cid(cid) => {
                for scope in &self.scopes {
                    if span_contains(scope.span, old.node(node).span) {
                        return scope
                            .cid_domains
                            .get(&cid)
                            .copied()
                            .unwrap_or(DomainLookup::Missing)
                            .option();
                    }
                }
                self.global_cid_domains
                    .get(&cid)
                    .copied()
                    .unwrap_or(DomainLookup::Missing)
                    .option()
            }
            Payload::Name(name) => {
                let span = old.node(node).span;
                for scope in &self.scopes {
                    if !span_contains(scope.span, span) {
                        continue;
                    }
                    let Some(&domain) = scope.params.get(&name) else {
                        continue;
                    };
                    if scope.assigned_names.contains(&name) {
                        return None;
                    }
                    return domain;
                }
                None
            }
            _ => None,
        }
    }

    pub(crate) fn property_receiver_satisfies_domain(
        &self,
        old: &Il,
        node: NodeId,
        requirement: DomainRequirement,
    ) -> bool {
        self.receiver_satisfies_domain(old, node, requirement)
    }
}

fn merge_domain(
    domains: &mut FxHashMap<u32, DomainLookup>,
    key: u32,
    domain: Option<DomainEvidence>,
) {
    let Some(domain) = domain else {
        return;
    };
    domains
        .entry(key)
        .or_insert(DomainLookup::Missing)
        .insert(domain);
}

fn node_domain_index(old: &Il) -> FxHashMap<(Span, NodeKind), DomainLookup> {
    let mut domains = FxHashMap::default();
    for record in &old.evidence {
        let EvidenceAnchor::Node { span, kind } = record.anchor else {
            continue;
        };
        let EvidenceKind::Domain(domain) = record.kind else {
            continue;
        };
        let entry = domains.entry((span, kind)).or_insert(DomainLookup::Missing);
        if record.status != EvidenceStatus::Asserted || !old.evidence_dependencies_asserted(record)
        {
            *entry = DomainLookup::Ambiguous;
        } else {
            entry.insert(domain);
        }
    }
    domains
}

#[cfg(test)]
fn canon_call(old: &Il, interner: &Interner, call_id: NodeId) -> CallCanon {
    let domains = ParamDomainIndex::new(old);
    canon_call_with_domains(old, interner, &domains, call_id)
}

pub(crate) fn canon_call_with_domains(
    old: &Il,
    interner: &Interner,
    domains: &ParamDomainIndex,
    call_id: NodeId,
) -> CallCanon {
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
                if let Some(contract) = library_free_function_builtin_contract(
                    old.meta.lang,
                    interner.resolve(s),
                    args.len(),
                ) {
                    if !library_api_evidence_admitted(
                        old,
                        interner,
                        call_id,
                        contract.id,
                        contract.callee,
                        args.len(),
                    ) {
                        return CallCanon::None;
                    }
                    match contract.result.args {
                        BuiltinArgContract::First => builtin!(contract.result.builtin, first),
                        BuiltinArgContract::All => builtin!(contract.result.builtin, all),
                    }
                }
            }
        }
        NodeKind::Field => {
            if let Payload::Name(s) = cn.payload {
                let fname = interner.resolve(s);
                let base = old.children(callee).first().copied();
                if let Some(contract) =
                    library_method_call_contract(old.meta.lang, fname, args.len())
                {
                    if !library_api_evidence_admitted(
                        old,
                        interner,
                        call_id,
                        contract.id,
                        contract.callee,
                        args.len(),
                    ) {
                        return CallCanon::None;
                    }
                    let Some(base) = base else {
                        return CallCanon::None;
                    };
                    let Some(proven) = prove_method_receiver(
                        old,
                        interner,
                        domains,
                        contract.result.receiver,
                        base,
                        args,
                    ) else {
                        return CallCanon::None;
                    };
                    return apply_method_contract(old, interner, contract.result, proven, args);
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
    domains: &ParamDomainIndex,
    contract: MethodReceiverContract,
    base: NodeId,
    args: &[NodeId],
) -> Option<ProvenReceiver> {
    match contract {
        MethodReceiverContract::ExactCollection => {
            exact_collection_receiver(old, interner, domains, base)
                .then_some(ProvenReceiver::Direct(base))
        }
        MethodReceiverContract::ExactProtocol => {
            exact_protocol_receiver(old, interner, domains, base)
                .then_some(ProvenReceiver::Direct(base))
        }
        MethodReceiverContract::ExactProtocolPairArgument => {
            (exact_protocol_receiver(old, interner, domains, base)
                && args
                    .first()
                    .is_some_and(|&arg| exact_protocol_receiver(old, interner, domains, arg)))
            .then_some(ProvenReceiver::Direct(base))
        }
        MethodReceiverContract::ExactOption => exact_option_receiver(old, interner, domains, base)
            .then_some(ProvenReceiver::Direct(base)),
        MethodReceiverContract::ExactString => {
            exact_string_receiver(old, domains, base).then_some(ProvenReceiver::Direct(base))
        }
        MethodReceiverContract::ExactInteger => {
            exact_integer_receiver(old, domains, base).then_some(ProvenReceiver::Direct(base))
        }
        MethodReceiverContract::ExactMap => {
            exact_map_receiver(old, interner, domains, base).then_some(ProvenReceiver::Direct(base))
        }
        MethodReceiverContract::ExactMapLiteral => {
            map_like_literal(old, interner, base).then_some(ProvenReceiver::Direct(base))
        }
        MethodReceiverContract::ExactCollectionOrMap => {
            (exact_collection_receiver(old, interner, domains, base)
                || exact_map_receiver(old, interner, domains, base))
            .then_some(ProvenReceiver::Direct(base))
        }
        MethodReceiverContract::ExactCollectionOrMapLiteral => {
            (exact_collection_receiver(old, interner, domains, base)
                || map_like_literal(old, interner, base))
            .then_some(ProvenReceiver::Direct(base))
        }
        MethodReceiverContract::ExactCollectionOrJavaKeySet => {
            if exact_collection_receiver(old, interner, domains, base) {
                return Some(ProvenReceiver::Direct(base));
            }
            if old.meta.lang == nose_il::Lang::Java {
                let map = key_set_receiver(old, interner, base)?;
                return map_like_literal(old, interner, map).then_some(ProvenReceiver::Direct(map));
            }
            None
        }
        MethodReceiverContract::ExactSetOrMap => (exact_set_param(old, domains, base)
            || exact_map_param(old, domains, base))
        .then_some(ProvenReceiver::Direct(base)),
        MethodReceiverContract::LiteralString => {
            literal_string_receiver(old, domains, base).then_some(ProvenReceiver::Direct(base))
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
                exact_option_receiver(old, interner, domains, base)
                    .then_some(ProvenReceiver::Direct(base))
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
                        if let Some(arg) = static_collection_adapter_arg(
                            old, interner, node, base, method, call_kids,
                        ) {
                            return arg;
                        }
                    }
                    if library_iterator_identity_adapter_contract(
                        old.meta.lang,
                        method,
                        call_kids.len() - 1,
                    )
                    .is_some_and(|contract| {
                        library_api_evidence_admitted(
                            old,
                            interner,
                            node,
                            contract.id,
                            contract.callee,
                            call_kids.len() - 1,
                        )
                    }) {
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

fn exact_collection_receiver(
    old: &Il,
    interner: &Interner,
    domains: &ParamDomainIndex,
    node: NodeId,
) -> bool {
    exact_protocol_receiver(old, interner, domains, node)
}

fn exact_protocol_receiver(
    old: &Il,
    interner: &Interner,
    domains: &ParamDomainIndex,
    node: NodeId,
) -> bool {
    if exact_collection_literal(old, interner, node) || exact_collection_param(old, domains, node) {
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
    if let Some(arg) = static_collection_adapter_arg(old, interner, node, receiver, method, kids) {
        return exact_collection_receiver(old, interner, domains, arg);
    }
    if let Some(contract) = library_method_call_contract(old.meta.lang, method, kids.len() - 1) {
        if contract.result.semantic == MethodSemanticContract::Builtin(Builtin::Zip)
            && contract.result.receiver == MethodReceiverContract::ExactProtocolPairArgument
            && contract.result.args == MethodBuiltinArgs::RustZip
            && library_api_evidence_admitted(
                old,
                interner,
                node,
                contract.id,
                contract.callee,
                kids.len() - 1,
            )
        {
            return exact_protocol_receiver(old, interner, domains, receiver)
                && exact_protocol_receiver(old, interner, domains, kids[1]);
        }
    }
    if library_iterator_identity_adapter_contract(old.meta.lang, method, kids.len() - 1)
        .is_some_and(|contract| {
            library_api_evidence_admitted(
                old,
                interner,
                node,
                contract.id,
                contract.callee,
                kids.len() - 1,
            )
        })
    {
        return exact_protocol_receiver(old, interner, domains, receiver);
    }
    if let Some(contract) = library_method_call_contract(old.meta.lang, method, kids.len() - 1) {
        if matches!(contract.result.semantic, MethodSemanticContract::HoF(_))
            && kids.len() >= 2
            && library_api_evidence_admitted(
                old,
                interner,
                node,
                contract.id,
                contract.callee,
                kids.len() - 1,
            )
        {
            return exact_protocol_receiver(old, interner, domains, receiver);
        }
    }
    false
}

fn exact_collection_param(old: &Il, domains: &ParamDomainIndex, node: NodeId) -> bool {
    domains.receiver_satisfies_domain(old, node, DomainRequirement::ArrayCollectionOrSet)
}

fn static_collection_adapter_arg(
    old: &Il,
    interner: &Interner,
    call: NodeId,
    receiver: NodeId,
    method: &str,
    call_kids: &[NodeId],
) -> Option<NodeId> {
    let receiver_name = name_of(old, interner, receiver)?;
    let contract = library_static_collection_adapter_contract(
        old.meta.lang,
        receiver_name,
        method,
        call_kids.len().saturating_sub(1),
    )?;
    match library_api_contract_evidence_for_call(
        old,
        interner,
        call,
        contract.id,
        contract.callee,
        call_kids.len().saturating_sub(1),
    ) {
        LibraryApiEvidenceStatus::Admitted => {}
        LibraryApiEvidenceStatus::Rejected => return None,
        LibraryApiEvidenceStatus::Missing => return None,
    }
    call_kids.get(1).copied()
}

fn library_api_evidence_admitted(
    old: &Il,
    interner: &Interner,
    call: NodeId,
    id: nose_semantics::LibraryApiContractId,
    callee: LibraryApiCalleeContract,
    arg_count: usize,
) -> bool {
    matches!(
        library_api_contract_evidence_for_call(old, interner, call, id, callee, arg_count),
        LibraryApiEvidenceStatus::Admitted
    )
}

fn exact_set_param(old: &Il, domains: &ParamDomainIndex, node: NodeId) -> bool {
    domains.receiver_satisfies_domain(old, node, DomainRequirement::Set)
}

fn exact_map_param(old: &Il, domains: &ParamDomainIndex, node: NodeId) -> bool {
    domains.receiver_satisfies_domain(old, node, DomainRequirement::Map)
}

fn exact_map_receiver(
    old: &Il,
    interner: &Interner,
    domains: &ParamDomainIndex,
    node: NodeId,
) -> bool {
    exact_map_param(old, domains, node) || map_like_literal(old, interner, node)
}

fn exact_option_param(old: &Il, domains: &ParamDomainIndex, node: NodeId) -> bool {
    domains.receiver_satisfies_domain(old, node, DomainRequirement::Option)
}

fn span_contains(outer: Span, inner: Span) -> bool {
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

fn exact_option_receiver(
    old: &Il,
    interner: &Interner,
    domains: &ParamDomainIndex,
    node: NodeId,
) -> bool {
    if exact_option_param(old, domains, node) {
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
    let Payload::Name(name) = old.node(kids[0]).payload else {
        return false;
    };
    let Some(contract) =
        library_rust_option_some_constructor_contract(old.meta.lang, interner.resolve(name), 1)
    else {
        return false;
    };
    library_api_evidence_admitted(old, interner, node, contract.id, contract.callee, 1)
}

fn exact_string_receiver(old: &Il, domains: &ParamDomainIndex, node: NodeId) -> bool {
    matches!(
        old.node(node).payload,
        Payload::LitStr(_) | Payload::Lit(nose_il::LitClass::Str)
    ) || domains.receiver_satisfies_domain(old, node, DomainRequirement::String)
}

fn literal_string_receiver(old: &Il, domains: &ParamDomainIndex, node: NodeId) -> bool {
    exact_string_receiver(old, domains, node)
}

fn exact_integer_receiver(old: &Il, domains: &ParamDomainIndex, node: NodeId) -> bool {
    matches!(
        old.node(node).payload,
        Payload::LitInt(_) | Payload::Lit(nose_il::LitClass::Int)
    ) || domains.receiver_satisfies_domain(old, node, DomainRequirement::Integer)
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
    let Some(contract) = library_free_name_map_factory_contract(old.meta.lang, callee) else {
        return false;
    };
    let LibraryApiCalleeContract::FreeName { .. } = contract.callee else {
        return false;
    };
    let LibraryMapFactoryResult::EntrySequence { entry_seq_tag } = contract.result else {
        return false;
    };
    matches!(
        library_api_contract_evidence_for_call(
            old,
            interner,
            node,
            contract.id,
            contract.callee,
            1,
        ),
        LibraryApiEvidenceStatus::Admitted
    ) && map_factory_entries_match_surface(old, interner, kids[1], entry_seq_tag)
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
    let contract = library_map_get_contract(old.meta.lang, interner.resolve(method), 1)?;
    if !library_api_evidence_admitted(old, interner, id, contract.id, contract.callee, 1) {
        return None;
    }
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
    let contract = library_map_key_view_contract(old.meta.lang, interner.resolve(method), 0)?;
    if !library_api_evidence_admitted(old, interner, id, contract.id, contract.callee, 0) {
        return None;
    }
    let contract = contract.result;
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
        EvidenceRecord, EvidenceStatus, FileId, FileMeta, IlBuilder, ImportEvidenceKind, Lang,
        LibraryApiEvidenceKind, ParamSemantic, SequenceSurfaceKind, Span, SymbolEvidenceKind, Unit,
        UnitKind,
    };

    fn sp() -> Span {
        Span::new(FileId(0), 1, 1, 1, 1)
    }

    fn sp_at(start: u32, end: u32, line: u32) -> Span {
        Span::new(FileId(0), start, end, line, line)
    }

    fn evidence(
        id: u32,
        anchor: EvidenceAnchor,
        kind: EvidenceKind,
        status: EvidenceStatus,
    ) -> EvidenceRecord {
        evidence_with_dependencies(id, anchor, kind, status, Vec::new())
    }

    fn evidence_with_dependencies(
        id: u32,
        anchor: EvidenceAnchor,
        kind: EvidenceKind,
        status: EvidenceStatus,
        dependencies: Vec<EvidenceId>,
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
            dependencies,
            status,
        }
    }

    fn next_evidence_id(il: &Il) -> u32 {
        il.evidence.len() as u32
    }

    fn method_call_receiver(il: &Il, call: NodeId) -> Option<NodeId> {
        let callee = *il.children(call).first()?;
        (il.kind(callee) == NodeKind::Field)
            .then(|| il.children(callee).first().copied())
            .flatten()
    }

    fn push_sequence_surface_evidence(
        il: &mut Il,
        node: NodeId,
        surface: SequenceSurfaceKind,
    ) -> EvidenceId {
        let id = next_evidence_id(il);
        il.evidence.push(evidence(
            id,
            EvidenceAnchor::sequence(il.node(node).span),
            EvidenceKind::SequenceSurface(surface),
            EvidenceStatus::Asserted,
        ));
        EvidenceId(id)
    }

    fn push_receiver_sequence_surface_evidence(
        il: &mut Il,
        call: NodeId,
        surface: SequenceSurfaceKind,
    ) -> EvidenceId {
        let receiver = method_call_receiver(il, call).expect("method receiver");
        push_sequence_surface_evidence(il, receiver, surface)
    }

    fn push_receiver_method_library_api_evidence(
        il: &mut Il,
        interner: &Interner,
        call: NodeId,
    ) -> Option<EvidenceId> {
        let kids = il.children(call);
        let (&callee, args) = kids.split_first()?;
        if il.kind(callee) != NodeKind::Field {
            return None;
        }
        let Payload::Name(method) = il.node(callee).payload else {
            return None;
        };
        let method = interner.resolve(method);
        let arg_count = args.len();
        let contract = library_map_get_contract(il.meta.lang, method, arg_count)
            .map(|contract| (contract.id, contract.callee))
            .or_else(|| {
                library_map_key_view_contract(il.meta.lang, method, arg_count)
                    .map(|contract| (contract.id, contract.callee))
            })
            .or_else(|| {
                library_iterator_identity_adapter_contract(il.meta.lang, method, arg_count)
                    .map(|contract| (contract.id, contract.callee))
            })
            .or_else(|| {
                library_method_call_contract(il.meta.lang, method, arg_count)
                    .map(|contract| (contract.id, contract.callee))
            })?;
        let dependencies = nose_semantics::library_api_receiver_dependencies_for_call(
            il, interner, call, contract.1,
        )?;
        let id = next_evidence_id(il);
        il.evidence.push(evidence_with_dependencies(
            id,
            EvidenceAnchor::node(il.node(call).span, NodeKind::Call),
            EvidenceKind::LibraryApi(LibraryApiEvidenceKind::Contract {
                contract_hash: nose_semantics::library_api_contract_id_hash(contract.0),
                callee_hash: nose_semantics::library_api_callee_contract_hash(contract.1),
                arity: arg_count as u16,
            }),
            EvidenceStatus::Asserted,
            dependencies,
        ));
        Some(EvidenceId(id))
    }

    fn push_free_function_builtin_library_api_evidence(
        il: &mut Il,
        interner: &Interner,
        call: NodeId,
    ) -> Option<EvidenceId> {
        let kids = il.children(call);
        let (&callee, args) = kids.split_first()?;
        let arg_count = args.len();
        let callee_span = il.node(callee).span;
        let call_span = il.node(call).span;
        let Payload::Name(symbol) = il.node(callee).payload else {
            return None;
        };
        let name = interner.resolve(symbol);
        let contract = library_free_function_builtin_contract(il.meta.lang, name, arg_count)?;
        let symbol_id = next_evidence_id(il);
        il.evidence.push(evidence(
            symbol_id,
            EvidenceAnchor::node(callee_span, NodeKind::Var),
            EvidenceKind::Symbol(SymbolEvidenceKind::UnshadowedGlobal {
                name_hash: stable_symbol_hash(name),
            }),
            EvidenceStatus::Asserted,
        ));
        let id = next_evidence_id(il);
        il.evidence.push(evidence_with_dependencies(
            id,
            EvidenceAnchor::node(call_span, NodeKind::Call),
            EvidenceKind::LibraryApi(LibraryApiEvidenceKind::Contract {
                contract_hash: nose_semantics::library_api_contract_id_hash(contract.id),
                callee_hash: nose_semantics::library_api_callee_contract_hash(contract.callee),
                arity: arg_count as u16,
            }),
            EvidenceStatus::Asserted,
            vec![EvidenceId(symbol_id)],
        ));
        Some(EvidenceId(id))
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
        let mut il = b.finish(
            root,
            FileMeta {
                path: "t".to_string(),
                lang,
            },
            Vec::new(),
            Vec::new(),
        );
        if literal_receiver {
            push_receiver_sequence_surface_evidence(&mut il, call, SequenceSurfaceKind::Collection);
            let _ = push_receiver_method_library_api_evidence(&mut il, &interner, call);
        }
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
        let mut il = b.finish(
            root,
            FileMeta {
                path: "t".to_string(),
                lang,
            },
            Vec::new(),
            Vec::new(),
        );
        if literal_receiver {
            push_receiver_sequence_surface_evidence(&mut il, call, SequenceSurfaceKind::Collection);
            let _ = push_receiver_method_library_api_evidence(&mut il, &interner, call);
        }
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
        let mut il = b.finish(
            root,
            FileMeta {
                path: "t".to_string(),
                lang,
            },
            Vec::new(),
            Vec::new(),
        );
        if literal_receiver {
            push_receiver_sequence_surface_evidence(&mut il, call, SequenceSurfaceKind::Collection);
        }
        if literal_arg {
            push_sequence_surface_evidence(&mut il, arg, SequenceSurfaceKind::Collection);
        }
        let _ = push_receiver_method_library_api_evidence(&mut il, &interner, call);
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
        let mut il = b.finish(
            root,
            FileMeta {
                path: "t".to_string(),
                lang,
            },
            Vec::new(),
            Vec::new(),
        );
        push_receiver_sequence_surface_evidence(&mut il, call, SequenceSurfaceKind::Map);
        let _ = push_receiver_method_library_api_evidence(&mut il, &interner, call);
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
        il.evidence.push(evidence(
            0,
            EvidenceAnchor::param(param_span),
            EvidenceKind::Domain(DomainEvidence::from_param_semantic(semantic)),
            EvidenceStatus::Asserted,
        ));
        if duplicate_param_name {
            il.evidence.push(evidence(
                1,
                EvidenceAnchor::param(duplicate_span),
                EvidenceKind::Domain(DomainEvidence::from_param_semantic(semantic)),
                EvidenceStatus::Asserted,
            ));
        }
        let _ = push_receiver_method_library_api_evidence(&mut il, &interner, call);
        (il, interner, call)
    }

    fn receiver_domain_method_call_il(domain: DomainEvidence) -> (Il, Interner, NodeId, Span) {
        let interner = Interner::new();
        let mut b = IlBuilder::new(FileId(0));
        let receiver_span = sp_at(20, 22, 2);
        let receiver = b.add(
            NodeKind::Var,
            Payload::Name(interner.intern("xs")),
            receiver_span,
            &[],
        );
        let field = b.add(
            NodeKind::Field,
            Payload::Name(interner.intern("some")),
            sp_at(23, 28, 2),
            &[receiver],
        );
        let func = b.add(
            NodeKind::Var,
            Payload::Name(interner.intern("f")),
            sp_at(29, 30, 2),
            &[],
        );
        let call = b.add(
            NodeKind::Call,
            Payload::None,
            sp_at(20, 31, 2),
            &[field, func],
        );
        let root = b.add(NodeKind::Module, Payload::None, sp_at(0, 40, 1), &[call]);
        let mut il = b.finish(
            root,
            FileMeta {
                path: "t.ts".to_string(),
                lang: Lang::TypeScript,
            },
            Vec::new(),
            Vec::new(),
        );
        il.evidence.push(evidence(
            0,
            EvidenceAnchor::node(receiver_span, NodeKind::Var),
            EvidenceKind::Domain(domain),
            EvidenceStatus::Asserted,
        ));
        let _ = push_receiver_method_library_api_evidence(&mut il, &interner, call);
        (il, interner, call, receiver_span)
    }

    fn typed_method_shadowed_by_untyped_inner_param_il() -> (Il, Interner, NodeId) {
        let interner = Interner::new();
        let mut b = IlBuilder::new(FileId(0));
        let xs = interner.intern("xs");
        let outer_param_span = Span::new(FileId(0), 2, 4, 1, 1);
        let inner_param_span = Span::new(FileId(0), 22, 24, 2, 2);
        let receiver_span = Span::new(FileId(0), 30, 32, 3, 3);
        let outer_param = b.add(NodeKind::Param, Payload::Name(xs), outer_param_span, &[]);
        let inner_param = b.add(NodeKind::Param, Payload::Name(xs), inner_param_span, &[]);
        let receiver = b.add(NodeKind::Var, Payload::Name(xs), receiver_span, &[]);
        let field = b.add(
            NodeKind::Field,
            Payload::Name(interner.intern("some")),
            Span::new(FileId(0), 30, 37, 3, 3),
            &[receiver],
        );
        let func = b.add(
            NodeKind::Var,
            Payload::Name(interner.intern("f")),
            Span::new(FileId(0), 38, 39, 3, 3),
            &[],
        );
        let call = b.add(
            NodeKind::Call,
            Payload::None,
            Span::new(FileId(0), 30, 42, 3, 3),
            &[field, func],
        );
        let inner_body = b.add(
            NodeKind::Block,
            Payload::None,
            Span::new(FileId(0), 25, 70, 2, 4),
            &[call],
        );
        let lambda = b.add(
            NodeKind::Lambda,
            Payload::None,
            Span::new(FileId(0), 20, 80, 2, 5),
            &[inner_param, inner_body],
        );
        let outer_body = b.add(
            NodeKind::Block,
            Payload::None,
            Span::new(FileId(0), 10, 90, 1, 6),
            &[lambda],
        );
        let function = b.add(
            NodeKind::Func,
            Payload::None,
            Span::new(FileId(0), 1, 100, 1, 7),
            &[outer_param, outer_body],
        );
        let root = b.add(
            NodeKind::Module,
            Payload::None,
            Span::new(FileId(0), 0, 101, 1, 7),
            &[function],
        );
        let mut il = b.finish(
            root,
            FileMeta {
                path: "t".to_string(),
                lang: Lang::TypeScript,
            },
            Vec::new(),
            Vec::new(),
        );
        il.evidence.push(evidence(
            0,
            EvidenceAnchor::param(outer_param_span),
            EvidenceKind::Domain(DomainEvidence::from_param_semantic(
                ParamSemantic::Collection,
            )),
            EvidenceStatus::Asserted,
        ));
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
        let mut il = b.finish(
            root,
            FileMeta {
                path: "t".to_string(),
                lang: Lang::JavaScript,
            },
            Vec::new(),
            cid_names,
        );
        if !shadow_console {
            il.evidence.push(evidence(
                0,
                EvidenceAnchor::node(il.node(receiver).span, NodeKind::Var),
                EvidenceKind::Symbol(SymbolEvidenceKind::UnshadowedGlobal {
                    name_hash: stable_symbol_hash("console"),
                }),
                EvidenceStatus::Asserted,
            ));
            let _ = push_receiver_method_library_api_evidence(&mut il, &interner, call);
        }
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
            let rhs = b.add(NodeKind::Seq, Payload::None, sp(), &[module]);
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
        let mut il = b.finish(
            root,
            FileMeta {
                path: "t".to_string(),
                lang: Lang::Go,
            },
            Vec::new(),
            Vec::new(),
        );
        if with_import {
            il.evidence.push(evidence(
                0,
                EvidenceAnchor::sequence(sp()),
                EvidenceKind::Import(ImportEvidenceKind::Namespace {
                    module_hash: stable_symbol_hash("math"),
                }),
                EvidenceStatus::Asserted,
            ));
            il.evidence.push(evidence(
                1,
                EvidenceAnchor::binding(sp(), stable_symbol_hash("math")),
                EvidenceKind::Symbol(SymbolEvidenceKind::ImportedNamespace {
                    module_hash: stable_symbol_hash("math"),
                }),
                EvidenceStatus::Asserted,
            ));
            il.evidence.push(evidence_with_dependencies(
                2,
                EvidenceAnchor::node(sp(), NodeKind::Var),
                EvidenceKind::Symbol(SymbolEvidenceKind::ImportedNamespace {
                    module_hash: stable_symbol_hash("math"),
                }),
                EvidenceStatus::Asserted,
                vec![EvidenceId(1)],
            ));
            let _ = push_receiver_method_library_api_evidence(&mut il, &interner, call);
        }
        (il, interner, call)
    }

    #[test]
    fn free_name_builtin_requires_language_contract() {
        let (il, interner, call) = free_call_il(Lang::JavaScript, "len", false);
        assert!(matches!(canon_call(&il, &interner, call), CallCanon::None));
    }

    #[test]
    fn free_name_builtin_requires_no_shadowing() {
        let (mut il, interner, call) = free_call_il(Lang::Python, "len", true);
        let _ = push_free_function_builtin_library_api_evidence(&mut il, &interner, call);
        assert!(matches!(canon_call(&il, &interner, call), CallCanon::None));
    }

    #[test]
    fn python_unshadowed_builtin_requires_library_api_evidence() {
        let (mut il, interner, call) = free_call_il(Lang::Python, "len", false);
        assert!(matches!(canon_call(&il, &interner, call), CallCanon::None));

        let _ = push_free_function_builtin_library_api_evidence(&mut il, &interner, call);
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
        let js_domains = ParamDomainIndex::new(&js);
        assert!(
            !exact_protocol_receiver(&js, &js_interner, &js_domains, js_iter),
            "a JS method named iter is not a Rust iterator adapter"
        );

        let (rust_bad, rust_bad_interner, rust_bad_iter) = method_call_il(Lang::Rust, "iter", true);
        let rust_bad_domains = ParamDomainIndex::new(&rust_bad);
        assert!(
            !exact_protocol_receiver(
                &rust_bad,
                &rust_bad_interner,
                &rust_bad_domains,
                rust_bad_iter
            ),
            "Rust iter with unexpected arguments must not bypass the arity contract"
        );

        let (rust, rust_interner, rust_iter) = method_call_no_arg_il(Lang::Rust, "iter", true);
        let rust_domains = ParamDomainIndex::new(&rust);
        assert!(
            exact_protocol_receiver(&rust, &rust_interner, &rust_domains, rust_iter),
            "Rust iter stays admitted through iterator_identity_adapter_contract"
        );
    }

    #[test]
    fn zip_protocol_pair_requires_kernel_contract() {
        let (js, js_interner, js_zip) =
            method_call_with_arg_il(Lang::JavaScript, "zip", true, true);
        let js_domains = ParamDomainIndex::new(&js);
        assert!(
            !exact_protocol_receiver(&js, &js_interner, &js_domains, js_zip),
            "a JS method named zip is not a Rust zip protocol contract"
        );

        let (rust, rust_interner, rust_zip) =
            method_call_with_arg_il(Lang::Rust, "zip", true, true);
        let rust_domains = ParamDomainIndex::new(&rust);
        assert!(
            exact_protocol_receiver(&rust, &rust_interner, &rust_domains, rust_zip),
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
    fn method_bool_reduction_consumes_receiver_domain_evidence() {
        let (il, interner, call, _) = receiver_domain_method_call_il(DomainEvidence::Collection);
        assert!(matches!(
            canon_call(&il, &interner, call),
            CallCanon::Builtin {
                op: Builtin::Any,
                arg_olds
            } if arg_olds.len() == 2
        ));

        let (mut il, interner, call, receiver_span) =
            receiver_domain_method_call_il(DomainEvidence::Collection);
        il.evidence.push(evidence(
            next_evidence_id(&il),
            EvidenceAnchor::node(receiver_span, NodeKind::Var),
            EvidenceKind::Domain(DomainEvidence::Map),
            EvidenceStatus::Asserted,
        ));
        assert!(
            matches!(canon_call(&il, &interner, call), CallCanon::None),
            "conflicting receiver-domain evidence must not fall back to selector matching"
        );
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

        assert!(
            !map_like_literal(&il, &interner, map),
            "raw map tag alone must not prove a semantic map surface"
        );

        il.evidence.push(evidence(
            0,
            EvidenceAnchor::sequence(sp()),
            EvidenceKind::SequenceSurface(SequenceSurfaceKind::Map),
            EvidenceStatus::Asserted,
        ));
        assert!(map_like_literal(&il, &interner, map));

        il.evidence.push(evidence(
            1,
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
        let entry_span = sp_at(10, 20, 1);
        let entries_span = sp_at(30, 40, 1);
        let key = b.add(NodeKind::Lit, Payload::LitStr(1), sp(), &[]);
        let value = b.add(NodeKind::Lit, Payload::LitInt(1), sp(), &[]);
        let entry = b.add(
            NodeKind::Seq,
            Payload::Name(interner.intern(entry_surface)),
            entry_span,
            &[key, value],
        );
        let entries = b.add(
            NodeKind::Seq,
            Payload::Name(interner.intern("array")),
            entries_span,
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
        let mut il = b.finish(
            root,
            FileMeta {
                path: "t".to_string(),
                lang: Lang::Rust,
            },
            units,
            Vec::new(),
        );
        let entry_kind = match entry_surface {
            "tuple" => SequenceSurfaceKind::Tuple,
            "array" => SequenceSurfaceKind::Collection,
            other => panic!("unexpected test entry surface: {other}"),
        };
        il.evidence.push(evidence(
            0,
            EvidenceAnchor::sequence(entry_span),
            EvidenceKind::SequenceSurface(entry_kind),
            EvidenceStatus::Asserted,
        ));
        il.evidence.push(evidence(
            1,
            EvidenceAnchor::sequence(entries_span),
            EvidenceKind::SequenceSurface(SequenceSurfaceKind::Collection),
            EvidenceStatus::Asserted,
        ));
        (il, interner, call)
    }

    fn push_rust_hash_map_library_api_evidence(il: &mut Il) {
        let contract =
            library_free_name_map_factory_contract(Lang::Rust, "std::collections::HashMap::from")
                .expect("Rust HashMap::from contract");
        let symbol_id = next_evidence_id(il);
        il.evidence.push(evidence(
            symbol_id,
            EvidenceAnchor::node(sp(), NodeKind::Var),
            EvidenceKind::Symbol(SymbolEvidenceKind::UnshadowedGlobal {
                name_hash: stable_symbol_hash("std::collections::HashMap::from"),
            }),
            EvidenceStatus::Asserted,
        ));
        let api_id = next_evidence_id(il);
        il.evidence.push(evidence_with_dependencies(
            api_id,
            EvidenceAnchor::node(sp(), NodeKind::Call),
            EvidenceKind::LibraryApi(LibraryApiEvidenceKind::Contract {
                contract_hash: nose_semantics::library_api_contract_id_hash(contract.id),
                callee_hash: nose_semantics::library_api_callee_contract_hash(contract.callee),
                arity: 1,
            }),
            EvidenceStatus::Asserted,
            vec![EvidenceId(symbol_id)],
        ));
    }

    #[test]
    fn rust_std_map_factory_requires_entry_surface_and_shadow_proof() {
        let (mut il, interner, call) = rust_hash_map_from_call("tuple", false);
        assert!(
            !rust_std_map_factory_call(&il, &interner, call),
            "raw Rust std path proof must not prove the migrated factory"
        );
        push_rust_hash_map_library_api_evidence(&mut il);
        assert!(rust_std_map_factory_call(&il, &interner, call));

        let (mut il, interner, call) = rust_hash_map_from_call("array", false);
        push_rust_hash_map_library_api_evidence(&mut il);
        assert!(
            !rust_std_map_factory_call(&il, &interner, call),
            "HashMap::from exact map proof requires tuple-shaped entries"
        );

        let (mut il, interner, call) = rust_hash_map_from_call("tuple", true);
        push_rust_hash_map_library_api_evidence(&mut il);
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
    fn method_bool_reduction_stops_at_untyped_inner_param_shadow() {
        let (il, interner, call) = typed_method_shadowed_by_untyped_inner_param_il();
        assert!(
            matches!(canon_call(&il, &interner, call), CallCanon::None),
            "an untyped inner parameter must shadow an outer typed parameter"
        );
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
