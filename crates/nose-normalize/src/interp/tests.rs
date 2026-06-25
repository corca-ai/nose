use super::*;

mod basic;
mod calls_symbolic;
mod hof;
mod state;
mod support;
mod try_effects;

use support::append_test_python_iterator_builtin_source_dependencies;

use nose_il::{
    stable_symbol_hash, CallTargetEvidenceKind, EffectEvidenceKind, EvidenceAnchor,
    EvidenceEmitter, EvidenceId, EvidenceKind, EvidenceProvenance, EvidenceRecord, EvidenceStatus,
    FileId, FileMeta, HoFKind, IlBuilder, Interner, Lang, LibraryApiEvidenceKind, LitClass,
    PlaceEvidenceKind, SourceCastKind, SourceFactKind, Span, SymbolEvidenceKind, Unit, UnitKind,
};
use nose_semantics::{
    language_core_evidence_provenance, library_api_callee_contract_hash,
    library_api_contract_id_hash, library_free_function_builtin_contract,
    library_method_call_contract, DomainEvidence, LibraryApiCalleeContract, LibraryApiContractId,
    LibraryApiShadowPolicy, LibraryMethodCallContract, MethodReceiverContract,
    MethodSemanticContract, BUILTIN_COMPAT_PACK_ID, BUILTIN_METHOD_CALL_PROTOCOL_PACK_ID,
    BUILTIN_METHOD_CALL_PROTOCOL_PRODUCER_ID, FREE_FUNCTION_BUILTIN_PROTOCOL_PACK_ID,
    FREE_FUNCTION_BUILTIN_PROTOCOL_PRODUCER_ID, PYTHON_ITERATOR_BUILTIN_PROTOCOL_PACK_ID,
    PYTHON_ITERATOR_BUILTIN_PROTOCOL_PRODUCER_ID,
};

fn language_core_provenance(lang: Lang) -> EvidenceProvenance {
    let (pack_id, producer_id) = language_core_evidence_provenance(lang);
    EvidenceProvenance {
        emitter: EvidenceEmitter::Builtin,
        pack_hash: Some(stable_symbol_hash(pack_id)),
        rule_hash: Some(stable_symbol_hash(producer_id)),
    }
}

fn run_admitted_unit(mut il: Il, root: NodeId, args: &[Value]) -> Option<Behavior> {
    admit_test_builtin_calls(&mut il);
    let interner = Interner::new();
    run_unit(&il, &interner, root, args)
}

fn run_admitted_unit_with_interner(
    mut il: Il,
    interner: &Interner,
    root: NodeId,
    args: &[Value],
) -> Option<Behavior> {
    admit_test_builtin_calls(&mut il);
    run_unit(&il, interner, root, args)
}

#[test]
fn behavior_equiv_treats_both_abort_as_equal() {
    let beh = |ret: Value, effects: Vec<Value>| Behavior {
        ret,
        effects,
        fields: vec![],
    };
    // Both abort (`Err`) with DIFFERENT pre-trap effects → equivalent: an erroring run
    // has no observable result, so reordering ops before a guaranteed trap preserves
    // behavior. This is the #-canon-preservation fix for impossible inputs.
    let abort_a = beh(Value::Err, vec![]);
    let abort_b = beh(Value::Err, vec![Value::Int(0), Value::Int(2)]);
    assert!(behavior_equiv(&abort_a, &abort_b));
    assert!(behavior_equiv(&abort_b, &abort_a));
    // Real behavior changes still compare unequal:
    // Ok→Err (the canon made it start/stop erroring)
    assert!(!behavior_equiv(
        &beh(Value::Int(1), vec![]),
        &beh(Value::Err, vec![])
    ));
    // two successful runs with different results
    assert!(!behavior_equiv(
        &beh(Value::Int(1), vec![]),
        &beh(Value::Int(2), vec![])
    ));
    // two successful runs with different effect traces
    assert!(!behavior_equiv(
        &beh(Value::Null, vec![Value::Int(1)]),
        &beh(Value::Null, vec![Value::Int(2)]),
    ));
    // identical successful runs are equal
    assert!(behavior_equiv(
        &beh(Value::Int(7), vec![Value::Int(1)]),
        &beh(Value::Int(7), vec![Value::Int(1)]),
    ));
}

fn test_span(offset: u32) -> Span {
    Span::new(FileId(0), offset, offset + 1, offset + 1, offset + 1)
}

fn admit_test_builtin_calls(il: &mut Il) {
    let mut seen_library_records = Vec::new();
    let mut next_id = 1000;
    for idx in 0..il.nodes.len() {
        let node = NodeId(idx as u32);
        let (NodeKind::Call, Payload::Builtin(builtin)) = (il.kind(node), il.node(node).payload)
        else {
            continue;
        };
        let span = il.node(node).span;
        if matches!(builtin, Builtin::Append) {
            il.evidence.push(test_effect_record(
                next_id,
                span,
                EffectEvidenceKind::BuilderAppendCall,
            ));
            next_id += 1;
        } else if let Some(contract_id) = test_library_contract_id_for_builtin(builtin) {
            if seen_library_records
                .iter()
                .any(|&(seen_span, seen_builtin)| seen_span == span && seen_builtin == builtin)
            {
                continue;
            }
            seen_library_records.push((span, builtin));
            let arg_count = il.children(node).len();
            let source_arg_count = test_free_function_builtin_source_arg_count(builtin, arg_count);
            let (contract_id, callee, arity, dependencies) = if let Some(contract) =
                test_free_function_builtin_contract(il.meta.lang, builtin, source_arg_count)
            {
                let symbol_id = EvidenceId(next_id);
                il.evidence.push(test_symbol_record(
                    next_id,
                    il.meta.lang,
                    span,
                    contract.result.name,
                ));
                next_id += 1;
                let mut dependencies = vec![symbol_id];
                append_test_python_iterator_builtin_source_dependencies(
                    il,
                    node,
                    builtin,
                    &mut next_id,
                    &mut dependencies,
                );
                (
                    contract.id,
                    contract.callee,
                    source_arg_count as u16,
                    dependencies,
                )
            } else if let Some((contract, arity)) =
                test_method_builtin_contract(il.meta.lang, builtin, arg_count)
            {
                let Some(dependencies) =
                    test_method_builtin_dependencies(il, node, contract, &mut next_id)
                else {
                    continue;
                };
                (contract.id, contract.callee, arity, dependencies)
            } else {
                (contract_id, test_callee_contract(), 0, Vec::new())
            };
            il.evidence.push(test_library_api_record(
                next_id,
                il.meta.lang,
                span,
                contract_id,
                callee,
                arity,
                dependencies,
            ));
            next_id += 1;
        } else if matches!(builtin, Builtin::UnsignedCast32) {
            il.evidence.push(test_source_record(
                next_id,
                span,
                SourceFactKind::Cast(SourceCastKind::CUnsigned32),
            ));
            next_id += 1;
        }
    }
}

fn test_free_function_builtin_contract(
    lang: Lang,
    builtin: Builtin,
    arg_count: usize,
) -> Option<nose_semantics::LibraryFreeFunctionBuiltinContract> {
    let name = test_free_function_builtin_name(builtin)?;
    let contract = library_free_function_builtin_contract(lang, name, arg_count)?;
    (contract.result.builtin == builtin).then_some(contract)
}

fn test_free_function_builtin_source_arg_count(
    builtin: Builtin,
    canonical_arg_count: usize,
) -> usize {
    if matches!(builtin, Builtin::Any | Builtin::All) && canonical_arg_count == 2 {
        1
    } else {
        canonical_arg_count
    }
}

fn test_free_function_builtin_name(builtin: Builtin) -> Option<&'static str> {
    match builtin {
        Builtin::Len => Some("len"),
        Builtin::Print => Some("print"),
        Builtin::Range => Some("range"),
        Builtin::Sum => Some("sum"),
        Builtin::Min => Some("min"),
        Builtin::Max => Some("max"),
        Builtin::Abs => Some("abs"),
        Builtin::Zip => Some("zip"),
        Builtin::Enumerate => Some("enumerate"),
        Builtin::Any => Some("any"),
        Builtin::All => Some("all"),
        _ => None,
    }
}

fn test_library_contract_id_for_builtin(builtin: Builtin) -> Option<LibraryApiContractId> {
    match builtin {
        Builtin::Len
        | Builtin::Print
        | Builtin::Range
        | Builtin::Sum
        | Builtin::Min
        | Builtin::Max
        | Builtin::Abs
        | Builtin::Zip
        | Builtin::Enumerate
        | Builtin::Any
        | Builtin::All => Some(LibraryApiContractId::FreeFunctionBuiltin(builtin)),
        Builtin::IsEmpty
        | Builtin::StartsWith
        | Builtin::EndsWith
        | Builtin::Contains
        | Builtin::StringContains
        | Builtin::GetOrDefault
        | Builtin::ValueOrDefault
        | Builtin::IsNull
        | Builtin::IsNotNull
        | Builtin::Join
        | Builtin::Reduce => Some(LibraryApiContractId::MethodCall(
            MethodSemanticContract::Builtin(builtin),
        )),
        Builtin::Append | Builtin::Keys | Builtin::DictEntry | Builtin::UnsignedCast32 => None,
    }
}

fn test_method_builtin_contract(
    lang: Lang,
    builtin: Builtin,
    canonical_arg_count: usize,
) -> Option<(LibraryMethodCallContract, u16)> {
    let (method, source_arg_count) = match (lang, builtin, canonical_arg_count) {
        (Lang::Rust, Builtin::ValueOrDefault, 2) => ("unwrap_or", 1),
        (Lang::Python, Builtin::Reduce, 3) => ("reduce", 3),
        _ => return None,
    };
    let contract = library_method_call_contract(lang, method, source_arg_count)?;
    (contract.result.semantic == MethodSemanticContract::Builtin(builtin))
        .then_some((contract, source_arg_count as u16))
}

fn test_method_builtin_dependencies(
    il: &mut Il,
    node: NodeId,
    contract: LibraryMethodCallContract,
    next_id: &mut u32,
) -> Option<Vec<EvidenceId>> {
    let LibraryApiCalleeContract::Method { receiver, .. } = contract.callee else {
        return None;
    };
    match receiver {
        MethodReceiverContract::ExactOption | MethodReceiverContract::RustMapGetOrExactOption => {
            let id = EvidenceId(*next_id);
            let receiver_node = il.children(node).first().copied()?;
            il.evidence.push(test_domain_record(
                *next_id,
                il,
                receiver_node,
                DomainEvidence::Option,
            ));
            *next_id += 1;
            Some(vec![id])
        }
        MethodReceiverContract::ImportedNamespace(module) => {
            let binding_id = EvidenceId(*next_id);
            il.evidence.push(test_imported_namespace_binding_record(
                *next_id,
                il.meta.lang,
                il.node(node).span,
                module,
            ));
            *next_id += 1;
            let occurrence_id = EvidenceId(*next_id);
            il.evidence.push(test_imported_namespace_record(
                *next_id,
                il.meta.lang,
                il.node(node).span,
                module,
                binding_id,
            ));
            *next_id += 1;
            Some(vec![occurrence_id])
        }
        _ => None,
    }
}

fn test_callee_contract() -> LibraryApiCalleeContract {
    LibraryApiCalleeContract::FreeName {
        name: "__test_builtin__",
        shadow: LibraryApiShadowPolicy::None,
    }
}

fn test_library_api_record(
    id: u32,
    lang: Lang,
    span: Span,
    contract_id: LibraryApiContractId,
    callee: LibraryApiCalleeContract,
    arity: u16,
    dependencies: Vec<EvidenceId>,
) -> EvidenceRecord {
    let (pack_id, rule_id) = if matches!(
        (lang, contract_id),
        (
            Lang::Python,
            LibraryApiContractId::FreeFunctionBuiltin(
                Builtin::Zip | Builtin::Enumerate | Builtin::Any | Builtin::All
            )
        )
    ) {
        (
            PYTHON_ITERATOR_BUILTIN_PROTOCOL_PACK_ID,
            PYTHON_ITERATOR_BUILTIN_PROTOCOL_PRODUCER_ID,
        )
    } else if matches!(contract_id, LibraryApiContractId::FreeFunctionBuiltin(_)) {
        (
            FREE_FUNCTION_BUILTIN_PROTOCOL_PACK_ID,
            FREE_FUNCTION_BUILTIN_PROTOCOL_PRODUCER_ID,
        )
    } else if matches!(contract_id, LibraryApiContractId::MethodCall(_)) {
        (
            BUILTIN_METHOD_CALL_PROTOCOL_PACK_ID,
            BUILTIN_METHOD_CALL_PROTOCOL_PRODUCER_ID,
        )
    } else {
        (BUILTIN_COMPAT_PACK_ID, "interp-test")
    };
    EvidenceRecord {
        id: EvidenceId(id),
        anchor: EvidenceAnchor::node(span, NodeKind::Call),
        kind: EvidenceKind::LibraryApi(LibraryApiEvidenceKind::Contract {
            contract_hash: library_api_contract_id_hash(contract_id),
            callee_hash: library_api_callee_contract_hash(callee),
            arity,
        }),
        provenance: EvidenceProvenance {
            emitter: EvidenceEmitter::Builtin,
            pack_hash: Some(stable_symbol_hash(pack_id)),
            rule_hash: Some(stable_symbol_hash(rule_id)),
        },
        dependencies,
        status: EvidenceStatus::Asserted,
    }
}

fn test_domain_record(id: u32, il: &Il, node: NodeId, domain: DomainEvidence) -> EvidenceRecord {
    EvidenceRecord {
        id: EvidenceId(id),
        anchor: EvidenceAnchor::node(il.node(node).span, il.kind(node)),
        kind: EvidenceKind::Domain(domain),
        provenance: EvidenceProvenance {
            emitter: EvidenceEmitter::Builtin,
            pack_hash: Some(stable_symbol_hash(BUILTIN_COMPAT_PACK_ID)),
            rule_hash: Some(stable_symbol_hash("interp-test")),
        },
        dependencies: Vec::new(),
        status: EvidenceStatus::Asserted,
    }
}

fn test_imported_namespace_binding_record(
    id: u32,
    lang: Lang,
    span: Span,
    module: &str,
) -> EvidenceRecord {
    EvidenceRecord {
        id: EvidenceId(id),
        anchor: EvidenceAnchor::binding(span, stable_symbol_hash(module)),
        kind: EvidenceKind::Symbol(SymbolEvidenceKind::ImportedNamespace {
            module_hash: stable_symbol_hash(module),
        }),
        provenance: language_core_provenance(lang),
        dependencies: Vec::new(),
        status: EvidenceStatus::Asserted,
    }
}

fn test_imported_namespace_record(
    id: u32,
    lang: Lang,
    span: Span,
    module: &str,
    binding_id: EvidenceId,
) -> EvidenceRecord {
    EvidenceRecord {
        id: EvidenceId(id),
        anchor: EvidenceAnchor::node(span, NodeKind::Var),
        kind: EvidenceKind::Symbol(SymbolEvidenceKind::ImportedNamespace {
            module_hash: stable_symbol_hash(module),
        }),
        provenance: language_core_provenance(lang),
        dependencies: vec![binding_id],
        status: EvidenceStatus::Asserted,
    }
}

fn test_symbol_record(id: u32, lang: Lang, span: Span, name: &str) -> EvidenceRecord {
    EvidenceRecord {
        id: EvidenceId(id),
        anchor: EvidenceAnchor::node(span, NodeKind::Var),
        kind: EvidenceKind::Symbol(SymbolEvidenceKind::UnshadowedGlobal {
            name_hash: stable_symbol_hash(name),
        }),
        provenance: language_core_provenance(lang),
        dependencies: Vec::new(),
        status: EvidenceStatus::Asserted,
    }
}

fn test_effect_record(id: u32, span: Span, effect: EffectEvidenceKind) -> EvidenceRecord {
    EvidenceRecord {
        id: EvidenceId(id),
        anchor: EvidenceAnchor::node(span, NodeKind::Call),
        kind: EvidenceKind::Effect(effect),
        provenance: EvidenceProvenance {
            emitter: EvidenceEmitter::Builtin,
            pack_hash: Some(stable_symbol_hash(BUILTIN_COMPAT_PACK_ID)),
            rule_hash: Some(stable_symbol_hash("interp-test")),
        },
        dependencies: Vec::new(),
        status: EvidenceStatus::Asserted,
    }
}

fn test_node_place_record(
    id: u32,
    il: &Il,
    node: NodeId,
    place: PlaceEvidenceKind,
    dependencies: Vec<EvidenceId>,
) -> EvidenceRecord {
    let (pack_id, producer_id) = language_core_evidence_provenance(Lang::Python);
    EvidenceRecord {
        id: EvidenceId(id),
        anchor: EvidenceAnchor::node(il.node(node).span, il.kind(node)),
        kind: EvidenceKind::Place(place),
        provenance: EvidenceProvenance {
            emitter: EvidenceEmitter::Builtin,
            pack_hash: Some(stable_symbol_hash(pack_id)),
            rule_hash: Some(stable_symbol_hash(producer_id)),
        },
        dependencies,
        status: EvidenceStatus::Asserted,
    }
}

fn test_node_effect_record(
    id: u32,
    il: &Il,
    node: NodeId,
    effect: EffectEvidenceKind,
    dependencies: Vec<EvidenceId>,
) -> EvidenceRecord {
    EvidenceRecord {
        id: EvidenceId(id),
        anchor: EvidenceAnchor::node(il.node(node).span, il.kind(node)),
        kind: EvidenceKind::Effect(effect),
        provenance: EvidenceProvenance {
            emitter: EvidenceEmitter::Builtin,
            pack_hash: Some(stable_symbol_hash(BUILTIN_COMPAT_PACK_ID)),
            rule_hash: Some(stable_symbol_hash("interp-test")),
        },
        dependencies,
        status: EvidenceStatus::Asserted,
    }
}

fn admit_test_self_field(
    il: &mut Il,
    interner: &Interner,
    receiver: NodeId,
    field: NodeId,
    field_name: nose_il::Symbol,
    first_id: u32,
) -> EvidenceId {
    let receiver_id = EvidenceId(first_id);
    let field_id = EvidenceId(first_id + 1);
    let receiver_record = test_node_place_record(
        first_id,
        il,
        receiver,
        PlaceEvidenceKind::SelfReceiver,
        Vec::new(),
    );
    let field_record = test_node_place_record(
        first_id + 1,
        il,
        field,
        PlaceEvidenceKind::SelfField {
            field_hash: stable_symbol_hash(interner.resolve(field_name)),
        },
        vec![receiver_id],
    );
    il.evidence.push(receiver_record);
    il.evidence.push(field_record);
    field_id
}

fn admit_test_self_field_write(
    il: &mut Il,
    interner: &Interner,
    receiver: NodeId,
    field: NodeId,
    assign: NodeId,
    field_name: nose_il::Symbol,
    first_id: u32,
) {
    let field_id = admit_test_self_field(il, interner, receiver, field, field_name, first_id);
    let effect_record = test_node_effect_record(
        first_id + 2,
        il,
        assign,
        EffectEvidenceKind::SelfFieldWrite {
            field_hash: stable_symbol_hash(interner.resolve(field_name)),
        },
        vec![field_id],
    );
    il.evidence.push(effect_record);
}

fn test_source_record(id: u32, span: Span, fact: SourceFactKind) -> EvidenceRecord {
    EvidenceRecord {
        id: EvidenceId(id),
        anchor: EvidenceAnchor::source_span(span),
        kind: EvidenceKind::Source(fact),
        provenance: EvidenceProvenance {
            emitter: EvidenceEmitter::Builtin,
            pack_hash: Some(stable_symbol_hash(BUILTIN_COMPAT_PACK_ID)),
            rule_hash: Some(stable_symbol_hash("interp-test")),
        },
        dependencies: Vec::new(),
        status: EvidenceStatus::Asserted,
    }
}

fn test_call_target_record(
    id: u32,
    call_span: Span,
    target_span: Span,
    name_hash: u64,
) -> EvidenceRecord {
    let (pack_id, producer_id) = language_core_evidence_provenance(Lang::Python);
    EvidenceRecord {
        id: EvidenceId(id),
        anchor: EvidenceAnchor::node(call_span, NodeKind::Call),
        kind: EvidenceKind::CallTarget(CallTargetEvidenceKind::DirectFunction {
            target_span,
            name_hash,
        }),
        provenance: EvidenceProvenance {
            emitter: EvidenceEmitter::Builtin,
            pack_hash: Some(stable_symbol_hash(pack_id)),
            rule_hash: Some(stable_symbol_hash(producer_id)),
        },
        dependencies: Vec::new(),
        status: EvidenceStatus::Asserted,
    }
}
