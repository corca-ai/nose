use super::support::*;

#[test]
fn promise_finally_literal_handler_preserves_fulfilled_value() {
    let PromiseFinallyFixture {
        mut il,
        interner,
        producer_call,
        finally_call,
        ..
    } = promise_finally_call_il("resolve", FinallyHandlerKind::Literal);
    let mut evidence = PromiseEvidenceDsl::new(&mut il, &interner);
    evidence.factory(producer_call, PromiseSettlementChannel::Fulfilled);
    evidence.continuation(finally_call, PromiseContinuation::Finally);

    let mut builder = Builder::new(&il, &interner);
    let promise_value = builder.eval(finally_call, &FxHashMap::default());
    let payload = assert_resolved_promise_boundary(&builder, promise_value);
    assert!(matches!(
        builder.nodes[payload as usize].op,
        ValOp::Const {
            kind: ConstKind::Int,
            bits: 1
        }
    ));
}

#[test]
fn promise_finally_literal_handler_preserves_rejected_value() {
    let PromiseFinallyFixture {
        mut il,
        interner,
        producer_call,
        finally_call,
        ..
    } = promise_finally_call_il("reject", FinallyHandlerKind::Literal);
    let mut evidence = PromiseEvidenceDsl::new(&mut il, &interner);
    evidence.factory(producer_call, PromiseSettlementChannel::Rejected);
    evidence.continuation(finally_call, PromiseContinuation::Finally);

    assert!(matches!(
        eval_op(&il, &interner, finally_call),
        ValOp::Call(code) if code == PROMISE_REJECTED_CODE
    ));
}

#[test]
fn promise_finally_returning_fulfilled_promise_preserves_original_state() {
    let PromiseFinallyFixture {
        mut il,
        interner,
        producer_call,
        handler_factory_call,
        finally_call,
    } = promise_finally_call_il("resolve", FinallyHandlerKind::FulfilledPromise);
    let mut evidence = PromiseEvidenceDsl::new(&mut il, &interner);
    evidence.factory(producer_call, PromiseSettlementChannel::Fulfilled);
    evidence.factory(
        handler_factory_call.expect("finally handler Promise.resolve call"),
        PromiseSettlementChannel::Fulfilled,
    );
    evidence.continuation(finally_call, PromiseContinuation::Finally);

    let mut builder = Builder::new(&il, &interner);
    let promise_value = builder.eval(finally_call, &FxHashMap::default());
    let payload = assert_resolved_promise_boundary(&builder, promise_value);
    assert!(matches!(
        builder.nodes[payload as usize].op,
        ValOp::Const {
            kind: ConstKind::Int,
            bits: 1
        }
    ));
}

#[test]
fn promise_finally_returning_rejected_promise_overrides_channel() {
    let PromiseFinallyFixture {
        mut il,
        interner,
        producer_call,
        handler_factory_call,
        finally_call,
    } = promise_finally_call_il("resolve", FinallyHandlerKind::RejectedPromise);
    let mut evidence = PromiseEvidenceDsl::new(&mut il, &interner);
    evidence.factory(producer_call, PromiseSettlementChannel::Fulfilled);
    evidence.factory(
        handler_factory_call.expect("finally handler Promise.reject call"),
        PromiseSettlementChannel::Rejected,
    );
    evidence.continuation(finally_call, PromiseContinuation::Finally);

    assert!(matches!(
        eval_op(&il, &interner, finally_call),
        ValOp::Call(code) if code == PROMISE_REJECTED_CODE
    ));
}

#[test]
fn promise_finally_unknown_handler_result_stays_opaque() {
    let PromiseFinallyFixture {
        mut il,
        interner,
        producer_call,
        finally_call,
        ..
    } = promise_finally_call_il("resolve", FinallyHandlerKind::Unknown);
    let mut evidence = PromiseEvidenceDsl::new(&mut il, &interner);
    evidence.factory(producer_call, PromiseSettlementChannel::Fulfilled);
    evidence.continuation(finally_call, PromiseContinuation::Finally);

    assert!(!matches!(
        eval_op(&il, &interner, finally_call),
        ValOp::Call(code) if code == PROMISE_RESOLVED_CODE || code == PROMISE_REJECTED_CODE
    ));
}

#[test]
fn promise_finally_handler_with_parameter_stays_opaque() {
    let PromiseFinallyFixture {
        mut il,
        interner,
        producer_call,
        finally_call,
        ..
    } = promise_finally_call_il("resolve", FinallyHandlerKind::ParamLiteral);
    let mut evidence = PromiseEvidenceDsl::new(&mut il, &interner);
    evidence.factory(producer_call, PromiseSettlementChannel::Fulfilled);
    evidence.continuation(finally_call, PromiseContinuation::Finally);

    assert!(!matches!(
        eval_op(&il, &interner, finally_call),
        ValOp::Call(code) if code == PROMISE_RESOLVED_CODE
    ));
}
