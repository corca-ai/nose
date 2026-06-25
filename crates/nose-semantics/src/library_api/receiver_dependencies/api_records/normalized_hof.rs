use super::canonical::canonical_record_has_unshadowed_symbol_dependency;
use super::receiver_proofs::receiver_contract_dependency_match;
use super::*;

pub(super) fn normalized_hof_method_call_dependencies_match(
    il: &Il,
    hof: NodeId,
    record: &EvidenceRecord,
    contract: LibraryMethodCallContract,
) -> bool {
    let Some(&receiver) = il.children(hof).first() else {
        return false;
    };
    receiver_contract_dependency_match(il, record, receiver, contract.result.receiver)
}

pub(super) fn normalized_hof_free_function_dependencies_match(
    il: &Il,
    hof: NodeId,
    record: &EvidenceRecord,
    name: &str,
) -> bool {
    let Some(&source) = il.children(hof).first() else {
        return false;
    };
    canonical_record_has_unshadowed_symbol_dependency(il, hof, record, name)
        && receiver_contract_dependency_match(
            il,
            record,
            source,
            MethodReceiverContract::ExactProtocol,
        )
}
