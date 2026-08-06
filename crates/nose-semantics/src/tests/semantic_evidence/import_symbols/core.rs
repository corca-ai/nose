use super::*;

#[test]
fn import_fact_contracts_resolve_evidence_only_binding_and_namespace_proofs() {
    let mut b = IlBuilder::new(FileId(0));
    let collections = b.add(
        NodeKind::Lit,
        Payload::LitStr(stable_symbol_hash("collections")),
        sp(1),
        &[],
    );
    let deque = b.add(
        NodeKind::Lit,
        Payload::LitStr(stable_symbol_hash("deque")),
        sp(1),
        &[],
    );
    let binding = b.add(NodeKind::Seq, Payload::None, sp(1), &[collections, deque]);
    let math = b.add(
        NodeKind::Lit,
        Payload::LitStr(stable_symbol_hash("math")),
        sp(2),
        &[],
    );
    let namespace = b.add(NodeKind::Seq, Payload::None, sp(2), &[math]);
    let raw_coordinates = b.add(NodeKind::Seq, Payload::None, sp(3), &[math]);
    let root = b.add(
        NodeKind::Module,
        Payload::None,
        sp(1),
        &[binding, namespace, raw_coordinates],
    );
    let mut il = finish_il(b, root, Lang::Python);

    assert_eq!(
        import_fact_contract(ImportFactKind::Binding).channel,
        ChannelEligibility::ExactProven
    );
    assert_eq!(import_fact_evidence_rhs(&il, binding), None);
    assert_eq!(import_fact_evidence_rhs(&il, namespace), None);

    il.evidence.push(import_fact_evidence(
        0,
        Lang::Python,
        sp(1),
        binding_import_fact("collections", "deque"),
        EvidenceStatus::Asserted,
    ));
    il.evidence.push(import_fact_evidence(
        1,
        Lang::Python,
        sp(2),
        namespace_import_fact("math"),
        EvidenceStatus::Asserted,
    ));

    assert_eq!(
        import_fact_evidence_rhs(&il, binding),
        Some(ImportFact {
            kind: ImportFactKind::Binding,
            module_hash: stable_symbol_hash("collections"),
            exported_hash: Some(stable_symbol_hash("deque")),
        })
    );
    assert_eq!(
        import_fact_evidence_rhs(&il, namespace),
        Some(ImportFact {
            kind: ImportFactKind::Namespace,
            module_hash: stable_symbol_hash("math"),
            exported_hash: None,
        })
    );
    assert_eq!(import_fact_evidence_rhs(&il, raw_coordinates), None);
}

#[test]
fn import_fact_admission_requires_matching_language_core_provenance() {
    let kind = binding_import_fact("collections", "deque");
    let cases = [
        (
            "external emitter",
            EvidenceProvenance {
                emitter: EvidenceEmitter::External,
                pack_hash: Some(stable_symbol_hash("com.example.pack")),
                rule_hash: Some(stable_symbol_hash("python.language.core")),
            },
            Vec::new(),
        ),
        (
            "compatibility pack",
            EvidenceProvenance {
                emitter: EvidenceEmitter::Builtin,
                pack_hash: Some(stable_symbol_hash(BUILTIN_COMPAT_PACK_ID)),
                rule_hash: Some(stable_symbol_hash("import_fact")),
            },
            Vec::new(),
        ),
        (
            "wrong language",
            language_core_provenance(Lang::Java),
            Vec::new(),
        ),
        (
            "missing pack",
            EvidenceProvenance {
                emitter: EvidenceEmitter::Builtin,
                pack_hash: None,
                rule_hash: Some(stable_symbol_hash("python.language.core")),
            },
            Vec::new(),
        ),
        (
            "broken dependency",
            language_core_provenance(Lang::Python),
            vec![EvidenceId(99)],
        ),
    ];

    for (reason, provenance, dependencies) in cases {
        let mut probe = ImportFactProbe::binding(10, "collections", "deque");
        probe.push(0, kind, provenance, dependencies);
        assert_eq!(probe.resolved(), None, "{reason} must be rejected");
    }
}

#[test]
fn imported_literal_producer_requires_matching_language_core_provenance() {
    let mut b = IlBuilder::new(FileId(0));
    let value = b.add(NodeKind::Seq, Payload::None, sp(15), &[]);
    let root = b.add(NodeKind::Module, Payload::None, sp(15), &[value]);
    let mut il = finish_il(b, root, Lang::Python);
    let kind = EvidenceKind::Import(ImportEvidenceKind::ImmutableLiteralExport {
        module_hash: stable_symbol_hash("tables"),
        exported_hash: stable_symbol_hash("LOOKUP"),
        root_kind: NodeKind::Seq,
    });

    il.evidence.push(imported_literal_evidence_with_provenance(
        0,
        sp(15),
        kind,
        EvidenceProvenance {
            emitter: EvidenceEmitter::External,
            pack_hash: Some(stable_symbol_hash("com.example.pack")),
            rule_hash: Some(stable_symbol_hash("python.language.core")),
        },
        EvidenceStatus::Asserted,
        Vec::new(),
    ));
    assert!(!imported_literal_producer_evidence_for_node(&il, value));

    il.evidence.clear();
    il.evidence.push(imported_literal_evidence_with_provenance(
        0,
        sp(15),
        kind,
        EvidenceProvenance {
            emitter: EvidenceEmitter::Builtin,
            pack_hash: Some(stable_symbol_hash(BUILTIN_COMPAT_PACK_ID)),
            rule_hash: Some(stable_symbol_hash("module_immutable_literal_export")),
        },
        EvidenceStatus::Asserted,
        Vec::new(),
    ));
    assert!(!imported_literal_producer_evidence_for_node(&il, value));

    il.evidence.clear();
    il.evidence.push(imported_literal_evidence_with_provenance(
        0,
        sp(15),
        kind,
        language_core_provenance(Lang::Java),
        EvidenceStatus::Asserted,
        Vec::new(),
    ));
    assert!(!imported_literal_producer_evidence_for_node(&il, value));

    il.evidence.clear();
    il.evidence.push(imported_literal_evidence_with_provenance(
        0,
        sp(15),
        kind,
        language_core_provenance(Lang::Python),
        EvidenceStatus::Asserted,
        vec![EvidenceId(99)],
    ));
    assert!(!imported_literal_producer_evidence_for_node(&il, value));

    il.evidence.clear();
    il.evidence.push(imported_literal_evidence_with_provenance(
        0,
        sp(15),
        kind,
        language_core_provenance(Lang::Python),
        EvidenceStatus::Asserted,
        Vec::new(),
    ));
    assert!(imported_literal_producer_evidence_for_node(&il, value));

    il.evidence.push(imported_literal_evidence_with_provenance(
        1,
        sp(15),
        kind,
        EvidenceProvenance {
            emitter: EvidenceEmitter::Builtin,
            pack_hash: Some(stable_symbol_hash(BUILTIN_COMPAT_PACK_ID)),
            rule_hash: Some(stable_symbol_hash("module_immutable_literal_export")),
        },
        EvidenceStatus::Asserted,
        Vec::new(),
    ));
    assert!(
        !imported_literal_producer_evidence_for_node(&il, value),
        "mixed valid and invalid imported-literal proof must stay closed"
    );
}

#[test]
fn ambiguous_import_evidence_stays_closed_without_raw_seq_fallback() {
    let mut probe = ImportFactProbe::binding(10, "collections", "deque");
    probe.push(
        0,
        namespace_import_fact("math"),
        language_core_provenance(Lang::Python),
        Vec::new(),
    );

    assert_eq!(
        probe.resolved(),
        Some(ImportFact {
            kind: ImportFactKind::Namespace,
            module_hash: stable_symbol_hash("math"),
            exported_hash: None,
        })
    );

    probe.push(
        1,
        binding_import_fact("collections", "deque"),
        language_core_provenance(Lang::Python),
        Vec::new(),
    );
    assert_eq!(probe.resolved(), None);
}

#[test]
fn imported_symbol_identity_does_not_fall_back_to_raw_import_seq() {
    let interner = Interner::new();
    let mut b = IlBuilder::new(FileId(0));
    let local = interner.intern("deque");
    let module = b.add(
        NodeKind::Lit,
        Payload::LitStr(stable_symbol_hash("collections")),
        sp(30),
        &[],
    );
    let exported = b.add(
        NodeKind::Lit,
        Payload::LitStr(stable_symbol_hash("deque")),
        sp(30),
        &[],
    );
    let lhs = b.add(NodeKind::Var, Payload::Name(local), sp(30), &[]);
    let rhs = b.add(NodeKind::Seq, Payload::None, sp(30), &[module, exported]);
    let assignment = b.add(NodeKind::Assign, Payload::None, sp(30), &[lhs, rhs]);
    let use_site = b.add(NodeKind::Var, Payload::Name(local), sp(31), &[]);
    let root = b.add(
        NodeKind::Module,
        Payload::None,
        sp(30),
        &[assignment, use_site],
    );
    let mut il = finish_il(b, root, Lang::Python);

    assert_eq!(import_fact_evidence_rhs(&il, rhs), None);
    assert!(!imported_binding_symbol(
        &il,
        &interner,
        use_site,
        "collections",
        "deque"
    ));

    il.evidence.push(evidence(
        0,
        EvidenceAnchor::binding(sp(30), stable_symbol_hash("deque")),
        EvidenceKind::Symbol(SymbolEvidenceKind::ImportedBinding {
            module_hash: stable_symbol_hash("collections"),
            exported_hash: stable_symbol_hash("deque"),
        }),
        EvidenceStatus::Asserted,
    ));
    assert!(imported_binding_symbol(
        &il,
        &interner,
        use_site,
        "collections",
        "deque"
    ));
}
