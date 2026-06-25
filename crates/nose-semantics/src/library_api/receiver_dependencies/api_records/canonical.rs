use super::receiver_proofs::*;
use super::*;

pub(crate) fn language_core_builtin_at_call(il: &Il, call: NodeId, builtin: Builtin) -> bool {
    let arity = il.children(call).len();
    match (il.meta.lang, builtin, arity) {
        (Lang::Go, Builtin::Contains, 2) => true,
        (Lang::Go, Builtin::Enumerate, 1) => true,
        (Lang::Python, Builtin::DictEntry, 2) => true,
        (
            Lang::JavaScript | Lang::TypeScript | Lang::Vue | Lang::Svelte | Lang::Html,
            Builtin::Keys,
            1,
        ) => true,
        (Lang::C, Builtin::UnsignedCast32, 1) => {
            source_cast_at_node(il, call) == Some(SourceCastKind::CUnsigned32)
        }
        (_, Builtin::Append, 2) => {
            asserted_effect_at_node(il, call, EffectEvidenceKind::BuilderAppendCall)
        }
        _ => false,
    }
}

/// The asserted same-span `LibraryApi` evidence record that licenses a canonical builtin call.
///
/// Normalization may rewrite a source/library call to `Payload::Builtin`, but the payload is only
/// an operation shape. Producers of downstream evidence can use this helper to preserve the
/// original source/API proof as a dependency instead of treating the canonical payload as proof.
pub fn library_api_dependency_id_for_canonical_builtin_call(
    il: &Il,
    call: NodeId,
    builtin: Builtin,
) -> Option<EvidenceId> {
    library_api_dependency_id_for_canonical_builtin_call_contract(
        il,
        None,
        call,
        |record, id, _, _| {
            library_api_record_models_canonical_builtin(il, call, record, id, builtin)
        },
    )
}

pub fn library_api_dependency_id_for_canonical_builtin_call_with_interner(
    il: &Il,
    interner: &Interner,
    call: NodeId,
    builtin: Builtin,
) -> Option<EvidenceId> {
    library_api_dependency_id_for_canonical_builtin_call_contract(
        il,
        Some(interner),
        call,
        |record, id, _, _| {
            library_api_record_models_canonical_builtin(il, call, record, id, builtin)
        },
    )
}

pub fn library_api_dependency_id_for_canonical_builtin_method_call(
    il: &Il,
    call: NodeId,
    builtin: Builtin,
    expected_callee: LibraryApiCalleeContract,
    expected_arity: u16,
) -> Option<EvidenceId> {
    library_api_dependency_id_for_canonical_builtin_call_contract(
        il,
        None,
        call,
        |record, id, callee, arity| {
            library_api_record_models_canonical_builtin(il, call, record, id, builtin)
                && callee == Some(expected_callee)
                && arity == expected_arity
        },
    )
}

pub fn library_api_dependency_id_for_canonical_builtin_method_call_with_interner(
    il: &Il,
    interner: &Interner,
    call: NodeId,
    builtin: Builtin,
    expected_callee: LibraryApiCalleeContract,
    expected_arity: u16,
) -> Option<EvidenceId> {
    library_api_dependency_id_for_canonical_builtin_call_contract(
        il,
        Some(interner),
        call,
        |record, id, callee, arity| {
            library_api_record_models_canonical_builtin(il, call, record, id, builtin)
                && callee == Some(expected_callee)
                && arity == expected_arity
        },
    )
}

pub(in crate::library_api) fn library_api_dependency_id_for_canonical_builtin_call_contract(
    il: &Il,
    interner: Option<&Interner>,
    call: NodeId,
    accepts: impl Fn(
        &EvidenceRecord,
        LibraryApiContractId,
        Option<LibraryApiCalleeContract>,
        u16,
    ) -> bool,
) -> Option<EvidenceId> {
    if il.kind(call) != NodeKind::Call {
        return None;
    }
    let span = il.node(call).span;
    let mut found = None;
    let mut imported_occurrence_cache = ImportedOccurrenceValidationCache::default();
    for record in il.evidence_anchored_at(span) {
        if !matches!(
            record.anchor,
            EvidenceAnchor::Node {
                span: record_span,
                kind: NodeKind::Call | NodeKind::Field,
            } if record_span == span
        ) {
            continue;
        }
        let EvidenceKind::LibraryApi(LibraryApiEvidenceKind::Contract {
            contract_hash,
            callee_hash,
            arity,
        }) = record.kind
        else {
            continue;
        };
        let Some(id) = library_api_contract_id_from_hash(contract_hash) else {
            continue;
        };
        let callee = library_api_callee_contract_for_hash(il.meta.lang, id, callee_hash);
        if !canonical_record_provenance_and_dependencies_match(
            il,
            interner,
            &mut imported_occurrence_cache,
            call,
            record,
            id,
            callee,
        ) {
            return None;
        }
        if !accepts(record, id, callee, arity) {
            return None;
        }
        if record.status != EvidenceStatus::Asserted || !il.evidence_dependencies_asserted(record) {
            return None;
        }
        match found {
            None => found = Some(record.id),
            Some(existing) if existing == record.id => {}
            Some(_) => return None,
        }
    }
    found
}

pub(super) fn canonical_record_provenance_and_dependencies_match(
    il: &Il,
    interner: Option<&Interner>,
    imported_occurrence_cache: &mut ImportedOccurrenceValidationCache,
    call: NodeId,
    record: &EvidenceRecord,
    id: LibraryApiContractId,
    callee: Option<LibraryApiCalleeContract>,
) -> bool {
    let Some(callee) = callee else {
        return !matches!(
            id,
            LibraryApiContractId::ScalarIntegerMethod(_) | LibraryApiContractId::MethodCall(_)
        );
    };
    if !library_api_record_provenance_matches_contract(id, callee, record) {
        return false;
    }
    match (id, callee) {
        (
            LibraryApiContractId::ScalarIntegerMethod(_),
            LibraryApiCalleeContract::Method {
                receiver: MethodReceiverContract::ExactInteger,
                ..
            },
        ) => il
            .children(call)
            .first()
            .is_some_and(|&arg| canonical_integer_arg_dependency_present(il, record, arg)),
        (
            LibraryApiContractId::ScalarIntegerMethod(_),
            LibraryApiCalleeContract::Method {
                receiver: MethodReceiverContract::UnshadowedGlobal("Math"),
                ..
            },
        ) => {
            canonical_record_has_unshadowed_math_dependency(il, call, record)
                && il
                    .children(call)
                    .iter()
                    .all(|&arg| canonical_integer_arg_dependency_present(il, record, arg))
        }
        (
            LibraryApiContractId::FreeFunctionBuiltin(_),
            LibraryApiCalleeContract::FreeName { name, .. },
        ) => canonical_record_has_unshadowed_symbol_dependency(il, call, record, name),
        (LibraryApiContractId::MethodCall(_), LibraryApiCalleeContract::Method { .. }) => {
            canonical_method_call_record_dependencies_match(
                il,
                interner,
                imported_occurrence_cache,
                call,
                record,
                id,
                callee,
            )
        }
        _ => true,
    }
}

pub(super) fn canonical_record_has_unshadowed_math_dependency(
    il: &Il,
    call: NodeId,
    record: &EvidenceRecord,
) -> bool {
    canonical_record_has_unshadowed_symbol_dependency(il, call, record, "Math")
}

pub(super) fn canonical_record_has_unshadowed_symbol_dependency(
    il: &Il,
    call: NodeId,
    record: &EvidenceRecord,
    name: &str,
) -> bool {
    let call_span = il.node(call).span;
    let expected = SymbolEvidenceKind::UnshadowedGlobal {
        name_hash: stable_symbol_hash(name),
    };
    record.dependencies.iter().any(|&id| {
        let Some(dependency) = il.evidence_record_by_id(id) else {
            return false;
        };
        let EvidenceAnchor::Node {
            span,
            kind: NodeKind::Var,
        } = dependency.anchor
        else {
            return false;
        };
        dependency.status == EvidenceStatus::Asserted
            && dependency.kind == EvidenceKind::Symbol(expected)
            && symbol_record_has_admitted_provenance(il, dependency)
            && il.evidence_dependencies_asserted(dependency)
            && span.file == call_span.file
            && span.start_byte == call_span.start_byte
            && span.end_byte <= call_span.end_byte
            && matches!(
                language_core_symbol_identity_at_anchor_matches(il, span, NodeKind::Var, expected),
                EvidenceResolution::Found(true)
            )
    })
}

pub(super) fn canonical_integer_arg_dependency_present(
    il: &Il,
    record: &EvidenceRecord,
    arg: NodeId,
) -> bool {
    if matches!(il.node(arg).payload, Payload::LitInt(_)) {
        return true;
    }
    let expected = EvidenceKind::Domain(DomainEvidence::Integer);
    record.dependencies.iter().any(|&id| {
        let Some(dependency) = il.evidence_record_by_id(id) else {
            return false;
        };
        if dependency.status != EvidenceStatus::Asserted || dependency.kind != expected {
            return false;
        }
        match dependency.anchor {
            EvidenceAnchor::Node { span, kind } => {
                span == il.node(arg).span && kind == il.kind(arg)
            }
            EvidenceAnchor::Param { span } => {
                let Payload::Cid(cid) = il.node(arg).payload else {
                    return false;
                };
                il.nodes.iter().any(|node| {
                    node.kind == NodeKind::Param
                        && node.span == span
                        && matches!(node.payload, Payload::Cid(param_cid) if param_cid == cid)
                })
            }
            _ => false,
        }
    })
}

pub(in crate::library_api) fn library_api_record_models_canonical_builtin(
    il: &Il,
    call: NodeId,
    record: &EvidenceRecord,
    id: LibraryApiContractId,
    builtin: Builtin,
) -> bool {
    if let LibraryApiContractId::FreeFunctionBuiltin(expected_builtin) = id {
        return expected_builtin == builtin
            && library_api_record_models_free_function_builtin(il, record, expected_builtin);
    }
    if library_api_record_models_rust_map_get_default(il, call, record, id, builtin) {
        return true;
    }
    if matches!(id, LibraryApiContractId::MethodCall(_)) {
        return library_api_record_models_method_call_builtin(il, record, id, builtin);
    }
    if library_api_contract_id_builtin_result(id) == Some(builtin) {
        return true;
    }
    false
}

pub(super) fn library_api_record_models_free_function_builtin(
    il: &Il,
    record: &EvidenceRecord,
    builtin: Builtin,
) -> bool {
    let EvidenceKind::LibraryApi(LibraryApiEvidenceKind::Contract {
        callee_hash, arity, ..
    }) = record.kind
    else {
        return false;
    };
    let Some(LibraryApiCalleeContract::FreeName { name, .. }) =
        library_api_callee_contract_for_hash(
            il.meta.lang,
            LibraryApiContractId::FreeFunctionBuiltin(builtin),
            callee_hash,
        )
    else {
        return false;
    };
    library_free_function_builtin_contract(il.meta.lang, name, arity as usize)
        .is_some_and(|contract| contract.result.builtin == builtin)
}

pub(super) fn library_api_record_models_method_call_builtin(
    il: &Il,
    record: &EvidenceRecord,
    id: LibraryApiContractId,
    builtin: Builtin,
) -> bool {
    let EvidenceKind::LibraryApi(LibraryApiEvidenceKind::Contract {
        callee_hash, arity, ..
    }) = record.kind
    else {
        return false;
    };
    let Some(callee) = library_api_callee_contract_for_hash(il.meta.lang, id, callee_hash) else {
        return false;
    };
    library_api_method_call_record_contract(il, id, callee, arity).is_some_and(|contract| {
        contract.result.semantic == MethodSemanticContract::Builtin(builtin)
    })
}

pub(super) fn library_api_method_call_record_contract(
    il: &Il,
    id: LibraryApiContractId,
    callee: LibraryApiCalleeContract,
    arity: u16,
) -> Option<LibraryMethodCallContract> {
    let LibraryApiContractId::MethodCall(expected) = id else {
        return None;
    };
    let LibraryApiCalleeContract::Method { method, .. } = callee else {
        return None;
    };
    library_method_call_contracts(il.meta.lang, method, arity as usize)
        .into_iter()
        .find(|contract| {
            contract.id == id && contract.callee == callee && contract.result.semantic == expected
        })
}

pub(super) fn canonical_method_call_record_dependencies_match(
    il: &Il,
    interner: Option<&Interner>,
    imported_occurrence_cache: &mut ImportedOccurrenceValidationCache,
    call: NodeId,
    record: &EvidenceRecord,
    id: LibraryApiContractId,
    callee: LibraryApiCalleeContract,
) -> bool {
    let EvidenceKind::LibraryApi(LibraryApiEvidenceKind::Contract { arity, .. }) = record.kind
    else {
        return false;
    };
    let Some(contract) = library_api_method_call_record_contract(il, id, callee, arity) else {
        return false;
    };
    method_call_receiver_dependencies_match(
        il,
        interner,
        imported_occurrence_cache,
        call,
        record,
        contract,
    )
}
