pub(super) use super::super::*;
pub(super) use nose_il::stable_symbol_hash;
pub(super) use nose_il::{
    DomainEvidence, EvidenceAnchor, EvidenceEmitter, EvidenceId, EvidenceKind, EvidenceProvenance,
    EvidenceRecord, EvidenceStatus, FileId, FileMeta, IlBuilder, ImportEvidenceKind, Lang,
    LibraryApiEvidenceKind, ParamSemantic, SequenceSurfaceKind, Span, SymbolEvidenceKind, Unit,
    UnitKind,
};
pub(super) use nose_semantics::{
    language_core_evidence_provenance, library_free_function_builtin_contract,
    library_free_name_map_factory_contract, library_receiver_method_api_contracts,
    FREE_FUNCTION_BUILTIN_PROTOCOL_PACK_ID, FREE_FUNCTION_BUILTIN_PROTOCOL_PRODUCER_ID,
};

pub(super) fn sp() -> Span {
    Span::new(FileId(0), 1, 1, 1, 1)
}

pub(super) fn sp_at(start: u32, end: u32, line: u32) -> Span {
    Span::new(FileId(0), start, end, line, line)
}

pub(super) fn evidence(
    id: u32,
    anchor: EvidenceAnchor,
    kind: EvidenceKind,
    status: EvidenceStatus,
) -> EvidenceRecord {
    evidence_with_dependencies(id, anchor, kind, status, Vec::new())
}

pub(super) fn evidence_with_dependencies(
    id: u32,
    anchor: EvidenceAnchor,
    kind: EvidenceKind,
    status: EvidenceStatus,
    dependencies: Vec<EvidenceId>,
) -> EvidenceRecord {
    EvidenceRecord {
        id: EvidenceId(id),
        anchor,
        kind,
        provenance: EvidenceProvenance {
            emitter: EvidenceEmitter::Builtin,
            pack_hash: Some(stable_symbol_hash(nose_semantics::BUILTIN_COMPAT_PACK_ID)),
            rule_hash: Some(stable_symbol_hash("test")),
        },
        dependencies,
        status,
    }
}

pub(super) fn language_core_evidence(
    id: u32,
    lang: Lang,
    anchor: EvidenceAnchor,
    kind: EvidenceKind,
    status: EvidenceStatus,
) -> EvidenceRecord {
    let (pack_id, producer_id) = language_core_evidence_provenance(lang);
    EvidenceRecord {
        id: EvidenceId(id),
        anchor,
        kind,
        provenance: EvidenceProvenance {
            emitter: EvidenceEmitter::Builtin,
            pack_hash: Some(stable_symbol_hash(pack_id)),
            rule_hash: Some(stable_symbol_hash(producer_id)),
        },
        dependencies: Vec::new(),
        status,
    }
}

pub(super) fn language_core_symbol_evidence(
    id: u32,
    lang: Lang,
    anchor: EvidenceAnchor,
    symbol: SymbolEvidenceKind,
    status: EvidenceStatus,
) -> EvidenceRecord {
    language_core_evidence(id, lang, anchor, EvidenceKind::Symbol(symbol), status)
}

pub(super) fn sequence_surface_evidence(
    id: u32,
    lang: Lang,
    span: Span,
    surface: SequenceSurfaceKind,
) -> EvidenceRecord {
    let (pack_id, producer_id) = language_core_evidence_provenance(lang);
    EvidenceRecord {
        id: EvidenceId(id),
        anchor: EvidenceAnchor::sequence(span),
        kind: EvidenceKind::SequenceSurface(surface),
        provenance: EvidenceProvenance {
            emitter: EvidenceEmitter::Builtin,
            pack_hash: Some(stable_symbol_hash(pack_id)),
            rule_hash: Some(stable_symbol_hash(producer_id)),
        },
        dependencies: Vec::new(),
        status: EvidenceStatus::Asserted,
    }
}

pub(super) fn next_evidence_id(il: &Il) -> u32 {
    il.evidence.len() as u32
}

pub(super) fn method_call_receiver(il: &Il, call: NodeId) -> Option<NodeId> {
    let callee = *il.children(call).first()?;
    (il.kind(callee) == NodeKind::Field)
        .then(|| il.children(callee).first().copied())
        .flatten()
}

pub(super) fn push_sequence_surface_evidence(
    il: &mut Il,
    node: NodeId,
    surface: SequenceSurfaceKind,
) -> EvidenceId {
    let id = next_evidence_id(il);
    il.evidence.push(sequence_surface_evidence(
        id,
        il.meta.lang,
        il.node(node).span,
        surface,
    ));
    EvidenceId(id)
}

pub(super) fn push_receiver_sequence_surface_evidence(
    il: &mut Il,
    call: NodeId,
    surface: SequenceSurfaceKind,
) -> EvidenceId {
    let receiver = method_call_receiver(il, call).expect("method receiver");
    push_sequence_surface_evidence(il, receiver, surface)
}

pub(super) fn push_imported_namespace_use(
    il: &mut Il,
    binding_id: u32,
    occurrence_id: u32,
    occurrence_node: NodeId,
    module: &str,
) {
    let symbol = SymbolEvidenceKind::ImportedNamespace {
        module_hash: stable_symbol_hash(module),
    };
    il.evidence.push(language_core_symbol_evidence(
        binding_id,
        il.meta.lang,
        EvidenceAnchor::binding(sp(), stable_symbol_hash(module)),
        symbol,
        EvidenceStatus::Asserted,
    ));
    let mut occurrence = language_core_symbol_evidence(
        occurrence_id,
        il.meta.lang,
        EvidenceAnchor::node(il.node(occurrence_node).span, NodeKind::Var),
        symbol,
        EvidenceStatus::Asserted,
    );
    occurrence.dependencies = vec![EvidenceId(binding_id)];
    il.evidence.push(occurrence);
}

pub(super) fn push_receiver_method_library_api_evidence(
    il: &mut Il,
    interner: &Interner,
    call: NodeId,
) -> Option<EvidenceId> {
    let kids = il.children(call);
    let (&callee, args) = kids.split_first()?;
    if il.kind(callee) != NodeKind::Field {
        return None;
    }
    let Payload::Name(method) = il.node(callee).payload else {
        return None;
    };
    let method = interner.resolve(method);
    let arg_count = args.len();
    let (contract, dependencies) =
        library_receiver_method_api_contracts(il.meta.lang, method, arg_count)
            .into_iter()
            .find_map(|contract| {
                let dependencies = nose_semantics::library_api_receiver_dependencies_for_call(
                    il,
                    interner,
                    call,
                    contract.callee,
                )?;
                Some((contract, dependencies))
            })?;
    let id = next_evidence_id(il);
    let mut record = evidence_with_dependencies(
        id,
        EvidenceAnchor::node(il.node(call).span, NodeKind::Call),
        EvidenceKind::LibraryApi(LibraryApiEvidenceKind::Contract {
            contract_hash: nose_semantics::library_api_contract_id_hash(contract.id),
            callee_hash: nose_semantics::library_api_callee_contract_hash(contract.callee),
            arity: arg_count as u16,
        }),
        EvidenceStatus::Asserted,
        dependencies,
    );
    record.provenance.pack_hash = Some(stable_symbol_hash(contract.pack_id));
    record.provenance.rule_hash = Some(stable_symbol_hash(contract.rule));
    il.evidence.push(record);
    Some(EvidenceId(id))
}

pub(super) fn push_free_function_builtin_library_api_evidence(
    il: &mut Il,
    interner: &Interner,
    call: NodeId,
) -> Option<EvidenceId> {
    let kids = il.children(call);
    let (&callee, args) = kids.split_first()?;
    let arg_count = args.len();
    let callee_span = il.node(callee).span;
    let call_span = il.node(call).span;
    let Payload::Name(symbol) = il.node(callee).payload else {
        return None;
    };
    let name = interner.resolve(symbol);
    let contract = library_free_function_builtin_contract(il.meta.lang, name, arg_count)?;
    let symbol_id = next_evidence_id(il);
    il.evidence.push(language_core_symbol_evidence(
        symbol_id,
        il.meta.lang,
        EvidenceAnchor::node(callee_span, NodeKind::Var),
        SymbolEvidenceKind::UnshadowedGlobal {
            name_hash: stable_symbol_hash(name),
        },
        EvidenceStatus::Asserted,
    ));
    let id = next_evidence_id(il);
    il.evidence.push(evidence_with_dependencies(
        id,
        EvidenceAnchor::node(call_span, NodeKind::Call),
        EvidenceKind::LibraryApi(LibraryApiEvidenceKind::Contract {
            contract_hash: nose_semantics::library_api_contract_id_hash(contract.id),
            callee_hash: nose_semantics::library_api_callee_contract_hash(contract.callee),
            arity: arg_count as u16,
        }),
        EvidenceStatus::Asserted,
        vec![EvidenceId(symbol_id)],
    ));
    let record = il.evidence.last_mut().expect("just pushed API evidence");
    record.provenance.pack_hash = Some(stable_symbol_hash(FREE_FUNCTION_BUILTIN_PROTOCOL_PACK_ID));
    record.provenance.rule_hash = Some(stable_symbol_hash(
        FREE_FUNCTION_BUILTIN_PROTOCOL_PRODUCER_ID,
    ));
    Some(EvidenceId(id))
}
