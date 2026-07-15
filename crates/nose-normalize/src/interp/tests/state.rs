use super::*;

fn run_cstyle_loop_with_update_err() -> Value {
    let sp = Span::synthetic(FileId(0));
    let mut b = IlBuilder::new(FileId(0));
    let i = b.add(NodeKind::Var, Payload::Cid(0), sp, &[]);
    let zero = b.add(NodeKind::Lit, Payload::LitInt(0), sp, &[]);
    let one = b.add(NodeKind::Lit, Payload::LitInt(1), sp, &[]);
    let init = b.add(NodeKind::Assign, Payload::None, sp, &[i, zero]);
    let cond = b.add(NodeKind::BinOp, Payload::Op(Op::Lt), sp, &[i, one]);
    let div = b.add(NodeKind::BinOp, Payload::Op(Op::Div), sp, &[one, zero]);
    let j = b.add(NodeKind::Var, Payload::Cid(1), sp, &[]);
    let update = b.add(NodeKind::Assign, Payload::None, sp, &[j, div]);
    let set_done = b.add(NodeKind::Assign, Payload::None, sp, &[i, one]);
    let body = b.add(NodeKind::Block, Payload::None, sp, &[set_done]);
    let loop_node = b.add(
        NodeKind::Loop,
        Payload::Loop(LoopKind::CStyle),
        sp,
        &[init, cond, update, body],
    );
    let seven = b.add(NodeKind::Lit, Payload::LitInt(7), sp, &[]);
    let ret = b.add(NodeKind::Return, Payload::None, sp, &[seven]);
    let block = b.add(NodeKind::Block, Payload::None, sp, &[loop_node, ret]);
    let func = b.add(NodeKind::Func, Payload::None, sp, &[block]);
    let il = b.finish(
        func,
        FileMeta {
            path: "t".into(),
            lang: Lang::C,
        },
        Vec::new(),
        Vec::new(),
    );
    run_admitted_unit(il, func, &[]).expect("run_unit").ret
}

#[test]
fn cstyle_loop_update_err_stops_execution() {
    assert_eq!(run_cstyle_loop_with_update_err(), Value::Err);
}

// #337: `def f(a): a[0] = 5; return a[0]` — the read must observe the WRITTEN 5 (in-place
// element mutation), not the pre-write element. Without mutation modeling the oracle was
// blind to `swap` vs `clobber`; with it, the value graph's read-forwarding and the oracle
// agree, so the two no longer false-merge.
fn run_index_store_then_read() -> Value {
    let sp = Span::synthetic(FileId(0));
    let mut b = IlBuilder::new(FileId(0));
    let a_param = b.add(NodeKind::Param, Payload::Cid(0), sp, &[]);
    let a_var = b.add(NodeKind::Var, Payload::Cid(0), sp, &[]);
    let zero = b.add(NodeKind::Lit, Payload::LitInt(0), sp, &[]);
    let target = b.add(NodeKind::Index, Payload::None, sp, &[a_var, zero]);
    let five = b.add(NodeKind::Lit, Payload::LitInt(5), sp, &[]);
    let store = b.add(NodeKind::Assign, Payload::None, sp, &[target, five]);
    let a_var2 = b.add(NodeKind::Var, Payload::Cid(0), sp, &[]);
    let zero2 = b.add(NodeKind::Lit, Payload::LitInt(0), sp, &[]);
    let read = b.add(NodeKind::Index, Payload::None, sp, &[a_var2, zero2]);
    let ret = b.add(NodeKind::Return, Payload::None, sp, &[read]);
    let block = b.add(NodeKind::Block, Payload::None, sp, &[store, ret]);
    let func = b.add(NodeKind::Func, Payload::None, sp, &[a_param, block]);
    let il = b.finish(
        func,
        FileMeta {
            path: "t".into(),
            lang: Lang::Python,
        },
        Vec::new(),
        Vec::new(),
    );
    run_admitted_unit(
        il,
        func,
        &[Value::List(vec![
            Value::Int(1),
            Value::Int(2),
            Value::Int(3),
        ])],
    )
    .expect("run_unit")
    .ret
}

#[test]
fn index_store_is_observed_by_later_read() {
    assert_eq!(run_index_store_then_read(), Value::Int(5));
}

// #342: the oracle models IEEE-754 float `+` as NON-associative, so it WITNESSES the
// `(a+b)+c` vs `a+(b+c)` difference the value graph now holds. `1e16 ± 1e16` loses the small
// term to rounding: `(1e16 + -1e16) + 1 = 1`, but `1e16 + (-1e16 + 1) = 0`.
#[test]
fn float_addition_is_non_associative_in_the_oracle() {
    let f = |x: f64| Value::Float(F64(x));
    let left = bin(Op::Add, &bin(Op::Add, &f(1e16), &f(-1e16)), &f(1.0));
    let right = bin(Op::Add, &f(1e16), &bin(Op::Add, &f(-1e16), &f(1.0)));
    assert_eq!(left, f(1.0));
    assert_eq!(right, f(0.0));
    assert_ne!(left, right, "float + is non-associative");
    // `Int` promotes to float under a float operand (the dynamic-language coercion).
    assert_eq!(bin(Op::Add, &Value::Int(2), &f(0.5)), f(2.5));
}

#[test]
fn declared_scalar_domains_reject_mixed_battery_values() {
    let float = Value::Float(F64(1.25));
    let string = Value::Str(vec![0x5eed]);
    assert_eq!(
        coerce_to_declared_domain(float.clone(), DomainEvidence::Float),
        float.clone()
    );
    assert_eq!(
        coerce_to_declared_domain(float.clone(), DomainEvidence::Number),
        float
    );
    assert_eq!(
        coerce_to_declared_domain(string.clone(), DomainEvidence::String),
        string
    );
    assert!(matches!(
        coerce_to_declared_domain(Value::Int(7), DomainEvidence::Float),
        Value::Float(_)
    ));
    assert!(matches!(
        coerce_to_declared_domain(Value::List(vec![]), DomainEvidence::Number),
        Value::Float(_)
    ));
    assert!(matches!(
        coerce_to_declared_domain(Value::Int(7), DomainEvidence::String),
        Value::Str(_)
    ));
}

// #342: `F64`'s behavior-comparison `Eq` canonicalizes the float corners — all NaNs are
// equal (two units returning NaN ARE behavior-equal) and `+0.0 == -0.0`.
#[test]
fn f64_eq_canonicalizes_nan_and_signed_zero() {
    assert_eq!(Value::Float(F64(f64::NAN)), Value::Float(F64(f64::NAN)));
    assert_eq!(Value::Float(F64(0.0)), Value::Float(F64(-0.0)));
    assert_ne!(Value::Float(F64(1.0)), Value::Float(F64(2.0)));
}

// #344: a JS-family `a & b` coerces operands to int32, so its result is int32 — the oracle
// computes `0xF_0000_0003 & 0xF_0000_0005` as `1` (the low-32-bit AND), not the bigint
// `0xF_0000_0001`. This is what lets `nose verify` witness the int32-vs-bigint split the
// value graph's `ToInt32` floor fingerprints. A non-JS language keeps the i64 result.
fn run_bitand(lang: Lang, a: i64, b: i64) -> Value {
    let sp = Span::synthetic(FileId(0));
    let mut bld = IlBuilder::new(FileId(0));
    let pa = bld.add(NodeKind::Param, Payload::Cid(0), sp, &[]);
    let pb = bld.add(NodeKind::Param, Payload::Cid(1), sp, &[]);
    let va = bld.add(NodeKind::Var, Payload::Cid(0), sp, &[]);
    let vb = bld.add(NodeKind::Var, Payload::Cid(1), sp, &[]);
    let and = bld.add(NodeKind::BinOp, Payload::Op(Op::BitAnd), sp, &[va, vb]);
    let ret = bld.add(NodeKind::Return, Payload::None, sp, &[and]);
    let func = bld.add(NodeKind::Func, Payload::None, sp, &[pa, pb, ret]);
    let il = bld.finish(
        func,
        FileMeta {
            path: "t".into(),
            lang,
        },
        Vec::new(),
        Vec::new(),
    );
    run_admitted_unit(il, func, &[Value::Int(a), Value::Int(b)])
        .expect("run_unit")
        .ret
}

#[test]
fn js_bitwise_and_wraps_to_int32_in_the_oracle() {
    // JS truncates to int32; Python keeps arbitrary precision.
    assert_eq!(
        run_bitand(Lang::JavaScript, 0xF_0000_0003, 0xF_0000_0005),
        Value::Int(1)
    );
    assert_eq!(
        run_bitand(Lang::Python, 0xF_0000_0003, 0xF_0000_0005),
        Value::Int(0xF_0000_0001)
    );
    // Small ints are identical either way (int32(x) == x).
    assert_eq!(run_bitand(Lang::JavaScript, 6, 3), Value::Int(2));
    assert_eq!(
        to_int32(Value::Float(F64(0xF_0000_0003_u64 as f64))),
        Value::Int(3)
    );
    assert_eq!(to_int32(Value::Float(F64(-1.9))), Value::Int(-1));
    assert_eq!(to_int32(Value::Float(F64(f64::NAN))), Value::Int(0));
}

fn run_foreach_with_iterable_err() -> Option<Value> {
    let sp = Span::synthetic(FileId(0));
    let mut b = IlBuilder::new(FileId(0));
    let target = b.add(NodeKind::Var, Payload::Cid(0), sp, &[]);
    let one = b.add(NodeKind::Lit, Payload::LitInt(1), sp, &[]);
    let zero = b.add(NodeKind::Lit, Payload::LitInt(0), sp, &[]);
    let iter_err = b.add(NodeKind::BinOp, Payload::Op(Op::Div), sp, &[one, zero]);
    let body = b.add(NodeKind::Block, Payload::None, sp, &[]);
    let loop_node = b.add(
        NodeKind::Loop,
        Payload::Loop(LoopKind::ForEach),
        sp,
        &[target, iter_err, body],
    );
    let seven = b.add(NodeKind::Lit, Payload::LitInt(7), sp, &[]);
    let ret = b.add(NodeKind::Return, Payload::None, sp, &[seven]);
    let block = b.add(NodeKind::Block, Payload::None, sp, &[loop_node, ret]);
    let func = b.add(NodeKind::Func, Payload::None, sp, &[block]);
    let il = b.finish(
        func,
        FileMeta {
            path: "t".into(),
            lang: Lang::Python,
        },
        Vec::new(),
        Vec::new(),
    );
    run_admitted_unit(il, func, &[]).map(|behavior| behavior.ret)
}

#[test]
fn foreach_iterable_err_stops_execution() {
    assert_eq!(run_foreach_with_iterable_err(), Some(Value::Err));
}

fn run_throw_then_return() -> Value {
    let sp = Span::synthetic(FileId(0));
    let mut b = IlBuilder::new(FileId(0));
    let thrown = b.add(NodeKind::Lit, Payload::LitStr(0xBAD), sp, &[]);
    let throw = b.add(NodeKind::Throw, Payload::None, sp, &[thrown]);
    let one = b.add(NodeKind::Lit, Payload::LitInt(1), sp, &[]);
    let ret = b.add(NodeKind::Return, Payload::None, sp, &[one]);
    let block = b.add(NodeKind::Block, Payload::None, sp, &[throw, ret]);
    let func = b.add(NodeKind::Func, Payload::None, sp, &[block]);
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
fn throw_is_err_behavior_and_stops_execution() {
    assert_eq!(run_throw_then_return(), Value::Err);
}

fn run_field_write_read() -> (Behavior, FieldKey) {
    let interner = Interner::new();
    let this_name = interner.intern("this");
    let field_name = interner.intern("x");
    let field_key = FieldKey {
        receiver: FieldPlace::SelfReceiver,
        field: stable_symbol_hash(interner.resolve(field_name)),
    };
    let mut b = IlBuilder::new(FileId(0));
    let write_receiver = b.add(NodeKind::Var, Payload::Name(this_name), test_span(1), &[]);
    let write_target = b.add(
        NodeKind::Field,
        Payload::Name(field_name),
        test_span(2),
        &[write_receiver],
    );
    let seven = b.add(NodeKind::Lit, Payload::LitInt(7), test_span(3), &[]);
    let assign = b.add(
        NodeKind::Assign,
        Payload::None,
        test_span(4),
        &[write_target, seven],
    );
    let read_receiver = b.add(NodeKind::Var, Payload::Name(this_name), test_span(5), &[]);
    let read_target = b.add(
        NodeKind::Field,
        Payload::Name(field_name),
        test_span(6),
        &[read_receiver],
    );
    let ret = b.add(
        NodeKind::Return,
        Payload::None,
        test_span(7),
        &[read_target],
    );
    let block = b.add(NodeKind::Block, Payload::None, test_span(8), &[assign, ret]);
    let func = b.add(NodeKind::Func, Payload::None, test_span(9), &[block]);
    let mut il = b.finish(
        func,
        FileMeta {
            path: "T.java".into(),
            lang: Lang::Java,
        },
        Vec::new(),
        Vec::new(),
    );
    admit_test_self_field_write(
        &mut il,
        &interner,
        write_receiver,
        write_target,
        assign,
        field_name,
        2000,
    );
    admit_test_self_field(
        &mut il,
        &interner,
        read_receiver,
        read_target,
        field_name,
        2010,
    );
    (
        run_unit(&il, &interner, func, &[]).expect("run_unit"),
        field_key,
    )
}

#[test]
fn field_write_can_be_read_back() {
    let (behavior, field_key) = run_field_write_read();
    assert_eq!(behavior.ret, Value::Int(7));
    assert_eq!(behavior.fields, vec![(field_key, Value::Int(7))]);
}

#[test]
fn raw_python_attribute_write_is_not_oracle_field_state_proof() {
    let interner = Interner::new();
    let field_name = interner.intern("x");
    let mut b = IlBuilder::new(FileId(0));
    let param = b.add(NodeKind::Param, Payload::Cid(0), test_span(1), &[]);
    let write_receiver = b.add(NodeKind::Var, Payload::Cid(0), test_span(2), &[]);
    let write_target = b.add(
        NodeKind::Field,
        Payload::Name(field_name),
        test_span(3),
        &[write_receiver],
    );
    let seven = b.add(NodeKind::Lit, Payload::LitInt(7), test_span(4), &[]);
    let assign = b.add(
        NodeKind::Assign,
        Payload::None,
        test_span(5),
        &[write_target, seven],
    );
    let read_receiver = b.add(NodeKind::Var, Payload::Cid(0), test_span(6), &[]);
    let read_target = b.add(
        NodeKind::Field,
        Payload::Name(field_name),
        test_span(7),
        &[read_receiver],
    );
    let ret = b.add(
        NodeKind::Return,
        Payload::None,
        test_span(8),
        &[read_target],
    );
    let block = b.add(NodeKind::Block, Payload::None, test_span(9), &[assign, ret]);
    let func = b.add(
        NodeKind::Func,
        Payload::None,
        test_span(10),
        &[param, block],
    );
    let il = b.finish(
        func,
        FileMeta {
            path: "t.py".into(),
            lang: Lang::Python,
        },
        Vec::new(),
        Vec::new(),
    );
    assert!(
        run_unit(&il, &interner, func, &[Value::Null]).is_none(),
        "raw attribute spelling must not prove exact field readback"
    );
}

fn run_field_read_with_error_receiver() -> Behavior {
    let interner = Interner::new();
    let this_name = interner.intern("this");
    let field_name = interner.intern("x");
    let mut b = IlBuilder::new(FileId(0));
    let write_receiver = b.add(NodeKind::Var, Payload::Name(this_name), test_span(1), &[]);
    let write_target = b.add(
        NodeKind::Field,
        Payload::Name(field_name),
        test_span(2),
        &[write_receiver],
    );
    let seven = b.add(NodeKind::Lit, Payload::LitInt(7), test_span(3), &[]);
    let assign = b.add(
        NodeKind::Assign,
        Payload::None,
        test_span(4),
        &[write_target, seven],
    );
    let one = b.add(NodeKind::Lit, Payload::LitInt(1), test_span(5), &[]);
    let zero = b.add(NodeKind::Lit, Payload::LitInt(0), test_span(6), &[]);
    let error_receiver = b.add(
        NodeKind::BinOp,
        Payload::Op(Op::Div),
        test_span(7),
        &[one, zero],
    );
    let read_target = b.add(
        NodeKind::Field,
        Payload::Name(field_name),
        test_span(8),
        &[error_receiver],
    );
    let ret = b.add(
        NodeKind::Return,
        Payload::None,
        test_span(9),
        &[read_target],
    );
    let block = b.add(
        NodeKind::Block,
        Payload::None,
        test_span(10),
        &[assign, ret],
    );
    let func = b.add(NodeKind::Func, Payload::None, test_span(11), &[block]);
    let mut il = b.finish(
        func,
        FileMeta {
            path: "T.java".into(),
            lang: Lang::Java,
        },
        Vec::new(),
        Vec::new(),
    );
    admit_test_self_field_write(
        &mut il,
        &interner,
        write_receiver,
        write_target,
        assign,
        field_name,
        2000,
    );
    run_unit(&il, &interner, func, &[]).expect("run_unit")
}

#[test]
fn field_read_propagates_receiver_err_before_cached_value() {
    assert_eq!(run_field_read_with_error_receiver().ret, Value::Err);
}

fn run_field_write_with_error_receiver() -> Behavior {
    let sp = Span::synthetic(FileId(0));
    let mut b = IlBuilder::new(FileId(0));
    let interner = Interner::new();
    let field_name = interner.intern("x");
    let one = b.add(NodeKind::Lit, Payload::LitInt(1), sp, &[]);
    let zero = b.add(NodeKind::Lit, Payload::LitInt(0), sp, &[]);
    let error_receiver = b.add(NodeKind::BinOp, Payload::Op(Op::Div), sp, &[one, zero]);
    let write_target = b.add(
        NodeKind::Field,
        Payload::Name(field_name),
        sp,
        &[error_receiver],
    );
    let seven = b.add(NodeKind::Lit, Payload::LitInt(7), sp, &[]);
    let assign = b.add(NodeKind::Assign, Payload::None, sp, &[write_target, seven]);
    let later = b.add(NodeKind::Lit, Payload::LitInt(9), sp, &[]);
    let ret = b.add(NodeKind::Return, Payload::None, sp, &[later]);
    let block = b.add(NodeKind::Block, Payload::None, sp, &[assign, ret]);
    let func = b.add(NodeKind::Func, Payload::None, sp, &[block]);
    let il = b.finish(
        func,
        FileMeta {
            path: "t".into(),
            lang: Lang::Python,
        },
        Vec::new(),
        Vec::new(),
    );
    run_admitted_unit(il, func, &[]).expect("run_unit")
}

#[test]
fn field_write_propagates_receiver_err_before_cached_value() {
    let behavior = run_field_write_with_error_receiver();
    assert_eq!(behavior.ret, Value::Err);
    assert!(behavior.fields.is_empty());
}

fn run_self_field_writes(swapped: bool) -> Behavior {
    let interner = Interner::new();
    let this_name = interner.intern("this");
    let x_name = interner.intern("x");
    let y_name = interner.intern("y");
    let mut b = IlBuilder::new(FileId(0));
    let x_receiver = b.add(NodeKind::Var, Payload::Name(this_name), test_span(1), &[]);
    let x_target = b.add(
        NodeKind::Field,
        Payload::Name(x_name),
        test_span(2),
        &[x_receiver],
    );
    let one = b.add(NodeKind::Lit, Payload::LitInt(1), test_span(3), &[]);
    let x_assign = b.add(
        NodeKind::Assign,
        Payload::None,
        test_span(4),
        &[x_target, one],
    );
    let y_receiver = b.add(NodeKind::Var, Payload::Name(this_name), test_span(5), &[]);
    let y_target = b.add(
        NodeKind::Field,
        Payload::Name(y_name),
        test_span(6),
        &[y_receiver],
    );
    let two = b.add(NodeKind::Lit, Payload::LitInt(2), test_span(7), &[]);
    let y_assign = b.add(
        NodeKind::Assign,
        Payload::None,
        test_span(8),
        &[y_target, two],
    );
    let statements = if swapped {
        vec![y_assign, x_assign]
    } else {
        vec![x_assign, y_assign]
    };
    let block = b.add(NodeKind::Block, Payload::None, test_span(9), &statements);
    let func = b.add(NodeKind::Func, Payload::None, test_span(10), &[block]);
    let mut il = b.finish(
        func,
        FileMeta {
            path: "T.java".into(),
            lang: Lang::Java,
        },
        Vec::new(),
        Vec::new(),
    );
    admit_test_self_field_write(
        &mut il, &interner, x_receiver, x_target, x_assign, x_name, 2000,
    );
    admit_test_self_field_write(
        &mut il, &interner, y_receiver, y_target, y_assign, y_name, 2010,
    );
    run_unit(&il, &interner, func, &[]).expect("self-field writes should interpret")
}

#[test]
fn self_field_final_state_is_order_insensitive() {
    assert_eq!(
        run_self_field_writes(false).fields,
        run_self_field_writes(true).fields
    );
}
