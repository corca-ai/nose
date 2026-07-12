use super::*;

#[derive(Clone, Copy)]
enum CompactMapBranch {
    CallbackParam,
    CapturedParam,
    Null,
    Literal,
    EffectfulCall,
}

#[derive(Clone, Copy)]
enum CompactMapCondition {
    CallbackParam,
    CapturedParam,
    EffectfulCall,
}

fn add_branch(
    b: &mut IlBuilder,
    interner: &Interner,
    branch: CompactMapBranch,
    line: u32,
) -> NodeId {
    match branch {
        CompactMapBranch::CallbackParam => b.add(NodeKind::Var, Payload::Cid(1), sp(line), &[]),
        CompactMapBranch::CapturedParam => b.add(NodeKind::Var, Payload::Cid(2), sp(line), &[]),
        CompactMapBranch::Null => b.add(NodeKind::Lit, Payload::Lit(LitClass::Null), sp(line), &[]),
        CompactMapBranch::Literal => b.add(NodeKind::Lit, Payload::LitInt(1), sp(line), &[]),
        CompactMapBranch::EffectfulCall => {
            let callee = b.add(
                NodeKind::Var,
                Payload::Name(interner.intern("observe")),
                sp(line),
                &[],
            );
            let value = b.add(NodeKind::Var, Payload::Cid(1), sp(line + 1), &[]);
            b.add(
                NodeKind::Call,
                Payload::None,
                sp(line + 2),
                &[callee, value],
            )
        }
    }
}

fn swift_compact_map_call_il(
    then_branch: CompactMapBranch,
    else_branch: CompactMapBranch,
    condition: CompactMapCondition,
    param_count: usize,
) -> (Il, Interner, NodeId, NodeId, NodeId) {
    let interner = Interner::new();
    let mut b = IlBuilder::new(FileId(0));
    let receiver = b.add(NodeKind::Var, Payload::Cid(0), sp(800), &[]);
    let callee = b.add(
        NodeKind::Field,
        Payload::Name(interner.intern("compactMap")),
        sp(801),
        &[receiver],
    );
    let mut callback_children = Vec::new();
    for offset in 0..param_count {
        callback_children.push(b.add(
            NodeKind::Param,
            Payload::Cid(1 + offset as u32),
            sp(802 + offset as u32),
            &[],
        ));
    }
    let condition = match condition {
        CompactMapCondition::CallbackParam => b.add(NodeKind::Var, Payload::Cid(1), sp(810), &[]),
        CompactMapCondition::CapturedParam => b.add(NodeKind::Var, Payload::Cid(2), sp(810), &[]),
        CompactMapCondition::EffectfulCall => {
            add_branch(&mut b, &interner, CompactMapBranch::EffectfulCall, 810)
        }
    };
    let then_node = add_branch(&mut b, &interner, then_branch, 811);
    let else_node = add_branch(&mut b, &interner, else_branch, 814);
    let conditional = b.add(
        NodeKind::If,
        Payload::None,
        sp(817),
        &[condition, then_node, else_node],
    );
    let output = b.add(NodeKind::ExprStmt, Payload::None, sp(818), &[conditional]);
    let body = b.add(NodeKind::Block, Payload::None, sp(819), &[output]);
    callback_children.push(body);
    let callback = b.add(
        NodeKind::Lambda,
        Payload::None,
        span(802, 820, 820),
        &callback_children,
    );
    let call = b.add(NodeKind::Call, Payload::None, sp(821), &[callee, callback]);
    let source = b.add(NodeKind::Param, Payload::Cid(0), sp(822), &[]);
    let captured = b.add(NodeKind::Param, Payload::Cid(2), sp(822), &[]);
    let root = b.add(
        NodeKind::Func,
        Payload::None,
        span(799, 824, 823),
        &[source, captured, call],
    );
    (
        finish_il(b, root, Lang::Swift),
        interner,
        call,
        receiver,
        source,
    )
}

fn push_bracket_array_source_proof(il: &mut Il, id: u32, source: NodeId) {
    il.evidence.push(language_core_evidence(
        id,
        EvidenceAnchor::param(il.node(source).span),
        EvidenceKind::Type(TypeEvidenceKind::SwiftBracketArrayParameter),
        EvidenceStatus::Asserted,
        Lang::Swift,
    ));
}

fn admitted_compact_map(
    then_branch: CompactMapBranch,
    else_branch: CompactMapBranch,
    condition: CompactMapCondition,
    param_count: usize,
) -> Option<AdmittedLibraryApiCall<LibraryMethodCallContract>> {
    let (mut il, interner, call, receiver, source) =
        swift_compact_map_call_il(then_branch, else_branch, condition, param_count);
    push_bracket_array_source_proof(&mut il, 0, source);
    push_receiver_domain_dependency(&mut il, 1, receiver, DomainEvidence::Collection);
    let contract =
        library_method_call_contract(Lang::Swift, "compactMap", 1).expect("compactMap row");
    il.evidence
        .push(sequence_hof_record(2, &il, call, contract, 1, &[1]));
    admitted_library_method_call_at_call(&il, &interner, call)
}

#[test]
fn swift_compact_map_admits_both_exact_optional_emission_orientations() {
    for (then_branch, else_branch) in [
        (CompactMapBranch::CallbackParam, CompactMapBranch::Null),
        (CompactMapBranch::Null, CompactMapBranch::CallbackParam),
    ] {
        let occurrence = admitted_compact_map(
            then_branch,
            else_branch,
            CompactMapCondition::CallbackParam,
            1,
        )
        .expect("exact conditional optional emission admits");
        assert_eq!(
            occurrence.contract.id,
            LibraryApiContractId::MethodCall(MethodSemanticContract::HoF(HoFKind::FilterMap))
        );
    }
}

#[test]
fn swift_compact_map_keeps_drop_and_emitted_value_as_independent_coordinates() {
    assert!(
        admitted_compact_map(
            CompactMapBranch::CallbackParam,
            CompactMapBranch::Null,
            CompactMapCondition::CallbackParam,
            1,
        )
        .is_some(),
        "the drop condition remains a separate value-graph coordinate"
    );
}

#[test]
fn swift_compact_map_rejects_non_optional_or_unmodeled_emission_shapes() {
    for (then_branch, else_branch, message) in [
        (
            CompactMapBranch::CapturedParam,
            CompactMapBranch::Null,
            "a captured value may be Optional or a custom nil-literal type",
        ),
        (
            CompactMapBranch::CallbackParam,
            CompactMapBranch::CapturedParam,
            "both branches emit",
        ),
        (
            CompactMapBranch::Null,
            CompactMapBranch::Null,
            "both branches drop",
        ),
        (
            CompactMapBranch::Literal,
            CompactMapBranch::Null,
            "contextual literal emission is outside the bound-value perimeter",
        ),
        (
            CompactMapBranch::EffectfulCall,
            CompactMapBranch::Null,
            "effectful emitted value",
        ),
    ] {
        assert!(
            admitted_compact_map(
                then_branch,
                else_branch,
                CompactMapCondition::CallbackParam,
                1,
            )
            .is_none(),
            "{message} must remain closed"
        );
    }
}

#[test]
fn swift_compact_map_rejects_effectful_conditions_and_non_unary_callbacks() {
    assert!(
        admitted_compact_map(
            CompactMapBranch::CallbackParam,
            CompactMapBranch::Null,
            CompactMapCondition::EffectfulCall,
            1,
        )
        .is_none(),
        "an observed condition call is not a pure drop coordinate"
    );
    assert!(
        admitted_compact_map(
            CompactMapBranch::CapturedParam,
            CompactMapBranch::Null,
            CompactMapCondition::CapturedParam,
            1,
        )
        .is_none(),
        "a same-binding captured condition/emission must not erase source cardinality"
    );
    assert!(
        admitted_compact_map(
            CompactMapBranch::CallbackParam,
            CompactMapBranch::Null,
            CompactMapCondition::CallbackParam,
            2,
        )
        .is_none(),
        "custom multi-parameter overload shapes remain closed"
    );
}

#[test]
fn swift_compact_map_requires_an_ordered_stdlib_receiver() {
    let (mut il, interner, call, receiver, source) = swift_compact_map_call_il(
        CompactMapBranch::CallbackParam,
        CompactMapBranch::Null,
        CompactMapCondition::CallbackParam,
        1,
    );
    push_bracket_array_source_proof(&mut il, 0, source);
    push_receiver_domain_dependency(&mut il, 1, receiver, DomainEvidence::Set);
    let contract =
        library_method_call_contract(Lang::Swift, "compactMap", 1).expect("compactMap row");
    il.evidence
        .push(sequence_hof_record(2, &il, call, contract, 1, &[1]));
    assert!(
        admitted_library_method_call_at_call(&il, &interner, call).is_none(),
        "custom/set-like receivers cannot masquerade as ordered stdlib compactMap"
    );
}

#[test]
fn swift_compact_map_requires_language_core_bracket_array_source_proof() {
    let make = || {
        swift_compact_map_call_il(
            CompactMapBranch::CallbackParam,
            CompactMapBranch::Null,
            CompactMapCondition::CallbackParam,
            1,
        )
    };
    let contract =
        library_method_call_contract(Lang::Swift, "compactMap", 1).expect("compactMap row");

    for (label, source_proof) in [
        ("missing source proof", None),
        ("pack-asserted bracket-array proof", Some(false)),
        ("language-core collection domain only", Some(true)),
    ] {
        let (mut il, interner, call, receiver, source) = make();
        if let Some(language_core_domain_only) = source_proof {
            let record = if language_core_domain_only {
                language_core_evidence(
                    0,
                    EvidenceAnchor::param(il.node(source).span),
                    EvidenceKind::Domain(DomainEvidence::Collection),
                    EvidenceStatus::Asserted,
                    Lang::Swift,
                )
            } else {
                evidence(
                    0,
                    EvidenceAnchor::param(il.node(source).span),
                    EvidenceKind::Type(TypeEvidenceKind::SwiftBracketArrayParameter),
                    EvidenceStatus::Asserted,
                )
            };
            il.evidence.push(record);
        }
        let next = il.evidence.len() as u32;
        push_receiver_domain_dependency(&mut il, next, receiver, DomainEvidence::Collection);
        il.evidence.push(sequence_hof_record(
            next + 1,
            &il,
            call,
            contract,
            1,
            &[next],
        ));
        assert!(
            admitted_library_method_call_at_call(&il, &interner, call).is_none(),
            "{label} must not prove stdlib bracket-array dispatch"
        );
    }
}
