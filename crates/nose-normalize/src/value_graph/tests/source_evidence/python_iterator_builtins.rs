use super::super::support::*;

#[test]
fn python_iterator_materializer_requires_factory_and_lazy_source_proof() {
    let (map_only, interner) = python_list_map_call_il(false);
    let map_only = crate::desugar::run(&map_only, &interner, &crate::NormalizeOptions::default());
    let list = map_only.children(map_only.root)[0];
    let map = map_only.children(list)[1];
    assert!(
        matches!(map_only.node(map).payload, Payload::HoF(HoFKind::Map)),
        "admitted Python map(lambda, xs) should desugar to a normalized HOF"
    );
    assert!(
        admitted_hof_demand_effect_profile_at_node(&map_only, map, HoFKind::Map)
            .is_some_and(|profile| profile.callback_effects_delayed_until_pull()),
        "Python map is pull-lazy until a terminal materializer consumes it"
    );
    let mut builder = Builder::new(&map_only, &interner);
    let value = builder.eval(list, &FxHashMap::default());
    assert!(
        !matches!(builder.nodes[value as usize].op, ValOp::Hof(k) if k == HoFKind::Map as u32),
        "list(map(...)) must not materialize without an admitted list/tuple/set factory proof"
    );

    let (with_list, interner) = python_list_map_call_il(true);
    let with_list = crate::desugar::run(&with_list, &interner, &crate::NormalizeOptions::default());
    let list = with_list.children(with_list.root)[0];
    let mut builder = Builder::new(&with_list, &interner);
    let value = builder.eval(list, &FxHashMap::default());
    assert!(
        matches!(builder.nodes[value as usize].op, ValOp::Hof(k) if k == HoFKind::Map as u32),
        "an admitted Python list(map(lambda, xs)) can consume the lazy iterator source"
    );
}

#[test]
fn python_iterator_materializer_accepts_zip_only_with_producer_and_factory_proof() {
    let (missing_factory, interner) = python_list_zip_call_il(false, true);
    let missing_factory = crate::desugar::run(
        &missing_factory,
        &interner,
        &crate::NormalizeOptions::default(),
    );
    let list = missing_factory.children(missing_factory.root)[0];
    let zip = missing_factory.children(list)[1];
    assert!(
        matches!(
            missing_factory.node(zip).payload,
            Payload::Builtin(Builtin::Zip)
        ),
        "admitted Python zip(left, right) should desugar to a canonical builtin producer"
    );
    let mut builder = Builder::new(&missing_factory, &interner);
    let value = builder.eval(list, &FxHashMap::default());
    assert!(
        !matches!(builder.nodes[value as usize].op, ValOp::Call(tag) if tag == builtin_tag(Builtin::Zip)),
        "list(zip(...)) must stay closed without terminal collection-factory proof"
    );

    let (missing_producer, interner) = python_list_zip_call_il(true, false);
    let missing_producer = crate::desugar::run(
        &missing_producer,
        &interner,
        &crate::NormalizeOptions::default(),
    );
    let list = missing_producer.children(missing_producer.root)[0];
    let mut builder = Builder::new(&missing_producer, &interner);
    let value = builder.eval(list, &FxHashMap::default());
    assert!(
        !matches!(builder.nodes[value as usize].op, ValOp::Call(tag) if tag == builtin_tag(Builtin::Zip)),
        "list(zip(...)) must stay closed without iterator producer proof"
    );

    let (with_both, interner) = python_list_zip_call_il(true, true);
    let with_both = crate::desugar::run(&with_both, &interner, &crate::NormalizeOptions::default());
    let list = with_both.children(with_both.root)[0];
    let mut builder = Builder::new(&with_both, &interner);
    let value = builder.eval(list, &FxHashMap::default());
    assert!(
        matches!(builder.nodes[value as usize].op, ValOp::Call(tag) if tag == builtin_tag(Builtin::Zip)),
        "an admitted Python list(zip(left, right)) can consume the lazy iterator source"
    );
}

#[test]
fn normalized_python_free_function_hof_keeps_symbol_dependency() {
    let interner = Interner::new();
    let mut b = IlBuilder::new(FileId(0));
    let source = b.add(
        NodeKind::Var,
        Payload::Name(interner.intern("xs")),
        sp(70),
        &[],
    );
    let lambda = identity_lambda(&mut b, 71, sp(71));
    let hof = b.add(
        NodeKind::HoF,
        Payload::HoF(HoFKind::Map),
        sp(72),
        &[source, lambda],
    );
    let mut il = finish_test_il(b, hof, Lang::Python);
    il.evidence.push(language_core_evidence(
        0,
        Lang::Python,
        EvidenceAnchor::node(sp(70), NodeKind::Var),
        EvidenceKind::Domain(DomainEvidence::Collection),
    ));
    let contract = library_free_function_hof_contract(Lang::Python, "map", 2).unwrap();
    il.evidence.push(python_iterator_builtin_protocol_evidence(
        1,
        sp(72),
        contract,
        2,
        vec![EvidenceId(0)],
    ));
    assert_eq!(
        admitted_hof_demand_effect_profile_at_node(&il, hof, HoFKind::Map),
        None,
        "normalized free-function HOFs must retain the original unshadowed builtin proof"
    );

    il.evidence.push(language_core_symbol_evidence(
        2,
        Lang::Python,
        EvidenceAnchor::node(sp(72), NodeKind::Var),
        SymbolEvidenceKind::UnshadowedGlobal {
            name_hash: stable_symbol_hash("map"),
        },
    ));
    il.evidence.push(python_iterator_builtin_protocol_evidence(
        3,
        sp(72),
        contract,
        2,
        vec![EvidenceId(2), EvidenceId(0)],
    ));
    assert!(
        admitted_hof_demand_effect_profile_at_node(&il, hof, HoFKind::Map)
            .is_some_and(|profile| profile.callback_effects_delayed_until_pull()),
        "with both symbol and source proof, normalized Python map regains lazy HOF semantics"
    );
}

fn python_list_map_call_il(include_list_factory_evidence: bool) -> (Il, Interner) {
    let interner = Interner::new();
    let mut b = IlBuilder::new(FileId(0));
    let list_callee = b.add(
        NodeKind::Var,
        Payload::Name(interner.intern("list")),
        sp(60),
        &[],
    );
    let map_callee = b.add(
        NodeKind::Var,
        Payload::Name(interner.intern("map")),
        sp(61),
        &[],
    );
    let lambda = identity_lambda(&mut b, 62, sp(62));
    let source = b.add(
        NodeKind::Var,
        Payload::Name(interner.intern("xs")),
        sp(63),
        &[],
    );
    let map = b.add(
        NodeKind::Call,
        Payload::None,
        sp(61),
        &[map_callee, lambda, source],
    );
    let list = b.add(NodeKind::Call, Payload::None, sp(60), &[list_callee, map]);
    let root = b.add(NodeKind::Block, Payload::None, sp(59), &[list]);
    let mut il = finish_test_il(b, root, Lang::Python);

    il.evidence.push(language_core_symbol_evidence(
        0,
        Lang::Python,
        EvidenceAnchor::node(sp(61), NodeKind::Var),
        SymbolEvidenceKind::UnshadowedGlobal {
            name_hash: stable_symbol_hash("map"),
        },
    ));
    il.evidence.push(language_core_evidence(
        1,
        Lang::Python,
        EvidenceAnchor::node(sp(63), NodeKind::Var),
        EvidenceKind::Domain(DomainEvidence::Collection),
    ));
    let map_contract = library_free_function_hof_contract(Lang::Python, "map", 2).unwrap();
    il.evidence.push(python_iterator_builtin_protocol_evidence(
        2,
        sp(61),
        map_contract,
        2,
        vec![EvidenceId(0), EvidenceId(1)],
    ));

    il.evidence.push(language_core_symbol_evidence(
        3,
        Lang::Python,
        EvidenceAnchor::node(sp(60), NodeKind::Var),
        SymbolEvidenceKind::UnshadowedGlobal {
            name_hash: stable_symbol_hash("list"),
        },
    ));
    if include_list_factory_evidence {
        let list_contract =
            library_free_name_collection_factory_contract(Lang::Python, "list").unwrap();
        il.evidence.push(python_builtin_collection_factory_evidence(
            4,
            sp(60),
            list_contract,
            1,
            vec![EvidenceId(3)],
        ));
    }

    (il, interner)
}

fn python_list_zip_call_il(
    include_list_factory_evidence: bool,
    include_zip_producer_evidence: bool,
) -> (Il, Interner) {
    let interner = Interner::new();
    let mut b = IlBuilder::new(FileId(0));
    let list_callee = b.add(
        NodeKind::Var,
        Payload::Name(interner.intern("list")),
        sp(80),
        &[],
    );
    let zip_callee = b.add(
        NodeKind::Var,
        Payload::Name(interner.intern("zip")),
        sp(81),
        &[],
    );
    let left = b.add(
        NodeKind::Var,
        Payload::Name(interner.intern("left")),
        sp(82),
        &[],
    );
    let right = b.add(
        NodeKind::Var,
        Payload::Name(interner.intern("right")),
        sp(83),
        &[],
    );
    let zip = b.add(
        NodeKind::Call,
        Payload::None,
        sp(81),
        &[zip_callee, left, right],
    );
    let list = b.add(NodeKind::Call, Payload::None, sp(80), &[list_callee, zip]);
    let root = b.add(NodeKind::Block, Payload::None, sp(79), &[list]);
    let mut il = finish_test_il(b, root, Lang::Python);

    il.evidence.push(language_core_symbol_evidence(
        0,
        Lang::Python,
        EvidenceAnchor::node(sp(81), NodeKind::Var),
        SymbolEvidenceKind::UnshadowedGlobal {
            name_hash: stable_symbol_hash("zip"),
        },
    ));
    il.evidence.push(language_core_evidence(
        1,
        Lang::Python,
        EvidenceAnchor::node(sp(82), NodeKind::Var),
        EvidenceKind::Domain(DomainEvidence::Collection),
    ));
    il.evidence.push(language_core_evidence(
        2,
        Lang::Python,
        EvidenceAnchor::node(sp(83), NodeKind::Var),
        EvidenceKind::Domain(DomainEvidence::Collection),
    ));
    if include_zip_producer_evidence {
        let zip_contract = library_free_function_builtin_contract(Lang::Python, "zip", 2).unwrap();
        il.evidence
            .push(python_iterator_builtin_protocol_builtin_evidence(
                3,
                sp(81),
                zip_contract,
                2,
                vec![EvidenceId(0), EvidenceId(1), EvidenceId(2)],
            ));
    }

    il.evidence.push(language_core_symbol_evidence(
        4,
        Lang::Python,
        EvidenceAnchor::node(sp(80), NodeKind::Var),
        SymbolEvidenceKind::UnshadowedGlobal {
            name_hash: stable_symbol_hash("list"),
        },
    ));
    if include_list_factory_evidence {
        let list_contract =
            library_free_name_collection_factory_contract(Lang::Python, "list").unwrap();
        il.evidence.push(python_builtin_collection_factory_evidence(
            5,
            sp(80),
            list_contract,
            1,
            vec![EvidenceId(4)],
        ));
    }

    (il, interner)
}
