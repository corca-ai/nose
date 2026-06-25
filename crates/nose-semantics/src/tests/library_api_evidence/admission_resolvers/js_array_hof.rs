use super::*;

fn js_array_hof_call_il(method: &str, arg_count: usize) -> (Il, Interner, NodeId, NodeId) {
    let interner = Interner::new();
    let mut b = IlBuilder::new(FileId(0));
    let receiver = b.add(NodeKind::Var, Payload::Cid(0), sp(210), &[]);
    let callee = b.add(
        NodeKind::Field,
        Payload::Name(interner.intern(method)),
        sp(211),
        &[receiver],
    );
    let args = (0..arg_count)
        .map(|idx| {
            b.add(
                NodeKind::Func,
                Payload::Cid(1 + idx as u32),
                sp(212 + idx as u32),
                &[],
            )
        })
        .collect::<Vec<_>>();
    let mut children = Vec::with_capacity(args.len() + 1);
    children.push(callee);
    children.extend(args);
    let call = b.add(NodeKind::Call, Payload::None, sp(220), &children);
    let root = b.add(NodeKind::Func, Payload::None, sp(221), &[call]);
    (
        finish_il(b, root, Lang::JavaScript),
        interner,
        call,
        receiver,
    )
}

enum JsArrayCallbackShape {
    Reference,
    EffectfulInlineCall,
}

fn js_array_hof_call_with_callback_il(
    method: &str,
    callback_shape: JsArrayCallbackShape,
) -> (Il, Interner, NodeId, NodeId) {
    let interner = Interner::new();
    let mut b = IlBuilder::new(FileId(0));
    let receiver = b.add(NodeKind::Var, Payload::Cid(0), sp(240), &[]);
    let callee = b.add(
        NodeKind::Field,
        Payload::Name(interner.intern(method)),
        sp(241),
        &[receiver],
    );
    let callback = js_array_callback_node(&mut b, &interner, callback_shape, 242);
    let call = b.add(NodeKind::Call, Payload::None, sp(249), &[callee, callback]);
    let root = b.add(NodeKind::Func, Payload::None, sp(250), &[call]);
    (
        finish_il(b, root, Lang::JavaScript),
        interner,
        call,
        receiver,
    )
}

fn js_array_callback_node(
    b: &mut IlBuilder,
    interner: &Interner,
    callback_shape: JsArrayCallbackShape,
    span_base: u32,
) -> NodeId {
    match callback_shape {
        JsArrayCallbackShape::Reference => b.add(
            NodeKind::Var,
            Payload::Name(interner.intern("callback")),
            sp(span_base),
            &[],
        ),
        JsArrayCallbackShape::EffectfulInlineCall => {
            let effect_callee = b.add(
                NodeKind::Var,
                Payload::Name(interner.intern("sideEffect")),
                sp(span_base + 1),
                &[],
            );
            let effect_arg = b.add(NodeKind::Var, Payload::Cid(1), sp(span_base + 2), &[]);
            let effect = b.add(
                NodeKind::Call,
                Payload::None,
                sp(span_base + 3),
                &[effect_callee, effect_arg],
            );
            let ret = b.add(
                NodeKind::Return,
                Payload::None,
                sp(span_base + 4),
                &[effect],
            );
            let body = b.add(NodeKind::Block, Payload::None, sp(span_base + 5), &[ret]);
            b.add(NodeKind::Lambda, Payload::None, sp(span_base + 6), &[body])
        }
    }
}

fn js_array_hof_chain_with_map_callback_il(
    callback_shape: JsArrayCallbackShape,
) -> (Il, Interner, NodeId, NodeId, NodeId) {
    let interner = Interner::new();
    let mut b = IlBuilder::new(FileId(0));
    let receiver = b.add(NodeKind::Var, Payload::Cid(0), sp(260), &[]);
    let map_callee = b.add(
        NodeKind::Field,
        Payload::Name(interner.intern("map")),
        sp(261),
        &[receiver],
    );
    let map_fn = js_array_callback_node(&mut b, &interner, callback_shape, 262);
    let map_call = b.add(
        NodeKind::Call,
        Payload::None,
        sp(270),
        &[map_callee, map_fn],
    );
    let filter_callee = b.add(
        NodeKind::Field,
        Payload::Name(interner.intern("filter")),
        sp(271),
        &[map_call],
    );
    let filter_fn = b.add(NodeKind::Func, Payload::Cid(2), sp(272), &[]);
    let filter_call = b.add(
        NodeKind::Call,
        Payload::None,
        sp(273),
        &[filter_callee, filter_fn],
    );
    let root = b.add(NodeKind::Func, Payload::None, sp(274), &[filter_call]);
    (
        finish_il(b, root, Lang::JavaScript),
        interner,
        filter_call,
        map_call,
        receiver,
    )
}

fn js_array_normalized_hof_with_callback_il(
    callback_shape: JsArrayCallbackShape,
) -> (Il, NodeId, NodeId) {
    let interner = Interner::new();
    let mut b = IlBuilder::new(FileId(0));
    let receiver = b.add(NodeKind::Var, Payload::Cid(0), sp(280), &[]);
    let callback = js_array_callback_node(&mut b, &interner, callback_shape, 281);
    let hof = b.add(
        NodeKind::HoF,
        Payload::HoF(HoFKind::Map),
        sp(290),
        &[receiver, callback],
    );
    let root = b.add(NodeKind::Func, Payload::None, sp(291), &[hof]);
    (finish_il(b, root, Lang::JavaScript), hof, receiver)
}

fn js_array_normalized_nested_hof_callback_il() -> (Il, Interner, NodeId, NodeId, NodeId, NodeId) {
    let interner = Interner::new();
    let mut b = IlBuilder::new(FileId(0));
    let outer_receiver = b.add(NodeKind::Var, Payload::Cid(0), sp(300), &[]);
    let inner_receiver = b.add(NodeKind::Var, Payload::Cid(1), sp(301), &[]);
    let outer_param = b.add(NodeKind::Param, Payload::Cid(2), sp(302), &[]);
    let inner_param = b.add(NodeKind::Param, Payload::Cid(3), sp(303), &[]);
    let captured_outer = b.add(NodeKind::Var, Payload::Cid(2), sp(304), &[]);
    let inner_value = b.add(NodeKind::Var, Payload::Cid(3), sp(305), &[]);
    let sum = b.add(
        NodeKind::BinOp,
        Payload::Op(Op::Add),
        sp(306),
        &[captured_outer, inner_value],
    );
    let inner_return = b.add(NodeKind::Return, Payload::None, sp(307), &[sum]);
    let inner_body = b.add(NodeKind::Block, Payload::None, sp(308), &[inner_return]);
    let inner_lambda = b.add(
        NodeKind::Lambda,
        Payload::None,
        sp(309),
        &[inner_param, inner_body],
    );
    let inner_hof = b.add(
        NodeKind::HoF,
        Payload::HoF(HoFKind::Map),
        sp(310),
        &[inner_receiver, inner_lambda],
    );
    let outer_return = b.add(NodeKind::Return, Payload::None, sp(311), &[inner_hof]);
    let outer_body = b.add(NodeKind::Block, Payload::None, sp(312), &[outer_return]);
    let outer_lambda = b.add(
        NodeKind::Lambda,
        Payload::None,
        sp(313),
        &[outer_param, outer_body],
    );
    let outer_hof = b.add(
        NodeKind::HoF,
        Payload::HoF(HoFKind::FlatMap),
        sp(314),
        &[outer_receiver, outer_lambda],
    );
    let root = b.add(NodeKind::Func, Payload::None, sp(315), &[outer_hof]);
    (
        finish_il(b, root, Lang::JavaScript),
        interner,
        outer_hof,
        inner_hof,
        outer_receiver,
        inner_receiver,
    )
}

fn js_array_hof_chain_il() -> (Il, Interner, NodeId, NodeId, NodeId) {
    let interner = Interner::new();
    let mut b = IlBuilder::new(FileId(0));
    let receiver = b.add(NodeKind::Var, Payload::Cid(0), sp(230), &[]);
    let map_callee = b.add(
        NodeKind::Field,
        Payload::Name(interner.intern("map")),
        sp(231),
        &[receiver],
    );
    let map_fn = b.add(NodeKind::Func, Payload::Cid(1), sp(232), &[]);
    let map_call = b.add(
        NodeKind::Call,
        Payload::None,
        sp(233),
        &[map_callee, map_fn],
    );
    let filter_callee = b.add(
        NodeKind::Field,
        Payload::Name(interner.intern("filter")),
        sp(234),
        &[map_call],
    );
    let filter_fn = b.add(NodeKind::Func, Payload::Cid(2), sp(235), &[]);
    let filter_call = b.add(
        NodeKind::Call,
        Payload::None,
        sp(236),
        &[filter_callee, filter_fn],
    );
    let root = b.add(NodeKind::Func, Payload::None, sp(237), &[filter_call]);
    (
        finish_il(b, root, Lang::JavaScript),
        interner,
        filter_call,
        map_call,
        receiver,
    )
}

fn push_receiver_domain_dependency(il: &mut Il, receiver: NodeId, domain: DomainEvidence) {
    push_receiver_domain_dependency_with_id(il, 0, receiver, domain);
}

fn push_receiver_domain_dependency_with_id(
    il: &mut Il,
    id: u32,
    receiver: NodeId,
    domain: DomainEvidence,
) {
    il.evidence.push(evidence(
        id,
        EvidenceAnchor::node(il.node(receiver).span, il.kind(receiver)),
        EvidenceKind::Domain(domain),
        EvidenceStatus::Asserted,
    ));
}

fn js_array_hof_record(
    id: u32,
    il: &Il,
    call: NodeId,
    contract: LibraryMethodCallContract,
    dependencies: &[u32],
) -> EvidenceRecord {
    js_like_builtin_array_record(
        id,
        il.node(call).span,
        contract.id,
        contract.callee,
        EvidenceStatus::Asserted,
        dependencies,
    )
}

#[test]
fn admitted_js_array_hof_requires_array_pack_and_array_receiver_proof() {
    let (mut raw_shape, interner, call, receiver) = js_array_hof_call_il("map", 1);
    push_receiver_domain_dependency(&mut raw_shape, receiver, DomainEvidence::Array);
    assert!(
        admitted_library_method_call_at_call(&raw_shape, &interner, call).is_none(),
        "raw JS Array.map shape plus array receiver proof is not enough"
    );

    let contract =
        library_method_call_contract(Lang::JavaScript, "map", 1).expect("JS Array.map row");

    let (mut missing_dependency, interner, call, _receiver) = js_array_hof_call_il("map", 1);
    missing_dependency.evidence.push(js_array_hof_record(
        1,
        &missing_dependency,
        call,
        contract,
        &[],
    ));
    assert!(
        admitted_library_method_call_at_call(&missing_dependency, &interner, call).is_none(),
        "same-span JS Array HOF evidence without receiver proof is rejected"
    );

    let (mut wrong_pack, interner, call, receiver) = js_array_hof_call_il("map", 1);
    push_receiver_domain_dependency(&mut wrong_pack, receiver, DomainEvidence::Array);
    wrong_pack
        .evidence
        .push(library_api_record_with_provenance_and_arity(
            1,
            wrong_pack.node(call).span,
            contract.id,
            contract.callee,
            1,
            EvidenceStatus::Asserted,
            &[0],
            BUILTIN_METHOD_CALL_PROTOCOL_PACK_ID,
            BUILTIN_METHOD_CALL_PROTOCOL_PRODUCER_ID,
        ));
    assert!(
        admitted_library_method_call_at_call(&wrong_pack, &interner, call).is_none(),
        "JS Array HOF evidence under the generic method-call pack is rejected"
    );

    let (mut wrong_producer, interner, call, receiver) = js_array_hof_call_il("map", 1);
    push_receiver_domain_dependency(&mut wrong_producer, receiver, DomainEvidence::Array);
    wrong_producer
        .evidence
        .push(library_api_record_with_provenance_and_arity(
            1,
            wrong_producer.node(call).span,
            contract.id,
            contract.callee,
            1,
            EvidenceStatus::Asserted,
            &[0],
            JS_LIKE_BUILTIN_ARRAY_PACK_ID,
            "wrong.javascript.builtins.array-api",
        ));
    assert!(
        admitted_library_method_call_at_call(&wrong_producer, &interner, call).is_none(),
        "JS Array HOF evidence with the wrong producer is rejected"
    );

    let (mut collection_receiver, interner, call, receiver) = js_array_hof_call_il("map", 1);
    push_receiver_domain_dependency(
        &mut collection_receiver,
        receiver,
        DomainEvidence::Collection,
    );
    collection_receiver.evidence.push(js_array_hof_record(
        1,
        &collection_receiver,
        call,
        contract,
        &[0],
    ));
    assert!(
        admitted_library_method_call_at_call(&collection_receiver, &interner, call).is_none(),
        "generic collection receiver proof is not exact enough for JS Array HOFs"
    );

    let (mut admitted, interner, call, receiver) = js_array_hof_call_il("map", 1);
    push_receiver_domain_dependency(&mut admitted, receiver, DomainEvidence::Array);
    admitted
        .evidence
        .push(js_array_hof_record(1, &admitted, call, contract, &[0]));
    let occurrence = admitted_library_method_call_at_call(&admitted, &interner, call)
        .expect("JS Array.map with Array receiver proof admits");
    assert_eq!(
        occurrence.contract.id,
        LibraryApiContractId::MethodCall(MethodSemanticContract::HoF(HoFKind::Map))
    );
    assert_eq!(occurrence.contract.pack_id, JS_LIKE_BUILTIN_ARRAY_PACK_ID);
    assert_eq!(occurrence.receiver, Some(receiver));
    assert_eq!(occurrence.arg_count, 1);
}

#[test]
fn admitted_js_array_pack_covers_supported_hofs_and_terminals() {
    for (method, semantic) in [
        ("filter", MethodSemanticContract::HoF(HoFKind::Filter)),
        ("flatMap", MethodSemanticContract::HoF(HoFKind::FlatMap)),
        ("some", MethodSemanticContract::Builtin(Builtin::Any)),
        ("every", MethodSemanticContract::Builtin(Builtin::All)),
    ] {
        let (mut il, interner, call, receiver) = js_array_hof_call_il(method, 1);
        push_receiver_domain_dependency(&mut il, receiver, DomainEvidence::Array);
        let contract =
            library_method_call_contract(Lang::JavaScript, method, 1).expect("JS Array row");
        il.evidence
            .push(js_array_hof_record(1, &il, call, contract, &[0]));
        let occurrence = admitted_library_method_call_at_call(&il, &interner, call)
            .expect("supported JS Array HOF/terminal admits");
        assert_eq!(
            occurrence.contract.id,
            LibraryApiContractId::MethodCall(semantic)
        );
        assert_eq!(occurrence.contract.pack_id, JS_LIKE_BUILTIN_ARRAY_PACK_ID);
    }
}

#[test]
fn js_array_hof_pack_rejects_this_arg_and_deferred_methods() {
    let (mut this_arg, interner, call, receiver) = js_array_hof_call_il("map", 2);
    push_receiver_domain_dependency(&mut this_arg, receiver, DomainEvidence::Array);
    assert!(
        library_method_call_contract(Lang::JavaScript, "map", 2).is_none(),
        "two-argument JS Array.map remains closed until thisArg binding is modeled"
    );
    assert!(
        admitted_library_method_call_at_call(&this_arg, &interner, call).is_none(),
        "Array.map callback thisArg shape stays fail-closed"
    );

    let (mut find, interner, call, receiver) = js_array_hof_call_il("find", 1);
    push_receiver_domain_dependency(&mut find, receiver, DomainEvidence::Array);
    assert!(
        library_method_call_contract(Lang::JavaScript, "find", 1).is_none(),
        "Array.find stays closed until absence/default semantics are represented"
    );
    assert!(
        admitted_library_method_call_at_call(&find, &interner, call).is_none(),
        "Array.find evidence cannot mint a deferred JS Array HOF contract"
    );
}

#[test]
fn js_array_hof_pack_rejects_non_inline_or_effectful_callbacks() {
    let contract =
        library_method_call_contract(Lang::JavaScript, "map", 1).expect("JS Array.map row");
    for (shape, message) in [
        (
            JsArrayCallbackShape::Reference,
            "function-reference callbacks stay closed until callback effects are proven",
        ),
        (
            JsArrayCallbackShape::EffectfulInlineCall,
            "effectful inline callbacks stay closed for JS Array HOF admission",
        ),
    ] {
        let (mut il, interner, call, receiver) = js_array_hof_call_with_callback_il("map", shape);
        push_receiver_domain_dependency(&mut il, receiver, DomainEvidence::Array);
        il.evidence
            .push(js_array_hof_record(1, &il, call, contract, &[0]));
        assert!(
            admitted_library_method_call_at_call(&il, &interner, call).is_none(),
            "{message}"
        );
    }
}

#[test]
fn js_array_hof_result_domain_rechecks_callback_obligation() {
    let (mut il, interner, filter_call, map_call, receiver) =
        js_array_hof_chain_with_map_callback_il(JsArrayCallbackShape::EffectfulInlineCall);
    push_receiver_domain_dependency(&mut il, receiver, DomainEvidence::Array);
    let map_contract =
        library_method_call_contract(Lang::JavaScript, "map", 1).expect("JS Array.map row");
    il.evidence
        .push(js_array_hof_record(1, &il, map_call, map_contract, &[0]));
    let filter_contract =
        library_method_call_contract(Lang::JavaScript, "filter", 1).expect("JS Array.filter row");
    il.evidence.push(js_array_hof_record(
        2,
        &il,
        filter_call,
        filter_contract,
        &[1],
    ));

    assert!(
        admitted_library_method_call_at_call(&il, &interner, filter_call).is_none(),
        "invalid Array.map callback evidence must not prove a follow-on Array.filter receiver"
    );
}

#[test]
fn js_array_normalized_hof_rechecks_callback_obligation() {
    let (mut il, hof, receiver) =
        js_array_normalized_hof_with_callback_il(JsArrayCallbackShape::EffectfulInlineCall);
    push_receiver_domain_dependency(&mut il, receiver, DomainEvidence::Array);
    let contract =
        library_method_call_contract(Lang::JavaScript, "map", 1).expect("JS Array.map row");
    il.evidence
        .push(js_array_hof_record(1, &il, hof, contract, &[0]));

    assert!(
        !admitted_hof_api_at_node(&il, hof, HoFKind::Map),
        "normalized JS Array.map evidence must still satisfy callback obligations"
    );
}

#[test]
fn js_array_normalized_hof_allows_admitted_nested_hof_callback() {
    let (mut il, interner, outer_hof, inner_hof, outer_receiver, inner_receiver) =
        js_array_normalized_nested_hof_callback_il();
    push_receiver_domain_dependency_with_id(&mut il, 0, outer_receiver, DomainEvidence::Array);
    push_receiver_domain_dependency_with_id(&mut il, 1, inner_receiver, DomainEvidence::Array);
    let inner_contract =
        library_method_call_contract(Lang::JavaScript, "map", 1).expect("JS Array.map row");
    il.evidence
        .push(js_array_hof_record(2, &il, inner_hof, inner_contract, &[1]));
    let outer_contract =
        library_method_call_contract(Lang::JavaScript, "flatMap", 1).expect("JS Array.flatMap row");
    il.evidence
        .push(js_array_hof_record(3, &il, outer_hof, outer_contract, &[0]));

    assert!(
        admitted_hof_api_at_node_with_interner(&il, Some(&interner), inner_hof, HoFKind::Map),
        "inner JS Array.map evidence should admit the nested normalized HOF"
    );
    assert!(
        admitted_hof_api_at_node_with_interner(&il, Some(&interner), outer_hof, HoFKind::FlatMap),
        "outer JS Array.flatMap callback may contain an admitted nested JS Array HOF"
    );
}

#[test]
fn js_array_hof_result_domain_can_prove_follow_on_array_receiver() {
    let (mut il, interner, filter_call, map_call, receiver) = js_array_hof_chain_il();
    push_receiver_domain_dependency(&mut il, receiver, DomainEvidence::Array);
    let map_contract =
        library_method_call_contract(Lang::JavaScript, "map", 1).expect("JS Array.map row");
    il.evidence
        .push(js_array_hof_record(1, &il, map_call, map_contract, &[0]));
    let filter_contract =
        library_method_call_contract(Lang::JavaScript, "filter", 1).expect("JS Array.filter row");
    il.evidence.push(js_array_hof_record(
        2,
        &il,
        filter_call,
        filter_contract,
        &[1],
    ));

    let occurrence = admitted_library_method_call_at_call(&il, &interner, filter_call)
        .expect("Array.map result proves follow-on Array.filter receiver");
    assert_eq!(
        occurrence.contract.id,
        LibraryApiContractId::MethodCall(MethodSemanticContract::HoF(HoFKind::Filter))
    );
    assert_eq!(occurrence.receiver, Some(map_call));
}
