use super::*;

#[derive(Clone, Copy)]
enum FlatMapOutput {
    InnerMap,
    IdentityCollection,
    CapturedScalar,
    NestedFlatMap,
}

struct FlatMapFixture {
    il: Il,
    interner: Interner,
    outer_call: NodeId,
    outer_receiver: NodeId,
    outer_source_param: NodeId,
    callback_collection_param: NodeId,
    inner_call: Option<NodeId>,
    inner_receiver: Option<NodeId>,
}

fn swift_flat_map_call_il(output: FlatMapOutput, dispatch_barrier: bool) -> FlatMapFixture {
    let interner = Interner::new();
    let mut b = IlBuilder::new(FileId(0));
    let outer_source_param = b.add(NodeKind::Param, Payload::Cid(0), sp(900), &[]);
    let captured_scalar_param = b.add(NodeKind::Param, Payload::Cid(3), sp(901), &[]);
    let outer_receiver = b.add(NodeKind::Var, Payload::Cid(0), sp(902), &[]);
    let outer_callee = b.add(
        NodeKind::Field,
        Payload::Name(interner.intern("flatMap")),
        sp(903),
        &[outer_receiver],
    );
    let callback_collection_param = b.add(NodeKind::Param, Payload::Cid(1), sp(904), &[]);

    let (callback_output, inner_call, inner_receiver) = match output {
        FlatMapOutput::IdentityCollection => (
            b.add(NodeKind::Var, Payload::Cid(1), sp(905), &[]),
            None,
            None,
        ),
        FlatMapOutput::CapturedScalar => (
            b.add(NodeKind::Var, Payload::Cid(3), sp(905), &[]),
            None,
            None,
        ),
        FlatMapOutput::InnerMap | FlatMapOutput::NestedFlatMap => {
            let inner_receiver = b.add(NodeKind::Var, Payload::Cid(1), sp(906), &[]);
            let method = match output {
                FlatMapOutput::InnerMap => "map",
                FlatMapOutput::NestedFlatMap => "flatMap",
                _ => unreachable!(),
            };
            let inner_callee = b.add(
                NodeKind::Field,
                Payload::Name(interner.intern(method)),
                sp(907),
                &[inner_receiver],
            );
            let emitted_param = b.add(NodeKind::Param, Payload::Cid(2), sp(908), &[]);
            let emitted = b.add(NodeKind::Var, Payload::Cid(2), sp(909), &[]);
            let emitted_expr = b.add(NodeKind::ExprStmt, Payload::None, sp(910), &[emitted]);
            let inner_body = b.add(NodeKind::Block, Payload::None, sp(911), &[emitted_expr]);
            let inner_callback = b.add(
                NodeKind::Lambda,
                Payload::None,
                span(908, 912, 911),
                &[emitted_param, inner_body],
            );
            let inner_call = b.add(
                NodeKind::Call,
                Payload::None,
                sp(913),
                &[inner_callee, inner_callback],
            );
            (inner_call, Some(inner_call), Some(inner_receiver))
        }
    };
    let callback_expr = b.add(
        NodeKind::ExprStmt,
        Payload::None,
        sp(914),
        &[callback_output],
    );
    let callback_body = b.add(NodeKind::Block, Payload::None, sp(915), &[callback_expr]);
    let outer_callback = b.add(
        NodeKind::Lambda,
        Payload::None,
        span(904, 916, 915),
        &[callback_collection_param, callback_body],
    );
    let outer_call = b.add(
        NodeKind::Call,
        Payload::None,
        sp(917),
        &[outer_callee, outer_callback],
    );
    let mut root_children = vec![outer_source_param, captured_scalar_param, outer_call];
    if dispatch_barrier {
        root_children.push(b.add(
            NodeKind::Block,
            Payload::Name(interner.intern(SWIFT_FLAT_MAP_DISPATCH_BARRIER_MARKER)),
            sp(918),
            &[],
        ));
    }
    let root = b.add(
        NodeKind::Func,
        Payload::None,
        span(899, 920, 919),
        &root_children,
    );
    FlatMapFixture {
        il: finish_il(b, root, Lang::Swift),
        interner,
        outer_call,
        outer_receiver,
        outer_source_param,
        callback_collection_param,
        inner_call,
        inner_receiver,
    }
}

fn push_bracket_array_proof(il: &mut Il, id: u32, param: NodeId) {
    il.push_evidence(language_core_evidence(
        id,
        EvidenceAnchor::param(il.node(param).span),
        EvidenceKind::Type(TypeEvidenceKind::SwiftBracketArrayParameter),
        EvidenceStatus::Asserted,
        Lang::Swift,
    ));
}

fn add_flat_map_evidence(
    fixture: &mut FlatMapFixture,
    prove_outer_source: bool,
    prove_inner_source: bool,
) {
    let mut next = 0;
    if prove_outer_source {
        push_bracket_array_proof(&mut fixture.il, next, fixture.outer_source_param);
        next += 1;
    }
    if prove_inner_source {
        push_bracket_array_proof(&mut fixture.il, next, fixture.callback_collection_param);
        next += 1;
    }
    push_receiver_domain_dependency(
        &mut fixture.il,
        next,
        fixture.outer_receiver,
        DomainEvidence::Collection,
    );
    let outer_receiver_evidence = next;
    next += 1;

    if let (Some(inner_call), Some(inner_receiver)) = (fixture.inner_call, fixture.inner_receiver) {
        push_receiver_domain_dependency(
            &mut fixture.il,
            next,
            inner_receiver,
            DomainEvidence::Collection,
        );
        let inner_receiver_evidence = next;
        next += 1;
        let method = match fixture.il.node(fixture.il.children(inner_call)[0]).payload {
            Payload::Name(name) => fixture.interner.resolve(name),
            _ => unreachable!(),
        };
        let contract = library_method_call_contract(Lang::Swift, method, 1).unwrap();
        fixture.il.push_evidence(sequence_hof_record(
            next,
            &fixture.il,
            inner_call,
            contract,
            1,
            &[inner_receiver_evidence],
        ));
        next += 1;
    }

    let contract = library_method_call_contract(Lang::Swift, "flatMap", 1).unwrap();
    fixture.il.push_evidence(sequence_hof_record(
        next,
        &fixture.il,
        fixture.outer_call,
        contract,
        1,
        &[outer_receiver_evidence],
    ));
}

fn admitted_flat_map(
    output: FlatMapOutput,
    prove_outer_source: bool,
    prove_inner_source: bool,
    dispatch_barrier: bool,
) -> bool {
    let mut fixture = swift_flat_map_call_il(output, dispatch_barrier);
    add_flat_map_evidence(&mut fixture, prove_outer_source, prove_inner_source);
    admitted_library_method_call_at_call(&fixture.il, &fixture.interner, fixture.outer_call)
        .is_some()
}

#[test]
fn swift_flat_map_admits_proven_one_level_map_and_identity_collection() {
    assert!(admitted_flat_map(
        FlatMapOutput::InnerMap,
        true,
        true,
        false
    ));
    assert!(admitted_flat_map(
        FlatMapOutput::IdentityCollection,
        true,
        true,
        false
    ));
}

#[test]
fn swift_flat_map_requires_outer_and_inner_bracket_array_proofs() {
    assert!(!admitted_flat_map(
        FlatMapOutput::InnerMap,
        false,
        true,
        false
    ));
    assert!(!admitted_flat_map(
        FlatMapOutput::InnerMap,
        true,
        false,
        false
    ));
}

#[test]
fn swift_flat_map_rejects_scalar_nested_depth_and_custom_dispatch() {
    assert!(!admitted_flat_map(
        FlatMapOutput::CapturedScalar,
        true,
        true,
        false
    ));
    assert!(!admitted_flat_map(
        FlatMapOutput::NestedFlatMap,
        true,
        true,
        false
    ));
    assert!(!admitted_flat_map(
        FlatMapOutput::InnerMap,
        true,
        true,
        true
    ));
}
