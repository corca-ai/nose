//! Cross-language idiom canonicalization: collapse equivalent builtins from
//! different languages to one [`Builtin`] op so that, e.g., Python `len(xs)`,
//! JS `xs.length`, and Go `len(xs)` all converge. Detection on `xs.length` (a
//! field access) is handled by the caller; this module only inspects `Call`s.
//!
//! proof-obligation: normalize.value_graph.bool_reduce
//! proof-obligation: normalize.value_graph.functor
//! proof-obligation: normalize.value_graph.min_max

use nose_il::{Builtin, HoFKind, Il, Interner, NodeId, NodeKind, Payload};
use nose_semantics::{
    admitted_free_function_builtin_at_call, admitted_free_function_hof_at_call,
    admitted_free_name_map_factory_at_call, admitted_iterator_identity_adapter_at_call,
    admitted_library_method_call_at_call, admitted_map_get_at_call, admitted_map_key_view_at_call,
    admitted_rust_option_some_constructor_at_call, admitted_static_collection_adapter_at_call,
    asserted_unshadowed_global_symbol, imported_namespace_symbol, seq_surface_contract_for_node,
    BuiltinArgContract, DomainRequirement, LibraryMapFactoryResult, MapKeyViewKind,
    MethodBuiltinArgs, MethodCallContract, MethodReceiverContract, MethodSemanticContract,
    ReceiverDomainEvidenceIndex, SEQ_VALUE_COLLECTION, SEQ_VALUE_MAP,
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

#[cfg(test)]
fn canon_call(old: &Il, interner: &Interner, call_id: NodeId) -> CallCanon {
    let domains = ReceiverDomainEvidenceIndex::new(old, interner);
    canon_call_with_domains(old, interner, &domains, call_id)
}

pub(crate) fn canon_call_with_domains(
    old: &Il,
    interner: &Interner,
    domains: &ReceiverDomainEvidenceIndex<'_>,
    call_id: NodeId,
) -> CallCanon {
    let kids = old.children(call_id);
    let Some((_callee, args)) = kids.split_first() else {
        return CallCanon::None;
    };
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
    if let Some(admitted) = admitted_free_function_builtin_at_call(old, interner, call_id) {
        let contract = admitted.contract;
        match contract.result.args {
            BuiltinArgContract::First => builtin!(contract.result.builtin, first),
            BuiltinArgContract::All => builtin!(contract.result.builtin, all),
        }
    }
    if let Some(admitted) = admitted_free_function_hof_at_call(old, interner, call_id) {
        let contract = admitted.contract.result;
        let Some(&collection_old) = args.get(contract.source_arg) else {
            return CallCanon::None;
        };
        let Some(&fn_old) = args.get(contract.callback_arg) else {
            return CallCanon::None;
        };
        if old.kind(fn_old) != NodeKind::Lambda {
            return CallCanon::None;
        }
        return CallCanon::HoF {
            kind: contract.kind,
            collection_old,
            fn_old,
        };
    }
    if let Some(admitted) = admitted_library_method_call_at_call(old, interner, call_id) {
        let Some(base) = admitted.receiver else {
            return CallCanon::None;
        };
        let contract = admitted.contract.result;
        let Some(proven) =
            prove_method_receiver(old, interner, domains, contract.receiver, base, args)
        else {
            return CallCanon::None;
        };
        return apply_method_contract(old, interner, contract, proven, args);
    }
    CallCanon::None
}

mod arguments;
mod map_surfaces;
mod receiver_evidence;
mod receivers;

use arguments::*;
use map_surfaces::*;
use receiver_evidence::*;
use receivers::*;

#[cfg(test)]
mod tests;
