use super::super::*;
pub(super) use nose_il::{
    stable_symbol_hash, CallTargetEvidenceKind, EvidenceAnchor, EvidenceId, EvidenceKind,
    EvidenceRecord, EvidenceStatus, FileId, FileMeta, IlBuilder, Lang, Span, Unit, UnitKind,
};
use nose_normalize::{normalize, NormalizeOptions};
use nose_semantics::test_support::{
    library_api_test_evidence_with_dependencies,
    method_call_library_api_test_evidence_with_dependencies, LibraryApiTestContract,
};
use nose_semantics::{
    language_core_evidence_provenance, library_map_get_contract, BUILTIN_COMPAT_PACK_ID,
    MAP_GET_PROTOCOL_PACK_ID, MAP_GET_PROTOCOL_PRODUCER_ID,
};

pub(super) fn sp(line: u32) -> Span {
    Span::new(FileId(0), line, line, line, line)
}

pub(super) fn normalized_python(src: &str, interner: &Interner) -> Il {
    let raw =
        nose_frontend::lower_source(FileId(0), "t.py", src.as_bytes(), Lang::Python, interner)
            .expect("lower python source");
    normalize(&raw, interner, &NormalizeOptions::default())
}

pub(super) fn normalized_swift(src: &str, interner: &Interner) -> Il {
    let raw =
        nose_frontend::lower_source(FileId(0), "t.swift", src.as_bytes(), Lang::Swift, interner)
            .expect("lower Swift source");
    normalize(&raw, interner, &NormalizeOptions::default())
}

pub(super) fn first_call_with_target(
    il: &Il,
    interner: &Interner,
    target_matches: impl Fn(CallTargetEvidenceKind) -> bool,
) -> NodeId {
    il.nodes
        .iter()
        .enumerate()
        .find_map(|(idx, node)| {
            if node.kind != NodeKind::Call {
                return None;
            }
            let call = NodeId(idx as u32);
            matches!(
                call_target_evidence_status_at_call(il, interner, call),
                CallTargetEvidenceStatus::Admitted(target) if target_matches(target)
            )
            .then_some(call)
        })
        .expect("admitted call-target call")
}

pub(super) fn evidence(
    id: u32,
    anchor: EvidenceAnchor,
    kind: EvidenceKind,
    dependencies: Vec<EvidenceId>,
) -> EvidenceRecord {
    EvidenceRecord::builtin(
        EvidenceId(id),
        anchor,
        kind,
        BUILTIN_COMPAT_PACK_ID,
        "strict-exact-test",
        dependencies,
        EvidenceStatus::Asserted,
    )
}

pub(super) fn language_core_evidence(
    id: u32,
    lang: Lang,
    anchor: EvidenceAnchor,
    kind: EvidenceKind,
    dependencies: Vec<EvidenceId>,
) -> EvidenceRecord {
    let mut record = evidence(id, anchor, kind, dependencies);
    let (pack_id, producer_id) = language_core_evidence_provenance(lang);
    record.provenance.pack_hash = Some(stable_symbol_hash(pack_id));
    record.provenance.rule_hash = Some(stable_symbol_hash(producer_id));
    record
}

pub(super) fn method_call_library_api_evidence(
    id: u32,
    lang: Lang,
    method: &str,
    call_span: Span,
    arity: usize,
    dependencies: Vec<EvidenceId>,
) -> EvidenceRecord {
    method_call_library_api_test_evidence_with_dependencies(
        id,
        lang,
        method,
        call_span,
        arity,
        dependencies,
    )
}

pub(super) fn map_get_library_api_evidence(
    id: u32,
    lang: Lang,
    method: &str,
    call_span: Span,
    dependencies: Vec<EvidenceId>,
) -> EvidenceRecord {
    let contract = library_map_get_contract(lang, method, 1).expect("map get contract");
    let mut record = library_api_test_evidence_with_dependencies(
        id,
        call_span,
        LibraryApiTestContract {
            id: contract.id,
            callee: contract.callee,
            arity: 1,
        },
        EvidenceStatus::Asserted,
        dependencies,
        (BUILTIN_COMPAT_PACK_ID, "strict-exact-test"),
    );
    record.provenance.pack_hash = Some(stable_symbol_hash(MAP_GET_PROTOCOL_PACK_ID));
    record.provenance.rule_hash = Some(stable_symbol_hash(MAP_GET_PROTOCOL_PRODUCER_ID));
    record
}

pub(super) fn call_target_evidence(
    id: u32,
    lang: Lang,
    call_span: Span,
    target: CallTargetEvidenceKind,
    dependencies: Vec<EvidenceId>,
) -> EvidenceRecord {
    language_core_evidence(
        id,
        lang,
        EvidenceAnchor::node(call_span, NodeKind::Call),
        EvidenceKind::CallTarget(target),
        dependencies,
    )
}
