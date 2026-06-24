use super::*;

#[test]
fn strict_exact_java_collections_singleton_list_uses_fixed_element_contract() {
    let interner = Interner::new();
    let mut b = IlBuilder::new(FileId(0));
    let import = imported_binding_assignment(&mut b, &interner, "Collections", sp(120));
    let receiver = b.add(
        NodeKind::Var,
        Payload::Name(interner.intern("Collections")),
        sp(121),
        &[],
    );
    let callee = b.add(
        NodeKind::Field,
        Payload::Name(interner.intern("singletonList")),
        sp(122),
        &[receiver],
    );
    let item = b.add(NodeKind::Lit, Payload::LitInt(1), sp(123), &[]);
    let call = b.add(NodeKind::Call, Payload::None, sp(124), &[callee, item]);
    let root = b.add(NodeKind::Block, Payload::None, sp(120), &[import, call]);
    let mut il = finish_java_il(b, root);
    push_java_util_import_symbol(&mut il, "Collections", sp(120), sp(121));

    let facts = StrictFacts::collect(&il, &interner);
    assert!(!strict_exact_java_collection_factory_safe(
        &il, &interner, &facts, call
    ));

    let contract =
        library_java_collection_factory_contract(Lang::Java, "Collections", "singletonList")
            .expect("Collections.singletonList contract");
    push_java_stdlib_api_evidence(
        &mut il,
        2,
        sp(124),
        contract.id,
        contract.callee,
        1,
        JAVA_STDLIB_COLLECTION_FACTORY_PACK_ID,
        JAVA_STDLIB_COLLECTION_FACTORY_PRODUCER_ID,
    );
    let facts = StrictFacts::collect(&il, &interner);
    assert!(strict_exact_java_collection_factory_safe(
        &il, &interner, &facts, call
    ));
}

#[test]
fn strict_exact_java_collections_empty_map_uses_zero_arity_map_contract() {
    let interner = Interner::new();
    let mut b = IlBuilder::new(FileId(0));
    let import = imported_binding_assignment(&mut b, &interner, "Collections", sp(130));
    let receiver = b.add(
        NodeKind::Var,
        Payload::Name(interner.intern("Collections")),
        sp(131),
        &[],
    );
    let callee = b.add(
        NodeKind::Field,
        Payload::Name(interner.intern("emptyMap")),
        sp(132),
        &[receiver],
    );
    let call = b.add(NodeKind::Call, Payload::None, sp(133), &[callee]);
    let root = b.add(NodeKind::Block, Payload::None, sp(130), &[import, call]);
    let mut il = finish_java_il(b, root);
    push_java_util_import_symbol(&mut il, "Collections", sp(130), sp(131));

    let facts = StrictFacts::collect(&il, &interner);
    assert!(!strict_exact_java_map_factory_safe(
        &il, &interner, &facts, call
    ));

    let contract = library_java_map_factory_contract(Lang::Java, "Collections", "emptyMap")
        .expect("Collections.emptyMap contract");
    push_java_stdlib_api_evidence(
        &mut il,
        2,
        sp(133),
        contract.id,
        contract.callee,
        0,
        JAVA_STDLIB_MAP_FACTORY_PACK_ID,
        JAVA_STDLIB_MAP_FACTORY_PRODUCER_ID,
    );
    let facts = StrictFacts::collect(&il, &interner);
    assert!(strict_exact_java_map_factory_safe(
        &il, &interner, &facts, call
    ));
}
