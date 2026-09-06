use super::*;

#[test]
fn imported_occurrence_symbol_evidence_requires_binding_dependency() {
    let interner = Interner::new();
    let mut b = IlBuilder::new(FileId(0));
    let local_hash = stable_symbol_hash("m");
    let receiver = b.add(
        NodeKind::Var,
        Payload::Name(interner.intern("m")),
        sp(20),
        &[],
    );
    let root = b.add(NodeKind::Module, Payload::None, sp(20), &[receiver]);
    let mut il = finish_il(b, root, Lang::Python);
    il.push_evidence(language_core_evidence(
        0,
        EvidenceAnchor::node(sp(20), NodeKind::Var),
        EvidenceKind::Symbol(SymbolEvidenceKind::ImportedNamespace {
            module_hash: stable_symbol_hash("math"),
        }),
        EvidenceStatus::Asserted,
        Lang::Python,
    ));

    assert!(!imported_namespace_symbol(&il, &interner, receiver, "math"));

    il.evidence.clear();
    il.push_evidence(evidence(
        0,
        EvidenceAnchor::binding(sp(19), local_hash),
        EvidenceKind::Symbol(SymbolEvidenceKind::ImportedNamespace {
            module_hash: stable_symbol_hash("math"),
        }),
        EvidenceStatus::Asserted,
    ));
    il.push_evidence(language_core_evidence_with_dependencies(
        1,
        EvidenceAnchor::node(sp(20), NodeKind::Var),
        EvidenceKind::Symbol(SymbolEvidenceKind::ImportedNamespace {
            module_hash: stable_symbol_hash("math"),
        }),
        EvidenceStatus::Asserted,
        vec![EvidenceId(0)],
        Lang::Python,
    ));

    assert!(imported_namespace_symbol(&il, &interner, receiver, "math"));
    assert!(!imported_namespace_symbol(
        &il,
        &interner,
        receiver,
        "collections"
    ));

    il.push_evidence(evidence(
        2,
        EvidenceAnchor::node(sp(20), NodeKind::Var),
        EvidenceKind::Symbol(SymbolEvidenceKind::ImportedBinding {
            module_hash: stable_symbol_hash("collections"),
            exported_hash: stable_symbol_hash("deque"),
        }),
        EvidenceStatus::Asserted,
    ));
    assert!(
        !imported_namespace_symbol(&il, &interner, receiver, "math"),
        "conflicting same-node Symbol identity must keep imported namespace proof closed"
    );

    il.evidence.clear();
    il.push_evidence(evidence(
        0,
        EvidenceAnchor::binding(sp(19), local_hash),
        EvidenceKind::Symbol(SymbolEvidenceKind::ImportedNamespace {
            module_hash: stable_symbol_hash("math"),
        }),
        EvidenceStatus::Asserted,
    ));
    il.push_evidence(evidence_with_dependencies(
        1,
        EvidenceAnchor::node(sp(20), NodeKind::Var),
        EvidenceKind::Symbol(SymbolEvidenceKind::ImportedNamespace {
            module_hash: stable_symbol_hash("math"),
        }),
        EvidenceStatus::Asserted,
        vec![EvidenceId(0)],
    ));
    assert!(
        !imported_namespace_symbol(&il, &interner, receiver, "math"),
        "legacy broad imported-namespace occurrence evidence must not prove the public symbol identity"
    );

    il.evidence.clear();
    il.push_evidence(evidence(
        0,
        EvidenceAnchor::binding(sp(19), local_hash),
        EvidenceKind::Symbol(SymbolEvidenceKind::ImportedNamespace {
            module_hash: stable_symbol_hash("math"),
        }),
        EvidenceStatus::Asserted,
    ));
    il.push_evidence(language_core_evidence_with_dependencies(
        1,
        EvidenceAnchor::node(sp(20), NodeKind::Var),
        EvidenceKind::Symbol(SymbolEvidenceKind::ImportedNamespace {
            module_hash: stable_symbol_hash("math"),
        }),
        EvidenceStatus::Asserted,
        vec![EvidenceId(0)],
        Lang::JavaScript,
    ));
    assert!(
        !imported_namespace_symbol(&il, &interner, receiver, "math"),
        "wrong-language imported-namespace occurrence evidence must not prove the public symbol identity"
    );

    il.evidence.clear();
    il.push_evidence(evidence(
        0,
        EvidenceAnchor::binding(sp(19), local_hash),
        EvidenceKind::Symbol(SymbolEvidenceKind::ImportedNamespace {
            module_hash: stable_symbol_hash("math"),
        }),
        EvidenceStatus::Asserted,
    ));
    let mut external = language_core_evidence_with_dependencies(
        1,
        EvidenceAnchor::node(sp(20), NodeKind::Var),
        EvidenceKind::Symbol(SymbolEvidenceKind::ImportedNamespace {
            module_hash: stable_symbol_hash("math"),
        }),
        EvidenceStatus::Asserted,
        vec![EvidenceId(0)],
        Lang::Python,
    );
    external.provenance.emitter = EvidenceEmitter::External;
    il.push_evidence(external);
    assert!(
        !imported_namespace_symbol(&il, &interner, receiver, "math"),
        "external imported-namespace occurrence evidence must not prove the public symbol identity"
    );

    il.evidence.clear();
    il.push_evidence(evidence(
        0,
        EvidenceAnchor::binding(sp(19), local_hash),
        EvidenceKind::Symbol(SymbolEvidenceKind::ImportedNamespace {
            module_hash: stable_symbol_hash("math"),
        }),
        EvidenceStatus::Asserted,
    ));
    let mut missing_pack = language_core_evidence_with_dependencies(
        1,
        EvidenceAnchor::node(sp(20), NodeKind::Var),
        EvidenceKind::Symbol(SymbolEvidenceKind::ImportedNamespace {
            module_hash: stable_symbol_hash("math"),
        }),
        EvidenceStatus::Asserted,
        vec![EvidenceId(0)],
        Lang::Python,
    );
    missing_pack.provenance.pack_hash = None;
    il.push_evidence(missing_pack);
    assert!(
        !imported_namespace_symbol(&il, &interner, receiver, "math"),
        "missing-pack imported-namespace occurrence evidence must not prove the public symbol identity"
    );
}

#[test]
fn symbol_evidence_blocks_import_assignment_fallback() {
    let interner = Interner::new();
    let mut b = IlBuilder::new(FileId(0));
    let local = interner.intern("math");
    let lhs = b.add(NodeKind::Var, Payload::Name(local), sp(21), &[]);
    let module = b.add(
        NodeKind::Lit,
        Payload::LitStr(stable_symbol_hash("math")),
        sp(21),
        &[],
    );
    let rhs = b.add(NodeKind::Seq, Payload::None, sp(21), &[module]);
    let assign = b.add(NodeKind::Assign, Payload::None, sp(21), &[lhs, rhs]);
    let receiver = b.add(NodeKind::Var, Payload::Name(local), sp(22), &[]);
    let root = b.add(NodeKind::Module, Payload::None, sp(21), &[assign, receiver]);
    let mut il = finish_il(b, root, Lang::Python);
    il.push_evidence(evidence(
        0,
        EvidenceAnchor::binding(sp(21), stable_symbol_hash("math")),
        EvidenceKind::Symbol(SymbolEvidenceKind::ImportedNamespace {
            module_hash: stable_symbol_hash("other"),
        }),
        EvidenceStatus::Asserted,
    ));

    assert!(!imported_namespace_symbol(&il, &interner, receiver, "math"));
}

#[test]
fn binding_symbol_evidence_does_not_prove_rebound_alias_uses() {
    let interner = Interner::new();
    let mut b = IlBuilder::new(FileId(0));
    let local = interner.intern("math");
    let lhs = b.add(NodeKind::Var, Payload::Name(local), sp(24), &[]);
    let module = b.add(
        NodeKind::Lit,
        Payload::LitStr(stable_symbol_hash("math")),
        sp(24),
        &[],
    );
    let rhs = b.add(NodeKind::Seq, Payload::None, sp(24), &[module]);
    let import_assign = b.add(NodeKind::Assign, Payload::None, sp(24), &[lhs, rhs]);
    let rebound_lhs = b.add(NodeKind::Var, Payload::Name(local), sp(25), &[]);
    let rebound_rhs = b.add(NodeKind::Lit, Payload::LitInt(0), sp(25), &[]);
    let rebound = b.add(
        NodeKind::Assign,
        Payload::None,
        sp(25),
        &[rebound_lhs, rebound_rhs],
    );
    let receiver = b.add(NodeKind::Var, Payload::Name(local), sp(26), &[]);
    let root = b.add(
        NodeKind::Module,
        Payload::None,
        sp(24),
        &[import_assign, rebound, receiver],
    );
    let mut il = finish_il(b, root, Lang::Python);
    il.push_evidence(evidence(
        0,
        EvidenceAnchor::binding(sp(24), stable_symbol_hash("math")),
        EvidenceKind::Symbol(SymbolEvidenceKind::ImportedNamespace {
            module_hash: stable_symbol_hash("math"),
        }),
        EvidenceStatus::Asserted,
    ));

    assert!(!imported_namespace_symbol(&il, &interner, receiver, "math"));
}

#[test]
fn global_symbol_requires_asserted_evidence() {
    let interner = Interner::new();
    let mut b = IlBuilder::new(FileId(0));
    let math = b.add(
        NodeKind::Var,
        Payload::Name(interner.intern("Math")),
        sp(23),
        &[],
    );
    let root = b.add(NodeKind::Module, Payload::None, sp(23), &[math]);
    let mut il = finish_il(b, root, Lang::JavaScript);

    assert!(
        !asserted_unshadowed_global_symbol(&il, math, "Math"),
        "a bare spelling without Symbol evidence must not open the exact path"
    );

    il.push_evidence(language_core_evidence(
        0,
        EvidenceAnchor::node(sp(23), NodeKind::Var),
        EvidenceKind::Symbol(SymbolEvidenceKind::UnshadowedGlobal {
            name_hash: stable_symbol_hash("Math"),
        }),
        EvidenceStatus::Ambiguous,
        Lang::JavaScript,
    ));
    assert!(
        !asserted_unshadowed_global_symbol(&il, math, "Math"),
        "ambiguous Symbol evidence keeps the exact path closed"
    );

    il.evidence.clear();
    il.push_evidence(evidence(
        0,
        EvidenceAnchor::node(sp(23), NodeKind::Var),
        EvidenceKind::Symbol(SymbolEvidenceKind::UnshadowedGlobal {
            name_hash: stable_symbol_hash("Math"),
        }),
        EvidenceStatus::Asserted,
    ));
    assert!(
        !asserted_unshadowed_global_symbol(&il, math, "Math"),
        "legacy broad Symbol evidence must not prove the unshadowed global"
    );

    il.evidence.clear();
    il.push_evidence(language_core_evidence(
        0,
        EvidenceAnchor::node(sp(23), NodeKind::Var),
        EvidenceKind::Symbol(SymbolEvidenceKind::UnshadowedGlobal {
            name_hash: stable_symbol_hash("Math"),
        }),
        EvidenceStatus::Asserted,
        Lang::Python,
    ));
    assert!(
        !asserted_unshadowed_global_symbol(&il, math, "Math"),
        "wrong-language Symbol evidence must not prove the unshadowed global"
    );

    il.evidence.clear();
    let mut external = language_core_evidence(
        0,
        EvidenceAnchor::node(sp(23), NodeKind::Var),
        EvidenceKind::Symbol(SymbolEvidenceKind::UnshadowedGlobal {
            name_hash: stable_symbol_hash("Math"),
        }),
        EvidenceStatus::Asserted,
        Lang::JavaScript,
    );
    external.provenance.emitter = EvidenceEmitter::External;
    il.push_evidence(external);
    assert!(
        !asserted_unshadowed_global_symbol(&il, math, "Math"),
        "external Symbol evidence must not prove the unshadowed global"
    );

    il.evidence.clear();
    let mut missing_pack = language_core_evidence(
        0,
        EvidenceAnchor::node(sp(23), NodeKind::Var),
        EvidenceKind::Symbol(SymbolEvidenceKind::UnshadowedGlobal {
            name_hash: stable_symbol_hash("Math"),
        }),
        EvidenceStatus::Asserted,
        Lang::JavaScript,
    );
    missing_pack.provenance.pack_hash = None;
    il.push_evidence(missing_pack);
    assert!(
        !asserted_unshadowed_global_symbol(&il, math, "Math"),
        "missing-pack Symbol evidence must not prove the unshadowed global"
    );

    il.evidence.clear();
    il.push_evidence(language_core_evidence(
        0,
        EvidenceAnchor::node(sp(23), NodeKind::Var),
        EvidenceKind::Symbol(SymbolEvidenceKind::UnshadowedGlobal {
            name_hash: stable_symbol_hash("Math"),
        }),
        EvidenceStatus::Asserted,
        Lang::JavaScript,
    ));
    assert!(
        asserted_unshadowed_global_symbol(&il, math, "Math"),
        "matching language-core Symbol evidence proves the unshadowed global"
    );
}
