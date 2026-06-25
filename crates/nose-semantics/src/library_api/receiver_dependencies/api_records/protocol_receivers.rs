use super::canonical::library_api_method_call_record_contract;
use super::*;

pub(super) fn library_api_dependency_contract_satisfies_protocol_receiver(
    il: &Il,
    actual_id: LibraryApiContractId,
    callee: LibraryApiCalleeContract,
    arity: u16,
    receiver: MethodReceiverContract,
) -> bool {
    match actual_id {
        LibraryApiContractId::MapKeyView(kind) => map_key_view_satisfies_receiver(kind, receiver),
        LibraryApiContractId::IteratorIdentityAdapter
        | LibraryApiContractId::StaticCollectionAdapter => {
            protocol_api_satisfies_receiver(receiver)
        }
        LibraryApiContractId::FreeFunctionHof(HoFKind::Map | HoFKind::Filter)
        | LibraryApiContractId::FreeFunctionBuiltin(Builtin::Zip | Builtin::Enumerate) => {
            protocol_api_satisfies_receiver(receiver)
        }
        LibraryApiContractId::MethodCall(
            MethodSemanticContract::HoF(_) | MethodSemanticContract::Builtin(Builtin::Zip),
        ) => {
            protocol_api_satisfies_receiver(receiver)
                && library_api_method_call_record_contract(il, actual_id, callee, arity).is_some()
        }
        _ => false,
    }
}

fn protocol_api_satisfies_receiver(receiver: MethodReceiverContract) -> bool {
    matches!(
        receiver,
        MethodReceiverContract::ExactProtocol | MethodReceiverContract::ExactProtocolPairArgument
    )
}

fn map_key_view_satisfies_receiver(kind: MapKeyViewKind, receiver: MethodReceiverContract) -> bool {
    match kind {
        MapKeyViewKind::Collection => matches!(
            receiver,
            MethodReceiverContract::ExactCollection
                | MethodReceiverContract::ExactCollectionOrMap
                | MethodReceiverContract::ExactCollectionOrJavaKeySet
                | MethodReceiverContract::ExactProtocol
                | MethodReceiverContract::ExactProtocolPairArgument
        ),
        MapKeyViewKind::Iterator => protocol_api_satisfies_receiver(receiver),
    }
}
