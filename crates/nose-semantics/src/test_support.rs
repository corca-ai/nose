use nose_il::{
    EvidenceAnchor, EvidenceId, EvidenceKind, EvidenceRecord, EvidenceStatus, Lang,
    LibraryApiEvidenceKind, NodeKind, Span,
};

use crate::{
    language_core_evidence_provenance, library_api_callee_contract_hash,
    library_api_contract_id_hash, library_method_call_contract, LibraryApiCalleeContract,
    LibraryApiContractId, BUILTIN_COMPAT_PACK_ID, BUILTIN_METHOD_CALL_PROTOCOL_PACK_ID,
    BUILTIN_METHOD_CALL_PROTOCOL_PRODUCER_ID, FREE_FUNCTION_BUILTIN_PROTOCOL_PACK_ID,
    FREE_FUNCTION_BUILTIN_PROTOCOL_PRODUCER_ID,
};

#[derive(Clone, Copy)]
pub struct LibraryApiTestContract {
    pub id: LibraryApiContractId,
    pub callee: LibraryApiCalleeContract,
    pub arity: u16,
}

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

pub fn compat_library_api_test_evidence_with_dependencies(
    id: u32,
    span: Span,
    contract: LibraryApiTestContract,
    status: EvidenceStatus,
    dependencies: Vec<EvidenceId>,
) -> EvidenceRecord {
    library_api_test_evidence_with_dependencies(
        id,
        span,
        contract,
        status,
        dependencies,
        (BUILTIN_COMPAT_PACK_ID, "test"),
    )
}

pub fn builtin_library_api_test_evidence_with_dependencies(
    id: u32,
    span: Span,
    contract: LibraryApiTestContract,
    status: EvidenceStatus,
    dependencies: Vec<EvidenceId>,
) -> EvidenceRecord {
    let provenance = if matches!(contract.id, LibraryApiContractId::FreeFunctionBuiltin(_)) {
        (
            FREE_FUNCTION_BUILTIN_PROTOCOL_PACK_ID,
            FREE_FUNCTION_BUILTIN_PROTOCOL_PRODUCER_ID,
        )
    } else if matches!(contract.id, LibraryApiContractId::MethodCall(_)) {
        (
            BUILTIN_METHOD_CALL_PROTOCOL_PACK_ID,
            BUILTIN_METHOD_CALL_PROTOCOL_PRODUCER_ID,
        )
    } else {
        (BUILTIN_COMPAT_PACK_ID, "test")
    };
    library_api_test_evidence_with_dependencies(
        id,
        span,
        contract,
        status,
        dependencies,
        provenance,
    )
}

pub fn method_call_library_api_test_evidence_with_dependencies(
    id: u32,
    lang: Lang,
    method: &str,
    span: Span,
    arity: usize,
    dependencies: Vec<EvidenceId>,
) -> EvidenceRecord {
    let contract = library_method_call_contract(lang, method, arity).expect("method call contract");
    library_api_test_evidence_with_dependencies(
        id,
        span,
        LibraryApiTestContract {
            id: contract.id,
            callee: contract.callee,
            arity: arity as u16,
        },
        EvidenceStatus::Asserted,
        dependencies,
        (contract.pack_id, contract.producer_id),
    )
}

pub fn library_api_test_evidence_with_dependencies(
    id: u32,
    span: Span,
    contract: LibraryApiTestContract,
    status: EvidenceStatus,
    dependencies: Vec<EvidenceId>,
    provenance: (&str, &str),
) -> EvidenceRecord {
    EvidenceRecord::builtin(
        EvidenceId(id),
        EvidenceAnchor::node(span, NodeKind::Call),
        EvidenceKind::LibraryApi(LibraryApiEvidenceKind::Contract {
            contract_hash: library_api_contract_id_hash(contract.id),
            callee_hash: library_api_callee_contract_hash(contract.callee),
            arity: contract.arity,
        }),
        provenance.0,
        provenance.1,
        dependencies,
        status,
    )
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{SEQUENCE_HOF_ADAPTER_PROTOCOL_PACK_ID, SEQUENCE_HOF_ADAPTER_PROTOCOL_PRODUCER_ID};
    use nose_il::stable_symbol_hash;

    #[test]
    fn method_call_library_api_test_evidence_uses_contract_provenance() {
        let record = method_call_library_api_test_evidence_with_dependencies(
            7,
            Lang::Rust,
            "map",
            Span::new(nose_il::FileId(0), 1, 1, 1, 1),
            1,
            Vec::new(),
        );

        assert_eq!(
            record.provenance.pack_hash,
            Some(stable_symbol_hash(SEQUENCE_HOF_ADAPTER_PROTOCOL_PACK_ID))
        );
        assert_eq!(
            record.provenance.rule_hash,
            Some(stable_symbol_hash(
                SEQUENCE_HOF_ADAPTER_PROTOCOL_PRODUCER_ID
            ))
        );
    }
}
