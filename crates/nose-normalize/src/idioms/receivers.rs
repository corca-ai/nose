use super::*;

#[derive(Clone, Copy)]
pub(super) enum ProvenReceiver {
    Direct(NodeId),
    MapGet { map: NodeId, key: NodeId },
}

pub(super) fn prove_method_receiver(
    old: &Il,
    interner: &Interner,
    domains: &ReceiverDomainEvidenceIndex<'_>,
    contract: MethodReceiverContract,
    base: NodeId,
    args: &[NodeId],
) -> Option<ProvenReceiver> {
    match contract {
        MethodReceiverContract::ExactArray => exact_array_receiver(old, interner, domains, base)
            .then_some(ProvenReceiver::Direct(base)),
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
        MethodReceiverContract::ExactResult => domains
            .receiver_satisfies_domain(base, DomainRequirement::RESULT)
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
        MethodReceiverContract::ExactSetOrMap => (exact_set_param(domains, base)
            || exact_map_param(domains, base))
        .then_some(ProvenReceiver::Direct(base)),
        MethodReceiverContract::LiteralString => {
            literal_string_receiver(old, domains, base).then_some(ProvenReceiver::Direct(base))
        }
        MethodReceiverContract::UnshadowedGlobal(global) => {
            asserted_unshadowed_global_symbol(old, base, global)
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

pub(super) fn apply_method_contract(
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
        MethodBuiltinArgs::ReceiverOnly => receiver_only_args(op, receiver, direct),
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
            map_get_default_or_zero_arg_lambda_args(old, op, direct, args)
        }
        MethodBuiltinArgs::RustMapGetOrOptionDefault => {
            rust_map_get_or_option_default_args(op, receiver, args)
        }
        MethodBuiltinArgs::RustOptionDefaultLambda => {
            rust_option_default_lambda_args(old, op, direct, args)
        }
        MethodBuiltinArgs::RustOptionMapOrIdentity => {
            rust_option_map_or_identity_args(old, op, direct, args)
        }
        MethodBuiltinArgs::RustZip => rust_zip_args(old, interner, op, direct, args),
        MethodBuiltinArgs::Fold => fold_args(old, interner, op, direct, args),
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
            collection_reduction_args(old, interner, op, direct)
        }
    }
}
