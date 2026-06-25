use super::*;

pub(super) fn record_post_lower_free_name_var_library_api<F>(
    il: &mut Il,
    interner: &Interner,
    var: NodeId,
    arity: usize,
    contract_for_name: F,
) -> bool
where
    F: FnOnce(Lang, &str) -> Option<PostLowerLibraryApiContract>,
{
    let Some(name) = post_lower_var_name(il, interner, var) else {
        return false;
    };
    let Some(contract) = contract_for_name(il.meta.lang, name) else {
        return false;
    };
    let LibraryApiCalleeContract::FreeName { name, shadow } = contract.callee else {
        return false;
    };
    if !library_api_free_name_shadow_safe(il.meta.lang, name, shadow, |candidate| {
        post_lower_file_defines_name_visible_at(il, interner, candidate, il.node(var).span)
    }) {
        return false;
    }
    let Some(symbol_dependency) = post_lower_unshadowed_symbol_evidence_id(il, var, name) else {
        return false;
    };
    let api = post_lower_library_api_node_evidence_with_pack_id(
        il,
        var,
        contract.id,
        contract.callee,
        arity,
        contract.pack_id,
        contract.rule,
        vec![symbol_dependency],
    );
    if let Some(domain) = contract.result_domain {
        post_lower_record_library_api_node_result_domain(il, var, domain, api);
    }
    true
}
