use super::*;

trait LibraryApiFixtureContract: Copy {
    fn fixture_contract_id(self) -> LibraryApiContractId;
    fn fixture_callee(self) -> LibraryApiCalleeContract;
}

macro_rules! impl_fixture_contract {
    ($($contract:ty),+ $(,)?) => {
        $(
            impl LibraryApiFixtureContract for $contract {
                fn fixture_contract_id(self) -> LibraryApiContractId {
                    self.id
                }

                fn fixture_callee(self) -> LibraryApiCalleeContract {
                    self.callee
                }
            }
        )+
    };
}

impl_fixture_contract!(
    LibraryCollectionFactoryContract,
    LibraryFreeFunctionBuiltinContract,
    LibraryMapEntryFactoryContract,
    LibraryMapFactoryContract,
    LibraryMapGetContract,
    LibraryMapKeyViewContract,
    LibraryMethodCallContract,
    LibraryStaticCollectionAdapterContract,
);

#[derive(Clone, Copy)]
struct LibraryApiFixturePack {
    pack_id: &'static str,
    producer_id: &'static str,
}

impl LibraryApiFixturePack {
    fn contract_record<C: LibraryApiFixtureContract>(
        self,
        id: u32,
        span: Span,
        contract: C,
        status: EvidenceStatus,
        dependencies: &[u32],
    ) -> EvidenceRecord {
        self.contract_record_with_arity(id, span, contract, 1, status, dependencies)
    }

    fn contract_record_with_arity<C: LibraryApiFixtureContract>(
        self,
        id: u32,
        span: Span,
        contract: C,
        arity: u16,
        status: EvidenceStatus,
        dependencies: &[u32],
    ) -> EvidenceRecord {
        self.parts_record_with_arity(
            id,
            span,
            contract.fixture_contract_id(),
            contract.fixture_callee(),
            arity,
            status,
            dependencies,
        )
    }

    fn parts_record(
        self,
        id: u32,
        span: Span,
        contract_id: LibraryApiContractId,
        callee: LibraryApiCalleeContract,
        status: EvidenceStatus,
        dependencies: &[u32],
    ) -> EvidenceRecord {
        self.parts_record_with_arity(id, span, contract_id, callee, 1, status, dependencies)
    }

    fn parts_record_with_arity(
        self,
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
            self.pack_id,
            self.producer_id,
        )
    }
}

macro_rules! library_api_fixture_pack {
    ($pack_id:expr, $producer_id:expr) => {
        LibraryApiFixturePack {
            pack_id: $pack_id,
            producer_id: $producer_id,
        }
    };
}

pub(crate) fn free_function_builtin_protocol_record(
    id: u32,
    span: Span,
    contract: LibraryFreeFunctionBuiltinContract,
    arity: u16,
    status: EvidenceStatus,
    dependencies: &[u32],
) -> EvidenceRecord {
    library_api_fixture_pack!(
        FREE_FUNCTION_BUILTIN_PROTOCOL_PACK_ID,
        FREE_FUNCTION_BUILTIN_PROTOCOL_PRODUCER_ID
    )
    .contract_record_with_arity(id, span, contract, arity, status, dependencies)
}

pub(crate) fn python_iterator_builtin_protocol_record(
    id: u32,
    span: Span,
    contract_id: LibraryApiContractId,
    callee: LibraryApiCalleeContract,
    arity: u16,
    status: EvidenceStatus,
    dependencies: &[u32],
) -> EvidenceRecord {
    library_api_fixture_pack!(
        PYTHON_ITERATOR_BUILTIN_PROTOCOL_PACK_ID,
        PYTHON_ITERATOR_BUILTIN_PROTOCOL_PRODUCER_ID
    )
    .parts_record_with_arity(id, span, contract_id, callee, arity, status, dependencies)
}

pub(crate) fn js_like_builtin_promise_record(
    id: u32,
    span: Span,
    contract_id: LibraryApiContractId,
    callee: LibraryApiCalleeContract,
    status: EvidenceStatus,
    dependencies: &[u32],
) -> EvidenceRecord {
    library_api_fixture_pack!(
        JS_LIKE_BUILTIN_PROMISE_PACK_ID,
        JS_LIKE_BUILTIN_PROMISE_PRODUCER_ID
    )
    .parts_record(id, span, contract_id, callee, status, dependencies)
}

pub(crate) fn js_like_builtin_array_record(
    id: u32,
    span: Span,
    contract_id: LibraryApiContractId,
    callee: LibraryApiCalleeContract,
    status: EvidenceStatus,
    dependencies: &[u32],
) -> EvidenceRecord {
    library_api_fixture_pack!(
        JS_LIKE_BUILTIN_ARRAY_PACK_ID,
        JS_LIKE_BUILTIN_ARRAY_PRODUCER_ID
    )
    .parts_record(id, span, contract_id, callee, status, dependencies)
}

pub(crate) fn js_like_builtin_boolean_record(
    id: u32,
    span: Span,
    contract_id: LibraryApiContractId,
    callee: LibraryApiCalleeContract,
    status: EvidenceStatus,
    dependencies: &[u32],
) -> EvidenceRecord {
    library_api_fixture_pack!(
        JS_LIKE_BUILTIN_BOOLEAN_PACK_ID,
        JS_LIKE_BUILTIN_BOOLEAN_PRODUCER_ID
    )
    .parts_record(id, span, contract_id, callee, status, dependencies)
}

pub(crate) fn js_like_builtin_regex_record(
    id: u32,
    span: Span,
    contract_id: LibraryApiContractId,
    callee: LibraryApiCalleeContract,
    status: EvidenceStatus,
    dependencies: &[u32],
) -> EvidenceRecord {
    library_api_fixture_pack!(
        JS_LIKE_BUILTIN_REGEX_PACK_ID,
        JS_LIKE_BUILTIN_REGEX_PRODUCER_ID
    )
    .parts_record(id, span, contract_id, callee, status, dependencies)
}

pub(crate) fn js_like_builtin_static_index_membership_record(
    id: u32,
    span: Span,
    contract_id: LibraryApiContractId,
    callee: LibraryApiCalleeContract,
    status: EvidenceStatus,
    dependencies: &[u32],
) -> EvidenceRecord {
    library_api_fixture_pack!(
        JS_LIKE_BUILTIN_STATIC_INDEX_MEMBERSHIP_PACK_ID,
        JS_LIKE_BUILTIN_STATIC_INDEX_MEMBERSHIP_PRODUCER_ID
    )
    .parts_record(id, span, contract_id, callee, status, dependencies)
}

pub(crate) fn js_like_builtin_collection_constructor_record(
    id: u32,
    span: Span,
    contract_id: LibraryApiContractId,
    callee: LibraryApiCalleeContract,
    status: EvidenceStatus,
    dependencies: &[u32],
) -> EvidenceRecord {
    library_api_fixture_pack!(
        JS_LIKE_BUILTIN_COLLECTION_CONSTRUCTOR_PACK_ID,
        JS_LIKE_BUILTIN_COLLECTION_CONSTRUCTOR_PRODUCER_ID
    )
    .parts_record(id, span, contract_id, callee, status, dependencies)
}

pub(crate) fn python_builtin_collection_factory_record(
    id: u32,
    span: Span,
    contract: LibraryCollectionFactoryContract,
    status: EvidenceStatus,
    dependencies: &[u32],
) -> EvidenceRecord {
    library_api_fixture_pack!(
        PYTHON_BUILTIN_COLLECTION_FACTORY_PACK_ID,
        PYTHON_BUILTIN_COLLECTION_FACTORY_PRODUCER_ID
    )
    .contract_record(id, span, contract, status, dependencies)
}

pub(crate) fn python_stdlib_collection_factory_record(
    id: u32,
    span: Span,
    contract: LibraryCollectionFactoryContract,
    status: EvidenceStatus,
    dependencies: &[u32],
) -> EvidenceRecord {
    library_api_fixture_pack!(
        PYTHON_STDLIB_COLLECTION_FACTORY_PACK_ID,
        PYTHON_STDLIB_COLLECTION_FACTORY_PRODUCER_ID
    )
    .contract_record(id, span, contract, status, dependencies)
}

pub(crate) fn ruby_stdlib_set_record(
    id: u32,
    span: Span,
    contract: LibraryCollectionFactoryContract,
    status: EvidenceStatus,
    dependencies: &[u32],
) -> EvidenceRecord {
    library_api_fixture_pack!(RUBY_STDLIB_SET_PACK_ID, RUBY_STDLIB_SET_PRODUCER_ID).contract_record(
        id,
        span,
        contract,
        status,
        dependencies,
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
    library_api_fixture_pack!(RUST_STDLIB_VEC_PACK_ID, RUST_STDLIB_VEC_PRODUCER_ID)
        .contract_record_with_arity(id, span, contract, arity, status, dependencies)
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
    library_api_fixture_pack!(RUST_STDLIB_OPTION_PACK_ID, RUST_STDLIB_OPTION_PRODUCER_ID)
        .parts_record_with_arity(id, span, contract_id, callee, arity, status, dependencies)
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
    library_api_fixture_pack!(RUST_STDLIB_RESULT_PACK_ID, RUST_STDLIB_RESULT_PRODUCER_ID)
        .parts_record_with_arity(id, span, contract_id, callee, arity, status, dependencies)
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
    library_api_fixture_pack!(
        RUST_STDLIB_INTEGER_METHOD_PACK_ID,
        RUST_STDLIB_INTEGER_METHOD_PRODUCER_ID
    )
    .parts_record_with_arity(id, span, contract_id, callee, arity, status, dependencies)
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
    library_api_fixture_pack!(JAVA_STDLIB_MATH_PACK_ID, JAVA_STDLIB_MATH_PRODUCER_ID)
        .parts_record_with_arity(id, span, contract_id, callee, arity, status, dependencies)
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
    library_api_fixture_pack!(MAP_GET_PROTOCOL_PACK_ID, MAP_GET_PROTOCOL_PRODUCER_ID)
        .contract_record_with_arity(id, span, contract, arity, status, dependencies)
}

pub(crate) fn map_get_default_protocol_record(
    id: u32,
    span: Span,
    contract: LibraryMethodCallContract,
    status: EvidenceStatus,
    dependencies: &[u32],
) -> EvidenceRecord {
    library_api_fixture_pack!(
        MAP_GET_DEFAULT_PROTOCOL_PACK_ID,
        MAP_GET_DEFAULT_PROTOCOL_PRODUCER_ID
    )
    .contract_record_with_arity(id, span, contract, 2, status, dependencies)
}

mod receiver_records;
pub(super) use receiver_records::*;
