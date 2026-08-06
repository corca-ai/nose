use super::*;

mod direct;
mod evidence;

pub(in crate::value_graph::tests) use direct::*;
pub(in crate::value_graph::tests) use evidence::*;

pub(in crate::value_graph::tests) fn promise_resolve_then_call_il(
    literal_arg: bool,
) -> (Il, Interner, NodeId, NodeId) {
    let interner = Interner::new();
    let mut b = IlBuilder::new(FileId(0));
    let promise = b.add(
        NodeKind::Var,
        Payload::Name(interner.intern("Promise")),
        sp(90),
        &[],
    );
    let resolve_callee = b.add(
        NodeKind::Field,
        Payload::Name(interner.intern("resolve")),
        sp(91),
        &[promise],
    );
    let arg = if literal_arg {
        b.add(NodeKind::Lit, Payload::LitInt(1), sp(92), &[])
    } else {
        b.add(
            NodeKind::Var,
            Payload::Name(interner.intern("maybeThenable")),
            sp(92),
            &[],
        )
    };
    let resolve_call = b.add(
        NodeKind::Call,
        Payload::None,
        sp(93),
        &[resolve_callee, arg],
    );
    let then_callee = b.add(
        NodeKind::Field,
        Payload::Name(interner.intern("then")),
        sp(94),
        &[resolve_call],
    );
    let param = b.add(NodeKind::Param, Payload::Cid(0), sp(95), &[]);
    let param_ref = b.add(NodeKind::Var, Payload::Cid(0), sp(96), &[]);
    let one = b.add(NodeKind::Lit, Payload::LitInt(1), sp(97), &[]);
    let callback_body = b.add(
        NodeKind::BinOp,
        Payload::Op(Op::Add),
        sp(98),
        &[param_ref, one],
    );
    let callback = b.add(
        NodeKind::Lambda,
        Payload::None,
        sp(99),
        &[param, callback_body],
    );
    let then_call = b.add(
        NodeKind::Call,
        Payload::None,
        sp(100),
        &[then_callee, callback],
    );
    let sync_left = b.add(NodeKind::Lit, Payload::LitInt(1), sp(101), &[]);
    let sync_right = b.add(NodeKind::Lit, Payload::LitInt(1), sp(102), &[]);
    let sync_add = b.add(
        NodeKind::BinOp,
        Payload::Op(Op::Add),
        sp(103),
        &[sync_left, sync_right],
    );
    let root = b.add(
        NodeKind::Block,
        Payload::None,
        sp(104),
        &[then_call, sync_add],
    );
    (
        finish_test_il(b, root, Lang::TypeScript),
        interner,
        then_call,
        sync_add,
    )
}

pub(in crate::value_graph::tests) fn promise_like_receiver_then_call_il() -> (Il, Interner, NodeId)
{
    let interner = Interner::new();
    let mut b = IlBuilder::new(FileId(0));
    let receiver = b.add(NodeKind::Var, Payload::Cid(0), sp(110), &[]);
    let then_callee = b.add(
        NodeKind::Field,
        Payload::Name(interner.intern("then")),
        sp(111),
        &[receiver],
    );
    let param = b.add(NodeKind::Param, Payload::Cid(1), sp(112), &[]);
    let param_ref = b.add(NodeKind::Var, Payload::Cid(1), sp(113), &[]);
    let one = b.add(NodeKind::Lit, Payload::LitInt(1), sp(114), &[]);
    let callback_body = b.add(
        NodeKind::BinOp,
        Payload::Op(Op::Add),
        sp(115),
        &[param_ref, one],
    );
    let callback = b.add(
        NodeKind::Lambda,
        Payload::None,
        sp(116),
        &[param, callback_body],
    );
    let then_call = b.add(
        NodeKind::Call,
        Payload::None,
        sp(117),
        &[then_callee, callback],
    );
    (
        finish_test_il(b, then_call, Lang::TypeScript),
        interner,
        then_call,
    )
}

pub(in crate::value_graph::tests) fn promise_reject_catch_call_il() -> (Il, Interner, NodeId, NodeId)
{
    let interner = Interner::new();
    let mut b = IlBuilder::new(FileId(0));
    let reject_call = promise_static_call(&mut b, &interner, "reject", 1, 120);
    let catch_callee = b.add(
        NodeKind::Field,
        Payload::Name(interner.intern("catch")),
        sp(124),
        &[reject_call],
    );
    let callback = add_increment_lambda(&mut b, 125, 1);
    let catch_call = b.add(
        NodeKind::Call,
        Payload::None,
        sp(130),
        &[catch_callee, callback],
    );
    let sync_add = add_sync_add(&mut b, 131);
    let root = b.add(
        NodeKind::Block,
        Payload::None,
        sp(134),
        &[catch_call, sync_add],
    );
    (
        finish_test_il(b, root, Lang::TypeScript),
        interner,
        catch_call,
        sync_add,
    )
}

pub(in crate::value_graph::tests) fn promise_reject_then_rejection_call_il(
) -> (Il, Interner, NodeId) {
    let interner = Interner::new();
    let mut b = IlBuilder::new(FileId(0));
    let reject_call = promise_static_call(&mut b, &interner, "reject", 1, 140);
    let then_callee = b.add(
        NodeKind::Field,
        Payload::Name(interner.intern("then")),
        sp(144),
        &[reject_call],
    );
    let undefined = b.add(
        NodeKind::Var,
        Payload::Name(interner.intern("undefined")),
        sp(145),
        &[],
    );
    let callback = add_increment_lambda(&mut b, 146, 1);
    let then_call = b.add(
        NodeKind::Call,
        Payload::None,
        sp(151),
        &[then_callee, undefined, callback],
    );
    (
        finish_test_il(b, then_call, Lang::TypeScript),
        interner,
        then_call,
    )
}

pub(in crate::value_graph::tests) fn promise_then_returning_factory_il(
    method: &str,
) -> (Il, Interner, NodeId) {
    let interner = Interner::new();
    let mut b = IlBuilder::new(FileId(0));
    let resolve_call = promise_static_call(&mut b, &interner, "resolve", 1, 160);
    let then_callee = b.add(
        NodeKind::Field,
        Payload::Name(interner.intern("then")),
        sp(164),
        &[resolve_call],
    );
    let param = b.add(NodeKind::Param, Payload::Cid(0), sp(165), &[]);
    let param_ref = b.add(NodeKind::Var, Payload::Cid(0), sp(166), &[]);
    let one = b.add(NodeKind::Lit, Payload::LitInt(1), sp(167), &[]);
    let sum = b.add(
        NodeKind::BinOp,
        Payload::Op(Op::Add),
        sp(168),
        &[param_ref, one],
    );
    let factory_callee = {
        let promise = b.add(
            NodeKind::Var,
            Payload::Name(interner.intern("Promise")),
            sp(169),
            &[],
        );
        b.add(
            NodeKind::Field,
            Payload::Name(interner.intern(method)),
            sp(170),
            &[promise],
        )
    };
    let factory_call = b.add(
        NodeKind::Call,
        Payload::None,
        sp(171),
        &[factory_callee, sum],
    );
    let callback = b.add(
        NodeKind::Lambda,
        Payload::None,
        sp(172),
        &[param, factory_call],
    );
    let then_call = b.add(
        NodeKind::Call,
        Payload::None,
        sp(173),
        &[then_callee, callback],
    );
    (
        finish_test_il(b, then_call, Lang::TypeScript),
        interner,
        then_call,
    )
}

pub(in crate::value_graph::tests) fn promise_then_returning_unknown_il() -> (Il, Interner, NodeId) {
    let interner = Interner::new();
    let mut b = IlBuilder::new(FileId(0));
    let resolve_call = promise_static_call(&mut b, &interner, "resolve", 1, 180);
    let then_callee = b.add(
        NodeKind::Field,
        Payload::Name(interner.intern("then")),
        sp(184),
        &[resolve_call],
    );
    let param = b.add(NodeKind::Param, Payload::Cid(0), sp(185), &[]);
    let maybe_thenable = b.add(
        NodeKind::Var,
        Payload::Name(interner.intern("maybeThenable")),
        sp(186),
        &[],
    );
    let callback = b.add(
        NodeKind::Lambda,
        Payload::None,
        sp(187),
        &[param, maybe_thenable],
    );
    let then_call = b.add(
        NodeKind::Call,
        Payload::None,
        sp(188),
        &[then_callee, callback],
    );
    (
        finish_test_il(b, then_call, Lang::TypeScript),
        interner,
        then_call,
    )
}

pub(super) fn promise_static_call(
    b: &mut IlBuilder,
    interner: &Interner,
    method: &str,
    value: i64,
    base_line: u32,
) -> NodeId {
    let promise = b.add(
        NodeKind::Var,
        Payload::Name(interner.intern("Promise")),
        sp(base_line),
        &[],
    );
    let callee = b.add(
        NodeKind::Field,
        Payload::Name(interner.intern(method)),
        sp(base_line + 1),
        &[promise],
    );
    let arg = b.add(
        NodeKind::Lit,
        Payload::LitInt(value),
        sp(base_line + 2),
        &[],
    );
    b.add(
        NodeKind::Call,
        Payload::None,
        sp(base_line + 3),
        &[callee, arg],
    )
}

pub(in crate::value_graph::tests) fn add_increment_lambda(
    b: &mut IlBuilder,
    base_line: u32,
    cid: u32,
) -> NodeId {
    let param = b.add(NodeKind::Param, Payload::Cid(cid), sp(base_line), &[]);
    let param_ref = b.add(NodeKind::Var, Payload::Cid(cid), sp(base_line + 1), &[]);
    let one = b.add(NodeKind::Lit, Payload::LitInt(1), sp(base_line + 2), &[]);
    let body = b.add(
        NodeKind::BinOp,
        Payload::Op(Op::Add),
        sp(base_line + 3),
        &[param_ref, one],
    );
    b.add(
        NodeKind::Lambda,
        Payload::None,
        sp(base_line + 4),
        &[param, body],
    )
}

pub(in crate::value_graph::tests) fn add_sync_add(b: &mut IlBuilder, base_line: u32) -> NodeId {
    let left = b.add(NodeKind::Lit, Payload::LitInt(1), sp(base_line), &[]);
    let right = b.add(NodeKind::Lit, Payload::LitInt(1), sp(base_line + 1), &[]);
    b.add(
        NodeKind::BinOp,
        Payload::Op(Op::Add),
        sp(base_line + 2),
        &[left, right],
    )
}
