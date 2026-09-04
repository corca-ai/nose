use nose_il::{
    stable_symbol_hash, Builtin, EvidenceAnchor, EvidenceId, EvidenceKind, EvidenceRecord,
    EvidenceStatus, FileId, FileMeta, Il, IlBuilder, Interner, Lang, NodeId, NodeKind, Payload,
    SequenceSurfaceKind, SourceCallKind, SourceFactKind, Span, SymbolEvidenceKind,
};
pub(super) use nose_semantics::test_support::{
    builtin_library_api_test_evidence_with_dependencies, compat_test_asserted_evidence as evidence,
    guava_immutable_map_eleven_entry_payloads, guava_immutable_map_of_test_il,
    language_core_test_asserted_evidence as language_core_evidence,
    method_call_library_api_test_evidence_with_dependencies, GuavaImmutableMapFixtureImportRhs,
    GuavaImmutableMapFixtureOptions, GuavaImmutableMapFixtureRoot,
    GuavaImmutableMapFixtureSpanLines, LibraryApiTestContract,
};
use nose_semantics::{
    LibraryApiCalleeContract, LibraryApiContractId, LibraryCollectionFactoryContract,
    LibraryMapFactoryContract, JAVA_STDLIB_COLLECTION_FACTORY_PACK_ID,
    JAVA_STDLIB_COLLECTION_FACTORY_PRODUCER_ID, JS_LIKE_BUILTIN_COLLECTION_CONSTRUCTOR_PACK_ID,
    JS_LIKE_BUILTIN_COLLECTION_CONSTRUCTOR_PRODUCER_ID, PYTHON_BUILTIN_COLLECTION_FACTORY_PACK_ID,
    PYTHON_BUILTIN_COLLECTION_FACTORY_PRODUCER_ID, SWIFT_STDLIB_COLLECTION_FACTORY_PACK_ID,
    SWIFT_STDLIB_COLLECTION_FACTORY_PRODUCER_ID,
};

pub(super) fn sp(line: u32) -> Span {
    Span::new(FileId(0), line, line, line, line)
}

pub(super) fn language_core_symbol_evidence(
    id: u32,
    lang: Lang,
    anchor: EvidenceAnchor,
    symbol: SymbolEvidenceKind,
    dependencies: Vec<EvidenceId>,
) -> EvidenceRecord {
    language_core_evidence(id, lang, anchor, EvidenceKind::Symbol(symbol), dependencies)
}

pub(super) fn sequence_surface_evidence(
    id: u32,
    lang: Lang,
    span: Span,
    surface: SequenceSurfaceKind,
) -> EvidenceRecord {
    language_core_evidence(
        id,
        lang,
        EvidenceAnchor::sequence(span),
        EvidenceKind::SequenceSurface(surface),
        Vec::new(),
    )
}

pub(super) fn library_api_contract_evidence(
    id: u32,
    call_span: Span,
    contract_id: LibraryApiContractId,
    callee: LibraryApiCalleeContract,
    arity: u16,
    dependencies: Vec<EvidenceId>,
) -> EvidenceRecord {
    builtin_library_api_test_evidence_with_dependencies(
        id,
        call_span,
        LibraryApiTestContract {
            id: contract_id,
            callee,
            arity,
        },
        EvidenceStatus::Asserted,
        dependencies,
    )
}

pub(super) fn js_like_builtin_collection_constructor_evidence(
    id: u32,
    call_span: Span,
    contract_id: LibraryApiContractId,
    callee: LibraryApiCalleeContract,
    arity: u16,
    dependencies: Vec<EvidenceId>,
) -> EvidenceRecord {
    let mut record =
        library_api_contract_evidence(id, call_span, contract_id, callee, arity, dependencies);
    record.provenance.pack_hash = Some(stable_symbol_hash(
        JS_LIKE_BUILTIN_COLLECTION_CONSTRUCTOR_PACK_ID,
    ));
    record.provenance.rule_hash = Some(stable_symbol_hash(
        JS_LIKE_BUILTIN_COLLECTION_CONSTRUCTOR_PRODUCER_ID,
    ));
    record
}

pub(super) fn python_builtin_collection_factory_evidence(
    id: u32,
    call_span: Span,
    contract: LibraryCollectionFactoryContract,
    arity: u16,
    dependencies: Vec<EvidenceId>,
) -> EvidenceRecord {
    let mut record = library_api_contract_evidence(
        id,
        call_span,
        contract.id,
        contract.callee,
        arity,
        dependencies,
    );
    record.provenance.pack_hash = Some(stable_symbol_hash(
        PYTHON_BUILTIN_COLLECTION_FACTORY_PACK_ID,
    ));
    record.provenance.rule_hash = Some(stable_symbol_hash(
        PYTHON_BUILTIN_COLLECTION_FACTORY_PRODUCER_ID,
    ));
    record
}

pub(super) fn swift_stdlib_collection_factory_evidence(
    id: u32,
    call_span: Span,
    contract: LibraryCollectionFactoryContract,
    arity: u16,
    dependencies: Vec<EvidenceId>,
) -> EvidenceRecord {
    let mut record = library_api_contract_evidence(
        id,
        call_span,
        contract.id,
        contract.callee,
        arity,
        dependencies,
    );
    record.provenance.pack_hash = Some(stable_symbol_hash(SWIFT_STDLIB_COLLECTION_FACTORY_PACK_ID));
    record.provenance.rule_hash = Some(stable_symbol_hash(
        SWIFT_STDLIB_COLLECTION_FACTORY_PRODUCER_ID,
    ));
    record
}

pub(super) fn swift_stdlib_map_factory_evidence(
    id: u32,
    call_span: Span,
    contract: LibraryMapFactoryContract,
    arity: u16,
    dependencies: Vec<EvidenceId>,
) -> EvidenceRecord {
    let mut record = library_api_contract_evidence(
        id,
        call_span,
        contract.id,
        contract.callee,
        arity,
        dependencies,
    );
    record.provenance.pack_hash = Some(stable_symbol_hash(SWIFT_STDLIB_COLLECTION_FACTORY_PACK_ID));
    record.provenance.rule_hash = Some(stable_symbol_hash(
        SWIFT_STDLIB_COLLECTION_FACTORY_PRODUCER_ID,
    ));
    record
}

pub(super) fn method_call_library_api_evidence(
    id: u32,
    lang: Lang,
    method: &str,
    call_span: Span,
    arity: usize,
    dependencies: Vec<EvidenceId>,
) -> EvidenceRecord {
    method_call_library_api_test_evidence_with_dependencies(
        id,
        lang,
        method,
        call_span,
        arity,
        dependencies,
    )
}

/// Push the `List.of(…)`-shaped factory contract plus the dependent `contains`
/// method-call evidence used by the Java collection-factory tests.
pub(super) fn push_java_factory_contract_evidence(
    il: &mut Il,
    contract_id: LibraryApiContractId,
    callee: LibraryApiCalleeContract,
) {
    let mut record =
        library_api_contract_evidence(2, sp(25), contract_id, callee, 2, vec![EvidenceId(1)]);
    record.provenance.pack_hash = Some(stable_symbol_hash(JAVA_STDLIB_COLLECTION_FACTORY_PACK_ID));
    record.provenance.rule_hash = Some(stable_symbol_hash(
        JAVA_STDLIB_COLLECTION_FACTORY_PRODUCER_ID,
    ));
    il.push_evidence(record);
    il.push_evidence(method_call_library_api_evidence(
        3,
        Lang::Java,
        "contains",
        sp(28),
        1,
        vec![EvidenceId(2)],
    ));
}

pub(super) fn js_new_set_il(interner: &Interner) -> (Il, NodeId) {
    let mut b = IlBuilder::new(FileId(0));
    let set = b.add(
        NodeKind::Var,
        Payload::Name(interner.intern("Set")),
        sp(10),
        &[],
    );
    let one = b.add(NodeKind::Lit, Payload::LitInt(1), sp(11), &[]);
    let array = b.add(
        NodeKind::Seq,
        Payload::Name(interner.intern("array")),
        sp(12),
        &[one],
    );
    let call = b.add(NodeKind::Call, Payload::None, sp(13), &[set, array]);
    let root = b.add(NodeKind::Block, Payload::None, sp(13), &[call]);
    let mut il = b.finish(
        root,
        FileMeta {
            path: "t.js".into(),
            lang: Lang::JavaScript,
        },
        Vec::new(),
        Vec::new(),
    );
    il.push_evidence(evidence(
        0,
        EvidenceAnchor::source_span(sp(13)),
        EvidenceKind::Source(SourceFactKind::Call(SourceCallKind::Construct)),
        Vec::new(),
    ));
    il.push_evidence(language_core_symbol_evidence(
        1,
        Lang::JavaScript,
        EvidenceAnchor::node(sp(10), NodeKind::Var),
        SymbolEvidenceKind::UnshadowedGlobal {
            name_hash: stable_symbol_hash("Set"),
        },
        Vec::new(),
    ));
    il.push_evidence(sequence_surface_evidence(
        2,
        Lang::JavaScript,
        sp(12),
        SequenceSurfaceKind::Collection,
    ));
    (il, call)
}

pub(super) fn js_typeof_call_il(interner: &Interner) -> (Il, NodeId) {
    let mut b = IlBuilder::new(FileId(0));
    let callee = b.add(
        NodeKind::Var,
        Payload::Name(interner.intern("typeof")),
        sp(42),
        &[],
    );
    let arg = b.add(NodeKind::Lit, Payload::LitInt(1), sp(43), &[]);
    let call = b.add(NodeKind::Call, Payload::None, sp(44), &[callee, arg]);
    let root = b.add(NodeKind::Block, Payload::None, sp(44), &[call]);
    let il = b.finish(
        root,
        FileMeta {
            path: "t.ts".into(),
            lang: Lang::TypeScript,
        },
        Vec::new(),
        Vec::new(),
    );
    (il, call)
}

pub(super) fn raw_array_seq_il(interner: &Interner) -> (Il, NodeId) {
    let mut b = IlBuilder::new(FileId(0));
    let one = b.add(NodeKind::Lit, Payload::LitInt(1), sp(60), &[]);
    let seq = b.add(
        NodeKind::Seq,
        Payload::Name(interner.intern("array")),
        sp(61),
        &[one],
    );
    let root = b.add(NodeKind::Block, Payload::None, sp(59), &[seq]);
    let il = b.finish(
        root,
        FileMeta {
            path: "t.js".into(),
            lang: Lang::JavaScript,
        },
        Vec::new(),
        Vec::new(),
    );
    (il, seq)
}

pub(super) fn ts_contains_call_il(interner: &Interner) -> (Il, NodeId, Span) {
    let mut b = IlBuilder::new(FileId(0));
    let receiver_span = sp(50);
    let receiver = b.add(
        NodeKind::Var,
        Payload::Name(interner.intern("xs")),
        receiver_span,
        &[],
    );
    let callee = b.add(
        NodeKind::Field,
        Payload::Name(interner.intern("includes")),
        sp(51),
        &[receiver],
    );
    let item = b.add(NodeKind::Lit, Payload::LitInt(7), sp(52), &[]);
    let call = b.add(NodeKind::Call, Payload::None, sp(53), &[callee, item]);
    let root = b.add(NodeKind::Block, Payload::None, sp(49), &[call]);
    let il = b.finish(
        root,
        FileMeta {
            path: "t.ts".into(),
            lang: Lang::TypeScript,
        },
        Vec::new(),
        Vec::new(),
    );
    (il, call, receiver_span)
}

pub(super) fn canonical_python_abs_il() -> (Il, NodeId) {
    let mut b = IlBuilder::new(FileId(0));
    let arg = b.add(NodeKind::Lit, Payload::LitInt(-1), sp(71), &[]);
    let call = b.add(
        NodeKind::Call,
        Payload::Builtin(Builtin::Abs),
        sp(72),
        &[arg],
    );
    let root = b.add(NodeKind::Block, Payload::None, sp(70), &[call]);
    let il = b.finish(
        root,
        FileMeta {
            path: "t.py".into(),
            lang: Lang::Python,
        },
        Vec::new(),
        Vec::new(),
    );
    (il, call)
}
