use super::*;

pub(super) fn record_post_lower_java_collection_constructor_library_api(
    il: &mut Il,
    interner: &Interner,
    call: NodeId,
) -> bool {
    let kids = il.children(call);
    let Some((&callee, args)) = kids.split_first() else {
        return false;
    };
    let arg_count = args.len();
    let Some(type_name) = post_lower_var_name(il, interner, callee) else {
        return false;
    };
    let Some(contract) =
        library_java_collection_constructor_contract(il.meta.lang, type_name, arg_count)
    else {
        return false;
    };
    let LibraryApiCalleeContract::JavaUtilConstructor { type_ref } = contract.callee else {
        return false;
    };
    let Some(source_dependency) =
        post_lower_source_call_evidence_id(il, call, SourceCallKind::Construct)
    else {
        return false;
    };
    let mut dependencies = vec![source_dependency];
    if type_name == type_ref.simple_type {
        if !type_ref.simple_name_is_allowed() {
            return false;
        }
        if type_ref.simple_name_rejects_local_shadow()
            && post_lower_unit_defines_name(
                il,
                interner,
                type_ref.simple_type,
                il.node(callee).span,
            )
        {
            return false;
        }
        if type_ref.simple_name_requires_import() {
            if let Some(dependency) = post_lower_imported_binding_symbol_evidence_id(
                il,
                interner,
                callee,
                type_ref.module,
                type_ref.simple_type,
            ) {
                dependencies.push(dependency);
            } else {
                let Some(dependency) = post_lower_java_wildcard_import_evidence_id(
                    il,
                    interner,
                    type_ref.module,
                    type_ref.simple_type,
                    il.node(call).span,
                ) else {
                    return false;
                };
                dependencies.push(dependency);
            }
        }
    } else if !type_ref.matches_qualified_name(type_name) {
        return false;
    }
    let api = post_lower_library_api_evidence_with_pack_id(
        il,
        call,
        contract.id,
        contract.callee,
        arg_count,
        JAVA_STDLIB_COLLECTION_CONSTRUCTOR_PACK_ID,
        JAVA_STDLIB_COLLECTION_CONSTRUCTOR_PRODUCER_ID,
        dependencies,
    );
    post_lower_record_library_api_result_domain(
        il,
        call,
        library_collection_factory_result_domain_for_arity(contract, arg_count),
        api,
    );
    true
}
