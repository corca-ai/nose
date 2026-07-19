use super::{SemanticPackDependencyEvidenceId, SemanticPackOccurrenceEvidence};
use crate::{
    asserted_language_core_record, imported_binding_symbol,
    imported_occurrence_symbol_dependencies_valid, SemanticPackV1ArityKind,
    SemanticPackV1CallShape, SemanticPackV1Contract, SemanticPackV1ImportRole,
    SemanticPackV1Language,
};
use nose_il::{
    stable_symbol_hash, Corpus, EvidenceAnchor, EvidenceEmitter, EvidenceId, EvidenceKind,
    EvidenceRecord, EvidenceStatus, Il, ImportEvidenceKind, Interner, Lang, NodeId, NodeKind,
    Payload, SymbolEvidenceKind,
};

pub(super) fn occurrences_for_contract(
    pack_id: &str,
    contract: &SemanticPackV1Contract,
    dependency: SemanticPackDependencyEvidenceId,
    corpus: &Corpus,
) -> Vec<SemanticPackOccurrenceEvidence> {
    let mut occurrences = Vec::new();
    for il in &corpus.files {
        if !language_matches(contract.language, il.meta.lang) {
            continue;
        }
        for index in 0..il.nodes.len() {
            let call = NodeId(index as u32);
            if let Some(occurrence) =
                occurrence_at_call(pack_id, contract, dependency, il, &corpus.interner, call)
            {
                occurrences.push(occurrence);
            }
        }
    }
    occurrences
}

fn occurrence_at_call(
    pack_id: &str,
    contract: &SemanticPackV1Contract,
    dependency: SemanticPackDependencyEvidenceId,
    il: &Il,
    interner: &Interner,
    call: NodeId,
) -> Option<SemanticPackOccurrenceEvidence> {
    if il.kind(call) != NodeKind::Call {
        return None;
    }
    let (&callee, args) = il.children(call).split_first()?;
    if args.len() > u16::MAX as usize || !arity_matches(contract, args.len() as u16) {
        return None;
    }
    let imported = match (contract.import.role, contract.call.shape) {
        (SemanticPackV1ImportRole::Type, SemanticPackV1CallShape::StaticMethod) => {
            imported_type_callee(il, interner, callee, contract)?
        }
        (SemanticPackV1ImportRole::StaticMember, SemanticPackV1CallShape::FreeFunction) => {
            imported_static_member_callee(il, interner, callee, contract)?
        }
        _ => return None,
    };
    let call_span = il.node(call).span;
    Some(SemanticPackOccurrenceEvidence {
        pack_id: pack_id.to_string(),
        row_id: contract.id.clone(),
        channel: contract.channel,
        call_span,
        arity: args.len() as u16,
        dependency,
        import_evidence: imported.import,
        symbol_evidence: imported.symbol,
        receiver_evidence: imported.receiver,
        receiver_span: imported.receiver_span,
        effect_evidence: builtin_evidence_ids_at_call(il, call, EvidenceClass::Effect),
        call_target_evidence: builtin_evidence_ids_at_call(il, call, EvidenceClass::CallTarget),
        domain_evidence: builtin_evidence_ids_at_call(il, call, EvidenceClass::Domain),
        place_evidence: builtin_evidence_ids_at_call(il, call, EvidenceClass::Place),
    })
}

struct ImportedProof {
    import: EvidenceId,
    symbol: EvidenceId,
    receiver: Option<EvidenceId>,
    receiver_span: Option<nose_il::Span>,
}

fn imported_type_callee(
    il: &Il,
    interner: &Interner,
    callee: NodeId,
    contract: &SemanticPackV1Contract,
) -> Option<ImportedProof> {
    if il.kind(callee) != NodeKind::Field
        || !matches!(il.node(callee).payload, Payload::Name(member) if interner.resolve(member) == contract.call.member)
    {
        return None;
    }
    let receiver = il.children(callee).first().copied()?;
    let proof = imported_binding_proof(
        il,
        interner,
        receiver,
        &contract.import.module,
        &contract.import.name,
    )?;
    Some(ImportedProof {
        import: proof.import,
        symbol: proof.symbol,
        receiver: Some(proof.symbol),
        receiver_span: Some(il.node(receiver).span),
    })
}

fn imported_static_member_callee(
    il: &Il,
    interner: &Interner,
    callee: NodeId,
    contract: &SemanticPackV1Contract,
) -> Option<ImportedProof> {
    if il.kind(callee) != NodeKind::Var || contract.call.member != contract.import.name {
        return None;
    }
    let proof = imported_binding_proof(
        il,
        interner,
        callee,
        &contract.import.module,
        &contract.import.name,
    )?;
    Some(ImportedProof {
        import: proof.import,
        symbol: proof.symbol,
        receiver: None,
        receiver_span: None,
    })
}

fn imported_binding_proof(
    il: &Il,
    interner: &Interner,
    occurrence: NodeId,
    module: &str,
    exported: &str,
) -> Option<ImportedProof> {
    if !imported_binding_symbol(il, interner, occurrence, module, exported) {
        return None;
    }
    let expected = SymbolEvidenceKind::ImportedBinding {
        module_hash: stable_symbol_hash(module),
        exported_hash: stable_symbol_hash(exported),
    };
    let occurrence_record = unique_record(
        il.evidence_anchored_at(il.node(occurrence).span)
            .filter(|record| {
                record.anchor == EvidenceAnchor::node(il.node(occurrence).span, NodeKind::Var)
                    && record.kind == EvidenceKind::Symbol(expected)
            }),
    );
    if let Some(record) = occurrence_record {
        if !asserted_language_core_record(il, record)
            || !imported_occurrence_symbol_dependencies_valid(il, interner, record, expected)
        {
            return None;
        }
    }
    let local_hash = local_name_hash(il, interner, occurrence)?;
    let binding = unique_record(il.evidence.iter().filter(|record| {
        matches!(
            record.anchor,
            EvidenceAnchor::Binding {
                local_hash: actual,
                ..
            } if actual == local_hash
        ) && record.kind == EvidenceKind::Symbol(expected)
    }))?;
    if !asserted_language_core_record(il, binding) {
        return None;
    }
    let EvidenceAnchor::Binding { span, .. } = binding.anchor else {
        return None;
    };
    let import_kind = EvidenceKind::Import(ImportEvidenceKind::Binding {
        module_hash: stable_symbol_hash(module),
        exported_hash: stable_symbol_hash(exported),
    });
    let import = unique_record(il.evidence_anchored_at(span).filter(|record| {
        record.anchor == EvidenceAnchor::binding(span, local_hash) && record.kind == import_kind
    }))?;
    if !asserted_language_core_record(il, import) {
        return None;
    }
    Some(ImportedProof {
        import: import.id,
        symbol: occurrence_record.unwrap_or(binding).id,
        receiver: None,
        receiver_span: None,
    })
}

fn unique_record<'a>(
    records: impl Iterator<Item = &'a EvidenceRecord>,
) -> Option<&'a EvidenceRecord> {
    let mut records = records;
    let first = records.next()?;
    records.next().is_none().then_some(first)
}

fn local_name_hash(il: &Il, interner: &Interner, node: NodeId) -> Option<u64> {
    let Payload::Name(name) = il.node(node).payload else {
        return None;
    };
    Some(stable_symbol_hash(interner.resolve(name)))
}

fn arity_matches(contract: &SemanticPackV1Contract, arity: u16) -> bool {
    let Ok(arity) = u8::try_from(arity) else {
        return false;
    };
    match contract.call.arity.kind {
        SemanticPackV1ArityKind::Range => {
            contract.call.arity.min.is_some_and(|min| arity >= min)
                && contract.call.arity.max.is_some_and(|max| arity <= max)
        }
        SemanticPackV1ArityKind::Set => contract.call.arity.values.binary_search(&arity).is_ok(),
    }
}

#[derive(Clone, Copy)]
enum EvidenceClass {
    Effect,
    CallTarget,
    Domain,
    Place,
}

fn builtin_evidence_ids_at_call(il: &Il, call: NodeId, class: EvidenceClass) -> Vec<EvidenceId> {
    let span = il.node(call).span;
    il.evidence_anchored_at(span)
        .filter(|record| {
            record.anchor == EvidenceAnchor::node(span, NodeKind::Call)
                && record.status == EvidenceStatus::Asserted
                && record.provenance.emitter == EvidenceEmitter::Builtin
                && il.evidence_dependencies_asserted(record)
                && match class {
                    EvidenceClass::Effect => matches!(record.kind, EvidenceKind::Effect(_)),
                    EvidenceClass::CallTarget => {
                        matches!(record.kind, EvidenceKind::CallTarget(_))
                    }
                    EvidenceClass::Domain => matches!(record.kind, EvidenceKind::Domain(_)),
                    EvidenceClass::Place => matches!(record.kind, EvidenceKind::Place(_)),
                }
        })
        .map(|record| record.id)
        .collect()
}

fn language_matches(language: SemanticPackV1Language, lang: Lang) -> bool {
    matches!((language, lang), (SemanticPackV1Language::Java, Lang::Java))
}
