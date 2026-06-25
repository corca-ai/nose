use super::*;

pub fn library_api_contract_evidence_for_call(
    il: &Il,
    interner: &Interner,
    node: NodeId,
    id: LibraryApiContractId,
    callee: LibraryApiCalleeContract,
    arg_count: usize,
) -> LibraryApiEvidenceStatus {
    if il.kind(node) != NodeKind::Call || arg_count > u16::MAX as usize {
        return LibraryApiEvidenceStatus::Rejected;
    }
    let expected = LibraryApiEvidenceKind::Contract {
        contract_hash: library_api_contract_id_hash(id),
        callee_hash: library_api_callee_contract_hash(callee),
        arity: arg_count as u16,
    };
    let span = il.node(node).span;
    let mut saw_library_api_evidence = false;
    let mut admitted = false;
    for record in il.evidence_anchored_at(span) {
        if record.anchor != EvidenceAnchor::node(span, NodeKind::Call) {
            continue;
        }
        let EvidenceKind::LibraryApi(api) = record.kind else {
            continue;
        };
        saw_library_api_evidence = true;
        if record.status != EvidenceStatus::Asserted
            || api != expected
            || !library_api_record_provenance_matches_contract(id, callee, record)
            || !il.evidence_dependencies_asserted(record)
            || !library_api_callee_shape_matches(il, interner, node, callee)
            || !library_api_dependencies_match_callee(il, interner, node, callee, record)
        {
            return LibraryApiEvidenceStatus::Rejected;
        }
        admitted = true;
    }
    if admitted {
        LibraryApiEvidenceStatus::Admitted
    } else if saw_library_api_evidence {
        LibraryApiEvidenceStatus::Rejected
    } else {
        LibraryApiEvidenceStatus::Missing
    }
}

pub fn library_api_contract_evidence_for_node(
    il: &Il,
    interner: &Interner,
    node: NodeId,
    id: LibraryApiContractId,
    callee: LibraryApiCalleeContract,
    arg_count: usize,
) -> LibraryApiEvidenceStatus {
    if arg_count > u16::MAX as usize {
        return LibraryApiEvidenceStatus::Rejected;
    }
    let expected = LibraryApiEvidenceKind::Contract {
        contract_hash: library_api_contract_id_hash(id),
        callee_hash: library_api_callee_contract_hash(callee),
        arity: arg_count as u16,
    };
    let anchor = EvidenceAnchor::node(il.node(node).span, il.kind(node));
    let mut saw_library_api_evidence = false;
    let mut admitted = false;
    for record in il.evidence_anchored_at(anchor.span()) {
        if record.anchor != anchor {
            continue;
        }
        let EvidenceKind::LibraryApi(api) = record.kind else {
            continue;
        };
        saw_library_api_evidence = true;
        if record.status != EvidenceStatus::Asserted
            || api != expected
            || !library_api_record_provenance_matches_contract(id, callee, record)
            || !il.evidence_dependencies_asserted(record)
            || !library_api_node_callee_shape_matches(il, interner, node, callee)
            || !library_api_dependencies_match_callee_node(il, interner, node, callee, record)
        {
            return LibraryApiEvidenceStatus::Rejected;
        }
        admitted = true;
    }
    if admitted {
        LibraryApiEvidenceStatus::Admitted
    } else if saw_library_api_evidence {
        LibraryApiEvidenceStatus::Rejected
    } else {
        LibraryApiEvidenceStatus::Missing
    }
}

pub fn library_api_contract_evidence_at_call_span(
    il: &Il,
    interner: &Interner,
    query: LibraryApiSpanEvidenceQuery,
) -> LibraryApiEvidenceStatus {
    let Some(span) = query.call_span else {
        return LibraryApiEvidenceStatus::Missing;
    };
    if query.arg_count > u16::MAX as usize {
        return LibraryApiEvidenceStatus::Rejected;
    }
    let expected = LibraryApiEvidenceKind::Contract {
        contract_hash: library_api_contract_id_hash(query.id),
        callee_hash: library_api_callee_contract_hash(query.callee),
        arity: query.arg_count as u16,
    };
    let source_call = node_at_span_with_kind(il, span, NodeKind::Call);
    let mut saw_library_api_evidence = false;
    let mut admitted = false;
    for record in il.evidence_anchored_at(span) {
        if record.anchor != EvidenceAnchor::node(span, NodeKind::Call) {
            continue;
        }
        let EvidenceKind::LibraryApi(api) = record.kind else {
            continue;
        };
        saw_library_api_evidence = true;
        let source_call_matches = source_call.is_some_and(|node| {
            library_api_source_call_spans_match_query(
                il,
                node,
                query.callee_span,
                query.receiver_span,
            ) && library_api_callee_shape_matches(il, interner, node, query.callee)
                && library_api_dependencies_match_callee(il, interner, node, query.callee, record)
        });
        let span_query_matches = library_api_dependencies_match_callee_at_span(
            il,
            interner,
            span,
            query.callee_span,
            query.receiver_span,
            query.callee,
            record,
        );
        if record.status != EvidenceStatus::Asserted
            || api != expected
            || !library_api_record_provenance_matches_contract(query.id, query.callee, record)
            || !il.evidence_dependencies_asserted(record)
            || (!source_call_matches && !span_query_matches)
        {
            return LibraryApiEvidenceStatus::Rejected;
        }
        admitted = true;
    }
    if admitted {
        LibraryApiEvidenceStatus::Admitted
    } else if saw_library_api_evidence {
        LibraryApiEvidenceStatus::Rejected
    } else {
        LibraryApiEvidenceStatus::Missing
    }
}

pub(in crate::library_api) fn library_api_record_provenance_matches_contract(
    id: LibraryApiContractId,
    callee: LibraryApiCalleeContract,
    record: &EvidenceRecord,
) -> bool {
    let Some((pack_id, producer_id)) = library_api_contract_provenance_ids(id, callee) else {
        return false;
    };
    library_api_record_has_builtin_provenance(record, pack_id, producer_id)
}

fn library_api_record_has_builtin_provenance(
    record: &EvidenceRecord,
    pack_id: &'static str,
    producer_id: &'static str,
) -> bool {
    record.provenance.emitter == EvidenceEmitter::Builtin
        && record.provenance.pack_hash == Some(stable_symbol_hash(pack_id))
        && record.provenance.rule_hash == Some(stable_symbol_hash(producer_id))
}

fn library_api_contract_provenance_ids(
    id: LibraryApiContractId,
    callee: LibraryApiCalleeContract,
) -> Option<(&'static str, &'static str)> {
    python_library_api_contract_provenance_ids(id)
        .or_else(|| js_like_library_api_contract_provenance_ids(id))
        .or_else(|| swift_library_api_contract_provenance_ids(id))
        .or_else(|| rust_library_api_contract_provenance_ids(id, callee))
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
    id: LibraryApiContractId,
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

fn rust_library_api_contract_provenance_ids(
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
        _ => None,
    }
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

fn library_api_source_call_spans_match_query(
    il: &Il,
    source_call: NodeId,
    callee_span: Option<Span>,
    receiver_span: Option<Span>,
) -> bool {
    let Some(&callee) = il.children(source_call).first() else {
        return false;
    };
    if callee_span.is_some_and(|span| il.node(callee).span != span) {
        return false;
    }
    if let Some(span) = receiver_span {
        let Some(&receiver) = il.children(callee).first() else {
            return false;
        };
        if il.node(receiver).span != span {
            return false;
        }
    }
    true
}
