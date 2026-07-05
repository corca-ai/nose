#![allow(clippy::too_many_arguments, clippy::too_many_lines)]

use super::*;

mod admission_resolvers;
mod callee_sources;
mod canonical_builtin;
mod resolution;

fn library_api_record(
    id: u32,
    span: Span,
    contract_id: LibraryApiContractId,
    callee: LibraryApiCalleeContract,
    status: EvidenceStatus,
    dependencies: &[u32],
) -> EvidenceRecord {
    library_api_record_with_arity(id, span, contract_id, callee, 1, status, dependencies)
}

fn language_core_symbol_record(
    id: u32,
    anchor: EvidenceAnchor,
    symbol: SymbolEvidenceKind,
    status: EvidenceStatus,
    dependencies: &[u32],
    lang: Lang,
) -> EvidenceRecord {
    language_core_evidence_with_dependencies(
        id,
        anchor,
        EvidenceKind::Symbol(symbol),
        status,
        dependencies.iter().copied().map(EvidenceId).collect(),
        lang,
    )
}

fn library_api_record_with_provenance(
    id: u32,
    span: Span,
    contract_id: LibraryApiContractId,
    callee: LibraryApiCalleeContract,
    status: EvidenceStatus,
    dependencies: &[u32],
    pack_id: &str,
    rule: &str,
) -> EvidenceRecord {
    library_api_record_with_provenance_and_arity(
        id,
        span,
        contract_id,
        callee,
        1,
        status,
        dependencies,
        pack_id,
        rule,
    )
}

fn library_api_record_with_provenance_and_arity(
    id: u32,
    span: Span,
    contract_id: LibraryApiContractId,
    callee: LibraryApiCalleeContract,
    arity: u16,
    status: EvidenceStatus,
    dependencies: &[u32],
    pack_id: &str,
    rule: &str,
) -> EvidenceRecord {
    test_support::library_api_test_evidence_with_dependencies(
        id,
        span,
        test_support::LibraryApiTestContract {
            id: contract_id,
            callee,
            arity,
        },
        status,
        dependencies.iter().copied().map(EvidenceId).collect(),
        (pack_id, rule),
    )
}

fn property_builtin_record(
    id: u32,
    span: Span,
    contract: LibraryPropertyBuiltinContract,
    status: EvidenceStatus,
    dependencies: &[u32],
) -> EvidenceRecord {
    library_api_record_with_provenance_and_arity(
        id,
        span,
        contract.id,
        contract.callee,
        0,
        status,
        dependencies,
        PROPERTY_BUILTIN_PROTOCOL_PACK_ID,
        PROPERTY_BUILTIN_PROTOCOL_PRODUCER_ID,
    )
}

fn python_stdlib_math_record(
    id: u32,
    span: Span,
    contract: LibraryImportedNamespaceFunctionContract,
    arity: u16,
    status: EvidenceStatus,
    dependencies: &[u32],
) -> EvidenceRecord {
    library_api_record_with_provenance_and_arity(
        id,
        span,
        contract.id,
        contract.callee,
        arity,
        status,
        dependencies,
        PYTHON_STDLIB_MATH_PACK_ID,
        PYTHON_STDLIB_MATH_PRODUCER_ID,
    )
}

mod records;
pub(super) use records::*;
