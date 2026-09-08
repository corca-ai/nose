use super::*;
use nose_il::{Builtin, EffectEvidenceKind};
use nose_il::{
    EvidenceEmitter, EvidenceProvenance, FileId, FileMeta, IlBuilder, Lang, SequenceSurfaceKind,
    Span, Unit, UnitKind,
};

fn sp(line: u32) -> Span {
    Span::new(FileId(0), line, line, line, line)
}

fn language_core_provenance(lang: Lang) -> EvidenceProvenance {
    let (pack_id, producer_id) = language_core_evidence_provenance(lang);
    EvidenceProvenance {
        emitter: EvidenceEmitter::Builtin,
        pack_hash: Some(stable_symbol_hash(pack_id)),
        rule_hash: Some(stable_symbol_hash(producer_id)),
    }
}

fn finish(builder: IlBuilder, root: NodeId, lang: Lang) -> Il {
    builder.finish(
        root,
        FileMeta {
            path: "t".into(),
            lang,
        },
        vec![Unit {
            root,
            kind: UnitKind::Function,
            name: None,
            origin: Default::default(),
        }],
        Vec::new(),
    )
}

fn sequence_evidence(id: u32, span: Span, kind: SequenceSurfaceKind, lang: Lang) -> EvidenceRecord {
    EvidenceRecord {
        id: EvidenceId(id),
        anchor: EvidenceAnchor::sequence(span),
        kind: EvidenceKind::SequenceSurface(kind),
        provenance: language_core_provenance(lang),
        dependencies: Vec::new(),
        status: EvidenceStatus::Asserted,
    }
}

fn binding_domain_record<'a>(il: &'a Il, name: &str) -> Option<&'a EvidenceRecord> {
    let local_hash = stable_symbol_hash(name);
    il.evidence.iter().find(|record| {
        matches!(
            record.anchor,
            EvidenceAnchor::Binding {
                local_hash: anchor_hash,
                ..
            } if anchor_hash == local_hash
        ) && matches!(record.kind, EvidenceKind::Domain(_))
    })
}

fn array_assignment(
    b: &mut IlBuilder,
    interner: &Interner,
    name: Symbol,
    assign_span: Span,
    seq_span: Span,
) -> (NodeId, NodeId) {
    let lhs = b.add(NodeKind::Var, Payload::Name(name), assign_span, &[]);
    let seq = b.add(
        NodeKind::Seq,
        Payload::Name(interner.intern("array")),
        seq_span,
        &[],
    );
    let assign = b.add(NodeKind::Assign, Payload::None, assign_span, &[lhs, seq]);
    (assign, seq)
}

fn append_call(b: &mut IlBuilder, name: Symbol, span: Span) -> NodeId {
    let receiver = b.add(NodeKind::Var, Payload::Name(name), span, &[]);
    let item = b.add(NodeKind::Lit, Payload::LitInt(1), span, &[]);
    b.add(
        NodeKind::Call,
        Payload::Builtin(Builtin::Append),
        span,
        &[receiver, item],
    )
}

fn finish_with_sequence_evidence_for_lang(b: IlBuilder, root: NodeId, lang: Lang) -> Il {
    let mut il = finish(b, root, lang);
    il.push_evidence(sequence_evidence(
        0,
        sp(2),
        SequenceSurfaceKind::Collection,
        lang,
    ));
    il
}

fn finish_with_sequence_evidence(b: IlBuilder, root: NodeId) -> Il {
    finish_with_sequence_evidence_for_lang(b, root, Lang::TypeScript)
}

#[derive(Clone, Copy)]
enum MutationCase {
    Direct,
    NestedModule,
    NestedLocal,
}

fn mutation_case_il(interner: &Interner, case: MutationCase) -> Il {
    let xs = interner.intern("xs");
    let mut b = IlBuilder::new(FileId(0));
    let (assign, _) = array_assignment(&mut b, interner, xs, sp(1), sp(2));
    let append_span = match case {
        MutationCase::Direct => sp(3),
        MutationCase::NestedModule => sp(4),
        MutationCase::NestedLocal => sp(5),
    };
    let append = append_call(&mut b, xs, append_span);
    let root = match case {
        MutationCase::Direct => b.add(NodeKind::Func, Payload::None, sp(1), &[assign, append]),
        MutationCase::NestedModule => {
            let body = b.add(NodeKind::Block, Payload::None, sp(4), &[append]);
            let nested = b.add(NodeKind::Func, Payload::None, sp(3), &[body]);
            b.add(NodeKind::Module, Payload::None, sp(1), &[assign, nested])
        }
        MutationCase::NestedLocal => {
            let nested_body = b.add(NodeKind::Block, Payload::None, sp(5), &[append]);
            let nested = b.add(NodeKind::Func, Payload::None, sp(4), &[nested_body]);
            b.add(NodeKind::Func, Payload::None, sp(1), &[assign, nested])
        }
    };
    let mut il = finish_with_sequence_evidence(b, root);
    il.find_or_push_first_party_evidence(
        EvidenceAnchor::node(append_span, NodeKind::Call),
        EvidenceKind::Effect(EffectEvidenceKind::BuilderAppendCall),
        nose_semantics::BUILTIN_COMPAT_PACK_ID,
        "test_builder_append_effect",
        Vec::new(),
    );
    il
}

#[test]
fn records_binding_domain_from_sequence_surface_evidence() {
    let interner = Interner::new();
    let xs = interner.intern("xs");
    let mut b = IlBuilder::new(FileId(0));
    let (assign, _) = array_assignment(&mut b, &interner, xs, sp(1), sp(2));
    let root = b.add(NodeKind::Func, Payload::None, sp(1), &[assign]);
    let mut il = finish_with_sequence_evidence(b, root);

    run(&mut il, &interner);

    let record = binding_domain_record(&il, "xs").expect("binding domain evidence");
    assert!(matches!(
        record.kind,
        EvidenceKind::Domain(DomainEvidence::Collection)
    ));
    assert_eq!(record.dependencies, vec![EvidenceId(0)]);
    assert_eq!(
        record.provenance,
        language_core_provenance(Lang::TypeScript)
    );
}

#[test]
fn binding_domain_provenance_follows_file_language() {
    let interner = Interner::new();
    for lang in [Lang::Python, Lang::Java, Lang::Rust] {
        let xs = interner.intern("xs");
        let mut b = IlBuilder::new(FileId(0));
        let (assign, _) = array_assignment(&mut b, &interner, xs, sp(1), sp(2));
        let root = b.add(NodeKind::Func, Payload::None, sp(1), &[assign]);
        let mut il = finish_with_sequence_evidence_for_lang(b, root, lang);

        run(&mut il, &interner);

        let record = binding_domain_record(&il, "xs").expect("binding domain evidence");
        assert_eq!(record.provenance, language_core_provenance(lang));
    }
}

#[test]
fn binding_domain_chains_through_prior_immutable_binding() {
    let interner = Interner::new();
    let xs = interner.intern("xs");
    let ys = interner.intern("ys");
    let mut b = IlBuilder::new(FileId(0));
    let (xs_assign, _) = array_assignment(&mut b, &interner, xs, sp(1), sp(2));
    let ys_lhs = b.add(NodeKind::Var, Payload::Name(ys), sp(3), &[]);
    let xs_ref = b.add(NodeKind::Var, Payload::Name(xs), sp(4), &[]);
    let ys_assign = b.add(NodeKind::Assign, Payload::None, sp(3), &[ys_lhs, xs_ref]);
    let root = b.add(
        NodeKind::Func,
        Payload::None,
        sp(1),
        &[xs_assign, ys_assign],
    );
    let mut il = finish_with_sequence_evidence(b, root);

    run(&mut il, &interner);

    let xs_record = binding_domain_record(&il, "xs").expect("xs binding evidence");
    let ys_record = binding_domain_record(&il, "ys").expect("ys binding evidence");
    assert!(matches!(
        ys_record.kind,
        EvidenceKind::Domain(DomainEvidence::Collection)
    ));
    assert_eq!(ys_record.dependencies, vec![xs_record.id]);
}

#[test]
fn mutations_block_binding_domain_evidence() {
    let interner = Interner::new();
    for case in [
        MutationCase::Direct,
        MutationCase::NestedModule,
        MutationCase::NestedLocal,
    ] {
        let mut il = mutation_case_il(&interner, case);
        run(&mut il, &interner);
        assert!(binding_domain_record(&il, "xs").is_none());
    }
}
