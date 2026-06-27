use super::support::*;

#[test]
fn does_not_emit_imported_function_target_for_ambiguous_binding_symbol() {
    let interner = Interner::new();
    let p = interner.intern("p");
    let mut b = IlBuilder::new(FileId(0));
    let callee = b.add(NodeKind::Var, Payload::Name(p), sp(10), &[]);
    let call = b.add(NodeKind::Call, Payload::None, sp(11), &[callee]);
    let ret = b.add(NodeKind::Return, Payload::None, sp(12), &[call]);
    let body = b.add(NodeKind::Block, Payload::None, sp(13), &[ret]);
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
        EvidenceStatus::Ambiguous,
    ));

    run(&mut il, &interner);
    assert_eq!(call_target_evidence_at_call(&il, &interner, call), None);
}

#[test]
fn does_not_emit_imported_function_target_when_local_assignment_rebinds_alias() {
    let interner = Interner::new();
    let p = interner.intern("p");
    let mut b = IlBuilder::new(FileId(0));
    let lhs = b.add(NodeKind::Var, Payload::Name(p), wide_sp(10, 11), &[]);
    let rhs = b.add(NodeKind::Lit, Payload::LitInt(1), wide_sp(12, 13), &[]);
    let assign = b.add(
        NodeKind::Assign,
        Payload::None,
        wide_sp(10, 13),
        &[lhs, rhs],
    );
    let callee = b.add(NodeKind::Var, Payload::Name(p), wide_sp(30, 31), &[]);
    let call = b.add(NodeKind::Call, Payload::None, wide_sp(31, 32), &[callee]);
    let ret = b.add(NodeKind::Return, Payload::None, wide_sp(31, 33), &[call]);
    let body = b.add(
        NodeKind::Block,
        Payload::None,
        wide_sp(9, 40),
        &[assign, ret],
    );
    let func = b.add(NodeKind::Func, Payload::None, wide_sp(8, 50), &[body]);
    let module = b.add(NodeKind::Module, Payload::None, wide_sp(0, 60), &[func]);
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

    assert_eq!(call_target_evidence_at_call(&il, &interner, call), None);
    assert!(!il.evidence.iter().any(|record| {
        record.anchor == EvidenceAnchor::node(wide_sp(30, 31), NodeKind::Var)
            && matches!(record.kind, EvidenceKind::Symbol(_))
    }));
}

#[test]
fn does_not_emit_imported_function_target_when_parameter_shadows_alias() {
    let interner = Interner::new();
    let p = interner.intern("p");
    let mut b = IlBuilder::new(FileId(0));
    let param = b.add(NodeKind::Param, Payload::Cid(0), wide_sp(10, 11), &[]);
    let callee = b.add(NodeKind::Var, Payload::Cid(0), wide_sp(30, 31), &[]);
    let call = b.add(NodeKind::Call, Payload::None, wide_sp(31, 32), &[callee]);
    let ret = b.add(NodeKind::Return, Payload::None, wide_sp(31, 33), &[call]);
    let body = b.add(NodeKind::Block, Payload::None, wide_sp(20, 40), &[ret]);
    let func = b.add(
        NodeKind::Func,
        Payload::None,
        wide_sp(8, 50),
        &[param, body],
    );
    let module = b.add(NodeKind::Module, Payload::None, wide_sp(0, 60), &[func]);
    let mut il = b.finish(
        module,
        FileMeta {
            path: "t".into(),
            lang: Lang::Python,
        },
        Vec::new(),
        Vec::new(),
    );
    il.cid_names = vec![p];
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

    assert_eq!(call_target_evidence_at_call(&il, &interner, call), None);
    assert!(!il.evidence.iter().any(|record| {
        record.anchor == EvidenceAnchor::node(wide_sp(30, 31), NodeKind::Var)
            && matches!(record.kind, EvidenceKind::Symbol(_))
    }));
}

#[test]
fn symbol_evidence_suppresses_direct_function_raw_name_fallback() {
    let interner = Interner::new();
    let (mut il, func, call) = function_with_call(&interner, "f", "f", false);
    il.evidence.push(binding_symbol(
        0,
        sp(1),
        "f",
        SymbolEvidenceKind::ImportedBinding {
            module_hash: stable_symbol_hash("other"),
            exported_hash: stable_symbol_hash("f"),
        },
        EvidenceStatus::Asserted,
    ));

    run(&mut il, &interner);

    assert!(!direct_function_call_target_at_call(
        &il, &interner, call, func
    ));
    assert_eq!(call_target_evidence_at_call(&il, &interner, call), None);
}

#[test]
fn does_not_emit_scoped_member_target_without_imported_root_proof() {
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

    run(&mut il, &interner);

    assert_eq!(call_target_evidence_at_call(&il, &interner, call), None);
}

#[test]
fn does_not_emit_scoped_member_target_for_language_roots() {
    let interner = Interner::new();
    let var_os = interner.intern("std::env::var_os");
    let mut b = IlBuilder::new(FileId(0));
    let callee = b.add(NodeKind::Var, Payload::Name(var_os), sp(10), &[]);
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
        "std",
        SymbolEvidenceKind::ImportedNamespace {
            module_hash: stable_symbol_hash("std"),
        },
        EvidenceStatus::Asserted,
    ));

    run(&mut il, &interner);

    assert_eq!(call_target_evidence_at_call(&il, &interner, call), None);
}
