use super::*;

pub(super) fn post_lower_rust_result_constructor_contract(
    lang: Lang,
    callee_name: &str,
    arg_count: usize,
) -> Option<PostLowerLibraryApiContract> {
    library_rust_result_ok_constructor_contract(lang, callee_name, arg_count)
        .or_else(|| library_rust_result_err_constructor_contract(lang, callee_name, arg_count))
        .map(|contract| PostLowerLibraryApiContract {
            id: contract.id,
            callee: contract.callee,
            pack_id: contract.pack_id,
            rule: RUST_STDLIB_RESULT_PRODUCER_ID,
            result_domain: Some(contract.result_domain),
        })
}

pub(super) fn record_post_lower_rust_result_pattern_library_api(
    il: &mut Il,
    interner: &Interner,
    var: NodeId,
) -> bool {
    record_post_lower_free_name_var_library_api(il, interner, var, 1, |lang, name| {
        library_rust_result_ok_constructor_contract(lang, name, 1)
            .or_else(|| library_rust_result_err_constructor_contract(lang, name, 1))
            .map(|contract| PostLowerLibraryApiContract {
                id: contract.id,
                callee: contract.callee,
                pack_id: contract.pack_id,
                rule: RUST_STDLIB_RESULT_PRODUCER_ID,
                result_domain: None,
            })
    })
}
