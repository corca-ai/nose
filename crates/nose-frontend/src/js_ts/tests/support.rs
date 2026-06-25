use nose_il::{
    stable_symbol_hash, EvidenceAnchor, EvidenceKind, EvidenceStatus, FileId, GuardEvidenceKind,
    Il, Interner, Lang, LibraryApiEvidenceKind, NodeId, NodeKind, Op, Payload, SequenceSurfaceKind,
    SourceBindingKind, SourceFactKind, SourceOperatorKind, Span, SymbolEvidenceKind, UnitKind,
};

pub(super) fn lower_js(src: &str) -> Il {
    let interner = Interner::new();
    crate::lower_source(
        FileId(0),
        "t.js",
        src.as_bytes(),
        Lang::JavaScript,
        &interner,
    )
    .expect("lower js")
}

pub(super) fn lower_ts_with_interner(src: &str) -> (Il, Interner) {
    let interner = Interner::new();
    let il = crate::lower_source(
        FileId(0),
        "t.ts",
        src.as_bytes(),
        Lang::TypeScript,
        &interner,
    )
    .expect("lower ts");
    (il, interner)
}

pub(super) fn raw_names(il: &Il, interner: &Interner) -> Vec<String> {
    let mut out = Vec::new();
    for node in &il.nodes {
        if node.kind != NodeKind::Raw {
            continue;
        }
        let Payload::Name(name) = node.payload else {
            continue;
        };
        out.push(interner.resolve(name).to_string());
    }
    out
}

pub(super) fn seq_names(il: &Il, interner: &Interner) -> Vec<String> {
    let mut out = Vec::new();
    for node in &il.nodes {
        if node.kind != NodeKind::Seq {
            continue;
        }
        let Payload::Name(name) = node.payload else {
            continue;
        };
        out.push(interner.resolve(name).to_string());
    }
    out
}

pub(super) fn sequence_surface_count_for_seq_name(
    il: &Il,
    interner: &Interner,
    tag: &str,
    surface: SequenceSurfaceKind,
) -> usize {
    il.nodes
        .iter()
        .filter(|node| {
            node.kind == NodeKind::Seq
                && matches!(node.payload, Payload::Name(name) if interner.resolve(name) == tag)
        })
        .map(|node| EvidenceAnchor::sequence(node.span))
        .map(|anchor| {
            il.evidence
                .iter()
                .filter(|record| {
                    record.anchor == anchor
                        && record.kind == EvidenceKind::SequenceSurface(surface)
                        && record.status == EvidenceStatus::Asserted
                })
                .count()
        })
        .sum()
}

pub(super) fn unit_root_seq_names(il: &Il, interner: &Interner, kind: UnitKind) -> Vec<String> {
    il.units
        .iter()
        .filter(|unit| unit.kind == kind)
        .filter_map(|unit| {
            let node = il.node(unit.root);
            if node.kind != NodeKind::Seq {
                return None;
            }
            let Payload::Name(name) = node.payload else {
                return None;
            };
            Some(interner.resolve(name).to_string())
        })
        .collect()
}

pub(super) fn source_binding_count(il: &Il, binding: SourceBindingKind) -> usize {
    il.evidence
        .iter()
        .filter(|record| {
            matches!(
                record.kind,
                EvidenceKind::Source(SourceFactKind::Binding(actual)) if actual == binding
            )
        })
        .count()
}

pub(super) fn unshadowed_global_evidence_count(il: &Il, name: &str) -> usize {
    let expected = stable_symbol_hash(name);
    il.evidence
        .iter()
        .filter(|record| {
            matches!(
                record.kind,
                EvidenceKind::Symbol(SymbolEvidenceKind::UnshadowedGlobal { name_hash })
                    if name_hash == expected
            )
        })
        .count()
}

pub(super) fn qualified_global_evidence_count(il: &Il, path: &str, kind: NodeKind) -> usize {
    qualified_global_evidence_records(il, path, kind).len()
}

pub(super) fn qualified_global_evidence_records<'a>(
    il: &'a Il,
    path: &str,
    kind: NodeKind,
) -> Vec<&'a nose_il::EvidenceRecord> {
    let expected = stable_symbol_hash(path);
    il.evidence
        .iter()
        .filter(|record| {
            matches!(
                record.anchor,
                EvidenceAnchor::Node {
                    kind: anchor_kind,
                    ..
                } if anchor_kind == kind
            ) && matches!(
                record.kind,
                EvidenceKind::Symbol(SymbolEvidenceKind::QualifiedGlobal { path_hash })
                    if path_hash == expected
            )
        })
        .collect()
}

pub(super) fn evidence_anchor_span(anchor: EvidenceAnchor) -> Span {
    match anchor {
        EvidenceAnchor::SourceSpan(span)
        | EvidenceAnchor::Node { span, .. }
        | EvidenceAnchor::Param { span }
        | EvidenceAnchor::Binding { span, .. }
        | EvidenceAnchor::Sequence { span } => span,
    }
}

pub(super) fn evidence_by_id(il: &Il, id: nose_il::EvidenceId) -> Option<&nose_il::EvidenceRecord> {
    il.evidence
        .get(id.0 as usize)
        .filter(|record| record.id == id)
        .or_else(|| il.evidence.iter().find(|record| record.id == id))
}

pub(super) fn record_has_source_unshadowed_dependency(
    il: &Il,
    record: &nose_il::EvidenceRecord,
    root: &str,
) -> bool {
    let span = evidence_anchor_span(record.anchor);
    let expected = stable_symbol_hash(root);
    record.dependencies.iter().any(|&id| {
        evidence_by_id(il, id).is_some_and(|dependency| {
            dependency.anchor == EvidenceAnchor::source_span(span)
                && matches!(
                    dependency.kind,
                    EvidenceKind::Symbol(SymbolEvidenceKind::UnshadowedGlobal { name_hash })
                        if name_hash == expected
                )
        })
    })
}

pub(super) fn record_has_qualified_global_dependency_with_root(
    il: &Il,
    record: &nose_il::EvidenceRecord,
    path: &str,
    root: &str,
) -> bool {
    let expected = stable_symbol_hash(path);
    record.dependencies.iter().any(|&id| {
        evidence_by_id(il, id).is_some_and(|dependency| {
            matches!(
                dependency.kind,
                EvidenceKind::Symbol(SymbolEvidenceKind::QualifiedGlobal { path_hash })
                    if path_hash == expected
            ) && record_has_source_unshadowed_dependency(il, dependency, root)
        })
    })
}

pub(super) fn source_operator_evidence_count(il: &Il, operator: SourceOperatorKind) -> usize {
    il.evidence
        .iter()
        .filter(|record| {
            matches!(
                record.kind,
                EvidenceKind::Source(SourceFactKind::Operator(actual)) if actual == operator
            )
        })
        .count()
}

pub(super) fn library_api_evidence_count(
    il: &Il,
    contract_hash: u64,
    callee_hash: u64,
    arity: u16,
) -> usize {
    il.evidence
        .iter()
        .filter(|record| {
            matches!(
                record.kind,
                EvidenceKind::LibraryApi(LibraryApiEvidenceKind::Contract {
                    contract_hash: actual_contract,
                    callee_hash: actual_callee,
                    arity: actual_arity,
                }) if actual_contract == contract_hash
                    && actual_callee == callee_hash
                    && actual_arity == arity
            )
        })
        .count()
}

pub(super) fn library_api_dependency_counts(il: &Il) -> Vec<usize> {
    il.evidence
        .iter()
        .filter_map(|record| {
            matches!(record.kind, EvidenceKind::LibraryApi(_)).then_some(record.dependencies.len())
        })
        .collect()
}

pub(super) fn library_api_dependency_counts_for(
    il: &Il,
    contract_hash: u64,
    callee_hash: u64,
    arity: u16,
) -> Vec<usize> {
    il.evidence
        .iter()
        .filter_map(|record| {
            matches!(
                record.kind,
                EvidenceKind::LibraryApi(LibraryApiEvidenceKind::Contract {
                    contract_hash: actual_contract,
                    callee_hash: actual_callee,
                    arity: actual_arity,
                }) if actual_contract == contract_hash
                    && actual_callee == callee_hash
                    && actual_arity == arity
            )
            .then_some(record.dependencies.len())
        })
        .collect()
}

pub(super) fn js_record_shape_guard_evidence(il: &Il) -> Vec<&nose_il::EvidenceRecord> {
    il.evidence
        .iter()
        .filter(|record| {
            matches!(
                record.kind,
                EvidenceKind::Guard(GuardEvidenceKind::JsRecordShape { .. })
            )
        })
        .collect()
}

pub(super) fn js_own_property_guard_evidence(il: &Il) -> Vec<&nose_il::EvidenceRecord> {
    il.evidence
        .iter()
        .filter(|record| {
            matches!(
                record.kind,
                EvidenceKind::Guard(GuardEvidenceKind::JsOwnProperty { .. })
            )
        })
        .collect()
}

pub(super) fn switch_labels_for_return(src: &str, expected_return: i64) -> Vec<i64> {
    let interner = Interner::new();
    let il = crate::lower_source(
        FileId(0),
        "t.js",
        src.as_bytes(),
        Lang::JavaScript,
        &interner,
    )
    .expect("lower js");
    let mut out = Vec::new();
    for (idx, node) in il.nodes.iter().enumerate() {
        if node.kind != NodeKind::If {
            continue;
        }
        let kids = il.children(NodeId(idx as u32));
        if kids.len() >= 2 && block_contains_return_int(&il, kids[1], expected_return) {
            collect_eq_rhs_ints(&il, kids[0], &mut out);
        }
    }
    out.sort_unstable();
    out
}

pub(super) fn block_contains_return_int(il: &Il, node: NodeId, expected: i64) -> bool {
    match il.kind(node) {
        NodeKind::Block => il
            .children(node)
            .iter()
            .any(|&child| block_contains_return_int(il, child, expected)),
        NodeKind::Return => il.children(node).first().is_some_and(
            |&expr| matches!(il.node(expr).payload, Payload::LitInt(v) if v == expected),
        ),
        _ => false,
    }
}

pub(super) fn collect_eq_rhs_ints(il: &Il, node: NodeId, out: &mut Vec<i64>) {
    if il.kind(node) != NodeKind::BinOp {
        return;
    }
    let kids = il.children(node);
    match il.node(node).payload {
        Payload::Op(Op::Or) if kids.len() == 2 => {
            collect_eq_rhs_ints(il, kids[0], out);
            collect_eq_rhs_ints(il, kids[1], out);
        }
        Payload::Op(Op::Eq) if kids.len() == 2 => {
            if let Payload::LitInt(value) = il.node(kids[1]).payload {
                out.push(value);
            }
        }
        _ => {}
    }
}
