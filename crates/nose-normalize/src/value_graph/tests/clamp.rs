use super::support::*;

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

#[derive(Clone, Copy)]
enum BoundOrderEvidenceMode {
    None,
    Valid,
    SwappedOperands,
    WrongActivation,
    Ambiguous,
    MissingDependency,
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
        let contract = library_scalar_integer_method_contract(il.meta.lang, method, arg_count)
            .expect("min/max integer contract");
        let math_id = next_id;
        next_id += 1;
        il.push_evidence(language_core_symbol_evidence(
            math_id,
            il.meta.lang,
            EvidenceAnchor::node(il.node(node).span, NodeKind::Var),
            SymbolEvidenceKind::UnshadowedGlobal {
                name_hash: stable_symbol_hash("Math"),
            },
        ));
        let mut dependencies = vec![EvidenceId(math_id)];
        let args = il.children(node).to_vec();
        for arg in args {
            if matches!(il.node(arg).payload, Payload::LitInt(_)) {
                continue;
            }
            let arg_id = next_id;
            next_id += 1;
            il.push_evidence(evidence(
                arg_id,
                EvidenceAnchor::node(il.node(arg).span, il.kind(arg)),
                EvidenceKind::Domain(DomainEvidence::Integer),
            ));
            dependencies.push(EvidenceId(arg_id));
        }
        il.push_evidence(java_stdlib_math_evidence(
            next_id,
            il.node(node).span,
            contract.id,
            contract.callee,
            arg_count as u16,
            dependencies,
        ));
        next_id += 1;
    }
}

fn push_bound_order_guard_evidence(
    il: &mut Il,
    id: u32,
    cond: NodeId,
    lower: NodeId,
    upper: NodeId,
    activation: BoundOrderGuardActivation,
    dependencies: Vec<EvidenceId>,
) {
    il.push_evidence(language_core_evidence_with_dependencies(
        id,
        il.meta.lang,
        EvidenceAnchor::node(il.node(cond).span, NodeKind::BinOp),
        EvidenceKind::Guard(GuardEvidenceKind::BoundOrder {
            lower_span: il.node(lower).span,
            upper_span: il.node(upper).span,
            activation,
        }),
        dependencies,
    ));
}

fn opposite_activation(activation: BoundOrderGuardActivation) -> BoundOrderGuardActivation {
    match activation {
        BoundOrderGuardActivation::WhenTrue => BoundOrderGuardActivation::WhenFalse,
        BoundOrderGuardActivation::WhenFalse => BoundOrderGuardActivation::WhenTrue,
    }
}

fn push_bound_order_evidence_mode(
    il: &mut Il,
    mode: BoundOrderEvidenceMode,
    cond: NodeId,
    lower: NodeId,
    upper: NodeId,
    valid_activation: BoundOrderGuardActivation,
) {
    match mode {
        BoundOrderEvidenceMode::None => {}
        BoundOrderEvidenceMode::Valid => push_bound_order_guard_evidence(
            il,
            50,
            cond,
            lower,
            upper,
            valid_activation,
            Vec::new(),
        ),
        BoundOrderEvidenceMode::SwappedOperands => push_bound_order_guard_evidence(
            il,
            50,
            cond,
            upper,
            lower,
            valid_activation,
            Vec::new(),
        ),
        BoundOrderEvidenceMode::WrongActivation => push_bound_order_guard_evidence(
            il,
            50,
            cond,
            lower,
            upper,
            opposite_activation(valid_activation),
            Vec::new(),
        ),
        BoundOrderEvidenceMode::Ambiguous => {
            push_bound_order_guard_evidence(
                il,
                50,
                cond,
                lower,
                upper,
                valid_activation,
                Vec::new(),
            );
            push_bound_order_guard_evidence(
                il,
                51,
                cond,
                upper,
                lower,
                valid_activation,
                Vec::new(),
            );
        }
        BoundOrderEvidenceMode::MissingDependency => push_bound_order_guard_evidence(
            il,
            50,
            cond,
            lower,
            upper,
            valid_activation,
            vec![EvidenceId(999)],
        ),
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
    evidence_mode: BoundOrderEvidenceMode,
) -> (usize, usize) {
    let interner = Interner::new();
    let mut b = IlBuilder::new(FileId(0));
    let px = param(&mut b, 0, 1);
    let plo = param(&mut b, 1, 2);
    let phi = param(&mut b, 2, 3);
    let mut stmts = Vec::new();
    let mut guard_evidence_nodes = None;
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
        guard_evidence_nodes = Some((cond, lo_guard, hi_guard));
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
            origin: Default::default(),
        }],
        Vec::new(),
    );
    for (idx, semantic) in semantics.into_iter().enumerate() {
        if let Some(semantic) = semantic {
            il.push_evidence(evidence(
                idx as u32,
                EvidenceAnchor::param(sp(idx as u32 + 1)),
                EvidenceKind::Domain(DomainEvidence::from_param_semantic(semantic)),
            ));
        }
    }
    if let Some((cond, lo_guard, hi_guard)) = guard_evidence_nodes {
        push_bound_order_evidence_mode(
            &mut il,
            evidence_mode,
            cond,
            lo_guard,
            hi_guard,
            BoundOrderGuardActivation::WhenFalse,
        );
    }
    push_canonical_java_minmax_builtin_evidence(&mut il, 100);
    let mut builder = Builder::new(&il, &interner);
    builder.build_unit(func);
    (
        builder.clamp_candidate_count,
        builder.clamp_proof_backed_candidate_count,
    )
}

fn positive_branch_guarded_function(
    semantics: [Option<ParamSemantic>; 3],
    evidence_mode: BoundOrderEvidenceMode,
) -> (usize, usize) {
    let interner = Interner::new();
    let mut b = IlBuilder::new(FileId(0));
    let px = param(&mut b, 0, 1);
    let plo = param(&mut b, 1, 2);
    let phi = param(&mut b, 2, 3);
    let lo_guard = var(&mut b, 1);
    let hi_guard = var(&mut b, 2);
    let cond = b.add(
        NodeKind::BinOp,
        Payload::Op(Op::Le),
        sp(4),
        &[lo_guard, hi_guard],
    );
    let x = var(&mut b, 0);
    let lo = var(&mut b, 1);
    let hi = var(&mut b, 2);
    let expr = clamp_expr(&mut b, ClampShape::MinMax, x, lo, hi);
    let ret = b.add(NodeKind::Return, Payload::None, sp(5), &[expr]);
    let then_block = b.add(NodeKind::Block, Payload::None, sp(5), &[ret]);
    let err = int_lit(&mut b, 0);
    let throw = b.add(NodeKind::Throw, Payload::None, sp(6), &[err]);
    let else_block = b.add(NodeKind::Block, Payload::None, sp(6), &[throw]);
    let if_stmt = b.add(
        NodeKind::If,
        Payload::None,
        sp(4),
        &[cond, then_block, else_block],
    );
    let body = b.add(NodeKind::Block, Payload::None, sp(4), &[if_stmt]);
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
            origin: Default::default(),
        }],
        Vec::new(),
    );
    for (idx, semantic) in semantics.into_iter().enumerate() {
        if let Some(semantic) = semantic {
            il.push_evidence(evidence(
                idx as u32,
                EvidenceAnchor::param(sp(idx as u32 + 1)),
                EvidenceKind::Domain(DomainEvidence::from_param_semantic(semantic)),
            ));
        }
    }
    push_bound_order_evidence_mode(
        &mut il,
        evidence_mode,
        cond,
        lo_guard,
        hi_guard,
        BoundOrderGuardActivation::WhenTrue,
    );
    push_canonical_java_minmax_builtin_evidence(&mut il, 100);
    let mut builder = Builder::new(&il, &interner);
    builder.build_unit(func);
    (
        builder.clamp_candidate_count,
        builder.clamp_proof_backed_candidate_count,
    )
}

fn literal_bound_function(
    shape: ClampShape,
    lo_value: i64,
    hi_value: i64,
) -> (usize, usize, Vec<ValueLaw>) {
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
            origin: Default::default(),
        }],
        Vec::new(),
    );
    il.push_evidence(evidence(
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
        builder.value_laws,
    )
}

#[test]
fn literal_bound_order_is_proof_backed_only_when_ordered() {
    assert_eq!(
        literal_bound_function(ClampShape::MinMax, 1, 10),
        (1, 1, vec![ValueLaw::IntegerClampOrderedMinMax])
    );
    assert_eq!(
        literal_bound_function(ClampShape::MinMax, 10, 1),
        (1, 0, Vec::new())
    );
}

#[test]
fn guarded_bound_order_requires_asserted_exiting_inverse_guard_evidence() {
    let integer = Some(ParamSemantic::Integer);
    assert_eq!(
        guarded_function(
            GuardShape::Exiting,
            ClampShape::MinMax,
            [integer; 3],
            BoundOrderEvidenceMode::Valid,
        ),
        (1, 1)
    );
    assert_eq!(
        guarded_function(
            GuardShape::Exiting,
            ClampShape::MinMax,
            [integer; 3],
            BoundOrderEvidenceMode::None,
        ),
        (1, 0),
        "guard shape alone is not a proof fact"
    );
    assert_eq!(
        guarded_function(
            GuardShape::NonExiting,
            ClampShape::MinMax,
            [integer; 3],
            BoundOrderEvidenceMode::Valid,
        ),
        (1, 0)
    );
    assert_eq!(
        guarded_function(
            GuardShape::None,
            ClampShape::MinMax,
            [integer; 3],
            BoundOrderEvidenceMode::Valid,
        ),
        (1, 0)
    );
}

#[test]
fn positive_branch_bound_order_is_proof_backed_inside_branch() {
    let integer = Some(ParamSemantic::Integer);
    assert_eq!(
        positive_branch_guarded_function([integer; 3], BoundOrderEvidenceMode::Valid),
        (1, 1)
    );
    assert_eq!(
        positive_branch_guarded_function([integer; 3], BoundOrderEvidenceMode::None),
        (1, 0)
    );
}

#[test]
fn bound_order_evidence_must_match_exact_operands_activation_and_dependencies() {
    let integer = Some(ParamSemantic::Integer);
    for mode in [
        BoundOrderEvidenceMode::SwappedOperands,
        BoundOrderEvidenceMode::WrongActivation,
        BoundOrderEvidenceMode::Ambiguous,
        BoundOrderEvidenceMode::MissingDependency,
    ] {
        assert_eq!(
            guarded_function(GuardShape::Exiting, ClampShape::MinMax, [integer; 3], mode),
            (1, 0)
        );
    }
}

#[test]
fn proof_rejects_floatish_number_and_wrong_shapes() {
    let integer = Some(ParamSemantic::Integer);
    let number = Some(ParamSemantic::Number);
    assert_eq!(
        guarded_function(
            GuardShape::Exiting,
            ClampShape::MinMax,
            [number; 3],
            BoundOrderEvidenceMode::Valid,
        ),
        (1, 0),
        "float-sensitive Number params need a separate NaN/domain proof"
    );
    assert_eq!(
        guarded_function(
            GuardShape::Exiting,
            ClampShape::SwappedBounds,
            [integer; 3],
            BoundOrderEvidenceMode::Valid,
        ),
        (1, 0)
    );
    assert_eq!(
        guarded_function(
            GuardShape::Exiting,
            ClampShape::WrongNesting,
            [integer; 3],
            BoundOrderEvidenceMode::Valid,
        ),
        (1, 0)
    );
}
