use super::*;
use crate::strict_exact::{
    strict_exact_swift_collection_factory_safe, strict_exact_swift_map_factory_safe,
};
use nose_semantics::library_swift_map_factory_contract;

#[test]
fn strict_exact_swift_factories_use_library_api_evidence_and_shape_boundaries() {
    let interner = Interner::new();
    let (mut array_il, array_call) = swift_collection_factory_il(&interner, "Array");
    let facts = StrictFacts::collect(&array_il, &interner);
    assert!(!strict_exact_swift_collection_factory_safe(
        &array_il, &interner, &facts, array_call
    ));
    let array_contract = library_free_name_collection_factory_contract(Lang::Swift, "Array")
        .expect("Swift Array contract");
    array_il
        .evidence
        .push(swift_stdlib_collection_factory_evidence(
            3,
            sp(53),
            array_contract,
            1,
            vec![EvidenceId(1)],
        ));
    let facts = StrictFacts::collect(&array_il, &interner);
    assert!(!strict_exact_swift_collection_factory_safe(
        &array_il, &interner, &facts, array_call
    ));

    let (mut set_il, set_call) = swift_collection_factory_il(&interner, "Set");
    let set_contract = library_free_name_collection_factory_contract(Lang::Swift, "Set")
        .expect("Swift Set contract");
    set_il
        .evidence
        .push(swift_stdlib_collection_factory_evidence(
            3,
            sp(53),
            set_contract,
            1,
            vec![EvidenceId(1)],
        ));
    let facts = StrictFacts::collect(&set_il, &interner);
    assert!(strict_exact_swift_collection_factory_safe(
        &set_il, &interner, &facts, set_call
    ));

    let (mut map_il, map_call) = swift_dictionary_unique_keys_il(&interner, false);
    let facts = StrictFacts::collect(&map_il, &interner);
    assert!(!strict_exact_swift_map_factory_safe(
        &map_il, &interner, &facts, map_call
    ));
    let map_contract =
        library_swift_map_factory_contract(Lang::Swift, "Dictionary", "uniqueKeysWithValues")
            .expect("Swift Dictionary contract");
    map_il.evidence.push(swift_stdlib_map_factory_evidence(
        5,
        sp(63),
        map_contract,
        1,
        vec![EvidenceId(4)],
    ));
    let facts = StrictFacts::collect(&map_il, &interner);
    assert!(strict_exact_swift_map_factory_safe(
        &map_il, &interner, &facts, map_call
    ));

    let (mut duplicate, duplicate_call) = swift_dictionary_unique_keys_il(&interner, true);
    duplicate.evidence.push(swift_stdlib_map_factory_evidence(
        5,
        sp(63),
        map_contract,
        1,
        vec![EvidenceId(4)],
    ));
    let facts = StrictFacts::collect(&duplicate, &interner);
    assert!(!strict_exact_swift_map_factory_safe(
        &duplicate,
        &interner,
        &facts,
        duplicate_call
    ));
}

fn swift_collection_factory_il(interner: &Interner, factory: &str) -> (Il, NodeId) {
    let mut b = IlBuilder::new(FileId(0));
    let callee = b.add(
        NodeKind::Var,
        Payload::Name(interner.intern(factory)),
        sp(50),
        &[],
    );
    let item = b.add(NodeKind::Lit, Payload::LitInt(1), sp(51), &[]);
    let entries = b.add(
        NodeKind::Seq,
        Payload::Name(interner.intern("array")),
        sp(52),
        &[item],
    );
    let call = b.add(NodeKind::Call, Payload::None, sp(53), &[callee, entries]);
    let root = b.add(NodeKind::Block, Payload::None, sp(50), &[call]);
    let mut il = b.finish(
        root,
        FileMeta {
            path: "t.swift".into(),
            lang: Lang::Swift,
        },
        Vec::new(),
        Vec::new(),
    );
    il.evidence.push(sequence_surface_evidence(
        0,
        Lang::Swift,
        sp(52),
        SequenceSurfaceKind::Collection,
    ));
    il.evidence.push(language_core_symbol_evidence(
        1,
        Lang::Swift,
        EvidenceAnchor::node(sp(50), NodeKind::Var),
        SymbolEvidenceKind::UnshadowedGlobal {
            name_hash: stable_symbol_hash(factory),
        },
        Vec::new(),
    ));
    (il, call)
}

fn swift_dictionary_unique_keys_il(interner: &Interner, duplicate: bool) -> (Il, NodeId) {
    let mut b = IlBuilder::new(FileId(0));
    let callee = b.add(
        NodeKind::Var,
        Payload::Name(interner.intern("Dictionary")),
        sp(60),
        &[],
    );
    let first_key = b.add(
        NodeKind::Lit,
        Payload::LitStr(stable_symbol_hash("red")),
        sp(61),
        &[],
    );
    let first_value = b.add(NodeKind::Lit, Payload::LitInt(1), sp(61), &[]);
    let first_entry = b.add(
        NodeKind::Seq,
        Payload::Name(interner.intern("tuple")),
        sp(61),
        &[first_key, first_value],
    );
    let second_key = b.add(
        NodeKind::Lit,
        Payload::LitStr(stable_symbol_hash(if duplicate { "red" } else { "blue" })),
        sp(62),
        &[],
    );
    let second_value = b.add(NodeKind::Lit, Payload::LitInt(2), sp(62), &[]);
    let second_entry = b.add(
        NodeKind::Seq,
        Payload::Name(interner.intern("tuple")),
        sp(62),
        &[second_key, second_value],
    );
    let entries = b.add(
        NodeKind::Seq,
        Payload::Name(interner.intern("array")),
        sp(63),
        &[first_entry, second_entry],
    );
    let kwarg = b.add(
        NodeKind::KwArg,
        Payload::Name(interner.intern("uniqueKeysWithValues")),
        sp(63),
        &[entries],
    );
    let call = b.add(NodeKind::Call, Payload::None, sp(63), &[callee, kwarg]);
    let root = b.add(NodeKind::Block, Payload::None, sp(60), &[call]);
    let mut il = b.finish(
        root,
        FileMeta {
            path: "t.swift".into(),
            lang: Lang::Swift,
        },
        Vec::new(),
        Vec::new(),
    );
    il.evidence.push(sequence_surface_evidence(
        0,
        Lang::Swift,
        sp(61),
        SequenceSurfaceKind::Tuple,
    ));
    il.evidence.push(sequence_surface_evidence(
        1,
        Lang::Swift,
        sp(62),
        SequenceSurfaceKind::Tuple,
    ));
    il.evidence.push(sequence_surface_evidence(
        2,
        Lang::Swift,
        sp(63),
        SequenceSurfaceKind::Collection,
    ));
    il.evidence.push(language_core_symbol_evidence(
        4,
        Lang::Swift,
        EvidenceAnchor::node(sp(60), NodeKind::Var),
        SymbolEvidenceKind::UnshadowedGlobal {
            name_hash: stable_symbol_hash("Dictionary"),
        },
        Vec::new(),
    ));
    (il, call)
}
