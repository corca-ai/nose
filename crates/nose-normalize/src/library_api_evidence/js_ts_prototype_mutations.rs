use super::*;

pub(super) fn js_ts_builtin_prototype_mutated_in_file(
    il: &Il,
    interner: &Interner,
    id: LibraryApiContractId,
    callee: LibraryApiCalleeContract,
) -> bool {
    nose_semantics::js_ts_builtin_prototype_mutated_in_file(
        il,
        interner,
        id,
        callee,
        top_level_statements_for(il),
        module_unshadowed_var_name,
    )
}

fn module_unshadowed_var_name(il: &Il, interner: &Interner, node: NodeId, expected: &str) -> bool {
    node_name(il, interner, node) == Some(expected)
        && !file_defines_name_visible_at(il, interner, expected, il.node(node).span)
}
