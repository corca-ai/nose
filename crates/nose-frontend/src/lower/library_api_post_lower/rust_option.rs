use super::*;

pub(super) fn record_post_lower_rust_option_none_library_api(
    il: &mut Il,
    interner: &Interner,
    var: NodeId,
) -> bool {
    record_post_lower_free_name_var_library_api(il, interner, var, 0, |lang, name| {
        library_rust_option_none_sentinel_contract(lang, name).map(|contract| {
            PostLowerLibraryApiContract {
                id: contract.id,
                callee: contract.callee,
                pack_id: contract.pack_id,
                rule: RUST_STDLIB_OPTION_PRODUCER_ID,
                result_domain: Some(contract.result_domain),
            }
        })
    })
}

pub(super) fn record_post_lower_rust_option_some_pattern_library_api(
    il: &mut Il,
    interner: &Interner,
    var: NodeId,
) -> bool {
    record_post_lower_free_name_var_library_api(il, interner, var, 1, |lang, name| {
        library_rust_option_some_constructor_contract(lang, name, 1).map(|contract| {
            PostLowerLibraryApiContract {
                id: contract.id,
                callee: contract.callee,
                pack_id: contract.pack_id,
                rule: RUST_STDLIB_OPTION_PRODUCER_ID,
                result_domain: None,
            }
        })
    })
}
