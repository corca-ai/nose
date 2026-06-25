use super::*;

#[test]
fn swift_collection_factory_value_graph_uses_library_api_evidence() {
    let interner = Interner::new();
    let (mut il, call) = swift_collection_factory_il(&interner, "Array");
    il.evidence.push(language_core_symbol_evidence(
        1,
        Lang::Swift,
        EvidenceAnchor::node(sp(76), NodeKind::Var),
        SymbolEvidenceKind::UnshadowedGlobal {
            name_hash: stable_symbol_hash("Array"),
        },
    ));
    assert!(
        !matches!(
            eval_op(&il, &interner, call),
            ValOp::Seq(SEQ_VALUE_COLLECTION)
        ),
        "Swift global symbol proof alone must not canonicalize Array(sequence)"
    );

    let contract = library_free_name_collection_factory_contract(Lang::Swift, "Array")
        .expect("Swift Array contract");
    il.evidence.push(swift_stdlib_collection_factory_evidence(
        2,
        sp(79),
        contract,
        1,
        vec![EvidenceId(1)],
    ));
    assert!(
        !matches!(eval_op(&il, &interner, call), ValOp::Seq(SEQ_VALUE_COLLECTION)),
        "Array(sequence) preserves order and multiplicity, so it must not use membership canonicalization"
    );

    let (mut set_il, set_call) = swift_collection_factory_il(&interner, "Set");
    set_il.evidence.push(language_core_symbol_evidence(
        1,
        Lang::Swift,
        EvidenceAnchor::node(sp(76), NodeKind::Var),
        SymbolEvidenceKind::UnshadowedGlobal {
            name_hash: stable_symbol_hash("Set"),
        },
    ));
    let set_contract = library_free_name_collection_factory_contract(Lang::Swift, "Set")
        .expect("Swift Set contract");
    set_il
        .evidence
        .push(swift_stdlib_collection_factory_evidence(
            2,
            sp(79),
            set_contract,
            1,
            vec![EvidenceId(1)],
        ));
    assert!(
        matches!(
            eval_op(&set_il, &interner, set_call),
            ValOp::Seq(SEQ_VALUE_COLLECTION)
        ),
        "Set(sequence) may use membership canonicalization"
    );
}

fn swift_collection_factory_il(interner: &Interner, factory: &str) -> (Il, NodeId) {
    let mut b = IlBuilder::new(FileId(0));
    let callee = b.add(
        NodeKind::Var,
        Payload::Name(interner.intern(factory)),
        sp(76),
        &[],
    );
    let item = b.add(NodeKind::Lit, Payload::LitInt(1), sp(77), &[]);
    let seq = b.add(
        NodeKind::Seq,
        Payload::Name(interner.intern("array")),
        sp(78),
        &[item],
    );
    let call = b.add(NodeKind::Call, Payload::None, sp(79), &[callee, seq]);
    let root = b.add(NodeKind::Block, Payload::None, sp(75), &[call]);
    let mut il = finish_test_il(b, root, Lang::Swift);
    il.evidence
        .push(collection_sequence_evidence(0, Lang::Swift, sp(78)));
    (il, call)
}

#[test]
fn swift_dictionary_factory_value_graph_requires_tuple_entries_without_static_duplicates() {
    let interner = Interner::new();
    let (mut il, call) = swift_dictionary_unique_keys_il(&interner, false);
    assert!(
        !matches!(eval_op(&il, &interner, call), ValOp::Seq(SEQ_VALUE_MAP)),
        "Swift Dictionary global symbol proof alone must not canonicalize the map factory"
    );
    let contract =
        library_swift_map_factory_contract(Lang::Swift, "Dictionary", "uniqueKeysWithValues")
            .expect("Swift Dictionary contract");
    il.evidence.push(swift_stdlib_map_factory_evidence(
        5,
        sp(88),
        contract,
        1,
        vec![EvidenceId(4)],
    ));
    assert!(
        matches!(eval_op(&il, &interner, call), ValOp::Seq(SEQ_VALUE_MAP)),
        "admitted Swift Dictionary(uniqueKeysWithValues:) evidence should canonicalize tuple entries"
    );

    let (mut duplicate, duplicate_call) = swift_dictionary_unique_keys_il(&interner, true);
    duplicate.evidence.push(swift_stdlib_map_factory_evidence(
        5,
        sp(88),
        contract,
        1,
        vec![EvidenceId(4)],
    ));
    assert!(
        !matches!(
            eval_op(&duplicate, &interner, duplicate_call),
            ValOp::Seq(SEQ_VALUE_MAP)
        ),
        "static duplicate keys are outside Dictionary(uniqueKeysWithValues:) map canonicalization"
    );
}

fn swift_dictionary_unique_keys_il(interner: &Interner, duplicate: bool) -> (Il, NodeId) {
    let mut b = IlBuilder::new(FileId(0));
    let callee = b.add(
        NodeKind::Var,
        Payload::Name(interner.intern("Dictionary")),
        sp(80),
        &[],
    );
    let first_key = b.add(
        NodeKind::Lit,
        Payload::LitStr(stable_symbol_hash("red")),
        sp(81),
        &[],
    );
    let first_value = b.add(NodeKind::Lit, Payload::LitInt(1), sp(82), &[]);
    let first_entry = b.add(
        NodeKind::Seq,
        Payload::Name(interner.intern("tuple")),
        sp(83),
        &[first_key, first_value],
    );
    let second_key = b.add(
        NodeKind::Lit,
        Payload::LitStr(stable_symbol_hash(if duplicate { "red" } else { "blue" })),
        sp(84),
        &[],
    );
    let second_value = b.add(NodeKind::Lit, Payload::LitInt(2), sp(85), &[]);
    let second_entry = b.add(
        NodeKind::Seq,
        Payload::Name(interner.intern("tuple")),
        sp(86),
        &[second_key, second_value],
    );
    let entries = b.add(
        NodeKind::Seq,
        Payload::Name(interner.intern("array")),
        sp(87),
        &[first_entry, second_entry],
    );
    let label = b.add(
        NodeKind::KwArg,
        Payload::Name(interner.intern("uniqueKeysWithValues")),
        sp(88),
        &[entries],
    );
    let call = b.add(NodeKind::Call, Payload::None, sp(88), &[callee, label]);
    let root = b.add(NodeKind::Block, Payload::None, sp(80), &[call]);
    let mut il = finish_test_il(b, root, Lang::Swift);
    il.evidence.push(language_core_evidence(
        0,
        Lang::Swift,
        EvidenceAnchor::sequence(sp(83)),
        EvidenceKind::SequenceSurface(SequenceSurfaceKind::Tuple),
    ));
    il.evidence.push(language_core_evidence(
        1,
        Lang::Swift,
        EvidenceAnchor::sequence(sp(86)),
        EvidenceKind::SequenceSurface(SequenceSurfaceKind::Tuple),
    ));
    il.evidence
        .push(collection_sequence_evidence(2, Lang::Swift, sp(87)));
    il.evidence.push(language_core_symbol_evidence(
        4,
        Lang::Swift,
        EvidenceAnchor::node(sp(80), NodeKind::Var),
        SymbolEvidenceKind::UnshadowedGlobal {
            name_hash: stable_symbol_hash("Dictionary"),
        },
    ));
    (il, call)
}
