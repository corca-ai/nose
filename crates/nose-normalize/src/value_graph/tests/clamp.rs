use super::*;

#[derive(Clone, Copy)]
enum ClampShape {
    MinMax,
    SwappedBounds,
    WrongNesting,
}

#[derive(Clone, Copy)]
enum GuardShape {
    None,
    Exiting,
    NonExiting,
}

fn param(b: &mut IlBuilder, cid: u32, line: u32) -> NodeId {
    b.add(NodeKind::Param, Payload::Cid(cid), sp(line), &[])
}

fn var(b: &mut IlBuilder, cid: u32) -> NodeId {
    b.add(NodeKind::Var, Payload::Cid(cid), sp(10 + cid), &[])
}

fn int_lit(b: &mut IlBuilder, value: i64) -> NodeId {
    b.add(NodeKind::Lit, Payload::LitInt(value), sp(20), &[])
}

fn builtin(b: &mut IlBuilder, op: Builtin, args: &[NodeId]) -> NodeId {
    b.add(
        NodeKind::Call,
        Payload::Builtin(op),
        sp(30 + b.len() as u32),
        args,
    )
}

fn push_canonical_java_minmax_builtin_evidence(il: &mut Il, first_id: u32) {
    let mut next_id = first_id;
    for idx in 0..il.nodes.len() {
        let node = NodeId(idx as u32);
        let (Payload::Builtin(builtin), arg_count) =
            (il.node(node).payload, il.children(node).len())
        else {
            continue;
        };
        let method = match builtin {
            Builtin::Min => "min",
            Builtin::Max => "max",
            _ => continue,
        };
        let contract = library_method_call_contract(il.meta.lang, method, arg_count)
            .expect("min/max contract");
        il.evidence.push(library_api_contract_evidence(
            next_id,
            il.node(node).span,
            contract.id,
            contract.callee,
            arg_count as u16,
            Vec::new(),
        ));
        next_id += 1;
    }
}

fn clamp_expr(b: &mut IlBuilder, shape: ClampShape, x: NodeId, lo: NodeId, hi: NodeId) -> NodeId {
    match shape {
        ClampShape::MinMax => {
            let inner = builtin(b, Builtin::Max, &[x, lo]);
            builtin(b, Builtin::Min, &[inner, hi])
        }
        ClampShape::SwappedBounds => {
            let inner = builtin(b, Builtin::Max, &[x, hi]);
            builtin(b, Builtin::Min, &[inner, lo])
        }
        ClampShape::WrongNesting => {
            let inner = builtin(b, Builtin::Min, &[x, lo]);
            builtin(b, Builtin::Max, &[inner, hi])
        }
    }
}

fn guarded_function(
    guard: GuardShape,
    shape: ClampShape,
    semantics: [Option<ParamSemantic>; 3],
) -> (usize, usize) {
    let interner = Interner::new();
    let mut b = IlBuilder::new(FileId(0));
    let px = param(&mut b, 0, 1);
    let plo = param(&mut b, 1, 2);
    let phi = param(&mut b, 2, 3);
    let mut stmts = Vec::new();
    if !matches!(guard, GuardShape::None) {
        let hi_guard = var(&mut b, 2);
        let lo_guard = var(&mut b, 1);
        let cond = b.add(
            NodeKind::BinOp,
            Payload::Op(Op::Lt),
            sp(4),
            &[hi_guard, lo_guard],
        );
        let then_stmt = match guard {
            GuardShape::Exiting => {
                let err = int_lit(&mut b, 0);
                b.add(NodeKind::Throw, Payload::None, sp(5), &[err])
            }
            GuardShape::NonExiting => {
                let err = int_lit(&mut b, 0);
                b.add(NodeKind::ExprStmt, Payload::None, sp(5), &[err])
            }
            GuardShape::None => unreachable!(),
        };
        let then_block = b.add(NodeKind::Block, Payload::None, sp(5), &[then_stmt]);
        stmts.push(b.add(NodeKind::If, Payload::None, sp(4), &[cond, then_block]));
    }
    let x = var(&mut b, 0);
    let lo = var(&mut b, 1);
    let hi = var(&mut b, 2);
    let expr = clamp_expr(&mut b, shape, x, lo, hi);
    let ret = b.add(NodeKind::Return, Payload::None, sp(6), &[expr]);
    stmts.push(ret);
    let body = b.add(NodeKind::Block, Payload::None, sp(4), &stmts);
    let func = b.add(NodeKind::Func, Payload::None, sp(1), &[px, plo, phi, body]);
    let module = b.add(NodeKind::Module, Payload::None, sp(1), &[func]);
    let mut il = b.finish(
        module,
        FileMeta {
            path: "t.java".to_string(),
            lang: Lang::Java,
        },
        vec![Unit {
            root: func,
            kind: UnitKind::Function,
            name: None,
        }],
        Vec::new(),
    );
    for (idx, semantic) in semantics.into_iter().enumerate() {
        if let Some(semantic) = semantic {
            il.evidence.push(evidence(
                idx as u32,
                EvidenceAnchor::param(sp(idx as u32 + 1)),
                EvidenceKind::Domain(DomainEvidence::from_param_semantic(semantic)),
            ));
        }
    }
    push_canonical_java_minmax_builtin_evidence(&mut il, 100);
    let mut builder = Builder::new(&il, &interner);
    builder.build_unit(func);
    (
        builder.clamp_candidate_count,
        builder.clamp_proof_backed_candidate_count,
    )
}

fn literal_bound_function(shape: ClampShape, lo_value: i64, hi_value: i64) -> (usize, usize) {
    let interner = Interner::new();
    let mut b = IlBuilder::new(FileId(0));
    let px = param(&mut b, 0, 1);
    let x = var(&mut b, 0);
    let lo = int_lit(&mut b, lo_value);
    let hi = int_lit(&mut b, hi_value);
    let expr = clamp_expr(&mut b, shape, x, lo, hi);
    let ret = b.add(NodeKind::Return, Payload::None, sp(1), &[expr]);
    let body = b.add(NodeKind::Block, Payload::None, sp(1), &[ret]);
    let func = b.add(NodeKind::Func, Payload::None, sp(1), &[px, body]);
    let module = b.add(NodeKind::Module, Payload::None, sp(1), &[func]);
    let mut il = b.finish(
        module,
        FileMeta {
            path: "t.java".to_string(),
            lang: Lang::Java,
        },
        vec![Unit {
            root: func,
            kind: UnitKind::Function,
            name: None,
        }],
        Vec::new(),
    );
    il.evidence.push(evidence(
        0,
        EvidenceAnchor::param(sp(1)),
        EvidenceKind::Domain(DomainEvidence::Integer),
    ));
    push_canonical_java_minmax_builtin_evidence(&mut il, 100);
    let mut builder = Builder::new(&il, &interner);
    builder.build_unit(func);
    (
        builder.clamp_candidate_count,
        builder.clamp_proof_backed_candidate_count,
    )
}

#[test]
fn literal_bound_order_is_proof_backed_only_when_ordered() {
    assert_eq!(literal_bound_function(ClampShape::MinMax, 1, 10), (1, 1));
    assert_eq!(literal_bound_function(ClampShape::MinMax, 10, 1), (1, 0));
}

#[test]
fn guarded_bound_order_requires_exiting_inverse_guard() {
    let integer = Some(ParamSemantic::Integer);
    assert_eq!(
        guarded_function(GuardShape::Exiting, ClampShape::MinMax, [integer; 3]),
        (1, 1)
    );
    assert_eq!(
        guarded_function(GuardShape::NonExiting, ClampShape::MinMax, [integer; 3]),
        (1, 0)
    );
    assert_eq!(
        guarded_function(GuardShape::None, ClampShape::MinMax, [integer; 3]),
        (1, 0)
    );
}

#[test]
fn proof_rejects_floatish_number_and_wrong_shapes() {
    let integer = Some(ParamSemantic::Integer);
    let number = Some(ParamSemantic::Number);
    assert_eq!(
        guarded_function(GuardShape::Exiting, ClampShape::MinMax, [number; 3]),
        (1, 0),
        "float-sensitive Number params need a separate NaN/domain proof"
    );
    assert_eq!(
        guarded_function(GuardShape::Exiting, ClampShape::SwappedBounds, [integer; 3]),
        (1, 0)
    );
    assert_eq!(
        guarded_function(GuardShape::Exiting, ClampShape::WrongNesting, [integer; 3]),
        (1, 0)
    );
}
