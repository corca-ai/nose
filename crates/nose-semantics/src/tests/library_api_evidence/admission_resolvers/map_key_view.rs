use super::*;

fn map_key_view_call_il(
    lang: Lang,
    method: &str,
    arg_count: usize,
) -> (Il, Interner, NodeId, NodeId, NodeId) {
    let interner = Interner::new();
    let mut b = IlBuilder::new(FileId(0));
    let receiver = b.add(NodeKind::Var, Payload::Cid(0), sp(142), &[]);
    let callee = b.add(
        NodeKind::Field,
        Payload::Name(interner.intern(method)),
        sp(143),
        &[receiver],
    );
    let args = (0..arg_count)
        .map(|idx| {
            b.add(
                NodeKind::Var,
                Payload::Cid(1 + idx as u32),
                sp(144 + idx as u32),
                &[],
            )
        })
        .collect::<Vec<_>>();
    let mut children = Vec::with_capacity(args.len() + 1);
    children.push(callee);
    children.extend(args);
    let call = b.add(NodeKind::Call, Payload::None, sp(146), &children);
    let root = b.add(NodeKind::Func, Payload::None, sp(147), &[call]);
    (finish_il(b, root, lang), interner, call, callee, receiver)
}

fn push_map_receiver_dependency(il: &mut Il, receiver: NodeId) {
    il.evidence.push(evidence(
        0,
        EvidenceAnchor::node(il.node(receiver).span, il.kind(receiver)),
        EvidenceKind::Domain(DomainEvidence::Map),
        EvidenceStatus::Asserted,
    ));
}

fn object_keys_call_il() -> (Il, Interner, NodeId, NodeId, NodeId) {
    let interner = Interner::new();
    let mut b = IlBuilder::new(FileId(0));
    let object_root = b.add(
        NodeKind::Var,
        Payload::Name(interner.intern("Object")),
        sp(180),
        &[],
    );
    let callee = b.add(
        NodeKind::Field,
        Payload::Name(interner.intern("keys")),
        sp(181),
        &[object_root],
    );
    let key = b.add(
        NodeKind::Lit,
        Payload::LitStr(stable_symbol_hash("red")),
        sp(182),
        &[],
    );
    let value = b.add(NodeKind::Lit, Payload::LitInt(1), sp(183), &[]);
    let pair = b.add(
        NodeKind::Seq,
        Payload::Name(interner.intern("pair")),
        sp(184),
        &[key, value],
    );
    let object = b.add(
        NodeKind::Seq,
        Payload::Name(interner.intern("object")),
        sp(185),
        &[pair],
    );
    let call = b.add(NodeKind::Call, Payload::None, sp(186), &[callee, object]);
    let root = b.add(NodeKind::Func, Payload::None, sp(187), &[call]);
    (
        finish_il(b, root, Lang::TypeScript),
        interner,
        call,
        callee,
        object,
    )
}

fn push_object_keys_dependencies(
    il: &mut Il,
    callee: NodeId,
    object_root: NodeId,
    object: NodeId,
) -> Vec<u32> {
    il.evidence.push(evidence(
        0,
        EvidenceAnchor::source_span(il.node(callee).span),
        EvidenceKind::Symbol(SymbolEvidenceKind::UnshadowedGlobal {
            name_hash: stable_symbol_hash("Object"),
        }),
        EvidenceStatus::Asserted,
    ));
    il.evidence.push(evidence_with_dependencies(
        1,
        EvidenceAnchor::node(il.node(callee).span, NodeKind::Field),
        EvidenceKind::Symbol(SymbolEvidenceKind::QualifiedGlobal {
            path_hash: stable_symbol_hash("Object.keys"),
        }),
        EvidenceStatus::Asserted,
        vec![EvidenceId(0)],
    ));
    il.evidence.push(language_core_evidence(
        2,
        EvidenceAnchor::node(il.node(object_root).span, NodeKind::Var),
        EvidenceKind::Symbol(SymbolEvidenceKind::UnshadowedGlobal {
            name_hash: stable_symbol_hash("Object"),
        }),
        EvidenceStatus::Asserted,
        Lang::TypeScript,
    ));
    il.evidence.push(language_core_evidence(
        3,
        EvidenceAnchor::sequence(il.node(object).span),
        EvidenceKind::SequenceSurface(SequenceSurfaceKind::Map),
        EvidenceStatus::Asserted,
        Lang::TypeScript,
    ));
    vec![1, 2, 3]
}

fn assert_admitted_map_key_view(lang: Lang, method: &str, expected: MapKeyViewKind) {
    let (mut il, interner, call, _callee, receiver) = map_key_view_call_il(lang, method, 0);
    push_map_receiver_dependency(&mut il, receiver);
    let contract = library_map_key_view_contract(lang, method, 0).expect("map-key-view contract");
    il.evidence.push(map_key_view_protocol_record(
        1,
        il.node(call).span,
        contract,
        EvidenceStatus::Asserted,
        &[0],
    ));

    let occurrence = admitted_map_key_view_at_call(&il, &interner, call).unwrap();
    assert_eq!(
        occurrence.contract.id,
        LibraryApiContractId::MapKeyView(expected)
    );
    assert_eq!(occurrence.contract.result.kind, expected);
    assert_eq!(occurrence.receiver, Some(receiver));
    assert_eq!(occurrence.arg_count, 0);
}

#[test]
fn admitted_object_keys_key_view_requires_object_argument_proof() {
    let contract = library_object_key_view_contract(Lang::TypeScript, "Object", "keys", 1)
        .expect("Object.keys contract");
    let (raw, interner, call, callee, _object) = object_keys_call_il();
    let object_root = raw.children(callee)[0];
    let occurrence = LibraryApiSpanCall {
        call_span: Some(raw.node(call).span),
        callee_span: Some(raw.node(callee).span),
        receiver_span: Some(raw.node(object_root).span),
        arg_count: 1,
    };
    assert!(
        admitted_object_key_view_at_call_span(
            &raw,
            &interner,
            occurrence,
            "Object",
            stable_symbol_hash("keys"),
        )
        .is_none(),
        "raw Object.keys shape is not key-view proof"
    );

    let (mut forged, interner, call, callee, object) = object_keys_call_il();
    let object_root = forged.children(callee)[0];
    let mut deps = push_object_keys_dependencies(&mut forged, callee, object_root, object);
    deps.pop();
    forged
        .evidence
        .push(library_api_record_with_provenance_and_arity(
            3,
            forged.node(call).span,
            contract.id,
            contract.callee,
            1,
            EvidenceStatus::Asserted,
            &deps,
            MAP_KEY_VIEW_PROTOCOL_PACK_ID,
            MAP_KEY_VIEW_PROTOCOL_PRODUCER_ID,
        ));
    let occurrence = LibraryApiSpanCall {
        call_span: Some(forged.node(call).span),
        callee_span: Some(forged.node(callee).span),
        receiver_span: Some(forged.node(object_root).span),
        arg_count: 1,
    };
    assert!(
        admitted_object_key_view_at_call_span(
            &forged,
            &interner,
            occurrence,
            "Object",
            stable_symbol_hash("keys"),
        )
        .is_none(),
        "Object.keys key-view evidence must include object argument proof"
    );

    let (mut admitted, interner, call, callee, object) = object_keys_call_il();
    let object_root = admitted.children(callee)[0];
    let deps = push_object_keys_dependencies(&mut admitted, callee, object_root, object);
    admitted
        .evidence
        .push(library_api_record_with_provenance_and_arity(
            3,
            admitted.node(call).span,
            contract.id,
            contract.callee,
            1,
            EvidenceStatus::Asserted,
            &deps,
            MAP_KEY_VIEW_PROTOCOL_PACK_ID,
            MAP_KEY_VIEW_PROTOCOL_PRODUCER_ID,
        ));
    let occurrence = LibraryApiSpanCall {
        call_span: Some(admitted.node(call).span),
        callee_span: Some(admitted.node(callee).span),
        receiver_span: Some(admitted.node(object_root).span),
        arg_count: 1,
    };
    let resolved = admitted_object_key_view_at_call_span(
        &admitted,
        &interner,
        occurrence,
        "Object",
        stable_symbol_hash("keys"),
    )
    .expect("Object.keys key-view should be admitted with dependencies");
    assert_eq!(
        resolved.contract.id,
        LibraryApiContractId::MapKeyView(MapKeyViewKind::Collection)
    );
}

#[test]
fn admitted_map_key_view_requires_protocol_pack_provenance() {
    let (mut raw_shape, interner, call, _callee, receiver) =
        map_key_view_call_il(Lang::TypeScript, "keys", 0);
    push_map_receiver_dependency(&mut raw_shape, receiver);
    assert!(
        admitted_map_key_view_at_call(&raw_shape, &interner, call).is_none(),
        "raw map.keys() shape plus map receiver proof is not enough"
    );

    let contract = library_map_key_view_contract(Lang::TypeScript, "keys", 0)
        .expect("TypeScript map-key-view contract");

    let (mut missing_dependency, interner, call, _callee, _receiver) =
        map_key_view_call_il(Lang::TypeScript, "keys", 0);
    missing_dependency
        .evidence
        .push(map_key_view_protocol_record(
            1,
            missing_dependency.node(call).span,
            contract,
            EvidenceStatus::Asserted,
            &[],
        ));
    assert!(
        admitted_map_key_view_at_call(&missing_dependency, &interner, call).is_none(),
        "map-key-view evidence without map receiver proof is rejected"
    );

    let (mut wrong_pack, interner, call, _callee, receiver) =
        map_key_view_call_il(Lang::TypeScript, "keys", 0);
    push_map_receiver_dependency(&mut wrong_pack, receiver);
    wrong_pack.evidence.push(library_api_record_with_provenance(
        1,
        wrong_pack.node(call).span,
        contract.id,
        contract.callee,
        EvidenceStatus::Asserted,
        &[0],
        BUILTIN_COMPAT_PACK_ID,
        MAP_KEY_VIEW_PROTOCOL_PRODUCER_ID,
    ));
    assert!(
        admitted_map_key_view_at_call(&wrong_pack, &interner, call).is_none(),
        "map-key-view evidence under the compatibility pack is rejected"
    );

    let (mut wrong_producer, interner, call, _callee, receiver) =
        map_key_view_call_il(Lang::TypeScript, "keys", 0);
    push_map_receiver_dependency(&mut wrong_producer, receiver);
    wrong_producer
        .evidence
        .push(library_api_record_with_provenance(
            1,
            wrong_producer.node(call).span,
            contract.id,
            contract.callee,
            EvidenceStatus::Asserted,
            &[0],
            MAP_KEY_VIEW_PROTOCOL_PACK_ID,
            "wrong.protocols.map-key-view-api",
        ));
    assert!(
        admitted_map_key_view_at_call(&wrong_producer, &interner, call).is_none(),
        "map-key-view evidence with the wrong producer is rejected"
    );

    let (mut wrong_emitter, interner, call, _callee, receiver) =
        map_key_view_call_il(Lang::TypeScript, "keys", 0);
    push_map_receiver_dependency(&mut wrong_emitter, receiver);
    let mut external_record = map_key_view_protocol_record(
        1,
        wrong_emitter.node(call).span,
        contract,
        EvidenceStatus::Asserted,
        &[0],
    );
    external_record.provenance.emitter = EvidenceEmitter::External;
    wrong_emitter.evidence.push(external_record);
    assert!(
        admitted_map_key_view_at_call(&wrong_emitter, &interner, call).is_none(),
        "map-key-view evidence from an external emitter is rejected"
    );

    assert_admitted_map_key_view(Lang::Python, "keys", MapKeyViewKind::Collection);
    assert_admitted_map_key_view(Lang::Ruby, "keys", MapKeyViewKind::Collection);
    assert_admitted_map_key_view(Lang::Java, "keySet", MapKeyViewKind::Collection);
    assert_admitted_map_key_view(Lang::TypeScript, "keys", MapKeyViewKind::Iterator);
}

#[test]
fn forged_map_key_view_evidence_does_not_open_unsupported_arity() {
    let contract = library_map_key_view_contract(Lang::TypeScript, "keys", 0)
        .expect("TypeScript map-key-view contract");
    let (mut unsupported_arity, interner, call, _callee, receiver) =
        map_key_view_call_il(Lang::TypeScript, "keys", 1);
    push_map_receiver_dependency(&mut unsupported_arity, receiver);
    unsupported_arity
        .evidence
        .push(map_key_view_protocol_record(
            1,
            unsupported_arity.node(call).span,
            contract,
            EvidenceStatus::Asserted,
            &[0],
        ));
    assert!(
        admitted_map_key_view_at_call(&unsupported_arity, &interner, call).is_none(),
        "forged map-key-view evidence cannot open unsupported source arity"
    );
}

#[test]
fn admitted_map_key_view_span_resolver_requires_protocol_pack_provenance() {
    let (mut raw_shape, interner, call, callee, receiver) =
        map_key_view_call_il(Lang::TypeScript, "keys", 0);
    push_map_receiver_dependency(&mut raw_shape, receiver);
    let occurrence = LibraryApiSpanCall {
        call_span: Some(raw_shape.node(call).span),
        callee_span: Some(raw_shape.node(callee).span),
        receiver_span: Some(raw_shape.node(receiver).span),
        arg_count: 0,
    };
    assert!(
        admitted_map_key_view_at_call_span(
            &raw_shape,
            &interner,
            occurrence,
            stable_symbol_hash("keys")
        )
        .is_none(),
        "raw span-backed map.keys() shape plus map receiver proof is not enough"
    );

    let contract = library_map_key_view_contract(Lang::TypeScript, "keys", 0)
        .expect("TypeScript map-key-view contract");
    let (mut admitted, interner, call, callee, receiver) =
        map_key_view_call_il(Lang::TypeScript, "keys", 0);
    push_map_receiver_dependency(&mut admitted, receiver);
    admitted.evidence.push(map_key_view_protocol_record(
        1,
        admitted.node(call).span,
        contract,
        EvidenceStatus::Asserted,
        &[0],
    ));
    let occurrence = LibraryApiSpanCall {
        call_span: Some(admitted.node(call).span),
        callee_span: Some(admitted.node(callee).span),
        receiver_span: Some(admitted.node(receiver).span),
        arg_count: 0,
    };

    let resolved = admitted_map_key_view_at_call_span(
        &admitted,
        &interner,
        occurrence,
        stable_symbol_hash("keys"),
    )
    .unwrap();
    assert_eq!(
        resolved.contract.id,
        LibraryApiContractId::MapKeyView(MapKeyViewKind::Iterator)
    );
    assert_eq!(resolved.call_span, Some(admitted.node(call).span));
    assert_eq!(resolved.callee_span, Some(admitted.node(callee).span));
    assert_eq!(resolved.receiver_span, Some(admitted.node(receiver).span));
    assert_eq!(resolved.arg_count, 0);
}
