use super::*;

pub(in crate::value_graph::tests) struct ImportedPromiseFixture {
    pub(in crate::value_graph::tests) il: Il,
    pub(in crate::value_graph::tests) interner: Interner,
    pub(in crate::value_graph::tests) producer_call: NodeId,
    pub(in crate::value_graph::tests) producer_payload: NodeId,
    pub(in crate::value_graph::tests) continuation_call: NodeId,
    pub(in crate::value_graph::tests) sync_add: NodeId,
}

pub(in crate::value_graph::tests) fn imported_promise_then_call_il(
    literal_payload: bool,
) -> ImportedPromiseFixture {
    imported_promise_continuation_call_il("then", literal_payload)
}

pub(in crate::value_graph::tests) fn imported_promise_catch_call_il() -> ImportedPromiseFixture {
    imported_promise_continuation_call_il("catch", true)
}

fn imported_promise_continuation_call_il(
    continuation: &str,
    literal_payload: bool,
) -> ImportedPromiseFixture {
    let interner = Interner::new();
    let mut b = IlBuilder::new(FileId(0));
    let producer_callee = b.add(
        NodeKind::Var,
        Payload::Name(interner.intern("load")),
        sp(118),
        &[],
    );
    let producer_payload = if literal_payload {
        b.add(NodeKind::Lit, Payload::LitInt(1), sp(119), &[])
    } else {
        b.add(
            NodeKind::Var,
            Payload::Name(interner.intern("maybeThenable")),
            sp(119),
            &[],
        )
    };
    let producer_call = b.add(
        NodeKind::Call,
        Payload::None,
        sp(120),
        &[producer_callee, producer_payload],
    );
    let continuation_callee = b.add(
        NodeKind::Field,
        Payload::Name(interner.intern(continuation)),
        sp(121),
        &[producer_call],
    );
    let callback = add_increment_lambda(&mut b, 122, 1);
    let continuation_call = b.add(
        NodeKind::Call,
        Payload::None,
        sp(127),
        &[continuation_callee, callback],
    );
    let sync_add = add_sync_add(&mut b, 128);
    let root = b.add(
        NodeKind::Block,
        Payload::None,
        sp(131),
        &[continuation_call, sync_add],
    );
    ImportedPromiseFixture {
        il: finish_test_il(b, root, Lang::TypeScript),
        interner,
        producer_call,
        producer_payload,
        continuation_call,
        sync_add,
    }
}
