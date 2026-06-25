use super::*;

pub fn admitted_free_function_builtin_at_call(
    il: &Il,
    interner: &Interner,
    call: NodeId,
) -> Option<AdmittedLibraryApiCall<LibraryFreeFunctionBuiltinContract>> {
    admitted_free_name_call(il, interner, call, |name, arg_count| {
        library_free_function_builtin_contract(il.meta.lang, name, arg_count)
    })
}

pub fn admitted_free_function_hof_at_call(
    il: &Il,
    interner: &Interner,
    call: NodeId,
) -> Option<AdmittedLibraryApiCall<LibraryFreeFunctionHofContract>> {
    admitted_free_name_call(il, interner, call, |name, arg_count| {
        library_free_function_hof_contract(il.meta.lang, name, arg_count)
    })
}
