#![allow(clippy::cognitive_complexity, clippy::too_many_lines)]

use super::*;

mod core_factories;
mod core_misc;
mod param_domains;
mod post_lower_free_names;
mod post_lower_properties;
mod post_lower_python_iterator_builtins;
mod post_lower_results;

fn sp() -> Span {
    Span::new(FileId(0), 0, 1, 1, 1)
}

fn sp_at(line: u32) -> Span {
    Span::new(FileId(0), line, line + 1, line, line)
}

fn library_api_evidence_count(lo: &Lowering, contract_hash: u64, callee_hash: u64) -> usize {
    library_api_evidence_count_in_records(&lo.evidence, contract_hash, callee_hash)
}

fn library_api_evidence_count_in_records(
    evidence: &[EvidenceRecord],
    contract_hash: u64,
    callee_hash: u64,
) -> usize {
    evidence
        .iter()
        .filter(|record| {
            matches!(
                record.kind,
                EvidenceKind::LibraryApi(LibraryApiEvidenceKind::Contract {
                    contract_hash: actual_contract,
                    callee_hash: actual_callee,
                    ..
                }) if actual_contract == contract_hash && actual_callee == callee_hash
            )
        })
        .count()
}

fn library_api_evidence_ids_in_records(
    evidence: &[EvidenceRecord],
    contract_hash: u64,
    callee_hash: u64,
) -> Vec<EvidenceId> {
    evidence
        .iter()
        .filter_map(|record| {
            matches!(
                record.kind,
                EvidenceKind::LibraryApi(LibraryApiEvidenceKind::Contract {
                    contract_hash: actual_contract,
                    callee_hash: actual_callee,
                    ..
                }) if actual_contract == contract_hash && actual_callee == callee_hash
            )
            .then_some(record.id)
        })
        .collect()
}

fn library_api_evidence_ids_at(
    evidence: &[EvidenceRecord],
    span: Span,
    contract_hash: u64,
    callee_hash: u64,
    arity: u16,
) -> Vec<EvidenceId> {
    evidence
        .iter()
        .filter_map(|record| {
            (record.anchor == EvidenceAnchor::node(span, NodeKind::Call)
                && matches!(
                    record.kind,
                    EvidenceKind::LibraryApi(LibraryApiEvidenceKind::Contract {
                        contract_hash: actual_contract,
                        callee_hash: actual_callee,
                        arity: actual_arity,
                    }) if actual_contract == contract_hash
                        && actual_callee == callee_hash
                        && actual_arity == arity
                ))
            .then_some(record.id)
        })
        .collect()
}

fn library_api_evidence_ids_at_node(
    evidence: &[EvidenceRecord],
    span: Span,
    kind: NodeKind,
    contract_hash: u64,
    callee_hash: u64,
    arity: u16,
) -> Vec<EvidenceId> {
    evidence
        .iter()
        .filter_map(|record| {
            (record.anchor == EvidenceAnchor::node(span, kind)
                && matches!(
                    record.kind,
                    EvidenceKind::LibraryApi(LibraryApiEvidenceKind::Contract {
                        contract_hash: actual_contract,
                        callee_hash: actual_callee,
                        arity: actual_arity,
                    }) if actual_contract == contract_hash
                        && actual_callee == callee_hash
                        && actual_arity == arity
                ))
            .then_some(record.id)
        })
        .collect()
}

fn contract_api_count(
    evidence: &[EvidenceRecord],
    id: LibraryApiContractId,
    callee: LibraryApiCalleeContract,
) -> usize {
    contract_api_ids(evidence, id, callee).len()
}

fn contract_api_records(
    evidence: &[EvidenceRecord],
    id: LibraryApiContractId,
    callee: LibraryApiCalleeContract,
) -> Vec<&EvidenceRecord> {
    let contract_hash = library_api_contract_id_hash(id);
    let callee_hash = library_api_callee_contract_hash(callee);
    evidence
        .iter()
        .filter(|record| {
            matches!(
                record.kind,
                EvidenceKind::LibraryApi(LibraryApiEvidenceKind::Contract {
                    contract_hash: actual_contract,
                    callee_hash: actual_callee,
                    ..
                }) if actual_contract == contract_hash && actual_callee == callee_hash
            )
        })
        .collect()
}

fn contract_api_ids(
    evidence: &[EvidenceRecord],
    id: LibraryApiContractId,
    callee: LibraryApiCalleeContract,
) -> Vec<EvidenceId> {
    library_api_evidence_ids_in_records(
        evidence,
        library_api_contract_id_hash(id),
        library_api_callee_contract_hash(callee),
    )
}

fn lower_fixture(path: &str, src: &[u8], lang: Lang, interner: &Interner) -> Il {
    crate::lower_source(FileId(0), path, src, lang, interner).expect("lowering should succeed")
}

fn array_seq(lo: &mut Lowering, interner: &Interner, sp: Span) -> NodeId {
    lo.add(
        NodeKind::Seq,
        Payload::Name(interner.intern("array")),
        sp,
        &[],
    )
}

fn field_callee(
    lo: &mut Lowering,
    interner: &Interner,
    base: NodeId,
    member: &str,
    sp: Span,
) -> NodeId {
    lo.add(
        NodeKind::Field,
        Payload::Name(interner.intern(member)),
        sp,
        &[base],
    )
}

fn named_node_span(il: &Il, interner: &Interner, kind: NodeKind, name: &str) -> Option<Span> {
    il.nodes.iter().find_map(|node| {
        (node.kind == kind
            && matches!(
                node.payload,
                Payload::Name(symbol) if interner.resolve(symbol) == name
            ))
        .then_some(node.span)
    })
}

fn call_with_callee_named(il: &Il, interner: &Interner, name: &str) -> Option<NodeId> {
    il.nodes.iter().enumerate().find_map(|(idx, node)| {
        (node.kind == NodeKind::Call
            && il
                .children(NodeId(idx as u32))
                .first()
                .is_some_and(|&callee| {
                    matches!(
                        il.node(callee).payload,
                        Payload::Name(symbol) if interner.resolve(symbol) == name
                    )
                }))
        .then_some(NodeId(idx as u32))
    })
}

fn call_span_with_callee_named(il: &Il, interner: &Interner, name: &str) -> Option<Span> {
    call_with_callee_named(il, interner, name).map(|call| il.node(call).span)
}

fn call_span_with_field_callee_named(il: &Il, interner: &Interner, name: &str) -> Option<Span> {
    il.nodes.iter().enumerate().find_map(|(idx, node)| {
        (node.kind == NodeKind::Call
            && il
                .children(NodeId(idx as u32))
                .first()
                .is_some_and(|&callee| {
                    il.kind(callee) == NodeKind::Field
                        && matches!(
                            il.node(callee).payload,
                            Payload::Name(symbol) if interner.resolve(symbol) == name
                        )
                }))
        .then_some(node.span)
    })
}

fn result_domain_depends_on_api(
    evidence: &[EvidenceRecord],
    span: Span,
    domain: DomainEvidence,
    api_ids: &[EvidenceId],
) -> bool {
    evidence.iter().any(|record| {
        record.anchor == EvidenceAnchor::node(span, NodeKind::Call)
            && record.kind == EvidenceKind::Domain(domain)
            && record.dependencies.len() == 1
            && api_ids.contains(&record.dependencies[0])
    })
}

fn result_domain_depends_on_api_at_node(
    evidence: &[EvidenceRecord],
    span: Span,
    kind: NodeKind,
    domain: DomainEvidence,
    api_ids: &[EvidenceId],
) -> bool {
    evidence.iter().any(|record| {
        record.anchor == EvidenceAnchor::node(span, kind)
            && record.kind == EvidenceKind::Domain(domain)
            && record.dependencies.len() == 1
            && api_ids.contains(&record.dependencies[0])
    })
}

fn result_domain_any_count_at(evidence: &[EvidenceRecord], span: Span) -> usize {
    evidence
        .iter()
        .filter(|record| {
            record.anchor == EvidenceAnchor::node(span, NodeKind::Call)
                && matches!(record.kind, EvidenceKind::Domain(_))
        })
        .count()
}

fn result_domain_depends_on_any_api(
    evidence: &[EvidenceRecord],
    domain: DomainEvidence,
    api_ids: &[EvidenceId],
) -> bool {
    evidence.iter().any(|record| {
        matches!(record.kind, EvidenceKind::Domain(actual) if actual == domain)
            && record
                .dependencies
                .iter()
                .any(|dependency| api_ids.contains(dependency))
    })
}

fn result_domain_record_count(evidence: &[EvidenceRecord], domain: DomainEvidence) -> usize {
    evidence
        .iter()
        .filter(|record| matches!(record.kind, EvidenceKind::Domain(actual) if actual == domain))
        .filter(|record| !record.dependencies.is_empty())
        .count()
}

fn param_domain_records(
    evidence: &[EvidenceRecord],
    domain: DomainEvidence,
) -> Vec<&EvidenceRecord> {
    evidence
        .iter()
        .filter(|record| {
            matches!(record.anchor, EvidenceAnchor::Param { .. })
                && matches!(record.kind, EvidenceKind::Domain(actual) if actual == domain)
        })
        .collect()
}

fn param_domain_record_count(evidence: &[EvidenceRecord], domain: DomainEvidence) -> usize {
    param_domain_records(evidence, domain).len()
}

fn param_domain_record_count_from_pack(
    evidence: &[EvidenceRecord],
    domain: DomainEvidence,
    pack_id: &str,
) -> usize {
    let pack_hash = stable_symbol_hash(pack_id);
    param_domain_records(evidence, domain)
        .into_iter()
        .filter(|record| record.provenance.pack_hash == Some(pack_hash))
        .count()
}

fn imported_binding_symbol_ids(
    evidence: &[EvidenceRecord],
    module: &str,
    exported: &str,
) -> Vec<EvidenceId> {
    let module_hash = stable_symbol_hash(module);
    let exported_hash = stable_symbol_hash(exported);
    evidence
        .iter()
        .filter_map(|record| {
            matches!(
                record.kind,
                EvidenceKind::Symbol(SymbolEvidenceKind::ImportedBinding {
                    module_hash: actual_module,
                    exported_hash: actual_exported,
                }) if actual_module == module_hash && actual_exported == exported_hash
            )
            .then_some(record.id)
        })
        .collect()
}

fn call_node_with_result_domain(il: &Il, domain: DomainEvidence) -> Option<NodeId> {
    let span = il
        .evidence
        .iter()
        .find_map(|record| match (record.anchor, record.kind) {
            (
                EvidenceAnchor::Node {
                    span,
                    kind: NodeKind::Call,
                },
                EvidenceKind::Domain(actual),
            ) if actual == domain => Some(span),
            _ => None,
        })?;
    il.nodes.iter().enumerate().find_map(|(idx, node)| {
        (node.kind == NodeKind::Call && node.span == span).then_some(NodeId(idx as u32))
    })
}
