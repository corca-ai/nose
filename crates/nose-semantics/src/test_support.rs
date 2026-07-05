use nose_il::{EvidenceAnchor, EvidenceId, EvidenceKind, EvidenceRecord, EvidenceStatus, Lang};

use crate::{language_core_evidence_provenance, BUILTIN_COMPAT_PACK_ID};

pub fn compat_test_evidence_with_dependencies(
    id: u32,
    anchor: EvidenceAnchor,
    kind: EvidenceKind,
    status: EvidenceStatus,
    dependencies: Vec<EvidenceId>,
) -> EvidenceRecord {
    EvidenceRecord::builtin(
        EvidenceId(id),
        anchor,
        kind,
        BUILTIN_COMPAT_PACK_ID,
        "test",
        dependencies,
        status,
    )
}

pub fn compat_test_asserted_evidence(
    id: u32,
    anchor: EvidenceAnchor,
    kind: EvidenceKind,
    dependencies: Vec<EvidenceId>,
) -> EvidenceRecord {
    compat_test_evidence_with_dependencies(id, anchor, kind, EvidenceStatus::Asserted, dependencies)
}

pub fn language_core_test_evidence(
    id: u32,
    lang: Lang,
    anchor: EvidenceAnchor,
    kind: EvidenceKind,
    status: EvidenceStatus,
) -> EvidenceRecord {
    language_core_test_evidence_with_dependencies(id, lang, anchor, kind, status, Vec::new())
}

pub fn language_core_test_evidence_with_dependencies(
    id: u32,
    lang: Lang,
    anchor: EvidenceAnchor,
    kind: EvidenceKind,
    status: EvidenceStatus,
    dependencies: Vec<EvidenceId>,
) -> EvidenceRecord {
    let (pack_id, producer_id) = language_core_evidence_provenance(lang);
    EvidenceRecord::builtin(
        EvidenceId(id),
        anchor,
        kind,
        pack_id,
        producer_id,
        dependencies,
        status,
    )
}

pub fn language_core_test_asserted_evidence(
    id: u32,
    lang: Lang,
    anchor: EvidenceAnchor,
    kind: EvidenceKind,
    dependencies: Vec<EvidenceId>,
) -> EvidenceRecord {
    language_core_test_evidence_with_dependencies(
        id,
        lang,
        anchor,
        kind,
        EvidenceStatus::Asserted,
        dependencies,
    )
}
