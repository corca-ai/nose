use crate::library_api::contracts::{
    library_iterator_identity_adapter_result_domain, library_receiver_method_api_result_domain,
};

use super::*;

pub fn library_receiver_method_api_contract(
    lang: Lang,
    method: &str,
    arg_count: usize,
) -> Option<LibraryReceiverMethodApiContract> {
    library_receiver_method_api_contracts(lang, method, arg_count)
        .into_iter()
        .next()
}

pub fn library_receiver_method_api_contracts(
    lang: Lang,
    method: &str,
    arg_count: usize,
) -> Vec<LibraryReceiverMethodApiContract> {
    let mut contracts = Vec::new();
    contracts.extend(receiver_map_get_api_contract(lang, method, arg_count));
    contracts.extend(receiver_map_key_view_api_contract(lang, method, arg_count));
    contracts.extend(receiver_iterator_adapter_api_contract(
        lang, method, arg_count,
    ));
    contracts.extend(receiver_scalar_integer_api_contract(
        lang, method, arg_count,
    ));
    contracts.extend(receiver_rust_option_api_contract(lang, method, arg_count));
    contracts.extend(receiver_rust_result_api_contract(lang, method, arg_count));
    contracts.extend(receiver_promise_api_contract(lang, method, arg_count));
    contracts.extend(receiver_map_get_default_api_contract(
        lang, method, arg_count,
    ));
    contracts.extend(receiver_membership_api_contract(lang, method, arg_count));
    contracts.extend(receiver_builtin_method_api_contracts(
        lang, method, arg_count,
    ));
    contracts
}

fn receiver_map_get_api_contract(
    lang: Lang,
    method: &str,
    arg_count: usize,
) -> Option<LibraryReceiverMethodApiContract> {
    library_map_get_contract(lang, method, arg_count).map(|contract| {
        receiver_method_api_contract(
            contract.pack_id,
            contract.id,
            contract.callee,
            MAP_GET_PROTOCOL_PRODUCER_ID,
        )
    })
}

fn receiver_map_key_view_api_contract(
    lang: Lang,
    method: &str,
    arg_count: usize,
) -> Option<LibraryReceiverMethodApiContract> {
    library_map_key_view_contract(lang, method, arg_count).map(|contract| {
        receiver_method_api_contract(
            contract.pack_id,
            contract.id,
            contract.callee,
            MAP_KEY_VIEW_PROTOCOL_PRODUCER_ID,
        )
    })
}

fn receiver_iterator_adapter_api_contract(
    lang: Lang,
    method: &str,
    arg_count: usize,
) -> Option<LibraryReceiverMethodApiContract> {
    library_iterator_identity_adapter_contract(lang, method, arg_count).map(|contract| {
        LibraryReceiverMethodApiContract {
            pack_id: contract.pack_id,
            id: contract.id,
            callee: contract.callee,
            rule: ITERATOR_IDENTITY_ADAPTER_PRODUCER_ID,
            result_domain: library_iterator_identity_adapter_result_domain(
                contract.callee,
                arg_count,
            ),
        }
    })
}

fn receiver_scalar_integer_api_contract(
    lang: Lang,
    method: &str,
    arg_count: usize,
) -> Option<LibraryReceiverMethodApiContract> {
    library_scalar_integer_method_contract(lang, method, arg_count).map(|contract| {
        let rule = match contract.pack_id {
            RUST_STDLIB_INTEGER_METHOD_PACK_ID => RUST_STDLIB_INTEGER_METHOD_PRODUCER_ID,
            JAVA_STDLIB_MATH_PACK_ID => JAVA_STDLIB_MATH_PRODUCER_ID,
            _ => "library_api_scalar_integer_method",
        };
        receiver_method_api_contract(contract.pack_id, contract.id, contract.callee, rule)
    })
}

fn receiver_rust_option_api_contract(
    lang: Lang,
    method: &str,
    arg_count: usize,
) -> Option<LibraryReceiverMethodApiContract> {
    library_rust_option_and_then_contract(lang, method, arg_count).map(|contract| {
        receiver_method_api_contract(
            contract.pack_id,
            contract.id,
            contract.callee,
            RUST_STDLIB_OPTION_PRODUCER_ID,
        )
    })
}

fn receiver_rust_result_api_contract(
    lang: Lang,
    method: &str,
    arg_count: usize,
) -> Option<LibraryReceiverMethodApiContract> {
    library_rust_result_predicate_contract(lang, method, arg_count)
}

fn receiver_promise_api_contract(
    lang: Lang,
    method: &str,
    arg_count: usize,
) -> Option<LibraryReceiverMethodApiContract> {
    library_promise_then_contract(lang, method, arg_count).map(|contract| {
        receiver_method_api_contract(
            contract.pack_id,
            contract.id,
            contract.callee,
            JS_LIKE_BUILTIN_PROMISE_PRODUCER_ID,
        )
    })
}

fn receiver_map_get_default_api_contract(
    lang: Lang,
    method: &str,
    arg_count: usize,
) -> Option<LibraryReceiverMethodApiContract> {
    library_map_get_default_contract(lang, method, arg_count).map(|contract| {
        receiver_method_api_contract(
            MAP_GET_DEFAULT_PROTOCOL_PACK_ID,
            contract.id,
            contract.callee,
            MAP_GET_DEFAULT_PROTOCOL_PRODUCER_ID,
        )
    })
}

fn receiver_membership_api_contract(
    lang: Lang,
    method: &str,
    arg_count: usize,
) -> Option<LibraryReceiverMethodApiContract> {
    library_receiver_membership_contract(lang, method, arg_count).map(|contract| {
        receiver_method_api_contract(
            RECEIVER_MEMBERSHIP_PROTOCOL_PACK_ID,
            contract.id,
            contract.callee,
            RECEIVER_MEMBERSHIP_PROTOCOL_PRODUCER_ID,
        )
    })
}

fn receiver_builtin_method_api_contracts(
    lang: Lang,
    method: &str,
    arg_count: usize,
) -> Vec<LibraryReceiverMethodApiContract> {
    library_method_call_contracts(lang, method, arg_count)
        .into_iter()
        .map(|contract| {
            receiver_method_api_contract(
                contract.pack_id,
                contract.id,
                contract.callee,
                contract.producer_id,
            )
        })
        .collect()
}

fn receiver_method_api_contract(
    pack_id: &'static str,
    id: LibraryApiContractId,
    callee: LibraryApiCalleeContract,
    rule: &'static str,
) -> LibraryReceiverMethodApiContract {
    LibraryReceiverMethodApiContract {
        pack_id,
        id,
        callee,
        rule,
        result_domain: library_receiver_method_api_result_domain(id),
    }
}
