use super::*;
use nose_il::Builtin;

#[test]
fn js_static_index_membership_emits_occurrence_evidence() {
    let interner = Interner::new();
    let il = crate::lower_source(
        FileId(0),
        "index.js",
        b"function f(value) { return [\"red\", \"blue\"].indexOf(value) !== -1; }\n",
        Lang::JavaScript,
        &interner,
    )
    .expect("js lowering should succeed");
    let contract =
        library_static_index_membership_contract(Lang::JavaScript, "indexOf", 1).unwrap();
    let api = il
        .evidence
        .iter()
        .find(|record| {
            matches!(
                record.kind,
                EvidenceKind::LibraryApi(LibraryApiEvidenceKind::Contract {
                    contract_hash,
                    callee_hash,
                    ..
                }) if contract_hash == library_api_contract_id_hash(contract.id)
                    && callee_hash == library_api_callee_contract_hash(contract.callee)
            )
        })
        .expect("static index membership should emit a LibraryApi occurrence");
    assert_eq!(
        api.provenance.pack_hash,
        Some(stable_symbol_hash(
            nose_semantics::JS_LIKE_BUILTIN_STATIC_INDEX_MEMBERSHIP_PACK_ID
        ))
    );
    assert_eq!(
        api.provenance.rule_hash,
        Some(stable_symbol_hash(
            JS_LIKE_BUILTIN_STATIC_INDEX_MEMBERSHIP_PRODUCER_ID
        ))
    );
    assert!(api.dependencies.iter().any(|id| {
        il.evidence_record_by_id(*id).is_some_and(|record| {
            matches!(
                record.kind,
                EvidenceKind::SequenceSurface(SequenceSurfaceKind::Collection)
            )
        })
    }));
    let collection_domain = il
        .evidence
        .iter()
        .find(|record| {
            matches!(
                record.anchor,
                EvidenceAnchor::Node {
                    kind: NodeKind::Seq,
                    ..
                }
            ) && record.kind == EvidenceKind::Domain(DomainEvidence::Collection)
        })
        .expect("collection literal should emit receiver-domain evidence");
    assert!(collection_domain.dependencies.iter().any(|id| {
        il.evidence_record_by_id(*id).is_some_and(|record| {
            matches!(
                record.kind,
                EvidenceKind::SequenceSurface(SequenceSurfaceKind::Collection)
            )
        })
    }));
}

#[test]
fn collection_literal_assignments_emit_binding_domain_evidence() {
    let interner = Interner::new();
    let il = lower_fixture(
        "binding_literal.rs",
        b"fn f(value: &str) -> bool { let values = [\"red\", \"blue\"]; values.contains(&value) }",
        Lang::Rust,
        &interner,
    );
    assert_eq!(
        binding_domain_record_count(&il.evidence, DomainEvidence::Collection),
        1,
        "collection literal assignments should expose dependency-backed binding-domain evidence"
    );
    let binding_domain = il
        .evidence
        .iter()
        .find(|record| {
            matches!(record.anchor, EvidenceAnchor::Binding { .. })
                && record.kind == EvidenceKind::Domain(DomainEvidence::Collection)
        })
        .expect("binding domain evidence");
    assert!(binding_domain.dependencies.iter().any(|id| {
        il.evidence_record_by_id(*id).is_some_and(|record| {
            record.kind == EvidenceKind::Domain(DomainEvidence::Collection)
                && matches!(
                    record.anchor,
                    EvidenceAnchor::Node {
                        kind: NodeKind::Seq,
                        ..
                    }
                )
        })
    }));
}

#[test]
fn core_lowering_emits_java_and_regex_library_api_occurrences() {
    let interner = Interner::new();
    let mut lo = Lowering::new(FileId(0), b"", Lang::Java, &interner);
    import_binding(&mut lo, sp_at(1), "List", "java.util", "List");
    let list = lo.var("List", sp_at(2));
    let callee = lo.add(
        NodeKind::Field,
        Payload::Name(interner.intern("of")),
        sp_at(3),
        &[list],
    );
    let item = lo.int_lit("1", sp_at(4));
    lo.add(NodeKind::Call, Payload::None, sp_at(5), &[callee, item]);
    let contract = library_java_collection_factory_contract(Lang::Java, "List", "of")
        .expect("List.of contract");
    assert_eq!(
        library_api_evidence_count(
            &lo,
            library_api_contract_id_hash(contract.id),
            library_api_callee_contract_hash(contract.callee),
        ),
        1
    );

    let mut lo = Lowering::new(FileId(0), b"", Lang::JavaScript, &interner);
    let regex = lo.str_lit("/x/", sp_at(10));
    lo.record_source_fact(
        sp_at(10),
        SourceFactKind::Literal(nose_il::SourceLiteralKind::Regex),
    );
    let callee = lo.add(
        NodeKind::Field,
        Payload::Name(interner.intern("test")),
        sp_at(11),
        &[regex],
    );
    let subject = lo.var("subject", sp_at(12));
    lo.add(NodeKind::Call, Payload::None, sp_at(13), &[callee, subject]);
    let contract =
        library_regex_test_contract(Lang::JavaScript, "test", 1).expect("regex test contract");
    assert_eq!(
        library_api_evidence_count(
            &lo,
            library_api_contract_id_hash(contract.id),
            library_api_callee_contract_hash(contract.callee),
        ),
        1
    );
    let regex_records = contract_api_records(&lo.evidence, contract.id, contract.callee);
    assert_eq!(
        regex_records[0].provenance.pack_hash,
        Some(stable_symbol_hash(
            nose_semantics::JS_LIKE_BUILTIN_REGEX_PACK_ID
        ))
    );
    assert_eq!(
        regex_records[0].provenance.rule_hash,
        Some(stable_symbol_hash(JS_LIKE_BUILTIN_REGEX_PRODUCER_ID))
    );
}

#[test]
fn generic_source_facts_use_builtin_language_pack_provenance() {
    let interner = Interner::new();
    let mut lo = Lowering::new(FileId(0), b"", Lang::Python, &interner);
    lo.record_source_fact(
        sp_at(1),
        SourceFactKind::Comprehension(SourceComprehensionKind::PythonListComprehension),
    );
    let record = lo.evidence.last().expect("source fact evidence");
    assert_eq!(
        record.provenance.pack_hash,
        Some(stable_symbol_hash(nose_semantics::PYTHON_LANGUAGE_PACK_ID))
    );
    assert_eq!(
        record.provenance.rule_hash,
        Some(stable_symbol_hash(
            nose_semantics::PYTHON_SOURCE_FACT_PRODUCER_ID
        ))
    );
}

#[test]
fn generic_lowering_evidence_uses_builtin_language_core_provenance() {
    let interner = Interner::new();
    let mut lo = Lowering::new(FileId(0), b"", Lang::Java, &interner);
    lo.record_evidence(
        EvidenceAnchor::node(sp_at(1), NodeKind::Var),
        EvidenceKind::Place(PlaceEvidenceKind::SelfReceiver),
        "place_self_receiver",
    );
    let record = lo.evidence.last().expect("generic evidence");
    assert_eq!(
        record.provenance.pack_hash,
        Some(stable_symbol_hash(nose_semantics::JAVA_LANGUAGE_PACK_ID))
    );
    assert_eq!(
        record.provenance.rule_hash,
        Some(stable_symbol_hash(
            nose_semantics::JAVA_LANGUAGE_CORE_PRODUCER_ID
        ))
    );
}

#[test]
fn core_lowering_emits_effect_and_place_evidence() {
    let interner = Interner::new();
    let mut lo = Lowering::new(FileId(0), b"", Lang::Java, &interner);

    let receiver = lo.add(
        NodeKind::Var,
        Payload::Name(interner.intern("this")),
        sp_at(1),
        &[],
    );
    let field = lo.add(
        NodeKind::Field,
        Payload::Name(interner.intern("value")),
        sp_at(2),
        &[receiver],
    );
    let value = lo.add(
        NodeKind::Var,
        Payload::Name(interner.intern("next")),
        sp_at(3),
        &[],
    );
    let assign = lo.add(NodeKind::Assign, Payload::None, sp_at(4), &[field, value]);
    let index = lo.add(NodeKind::Index, Payload::None, sp_at(5), &[receiver, value]);
    let index_assign = lo.add(NodeKind::Assign, Payload::None, sp_at(6), &[index, value]);
    let append = lo.add(
        NodeKind::Call,
        Payload::Builtin(Builtin::Append),
        sp_at(7),
        &[receiver, value],
    );

    let field_hash = stable_symbol_hash("value");
    let self_receiver = lo
        .evidence
        .iter()
        .find(|record| {
            record.anchor == EvidenceAnchor::node(sp_at(1), NodeKind::Var)
                && record.kind == EvidenceKind::Place(PlaceEvidenceKind::SelfReceiver)
        })
        .expect("Java this should emit self-receiver place evidence");
    let self_field = lo
        .evidence
        .iter()
        .find(|record| {
            record.anchor == EvidenceAnchor::node(sp_at(2), NodeKind::Field)
                && record.kind == EvidenceKind::Place(PlaceEvidenceKind::SelfField { field_hash })
        })
        .expect("Java this.field should emit self-field place evidence");
    assert_eq!(self_field.dependencies, vec![self_receiver.id]);
    let self_field_write = lo
        .evidence
        .iter()
        .find(|record| {
            record.anchor == EvidenceAnchor::node(sp_at(4), NodeKind::Assign)
                && record.kind
                    == EvidenceKind::Effect(EffectEvidenceKind::SelfFieldWrite { field_hash })
        })
        .expect("Java this.field assignment should emit self-field write evidence");
    assert_eq!(self_field_write.dependencies, vec![self_field.id]);
    assert!(lo.evidence.iter().any(|record| {
        record.anchor == EvidenceAnchor::node(sp_at(6), NodeKind::Assign)
            && record.kind == EvidenceKind::Effect(EffectEvidenceKind::NonOverloadableIndexWrite)
    }));
    assert!(!lo.evidence.iter().any(|record| {
        record.anchor == EvidenceAnchor::node(sp_at(7), NodeKind::Call)
            && record.kind == EvidenceKind::Effect(EffectEvidenceKind::BuilderAppendCall)
    }));
    assert_ne!(assign, index_assign);
    assert_ne!(append, receiver);
}
