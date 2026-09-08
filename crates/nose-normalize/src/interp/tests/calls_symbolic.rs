use super::*;

fn finish_python(b: IlBuilder, root: NodeId, units: Vec<Unit>) -> Il {
    b.finish(
        root,
        FileMeta {
            path: "t".into(),
            lang: Lang::Python,
        },
        units,
        Vec::new(),
    )
}

fn run_python_behavior(b: IlBuilder, root: NodeId, args: &[Value]) -> Behavior {
    run_admitted_unit(finish_python(b, root, Vec::new()), root, args).expect("run_unit")
}

fn run_python_ret(b: IlBuilder, root: NodeId, args: &[Value]) -> Value {
    run_python_behavior(b, root, args).ret
}

fn print_with_error_arg_then_return() -> Value {
    let sp = Span::synthetic(FileId(0));
    let mut b = IlBuilder::new(FileId(0));
    let one = b.add(NodeKind::Lit, Payload::LitInt(1), sp, &[]);
    let zero = b.add(NodeKind::Lit, Payload::LitInt(0), sp, &[]);
    let div = b.add(NodeKind::BinOp, Payload::Op(Op::Div), sp, &[one, zero]);
    let print = b.add(NodeKind::Call, Payload::Builtin(Builtin::Print), sp, &[div]);
    let print_stmt = b.add(NodeKind::ExprStmt, Payload::None, sp, &[print]);
    let seven = b.add(NodeKind::Lit, Payload::LitInt(7), sp, &[]);
    let ret = b.add(NodeKind::Return, Payload::None, sp, &[seven]);
    let block = b.add(NodeKind::Block, Payload::None, sp, &[print_stmt, ret]);
    let func = b.add(NodeKind::Func, Payload::None, sp, &[block]);
    run_python_ret(b, func, &[])
}

#[test]
fn eager_builtin_argument_err_stops_execution() {
    assert_eq!(print_with_error_arg_then_return(), Value::Err);
}

fn print_with_error_arg_before_effect_arg() -> Behavior {
    let sp = Span::synthetic(FileId(0));
    let mut b = IlBuilder::new(FileId(0));
    let one = b.add(NodeKind::Lit, Payload::LitInt(1), sp, &[]);
    let zero = b.add(NodeKind::Lit, Payload::LitInt(0), sp, &[]);
    let div = b.add(NodeKind::BinOp, Payload::Op(Op::Div), sp, &[one, zero]);
    let nested_print = b.add(NodeKind::Call, Payload::Builtin(Builtin::Print), sp, &[one]);
    let print = b.add(
        NodeKind::Call,
        Payload::Builtin(Builtin::Print),
        sp,
        &[div, nested_print],
    );
    let print_stmt = b.add(NodeKind::ExprStmt, Payload::None, sp, &[print]);
    let seven = b.add(NodeKind::Lit, Payload::LitInt(7), sp, &[]);
    let ret = b.add(NodeKind::Return, Payload::None, sp, &[seven]);
    let block = b.add(NodeKind::Block, Payload::None, sp, &[print_stmt, ret]);
    let func = b.add(NodeKind::Func, Payload::None, sp, &[block]);
    run_python_behavior(b, func, &[])
}

#[test]
fn eager_builtin_argument_err_stops_later_arguments() {
    let behavior = print_with_error_arg_before_effect_arg();
    assert_eq!(behavior.ret, Value::Err);
    assert!(behavior.effects.is_empty());
}

fn index_assignment_with_error_index_after_rhs_effect() -> Behavior {
    let sp = Span::synthetic(FileId(0));
    let mut b = IlBuilder::new(FileId(0));
    let param = b.add(NodeKind::Param, Payload::Cid(0), sp, &[]);
    let target_base = b.add(NodeKind::Var, Payload::Cid(0), sp, &[]);
    let one = b.add(NodeKind::Lit, Payload::LitInt(1), sp, &[]);
    let zero = b.add(NodeKind::Lit, Payload::LitInt(0), sp, &[]);
    let index_err = b.add(NodeKind::BinOp, Payload::Op(Op::Div), sp, &[one, zero]);
    let target = b.add(
        NodeKind::Index,
        Payload::None,
        sp,
        &[target_base, index_err],
    );
    let print = b.add(NodeKind::Call, Payload::Builtin(Builtin::Print), sp, &[one]);
    let assign = b.add(NodeKind::Assign, Payload::None, sp, &[target, print]);
    let seven = b.add(NodeKind::Lit, Payload::LitInt(7), sp, &[]);
    let ret = b.add(NodeKind::Return, Payload::None, sp, &[seven]);
    let block = b.add(NodeKind::Block, Payload::None, sp, &[assign, ret]);
    let func = b.add(NodeKind::Func, Payload::None, sp, &[param, block]);
    run_python_behavior(b, func, &[Value::List(Vec::new())])
}

#[test]
fn index_assignment_error_index_stops_after_rhs_effect() {
    let behavior = index_assignment_with_error_index_after_rhs_effect();
    assert_eq!(behavior.ret, Value::Err);
    assert_eq!(behavior.effects, vec![Value::Int(1)]);
}

fn index_assignment_with_error_base_before_index_effect() -> Behavior {
    let sp = Span::synthetic(FileId(0));
    let mut b = IlBuilder::new(FileId(0));
    let one = b.add(NodeKind::Lit, Payload::LitInt(1), sp, &[]);
    let zero = b.add(NodeKind::Lit, Payload::LitInt(0), sp, &[]);
    let base_err = b.add(NodeKind::BinOp, Payload::Op(Op::Div), sp, &[one, zero]);
    let print = b.add(NodeKind::Call, Payload::Builtin(Builtin::Print), sp, &[one]);
    let target = b.add(NodeKind::Index, Payload::None, sp, &[base_err, print]);
    let seven = b.add(NodeKind::Lit, Payload::LitInt(7), sp, &[]);
    let assign = b.add(NodeKind::Assign, Payload::None, sp, &[target, seven]);
    let later = b.add(NodeKind::Lit, Payload::LitInt(9), sp, &[]);
    let ret = b.add(NodeKind::Return, Payload::None, sp, &[later]);
    let block = b.add(NodeKind::Block, Payload::None, sp, &[assign, ret]);
    let func = b.add(NodeKind::Func, Payload::None, sp, &[block]);
    run_python_behavior(b, func, &[])
}

#[test]
fn index_assignment_checks_error_base_before_index_expr() {
    let behavior = index_assignment_with_error_base_before_index_effect();
    assert_eq!(behavior.ret, Value::Err);
    assert!(behavior.effects.is_empty());
}

fn self_call_with_error_arg_ignored_by_callee() -> Value {
    let sp = Span::synthetic(FileId(0));
    let mut b = IlBuilder::new(FileId(0));
    let interner = Interner::new();
    let func_name = interner.intern("f");
    let done_param = b.add(NodeKind::Param, Payload::Cid(0), sp, &[]);
    let ignored_param = b.add(NodeKind::Param, Payload::Cid(1), sp, &[]);
    let done_var = b.add(NodeKind::Var, Payload::Cid(0), sp, &[]);
    let seven = b.add(NodeKind::Lit, Payload::LitInt(7), sp, &[]);
    let done_ret = b.add(NodeKind::Return, Payload::None, sp, &[seven]);
    let if_done = b.add(NodeKind::If, Payload::None, sp, &[done_var, done_ret]);
    let callee = b.add(NodeKind::Var, Payload::Name(func_name), sp, &[]);
    let true_value = b.add(NodeKind::Lit, Payload::LitBool(true), sp, &[]);
    let one = b.add(NodeKind::Lit, Payload::LitInt(1), sp, &[]);
    let zero = b.add(NodeKind::Lit, Payload::LitInt(0), sp, &[]);
    let div = b.add(NodeKind::BinOp, Payload::Op(Op::Div), sp, &[one, zero]);
    let recursive_call = b.add(
        NodeKind::Call,
        Payload::None,
        sp,
        &[callee, true_value, div],
    );
    let recursive_ret = b.add(NodeKind::Return, Payload::None, sp, &[recursive_call]);
    let body = b.add(
        NodeKind::Block,
        Payload::None,
        sp,
        &[if_done, recursive_ret],
    );
    let func = b.add(
        NodeKind::Func,
        Payload::None,
        sp,
        &[done_param, ignored_param, body],
    );
    let mut il = finish_python(
        b,
        func,
        vec![Unit {
            root: func,
            kind: UnitKind::Function,
            name: Some(func_name),
            origin: Default::default(),
        }],
    );
    il.push_evidence(test_call_target_record(
        2000,
        il.node(recursive_call).span,
        il.node(func).span,
        interner.symbol_hash(func_name),
    ));
    run_admitted_unit_with_interner(il, &interner, func, &[Value::Bool(false), Value::Int(0)])
        .expect("run_unit")
        .ret
}

#[test]
fn self_call_argument_err_stops_execution() {
    assert_eq!(self_call_with_error_arg_ignored_by_callee(), Value::Err);
}

#[test]
fn unproven_call_becomes_symbolic_application_keyed_by_callee() {
    let sp = Span::synthetic(FileId(0));
    let mut b = IlBuilder::new(FileId(0));
    let interner = Interner::new();
    let func_name = interner.intern("f");
    let callee = b.add(NodeKind::Var, Payload::Name(func_name), sp, &[]);
    let call = b.add(NodeKind::Call, Payload::None, sp, &[callee]);
    let ret = b.add(NodeKind::Return, Payload::None, sp, &[call]);
    let body = b.add(NodeKind::Block, Payload::None, sp, &[ret]);
    let func = b.add(NodeKind::Func, Payload::None, sp, &[body]);
    let il = finish_python(
        b,
        func,
        vec![Unit {
            root: func,
            kind: UnitKind::Function,
            name: Some(func_name),
            origin: Default::default(),
        }],
    );

    // No call-target evidence: the call is OPAQUE, not a bail — a symbolic
    // application keyed by the callee's structural signature, effect-recorded.
    let beh = run_unit(&il, &interner, func, &[]).expect("symbolic run");
    assert!(matches!(beh.ret, Value::Sym(_)));
    assert_eq!(beh.effects, vec![beh.ret.clone()]);
}

/// Build `fn() { return <name>(<litint arg>) }` with an unproven callee.
fn opaque_call_behavior(name: &str, arg: i64, interner: &Interner) -> Behavior {
    let sp = Span::synthetic(FileId(0));
    let mut b = IlBuilder::new(FileId(0));
    let callee = b.add(NodeKind::Var, Payload::Name(interner.intern(name)), sp, &[]);
    let a = b.add(NodeKind::Lit, Payload::LitInt(arg), sp, &[]);
    let call = b.add(NodeKind::Call, Payload::None, sp, &[callee, a]);
    let ret = b.add(NodeKind::Return, Payload::None, sp, &[call]);
    let body = b.add(NodeKind::Block, Payload::None, sp, &[ret]);
    let func = b.add(NodeKind::Func, Payload::None, sp, &[body]);
    let il = finish_python(b, func, Vec::new());
    run_unit(&il, interner, func, &[]).expect("symbolic run")
}

#[test]
fn symbolic_application_is_differential_same_call_equal_else_distinct() {
    let interner = Interner::new();
    // Same callee + same argument: behaviors agree (the convention is comparable).
    assert_eq!(
        opaque_call_behavior("f", 3, &interner),
        opaque_call_behavior("f", 3, &interner)
    );
    // Different callee name or different argument: behaviors differ.
    assert_ne!(
        opaque_call_behavior("f", 3, &interner),
        opaque_call_behavior("g", 3, &interner)
    );
    assert_ne!(
        opaque_call_behavior("f", 3, &interner),
        opaque_call_behavior("f", 4, &interner)
    );
}

/// Two opaque calls in sequence: swapping their order swaps the effect trace —
/// call order stays observable behavior under the symbolic convention.
#[test]
fn opaque_call_order_is_observable() {
    let interner = Interner::new();
    let build = |first: &str, second: &str| {
        let sp = Span::synthetic(FileId(0));
        let mut b = IlBuilder::new(FileId(0));
        let stmt = |name: &str, b: &mut IlBuilder| {
            let callee = b.add(NodeKind::Var, Payload::Name(interner.intern(name)), sp, &[]);
            let call = b.add(NodeKind::Call, Payload::None, sp, &[callee]);
            b.add(NodeKind::ExprStmt, Payload::None, sp, &[call])
        };
        let s1 = stmt(first, &mut b);
        let s2 = stmt(second, &mut b);
        let body = b.add(NodeKind::Block, Payload::None, sp, &[s1, s2]);
        let func = b.add(NodeKind::Func, Payload::None, sp, &[body]);
        let il = finish_python(b, func, Vec::new());
        run_unit(&il, &interner, func, &[]).expect("symbolic run")
    };
    assert_eq!(build("f", "g"), build("f", "g"));
    assert_ne!(build("f", "g"), build("g", "f"));
}

/// Branching on a symbolic value still bails the unit — control flow is never guessed.
#[test]
fn branch_on_symbolic_value_bails() {
    let interner = Interner::new();
    let sp = Span::synthetic(FileId(0));
    let mut b = IlBuilder::new(FileId(0));
    let callee = b.add(NodeKind::Var, Payload::Name(interner.intern("f")), sp, &[]);
    let call = b.add(NodeKind::Call, Payload::None, sp, &[callee]);
    let t = b.add(NodeKind::Lit, Payload::LitInt(1), sp, &[]);
    let e = b.add(NodeKind::Lit, Payload::LitInt(2), sp, &[]);
    let ternary = b.add(NodeKind::If, Payload::None, sp, &[call, t, e]);
    let ret = b.add(NodeKind::Return, Payload::None, sp, &[ternary]);
    let body = b.add(NodeKind::Block, Payload::None, sp, &[ret]);
    let func = b.add(NodeKind::Func, Payload::None, sp, &[body]);
    let il = finish_python(b, func, Vec::new());
    assert!(run_unit(&il, &interner, func, &[]).is_none());
}

/// A symbolic operand inside a concrete operation must stay symbolic — collapsing
/// `len([f(x)])`-shaped compositions to a concrete `Err` would launder unknownness
/// into the hard soundness lane.
#[test]
fn symbolic_operand_never_launders_to_concrete_err() {
    let s = Value::Sym(7);
    assert!(matches!(bin(Op::Add, &Value::Int(1), &s), Value::Sym(_)));
    assert!(matches!(bin(Op::Eq, &s, &s), Value::Sym(_)));
    assert!(matches!(un(Op::Not, &s), Value::Sym(_)));
    assert!(matches!(
        un(Op::Neg, &Value::List(vec![s.clone()])),
        Value::Sym(_)
    ));
    assert!(contains_sym(&Value::List(vec![Value::Int(1), s])));
}

/// Err propagates from EITHER operand, so the SOUND `==`/`!=` operand-ordering canon
/// (which can move an erroring operand from left to right) does not look like a behavior
/// change. The fallthrough comparison arm would otherwise read `0 == Err` as a concrete
/// `Bool(false)` instead of raising (coevo series 9 oracle finding).
#[test]
fn err_propagates_from_either_operand() {
    for op in [Op::Eq, Op::Ne, Op::Lt, Op::Add, Op::Mul, Op::BitAnd] {
        assert!(
            matches!(bin(op, &Value::Int(0), &Value::Err), Value::Err),
            "{op:?}: right-operand Err must propagate"
        );
        assert!(
            matches!(bin(op, &Value::Err, &Value::Int(0)), Value::Err),
            "{op:?}: left-operand Err must propagate"
        );
    }
    // The pre-fix bug: `0 == Err` and `0 != Err` collapsed to concrete Bools.
    assert!(matches!(
        bin(Op::Eq, &Value::Int(0), &Value::Err),
        Value::Err
    ));
    assert!(matches!(
        bin(Op::Ne, &Value::Int(0), &Value::Err),
        Value::Err
    ));
}

/// `g(x) = x*x` and `f(x) = g(x) + 1` in one file — running `f(3)` must interpret the
/// cross-function call to `g` (not bail out as opaque), giving `3*3 + 1 = 10`. This is what
/// lets the oracle validate the interprocedural-inline canonicalization.
fn cross_function_call_result() -> Value {
    let sp = |n| Span {
        file: FileId(0),
        start_byte: n,
        end_byte: n + 1,
        start_line: n,
        end_line: n,
    };
    let mut b = IlBuilder::new(FileId(0));
    let interner = Interner::new();
    let g_name = interner.intern("g");
    let f_name = interner.intern("f");
    // g(x) = x * x
    let g_param = b.add(NodeKind::Param, Payload::Cid(0), sp(1), &[]);
    let gx1 = b.add(NodeKind::Var, Payload::Cid(0), sp(2), &[]);
    let gx2 = b.add(NodeKind::Var, Payload::Cid(0), sp(3), &[]);
    let g_mul = b.add(NodeKind::BinOp, Payload::Op(Op::Mul), sp(4), &[gx1, gx2]);
    let g_ret = b.add(NodeKind::Return, Payload::None, sp(5), &[g_mul]);
    let g_body = b.add(NodeKind::Block, Payload::None, sp(6), &[g_ret]);
    let g_func = b.add(NodeKind::Func, Payload::None, sp(7), &[g_param, g_body]);
    // f(x) = g(x) + 1
    let f_param = b.add(NodeKind::Param, Payload::Cid(0), sp(8), &[]);
    let callee = b.add(NodeKind::Var, Payload::Name(g_name), sp(9), &[]);
    let fx = b.add(NodeKind::Var, Payload::Cid(0), sp(10), &[]);
    let call = b.add(NodeKind::Call, Payload::None, sp(11), &[callee, fx]);
    let one = b.add(NodeKind::Lit, Payload::LitInt(1), sp(12), &[]);
    let f_add = b.add(NodeKind::BinOp, Payload::Op(Op::Add), sp(13), &[call, one]);
    let f_ret = b.add(NodeKind::Return, Payload::None, sp(14), &[f_add]);
    let f_body = b.add(NodeKind::Block, Payload::None, sp(15), &[f_ret]);
    let f_func = b.add(NodeKind::Func, Payload::None, sp(16), &[f_param, f_body]);
    let mut il = finish_python(
        b,
        f_func,
        vec![
            Unit {
                root: g_func,
                kind: UnitKind::Function,
                name: Some(g_name),
                origin: Default::default(),
            },
            Unit {
                root: f_func,
                kind: UnitKind::Function,
                name: Some(f_name),
                origin: Default::default(),
            },
        ],
    );
    il.push_evidence(test_call_target_record(
        2001,
        il.node(call).span,
        il.node(g_func).span,
        interner.symbol_hash(g_name),
    ));
    assert!(direct_function_call_target_at_call(
        &il, &interner, call, g_func
    ));
    run_admitted_unit_with_interner(il, &interner, f_func, &[Value::Int(3)])
        .expect("run_unit")
        .ret
}

#[test]
fn cross_function_call_is_interpreted() {
    assert_eq!(cross_function_call_result(), Value::Int(10));
}

fn run_any_all_with_error_predicate(all: bool) -> Value {
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
    let builtin = if all { Builtin::All } else { Builtin::Any };
    let call = b.add(
        NodeKind::Call,
        Payload::Builtin(builtin),
        sp,
        &[coll, lambda],
    );
    let ret = b.add(NodeKind::Return, Payload::None, sp, &[call]);
    let func = b.add(NodeKind::Func, Payload::None, sp, &[ret]);
    run_python_ret(b, func, &[])
}

#[test]
fn any_all_predicate_err_propagates() {
    assert_eq!(run_any_all_with_error_predicate(false), Value::Err);
    assert_eq!(run_any_all_with_error_predicate(true), Value::Err);
}

fn reduce_with_error_init_ignored_by_lambda() -> Value {
    let sp = Span::synthetic(FileId(0));
    let mut b = IlBuilder::new(FileId(0));
    let acc_param = b.add(NodeKind::Param, Payload::Cid(0), sp, &[]);
    let item_param = b.add(NodeKind::Param, Payload::Cid(1), sp, &[]);
    let seven = b.add(NodeKind::Lit, Payload::LitInt(7), sp, &[]);
    let lambda_ret = b.add(NodeKind::Return, Payload::None, sp, &[seven]);
    let lambda_body = b.add(NodeKind::Block, Payload::None, sp, &[lambda_ret]);
    let lambda = b.add(
        NodeKind::Lambda,
        Payload::None,
        sp,
        &[acc_param, item_param, lambda_body],
    );
    let one = b.add(NodeKind::Lit, Payload::LitInt(1), sp, &[]);
    let coll = b.add(NodeKind::Seq, Payload::None, sp, &[one]);
    let zero = b.add(NodeKind::Lit, Payload::LitInt(0), sp, &[]);
    let init_err = b.add(NodeKind::BinOp, Payload::Op(Op::Div), sp, &[one, zero]);
    let reduce = b.add(
        NodeKind::Call,
        Payload::Builtin(Builtin::Reduce),
        sp,
        &[lambda, coll, init_err],
    );
    let ret = b.add(NodeKind::Return, Payload::None, sp, &[reduce]);
    let func = b.add(NodeKind::Func, Payload::None, sp, &[ret]);
    run_python_ret(b, func, &[])
}

#[test]
fn reduce_init_err_propagates() {
    assert_eq!(reduce_with_error_init_ignored_by_lambda(), Value::Err);
}

/// Build `fn() { return base ** exp }` over integer literals and run it.
fn run_pow(base: i64, exp: i64) -> Value {
    let sp = Span::synthetic(FileId(0));
    let mut b = IlBuilder::new(FileId(0));
    let x = b.add(NodeKind::Lit, Payload::LitInt(base), sp, &[]);
    let y = b.add(NodeKind::Lit, Payload::LitInt(exp), sp, &[]);
    let pow = b.add(NodeKind::BinOp, Payload::Op(Op::Pow), sp, &[x, y]);
    let ret = b.add(NodeKind::Return, Payload::None, sp, &[pow]);
    let func = b.add(NodeKind::Func, Payload::None, sp, &[ret]);
    run_python_ret(b, func, &[])
}

#[test]
fn pow_negative_exponent_is_err_not_clamped_to_zero() {
    // The oracle models only i64; a negative exponent has no integer value, so it must
    // `Err` (like Div/Mod by zero) — NOT be silently clamped to `0`, which made
    // `2 ** -1` indistinguishable from `2 ** 0` and could license a false merge.
    assert_eq!(run_pow(2, 3), Value::Int(8));
    assert_eq!(run_pow(2, 0), Value::Int(1));
    assert_eq!(run_pow(2, -1), Value::Err);
}

#[test]
fn pow_exponent_beyond_u32_is_err_not_truncated() {
    // The exponent was cast `as u32`, so `2 ** 2^32` truncated to `2 ** 0 == 1` —
    // colliding distinct exponents. An exponent that doesn't fit u32 has no usable
    // value here, so it errs rather than wrap to a smaller exponent.
    assert_eq!(run_pow(2, 1 << 32), Value::Err);
    assert_eq!(run_pow(2, (1 << 32) + 5), Value::Err);
}

/// Build `fn() { return -lit }` over an integer literal and run it.
fn run_neg(v: i64) -> Value {
    let sp = Span::synthetic(FileId(0));
    let mut b = IlBuilder::new(FileId(0));
    let x = b.add(NodeKind::Lit, Payload::LitInt(v), sp, &[]);
    let neg = b.add(NodeKind::UnOp, Payload::Op(Op::Neg), sp, &[x]);
    let ret = b.add(NodeKind::Return, Payload::None, sp, &[neg]);
    let func = b.add(NodeKind::Func, Payload::None, sp, &[ret]);
    run_python_ret(b, func, &[])
}

#[test]
fn neg_of_i64_min_wraps_instead_of_panicking() {
    // Plain `-i` panics on `i64::MIN` (overflow); every other arithmetic op here uses
    // wrapping semantics, so negation must too — `wrapping_neg(i64::MIN) == i64::MIN`.
    assert_eq!(run_neg(5), Value::Int(-5));
    assert_eq!(run_neg(i64::MIN), Value::Int(i64::MIN));
}
