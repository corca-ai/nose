use super::super::*;

pub(in crate::value_graph::tests) struct BranchingPromiseFixture {
    pub(in crate::value_graph::tests) il: Il,
    pub(in crate::value_graph::tests) interner: Interner,
    pub(in crate::value_graph::tests) resolve_calls: [NodeId; 2],
    pub(in crate::value_graph::tests) then_call: NodeId,
}

pub(in crate::value_graph::tests) fn direct_function_branching_promise_then_fixture(
    mixed_rejection: bool,
) -> BranchingPromiseFixture {
    let interner = Interner::new();
    let load = interner.intern("load");
    let mut b = IlBuilder::new(FileId(0));

    let callee_param = b.add(NodeKind::Param, Payload::Cid(0), sp(410), &[]);
    let cond = b.add(NodeKind::Var, Payload::Cid(0), sp(411), &[]);
    let then_resolve = promise_static_call(&mut b, &interner, "resolve", 1, 412);
    let then_ret = b.add(NodeKind::Return, Payload::None, sp(416), &[then_resolve]);
    let then_block = b.add(NodeKind::Block, Payload::None, sp(417), &[then_ret]);
    let else_method = if mixed_rejection { "reject" } else { "resolve" };
    let else_resolve = promise_static_call(&mut b, &interner, else_method, 2, 418);
    let else_ret = b.add(NodeKind::Return, Payload::None, sp(422), &[else_resolve]);
    let else_block = b.add(NodeKind::Block, Payload::None, sp(423), &[else_ret]);
    let branch = b.add(
        NodeKind::If,
        Payload::None,
        sp(424),
        &[cond, then_block, else_block],
    );
    let load_body = b.add(NodeKind::Block, Payload::None, sp(425), &[branch]);
    let load_root = b.add(
        NodeKind::Func,
        Payload::None,
        sp(426),
        &[callee_param, load_body],
    );

    let caller_param = b.add(NodeKind::Param, Payload::Cid(10), sp(430), &[]);
    let load_callee = b.add(NodeKind::Var, Payload::Name(load), sp(431), &[]);
    let caller_arg = b.add(NodeKind::Var, Payload::Cid(10), sp(432), &[]);
    let load_call = b.add(
        NodeKind::Call,
        Payload::None,
        sp(433),
        &[load_callee, caller_arg],
    );
    let then_callee = b.add(
        NodeKind::Field,
        Payload::Name(interner.intern("then")),
        sp(434),
        &[load_call],
    );
    let fulfilled = add_increment_lambda(&mut b, 435, 1);
    let rejected = add_increment_lambda(&mut b, 440, 2);
    let then_children = if mixed_rejection {
        vec![then_callee, fulfilled, rejected]
    } else {
        vec![then_callee, fulfilled]
    };
    let then_call = b.add(NodeKind::Call, Payload::None, sp(445), &then_children);
    let caller_ret = b.add(NodeKind::Return, Payload::None, sp(446), &[then_call]);
    let caller_body = b.add(NodeKind::Block, Payload::None, sp(447), &[caller_ret]);
    let caller_root = b.add(
        NodeKind::Func,
        Payload::None,
        sp(448),
        &[caller_param, caller_body],
    );
    let root = b.add(
        NodeKind::Module,
        Payload::None,
        sp(449),
        &[load_root, caller_root],
    );
    let il = b.finish(
        root,
        FileMeta {
            path: "t".into(),
            lang: Lang::TypeScript,
        },
        vec![
            Unit {
                root: load_root,
                kind: UnitKind::Function,
                name: Some(load),
                origin: Default::default(),
            },
            Unit {
                root: caller_root,
                kind: UnitKind::Function,
                name: Some(interner.intern("f")),
                origin: Default::default(),
            },
        ],
        Vec::new(),
    );
    BranchingPromiseFixture {
        il,
        interner,
        resolve_calls: [then_resolve, else_resolve],
        then_call,
    }
}

pub(in crate::value_graph::tests) struct DirectMethodPromiseFixture {
    pub(in crate::value_graph::tests) il: Il,
    pub(in crate::value_graph::tests) interner: Interner,
    pub(in crate::value_graph::tests) method: Symbol,
    pub(in crate::value_graph::tests) method_root: NodeId,
    pub(in crate::value_graph::tests) method_call: NodeId,
    pub(in crate::value_graph::tests) resolve_call: NodeId,
    pub(in crate::value_graph::tests) then_call: NodeId,
    pub(in crate::value_graph::tests) sync_add: NodeId,
}

pub(in crate::value_graph::tests) fn direct_method_promise_then_fixture(
    uses_receiver_context: bool,
) -> DirectMethodPromiseFixture {
    let interner = Interner::new();
    let method = interner.intern("load");
    let worker = interner.intern("worker");
    let mut b = IlBuilder::new(FileId(0));

    let promise = b.add(
        NodeKind::Var,
        Payload::Name(interner.intern("Promise")),
        sp(210),
        &[],
    );
    let resolve_callee = b.add(
        NodeKind::Field,
        Payload::Name(interner.intern("resolve")),
        sp(211),
        &[promise],
    );
    let resolve_arg = if uses_receiver_context {
        let this_value = b.add(
            NodeKind::Var,
            Payload::Name(interner.intern("this")),
            sp(212),
            &[],
        );
        b.add(
            NodeKind::Field,
            Payload::Name(interner.intern("value")),
            sp(213),
            &[this_value],
        )
    } else {
        b.add(NodeKind::Lit, Payload::LitInt(1), sp(212), &[])
    };
    let resolve_call = b.add(
        NodeKind::Call,
        Payload::None,
        sp(214),
        &[resolve_callee, resolve_arg],
    );
    let method_ret = b.add(NodeKind::Return, Payload::None, sp(215), &[resolve_call]);
    let method_body = b.add(NodeKind::Block, Payload::None, sp(216), &[method_ret]);
    let method_root = b.add(NodeKind::Func, Payload::None, sp(217), &[method_body]);

    let receiver = b.add(NodeKind::Var, Payload::Name(worker), sp(220), &[]);
    let method_callee = b.add(NodeKind::Field, Payload::Name(method), sp(221), &[receiver]);
    let method_call = b.add(NodeKind::Call, Payload::None, sp(222), &[method_callee]);
    let then_callee = b.add(
        NodeKind::Field,
        Payload::Name(interner.intern("then")),
        sp(223),
        &[method_call],
    );
    let callback = add_increment_lambda(&mut b, 224, 1);
    let then_call = b.add(
        NodeKind::Call,
        Payload::None,
        sp(229),
        &[then_callee, callback],
    );
    let sync_add = add_sync_add(&mut b, 230);
    let root = b.add(
        NodeKind::Block,
        Payload::None,
        sp(233),
        &[method_root, then_call, sync_add],
    );
    let il = b.finish(
        root,
        FileMeta {
            path: "t".into(),
            lang: Lang::TypeScript,
        },
        vec![Unit {
            root: method_root,
            kind: UnitKind::Method,
            name: Some(method),
            origin: Default::default(),
        }],
        Vec::new(),
    );
    DirectMethodPromiseFixture {
        il,
        interner,
        method,
        method_root,
        method_call,
        resolve_call,
        then_call,
        sync_add,
    }
}
