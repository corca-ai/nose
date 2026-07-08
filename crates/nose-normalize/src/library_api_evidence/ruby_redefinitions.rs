use super::*;

pub(super) fn ruby_sequence_hof_method_redefined_in_file(
    il: &Il,
    interner: &Interner,
    id: LibraryApiContractId,
    callee: LibraryApiCalleeContract,
) -> bool {
    if il.meta.lang != Lang::Ruby {
        return false;
    }
    let LibraryApiContractId::MethodCall(
        MethodSemanticContract::HoF(HoFKind::Map | HoFKind::Filter | HoFKind::Reject)
        | MethodSemanticContract::Builtin(Builtin::Any | Builtin::All),
    ) = id
    else {
        return false;
    };
    let LibraryApiCalleeContract::Method {
        method,
        receiver: MethodReceiverContract::ExactArrayOrCollection,
    } = callee
    else {
        return false;
    };
    ruby_class_instance_method_redefined_in_file(
        il,
        interner,
        &["Array", "::Array", "Enumerable", "::Enumerable"],
        method,
    )
}

fn ruby_class_instance_method_redefined_in_file(
    il: &Il,
    interner: &Interner,
    class_names: &[&str],
    expected_method: &str,
) -> bool {
    nose_semantics::ruby_class_instance_method_redefined_in_file(
        il,
        interner,
        class_names,
        expected_method,
        node_name,
    )
}
