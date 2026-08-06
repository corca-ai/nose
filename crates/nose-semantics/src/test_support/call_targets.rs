use super::*;

#[derive(Clone, Copy)]
pub enum DirectFunctionFixtureScope {
    TopLevel,
    NestedTarget,
    EnclosingBinding,
}

#[derive(Clone, Copy)]
pub enum DirectFunctionFixtureSelector {
    TargetName,
    DifferentName,
}

pub struct DirectFunctionCallTargetFixture {
    pub il: Il,
    pub interner: Interner,
    pub target: NodeId,
    pub call: NodeId,
}

/// Build the shared IL coordinate contract used by call-target producers and consumers.
pub fn direct_function_call_target_test_il(
    scope: DirectFunctionFixtureScope,
    selector: DirectFunctionFixtureSelector,
) -> DirectFunctionCallTargetFixture {
    let interner = Interner::new();
    let target_name = interner.intern("f");
    let caller_name = interner.intern("g");
    let callee_name = match selector {
        DirectFunctionFixtureSelector::TargetName => target_name,
        DirectFunctionFixtureSelector::DifferentName => caller_name,
    };
    let mut builder = IlBuilder::new(FileId(0));
    let target_body = builder.add(NodeKind::Block, Payload::None, span(1), &[]);
    let target = builder.add(NodeKind::Func, Payload::None, span(2), &[target_body]);

    let (root, call, units) = match scope {
        DirectFunctionFixtureScope::TopLevel => {
            let call = direct_call(&mut builder, callee_name, 3);
            let root = builder.add(NodeKind::Module, Payload::None, span(6), &[target, call]);
            (root, call, Vec::new())
        }
        DirectFunctionFixtureScope::NestedTarget => {
            let call = direct_call(&mut builder, callee_name, 3);
            let ret = builder.add(NodeKind::Return, Payload::None, span(5), &[call]);
            let body = builder.add(NodeKind::Block, Payload::None, span(6), &[target, ret]);
            let outer = builder.add(NodeKind::Func, Payload::None, span(7), &[body]);
            let root = builder.add(NodeKind::Module, Payload::None, span(8), &[outer]);
            (
                root,
                call,
                vec![Unit {
                    root: target,
                    kind: UnitKind::Function,
                    name: Some(target_name),
                    origin: Default::default(),
                }],
            )
        }
        DirectFunctionFixtureScope::EnclosingBinding => {
            let shadow_lhs = builder.add(NodeKind::Var, Payload::Name(target_name), span(3), &[]);
            let shadow_rhs = builder.add(NodeKind::Lit, Payload::LitInt(1), span(4), &[]);
            let shadow = builder.add(
                NodeKind::Assign,
                Payload::None,
                span(5),
                &[shadow_lhs, shadow_rhs],
            );
            let call = direct_call(&mut builder, callee_name, 6);
            let ret = builder.add(NodeKind::Return, Payload::None, span(8), &[call]);
            let inner_body = builder.add(NodeKind::Block, Payload::None, span(9), &[ret]);
            let inner = builder.add(NodeKind::Func, Payload::None, span(10), &[inner_body]);
            let outer_body =
                builder.add(NodeKind::Block, Payload::None, span(11), &[shadow, inner]);
            let outer = builder.add(NodeKind::Func, Payload::None, span(12), &[outer_body]);
            let root = builder.add(NodeKind::Module, Payload::None, span(13), &[target, outer]);
            (
                root,
                call,
                vec![
                    Unit {
                        root: target,
                        kind: UnitKind::Function,
                        name: Some(target_name),
                        origin: Default::default(),
                    },
                    Unit {
                        root: outer,
                        kind: UnitKind::Function,
                        name: Some(caller_name),
                        origin: Default::default(),
                    },
                ],
            )
        }
    };
    let il = builder.finish(
        root,
        FileMeta {
            path: "call-target-fixture.py".into(),
            lang: Lang::Python,
        },
        units,
        Vec::new(),
    );
    DirectFunctionCallTargetFixture {
        il,
        interner,
        target,
        call,
    }
}

fn direct_call(builder: &mut IlBuilder, callee_name: nose_il::Symbol, line: u32) -> NodeId {
    let callee = builder.add(NodeKind::Var, Payload::Name(callee_name), span(line), &[]);
    builder.add(NodeKind::Call, Payload::None, span(line + 1), &[callee])
}

fn span(line: u32) -> Span {
    Span::new(FileId(0), line, line + 1, line, line)
}
