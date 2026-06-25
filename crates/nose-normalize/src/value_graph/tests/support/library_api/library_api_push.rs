use super::*;

pub(crate) fn push_method_call_library_api_evidence(
    il: &mut Il,
    interner: &Interner,
    id: u32,
    call: NodeId,
    method: &str,
    arity: usize,
) {
    let contract =
        library_method_call_contract(il.meta.lang, method, arity).expect("method contract");
    let dependencies = nose_semantics::library_api_receiver_dependencies_for_call(
        il,
        interner,
        call,
        contract.callee,
    )
    .expect("method receiver dependencies");
    let mut record = library_api_contract_evidence(
        id,
        il.node(call).span,
        contract.id,
        contract.callee,
        arity as u16,
        dependencies,
    );
    if is_map_get_default_method_call(contract) {
        record.provenance.pack_hash = Some(stable_symbol_hash(MAP_GET_DEFAULT_PROTOCOL_PACK_ID));
        record.provenance.rule_hash =
            Some(stable_symbol_hash(MAP_GET_DEFAULT_PROTOCOL_PRODUCER_ID));
    } else if is_receiver_membership_method_call(contract) {
        record.provenance.pack_hash =
            Some(stable_symbol_hash(RECEIVER_MEMBERSHIP_PROTOCOL_PACK_ID));
        record.provenance.rule_hash =
            Some(stable_symbol_hash(RECEIVER_MEMBERSHIP_PROTOCOL_PRODUCER_ID));
    } else if is_sequence_hof_method_call(il.meta.lang, contract.id, contract.callee) {
        record.provenance.pack_hash =
            Some(stable_symbol_hash(SEQUENCE_HOF_ADAPTER_PROTOCOL_PACK_ID));
        record.provenance.rule_hash = Some(stable_symbol_hash(
            SEQUENCE_HOF_ADAPTER_PROTOCOL_PRODUCER_ID,
        ));
    } else {
        record.provenance.pack_hash =
            Some(stable_symbol_hash(BUILTIN_METHOD_CALL_PROTOCOL_PACK_ID));
        record.provenance.rule_hash =
            Some(stable_symbol_hash(BUILTIN_METHOD_CALL_PROTOCOL_PRODUCER_ID));
    }
    il.evidence.push(record);
}

fn is_receiver_membership_method_call(contract: LibraryMethodCallContract) -> bool {
    contract.id
        == LibraryApiContractId::MethodCall(MethodSemanticContract::Builtin(Builtin::Contains))
        && matches!(
            contract.callee,
            LibraryApiCalleeContract::Method {
                receiver: MethodReceiverContract::ExactMap
                    | MethodReceiverContract::ExactCollectionOrMap
                    | MethodReceiverContract::ExactCollectionOrJavaKeySet
                    | MethodReceiverContract::ExactSetOrMap,
                ..
            }
        )
        && contract.result.args == MethodBuiltinArgs::FirstThenReceiver
}

fn is_map_get_default_method_call(contract: LibraryMethodCallContract) -> bool {
    contract.id
        == LibraryApiContractId::MethodCall(MethodSemanticContract::Builtin(Builtin::GetOrDefault))
        && matches!(
            contract.callee,
            LibraryApiCalleeContract::Method {
                receiver: MethodReceiverContract::ExactMap,
                ..
            }
        )
}

fn is_sequence_hof_method_call(
    lang: Lang,
    contract_id: LibraryApiContractId,
    callee: LibraryApiCalleeContract,
) -> bool {
    match lang {
        Lang::Rust => matches!(
            (contract_id, callee),
            (
                LibraryApiContractId::MethodCall(MethodSemanticContract::HoF(
                    HoFKind::Map | HoFKind::Filter | HoFKind::FilterMap | HoFKind::FlatMap,
                )),
                LibraryApiCalleeContract::Method {
                    method: "map" | "filter" | "filter_map" | "flat_map",
                    receiver: MethodReceiverContract::ExactProtocol,
                },
            ) | (
                LibraryApiContractId::MethodCall(MethodSemanticContract::Builtin(
                    Builtin::Any | Builtin::All,
                )),
                LibraryApiCalleeContract::Method {
                    method: "any" | "all",
                    receiver: MethodReceiverContract::ExactProtocol,
                },
            ) | (
                LibraryApiContractId::MethodCall(MethodSemanticContract::Builtin(Builtin::Len)),
                LibraryApiCalleeContract::Method {
                    method: "count",
                    receiver: MethodReceiverContract::ExactProtocol,
                },
            )
        ),
        Lang::Swift => matches!(
            (contract_id, callee),
            (
                LibraryApiContractId::MethodCall(MethodSemanticContract::HoF(
                    HoFKind::Map | HoFKind::Filter | HoFKind::FlatMap,
                )),
                LibraryApiCalleeContract::Method {
                    method: "map" | "filter" | "flatMap",
                    receiver: MethodReceiverContract::ExactArrayOrCollection,
                },
            )
        ),
        _ => false,
    }
}

pub(crate) fn push_library_api_evidence_for_callee(
    il: &mut Il,
    interner: &Interner,
    id: u32,
    call: NodeId,
    contract_id: LibraryApiContractId,
    callee: LibraryApiCalleeContract,
    arity: u16,
) {
    let dependencies =
        nose_semantics::library_api_receiver_dependencies_for_call(il, interner, call, callee)
            .expect("library api receiver dependencies");
    let record = if matches!(contract_id, LibraryApiContractId::ScalarIntegerMethod(_))
        && matches!(
            callee,
            LibraryApiCalleeContract::Method {
                receiver: MethodReceiverContract::ExactInteger,
                ..
            }
        ) {
        rust_stdlib_integer_method_evidence(
            id,
            il.node(call).span,
            contract_id,
            callee,
            arity,
            dependencies,
        )
    } else if matches!(contract_id, LibraryApiContractId::ScalarIntegerMethod(_))
        && matches!(
            callee,
            LibraryApiCalleeContract::Method {
                receiver: MethodReceiverContract::UnshadowedGlobal("Math"),
                ..
            }
        )
    {
        java_stdlib_math_evidence(
            id,
            il.node(call).span,
            contract_id,
            callee,
            arity,
            dependencies,
        )
    } else if matches!(contract_id, LibraryApiContractId::MapGet) {
        map_get_protocol_evidence(
            id,
            il.node(call).span,
            contract_id,
            callee,
            arity,
            dependencies,
        )
    } else if matches!(contract_id, LibraryApiContractId::MapKeyView(_)) {
        map_key_view_protocol_evidence(
            id,
            il.node(call).span,
            contract_id,
            callee,
            arity,
            dependencies,
        )
    } else if is_sequence_hof_method_call(il.meta.lang, contract_id, callee) {
        rust_sequence_hof_adapter_evidence(
            id,
            il.node(call).span,
            contract_id,
            callee,
            arity,
            dependencies,
        )
    } else if matches!(contract_id, LibraryApiContractId::MethodCall(_)) {
        let mut record = library_api_contract_evidence(
            id,
            il.node(call).span,
            contract_id,
            callee,
            arity,
            dependencies,
        );
        record.provenance.pack_hash =
            Some(stable_symbol_hash(BUILTIN_METHOD_CALL_PROTOCOL_PACK_ID));
        record.provenance.rule_hash =
            Some(stable_symbol_hash(BUILTIN_METHOD_CALL_PROTOCOL_PRODUCER_ID));
        record
    } else {
        library_api_contract_evidence(
            id,
            il.node(call).span,
            contract_id,
            callee,
            arity,
            dependencies,
        )
    };
    il.evidence.push(record);
}
