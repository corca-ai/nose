use super::*;

pub(crate) fn library_api_contract_evidence(
    id: u32,
    call_span: Span,
    contract_id: LibraryApiContractId,
    callee: LibraryApiCalleeContract,
    arity: u16,
    dependencies: Vec<EvidenceId>,
) -> EvidenceRecord {
    let mut record = evidence_with_dependencies(
        id,
        EvidenceAnchor::node(call_span, NodeKind::Call),
        EvidenceKind::LibraryApi(LibraryApiEvidenceKind::Contract {
            contract_hash: library_api_contract_id_hash(contract_id),
            callee_hash: library_api_callee_contract_hash(callee),
            arity,
        }),
        dependencies,
    );
    if matches!(contract_id, LibraryApiContractId::FreeFunctionBuiltin(_)) {
        record.provenance.pack_hash =
            Some(stable_symbol_hash(FREE_FUNCTION_BUILTIN_PROTOCOL_PACK_ID));
        record.provenance.rule_hash = Some(stable_symbol_hash(
            FREE_FUNCTION_BUILTIN_PROTOCOL_PRODUCER_ID,
        ));
    } else if matches!(contract_id, LibraryApiContractId::MethodCall(_)) {
        record.provenance.pack_hash =
            Some(stable_symbol_hash(BUILTIN_METHOD_CALL_PROTOCOL_PACK_ID));
        record.provenance.rule_hash =
            Some(stable_symbol_hash(BUILTIN_METHOD_CALL_PROTOCOL_PRODUCER_ID));
    }
    record
}

fn library_api_contract_evidence_with_pack(
    id: u32,
    call_span: Span,
    contract_id: LibraryApiContractId,
    callee: LibraryApiCalleeContract,
    arity: u16,
    dependencies: Vec<EvidenceId>,
    provenance: (&str, &str),
) -> EvidenceRecord {
    let mut record =
        library_api_contract_evidence(id, call_span, contract_id, callee, arity, dependencies);
    record.provenance.pack_hash = Some(stable_symbol_hash(provenance.0));
    record.provenance.rule_hash = Some(stable_symbol_hash(provenance.1));
    record
}

pub(crate) fn js_like_builtin_collection_constructor_evidence(
    id: u32,
    call_span: Span,
    contract_id: LibraryApiContractId,
    callee: LibraryApiCalleeContract,
    arity: u16,
    dependencies: Vec<EvidenceId>,
) -> EvidenceRecord {
    library_api_contract_evidence_with_pack(
        id,
        call_span,
        contract_id,
        callee,
        arity,
        dependencies,
        (
            JS_LIKE_BUILTIN_COLLECTION_CONSTRUCTOR_PACK_ID,
            JS_LIKE_BUILTIN_COLLECTION_CONSTRUCTOR_PRODUCER_ID,
        ),
    )
}

pub(crate) fn js_like_builtin_static_index_membership_evidence(
    id: u32,
    call_span: Span,
    contract_id: LibraryApiContractId,
    callee: LibraryApiCalleeContract,
    arity: u16,
    dependencies: Vec<EvidenceId>,
) -> EvidenceRecord {
    library_api_contract_evidence_with_pack(
        id,
        call_span,
        contract_id,
        callee,
        arity,
        dependencies,
        (
            JS_LIKE_BUILTIN_STATIC_INDEX_MEMBERSHIP_PACK_ID,
            JS_LIKE_BUILTIN_STATIC_INDEX_MEMBERSHIP_PRODUCER_ID,
        ),
    )
}

pub(crate) fn rust_stdlib_integer_method_evidence(
    id: u32,
    call_span: Span,
    contract_id: LibraryApiContractId,
    callee: LibraryApiCalleeContract,
    arity: u16,
    dependencies: Vec<EvidenceId>,
) -> EvidenceRecord {
    library_api_contract_evidence_with_pack(
        id,
        call_span,
        contract_id,
        callee,
        arity,
        dependencies,
        (
            RUST_STDLIB_INTEGER_METHOD_PACK_ID,
            RUST_STDLIB_INTEGER_METHOD_PRODUCER_ID,
        ),
    )
}

pub(crate) fn rust_sequence_hof_adapter_evidence(
    id: u32,
    call_span: Span,
    contract_id: LibraryApiContractId,
    callee: LibraryApiCalleeContract,
    arity: u16,
    dependencies: Vec<EvidenceId>,
) -> EvidenceRecord {
    library_api_contract_evidence_with_pack(
        id,
        call_span,
        contract_id,
        callee,
        arity,
        dependencies,
        (
            SEQUENCE_HOF_ADAPTER_PROTOCOL_PACK_ID,
            SEQUENCE_HOF_ADAPTER_PROTOCOL_PRODUCER_ID,
        ),
    )
}

pub(crate) fn java_stdlib_math_evidence(
    id: u32,
    call_span: Span,
    contract_id: LibraryApiContractId,
    callee: LibraryApiCalleeContract,
    arity: u16,
    dependencies: Vec<EvidenceId>,
) -> EvidenceRecord {
    library_api_contract_evidence_with_pack(
        id,
        call_span,
        contract_id,
        callee,
        arity,
        dependencies,
        (JAVA_STDLIB_MATH_PACK_ID, JAVA_STDLIB_MATH_PRODUCER_ID),
    )
}

pub(crate) fn map_get_protocol_evidence(
    id: u32,
    call_span: Span,
    contract_id: LibraryApiContractId,
    callee: LibraryApiCalleeContract,
    arity: u16,
    dependencies: Vec<EvidenceId>,
) -> EvidenceRecord {
    library_api_contract_evidence_with_pack(
        id,
        call_span,
        contract_id,
        callee,
        arity,
        dependencies,
        (MAP_GET_PROTOCOL_PACK_ID, MAP_GET_PROTOCOL_PRODUCER_ID),
    )
}

pub(crate) fn map_key_view_protocol_evidence(
    id: u32,
    call_span: Span,
    contract_id: LibraryApiContractId,
    callee: LibraryApiCalleeContract,
    arity: u16,
    dependencies: Vec<EvidenceId>,
) -> EvidenceRecord {
    library_api_contract_evidence_with_pack(
        id,
        call_span,
        contract_id,
        callee,
        arity,
        dependencies,
        (
            MAP_KEY_VIEW_PROTOCOL_PACK_ID,
            MAP_KEY_VIEW_PROTOCOL_PRODUCER_ID,
        ),
    )
}

pub(crate) fn python_builtin_collection_factory_evidence(
    id: u32,
    call_span: Span,
    contract: LibraryCollectionFactoryContract,
    arity: u16,
    dependencies: Vec<EvidenceId>,
) -> EvidenceRecord {
    library_api_contract_evidence_with_pack(
        id,
        call_span,
        contract.id,
        contract.callee,
        arity,
        dependencies,
        (
            PYTHON_BUILTIN_COLLECTION_FACTORY_PACK_ID,
            PYTHON_BUILTIN_COLLECTION_FACTORY_PRODUCER_ID,
        ),
    )
}

pub(crate) fn python_stdlib_collection_factory_evidence(
    id: u32,
    call_span: Span,
    contract: LibraryCollectionFactoryContract,
    arity: u16,
    dependencies: Vec<EvidenceId>,
) -> EvidenceRecord {
    library_api_contract_evidence_with_pack(
        id,
        call_span,
        contract.id,
        contract.callee,
        arity,
        dependencies,
        (
            PYTHON_STDLIB_COLLECTION_FACTORY_PACK_ID,
            PYTHON_STDLIB_COLLECTION_FACTORY_PRODUCER_ID,
        ),
    )
}

pub(crate) fn python_iterator_builtin_protocol_evidence(
    id: u32,
    call_span: Span,
    contract: LibraryFreeFunctionHofContract,
    arity: u16,
    dependencies: Vec<EvidenceId>,
) -> EvidenceRecord {
    library_api_contract_evidence_with_pack(
        id,
        call_span,
        contract.id,
        contract.callee,
        arity,
        dependencies,
        (
            PYTHON_ITERATOR_BUILTIN_PROTOCOL_PACK_ID,
            PYTHON_ITERATOR_BUILTIN_PROTOCOL_PRODUCER_ID,
        ),
    )
}

pub(crate) fn python_iterator_builtin_protocol_builtin_evidence(
    id: u32,
    call_span: Span,
    contract: LibraryFreeFunctionBuiltinContract,
    arity: u16,
    dependencies: Vec<EvidenceId>,
) -> EvidenceRecord {
    library_api_contract_evidence_with_pack(
        id,
        call_span,
        contract.id,
        contract.callee,
        arity,
        dependencies,
        (
            PYTHON_ITERATOR_BUILTIN_PROTOCOL_PACK_ID,
            PYTHON_ITERATOR_BUILTIN_PROTOCOL_PRODUCER_ID,
        ),
    )
}

pub(crate) fn java_stdlib_map_factory_evidence(
    id: u32,
    call_span: Span,
    contract: LibraryMapFactoryContract,
    arity: u16,
    dependencies: Vec<EvidenceId>,
) -> EvidenceRecord {
    library_api_contract_evidence_with_pack(
        id,
        call_span,
        contract.id,
        contract.callee,
        arity,
        dependencies,
        (
            JAVA_STDLIB_MAP_FACTORY_PACK_ID,
            JAVA_STDLIB_MAP_FACTORY_PRODUCER_ID,
        ),
    )
}

pub(crate) fn java_stdlib_collection_factory_evidence(
    id: u32,
    call_span: Span,
    contract: LibraryCollectionFactoryContract,
    arity: u16,
    dependencies: Vec<EvidenceId>,
) -> EvidenceRecord {
    library_api_contract_evidence_with_pack(
        id,
        call_span,
        contract.id,
        contract.callee,
        arity,
        dependencies,
        (
            JAVA_STDLIB_COLLECTION_FACTORY_PACK_ID,
            JAVA_STDLIB_COLLECTION_FACTORY_PRODUCER_ID,
        ),
    )
}

pub(crate) fn swift_stdlib_collection_factory_evidence(
    id: u32,
    call_span: Span,
    contract: LibraryCollectionFactoryContract,
    arity: u16,
    dependencies: Vec<EvidenceId>,
) -> EvidenceRecord {
    library_api_contract_evidence_with_pack(
        id,
        call_span,
        contract.id,
        contract.callee,
        arity,
        dependencies,
        (
            SWIFT_STDLIB_COLLECTION_FACTORY_PACK_ID,
            SWIFT_STDLIB_COLLECTION_FACTORY_PRODUCER_ID,
        ),
    )
}

pub(crate) fn swift_stdlib_map_factory_evidence(
    id: u32,
    call_span: Span,
    contract: LibraryMapFactoryContract,
    arity: u16,
    dependencies: Vec<EvidenceId>,
) -> EvidenceRecord {
    library_api_contract_evidence_with_pack(
        id,
        call_span,
        contract.id,
        contract.callee,
        arity,
        dependencies,
        (
            SWIFT_STDLIB_COLLECTION_FACTORY_PACK_ID,
            SWIFT_STDLIB_COLLECTION_FACTORY_PRODUCER_ID,
        ),
    )
}

pub(crate) fn java_guava_immutable_collection_factory_evidence(
    id: u32,
    call_span: Span,
    contract_id: LibraryApiContractId,
    callee: LibraryApiCalleeContract,
    arity: u16,
    dependencies: Vec<EvidenceId>,
) -> EvidenceRecord {
    library_api_contract_evidence_with_pack(
        id,
        call_span,
        contract_id,
        callee,
        arity,
        dependencies,
        (
            JAVA_GUAVA_IMMUTABLE_COLLECTION_FACTORY_PACK_ID,
            JAVA_GUAVA_IMMUTABLE_COLLECTION_FACTORY_PRODUCER_ID,
        ),
    )
}

pub(crate) fn java_stdlib_collection_constructor_evidence(
    id: u32,
    call_span: Span,
    contract: LibraryCollectionFactoryContract,
    arity: u16,
    dependencies: Vec<EvidenceId>,
) -> EvidenceRecord {
    library_api_contract_evidence_with_pack(
        id,
        call_span,
        contract.id,
        contract.callee,
        arity,
        dependencies,
        (
            JAVA_STDLIB_COLLECTION_CONSTRUCTOR_PACK_ID,
            JAVA_STDLIB_COLLECTION_CONSTRUCTOR_PRODUCER_ID,
        ),
    )
}

mod library_api_push;
pub(crate) use library_api_push::*;

pub(crate) fn eval_proven_collection_op(
    il: &Il,
    interner: &Interner,
    call: NodeId,
) -> Option<ValOp> {
    let mut builder = Builder::new(il, interner);
    let raw = builder.eval(call, &FxHashMap::default());
    builder
        .proven_collection_value(raw)
        .map(|value| builder.nodes[value as usize].op.clone())
}

pub(crate) fn receiver_domain_contains_call_il() -> (Il, Interner, NodeId, Span) {
    let interner = Interner::new();
    let mut b = IlBuilder::new(FileId(0));
    let receiver_span = sp(30);
    let receiver = b.add(
        NodeKind::Var,
        Payload::Name(interner.intern("xs")),
        receiver_span,
        &[],
    );
    let callee = b.add(
        NodeKind::Field,
        Payload::Name(interner.intern("includes")),
        sp(31),
        &[receiver],
    );
    let item = b.add(
        NodeKind::Var,
        Payload::Name(interner.intern("item")),
        sp(32),
        &[],
    );
    let call = b.add(NodeKind::Call, Payload::None, sp(33), &[callee, item]);
    let root = b.add(NodeKind::Block, Payload::None, sp(29), &[call]);
    let il = finish_test_il(b, root, Lang::TypeScript);
    (il, interner, call, receiver_span)
}

pub(crate) fn eval_op(il: &Il, interner: &Interner, node: NodeId) -> ValOp {
    let mut builder = Builder::new(il, interner);
    let value = builder.eval(node, &FxHashMap::default());
    builder.nodes[value as usize].op.clone()
}
