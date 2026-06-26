use super::*;

fn hof_with_error_lambda(kind: HoFKind) -> Value {
    let sp = Span::synthetic(FileId(0));
    let mut b = IlBuilder::new(FileId(0));
    let param = b.add(NodeKind::Param, Payload::Cid(0), sp, &[]);
    let one = b.add(NodeKind::Lit, Payload::LitInt(1), sp, &[]);
    let zero = b.add(NodeKind::Lit, Payload::LitInt(0), sp, &[]);
    let div = b.add(NodeKind::BinOp, Payload::Op(Op::Div), sp, &[one, zero]);
    let lambda_ret = b.add(NodeKind::Return, Payload::None, sp, &[div]);
    let lambda_body = b.add(NodeKind::Block, Payload::None, sp, &[lambda_ret]);
    let lambda = b.add(NodeKind::Lambda, Payload::None, sp, &[param, lambda_body]);
    let coll = b.add(NodeKind::Seq, Payload::None, sp, &[one]);
    let hof = b.add(NodeKind::HoF, Payload::HoF(kind), sp, &[coll, lambda]);
    let ret = b.add(NodeKind::Return, Payload::None, sp, &[hof]);
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
fn hof_map_propagates_lambda_errors() {
    assert_eq!(hof_with_error_lambda(HoFKind::Map), Value::Err);
}

#[test]
fn hof_filter_propagates_lambda_errors() {
    assert_eq!(hof_with_error_lambda(HoFKind::Filter), Value::Err);
}

#[test]
fn hof_flat_map_propagates_lambda_errors() {
    assert_eq!(hof_with_error_lambda(HoFKind::FlatMap), Value::Err);
}

#[test]
fn hof_filter_map_propagates_lambda_errors() {
    assert_eq!(hof_with_error_lambda(HoFKind::FilterMap), Value::Err);
}

fn hof_flat_map_value() -> Value {
    let sp = Span::synthetic(FileId(0));
    let mut b = IlBuilder::new(FileId(0));
    let param = b.add(NodeKind::Param, Payload::Cid(0), sp, &[]);
    let var = b.add(NodeKind::Var, Payload::Cid(0), sp, &[]);
    let pair = b.add(NodeKind::Seq, Payload::None, sp, &[var, var]);
    let lambda_ret = b.add(NodeKind::Return, Payload::None, sp, &[pair]);
    let lambda_body = b.add(NodeKind::Block, Payload::None, sp, &[lambda_ret]);
    let lambda = b.add(NodeKind::Lambda, Payload::None, sp, &[param, lambda_body]);
    let one = b.add(NodeKind::Lit, Payload::LitInt(1), sp, &[]);
    let two = b.add(NodeKind::Lit, Payload::LitInt(2), sp, &[]);
    let coll = b.add(NodeKind::Seq, Payload::None, sp, &[one, two]);
    let hof = b.add(
        NodeKind::HoF,
        Payload::HoF(HoFKind::FlatMap),
        sp,
        &[coll, lambda],
    );
    let ret = b.add(NodeKind::Return, Payload::None, sp, &[hof]);
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
fn hof_flat_map_flattens_lambda_lists() {
    assert_eq!(
        hof_flat_map_value(),
        Value::List(vec![
            Value::Int(1),
            Value::Int(1),
            Value::Int(2),
            Value::Int(2)
        ])
    );
}

fn hof_scalar_flat_map_value() -> Value {
    let sp = Span::synthetic(FileId(0));
    let mut b = IlBuilder::new(FileId(0));
    let param = b.add(NodeKind::Param, Payload::Cid(0), sp, &[]);
    let var = b.add(NodeKind::Var, Payload::Cid(0), sp, &[]);
    let lambda_ret = b.add(NodeKind::Return, Payload::None, sp, &[var]);
    let lambda_body = b.add(NodeKind::Block, Payload::None, sp, &[lambda_ret]);
    let lambda = b.add(NodeKind::Lambda, Payload::None, sp, &[param, lambda_body]);
    let one = b.add(NodeKind::Lit, Payload::LitInt(1), sp, &[]);
    let coll = b.add(NodeKind::Seq, Payload::None, sp, &[one]);
    let hof = b.add(
        NodeKind::HoF,
        Payload::HoF(HoFKind::FlatMap),
        sp,
        &[coll, lambda],
    );
    let ret = b.add(NodeKind::Return, Payload::None, sp, &[hof]);
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
fn hof_flat_map_scalar_lambda_result_is_err() {
    assert_eq!(hof_scalar_flat_map_value(), Value::Err);
}

fn hof_filter_map_value() -> Value {
    let sp = Span::synthetic(FileId(0));
    let mut b = IlBuilder::new(FileId(0));
    let param = b.add(NodeKind::Param, Payload::Cid(0), sp, &[]);
    let var = b.add(NodeKind::Var, Payload::Cid(0), sp, &[]);
    let zero = b.add(NodeKind::Lit, Payload::LitInt(0), sp, &[]);
    let is_zero = b.add(NodeKind::BinOp, Payload::Op(Op::Eq), sp, &[var, zero]);
    let var_again = b.add(NodeKind::Var, Payload::Cid(0), sp, &[]);
    let none = b.add(NodeKind::Lit, Payload::Lit(LitClass::Null), sp, &[]);
    let selected = b.add(NodeKind::If, Payload::None, sp, &[is_zero, var_again, none]);
    let lambda_ret = b.add(NodeKind::Return, Payload::None, sp, &[selected]);
    let lambda_body = b.add(NodeKind::Block, Payload::None, sp, &[lambda_ret]);
    let lambda = b.add(NodeKind::Lambda, Payload::None, sp, &[param, lambda_body]);
    let one = b.add(NodeKind::Lit, Payload::LitInt(1), sp, &[]);
    let coll = b.add(NodeKind::Seq, Payload::None, sp, &[zero, one]);
    let hof = b.add(
        NodeKind::HoF,
        Payload::HoF(HoFKind::FilterMap),
        sp,
        &[coll, lambda],
    );
    let ret = b.add(NodeKind::Return, Payload::None, sp, &[hof]);
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
fn hof_filter_map_drops_null_and_keeps_falsey_values() {
    assert_eq!(hof_filter_map_value(), Value::List(vec![Value::Int(0)]));
}

fn hof_with_empty_collection_and_error_lambda(kind: HoFKind) -> Value {
    let sp = Span::synthetic(FileId(0));
    let mut b = IlBuilder::new(FileId(0));
    let param = b.add(NodeKind::Param, Payload::Cid(0), sp, &[]);
    let one = b.add(NodeKind::Lit, Payload::LitInt(1), sp, &[]);
    let zero = b.add(NodeKind::Lit, Payload::LitInt(0), sp, &[]);
    let div = b.add(NodeKind::BinOp, Payload::Op(Op::Div), sp, &[one, zero]);
    let lambda_ret = b.add(NodeKind::Return, Payload::None, sp, &[div]);
    let lambda_body = b.add(NodeKind::Block, Payload::None, sp, &[lambda_ret]);
    let lambda = b.add(NodeKind::Lambda, Payload::None, sp, &[param, lambda_body]);
    let coll = b.add(NodeKind::Seq, Payload::None, sp, &[]);
    let hof = b.add(NodeKind::HoF, Payload::HoF(kind), sp, &[coll, lambda]);
    let ret = b.add(NodeKind::Return, Payload::None, sp, &[hof]);
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
fn hof_empty_collections_skip_lambda_errors() {
    let empty = Value::List(Vec::new());
    assert_eq!(
        hof_with_empty_collection_and_error_lambda(HoFKind::Map),
        empty
    );
    assert_eq!(
        hof_with_empty_collection_and_error_lambda(HoFKind::Filter),
        empty
    );
    assert_eq!(
        hof_with_empty_collection_and_error_lambda(HoFKind::Reject),
        empty
    );
    assert_eq!(
        hof_with_empty_collection_and_error_lambda(HoFKind::FlatMap),
        empty
    );
    assert_eq!(
        hof_with_empty_collection_and_error_lambda(HoFKind::FilterMap),
        empty
    );
}

fn hof_filter_map_with_scalar_collection() -> Value {
    let sp = Span::synthetic(FileId(0));
    let mut b = IlBuilder::new(FileId(0));
    let param = b.add(NodeKind::Param, Payload::Cid(0), sp, &[]);
    let var = b.add(NodeKind::Var, Payload::Cid(0), sp, &[]);
    let lambda_ret = b.add(NodeKind::Return, Payload::None, sp, &[var]);
    let lambda_body = b.add(NodeKind::Block, Payload::None, sp, &[lambda_ret]);
    let lambda = b.add(NodeKind::Lambda, Payload::None, sp, &[param, lambda_body]);
    let scalar = b.add(NodeKind::Lit, Payload::LitInt(1), sp, &[]);
    let hof = b.add(
        NodeKind::HoF,
        Payload::HoF(HoFKind::FilterMap),
        sp,
        &[scalar, lambda],
    );
    let ret = b.add(NodeKind::Return, Payload::None, sp, &[hof]);
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
fn hof_filter_map_scalar_collection_is_err() {
    assert_eq!(hof_filter_map_with_scalar_collection(), Value::Err);
}

fn hof_filter_map_with_captured_value() -> Value {
    let sp = Span::synthetic(FileId(0));
    let mut b = IlBuilder::new(FileId(0));
    let offset_var = b.add(NodeKind::Var, Payload::Cid(1), sp, &[]);
    let ten = b.add(NodeKind::Lit, Payload::LitInt(10), sp, &[]);
    let assign_offset = b.add(NodeKind::Assign, Payload::None, sp, &[offset_var, ten]);
    let param = b.add(NodeKind::Param, Payload::Cid(0), sp, &[]);
    let x = b.add(NodeKind::Var, Payload::Cid(0), sp, &[]);
    let offset = b.add(NodeKind::Var, Payload::Cid(1), sp, &[]);
    let sum = b.add(NodeKind::BinOp, Payload::Op(Op::Add), sp, &[x, offset]);
    let lambda_ret = b.add(NodeKind::Return, Payload::None, sp, &[sum]);
    let lambda_body = b.add(NodeKind::Block, Payload::None, sp, &[lambda_ret]);
    let lambda = b.add(NodeKind::Lambda, Payload::None, sp, &[param, lambda_body]);
    let one = b.add(NodeKind::Lit, Payload::LitInt(1), sp, &[]);
    let two = b.add(NodeKind::Lit, Payload::LitInt(2), sp, &[]);
    let coll = b.add(NodeKind::Seq, Payload::None, sp, &[one, two]);
    let hof = b.add(
        NodeKind::HoF,
        Payload::HoF(HoFKind::FilterMap),
        sp,
        &[coll, lambda],
    );
    let ret = b.add(NodeKind::Return, Payload::None, sp, &[hof]);
    let block = b.add(NodeKind::Block, Payload::None, sp, &[assign_offset, ret]);
    let func = b.add(NodeKind::Func, Payload::None, sp, &[block]);
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
fn hof_filter_map_lambda_captures_outer_environment() {
    assert_eq!(
        hof_filter_map_with_captured_value(),
        Value::List(vec![Value::Int(11), Value::Int(12)])
    );
}

fn hof_filter_map_effectful_lambda() -> Behavior {
    let sp = Span::synthetic(FileId(0));
    let mut b = IlBuilder::new(FileId(0));
    let param = b.add(NodeKind::Param, Payload::Cid(0), sp, &[]);
    let printed = b.add(NodeKind::Var, Payload::Cid(0), sp, &[]);
    let print = b.add(
        NodeKind::Call,
        Payload::Builtin(Builtin::Print),
        sp,
        &[printed],
    );
    let print_stmt = b.add(NodeKind::ExprStmt, Payload::None, sp, &[print]);
    let returned = b.add(NodeKind::Var, Payload::Cid(0), sp, &[]);
    let lambda_ret = b.add(NodeKind::Return, Payload::None, sp, &[returned]);
    let lambda_body = b.add(
        NodeKind::Block,
        Payload::None,
        sp,
        &[print_stmt, lambda_ret],
    );
    let lambda = b.add(NodeKind::Lambda, Payload::None, sp, &[param, lambda_body]);
    let one = b.add(NodeKind::Lit, Payload::LitInt(1), sp, &[]);
    let two = b.add(NodeKind::Lit, Payload::LitInt(2), sp, &[]);
    let coll = b.add(NodeKind::Seq, Payload::None, sp, &[one, two]);
    let hof = b.add(
        NodeKind::HoF,
        Payload::HoF(HoFKind::FilterMap),
        sp,
        &[coll, lambda],
    );
    let ret = b.add(NodeKind::Return, Payload::None, sp, &[hof]);
    let func = b.add(NodeKind::Func, Payload::None, sp, &[ret]);
    let il = b.finish(
        func,
        FileMeta {
            path: "t.py".into(),
            lang: Lang::Python,
        },
        Vec::new(),
        Vec::new(),
    );
    run_admitted_unit(il, func, &[]).expect("run_unit")
}

#[test]
fn hof_filter_map_effectful_lambda_records_effects() {
    let behavior = hof_filter_map_effectful_lambda();
    assert_eq!(
        behavior.ret,
        Value::List(vec![Value::Int(1), Value::Int(2)])
    );
    assert_eq!(behavior.effects, vec![Value::Int(1), Value::Int(2)]);
}

fn java_stream_nested_expression_lambda_behavior(outer_kind: HoFKind) -> Behavior {
    let sp = Span::synthetic(FileId(0));
    let mut b = IlBuilder::new(FileId(0));
    let xs_param = b.add(NodeKind::Param, Payload::Cid(0), sp, &[]);
    let ys_param = b.add(NodeKind::Param, Payload::Cid(1), sp, &[]);

    let x_param = b.add(NodeKind::Param, Payload::Cid(2), sp, &[]);
    let y_param = b.add(NodeKind::Param, Payload::Cid(3), sp, &[]);
    let x_in_inner = b.add(NodeKind::Var, Payload::Cid(2), sp, &[]);
    let y = b.add(NodeKind::Var, Payload::Cid(3), sp, &[]);
    let sum = b.add(NodeKind::BinOp, Payload::Op(Op::Add), sp, &[x_in_inner, y]);
    let inner_lambda = b.add(NodeKind::Lambda, Payload::None, sp, &[y_param, sum]);
    let ys = b.add(NodeKind::Var, Payload::Cid(1), sp, &[]);
    let inner_map = b.add(
        NodeKind::HoF,
        Payload::HoF(HoFKind::Map),
        sp,
        &[ys, inner_lambda],
    );
    let outer_lambda = b.add(NodeKind::Lambda, Payload::None, sp, &[x_param, inner_map]);
    let xs = b.add(NodeKind::Var, Payload::Cid(0), sp, &[]);
    let outer = b.add(
        NodeKind::HoF,
        Payload::HoF(outer_kind),
        sp,
        &[xs, outer_lambda],
    );
    let ret = b.add(NodeKind::Return, Payload::None, sp, &[outer]);
    let func = b.add(
        NodeKind::Func,
        Payload::None,
        sp,
        &[xs_param, ys_param, ret],
    );
    let il = b.finish(
        func,
        FileMeta {
            path: "T.java".into(),
            lang: Lang::Java,
        },
        Vec::new(),
        Vec::new(),
    );
    run_admitted_unit(
        il,
        func,
        &[
            Value::List(vec![Value::Int(1), Value::Int(2)]),
            Value::List(vec![Value::Int(10), Value::Int(20)]),
        ],
    )
    .expect("run_unit")
}

#[test]
fn java_stream_flat_map_expression_lambdas_are_interpretable() {
    assert_eq!(
        java_stream_nested_expression_lambda_behavior(HoFKind::FlatMap).ret,
        Value::List(vec![
            Value::Int(11),
            Value::Int(21),
            Value::Int(12),
            Value::Int(22),
        ])
    );
}

#[test]
fn java_stream_map_returning_stream_stays_nested() {
    assert_eq!(
        java_stream_nested_expression_lambda_behavior(HoFKind::Map).ret,
        Value::List(vec![
            Value::List(vec![Value::Int(11), Value::Int(21)]),
            Value::List(vec![Value::Int(12), Value::Int(22)]),
        ])
    );
}

fn flat_map_effectful_lambda_behavior() -> Behavior {
    let sp = Span::synthetic(FileId(0));
    let mut b = IlBuilder::new(FileId(0));
    let xs_param = b.add(NodeKind::Param, Payload::Cid(0), sp, &[]);

    let x_param = b.add(NodeKind::Param, Payload::Cid(1), sp, &[]);
    let printed = b.add(NodeKind::Var, Payload::Cid(1), sp, &[]);
    let print = b.add(
        NodeKind::Call,
        Payload::Builtin(Builtin::Print),
        sp,
        &[printed],
    );
    let print_stmt = b.add(NodeKind::ExprStmt, Payload::None, sp, &[print]);
    let returned = b.add(NodeKind::Var, Payload::Cid(1), sp, &[]);
    let single = b.add(NodeKind::Seq, Payload::None, sp, &[returned]);
    let lambda_ret = b.add(NodeKind::Return, Payload::None, sp, &[single]);
    let lambda_body = b.add(
        NodeKind::Block,
        Payload::None,
        sp,
        &[print_stmt, lambda_ret],
    );
    let lambda = b.add(NodeKind::Lambda, Payload::None, sp, &[x_param, lambda_body]);
    let xs = b.add(NodeKind::Var, Payload::Cid(0), sp, &[]);
    let flat_map = b.add(
        NodeKind::HoF,
        Payload::HoF(HoFKind::FlatMap),
        sp,
        &[xs, lambda],
    );
    let ret = b.add(NodeKind::Return, Payload::None, sp, &[flat_map]);
    let func = b.add(NodeKind::Func, Payload::None, sp, &[xs_param, ret]);
    let il = b.finish(
        func,
        FileMeta {
            path: "t.py".into(),
            lang: Lang::Python,
        },
        Vec::new(),
        Vec::new(),
    );
    run_admitted_unit(il, func, &[Value::List(vec![Value::Int(1), Value::Int(2)])])
        .expect("run_unit")
}

#[test]
fn flat_map_effectful_lambda_records_effects() {
    let behavior = flat_map_effectful_lambda_behavior();
    assert_eq!(
        behavior.ret,
        Value::List(vec![Value::Int(1), Value::Int(2)])
    );
    assert_eq!(behavior.effects, vec![Value::Int(1), Value::Int(2)]);
}
