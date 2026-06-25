use super::*;

pub(in crate::library_api) fn library_api_record_provenance_matches_contract(
    lang: Lang,
    id: LibraryApiContractId,
    callee: LibraryApiCalleeContract,
    record: &EvidenceRecord,
) -> bool {
    let Some((pack_id, producer_id)) = library_api_contract_provenance_ids(lang, id, callee) else {
        return false;
    };
    library_api_record_has_builtin_provenance(record, pack_id, producer_id)
}

pub(in crate::library_api) fn library_api_record_has_builtin_provenance(
    record: &EvidenceRecord,
    pack_id: &'static str,
    producer_id: &'static str,
) -> bool {
    record.provenance.emitter == EvidenceEmitter::Builtin
        && record.provenance.pack_hash == Some(stable_symbol_hash(pack_id))
        && record.provenance.rule_hash == Some(stable_symbol_hash(producer_id))
}

fn library_api_contract_provenance_ids(
    lang: Lang,
    id: LibraryApiContractId,
    callee: LibraryApiCalleeContract,
) -> Option<(&'static str, &'static str)> {
    python_library_api_contract_provenance_ids(id)
        .or_else(|| js_like_library_api_contract_provenance_ids(lang, id, callee))
        .or_else(|| swift_library_api_contract_provenance_ids(id))
        .or_else(|| rust_library_api_contract_provenance_ids(lang, id, callee))
        .or_else(|| java_library_api_contract_provenance_ids(id, callee))
        .or_else(|| go_library_api_contract_provenance_ids(id, callee))
        .or_else(|| protocol_library_api_contract_provenance_ids(id, callee))
}

fn python_library_api_contract_provenance_ids(
    id: LibraryApiContractId,
) -> Option<(&'static str, &'static str)> {
    match id {
        LibraryApiContractId::PythonBuiltinCollectionFactory => Some((
            PYTHON_BUILTIN_COLLECTION_FACTORY_PACK_ID,
            PYTHON_BUILTIN_COLLECTION_FACTORY_PRODUCER_ID,
        )),
        LibraryApiContractId::PythonImportedCollectionFactory => Some((
            PYTHON_STDLIB_COLLECTION_FACTORY_PACK_ID,
            PYTHON_STDLIB_COLLECTION_FACTORY_PRODUCER_ID,
        )),
        LibraryApiContractId::ImportedNamespaceFunction(
            ImportedNamespaceFunctionSemantic::ProductReduction {
                op: Op::Mul,
                identity: 1,
            },
        ) => Some((PYTHON_STDLIB_MATH_PACK_ID, PYTHON_STDLIB_MATH_PRODUCER_ID)),
        LibraryApiContractId::FreeFunctionHof(HoFKind::Map | HoFKind::Filter)
        | LibraryApiContractId::FreeFunctionBuiltin(
            Builtin::Zip | Builtin::Enumerate | Builtin::Any | Builtin::All,
        ) => Some((
            PYTHON_ITERATOR_BUILTIN_PROTOCOL_PACK_ID,
            PYTHON_ITERATOR_BUILTIN_PROTOCOL_PRODUCER_ID,
        )),
        _ => None,
    }
}

fn swift_library_api_contract_provenance_ids(
    id: LibraryApiContractId,
) -> Option<(&'static str, &'static str)> {
    match id {
        LibraryApiContractId::SwiftCollectionFactory(_)
        | LibraryApiContractId::SwiftMapFactory(_) => Some((
            SWIFT_STDLIB_COLLECTION_FACTORY_PACK_ID,
            SWIFT_STDLIB_COLLECTION_FACTORY_PRODUCER_ID,
        )),
        _ => None,
    }
}

fn js_like_library_api_contract_provenance_ids(
    lang: Lang,
    id: LibraryApiContractId,
    callee: LibraryApiCalleeContract,
) -> Option<(&'static str, &'static str)> {
    match id {
        LibraryApiContractId::PromiseFactory(PromiseFactoryKind::Resolve)
        | LibraryApiContractId::PromiseThen => Some((
            JS_LIKE_BUILTIN_PROMISE_PACK_ID,
            JS_LIKE_BUILTIN_PROMISE_PRODUCER_ID,
        )),
        LibraryApiContractId::MapKeyViewWrapper | LibraryApiContractId::JsArrayIsArray => Some((
            JS_LIKE_BUILTIN_ARRAY_PACK_ID,
            JS_LIKE_BUILTIN_ARRAY_PRODUCER_ID,
        )),
        LibraryApiContractId::MethodCall(semantic)
            if js_like_array_hof_method_callee(lang, semantic, callee) =>
        {
            Some((
                JS_LIKE_BUILTIN_ARRAY_PACK_ID,
                JS_LIKE_BUILTIN_ARRAY_PRODUCER_ID,
            ))
        }
        LibraryApiContractId::JsBooleanCoercion => Some((
            JS_LIKE_BUILTIN_BOOLEAN_PACK_ID,
            JS_LIKE_BUILTIN_BOOLEAN_PRODUCER_ID,
        )),
        LibraryApiContractId::RegexTest => Some((
            JS_LIKE_BUILTIN_REGEX_PACK_ID,
            JS_LIKE_BUILTIN_REGEX_PRODUCER_ID,
        )),
        LibraryApiContractId::JsLikeStaticIndexMembership(_) => Some((
            JS_LIKE_BUILTIN_STATIC_INDEX_MEMBERSHIP_PACK_ID,
            JS_LIKE_BUILTIN_STATIC_INDEX_MEMBERSHIP_PRODUCER_ID,
        )),
        LibraryApiContractId::JsLikeSetConstructor | LibraryApiContractId::JsLikeMapConstructor => {
            Some((
                JS_LIKE_BUILTIN_COLLECTION_CONSTRUCTOR_PACK_ID,
                JS_LIKE_BUILTIN_COLLECTION_CONSTRUCTOR_PRODUCER_ID,
            ))
        }
        _ => None,
    }
}

fn js_like_array_hof_method_callee(
    lang: Lang,
    semantic: MethodSemanticContract,
    callee: LibraryApiCalleeContract,
) -> bool {
    js_like_lang(lang)
        && matches!(
            (semantic, callee),
            (
                MethodSemanticContract::HoF(HoFKind::Map),
                LibraryApiCalleeContract::Method {
                    method: "map",
                    receiver: MethodReceiverContract::ExactArray,
                },
            ) | (
                MethodSemanticContract::HoF(HoFKind::Filter),
                LibraryApiCalleeContract::Method {
                    method: "filter",
                    receiver: MethodReceiverContract::ExactArray,
                },
            ) | (
                MethodSemanticContract::HoF(HoFKind::FlatMap),
                LibraryApiCalleeContract::Method {
                    method: "flatMap",
                    receiver: MethodReceiverContract::ExactArray,
                },
            ) | (
                MethodSemanticContract::Builtin(Builtin::Any),
                LibraryApiCalleeContract::Method {
                    method: "some",
                    receiver: MethodReceiverContract::ExactArray,
                },
            ) | (
                MethodSemanticContract::Builtin(Builtin::All),
                LibraryApiCalleeContract::Method {
                    method: "every",
                    receiver: MethodReceiverContract::ExactArray,
                },
            )
        )
}

fn rust_library_api_contract_provenance_ids(
    lang: Lang,
    id: LibraryApiContractId,
    callee: LibraryApiCalleeContract,
) -> Option<(&'static str, &'static str)> {
    match id {
        LibraryApiContractId::RustVecMacroFactory | LibraryApiContractId::RustVecNewFactory => {
            Some((RUST_STDLIB_VEC_PACK_ID, RUST_STDLIB_VEC_PRODUCER_ID))
        }
        LibraryApiContractId::RustOptionSomeConstructor
        | LibraryApiContractId::RustOptionNoneSentinel
        | LibraryApiContractId::RustOptionAndThen => {
            Some((RUST_STDLIB_OPTION_PACK_ID, RUST_STDLIB_OPTION_PRODUCER_ID))
        }
        LibraryApiContractId::RustResultOkConstructor
        | LibraryApiContractId::RustResultErrConstructor
        | LibraryApiContractId::RustResultIsOk
        | LibraryApiContractId::RustResultIsErr => {
            Some((RUST_STDLIB_RESULT_PACK_ID, RUST_STDLIB_RESULT_PRODUCER_ID))
        }
        LibraryApiContractId::ScalarIntegerMethod(_)
            if matches!(
                callee,
                LibraryApiCalleeContract::Method {
                    receiver: MethodReceiverContract::ExactInteger,
                    ..
                }
            ) =>
        {
            Some((
                RUST_STDLIB_INTEGER_METHOD_PACK_ID,
                RUST_STDLIB_INTEGER_METHOD_PRODUCER_ID,
            ))
        }
        LibraryApiContractId::RustStdCollectionFactory => Some((
            RUST_STDLIB_COLLECTION_FACTORY_PACK_ID,
            RUST_STDLIB_COLLECTION_FACTORY_PRODUCER_ID,
        )),
        LibraryApiContractId::RustStdMapFactory => Some((
            RUST_STDLIB_MAP_FACTORY_PACK_ID,
            RUST_STDLIB_MAP_FACTORY_PRODUCER_ID,
        )),
        LibraryApiContractId::MethodCall(_)
            if lang == Lang::Rust && rust_sequence_hof_method_callee(id, callee) =>
        {
            Some((
                SEQUENCE_HOF_ADAPTER_PROTOCOL_PACK_ID,
                SEQUENCE_HOF_ADAPTER_PROTOCOL_PRODUCER_ID,
            ))
        }
        _ => None,
    }
}

fn rust_sequence_hof_method_callee(
    id: LibraryApiContractId,
    callee: LibraryApiCalleeContract,
) -> bool {
    matches!(
        (id, callee),
        (
            LibraryApiContractId::MethodCall(MethodSemanticContract::HoF(
                HoFKind::Map | HoFKind::Filter | HoFKind::FilterMap | HoFKind::FlatMap,
            )),
            LibraryApiCalleeContract::Method {
                method: "map" | "filter" | "filter_map" | "flat_map",
                receiver: MethodReceiverContract::ExactProtocol,
            },
        ) | (
            LibraryApiContractId::MethodCall(MethodSemanticContract::Builtin(
                Builtin::Any | Builtin::All,
            )),
            LibraryApiCalleeContract::Method {
                method: "any" | "all",
                receiver: MethodReceiverContract::ExactProtocol,
            },
        ) | (
            LibraryApiContractId::MethodCall(MethodSemanticContract::Builtin(Builtin::Len)),
            LibraryApiCalleeContract::Method {
                method: "count",
                receiver: MethodReceiverContract::ExactProtocol,
            },
        )
    )
}

fn java_library_api_contract_provenance_ids(
    id: LibraryApiContractId,
    callee: LibraryApiCalleeContract,
) -> Option<(&'static str, &'static str)> {
    match id {
        LibraryApiContractId::ScalarIntegerMethod(_)
            if matches!(
                callee,
                LibraryApiCalleeContract::Method {
                    receiver: MethodReceiverContract::UnshadowedGlobal("Math"),
                    ..
                }
            ) =>
        {
            Some((JAVA_STDLIB_MATH_PACK_ID, JAVA_STDLIB_MATH_PRODUCER_ID))
        }
        LibraryApiContractId::JavaMapFactory(JavaMapFactoryKind::Of)
        | LibraryApiContractId::JavaMapFactory(JavaMapFactoryKind::OfEntries)
        | LibraryApiContractId::JavaMapFactory(JavaMapFactoryKind::CollectionsEmptyMap)
        | LibraryApiContractId::JavaMapFactory(JavaMapFactoryKind::CollectionsSingletonMap) => {
            Some((
                JAVA_STDLIB_MAP_FACTORY_PACK_ID,
                JAVA_STDLIB_MAP_FACTORY_PRODUCER_ID,
            ))
        }
        LibraryApiContractId::JavaMapFactory(JavaMapFactoryKind::GuavaImmutableMapOf) => Some((
            JAVA_GUAVA_IMMUTABLE_COLLECTION_FACTORY_PACK_ID,
            JAVA_GUAVA_IMMUTABLE_COLLECTION_FACTORY_PRODUCER_ID,
        )),
        LibraryApiContractId::JavaMapEntryFactory => Some((
            JAVA_STDLIB_MAP_ENTRY_PACK_ID,
            JAVA_STDLIB_MAP_ENTRY_PRODUCER_ID,
        )),
        LibraryApiContractId::JavaCollectionFactory(JavaCollectionFactoryKind::ListOf)
        | LibraryApiContractId::JavaCollectionFactory(JavaCollectionFactoryKind::SetOf)
        | LibraryApiContractId::JavaCollectionFactory(JavaCollectionFactoryKind::ArraysAsList)
        | LibraryApiContractId::JavaCollectionFactory(
            JavaCollectionFactoryKind::CollectionsEmptyList,
        )
        | LibraryApiContractId::JavaCollectionFactory(
            JavaCollectionFactoryKind::CollectionsEmptySet,
        )
        | LibraryApiContractId::JavaCollectionFactory(
            JavaCollectionFactoryKind::CollectionsSingleton,
        )
        | LibraryApiContractId::JavaCollectionFactory(
            JavaCollectionFactoryKind::CollectionsSingletonList,
        ) => Some((
            JAVA_STDLIB_COLLECTION_FACTORY_PACK_ID,
            JAVA_STDLIB_COLLECTION_FACTORY_PRODUCER_ID,
        )),
        LibraryApiContractId::JavaCollectionFactory(
            JavaCollectionFactoryKind::GuavaImmutableListOf
            | JavaCollectionFactoryKind::GuavaImmutableSetOf,
        ) => Some((
            JAVA_GUAVA_IMMUTABLE_COLLECTION_FACTORY_PACK_ID,
            JAVA_GUAVA_IMMUTABLE_COLLECTION_FACTORY_PRODUCER_ID,
        )),
        LibraryApiContractId::JavaCollectionConstructor(_) => Some((
            JAVA_STDLIB_COLLECTION_CONSTRUCTOR_PACK_ID,
            JAVA_STDLIB_COLLECTION_CONSTRUCTOR_PRODUCER_ID,
        )),
        LibraryApiContractId::StaticCollectionAdapter => Some((
            JAVA_STDLIB_STATIC_COLLECTION_ADAPTER_PACK_ID,
            JAVA_STDLIB_STATIC_COLLECTION_ADAPTER_PRODUCER_ID,
        )),
        _ => None,
    }
}

fn go_library_api_contract_provenance_ids(
    id: LibraryApiContractId,
    callee: LibraryApiCalleeContract,
) -> Option<(&'static str, &'static str)> {
    match (id, callee) {
        (
            LibraryApiContractId::MethodCall(MethodSemanticContract::Builtin(Builtin::Print)),
            LibraryApiCalleeContract::Method {
                receiver: MethodReceiverContract::ImportedNamespace("fmt"),
                ..
            },
        )
        | (
            LibraryApiContractId::MethodCall(MethodSemanticContract::Builtin(
                Builtin::StartsWith | Builtin::EndsWith,
            )),
            LibraryApiCalleeContract::Method {
                receiver: MethodReceiverContract::ImportedNamespace("strings"),
                ..
            },
        )
        | (
            LibraryApiContractId::MethodCall(MethodSemanticContract::Builtin(
                Builtin::StringContains,
            )),
            LibraryApiCalleeContract::Method {
                receiver: MethodReceiverContract::ImportedNamespace("strings"),
                ..
            },
        )
        | (
            LibraryApiContractId::MethodCall(MethodSemanticContract::Builtin(Builtin::Contains)),
            LibraryApiCalleeContract::Method {
                receiver: MethodReceiverContract::ImportedNamespace("slices"),
                ..
            },
        ) => Some((
            GO_STDLIB_NAMESPACE_CALL_PACK_ID,
            GO_STDLIB_NAMESPACE_CALL_PRODUCER_ID,
        )),
        _ => None,
    }
}

fn protocol_library_api_contract_provenance_ids(
    id: LibraryApiContractId,
    callee: LibraryApiCalleeContract,
) -> Option<(&'static str, &'static str)> {
    match id {
        LibraryApiContractId::FreeFunctionBuiltin(_) => Some((
            FREE_FUNCTION_BUILTIN_PROTOCOL_PACK_ID,
            FREE_FUNCTION_BUILTIN_PROTOCOL_PRODUCER_ID,
        )),
        LibraryApiContractId::IteratorIdentityAdapter => Some((
            ITERATOR_IDENTITY_ADAPTER_PACK_ID,
            ITERATOR_IDENTITY_ADAPTER_PRODUCER_ID,
        )),
        LibraryApiContractId::MapGet => {
            Some((MAP_GET_PROTOCOL_PACK_ID, MAP_GET_PROTOCOL_PRODUCER_ID))
        }
        LibraryApiContractId::MethodCall(MethodSemanticContract::Builtin(
            Builtin::GetOrDefault,
        )) if exact_map_method_callee(callee) => Some((
            MAP_GET_DEFAULT_PROTOCOL_PACK_ID,
            MAP_GET_DEFAULT_PROTOCOL_PRODUCER_ID,
        )),
        LibraryApiContractId::MethodCall(MethodSemanticContract::Builtin(Builtin::Contains))
            if receiver_membership_method_callee(callee) =>
        {
            Some((
                RECEIVER_MEMBERSHIP_PROTOCOL_PACK_ID,
                RECEIVER_MEMBERSHIP_PROTOCOL_PRODUCER_ID,
            ))
        }
        LibraryApiContractId::MapKeyView(_) => Some((
            MAP_KEY_VIEW_PROTOCOL_PACK_ID,
            MAP_KEY_VIEW_PROTOCOL_PRODUCER_ID,
        )),
        LibraryApiContractId::PropertyBuiltin(_) => Some((
            PROPERTY_BUILTIN_PROTOCOL_PACK_ID,
            PROPERTY_BUILTIN_PROTOCOL_PRODUCER_ID,
        )),
        LibraryApiContractId::RubySetFactory => {
            Some((RUBY_STDLIB_SET_PACK_ID, RUBY_STDLIB_SET_PRODUCER_ID))
        }
        LibraryApiContractId::MethodCall(_) => Some((
            BUILTIN_METHOD_CALL_PROTOCOL_PACK_ID,
            BUILTIN_METHOD_CALL_PROTOCOL_PRODUCER_ID,
        )),
        _ => None,
    }
}

fn exact_map_method_callee(callee: LibraryApiCalleeContract) -> bool {
    matches!(
        callee,
        LibraryApiCalleeContract::Method {
            receiver: MethodReceiverContract::ExactMap,
            ..
        }
    )
}

fn receiver_membership_method_callee(callee: LibraryApiCalleeContract) -> bool {
    matches!(
        callee,
        LibraryApiCalleeContract::Method {
            receiver: MethodReceiverContract::ExactMap
                | MethodReceiverContract::ExactCollectionOrMap
                | MethodReceiverContract::ExactCollectionOrJavaKeySet
                | MethodReceiverContract::ExactSetOrMap,
            ..
        }
    )
}
