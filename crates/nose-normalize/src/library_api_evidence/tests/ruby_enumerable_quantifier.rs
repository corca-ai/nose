use super::*;
use nose_il::{Symbol, Unit, UnitKind};
use nose_semantics::{
    library_api_callee_contract_hash, library_api_contract_id_hash, library_method_call_contract,
};

#[test]
fn ruby_enumerable_quantifier_api_evidence_uses_sequence_hof_pack() {
    let mut interner = Interner::new();
    let (mut il, call, receiver, _) = method_call_il(&mut interner, Lang::Ruby, "any?", 1);
    let (pack_id, producer_id) = language_core_evidence_provenance(Lang::Ruby);
    let receiver_domain = il.find_or_push_first_party_evidence(
        EvidenceAnchor::node(il.node(receiver).span, il.kind(receiver)),
        EvidenceKind::Domain(DomainEvidence::Collection),
        pack_id,
        producer_id,
        Vec::new(),
    );

    run(&mut il, &interner);

    let receiver_domains = node_domain_records(&il, receiver, DomainEvidence::Collection);
    let asserted_domains = asserted(receiver_domains);
    assert_eq!(asserted_domains.len(), 1);
    assert_eq!(
        asserted_domains[0].provenance,
        language_core_provenance(Lang::Ruby)
    );
    assert_eq!(asserted_domains[0].id, receiver_domain);

    let api = library_api_records(&il, call)
        .into_iter()
        .find(|record| record.status == EvidenceStatus::Asserted)
        .expect("Ruby any? API evidence");
    assert_eq!(
        api.provenance,
        pack_provenance(
            SEQUENCE_HOF_ADAPTER_PROTOCOL_PACK_ID,
            SEQUENCE_HOF_ADAPTER_PROTOCOL_PRODUCER_ID
        )
    );
    assert_eq!(api.dependencies, vec![asserted_domains[0].id]);
}

#[test]
fn ruby_enumerable_quantifier_api_evidence_closes_on_same_file_redefinition() {
    assert_ruby_any_api_evidence_closes_on_patch(
        "ruby_any_patch.rb",
        "same-file Ruby Array#any? redefinitions must close sequence-HOF admission",
        |builder, interner, any| {
            let array = interner.intern("Array");
            let patched_method = ruby_false_method(builder);
            let patched_class = builder.add(
                NodeKind::Block,
                Payload::None,
                patch_span(),
                &[patched_method],
            );
            (
                patched_class,
                vec![
                    Unit {
                        root: patched_method,
                        kind: UnitKind::Method,
                        name: Some(any),
                        origin: Default::default(),
                    },
                    Unit {
                        root: patched_class,
                        kind: UnitKind::Class,
                        name: Some(array),
                        origin: Default::default(),
                    },
                ],
            )
        },
    );
}

#[test]
fn ruby_enumerable_quantifier_api_evidence_closes_on_same_file_module_eval_redefinition() {
    assert_ruby_any_api_evidence_closes_on_patch(
        "ruby_any_module_eval_patch.rb",
        "same-file Ruby Enumerable.module_eval redefinitions must close sequence-HOF admission",
        |builder, interner, any| {
            let patched_method = ruby_false_method(builder);
            let enumerable = builder.add(
                NodeKind::Var,
                Payload::Name(interner.intern("Enumerable")),
                sp(10),
                &[],
            );
            let module_eval = builder.add(
                NodeKind::Field,
                Payload::Name(interner.intern("module_eval")),
                patch_span(),
                &[enumerable],
            );
            let patch_call = builder.add(
                NodeKind::Call,
                Payload::None,
                patch_span(),
                &[module_eval, patched_method],
            );
            (
                patch_call,
                vec![Unit {
                    root: patched_method,
                    kind: UnitKind::Method,
                    name: Some(any),
                    origin: Default::default(),
                }],
            )
        },
    );
}

fn assert_ruby_any_api_evidence_closes_on_patch(
    path: &'static str,
    failure_message: &'static str,
    build_patch: impl FnOnce(&mut IlBuilder, &Interner, Symbol) -> (NodeId, Vec<Unit>),
) {
    let interner = Interner::new();
    let any = interner.intern("any?");
    let mut builder = IlBuilder::new(FileId(0));
    let (patch, units) = build_patch(&mut builder, &interner, any);

    let receiver_span = sp(40);
    let receiver = builder.add(
        NodeKind::Seq,
        Payload::Name(interner.intern("array")),
        receiver_span,
        &[],
    );
    let field = builder.add(NodeKind::Field, Payload::Name(any), sp(41), &[receiver]);
    let predicate = builder.add(NodeKind::Var, Payload::Cid(1), sp(42), &[]);
    let call = builder.add(NodeKind::Call, Payload::None, sp(43), &[field, predicate]);
    let root = builder.add(NodeKind::Func, Payload::None, sp(44), &[call]);
    let module = builder.add(NodeKind::Module, Payload::None, sp(45), &[patch, root]);
    let mut il = builder.finish(
        module,
        FileMeta {
            path: path.into(),
            lang: Lang::Ruby,
        },
        units,
        Vec::new(),
    );
    let (pack_id, producer_id) = language_core_evidence_provenance(Lang::Ruby);
    il.find_or_push_first_party_evidence(
        EvidenceAnchor::sequence(receiver_span),
        EvidenceKind::SequenceSurface(SequenceSurfaceKind::Collection),
        pack_id,
        producer_id,
        Vec::new(),
    );
    let contract =
        library_method_call_contract(Lang::Ruby, "any?", 1).expect("Ruby any? method contract");

    run(&mut il, &interner);

    let api_records: Vec<_> = library_api_records(&il, call)
        .into_iter()
        .filter(|record| {
            record.status == EvidenceStatus::Asserted
                && matches!(
                    record.kind,
                    EvidenceKind::LibraryApi(LibraryApiEvidenceKind::Contract {
                        contract_hash,
                        callee_hash,
                        ..
                    }) if contract_hash == library_api_contract_id_hash(contract.id)
                        && callee_hash == library_api_callee_contract_hash(contract.callee)
                )
        })
        .collect();
    assert!(api_records.is_empty(), "{failure_message}");
}

fn ruby_false_method(builder: &mut IlBuilder) -> NodeId {
    let false_lit = builder.add(NodeKind::Lit, Payload::LitBool(false), sp(20), &[]);
    let method_body = builder.add(NodeKind::Block, Payload::None, sp(19), &[false_lit]);
    builder.add(NodeKind::Func, Payload::None, sp(18), &[method_body])
}

fn patch_span() -> Span {
    Span::new(FileId(0), 10, 30, 1, 5)
}
