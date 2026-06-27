use super::super::resolve_imported_immutable_bindings;
use super::super::snapshot::push_builtin_evidence_with_dependencies;
use nose_il::{
    stable_symbol_hash, EffectEvidenceKind, EvidenceAnchor, EvidenceEmitter, EvidenceId,
    EvidenceKind, EvidenceProvenance, EvidenceRecord, EvidenceStatus, FileId, FileMeta, Il,
    IlBuilder, ImportEvidenceKind, Interner, Lang, NodeId, NodeKind, Payload, SequenceSurfaceKind,
    Span, Symbol, SymbolEvidenceKind,
};

pub(super) fn module_with_binding_method(method: &str) -> (Il, Interner, Symbol, NodeId) {
    let interner = Interner::new();
    let mut b = IlBuilder::new(FileId(0));
    let span = Span::new(FileId(0), 0, 1, 1, 1);
    let lookup = interner.intern("LOOKUP");
    let lhs = b.add(NodeKind::Var, Payload::Name(lookup), span, &[]);
    let rhs = b.add(
        NodeKind::Seq,
        Payload::Name(interner.intern("array")),
        span,
        &[],
    );
    let assign = b.add(NodeKind::Assign, Payload::None, span, &[lhs, rhs]);
    let receiver = b.add(NodeKind::Var, Payload::Name(lookup), span, &[]);
    let field = b.add(
        NodeKind::Field,
        Payload::Name(interner.intern(method)),
        span,
        &[receiver],
    );
    let arg = b.add(NodeKind::Lit, Payload::LitInt(2), span, &[]);
    let call = b.add(NodeKind::Call, Payload::None, span, &[field, arg]);
    let stmt = b.add(NodeKind::ExprStmt, Payload::None, span, &[call]);
    let root = b.add(NodeKind::Module, Payload::None, span, &[assign, stmt]);
    let mut il = b.finish(
        root,
        FileMeta {
            path: "tables.js".into(),
            lang: Lang::JavaScript,
        },
        Vec::new(),
        Vec::new(),
    );
    push_builtin_evidence_with_dependencies(
        &mut il,
        EvidenceAnchor::node(span, NodeKind::Assign),
        EvidenceKind::Effect(EffectEvidenceKind::BindingWrite),
        "effect_binding_write_test",
        Vec::new(),
    );
    push_builtin_evidence_with_dependencies(
        &mut il,
        EvidenceAnchor::node(span, NodeKind::Call),
        EvidenceKind::Effect(EffectEvidenceKind::OpaqueArgumentEscape),
        "effect_opaque_argument_escape_test",
        Vec::new(),
    );
    if let Some(contract) =
        nose_semantics::module_binding_mutating_method_contract(Lang::JavaScript, method, 1)
    {
        push_builtin_evidence_with_dependencies(
            &mut il,
            EvidenceAnchor::node(span, NodeKind::Call),
            EvidenceKind::Effect(contract.effect),
            "effect_receiver_mutation_test",
            Vec::new(),
        );
    }
    (il, interner, lookup, assign)
}

pub(super) fn java_provider_and_importer(provider_src: &str, interner: &Interner) -> (Il, Il) {
    java_provider_and_importer_src(
        provider_src,
        "import static Tables.LOOKUP;\nclass Consumer {}",
        interner,
    )
}

pub(super) fn java_provider_and_importer_src(
    provider_src: &str,
    importer_src: &str,
    interner: &Interner,
) -> (Il, Il) {
    let provider = crate::lower_source(
        FileId(0),
        "Tables.java",
        provider_src.as_bytes(),
        Lang::Java,
        interner,
    )
    .expect("lower Java provider");
    let importer = crate::lower_source(
        FileId(1),
        "Consumer.java",
        importer_src.as_bytes(),
        Lang::Java,
        interner,
    )
    .expect("lower Java importer");
    (provider, importer)
}

pub(super) fn snapshot_count(il: &Il) -> usize {
    il.evidence
        .iter()
        .filter(|record| {
            matches!(
                record.kind,
                EvidenceKind::Import(ImportEvidenceKind::ImportedLiteralSnapshot { .. })
            )
        })
        .count()
}

pub(super) fn resolve_importer(provider: Il, importer: Il, interner: &Interner) -> Il {
    let mut files = vec![provider, importer];
    resolve_imported_immutable_bindings(&mut files, interner);
    files.remove(1)
}

pub(super) fn resolve_snapshot_count(provider: Il, importer: Il, interner: &Interner) -> usize {
    snapshot_count(&resolve_importer(provider, importer, interner))
}

pub(super) fn remove_library_api_evidence_by_rule(il: &mut Il, rule: &str) {
    let rule_hash = stable_symbol_hash(rule);
    il.evidence.retain(|record| {
        !matches!(record.kind, EvidenceKind::LibraryApi(_))
            || record.provenance.rule_hash != Some(rule_hash)
    });
}

pub(super) fn coordinate_import_binding_assignment(
    file: FileId,
    lang: Lang,
) -> (Il, Interner, Span, NodeId, NodeId) {
    let interner = Interner::new();
    let mut b = IlBuilder::new(file);
    let span = Span::new(file, 0, 1, 1, 1);
    let map = interner.intern("Map");
    let lhs = b.add(NodeKind::Var, Payload::Name(map), span, &[]);
    let module = b.add(
        NodeKind::Lit,
        Payload::LitStr(stable_symbol_hash("java.util")),
        span,
        &[],
    );
    let exported = b.add(
        NodeKind::Lit,
        Payload::LitStr(stable_symbol_hash("Map")),
        span,
        &[],
    );
    let rhs = b.add(NodeKind::Seq, Payload::None, span, &[module, exported]);
    let assign = b.add(NodeKind::Assign, Payload::None, span, &[lhs, rhs]);
    let root = b.add(NodeKind::Module, Payload::None, span, &[assign]);
    let il = b.finish(
        root,
        FileMeta {
            path: "imported.java".into(),
            lang,
        },
        Vec::new(),
        Vec::new(),
    );
    (il, interner, span, assign, rhs)
}

pub(super) fn test_provenance(rule: &str) -> EvidenceProvenance {
    EvidenceProvenance {
        emitter: EvidenceEmitter::External,
        pack_hash: Some(stable_symbol_hash("test.pack")),
        rule_hash: Some(stable_symbol_hash(rule)),
    }
}

pub(super) fn language_core_provenance(lang: Lang) -> EvidenceProvenance {
    let (pack_id, producer_id) = nose_semantics::language_core_evidence_provenance(lang);
    EvidenceProvenance {
        emitter: EvidenceEmitter::Builtin,
        pack_hash: Some(stable_symbol_hash(pack_id)),
        rule_hash: Some(stable_symbol_hash(producer_id)),
    }
}

pub(super) fn add_import_binding_evidence(
    il: &mut Il,
    span: Span,
    status: EvidenceStatus,
) -> EvidenceId {
    let id = EvidenceId(il.evidence.len() as u32);
    il.evidence.push(EvidenceRecord {
        id,
        anchor: EvidenceAnchor::sequence(span),
        kind: EvidenceKind::Import(ImportEvidenceKind::Binding {
            module_hash: stable_symbol_hash("java.util"),
            exported_hash: stable_symbol_hash("Map"),
        }),
        provenance: language_core_provenance(il.meta.lang),
        dependencies: Vec::new(),
        status,
    });
    id
}

pub(super) fn provider_with_lookup_export_evidence(interner: &Interner) -> (Il, NodeId) {
    let mut b = IlBuilder::new(FileId(0));
    let span = Span::new(FileId(0), 4, 12, 1, 1);
    let lookup = interner.intern("LOOKUP");
    let lhs = b.add(NodeKind::Var, Payload::Name(lookup), span, &[]);
    let tag = interner.intern("dictionary");
    let rhs = b.add(NodeKind::Seq, Payload::Name(tag), span, &[]);
    let assign = b.add(NodeKind::Assign, Payload::None, span, &[lhs, rhs]);
    let root = b.add(NodeKind::Module, Payload::None, span, &[assign]);
    let mut provider = b.finish(
        root,
        FileMeta {
            path: "tables.py".into(),
            lang: Lang::Python,
        },
        Vec::new(),
        Vec::new(),
    );
    provider.evidence.push(EvidenceRecord {
        id: EvidenceId(0),
        anchor: EvidenceAnchor::sequence(span),
        kind: EvidenceKind::SequenceSurface(SequenceSurfaceKind::Map),
        provenance: language_core_provenance(Lang::Python),
        dependencies: Vec::new(),
        status: EvidenceStatus::Asserted,
    });
    provider.evidence.push(EvidenceRecord {
        id: EvidenceId(1),
        anchor: EvidenceAnchor::node(span, NodeKind::Seq),
        kind: EvidenceKind::Import(ImportEvidenceKind::ImmutableLiteralExport {
            module_hash: stable_symbol_hash("tables"),
            exported_hash: stable_symbol_hash("LOOKUP"),
            root_kind: NodeKind::Seq,
        }),
        provenance: language_core_provenance(provider.meta.lang),
        dependencies: vec![EvidenceId(0)],
        status: EvidenceStatus::Asserted,
    });
    provider.evidence.push(EvidenceRecord {
        id: EvidenceId(2),
        anchor: EvidenceAnchor::binding(span, stable_symbol_hash("LOOKUP")),
        kind: EvidenceKind::Symbol(SymbolEvidenceKind::ImportedBinding {
            module_hash: stable_symbol_hash("tables"),
            exported_hash: stable_symbol_hash("LOOKUP"),
        }),
        provenance: test_provenance("symbol"),
        dependencies: vec![EvidenceId(0)],
        status: EvidenceStatus::Asserted,
    });
    provider.evidence.push(EvidenceRecord {
        id: EvidenceId(3),
        anchor: EvidenceAnchor::sequence(span),
        kind: EvidenceKind::SequenceSurface(SequenceSurfaceKind::Map),
        provenance: language_core_provenance(Lang::Python),
        dependencies: Vec::new(),
        status: EvidenceStatus::Ambiguous,
    });
    (provider, assign)
}

pub(super) fn lookup_dict_provider(interner: &Interner, lookup: Symbol) -> Il {
    let provider_span = Span::new(FileId(0), 4, 24, 1, 1);
    let mut b = IlBuilder::new(FileId(0));
    let lhs = b.add(NodeKind::Var, Payload::Name(lookup), provider_span, &[]);
    let key = b.add(
        NodeKind::Lit,
        Payload::LitStr(stable_symbol_hash("red")),
        provider_span,
        &[],
    );
    let value = b.add(NodeKind::Lit, Payload::LitInt(1), provider_span, &[]);
    let tag = interner.intern("dictionary");
    let rhs = b.add(
        NodeKind::Seq,
        Payload::Name(tag),
        provider_span,
        &[key, value],
    );
    let assign = b.add(NodeKind::Assign, Payload::None, provider_span, &[lhs, rhs]);
    let root = b.add(NodeKind::Module, Payload::None, provider_span, &[assign]);
    let mut provider = b.finish(
        root,
        FileMeta {
            path: "tables.py".into(),
            lang: Lang::Python,
        },
        Vec::new(),
        Vec::new(),
    );
    provider.evidence.push(EvidenceRecord {
        id: EvidenceId(0),
        anchor: EvidenceAnchor::sequence(provider_span),
        kind: EvidenceKind::SequenceSurface(SequenceSurfaceKind::Map),
        provenance: language_core_provenance(Lang::Python),
        dependencies: Vec::new(),
        status: EvidenceStatus::Asserted,
    });
    provider
}

/// `consumer.py` importer binding `LOOKUP` from `tables` with an asserted
/// static-import proof. Returns the importer and its import assignment.
pub(super) fn lookup_import_consumer(lookup: Symbol) -> (Il, NodeId) {
    lookup_import_consumer_with_lang(lookup, Lang::Python, "consumer.py")
}

pub(super) fn lookup_import_consumer_with_lang(
    lookup: Symbol,
    lang: Lang,
    path: &str,
) -> (Il, NodeId) {
    let import_span = Span::new(FileId(1), 0, 24, 1, 1);
    let mut b = IlBuilder::new(FileId(1));
    let lhs = b.add(NodeKind::Var, Payload::Name(lookup), import_span, &[]);
    let module = b.add(
        NodeKind::Lit,
        Payload::LitStr(stable_symbol_hash("tables")),
        import_span,
        &[],
    );
    let exported = b.add(
        NodeKind::Lit,
        Payload::LitStr(stable_symbol_hash("LOOKUP")),
        import_span,
        &[],
    );
    let import_rhs = b.add(
        NodeKind::Seq,
        Payload::None,
        import_span,
        &[module, exported],
    );
    let import_assign = b.add(
        NodeKind::Assign,
        Payload::None,
        import_span,
        &[lhs, import_rhs],
    );
    let root = b.add(
        NodeKind::Module,
        Payload::None,
        import_span,
        &[import_assign],
    );
    let mut importer = b.finish(
        root,
        FileMeta {
            path: path.into(),
            lang,
        },
        Vec::new(),
        Vec::new(),
    );
    importer.evidence.push(EvidenceRecord {
        id: EvidenceId(0),
        anchor: EvidenceAnchor::sequence(import_span),
        kind: EvidenceKind::Import(ImportEvidenceKind::Binding {
            module_hash: stable_symbol_hash("tables"),
            exported_hash: stable_symbol_hash("LOOKUP"),
        }),
        provenance: language_core_provenance(importer.meta.lang),
        dependencies: Vec::new(),
        status: EvidenceStatus::Asserted,
    });
    (importer, import_assign)
}
