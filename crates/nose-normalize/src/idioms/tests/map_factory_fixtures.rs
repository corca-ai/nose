use super::support::*;

pub(super) fn rust_hash_map_from_call(
    entry_surface: &str,
    shadow_std: bool,
    with_entries_surface: bool,
) -> (Il, Interner, NodeId) {
    let interner = Interner::new();
    let mut b = IlBuilder::new(FileId(0));
    let entry_span = sp_at(10, 20, 1);
    let entries_span = sp_at(30, 40, 1);
    let key = b.add(NodeKind::Lit, Payload::LitStr(1), sp(), &[]);
    let value = b.add(NodeKind::Lit, Payload::LitInt(1), sp(), &[]);
    let entry = b.add(
        NodeKind::Seq,
        Payload::Name(interner.intern(entry_surface)),
        entry_span,
        &[key, value],
    );
    let entries = b.add(
        NodeKind::Seq,
        Payload::Name(interner.intern("array")),
        entries_span,
        &[entry],
    );
    let callee = b.add(
        NodeKind::Var,
        Payload::Name(interner.intern("std::collections::HashMap::from")),
        sp(),
        &[],
    );
    let call = b.add(NodeKind::Call, Payload::None, sp(), &[callee, entries]);
    let root = b.add(NodeKind::Module, Payload::None, sp(), &[call]);
    let units = if shadow_std {
        vec![Unit {
            root,
            kind: UnitKind::Class,
            name: Some(interner.intern("std")),
            origin: Default::default(),
        }]
    } else {
        Vec::new()
    };
    let mut il = b.finish(
        root,
        FileMeta {
            path: "t".to_string(),
            lang: Lang::Rust,
        },
        units,
        Vec::new(),
    );
    let entry_kind = match entry_surface {
        "tuple" => SequenceSurfaceKind::Tuple,
        "array" => SequenceSurfaceKind::Collection,
        other => panic!("unexpected test entry surface: {other}"),
    };
    il.push_evidence(sequence_surface_evidence(
        0,
        Lang::Rust,
        entry_span,
        entry_kind,
    ));
    if with_entries_surface {
        il.push_evidence(sequence_surface_evidence(
            1,
            Lang::Rust,
            entries_span,
            SequenceSurfaceKind::Collection,
        ));
    }
    (il, interner, call)
}

pub(super) fn push_rust_hash_map_library_api_evidence(il: &mut Il) {
    let contract =
        library_free_name_map_factory_contract(Lang::Rust, "std::collections::HashMap::from")
            .expect("Rust HashMap::from contract");
    let symbol_id = next_evidence_id(il);
    il.push_evidence(language_core_symbol_evidence(
        symbol_id,
        Lang::Rust,
        EvidenceAnchor::node(sp(), NodeKind::Var),
        SymbolEvidenceKind::UnshadowedGlobal {
            name_hash: stable_symbol_hash("std::collections::HashMap::from"),
        },
        EvidenceStatus::Asserted,
    ));
    let api_id = next_evidence_id(il);
    let mut record = evidence_with_dependencies(
        api_id,
        EvidenceAnchor::node(sp(), NodeKind::Call),
        EvidenceKind::LibraryApi(LibraryApiEvidenceKind::Contract {
            contract_hash: nose_semantics::library_api_contract_id_hash(contract.id),
            callee_hash: nose_semantics::library_api_callee_contract_hash(contract.callee),
            arity: 1,
        }),
        EvidenceStatus::Asserted,
        vec![EvidenceId(symbol_id)],
    );
    record.provenance.pack_hash = Some(stable_symbol_hash(contract.pack_id));
    record.provenance.rule_hash = Some(stable_symbol_hash(
        nose_semantics::RUST_STDLIB_MAP_FACTORY_PRODUCER_ID,
    ));
    il.push_evidence(record);
}
