use super::*;

fn binary_il(lang: Lang, op: Op) -> (Il, NodeId) {
    let sp = Span::synthetic(FileId(0));
    let mut b = IlBuilder::new(FileId(0));
    let pa = b.add(NodeKind::Param, Payload::Cid(0), sp, &[]);
    let pb = b.add(NodeKind::Param, Payload::Cid(1), sp, &[]);
    let a = b.add(NodeKind::Var, Payload::Cid(0), sp, &[]);
    let b_value = b.add(NodeKind::Var, Payload::Cid(1), sp, &[]);
    let value = b.add(NodeKind::BinOp, Payload::Op(op), sp, &[a, b_value]);
    let ret = b.add(NodeKind::Return, Payload::None, sp, &[value]);
    let root = b.add(NodeKind::Func, Payload::None, sp, &[pa, pb, ret]);
    (
        b.finish(
            root,
            FileMeta {
                path: "number.ts".into(),
                lang,
            },
            Vec::new(),
            Vec::new(),
        ),
        root,
    )
}

fn run_binary(lang: Lang, op: Op, a: Value, b: Value) -> Option<Value> {
    let (il, root) = binary_il(lang, op);
    run_admitted_unit(il, root, &[a, b]).map(|behavior| behavior.ret)
}

fn run_conditional(lang: Lang, input: Value) -> Value {
    let sp = Span::synthetic(FileId(0));
    let mut b = IlBuilder::new(FileId(0));
    let param = b.add(NodeKind::Param, Payload::Cid(0), sp, &[]);
    let condition = b.add(NodeKind::Var, Payload::Cid(0), sp, &[]);
    let one = b.add(NodeKind::Lit, Payload::LitInt(1), sp, &[]);
    let two = b.add(NodeKind::Lit, Payload::LitInt(2), sp, &[]);
    let branch = b.add(NodeKind::If, Payload::None, sp, &[condition, one, two]);
    let ret = b.add(NodeKind::Return, Payload::None, sp, &[branch]);
    let root = b.add(NodeKind::Func, Payload::None, sp, &[param, ret]);
    let il = b.finish(
        root,
        FileMeta {
            path: "truthy.ts".into(),
            lang,
        },
        Vec::new(),
        Vec::new(),
    );
    run_admitted_unit(il, root, &[input])
        .expect("conditional must interpret")
        .ret
}

fn maybe_run_unary(lang: Lang, op: Op, input: Value) -> Option<Value> {
    let sp = Span::synthetic(FileId(0));
    let mut b = IlBuilder::new(FileId(0));
    let param = b.add(NodeKind::Param, Payload::Cid(0), sp, &[]);
    let value = b.add(NodeKind::Var, Payload::Cid(0), sp, &[]);
    let unary = b.add(NodeKind::UnOp, Payload::Op(op), sp, &[value]);
    let ret = b.add(NodeKind::Return, Payload::None, sp, &[unary]);
    let root = b.add(NodeKind::Func, Payload::None, sp, &[param, ret]);
    let il = b.finish(
        root,
        FileMeta {
            path: "unary.ts".into(),
            lang,
        },
        Vec::new(),
        Vec::new(),
    );
    run_admitted_unit(il, root, &[input]).map(|behavior| behavior.ret)
}

fn run_unary(lang: Lang, op: Op, input: Value) -> Value {
    maybe_run_unary(lang, op, input).expect("unary expression must interpret")
}

#[test]
fn javascript_number_uses_ieee_zero_division_and_remainder() {
    let float = |value| Value::Float(F64(value));

    assert_eq!(
        run_binary(Lang::TypeScript, Op::TrueDiv, float(1.0), float(0.0)),
        Some(float(f64::INFINITY))
    );
    assert!(matches!(
        run_binary(
            Lang::TypeScript,
            Op::TrueDiv,
            float(0.0),
            float(0.0)
        ),
        Some(Value::Float(F64(value))) if value.is_nan()
    ));
    assert!(matches!(
        run_binary(Lang::TypeScript, Op::Mod, float(1.0), float(0.0)),
        Some(Value::Float(F64(value))) if value.is_nan()
    ));

    // Python keeps the general oracle's explicit zero-division error behavior.
    assert_eq!(
        run_binary(Lang::Python, Op::TrueDiv, float(1.0), float(0.0)),
        Some(Value::Err)
    );
}

#[test]
fn javascript_exact_number_results_use_one_compact_representation() {
    assert_eq!(
        run_unary(Lang::TypeScript, Op::Neg, Value::Int(-1)),
        Value::Int(1),
        "double-negation's outer operation must agree with the exact literal `1`"
    );
    assert_eq!(
        js_number_bin(Op::Add, &Value::Int(1), &Value::Int(1)),
        Some(Value::Int(2)),
        "literal-derived exact arithmetic must remain compact"
    );
    assert_eq!(
        js_number_bin(Op::Add, &Value::Float(F64(1.0)), &Value::Int(1)),
        Some(Value::Float(F64(2.0))),
        "a Float operand keeps the internal IEEE-754 lane until observation"
    );
    assert_eq!(
        js_number_result(9_223_372_036_854_775_808.0, false),
        Value::Float(F64(9_223_372_036_854_775_808.0)),
        "the first value above i64::MAX must not saturate into a compact integer"
    );
    assert!(matches!(
        run_unary(Lang::TypeScript, Op::Neg, Value::Int(0)),
        Value::Float(F64(value)) if value.to_bits() == (-0.0f64).to_bits()
    ));
    let negative_zero = run_unary(Lang::TypeScript, Op::Neg, Value::Int(0));
    assert_eq!(
        run_unary(Lang::TypeScript, Op::Neg, negative_zero),
        Value::Int(0),
        "double-negated literal zero must agree with compact positive zero"
    );

    let from_positive_zero = run_unary(
        Lang::TypeScript,
        Op::Neg,
        run_unary(Lang::TypeScript, Op::Neg, Value::Float(F64(0.0))),
    );
    assert_eq!(from_positive_zero, Value::Int(0));
    let from_negative_zero = run_unary(
        Lang::TypeScript,
        Op::Neg,
        run_unary(Lang::TypeScript, Op::Neg, Value::Float(F64(-0.0))),
    );
    assert!(matches!(
        from_negative_zero,
        Value::Float(F64(value)) if value.to_bits() == (-0.0f64).to_bits()
    ));
}

#[test]
fn javascript_nan_is_falsy_without_changing_python() {
    let nan = Value::Float(F64(f64::NAN));
    assert_eq!(
        run_conditional(Lang::TypeScript, nan.clone()),
        Value::Int(2)
    );
    assert_eq!(
        run_unary(Lang::TypeScript, Op::Not, nan.clone()),
        Value::Bool(true)
    );
    assert_eq!(
        run_unary(Lang::Python, Op::Not, nan.clone()),
        Value::Bool(false)
    );
    assert_eq!(run_conditional(Lang::Python, nan), Value::Int(1));
}

#[test]
fn javascript_empty_array_is_truthy_without_changing_python() {
    let empty = Value::List(Vec::new());
    assert_eq!(
        run_conditional(Lang::TypeScript, empty.clone()),
        Value::Int(1)
    );
    assert_eq!(
        run_unary(Lang::TypeScript, Op::Not, empty.clone()),
        Value::Bool(false)
    );
    assert_eq!(run_conditional(Lang::Python, empty.clone()), Value::Int(2));
    assert_eq!(run_unary(Lang::Python, Op::Not, empty), Value::Bool(true));
}

#[test]
fn javascript_shifts_coerce_both_operands_and_mask_the_count() {
    let float = |value| Value::Float(F64(value));
    assert_eq!(
        run_binary(Lang::TypeScript, Op::Shl, float(-8.0), float(1.0)),
        Some(Value::Int(-16))
    );
    assert_eq!(
        run_binary(Lang::TypeScript, Op::Shr, float(-8.0), float(1.0)),
        Some(Value::Int(-4))
    );
    assert_eq!(
        run_binary(Lang::TypeScript, Op::Shl, float(-8.0), float(33.0)),
        Some(Value::Int(-16)),
        "JavaScript masks the shift count to five bits"
    );
    assert_eq!(
        run_binary(
            Lang::TypeScript,
            Op::BitAnd,
            Value::Bool(true),
            Value::Int(3)
        ),
        Some(Value::Int(1))
    );
    assert_eq!(
        run_binary(Lang::TypeScript, Op::BitOr, Value::Null, Value::Int(0)),
        Some(Value::Int(0))
    );
    assert_eq!(
        run_unary(Lang::TypeScript, Op::BitNot, Value::Bool(true)),
        Value::Int(-2)
    );
    assert_eq!(
        run_unary(Lang::TypeScript, Op::BitNot, Value::Null),
        Value::Int(-1)
    );
    assert_eq!(
        run_binary(
            Lang::TypeScript,
            Op::BitAnd,
            Value::Str(vec![1]),
            Value::Int(1)
        ),
        None,
        "unrepresented JavaScript string coercion must fail closed"
    );
    assert_eq!(
        maybe_run_unary(Lang::TypeScript, Op::BitNot, Value::Str(vec![1])),
        None,
        "unrepresented JavaScript unary coercion must also fail closed"
    );
}

#[test]
fn uncalibrated_javascript_number_operator_fails_closed() {
    assert_eq!(
        run_binary(
            Lang::TypeScript,
            Op::Pow,
            Value::Float(F64(1.0)),
            Value::Float(F64(f64::INFINITY))
        ),
        None,
        "JS exponent edge rules must not enter the hard lane through generic powf"
    );
    assert_eq!(
        run_binary(
            Lang::TypeScript,
            Op::Pow,
            Value::Str(vec![1]),
            Value::Str(vec![2])
        ),
        None,
        "coercive JS exponentiation must not fall through to a concrete Err"
    );
}
