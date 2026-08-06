use super::*;

pub(in crate::value_graph::tests) enum FinallyHandlerKind {
    Literal,
    FulfilledPromise,
    RejectedPromise,
    Unknown,
    ParamLiteral,
}

pub(in crate::value_graph::tests) struct PromiseFinallyFixture {
    pub(in crate::value_graph::tests) il: Il,
    pub(in crate::value_graph::tests) interner: Interner,
    pub(in crate::value_graph::tests) producer_call: NodeId,
    pub(in crate::value_graph::tests) handler_factory_call: Option<NodeId>,
    pub(in crate::value_graph::tests) finally_call: NodeId,
}

pub(in crate::value_graph::tests) fn promise_finally_call_il(
    producer_method: &str,
    handler_kind: FinallyHandlerKind,
) -> PromiseFinallyFixture {
    let interner = Interner::new();
    let mut b = IlBuilder::new(FileId(0));
    let producer_call = promise_static_call(&mut b, &interner, producer_method, 1, 460);
    let finally_callee = b.add(
        NodeKind::Field,
        Payload::Name(interner.intern("finally")),
        sp(464),
        &[producer_call],
    );
    let (handler, handler_factory_call) = finally_handler(&mut b, &interner, handler_kind, 465);
    let finally_call = b.add(
        NodeKind::Call,
        Payload::None,
        sp(472),
        &[finally_callee, handler],
    );
    let root = b.add(NodeKind::Block, Payload::None, sp(473), &[finally_call]);
    PromiseFinallyFixture {
        il: finish_test_il(b, root, Lang::TypeScript),
        interner,
        producer_call,
        handler_factory_call,
        finally_call,
    }
}

fn finally_handler(
    b: &mut IlBuilder,
    interner: &Interner,
    kind: FinallyHandlerKind,
    base_line: u32,
) -> (NodeId, Option<NodeId>) {
    let body = match kind {
        FinallyHandlerKind::Literal | FinallyHandlerKind::ParamLiteral => {
            b.add(NodeKind::Lit, Payload::LitInt(9), sp(base_line + 1), &[])
        }
        FinallyHandlerKind::FulfilledPromise => {
            promise_static_call(b, interner, "resolve", 9, base_line + 1)
        }
        FinallyHandlerKind::RejectedPromise => {
            promise_static_call(b, interner, "reject", 9, base_line + 1)
        }
        FinallyHandlerKind::Unknown => b.add(
            NodeKind::Var,
            Payload::Name(interner.intern("maybeThenable")),
            sp(base_line + 1),
            &[],
        ),
    };
    let params = if matches!(kind, FinallyHandlerKind::ParamLiteral) {
        vec![b.add(NodeKind::Param, Payload::Cid(99), sp(base_line), &[])]
    } else {
        Vec::new()
    };
    let mut children = params;
    children.push(body);
    let handler = b.add(
        NodeKind::Lambda,
        Payload::None,
        sp(base_line + 6),
        &children,
    );
    let handler_factory_call = matches!(
        kind,
        FinallyHandlerKind::FulfilledPromise | FinallyHandlerKind::RejectedPromise
    )
    .then_some(body);
    (handler, handler_factory_call)
}
