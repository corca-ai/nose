use super::*;

#[test]
fn qualified_global_symbol_contracts_are_language_and_path_scoped() {
    assert_eq!(
        qualified_global_symbol_contract(Lang::JavaScript, "Object.hasOwn"),
        Some(QualifiedGlobalSymbolContract {
            path: "Object.hasOwn",
            root: "Object",
            requires_unshadowed_root: true,
        })
    );
    assert_eq!(
        qualified_global_symbol_contract(Lang::TypeScript, "Array.from"),
        Some(QualifiedGlobalSymbolContract {
            path: "Array.from",
            root: "Array",
            requires_unshadowed_root: true,
        })
    );
    assert_eq!(
        qualified_global_symbol_contract(Lang::TypeScript, "Object.keys"),
        Some(QualifiedGlobalSymbolContract {
            path: "Object.keys",
            root: "Object",
            requires_unshadowed_root: true,
        })
    );
    assert!(qualified_global_symbol_contract(
        Lang::JavaScript,
        "Object.prototype.hasOwnProperty.call"
    )
    .is_some());
    assert!(qualified_global_symbol_contract(Lang::Python, "Array.from").is_none());
    assert!(qualified_global_symbol_contract(Lang::JavaScript, "value.hasOwnProperty").is_none());
    assert!(qualified_global_symbol_contract(Lang::JavaScript, "Array.fromAsync").is_none());
}

#[test]
fn qualified_global_symbol_requires_matching_node_evidence_and_root_dependency() {
    let interner = Interner::new();
    let build = || {
        let mut b = IlBuilder::new(FileId(0));
        let array = b.add(
            NodeKind::Var,
            Payload::Name(interner.intern("Array")),
            sp(27),
            &[],
        );
        let field = b.add(
            NodeKind::Field,
            Payload::Name(interner.intern("from")),
            sp(27),
            &[array],
        );
        let root = b.add(NodeKind::Module, Payload::None, sp(27), &[field]);
        (finish_il(b, root, Lang::JavaScript), field)
    };
    let (mut il, field) = build();

    assert!(!qualified_global_symbol(&il, field, "Array.from"));

    il.evidence.push(evidence(
        0,
        EvidenceAnchor::node(sp(27), NodeKind::Field),
        EvidenceKind::Symbol(SymbolEvidenceKind::QualifiedGlobal {
            path_hash: stable_symbol_hash("Array.from"),
        }),
        EvidenceStatus::Asserted,
    ));
    assert!(
        !qualified_global_symbol(&il, field, "Array.from"),
        "qualified API identity must not stand without a root proof"
    );

    let (mut il, field) = build();
    il.evidence.push(evidence_with_dependencies(
        0,
        EvidenceAnchor::node(sp(27), NodeKind::Field),
        EvidenceKind::Symbol(SymbolEvidenceKind::QualifiedGlobal {
            path_hash: stable_symbol_hash("Array.from"),
        }),
        EvidenceStatus::Asserted,
        vec![EvidenceId(1)],
    ));
    il.evidence.push(evidence(
        1,
        EvidenceAnchor::source_span(sp(27)),
        EvidenceKind::Symbol(SymbolEvidenceKind::UnshadowedGlobal {
            name_hash: stable_symbol_hash("Array"),
        }),
        EvidenceStatus::Asserted,
    ));
    assert!(qualified_global_symbol(&il, field, "Array.from"));
    assert!(qualified_global_symbol_at_span(
        &il,
        Some(sp(27)),
        NodeKind::Field,
        "Array.from"
    ));
    assert!(!qualified_global_symbol(&il, field, "Array.fromAsync"));

    il.evidence.push(evidence(
        2,
        EvidenceAnchor::node(sp(27), NodeKind::Field),
        EvidenceKind::Symbol(SymbolEvidenceKind::QualifiedGlobal {
            path_hash: stable_symbol_hash("Array.isArray"),
        }),
        EvidenceStatus::Asserted,
    ));
    assert!(!qualified_global_symbol(&il, field, "Array.from"));
}
