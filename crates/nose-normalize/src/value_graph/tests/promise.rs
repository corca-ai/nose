use super::support::*;

#[test]
fn promise_then_over_resolve_reduces_behind_promise_boundary() {
    let (mut il, interner, then_call, sync_add) = promise_resolve_then_call_il(true);
    let resolve_call = il.children(il.children(then_call)[0])[0];
    let mut evidence = PromiseEvidenceDsl::new(&mut il, &interner);
    evidence.factory(resolve_call, PromiseSettlementChannel::Fulfilled);
    evidence.continuation(then_call, PromiseContinuation::Then);

    let mut builder = Builder::new(&il, &interner);
    let promise_value = builder.eval(then_call, &FxHashMap::default());
    let payload = {
        let node = &builder.nodes[promise_value as usize];
        assert!(
            matches!(
                node.op,
                ValOp::Call(code) if code == PROMISE_RESOLVED_CODE
            ),
            "expected resolved Promise boundary, got {}",
            val_op_name(&node.op)
        );
        *node
            .args
            .first()
            .expect("Promise boundary wraps one payload")
    };
    assert!(matches!(
        builder.nodes[payload as usize].op,
        ValOp::Bin(op) if op == Op::Add as u32
    ));

    let sync_value = builder.eval(sync_add, &FxHashMap::default());
    assert_eq!(payload, sync_value);
    assert_ne!(
        promise_value, sync_value,
        "Promise-returning continuation must not converge with a synchronous payload"
    );
}

#[test]
fn promise_then_returning_resolve_flattens_into_single_promise_boundary() {
    let (mut il, interner, then_call) = promise_then_returning_factory_il("resolve");
    let resolve_call = il.children(il.children(then_call)[0])[0];
    let callback = il.children(then_call)[1];
    let returned_resolve_call = il.children(callback)[1];
    let mut evidence = PromiseEvidenceDsl::new(&mut il, &interner);
    evidence.factory(resolve_call, PromiseSettlementChannel::Fulfilled);
    evidence.factory(returned_resolve_call, PromiseSettlementChannel::Fulfilled);
    evidence.continuation(then_call, PromiseContinuation::Then);

    let mut builder = Builder::new(&il, &interner);
    let promise_value = builder.eval(then_call, &FxHashMap::default());
    let node = &builder.nodes[promise_value as usize];
    assert!(matches!(
        node.op,
        ValOp::Call(code) if code == PROMISE_RESOLVED_CODE
    ));
    let payload = *node.args.first().expect("Promise boundary wraps payload");
    assert!(
        !matches!(
            builder.nodes[payload as usize].op,
            ValOp::Call(code) if code == PROMISE_RESOLVED_CODE
        ),
        "handler-returned Promise.resolve must be assimilated rather than nested"
    );
    assert!(matches!(
        builder.nodes[payload as usize].op,
        ValOp::Bin(op) if op == Op::Add as u32
    ));
}

#[test]
fn promise_reject_catch_recovers_rejection_to_fulfilled_boundary() {
    let (mut il, interner, catch_call, sync_add) = promise_reject_catch_call_il();
    let reject_call = il.children(il.children(catch_call)[0])[0];
    let mut evidence = PromiseEvidenceDsl::new(&mut il, &interner);
    evidence.factory(reject_call, PromiseSettlementChannel::Rejected);
    evidence.continuation(catch_call, PromiseContinuation::Catch);
    assert!(
        nose_semantics::admitted_promise_resolve_at_call(&il, &interner, reject_call).is_some(),
        "Promise.reject factory evidence should admit the rejected channel"
    );
    assert!(
        nose_semantics::admitted_promise_catch_at_call(&il, &interner, catch_call).is_some(),
        "Promise.catch continuation evidence should admit the recovery channel"
    );

    let mut builder = Builder::new(&il, &interner);
    let promise_value = builder.eval(catch_call, &FxHashMap::default());
    let payload = {
        let node = &builder.nodes[promise_value as usize];
        assert!(
            matches!(
                node.op,
                ValOp::Call(code) if code == PROMISE_RESOLVED_CODE
            ),
            "expected resolved Promise boundary, got {}",
            val_op_name(&node.op)
        );
        *node.args.first().expect("Promise boundary wraps payload")
    };

    let sync_value = builder.eval(sync_add, &FxHashMap::default());
    assert_eq!(payload, sync_value);
    assert_ne!(
        promise_value, sync_value,
        "recovered catch result must remain behind a Promise boundary"
    );
}

#[test]
fn promise_reject_then_rejection_handler_recovers_like_catch() {
    let (mut il, interner, then_call) = promise_reject_then_rejection_call_il();
    let reject_call = il.children(il.children(then_call)[0])[0];
    let mut evidence = PromiseEvidenceDsl::new(&mut il, &interner);
    evidence.factory(reject_call, PromiseSettlementChannel::Rejected);
    evidence.continuation(then_call, PromiseContinuation::Then);

    assert!(matches!(
        eval_op(&il, &interner, then_call),
        ValOp::Call(code) if code == PROMISE_RESOLVED_CODE
    ));
}

#[test]
fn promise_then_returning_reject_preserves_rejection_channel() {
    let (mut il, interner, then_call) = promise_then_returning_factory_il("reject");
    let resolve_call = il.children(il.children(then_call)[0])[0];
    let callback = il.children(then_call)[1];
    let returned_reject_call = il.children(callback)[1];
    let mut evidence = PromiseEvidenceDsl::new(&mut il, &interner);
    evidence.factory(resolve_call, PromiseSettlementChannel::Fulfilled);
    evidence.factory(returned_reject_call, PromiseSettlementChannel::Rejected);
    evidence.continuation(then_call, PromiseContinuation::Then);

    assert!(matches!(
        eval_op(&il, &interner, then_call),
        ValOp::Call(code) if code == PROMISE_REJECTED_CODE
    ));
}

#[test]
fn promise_then_returning_possible_thenable_stays_opaque() {
    let (mut il, interner, then_call) = promise_then_returning_unknown_il();
    let resolve_call = il.children(il.children(then_call)[0])[0];
    let mut evidence = PromiseEvidenceDsl::new(&mut il, &interner);
    evidence.factory(resolve_call, PromiseSettlementChannel::Fulfilled);
    evidence.continuation(then_call, PromiseContinuation::Then);

    assert!(!matches!(
        eval_op(&il, &interner, then_call),
        ValOp::Call(code) if code == PROMISE_RESOLVED_CODE || code == PROMISE_REJECTED_CODE
    ));
}

#[test]
fn promise_then_over_possible_thenable_resolve_arg_stays_opaque() {
    let (mut il, interner, then_call, _sync_add) = promise_resolve_then_call_il(false);
    let resolve_call = il.children(il.children(then_call)[0])[0];
    let mut evidence = PromiseEvidenceDsl::new(&mut il, &interner);
    evidence.factory(resolve_call, PromiseSettlementChannel::Fulfilled);
    evidence.continuation(then_call, PromiseContinuation::Then);

    assert!(!matches!(
        eval_op(&il, &interner, then_call),
        ValOp::Call(code) if code == PROMISE_RESOLVED_CODE
    ));
}

#[test]
fn promise_then_over_explicit_thenable_resolve_arg_stays_opaque() {
    let (mut il, interner, then_call, _sync_add) = promise_resolve_then_call_il(false);
    let resolve_call = il.children(il.children(then_call)[0])[0];
    let resolve_arg = il.children(resolve_call)[1];
    let mut evidence = PromiseEvidenceDsl::new(&mut il, &interner);
    evidence.promise_like(resolve_arg);
    evidence.factory(resolve_call, PromiseSettlementChannel::Fulfilled);
    evidence.continuation(then_call, PromiseContinuation::Then);

    assert!(!matches!(
        eval_op(&il, &interner, then_call),
        ValOp::Call(code) if code == PROMISE_RESOLVED_CODE
    ));
}

#[test]
fn promise_like_receiver_without_supported_settled_producer_stays_opaque() {
    let (mut il, interner, then_call) = promise_like_receiver_then_call_il();
    let then_callee = il.children(then_call)[0];
    let receiver = il.children(then_callee)[0];
    let mut evidence = PromiseEvidenceDsl::new(&mut il, &interner);
    evidence.promise_like(receiver);
    evidence.continuation(then_call, PromiseContinuation::Then);

    assert!(!matches!(
        eval_op(&il, &interner, then_call),
        ValOp::Call(code) if code == PROMISE_RESOLVED_CODE
    ));
}

#[test]
fn imported_promise_then_with_fulfilled_contract_recovers_payload_boundary() {
    let ImportedPromiseFixture {
        mut il,
        interner,
        producer_call,
        producer_payload,
        continuation_call,
        sync_add,
    } = imported_promise_then_call_il(true);
    let mut evidence = PromiseEvidenceDsl::new(&mut il, &interner);
    evidence.imported_settlement(
        producer_call,
        producer_payload,
        PromiseSettlementChannel::Fulfilled,
    );
    evidence.continuation(continuation_call, PromiseContinuation::Then);

    let mut builder = Builder::new(&il, &interner);
    let promise_value = builder.eval(continuation_call, &FxHashMap::default());
    let payload = assert_resolved_promise_boundary(&builder, promise_value);
    let sync_value = builder.eval(sync_add, &FxHashMap::default());
    assert_eq!(payload, sync_value);
    assert_ne!(
        promise_value, sync_value,
        "imported Promise recovery must preserve the async boundary"
    );
}

#[test]
fn imported_promise_catch_with_rejected_contract_recovers_payload_boundary() {
    let ImportedPromiseFixture {
        mut il,
        interner,
        producer_call,
        producer_payload,
        continuation_call,
        sync_add,
    } = imported_promise_catch_call_il();
    let mut evidence = PromiseEvidenceDsl::new(&mut il, &interner);
    evidence.imported_settlement(
        producer_call,
        producer_payload,
        PromiseSettlementChannel::Rejected,
    );
    evidence.continuation(continuation_call, PromiseContinuation::Catch);

    let mut builder = Builder::new(&il, &interner);
    let promise_value = builder.eval(continuation_call, &FxHashMap::default());
    let payload = assert_resolved_promise_boundary(&builder, promise_value);
    let sync_value = builder.eval(sync_add, &FxHashMap::default());
    assert_eq!(payload, sync_value);
}

#[test]
fn imported_promise_then_without_settled_contract_stays_opaque() {
    let ImportedPromiseFixture {
        mut il,
        interner,
        producer_call,
        continuation_call,
        ..
    } = imported_promise_then_call_il(true);
    let mut evidence = PromiseEvidenceDsl::new(&mut il, &interner);
    evidence.promise_like(producer_call);
    evidence.continuation(continuation_call, PromiseContinuation::Then);

    assert!(!matches!(
        eval_op(&il, &interner, continuation_call),
        ValOp::Call(code) if code == PROMISE_RESOLVED_CODE || code == PROMISE_REJECTED_CODE
    ));
}

#[test]
fn imported_promise_fulfilled_contract_with_possible_thenable_payload_stays_opaque() {
    let ImportedPromiseFixture {
        mut il,
        interner,
        producer_call,
        producer_payload,
        continuation_call,
        ..
    } = imported_promise_then_call_il(false);
    let mut evidence = PromiseEvidenceDsl::new(&mut il, &interner);
    evidence.imported_settlement(
        producer_call,
        producer_payload,
        PromiseSettlementChannel::Fulfilled,
    );
    evidence.continuation(continuation_call, PromiseContinuation::Then);

    assert!(!matches!(
        eval_op(&il, &interner, continuation_call),
        ValOp::Call(code) if code == PROMISE_RESOLVED_CODE || code == PROMISE_REJECTED_CODE
    ));
}

#[test]
fn direct_method_promise_return_then_recovers_without_sync_erasure() {
    let DirectMethodPromiseFixture {
        mut il,
        interner,
        method,
        method_call,
        method_root,
        resolve_call,
        sync_add,
        then_call,
    } = direct_method_promise_then_fixture(false);
    il.push_evidence(language_core_evidence(
        100,
        Lang::TypeScript,
        EvidenceAnchor::node(il.node(method_call).span, NodeKind::Call),
        EvidenceKind::CallTarget(CallTargetEvidenceKind::DirectMethod {
            target_span: il.node(method_root).span,
            receiver_type_hash: stable_symbol_hash("Worker"),
            method_hash: interner.symbol_hash(method),
        }),
    ));
    PromiseEvidenceDsl::new(&mut il, &interner)
        .factory(resolve_call, PromiseSettlementChannel::Fulfilled);
    crate::call_target_evidence::run(&mut il, &interner);
    assert_eq!(
        nose_semantics::domain_evidence_for_receiver(&il, &interner, method_call),
        Some(DomainEvidence::PromiseLike),
        "direct method call result should gain PromiseLike receiver proof"
    );
    PromiseEvidenceDsl::new(&mut il, &interner).continuation(then_call, PromiseContinuation::Then);

    let mut builder = Builder::new(&il, &interner);
    let promise_value = builder.eval(then_call, &FxHashMap::default());
    let payload = {
        let node = &builder.nodes[promise_value as usize];
        assert!(
            matches!(
                node.op,
                ValOp::Call(code) if code == PROMISE_RESOLVED_CODE
            ),
            "expected resolved Promise boundary, got {}",
            val_op_name(&node.op)
        );
        *node.args.first().expect("Promise boundary wraps payload")
    };
    let sync_value = builder.eval(sync_add, &FxHashMap::default());
    assert_eq!(payload, sync_value);
    assert_ne!(
        promise_value, sync_value,
        "direct method Promise return recovery must preserve the Promise boundary"
    );
}

#[test]
fn direct_method_promise_return_stays_closed_when_return_uses_receiver_context() {
    let DirectMethodPromiseFixture {
        mut il,
        interner,
        method,
        method_call,
        method_root,
        resolve_call,
        then_call,
        ..
    } = direct_method_promise_then_fixture(true);
    il.push_evidence(language_core_evidence(
        100,
        Lang::TypeScript,
        EvidenceAnchor::node(il.node(method_call).span, NodeKind::Call),
        EvidenceKind::CallTarget(CallTargetEvidenceKind::DirectMethod {
            target_span: il.node(method_root).span,
            receiver_type_hash: stable_symbol_hash("Worker"),
            method_hash: interner.symbol_hash(method),
        }),
    ));
    let resolve_arg = il.children(resolve_call)[1];
    let mut evidence = PromiseEvidenceDsl::new(&mut il, &interner);
    evidence.domain(resolve_arg, DomainEvidence::Number);
    evidence.factory(resolve_call, PromiseSettlementChannel::Fulfilled);
    crate::call_target_evidence::run(&mut il, &interner);
    PromiseEvidenceDsl::new(&mut il, &interner).continuation(then_call, PromiseContinuation::Then);

    assert!(
        !matches!(
            eval_op(&il, &interner, then_call),
            ValOp::Call(code) if code == PROMISE_RESOLVED_CODE
        ),
        "DirectMethod return recovery must not evaluate methods that depend on receiver context"
    );
}

#[test]
fn direct_function_branching_promise_returns_recover_fulfilled_channel() {
    let BranchingPromiseFixture {
        mut il,
        interner,
        resolve_calls,
        then_call,
    } = direct_function_branching_promise_then_fixture(false);
    assert_branch_resolve_evidence_admits(&mut il, &interner, &resolve_calls);
    crate::call_target_evidence::run(&mut il, &interner);
    assert_branch_resolve_calls_remain_admitted(&il, &interner, &resolve_calls);
    let receiver = il.children(il.children(then_call)[0])[0];
    assert_eq!(
        nose_semantics::domain_evidence_for_receiver(&il, &interner, receiver),
        Some(DomainEvidence::PromiseLike),
        "branching direct function call result should gain PromiseLike receiver proof"
    );
    PromiseEvidenceDsl::new(&mut il, &interner).continuation(then_call, PromiseContinuation::Then);
    assert!(
        nose_semantics::admitted_promise_then_at_call(&il, &interner, then_call).is_some(),
        "branching direct function receiver should admit Promise.then evidence"
    );

    assert_branching_direct_body_evaluates_to_resolved_phi(&il, &interner, receiver);
    assert_then_call_recovers_resolved_add_boundary(&il, &interner, then_call);
}

#[test]
fn direct_function_mixed_fulfilled_rejected_branch_stays_closed() {
    let BranchingPromiseFixture {
        mut il,
        interner,
        resolve_calls,
        then_call,
    } = direct_function_branching_promise_then_fixture(true);
    let mut evidence = PromiseEvidenceDsl::new(&mut il, &interner);
    evidence.factory(resolve_calls[0], PromiseSettlementChannel::Fulfilled);
    evidence.factory(resolve_calls[1], PromiseSettlementChannel::Rejected);
    crate::call_target_evidence::run(&mut il, &interner);
    PromiseEvidenceDsl::new(&mut il, &interner).continuation(then_call, PromiseContinuation::Then);

    assert!(
        !matches!(
            eval_op(&il, &interner, then_call),
            ValOp::Call(code) if code == PROMISE_RESOLVED_CODE || code == PROMISE_REJECTED_CODE
        ),
        "mixed fulfilled/rejected producer branches need channel-specific control-flow proof"
    );
}

fn assert_branch_resolve_evidence_admits(
    il: &mut Il,
    interner: &Interner,
    resolve_calls: &[NodeId; 2],
) {
    for &resolve_call in resolve_calls {
        PromiseEvidenceDsl::new(il, interner)
            .factory(resolve_call, PromiseSettlementChannel::Fulfilled);
        assert!(
            nose_semantics::admitted_promise_resolve_at_call(il, interner, resolve_call).is_some(),
            "branch Promise.resolve call should admit factory evidence"
        );
    }
}

fn assert_branch_resolve_calls_remain_admitted(
    il: &Il,
    interner: &Interner,
    resolve_calls: &[NodeId; 2],
) {
    for &resolve_call in resolve_calls {
        assert!(
            nose_semantics::admitted_promise_resolve_at_call(il, interner, resolve_call).is_some(),
            "branch Promise.resolve call should remain admitted after call-target evidence"
        );
    }
}

fn assert_branching_direct_body_evaluates_to_resolved_phi(
    il: &Il,
    interner: &Interner,
    receiver: NodeId,
) {
    let mut builder = Builder::new(il, interner);
    let receiver_value = builder
        .eval_direct_function_return_call(receiver, &FxHashMap::default())
        .expect("branching direct function body should evaluate behind the sink fence");
    assert!(
        matches!(builder.nodes[receiver_value as usize].op, ValOp::Phi),
        "branching direct function producer should evaluate to a Phi of Promise boundaries"
    );
    let branch_values = builder.nodes[receiver_value as usize].args.clone();
    assert_resolved_promise_boundary(&builder, branch_values[1]);
    assert_resolved_promise_boundary(&builder, branch_values[2]);
}

fn assert_then_call_recovers_resolved_add_boundary(
    il: &Il,
    interner: &Interner,
    then_call: NodeId,
) {
    let mut builder = Builder::new(il, interner);
    let promise_value = builder.eval(then_call, &FxHashMap::default());
    let payload = assert_resolved_promise_boundary(&builder, promise_value);
    assert!(
        matches!(builder.nodes[payload as usize].op, ValOp::Bin(op) if op == Op::Add as u32),
        "fulfilled branch payloads should flow through the continuation"
    );
    assert_ne!(
        promise_value, payload,
        "branching Promise continuation recovery must preserve the Promise boundary"
    );
}
