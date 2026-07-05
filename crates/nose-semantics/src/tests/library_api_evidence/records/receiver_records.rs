use super::*;

pub(crate) fn receiver_membership_protocol_record(
    id: u32,
    span: Span,
    contract: LibraryMethodCallContract,
    status: EvidenceStatus,
    dependencies: &[u32],
) -> EvidenceRecord {
    library_api_fixture_pack!(
        RECEIVER_MEMBERSHIP_PROTOCOL_PACK_ID,
        RECEIVER_MEMBERSHIP_PROTOCOL_PRODUCER_ID
    )
    .contract_record(id, span, contract, status, dependencies)
}

pub(crate) fn map_key_view_protocol_record(
    id: u32,
    span: Span,
    contract: LibraryMapKeyViewContract,
    status: EvidenceStatus,
    dependencies: &[u32],
) -> EvidenceRecord {
    library_api_fixture_pack!(
        MAP_KEY_VIEW_PROTOCOL_PACK_ID,
        MAP_KEY_VIEW_PROTOCOL_PRODUCER_ID
    )
    .contract_record_with_arity(id, span, contract, 0, status, dependencies)
}

pub(crate) fn builtin_method_call_protocol_record(
    id: u32,
    span: Span,
    contract: LibraryMethodCallContract,
    arity: u16,
    status: EvidenceStatus,
    dependencies: &[u32],
) -> EvidenceRecord {
    library_api_fixture_pack!(contract.pack_id, contract.producer_id).contract_record_with_arity(
        id,
        span,
        contract,
        arity,
        status,
        dependencies,
    )
}

pub(crate) fn iterator_identity_adapter_record(
    id: u32,
    span: Span,
    contract_id: LibraryApiContractId,
    callee: LibraryApiCalleeContract,
    arity: u16,
    status: EvidenceStatus,
    dependencies: &[u32],
) -> EvidenceRecord {
    library_api_fixture_pack!(
        ITERATOR_IDENTITY_ADAPTER_PACK_ID,
        ITERATOR_IDENTITY_ADAPTER_PRODUCER_ID
    )
    .parts_record_with_arity(id, span, contract_id, callee, arity, status, dependencies)
}

pub(crate) fn rust_stdlib_collection_factory_record(
    id: u32,
    span: Span,
    contract: LibraryCollectionFactoryContract,
    status: EvidenceStatus,
    dependencies: &[u32],
) -> EvidenceRecord {
    library_api_fixture_pack!(
        RUST_STDLIB_COLLECTION_FACTORY_PACK_ID,
        RUST_STDLIB_COLLECTION_FACTORY_PRODUCER_ID
    )
    .contract_record(id, span, contract, status, dependencies)
}

pub(crate) fn rust_stdlib_map_factory_record(
    id: u32,
    span: Span,
    contract: LibraryMapFactoryContract,
    status: EvidenceStatus,
    dependencies: &[u32],
) -> EvidenceRecord {
    library_api_fixture_pack!(
        RUST_STDLIB_MAP_FACTORY_PACK_ID,
        RUST_STDLIB_MAP_FACTORY_PRODUCER_ID
    )
    .contract_record(id, span, contract, status, dependencies)
}

pub(crate) fn java_stdlib_collection_factory_record(
    id: u32,
    span: Span,
    contract: LibraryCollectionFactoryContract,
    arity: u16,
    status: EvidenceStatus,
    dependencies: &[u32],
) -> EvidenceRecord {
    library_api_fixture_pack!(
        JAVA_STDLIB_COLLECTION_FACTORY_PACK_ID,
        JAVA_STDLIB_COLLECTION_FACTORY_PRODUCER_ID
    )
    .contract_record_with_arity(id, span, contract, arity, status, dependencies)
}

pub(crate) fn java_stdlib_collection_constructor_record(
    id: u32,
    span: Span,
    contract: LibraryCollectionFactoryContract,
    arity: u16,
    status: EvidenceStatus,
    dependencies: &[u32],
) -> EvidenceRecord {
    library_api_fixture_pack!(
        JAVA_STDLIB_COLLECTION_CONSTRUCTOR_PACK_ID,
        JAVA_STDLIB_COLLECTION_CONSTRUCTOR_PRODUCER_ID
    )
    .contract_record_with_arity(id, span, contract, arity, status, dependencies)
}

pub(crate) fn java_stdlib_map_factory_record(
    id: u32,
    span: Span,
    contract: LibraryMapFactoryContract,
    arity: u16,
    status: EvidenceStatus,
    dependencies: &[u32],
) -> EvidenceRecord {
    library_api_fixture_pack!(
        JAVA_STDLIB_MAP_FACTORY_PACK_ID,
        JAVA_STDLIB_MAP_FACTORY_PRODUCER_ID
    )
    .contract_record_with_arity(id, span, contract, arity, status, dependencies)
}

pub(crate) fn java_stdlib_map_entry_record(
    id: u32,
    span: Span,
    contract: LibraryMapEntryFactoryContract,
    arity: u16,
    status: EvidenceStatus,
    dependencies: &[u32],
) -> EvidenceRecord {
    library_api_fixture_pack!(
        JAVA_STDLIB_MAP_ENTRY_PACK_ID,
        JAVA_STDLIB_MAP_ENTRY_PRODUCER_ID
    )
    .contract_record_with_arity(id, span, contract, arity, status, dependencies)
}

pub(crate) fn java_stdlib_static_collection_adapter_record(
    id: u32,
    span: Span,
    contract: LibraryStaticCollectionAdapterContract,
    arity: u16,
    status: EvidenceStatus,
    dependencies: &[u32],
) -> EvidenceRecord {
    library_api_fixture_pack!(
        JAVA_STDLIB_STATIC_COLLECTION_ADAPTER_PACK_ID,
        JAVA_STDLIB_STATIC_COLLECTION_ADAPTER_PRODUCER_ID
    )
    .contract_record_with_arity(id, span, contract, arity, status, dependencies)
}

pub(crate) fn library_api_record_with_arity(
    id: u32,
    span: Span,
    contract_id: LibraryApiContractId,
    callee: LibraryApiCalleeContract,
    arity: u16,
    status: EvidenceStatus,
    dependencies: &[u32],
) -> EvidenceRecord {
    evidence_with_dependencies(
        id,
        EvidenceAnchor::node(span, NodeKind::Call),
        EvidenceKind::LibraryApi(LibraryApiEvidenceKind::Contract {
            contract_hash: library_api_contract_id_hash(contract_id),
            callee_hash: library_api_callee_contract_hash(callee),
            arity,
        }),
        status,
        dependencies.iter().copied().map(EvidenceId).collect(),
    )
}

pub(crate) fn contract_status_for_call(
    il: &Il,
    interner: &Interner,
    call: NodeId,
    id: LibraryApiContractId,
    callee: LibraryApiCalleeContract,
) -> LibraryApiEvidenceStatus {
    library_api_contract_evidence_for_call(il, interner, call, id, callee, 1)
}

pub(crate) fn java_list_of_import_evidence_il(
    interner: &Interner,
    import_in_root: bool,
) -> (Il, NodeId, NodeId, Symbol, LibraryCollectionFactoryContract) {
    let mut b = IlBuilder::new(FileId(0));
    let local = interner.intern("List");
    let lhs = b.add(NodeKind::Var, Payload::Name(local), sp(30), &[]);
    let rhs = b.add(NodeKind::Seq, Payload::None, sp(30), &[]);
    let import = b.add(NodeKind::Assign, Payload::None, sp(30), &[lhs, rhs]);
    let receiver = b.add(NodeKind::Var, Payload::Name(local), sp(31), &[]);
    let callee = b.add(
        NodeKind::Field,
        Payload::Name(interner.intern("of")),
        sp(32),
        &[receiver],
    );
    let arg = b.add(NodeKind::Lit, Payload::LitInt(1), sp(33), &[]);
    let call = b.add(NodeKind::Call, Payload::None, sp(34), &[callee, arg]);
    let root = if import_in_root {
        b.add(NodeKind::Module, Payload::None, sp(29), &[import, call])
    } else {
        b.add(NodeKind::Func, Payload::None, sp(35), &[call])
    };
    let mut il = finish_il(b, root, Lang::Java);
    let contract = library_java_collection_factory_contract(Lang::Java, "List", "of")
        .expect("List.of contract");
    let binding_symbol = EvidenceKind::Symbol(SymbolEvidenceKind::ImportedBinding {
        module_hash: stable_symbol_hash("java.util"),
        exported_hash: stable_symbol_hash("List"),
    });
    il.evidence.push(evidence(
        0,
        EvidenceAnchor::binding(sp(30), stable_symbol_hash("List")),
        binding_symbol,
        EvidenceStatus::Asserted,
    ));
    il.evidence.push(evidence_with_dependencies(
        1,
        EvidenceAnchor::node(sp(31), NodeKind::Var),
        binding_symbol,
        EvidenceStatus::Asserted,
        vec![EvidenceId(0)],
    ));
    il.evidence.push(java_stdlib_collection_factory_record(
        2,
        sp(34),
        contract,
        1,
        EvidenceStatus::Asserted,
        &[1],
    ));
    (il, call, root, local, contract)
}
