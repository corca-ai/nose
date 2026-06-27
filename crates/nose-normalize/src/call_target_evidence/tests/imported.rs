use super::support::*;

#[test]
fn emits_imported_function_call_target_from_binding_symbol() {
    let interner = Interner::new();
    let p = interner.intern("p");
    let mut b = IlBuilder::new(FileId(0));
    let callee = b.add(NodeKind::Var, Payload::Name(p), sp(10), &[]);
    let arg = b.add(NodeKind::Lit, Payload::LitInt(3), sp(11), &[]);
    let call = b.add(NodeKind::Call, Payload::None, sp(12), &[callee, arg]);
    let ret = b.add(NodeKind::Return, Payload::None, sp(13), &[call]);
    let body = b.add(NodeKind::Block, Payload::None, sp(14), &[ret]);
    let func = b.add(NodeKind::Func, Payload::None, wide_sp(8, 20), &[body]);
    let module = b.add(NodeKind::Module, Payload::None, wide_sp(0, 30), &[func]);
    let mut il = b.finish(
        module,
        FileMeta {
            path: "t".into(),
            lang: Lang::Python,
        },
        Vec::new(),
        Vec::new(),
    );
    il.evidence.push(binding_symbol(
        0,
        sp(1),
        "p",
        SymbolEvidenceKind::ImportedBinding {
            module_hash: stable_symbol_hash("math"),
            exported_hash: stable_symbol_hash("prod"),
        },
        EvidenceStatus::Asserted,
    ));

    run(&mut il, &interner);

    let expected = CallTargetEvidenceKind::ImportedFunction {
        module_hash: stable_symbol_hash("math"),
        exported_hash: stable_symbol_hash("prod"),
        local_hash: interner.symbol_hash(p),
    };
    assert_eq!(
        call_target_evidence_at_call(&il, &interner, call),
        Some(expected)
    );
    assert!(imported_function_call_target_at_call(&il, &interner, call));
    let target_record = il
        .evidence
        .iter()
        .find(|record| record.kind == EvidenceKind::CallTarget(expected))
        .expect("imported function call-target evidence");
    assert_eq!(
        target_record.provenance,
        language_core_provenance(Lang::Python)
    );
    let [occurrence_dependency] = target_record.dependencies.as_slice() else {
        panic!("call-target should depend on exactly one occurrence symbol");
    };
    let occurrence = il
        .evidence_record_by_id(*occurrence_dependency)
        .expect("occurrence dependency");
    assert_eq!(
        occurrence.kind,
        EvidenceKind::Symbol(SymbolEvidenceKind::ImportedBinding {
            module_hash: stable_symbol_hash("math"),
            exported_hash: stable_symbol_hash("prod"),
        })
    );
    assert_eq!(occurrence.dependencies, vec![EvidenceId(0)]);
    assert_eq!(
        occurrence.provenance,
        language_core_provenance(Lang::Python)
    );
}

#[test]
fn emits_imported_member_call_target_from_namespace_symbol() {
    let interner = Interner::new();
    let m = interner.intern("m");
    let prod = interner.intern("prod");
    let mut b = IlBuilder::new(FileId(0));
    let receiver = b.add(NodeKind::Var, Payload::Name(m), sp(10), &[]);
    let callee = b.add(NodeKind::Field, Payload::Name(prod), sp(11), &[receiver]);
    let arg = b.add(NodeKind::Lit, Payload::LitInt(3), sp(12), &[]);
    let call = b.add(NodeKind::Call, Payload::None, sp(13), &[callee, arg]);
    let ret = b.add(NodeKind::Return, Payload::None, sp(14), &[call]);
    let body = b.add(NodeKind::Block, Payload::None, sp(15), &[ret]);
    let func = b.add(NodeKind::Func, Payload::None, wide_sp(8, 20), &[body]);
    let module = b.add(NodeKind::Module, Payload::None, wide_sp(0, 30), &[func]);
    let mut il = b.finish(
        module,
        FileMeta {
            path: "t".into(),
            lang: Lang::Python,
        },
        Vec::new(),
        Vec::new(),
    );
    il.evidence.push(binding_symbol(
        0,
        sp(1),
        "m",
        SymbolEvidenceKind::ImportedNamespace {
            module_hash: stable_symbol_hash("math"),
        },
        EvidenceStatus::Asserted,
    ));

    run(&mut il, &interner);

    let member_hash = interner.symbol_hash(prod);
    assert_eq!(
        call_target_evidence_at_call(&il, &interner, call),
        Some(CallTargetEvidenceKind::ImportedMember {
            module_hash: stable_symbol_hash("math"),
            exported_hash: member_hash,
            member_hash,
        })
    );
    assert!(imported_member_call_target_at_call(&il, &interner, call));
}

#[test]
fn emits_imported_member_call_target_from_imported_binding_receiver() {
    let interner = Interner::new();
    let map = interner.intern("Map");
    let of = interner.intern("of");
    let mut b = IlBuilder::new(FileId(0));
    let receiver = b.add(NodeKind::Var, Payload::Name(map), sp(10), &[]);
    let callee = b.add(NodeKind::Field, Payload::Name(of), sp(11), &[receiver]);
    let call = b.add(NodeKind::Call, Payload::None, sp(12), &[callee]);
    let ret = b.add(NodeKind::Return, Payload::None, sp(13), &[call]);
    let body = b.add(NodeKind::Block, Payload::None, sp(14), &[ret]);
    let func = b.add(NodeKind::Func, Payload::None, wide_sp(8, 20), &[body]);
    let module = b.add(NodeKind::Module, Payload::None, wide_sp(0, 30), &[func]);
    let mut il = b.finish(
        module,
        FileMeta {
            path: "t".into(),
            lang: Lang::Java,
        },
        Vec::new(),
        Vec::new(),
    );
    il.evidence.push(binding_symbol(
        0,
        sp(1),
        "Map",
        SymbolEvidenceKind::ImportedBinding {
            module_hash: stable_symbol_hash("java.util"),
            exported_hash: stable_symbol_hash("Map"),
        },
        EvidenceStatus::Asserted,
    ));

    run(&mut il, &interner);

    assert_eq!(
        call_target_evidence_at_call(&il, &interner, call),
        Some(CallTargetEvidenceKind::ImportedMember {
            module_hash: stable_symbol_hash("java.util"),
            exported_hash: stable_symbol_hash("Map"),
            member_hash: interner.symbol_hash(of),
        })
    );
}

#[test]
fn emits_imported_member_call_target_from_scoped_imported_binding_root() {
    let interner = Interner::new();
    let span_new = interner.intern("Span::new");
    let mut b = IlBuilder::new(FileId(0));
    let callee = b.add(NodeKind::Var, Payload::Name(span_new), sp(10), &[]);
    let call = b.add(NodeKind::Call, Payload::None, sp(11), &[callee]);
    let ret = b.add(NodeKind::Return, Payload::None, sp(12), &[call]);
    let body = b.add(NodeKind::Block, Payload::None, sp(13), &[ret]);
    let func = b.add(NodeKind::Func, Payload::None, wide_sp(8, 20), &[body]);
    let module = b.add(NodeKind::Module, Payload::None, wide_sp(0, 30), &[func]);
    let mut il = b.finish(
        module,
        FileMeta {
            path: "t".into(),
            lang: Lang::Rust,
        },
        Vec::new(),
        Vec::new(),
    );
    il.evidence.push(binding_symbol(
        0,
        sp(1),
        "Span",
        SymbolEvidenceKind::ImportedBinding {
            module_hash: stable_symbol_hash("nose_il"),
            exported_hash: stable_symbol_hash("Span"),
        },
        EvidenceStatus::Asserted,
    ));

    run(&mut il, &interner);

    let expected = CallTargetEvidenceKind::ImportedMember {
        module_hash: stable_symbol_hash("nose_il"),
        exported_hash: stable_symbol_hash("Span"),
        member_hash: stable_symbol_hash("new"),
    };
    assert_eq!(
        call_target_evidence_at_call(&il, &interner, call),
        Some(expected)
    );
    assert!(imported_member_call_target_at_call(&il, &interner, call));
    let target_record = il
        .evidence
        .iter()
        .find(|record| record.kind == EvidenceKind::CallTarget(expected))
        .expect("scoped imported member call-target evidence");
    let [occurrence_dependency] = target_record.dependencies.as_slice() else {
        panic!("scoped call-target should depend on exactly one occurrence symbol");
    };
    let occurrence = il
        .evidence_record_by_id(*occurrence_dependency)
        .expect("occurrence dependency");
    assert_eq!(
        occurrence.anchor,
        EvidenceAnchor::node(il.node(callee).span, NodeKind::Var)
    );
    assert_eq!(
        occurrence.kind,
        EvidenceKind::Symbol(SymbolEvidenceKind::ImportedBinding {
            module_hash: stable_symbol_hash("nose_il"),
            exported_hash: stable_symbol_hash("Span"),
        })
    );
    assert_eq!(occurrence.dependencies, vec![EvidenceId(0)]);
}

#[test]
fn scoped_imported_member_call_target_uses_full_suffix_for_nested_paths() {
    let interner = Interner::new();
    let value_from = interner.intern("json::value::from_value");
    let mut b = IlBuilder::new(FileId(0));
    let callee = b.add(NodeKind::Var, Payload::Name(value_from), sp(10), &[]);
    let call = b.add(NodeKind::Call, Payload::None, sp(11), &[callee]);
    let ret = b.add(NodeKind::Return, Payload::None, sp(12), &[call]);
    let body = b.add(NodeKind::Block, Payload::None, sp(13), &[ret]);
    let func = b.add(NodeKind::Func, Payload::None, wide_sp(8, 20), &[body]);
    let module = b.add(NodeKind::Module, Payload::None, wide_sp(0, 30), &[func]);
    let mut il = b.finish(
        module,
        FileMeta {
            path: "t".into(),
            lang: Lang::Rust,
        },
        Vec::new(),
        Vec::new(),
    );
    il.evidence.push(binding_symbol(
        0,
        sp(1),
        "json",
        SymbolEvidenceKind::ImportedNamespace {
            module_hash: stable_symbol_hash("serde_json"),
        },
        EvidenceStatus::Asserted,
    ));

    run(&mut il, &interner);

    assert_eq!(
        call_target_evidence_at_call(&il, &interner, call),
        Some(CallTargetEvidenceKind::ImportedMember {
            module_hash: stable_symbol_hash("serde_json"),
            exported_hash: stable_symbol_hash("value::from_value"),
            member_hash: stable_symbol_hash("value::from_value"),
        })
    );
}

#[test]
fn updates_legacy_first_party_imported_function_records() {
    let interner = Interner::new();
    let p = interner.intern("p");
    let mut b = IlBuilder::new(FileId(0));
    let callee = b.add(NodeKind::Var, Payload::Name(p), sp(10), &[]);
    let call = b.add(NodeKind::Call, Payload::None, sp(12), &[callee]);
    let ret = b.add(NodeKind::Return, Payload::None, sp(13), &[call]);
    let body = b.add(NodeKind::Block, Payload::None, sp(14), &[ret]);
    let func = b.add(NodeKind::Func, Payload::None, wide_sp(8, 20), &[body]);
    let module = b.add(NodeKind::Module, Payload::None, wide_sp(0, 30), &[func]);
    let mut il = b.finish(
        module,
        FileMeta {
            path: "t".into(),
            lang: Lang::Python,
        },
        Vec::new(),
        Vec::new(),
    );
    il.evidence.push(binding_symbol(
        0,
        sp(1),
        "p",
        SymbolEvidenceKind::ImportedBinding {
            module_hash: stable_symbol_hash("math"),
            exported_hash: stable_symbol_hash("prod"),
        },
        EvidenceStatus::Asserted,
    ));
    let symbol = EvidenceKind::Symbol(SymbolEvidenceKind::ImportedBinding {
        module_hash: stable_symbol_hash("math"),
        exported_hash: stable_symbol_hash("prod"),
    });
    let legacy_occurrence = il.find_or_push_first_party_evidence(
        EvidenceAnchor::node(il.node(callee).span, NodeKind::Var),
        symbol,
        BUILTIN_COMPAT_PACK_ID,
        "legacy_imported_symbol_occurrence",
        vec![EvidenceId(0)],
    );
    let target = EvidenceKind::CallTarget(CallTargetEvidenceKind::ImportedFunction {
        module_hash: stable_symbol_hash("math"),
        exported_hash: stable_symbol_hash("prod"),
        local_hash: interner.symbol_hash(p),
    });
    il.find_or_push_first_party_evidence(
        EvidenceAnchor::node(il.node(call).span, NodeKind::Call),
        target,
        BUILTIN_COMPAT_PACK_ID,
        "legacy_imported_function_call_target",
        vec![legacy_occurrence],
    );

    run(&mut il, &interner);

    let target_records: Vec<_> = il
        .evidence
        .iter()
        .filter(|record| {
            record.anchor == EvidenceAnchor::node(il.node(call).span, NodeKind::Call)
                && record.kind == target
        })
        .collect();
    assert_eq!(target_records.len(), 1);
    assert_eq!(
        target_records[0].provenance,
        language_core_provenance(Lang::Python)
    );
    let [occurrence_dependency] = target_records[0].dependencies.as_slice() else {
        panic!("call-target should depend on exactly one occurrence symbol");
    };
    let occurrence_records: Vec<_> = il
        .evidence
        .iter()
        .filter(|record| {
            record.anchor == EvidenceAnchor::node(il.node(callee).span, NodeKind::Var)
                && record.kind == symbol
        })
        .collect();
    assert_eq!(occurrence_records.len(), 1);
    assert_eq!(occurrence_records[0].id, *occurrence_dependency);
    assert_eq!(
        occurrence_records[0].provenance,
        language_core_provenance(Lang::Python)
    );
    assert_eq!(occurrence_records[0].dependencies, vec![EvidenceId(0)]);
}
