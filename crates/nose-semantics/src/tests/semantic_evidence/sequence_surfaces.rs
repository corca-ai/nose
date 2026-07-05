use super::*;

#[test]
fn sequence_surface_contracts_keep_value_and_exact_axes_separate() {
    let array = seq_surface_contract(Lang::JavaScript, Some("array")).unwrap();
    assert_eq!(array.value_tag, SEQ_VALUE_COLLECTION);
    assert!(array.exact_tree_safe);
    assert!(array.membership_collection);

    let untagged = seq_surface_contract(Lang::JavaScript, None).unwrap();
    assert_eq!(untagged.value_tag, SEQ_VALUE_UNTAGGED);
    assert!(!untagged.exact_tree_safe);
    assert!(!untagged.membership_collection);

    let object = seq_surface_contract(Lang::JavaScript, Some("object")).unwrap();
    assert_eq!(object.value_tag, SEQ_VALUE_MAP);
    assert!(object.exact_tree_safe);
    assert!(!object.membership_collection);
    assert!(object.imported_literal);
}

#[test]
fn go_sequence_surface_contracts_stay_language_scoped() {
    let go_map = seq_surface_contract(Lang::Go, Some("composite_literal")).unwrap();
    assert_eq!(
        go_map.value_tag,
        stable_symbol_hash("go_composite_map_literal")
    );
    assert!(!go_map.exact_tree_safe);
    assert!(!go_map.membership_collection);
    assert!(!go_map.imported_literal);

    let go_entry = seq_surface_contract(Lang::Go, Some("keyed_element")).unwrap();
    assert_eq!(go_entry.value_tag, stable_symbol_hash("keyed_element"));
    assert!(!go_entry.exact_tree_safe);
    assert!(!go_entry.membership_collection);

    assert!(seq_surface_contract(Lang::Python, Some("composite_literal")).is_none());
    assert!(seq_surface_contract(Lang::Python, Some("keyed_element")).is_none());
    assert!(imported_literal_seq_tag_safe(Lang::Python, "dictionary"));
    assert!(!imported_literal_seq_tag_safe(Lang::Ruby, "hash"));
}

#[test]
fn sequence_surface_evidence_must_match_the_lowered_surface() {
    let interner = Interner::new();
    let mut b = IlBuilder::new(FileId(0));
    let array = interner.intern("array");
    let seq = b.add(NodeKind::Seq, Payload::Name(array), sp(5), &[]);
    let root = b.add(NodeKind::Block, Payload::None, sp(5), &[seq]);
    let mut il = finish_il(b, root, Lang::JavaScript);

    assert_eq!(
        seq_surface_contract_for_node(&il, &interner, seq),
        None,
        "raw sequence tags do not prove semantic surfaces without evidence"
    );

    il.evidence.push(language_core_evidence(
        0,
        EvidenceAnchor::sequence(sp(5)),
        EvidenceKind::SequenceSurface(SequenceSurfaceKind::Collection),
        EvidenceStatus::Asserted,
        Lang::JavaScript,
    ));
    assert!(seq_surface_contract_for_node(&il, &interner, seq)
        .is_some_and(|contract| contract.membership_collection));

    il.evidence.push(language_core_evidence(
        1,
        EvidenceAnchor::sequence(sp(5)),
        EvidenceKind::SequenceSurface(SequenceSurfaceKind::Map),
        EvidenceStatus::Asserted,
        Lang::JavaScript,
    ));
    assert_eq!(seq_surface_contract_for_node(&il, &interner, seq), None);
}

#[test]
fn sequence_surface_evidence_requires_matching_language_core_provenance() {
    let interner = Interner::new();
    let mut b = IlBuilder::new(FileId(0));
    let array = interner.intern("array");
    let seq = b.add(NodeKind::Seq, Payload::Name(array), sp(15), &[]);
    let root = b.add(NodeKind::Block, Payload::None, sp(15), &[seq]);
    let mut il = finish_il(b, root, Lang::JavaScript);

    il.evidence.push(evidence(
        0,
        EvidenceAnchor::sequence(sp(15)),
        EvidenceKind::SequenceSurface(SequenceSurfaceKind::Collection),
        EvidenceStatus::Asserted,
    ));
    assert_eq!(
        seq_surface_contract_for_node(&il, &interner, seq),
        None,
        "legacy broad provenance must not prove a sequence surface"
    );

    il.evidence.clear();
    il.evidence.push(language_core_evidence(
        0,
        EvidenceAnchor::sequence(sp(15)),
        EvidenceKind::SequenceSurface(SequenceSurfaceKind::Collection),
        EvidenceStatus::Asserted,
        Lang::Python,
    ));
    assert_eq!(
        seq_surface_contract_for_node(&il, &interner, seq),
        None,
        "wrong-language builtin provenance must not prove a sequence surface"
    );

    il.evidence.clear();
    let mut external = language_core_evidence(
        0,
        EvidenceAnchor::sequence(sp(15)),
        EvidenceKind::SequenceSurface(SequenceSurfaceKind::Collection),
        EvidenceStatus::Asserted,
        Lang::JavaScript,
    );
    external.provenance.emitter = EvidenceEmitter::External;
    il.evidence.push(external);
    assert_eq!(
        seq_surface_contract_for_node(&il, &interner, seq),
        None,
        "external provenance must not prove a builtin sequence surface"
    );

    il.evidence.clear();
    il.evidence.push(language_core_evidence(
        0,
        EvidenceAnchor::sequence(sp(15)),
        EvidenceKind::SequenceSurface(SequenceSurfaceKind::Collection),
        EvidenceStatus::Asserted,
        Lang::JavaScript,
    ));
    assert!(seq_surface_contract_for_node(&il, &interner, seq)
        .is_some_and(|contract| contract.membership_collection));
}

#[test]
fn imported_literal_export_safety_requires_sequence_evidence() {
    let interner = Interner::new();
    let mut b = IlBuilder::new(FileId(0));
    let object = interner.intern("object");
    let key = b.add(
        NodeKind::Lit,
        Payload::LitStr(stable_symbol_hash("ready")),
        sp(6),
        &[],
    );
    let value = b.add(NodeKind::Lit, Payload::LitInt(1), sp(6), &[]);
    let entry = b.add(NodeKind::Seq, Payload::Name(object), sp(6), &[key, value]);
    let root = b.add(NodeKind::Block, Payload::None, sp(6), &[entry]);
    let mut il = finish_il(b, root, Lang::JavaScript);

    assert!(!imported_literal_export_safe(&il, &interner, entry));

    il.evidence.push(language_core_evidence(
        0,
        EvidenceAnchor::sequence(sp(6)),
        EvidenceKind::SequenceSurface(SequenceSurfaceKind::Map),
        EvidenceStatus::Asserted,
        Lang::JavaScript,
    ));
    assert!(imported_literal_export_safe(&il, &interner, entry));
}

#[test]
fn imported_literal_export_safety_rejects_import_coordinate_children() {
    let interner = Interner::new();
    let mut b = IlBuilder::new(FileId(0));
    let object = interner.intern("object");
    let imported = b.add(NodeKind::Seq, Payload::None, sp(7), &[]);
    let root_value = b.add(NodeKind::Seq, Payload::Name(object), sp(8), &[imported]);
    let root = b.add(NodeKind::Block, Payload::None, sp(8), &[root_value]);
    let mut il = finish_il(b, root, Lang::JavaScript);
    il.evidence.push(language_core_evidence(
        0,
        EvidenceAnchor::sequence(sp(8)),
        EvidenceKind::SequenceSurface(SequenceSurfaceKind::Map),
        EvidenceStatus::Asserted,
        Lang::JavaScript,
    ));
    il.evidence.push(language_core_evidence(
        1,
        EvidenceAnchor::sequence(sp(7)),
        EvidenceKind::Import(ImportEvidenceKind::Binding {
            module_hash: stable_symbol_hash("provider"),
            exported_hash: stable_symbol_hash("VALUE"),
        }),
        EvidenceStatus::Asserted,
        Lang::JavaScript,
    ));

    assert!(!imported_literal_export_safe(&il, &interner, root_value));
    assert_eq!(
        imported_literal_export_rejection_reason(&il, &interner, root_value),
        Some("provider-aggregate-child-import-coordinate-boundary")
    );
}

#[test]
fn imported_literal_export_rejection_reports_reference_children() {
    let interner = Interner::new();
    let mut b = IlBuilder::new(FileId(0));
    let array = interner.intern("array");
    let referenced = b.add(
        NodeKind::Var,
        Payload::Name(interner.intern("DESCRIPTOR")),
        sp(9),
        &[],
    );
    let root_value = b.add(NodeKind::Seq, Payload::Name(array), sp(9), &[referenced]);
    let root = b.add(NodeKind::Block, Payload::None, sp(9), &[root_value]);
    let mut il = finish_il(b, root, Lang::Rust);
    il.evidence.push(language_core_evidence(
        0,
        EvidenceAnchor::sequence(sp(9)),
        EvidenceKind::SequenceSurface(SequenceSurfaceKind::Collection),
        EvidenceStatus::Asserted,
        Lang::Rust,
    ));

    assert!(!imported_literal_export_safe(&il, &interner, root_value));
    assert_eq!(
        imported_literal_export_rejection_reason(&il, &interner, root_value),
        Some("provider-aggregate-child-reference-boundary")
    );
}

#[test]
fn imported_literal_export_safety_rejects_throwing_guava_map_factories() {
    let valid = [
        Payload::LitStr(stable_symbol_hash("red")),
        Payload::LitInt(1),
        Payload::LitStr(stable_symbol_hash("blue")),
        Payload::LitInt(2),
    ];
    let (il, interner, call) =
        test_support::guava_immutable_map_of_test_il(&valid, 20, guava_map_fixture_options());
    assert!(imported_literal_export_safe(&il, &interner, call));

    let duplicate = [
        Payload::LitStr(stable_symbol_hash("red")),
        Payload::LitInt(1),
        Payload::LitStr(stable_symbol_hash("red")),
        Payload::LitInt(2),
    ];
    let (il, interner, call) =
        test_support::guava_immutable_map_of_test_il(&duplicate, 30, guava_map_fixture_options());
    assert!(!imported_literal_export_safe(&il, &interner, call));

    let null_key = [
        Payload::Lit(LitClass::Null),
        Payload::LitInt(1),
        Payload::LitStr(stable_symbol_hash("blue")),
        Payload::LitInt(2),
    ];
    let (il, interner, call) =
        test_support::guava_immutable_map_of_test_il(&null_key, 40, guava_map_fixture_options());
    assert!(!imported_literal_export_safe(&il, &interner, call));

    let unsupported_arity = test_support::guava_immutable_map_eleven_entry_payloads();
    let (il, interner, call) = test_support::guava_immutable_map_of_test_il(
        &unsupported_arity,
        50,
        guava_map_fixture_options(),
    );
    assert!(!imported_literal_export_safe(&il, &interner, call));
}

fn guava_map_fixture_options() -> test_support::GuavaImmutableMapFixtureOptions {
    test_support::GuavaImmutableMapFixtureOptions {
        root_kind: test_support::GuavaImmutableMapFixtureRoot::Block,
        span_lines: test_support::GuavaImmutableMapFixtureSpanLines::SingleLine,
        import_rhs: test_support::GuavaImmutableMapFixtureImportRhs::EmptySeq,
        include_function_unit: true,
        path: "t",
    }
}

#[test]
fn go_zero_map_surface_helpers_require_evidence() {
    let interner = Interner::new();
    let mut b = IlBuilder::new(FileId(0));
    let key = b.add(
        NodeKind::Lit,
        Payload::LitStr(stable_symbol_hash("ready")),
        sp(32),
        &[],
    );
    let value = b.add(NodeKind::Lit, Payload::LitInt(1), sp(32), &[]);
    let entry = b.add(
        NodeKind::Seq,
        Payload::Name(interner.intern("keyed_element")),
        sp(32),
        &[key, value],
    );
    let map = b.add(
        NodeKind::Seq,
        Payload::Name(interner.intern("composite_literal")),
        sp(31),
        &[entry],
    );
    let root = b.add(NodeKind::Block, Payload::None, sp(31), &[map]);
    let mut il = finish_il(b, root, Lang::Go);

    assert!(go_zero_map_literal_contract_for_node(&il, &interner, map).is_none());
    assert!(go_zero_map_entry_contract_for_node(&il, &interner, entry).is_none());

    il.evidence.push(language_core_evidence(
        0,
        EvidenceAnchor::sequence(sp(31)),
        EvidenceKind::SequenceSurface(SequenceSurfaceKind::GoCompositeMapLiteral),
        EvidenceStatus::Asserted,
        Lang::Go,
    ));
    assert!(go_zero_map_literal_contract_for_node(&il, &interner, map).is_some());
    assert!(go_zero_map_entry_contract_for_node(&il, &interner, entry).is_none());

    il.evidence.push(language_core_evidence(
        1,
        EvidenceAnchor::sequence(sp(32)),
        EvidenceKind::SequenceSurface(SequenceSurfaceKind::GoMapEntry),
        EvidenceStatus::Asserted,
        Lang::Go,
    ));
    assert!(go_zero_map_entry_contract_for_node(&il, &interner, entry).is_some());
}

#[test]
fn rust_struct_expression_surface_is_exact_safe_but_not_collection_like() {
    let rust_struct = seq_surface_contract(Lang::Rust, Some("rust_struct_expression")).unwrap();
    assert_eq!(rust_struct.value_tag, SEQ_VALUE_RUST_STRUCT_EXPRESSION);
    assert!(rust_struct.exact_tree_safe);
    assert!(!rust_struct.membership_collection);
    assert!(!rust_struct.map_entry_list);
    assert!(!rust_struct.imported_literal);

    assert!(seq_surface_contract(Lang::JavaScript, Some("rust_struct_expression")).is_none());
    assert!(
        seq_surface_contract(Lang::Rust, None).is_some_and(|contract| {
            !contract.exact_tree_safe && contract.value_tag == SEQ_VALUE_UNTAGGED
        })
    );
}
