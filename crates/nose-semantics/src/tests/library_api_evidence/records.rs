use super::*;

pub(crate) fn free_function_builtin_protocol_record(
    id: u32,
    span: Span,
    contract: LibraryFreeFunctionBuiltinContract,
    arity: u16,
    status: EvidenceStatus,
    dependencies: &[u32],
) -> EvidenceRecord {
    library_api_record_with_provenance_and_arity(
        id,
        span,
        contract.id,
        contract.callee,
        arity,
        status,
        dependencies,
        FREE_FUNCTION_BUILTIN_PROTOCOL_PACK_ID,
        FREE_FUNCTION_BUILTIN_PROTOCOL_PRODUCER_ID,
    )
}

pub(crate) fn js_like_builtin_promise_record(
    id: u32,
    span: Span,
    contract_id: LibraryApiContractId,
    callee: LibraryApiCalleeContract,
    status: EvidenceStatus,
    dependencies: &[u32],
) -> EvidenceRecord {
    library_api_record_with_provenance(
        id,
        span,
        contract_id,
        callee,
        status,
        dependencies,
        JS_LIKE_BUILTIN_PROMISE_PACK_ID,
        JS_LIKE_BUILTIN_PROMISE_PRODUCER_ID,
    )
}

pub(crate) fn js_like_builtin_array_record(
    id: u32,
    span: Span,
    contract_id: LibraryApiContractId,
    callee: LibraryApiCalleeContract,
    status: EvidenceStatus,
    dependencies: &[u32],
) -> EvidenceRecord {
    library_api_record_with_provenance(
        id,
        span,
        contract_id,
        callee,
        status,
        dependencies,
        JS_LIKE_BUILTIN_ARRAY_PACK_ID,
        JS_LIKE_BUILTIN_ARRAY_PRODUCER_ID,
    )
}

pub(crate) fn js_like_builtin_boolean_record(
    id: u32,
    span: Span,
    contract_id: LibraryApiContractId,
    callee: LibraryApiCalleeContract,
    status: EvidenceStatus,
    dependencies: &[u32],
) -> EvidenceRecord {
    library_api_record_with_provenance(
        id,
        span,
        contract_id,
        callee,
        status,
        dependencies,
        JS_LIKE_BUILTIN_BOOLEAN_PACK_ID,
        JS_LIKE_BUILTIN_BOOLEAN_PRODUCER_ID,
    )
}

pub(crate) fn js_like_builtin_regex_record(
    id: u32,
    span: Span,
    contract_id: LibraryApiContractId,
    callee: LibraryApiCalleeContract,
    status: EvidenceStatus,
    dependencies: &[u32],
) -> EvidenceRecord {
    library_api_record_with_provenance(
        id,
        span,
        contract_id,
        callee,
        status,
        dependencies,
        JS_LIKE_BUILTIN_REGEX_PACK_ID,
        JS_LIKE_BUILTIN_REGEX_PRODUCER_ID,
    )
}

pub(crate) fn js_like_builtin_static_index_membership_record(
    id: u32,
    span: Span,
    contract_id: LibraryApiContractId,
    callee: LibraryApiCalleeContract,
    status: EvidenceStatus,
    dependencies: &[u32],
) -> EvidenceRecord {
    library_api_record_with_provenance(
        id,
        span,
        contract_id,
        callee,
        status,
        dependencies,
        JS_LIKE_BUILTIN_STATIC_INDEX_MEMBERSHIP_PACK_ID,
        JS_LIKE_BUILTIN_STATIC_INDEX_MEMBERSHIP_PRODUCER_ID,
    )
}

pub(crate) fn js_like_builtin_collection_constructor_record(
    id: u32,
    span: Span,
    contract_id: LibraryApiContractId,
    callee: LibraryApiCalleeContract,
    status: EvidenceStatus,
    dependencies: &[u32],
) -> EvidenceRecord {
    library_api_record_with_provenance(
        id,
        span,
        contract_id,
        callee,
        status,
        dependencies,
        JS_LIKE_BUILTIN_COLLECTION_CONSTRUCTOR_PACK_ID,
        JS_LIKE_BUILTIN_COLLECTION_CONSTRUCTOR_PRODUCER_ID,
    )
}

pub(crate) fn ruby_stdlib_set_record(
    id: u32,
    span: Span,
    contract: LibraryCollectionFactoryContract,
    status: EvidenceStatus,
    dependencies: &[u32],
) -> EvidenceRecord {
    library_api_record_with_provenance(
        id,
        span,
        contract.id,
        contract.callee,
        status,
        dependencies,
        RUBY_STDLIB_SET_PACK_ID,
        RUBY_STDLIB_SET_PRODUCER_ID,
    )
}

pub(crate) fn rust_stdlib_vec_record(
    id: u32,
    span: Span,
    contract: LibraryCollectionFactoryContract,
    arity: u16,
    status: EvidenceStatus,
    dependencies: &[u32],
) -> EvidenceRecord {
    let mut record = library_api_record_with_arity(
        id,
        span,
        contract.id,
        contract.callee,
        arity,
        status,
        dependencies,
    );
    record.provenance.pack_hash = Some(stable_symbol_hash(RUST_STDLIB_VEC_PACK_ID));
    record.provenance.rule_hash = Some(stable_symbol_hash(RUST_STDLIB_VEC_PRODUCER_ID));
    record
}

pub(crate) fn rust_stdlib_option_record(
    id: u32,
    span: Span,
    contract_id: LibraryApiContractId,
    callee: LibraryApiCalleeContract,
    arity: u16,
    status: EvidenceStatus,
    dependencies: &[u32],
) -> EvidenceRecord {
    library_api_record_with_provenance_and_arity(
        id,
        span,
        contract_id,
        callee,
        arity,
        status,
        dependencies,
        RUST_STDLIB_OPTION_PACK_ID,
        RUST_STDLIB_OPTION_PRODUCER_ID,
    )
}

pub(crate) fn rust_stdlib_result_record(
    id: u32,
    span: Span,
    contract_id: LibraryApiContractId,
    callee: LibraryApiCalleeContract,
    arity: u16,
    status: EvidenceStatus,
    dependencies: &[u32],
) -> EvidenceRecord {
    library_api_record_with_provenance_and_arity(
        id,
        span,
        contract_id,
        callee,
        arity,
        status,
        dependencies,
        RUST_STDLIB_RESULT_PACK_ID,
        RUST_STDLIB_RESULT_PRODUCER_ID,
    )
}

pub(crate) fn rust_stdlib_integer_method_record(
    id: u32,
    span: Span,
    contract_id: LibraryApiContractId,
    callee: LibraryApiCalleeContract,
    arity: u16,
    status: EvidenceStatus,
    dependencies: &[u32],
) -> EvidenceRecord {
    library_api_record_with_provenance_and_arity(
        id,
        span,
        contract_id,
        callee,
        arity,
        status,
        dependencies,
        RUST_STDLIB_INTEGER_METHOD_PACK_ID,
        RUST_STDLIB_INTEGER_METHOD_PRODUCER_ID,
    )
}

pub(crate) fn java_stdlib_math_record(
    id: u32,
    span: Span,
    contract_id: LibraryApiContractId,
    callee: LibraryApiCalleeContract,
    arity: u16,
    status: EvidenceStatus,
    dependencies: &[u32],
) -> EvidenceRecord {
    library_api_record_with_provenance_and_arity(
        id,
        span,
        contract_id,
        callee,
        arity,
        status,
        dependencies,
        JAVA_STDLIB_MATH_PACK_ID,
        JAVA_STDLIB_MATH_PRODUCER_ID,
    )
}

pub(crate) fn map_get_protocol_record(
    id: u32,
    span: Span,
    contract: LibraryMapGetContract,
    status: EvidenceStatus,
    dependencies: &[u32],
) -> EvidenceRecord {
    map_get_protocol_record_with_arity(id, span, contract, 1, status, dependencies)
}

pub(crate) fn map_get_protocol_record_with_arity(
    id: u32,
    span: Span,
    contract: LibraryMapGetContract,
    arity: u16,
    status: EvidenceStatus,
    dependencies: &[u32],
) -> EvidenceRecord {
    library_api_record_with_provenance_and_arity(
        id,
        span,
        contract.id,
        contract.callee,
        arity,
        status,
        dependencies,
        MAP_GET_PROTOCOL_PACK_ID,
        MAP_GET_PROTOCOL_PRODUCER_ID,
    )
}

pub(crate) fn map_get_default_protocol_record(
    id: u32,
    span: Span,
    contract: LibraryMethodCallContract,
    status: EvidenceStatus,
    dependencies: &[u32],
) -> EvidenceRecord {
    library_api_record_with_provenance_and_arity(
        id,
        span,
        contract.id,
        contract.callee,
        2,
        status,
        dependencies,
        MAP_GET_DEFAULT_PROTOCOL_PACK_ID,
        MAP_GET_DEFAULT_PROTOCOL_PRODUCER_ID,
    )
}

mod receiver_records;
pub(super) use receiver_records::*;
