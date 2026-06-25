use super::receiver_method_result_domains::receiver_method_result_domain_il;
use super::*;
use nose_semantics::{
    library_free_name_collection_factory_contract, library_free_name_map_factory_contract,
    library_map_get_contract, library_method_call_contract, library_swift_map_factory_contract,
    PYTHON_BUILTIN_COLLECTION_FACTORY_PRODUCER_ID, RUST_STDLIB_MAP_FACTORY_PRODUCER_ID,
    SWIFT_STDLIB_COLLECTION_FACTORY_PRODUCER_ID,
};

fn free_name_call_il(lang: Lang, name: &str, arg_count: usize) -> (Il, Interner, NodeId, NodeId) {
    let interner = Interner::new();
    let mut builder = IlBuilder::new(FileId(0));
    let callee = builder.add(
        NodeKind::Var,
        Payload::Name(interner.intern(name)),
        sp(10),
        &[],
    );
    let mut children = vec![callee];
    for idx in 0..arg_count {
        children.push(builder.add(
            NodeKind::Var,
            Payload::Cid((idx + 1) as u32),
            sp(11 + idx as u32),
            &[],
        ));
    }
    let call = builder.add(NodeKind::Call, Payload::None, sp(20), &children);
    let root = builder.add(NodeKind::Func, Payload::None, sp(21), &[call]);
    (
        builder.finish(
            root,
            FileMeta {
                path: "call-result-domain".into(),
                lang,
            },
            Vec::new(),
            Vec::new(),
        ),
        interner,
        call,
        callee,
    )
}

fn push_unshadowed_symbol(il: &mut Il, node: NodeId, name: &str) -> EvidenceId {
    let (pack_id, producer_id) = language_core_evidence_provenance(il.meta.lang);
    il.find_or_push_builtin_evidence(
        EvidenceAnchor::node(il.node(node).span, NodeKind::Var),
        EvidenceKind::Symbol(SymbolEvidenceKind::UnshadowedGlobal {
            name_hash: stable_symbol_hash(name),
        }),
        pack_id,
        producer_id,
        Vec::new(),
    )
}

fn call_domain_records(il: &Il, call: NodeId) -> Vec<&EvidenceRecord> {
    let anchor = EvidenceAnchor::node(il.node(call).span, NodeKind::Call);
    il.evidence_anchored_at(anchor.span())
        .filter(|record| record.anchor == anchor && matches!(record.kind, EvidenceKind::Domain(_)))
        .collect()
}

#[test]
fn preexisting_collection_api_records_materialize_result_domains() {
    for (name, expected_domain) in [
        ("list", DomainEvidence::Collection),
        ("set", DomainEvidence::Set),
    ] {
        let (mut il, interner, call, callee) = free_name_call_il(Lang::Python, name, 1);
        let contract = library_free_name_collection_factory_contract(Lang::Python, name).unwrap();
        let symbol = push_unshadowed_symbol(&mut il, callee, name);
        let anchor = EvidenceAnchor::node(il.node(call).span, NodeKind::Call);
        let api = upsert_builtin_evidence_with_pack_id(
            &mut il,
            anchor,
            EvidenceKind::LibraryApi(LibraryApiEvidenceKind::Contract {
                contract_hash: library_api_contract_id_hash(contract.id),
                callee_hash: library_api_callee_contract_hash(contract.callee),
                arity: 1,
            }),
            contract.pack_id,
            PYTHON_BUILTIN_COLLECTION_FACTORY_PRODUCER_ID,
            vec![symbol],
        );

        run(&mut il, &interner);

        let result_domains = node_domain_records(&il, call, expected_domain);
        assert_eq!(result_domains.len(), 1, "{name} should materialize domain");
        assert_eq!(
            result_domains[0].provenance,
            language_core_provenance(Lang::Python)
        );
        assert_eq!(result_domains[0].dependencies, vec![api]);
    }
}

#[test]
fn preexisting_map_api_records_materialize_result_domains() {
    let name = "std::collections::HashMap::from";
    let (mut il, interner, call, callee) = free_name_call_il(Lang::Rust, name, 1);
    let contract = library_free_name_map_factory_contract(Lang::Rust, name).unwrap();
    let symbol = push_unshadowed_symbol(&mut il, callee, name);
    let anchor = EvidenceAnchor::node(il.node(call).span, NodeKind::Call);
    let api = upsert_builtin_evidence_with_pack_id(
        &mut il,
        anchor,
        EvidenceKind::LibraryApi(LibraryApiEvidenceKind::Contract {
            contract_hash: library_api_contract_id_hash(contract.id),
            callee_hash: library_api_callee_contract_hash(contract.callee),
            arity: 1,
        }),
        contract.pack_id,
        RUST_STDLIB_MAP_FACTORY_PRODUCER_ID,
        vec![symbol],
    );

    run(&mut il, &interner);

    let result_domains = node_domain_records(&il, call, DomainEvidence::Map);
    assert_eq!(result_domains.len(), 1);
    assert_eq!(
        result_domains[0].provenance,
        language_core_provenance(Lang::Rust)
    );
    assert_eq!(result_domains[0].dependencies, vec![api]);
}

#[test]
fn preexisting_swift_map_api_records_do_not_materialize_arity_only_domains() {
    let interner = Interner::new();
    let mut builder = IlBuilder::new(FileId(0));
    let callee = builder.add(
        NodeKind::Var,
        Payload::Name(interner.intern("Dictionary")),
        sp(10),
        &[],
    );
    let values = builder.add(
        NodeKind::Var,
        Payload::Name(interner.intern("values")),
        sp(11),
        &[],
    );
    let kwarg = builder.add(
        NodeKind::KwArg,
        Payload::Name(interner.intern("uniqueKeysWithValues")),
        sp(12),
        &[values],
    );
    let call = builder.add(NodeKind::Call, Payload::None, sp(20), &[callee, kwarg]);
    let root = builder.add(NodeKind::Func, Payload::None, sp(21), &[call]);
    let mut il = builder.finish(
        root,
        FileMeta {
            path: "swift-call-result-domain".into(),
            lang: Lang::Swift,
        },
        Vec::new(),
        Vec::new(),
    );
    let contract =
        library_swift_map_factory_contract(Lang::Swift, "Dictionary", "uniqueKeysWithValues")
            .unwrap();
    let symbol = push_unshadowed_symbol(&mut il, callee, "Dictionary");
    let anchor = EvidenceAnchor::node(il.node(call).span, NodeKind::Call);
    upsert_builtin_evidence_with_pack_id(
        &mut il,
        anchor,
        EvidenceKind::LibraryApi(LibraryApiEvidenceKind::Contract {
            contract_hash: library_api_contract_id_hash(contract.id),
            callee_hash: library_api_callee_contract_hash(contract.callee),
            arity: 1,
        }),
        contract.pack_id,
        SWIFT_STDLIB_COLLECTION_FACTORY_PRODUCER_ID,
        vec![symbol],
    );

    run(&mut il, &interner);

    assert!(
        node_domain_records(&il, call, DomainEvidence::Map).is_empty(),
        "Swift Dictionary result domains require call-shape proof beyond arity"
    );
}

#[test]
fn result_domain_materialization_excludes_hof_and_map_get_lanes() {
    let (mut hof_il, hof_interner, hof_call, _) = receiver_method_result_domain_il(
        Lang::JavaScript,
        "receiver",
        "map",
        1,
        Some(DomainEvidence::Collection),
    );

    run(&mut hof_il, &hof_interner);

    assert!(
        asserted(library_api_records(&hof_il, hof_call)).len() == 1,
        "HOF API occurrence should still be admitted for downstream protocol consumers"
    );
    assert!(
        call_domain_records(&hof_il, hof_call).is_empty(),
        "HOF compatibility fallback must not materialize call result DomainEvidence"
    );

    let (mut get_il, get_interner, get_call, _) = receiver_method_result_domain_il(
        Lang::Rust,
        "receiver",
        "get",
        1,
        Some(DomainEvidence::Map),
    );

    run(&mut get_il, &get_interner);

    assert!(
        asserted(library_api_records(&get_il, get_call)).len() == 1,
        "Map.get API occurrence should still be admitted"
    );
    assert!(
        call_domain_records(&get_il, get_call).is_empty(),
        "Map.get value semantics must not materialize a map/option result domain"
    );
}

#[test]
fn result_domain_materialization_requires_admitted_api_evidence() {
    let (mut il, interner, call, callee) = free_name_call_il(Lang::Python, "list", 1);
    let contract = library_free_name_collection_factory_contract(Lang::Python, "list").unwrap();
    let symbol = push_unshadowed_symbol(&mut il, callee, "list");
    let anchor = EvidenceAnchor::node(il.node(call).span, NodeKind::Call);
    upsert_builtin_evidence_with_pack_id(
        &mut il,
        anchor,
        EvidenceKind::LibraryApi(LibraryApiEvidenceKind::Contract {
            contract_hash: library_api_contract_id_hash(contract.id),
            callee_hash: library_api_callee_contract_hash(contract.callee),
            arity: 2,
        }),
        contract.pack_id,
        PYTHON_BUILTIN_COLLECTION_FACTORY_PRODUCER_ID,
        vec![symbol],
    );

    run(&mut il, &interner);

    assert!(
        node_domain_records(&il, call, DomainEvidence::Collection).is_empty(),
        "wrong-arity LibraryApi evidence must not materialize result-domain evidence"
    );
}

#[test]
fn hard_negative_contracts_have_no_materialized_result_domain_mapping() {
    let hof = library_method_call_contract(Lang::JavaScript, "map", 1).unwrap();
    assert_eq!(
        nose_semantics::library_api_materialized_result_domain_for_arity(hof.id, hof.callee, 1),
        None
    );
    let get = library_map_get_contract(Lang::Rust, "get", 1).unwrap();
    assert_eq!(
        nose_semantics::library_api_materialized_result_domain_for_arity(get.id, get.callee, 1),
        None
    );
}
