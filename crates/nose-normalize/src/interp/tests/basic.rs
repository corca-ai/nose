use super::*;

/// Build `fn() { return len(<str literal>) }` and run it.
fn run_len_of_string() -> Value {
    let sp = Span::synthetic(FileId(0));
    let mut b = IlBuilder::new(FileId(0));
    let s = b.add(NodeKind::Lit, Payload::LitStr(0xABCD), sp, &[]);
    let call = b.add(NodeKind::Call, Payload::Builtin(Builtin::Len), sp, &[s]);
    let ret = b.add(NodeKind::Return, Payload::None, sp, &[call]);
    let func = b.add(NodeKind::Func, Payload::None, sp, &[ret]);
    let il = b.finish(
        func,
        FileMeta {
            path: "t".into(),
            lang: Lang::Python,
        },
        Vec::new(),
        Vec::new(),
    );
    run_admitted_unit(il, func, &[]).expect("run_unit").ret
}

#[test]
fn len_of_string_is_err_not_one() {
    // Strings are the free monoid over opaque piece hashes — character length is
    // unknown, so `len(str)` must be `Err` (matching the documented contract and the
    // sibling `IsEmpty`), not a hardcoded `Int(1)`.
    assert_eq!(run_len_of_string(), Value::Err);
}

#[test]
fn string_contains_proves_only_exact_chunk_occurrence() {
    let value = Value::Str(vec![1, 2, 3]);
    let needle = Value::Str(vec![2]);
    assert!(matches!(
        string_contains(Some(&value), Some(&needle)),
        Ok(Value::Bool(true))
    ));
}

#[test]
fn string_contains_fails_closed_on_opaque_substring_absence() {
    let value = Value::Str(vec![stable_symbol_hash("abc")]);
    let needle = Value::Str(vec![stable_symbol_hash("b")]);
    assert!(
        string_contains(Some(&value), Some(&needle)).is_err(),
        "opaque string chunks cannot prove character-level substring absence"
    );
}

#[test]
fn unadmitted_builtin_calls_become_identified_symbolic_effects() {
    let sp = Span::synthetic(FileId(0));
    let mut b = IlBuilder::new(FileId(0));
    let one = b.add(NodeKind::Lit, Payload::LitInt(1), sp, &[]);
    let xs = b.add(NodeKind::Seq, Payload::None, sp, &[one]);
    let call = b.add(NodeKind::Call, Payload::Builtin(Builtin::Len), sp, &[xs]);
    let ret = b.add(NodeKind::Return, Payload::None, sp, &[call]);
    let func = b.add(NodeKind::Func, Payload::None, sp, &[ret]);
    let il = b.finish(
        func,
        FileMeta {
            path: "t".into(),
            lang: Lang::Python,
        },
        Vec::new(),
        Vec::new(),
    );

    // Without admission the call no longer bails the unit: it interprets to a
    // SYMBOLIC value, recorded in the effect trace (an unknown callee may act).
    let interner = Interner::new();
    let beh = run_unit(&il, &interner, func, &[]).expect("symbolic run");
    assert!(matches!(beh.ret, Value::Sym(_)));
    assert_eq!(beh.effects, vec![beh.ret.clone()]);
    // The admitted run keeps its CONCRETE semantics — and differs from symbolic.
    assert_eq!(
        run_admitted_unit(il, func, &[]).expect("admitted run").ret,
        Value::Int(1)
    );
}

fn run_value_or_default(value: NodeId, default: NodeId, mut b: IlBuilder, sp: Span) -> Value {
    let call = b.add(
        NodeKind::Call,
        Payload::Builtin(Builtin::ValueOrDefault),
        sp,
        &[value, default],
    );
    let ret = b.add(NodeKind::Return, Payload::None, sp, &[call]);
    let func = b.add(NodeKind::Func, Payload::None, sp, &[ret]);
    let il = b.finish(
        func,
        FileMeta {
            path: "t".into(),
            lang: Lang::Rust,
        },
        Vec::new(),
        Vec::new(),
    );
    run_admitted_unit(il, func, &[]).expect("run_unit").ret
}

#[test]
fn value_or_default_uses_default_for_null() {
    let sp = Span::synthetic(FileId(0));
    let mut b = IlBuilder::new(FileId(0));
    let value = b.add(NodeKind::Lit, Payload::Lit(LitClass::Null), sp, &[]);
    let default = b.add(NodeKind::Lit, Payload::LitInt(7), sp, &[]);
    assert_eq!(run_value_or_default(value, default, b, sp), Value::Int(7));
}

#[test]
fn value_or_default_short_circuits_present_value() {
    let sp = Span::synthetic(FileId(0));
    let mut b = IlBuilder::new(FileId(0));
    let value = b.add(NodeKind::Lit, Payload::LitInt(7), sp, &[]);
    let one = b.add(NodeKind::Lit, Payload::LitInt(1), sp, &[]);
    let zero = b.add(NodeKind::Lit, Payload::LitInt(0), sp, &[]);
    let default_err = b.add(NodeKind::BinOp, Payload::Op(Op::Div), sp, &[one, zero]);
    assert_eq!(
        run_value_or_default(value, default_err, b, sp),
        Value::Int(7)
    );
}

#[test]
fn value_or_default_keeps_error_value() {
    let sp = Span::synthetic(FileId(0));
    let mut b = IlBuilder::new(FileId(0));
    let one = b.add(NodeKind::Lit, Payload::LitInt(1), sp, &[]);
    let zero = b.add(NodeKind::Lit, Payload::LitInt(0), sp, &[]);
    let value_err = b.add(NodeKind::BinOp, Payload::Op(Op::Div), sp, &[one, zero]);
    let default = b.add(NodeKind::Lit, Payload::LitInt(7), sp, &[]);
    assert_eq!(run_value_or_default(value_err, default, b, sp), Value::Err);
}

fn run_range(args: &[i64]) -> Value {
    let sp = Span::synthetic(FileId(0));
    let mut b = IlBuilder::new(FileId(0));
    let args: Vec<NodeId> = args
        .iter()
        .map(|arg| b.add(NodeKind::Lit, Payload::LitInt(*arg), sp, &[]))
        .collect();
    let call = b.add(NodeKind::Call, Payload::Builtin(Builtin::Range), sp, &args);
    let ret = b.add(NodeKind::Return, Payload::None, sp, &[call]);
    let func = b.add(NodeKind::Func, Payload::None, sp, &[ret]);
    let il = b.finish(
        func,
        FileMeta {
            path: "t".into(),
            lang: Lang::Python,
        },
        Vec::new(),
        Vec::new(),
    );
    run_admitted_unit(il, func, &[]).expect("run_unit").ret
}

#[test]
fn range_interprets_start_stop_and_step() {
    assert_eq!(
        run_range(&[1, 5, 2]),
        Value::List(vec![Value::Int(1), Value::Int(3)])
    );
}

#[test]
fn range_zero_step_is_err_behavior() {
    assert_eq!(run_range(&[1, 5, 0]), Value::Err);
}
