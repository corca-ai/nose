use super::*;

pub(super) fn library_api_contract_ids() -> Vec<LibraryApiContractId> {
    let mut ids = core_library_api_contract_ids();
    push_keyed_library_api_contract_ids(&mut ids);
    push_method_call_library_api_contract_ids(&mut ids);
    ids
}

fn core_library_api_contract_ids() -> Vec<LibraryApiContractId> {
    vec![
        LibraryApiContractId::PropertyBuiltin(Builtin::Len),
        LibraryApiContractId::PropertyBuiltin(Builtin::IsEmpty),
        LibraryApiContractId::PythonBuiltinCollectionFactory,
        LibraryApiContractId::PythonImportedCollectionFactory,
        LibraryApiContractId::FreeFunctionBuiltin(Builtin::Len),
        LibraryApiContractId::FreeFunctionBuiltin(Builtin::Append),
        LibraryApiContractId::FreeFunctionBuiltin(Builtin::Print),
        LibraryApiContractId::FreeFunctionBuiltin(Builtin::Range),
        LibraryApiContractId::FreeFunctionBuiltin(Builtin::Sum),
        LibraryApiContractId::FreeFunctionBuiltin(Builtin::Min),
        LibraryApiContractId::FreeFunctionBuiltin(Builtin::Max),
        LibraryApiContractId::FreeFunctionBuiltin(Builtin::Abs),
        LibraryApiContractId::FreeFunctionBuiltin(Builtin::Zip),
        LibraryApiContractId::FreeFunctionBuiltin(Builtin::Enumerate),
        LibraryApiContractId::FreeFunctionBuiltin(Builtin::Any),
        LibraryApiContractId::FreeFunctionBuiltin(Builtin::All),
        LibraryApiContractId::FreeFunctionHof(HoFKind::Map),
        LibraryApiContractId::FreeFunctionHof(HoFKind::Filter),
        LibraryApiContractId::RustOptionSomeConstructor,
        LibraryApiContractId::RustOptionNoneSentinel,
        LibraryApiContractId::RustOptionAndThen,
        LibraryApiContractId::RustResultOkConstructor,
        LibraryApiContractId::RustResultErrConstructor,
        LibraryApiContractId::RustResultIsOk,
        LibraryApiContractId::RustResultIsErr,
        LibraryApiContractId::RustStdCollectionFactory,
        LibraryApiContractId::RustStdMapFactory,
        LibraryApiContractId::SwiftCollectionFactory(SwiftCollectionFactoryKind::Array),
        LibraryApiContractId::SwiftCollectionFactory(SwiftCollectionFactoryKind::Set),
        LibraryApiContractId::SwiftMapFactory(SwiftMapFactoryKind::DictionaryUniqueKeysWithValues),
        LibraryApiContractId::RustVecMacroFactory,
        LibraryApiContractId::RustVecNewFactory,
        LibraryApiContractId::JavaMapEntryFactory,
        LibraryApiContractId::RubySetFactory,
        LibraryApiContractId::JsLikeSetConstructor,
        LibraryApiContractId::JsLikeMapConstructor,
        LibraryApiContractId::MapKeyViewWrapper,
        LibraryApiContractId::MapGet,
        LibraryApiContractId::JsArrayIsArray,
        LibraryApiContractId::JsBooleanCoercion,
        LibraryApiContractId::RegexTest,
        LibraryApiContractId::JsLikeStaticIndexMembership(StaticIndexMembershipKind::IndexOf),
        LibraryApiContractId::JsLikeStaticIndexMembership(StaticIndexMembershipKind::FindIndex),
        LibraryApiContractId::PromiseFactory(PromiseFactoryKind::Resolve),
        LibraryApiContractId::PromiseThen,
        LibraryApiContractId::IteratorIdentityAdapter,
        LibraryApiContractId::StaticCollectionAdapter,
    ]
}

fn push_keyed_library_api_contract_ids(ids: &mut Vec<LibraryApiContractId>) {
    ids.extend(
        [
            ScalarIntegerMethod::Abs,
            ScalarIntegerMethod::Min,
            ScalarIntegerMethod::Max,
            ScalarIntegerMethod::Clamp,
        ]
        .into_iter()
        .map(LibraryApiContractId::ScalarIntegerMethod),
    );
    ids.extend(
        [
            JavaCollectionFactoryKind::ListOf,
            JavaCollectionFactoryKind::SetOf,
            JavaCollectionFactoryKind::ArraysAsList,
            JavaCollectionFactoryKind::CollectionsEmptyList,
            JavaCollectionFactoryKind::CollectionsEmptySet,
            JavaCollectionFactoryKind::CollectionsSingleton,
            JavaCollectionFactoryKind::CollectionsSingletonList,
            JavaCollectionFactoryKind::GuavaImmutableListOf,
            JavaCollectionFactoryKind::GuavaImmutableSetOf,
        ]
        .into_iter()
        .map(LibraryApiContractId::JavaCollectionFactory),
    );
    ids.push(LibraryApiContractId::JavaCollectionConstructor(
        JavaCollectionConstructorKind::EmptyList,
    ));
    ids.extend(
        [
            JavaMapFactoryKind::Of,
            JavaMapFactoryKind::OfEntries,
            JavaMapFactoryKind::CollectionsEmptyMap,
            JavaMapFactoryKind::CollectionsSingletonMap,
            JavaMapFactoryKind::GuavaImmutableMapOf,
        ]
        .into_iter()
        .map(LibraryApiContractId::JavaMapFactory),
    );
    ids.extend(
        [MapKeyViewKind::Collection, MapKeyViewKind::Iterator]
            .into_iter()
            .map(LibraryApiContractId::MapKeyView),
    );
    ids.extend(
        [ImportedNamespaceFunctionSemantic::ProductReduction {
            op: Op::Mul,
            identity: 1,
        }]
        .into_iter()
        .map(LibraryApiContractId::ImportedNamespaceFunction),
    );
}

fn push_method_call_library_api_contract_ids(ids: &mut Vec<LibraryApiContractId>) {
    ids.extend(
        [
            MethodSemanticContract::Builtin(Builtin::Append),
            MethodSemanticContract::Builtin(Builtin::Print),
            MethodSemanticContract::Builtin(Builtin::Len),
            MethodSemanticContract::Builtin(Builtin::IsEmpty),
            MethodSemanticContract::Builtin(Builtin::IsNull),
            MethodSemanticContract::Builtin(Builtin::IsNotNull),
            MethodSemanticContract::Builtin(Builtin::StartsWith),
            MethodSemanticContract::Builtin(Builtin::EndsWith),
            MethodSemanticContract::Builtin(Builtin::Contains),
            MethodSemanticContract::Builtin(Builtin::StringContains),
            MethodSemanticContract::Builtin(Builtin::Join),
            MethodSemanticContract::Builtin(Builtin::GetOrDefault),
            MethodSemanticContract::Builtin(Builtin::ValueOrDefault),
            MethodSemanticContract::Builtin(Builtin::Reduce),
            MethodSemanticContract::Builtin(Builtin::Sum),
            MethodSemanticContract::Builtin(Builtin::Abs),
            MethodSemanticContract::Builtin(Builtin::Min),
            MethodSemanticContract::Builtin(Builtin::Max),
            MethodSemanticContract::Builtin(Builtin::Zip),
            MethodSemanticContract::Builtin(Builtin::Any),
            MethodSemanticContract::Builtin(Builtin::All),
            MethodSemanticContract::HoF(HoFKind::Map),
            MethodSemanticContract::HoF(HoFKind::Filter),
            MethodSemanticContract::HoF(HoFKind::FlatMap),
            MethodSemanticContract::HoF(HoFKind::FilterMap),
        ]
        .into_iter()
        .map(LibraryApiContractId::MethodCall),
    );
}
