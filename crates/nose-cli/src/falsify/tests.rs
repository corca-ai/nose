use super::domains::domain_pool;
use super::*;
use nose_il::{
    EvidenceAnchor, EvidenceId, EvidenceKind, EvidenceProvenance, EvidenceRecord, EvidenceStatus,
    FileId, FileMeta, IlBuilder, Lang, NodeKind, Op, Payload, Span,
};

mod domains;
mod projections;

fn set_param_domain(il: &mut Il, root: NodeId, domain: DomainEvidence) {
    let params: Vec<NodeId> = il
        .children(root)
        .iter()
        .copied()
        .filter(|&node| il.kind(node) == NodeKind::Param)
        .collect();
    for (index, param) in params.into_iter().enumerate() {
        il.evidence.push(EvidenceRecord::new(
            EvidenceId(index as u32),
            EvidenceAnchor::param(il.node(param).span),
            EvidenceKind::Domain(domain),
            EvidenceProvenance::builtin("nose.falsify.test", "declared-domain"),
            Vec::new(),
            EvidenceStatus::Asserted,
        ));
    }
}

fn finish(b: IlBuilder, root: NodeId, lang: Lang) -> Il {
    b.finish(
        root,
        FileMeta {
            path: "falsify-test".into(),
            lang,
        },
        Vec::new(),
        Vec::new(),
    )
}

fn two_arg_binop(op: Op, order: (u32, u32), lang: Lang) -> (Il, Interner, NodeId) {
    let interner = Interner::new();
    let sp = Span::synthetic(FileId(0));
    let mut b = IlBuilder::new(FileId(0));
    let pa = b.add(NodeKind::Param, Payload::Cid(0), sp, &[]);
    let pb = b.add(NodeKind::Param, Payload::Cid(1), sp, &[]);
    let left = b.add(NodeKind::Var, Payload::Cid(order.0), sp, &[]);
    let right = b.add(NodeKind::Var, Payload::Cid(order.1), sp, &[]);
    let bin = b.add(NodeKind::BinOp, Payload::Op(op), sp, &[left, right]);
    let ret = b.add(NodeKind::Return, Payload::None, sp, &[bin]);
    let root = b.add(NodeKind::Func, Payload::None, sp, &[pa, pb, ret]);
    (finish(b, root, lang), interner, root)
}

fn three_arg_add(left_associative: bool, lang: Lang) -> (Il, Interner, NodeId) {
    let interner = Interner::new();
    let sp = Span::synthetic(FileId(0));
    let mut b = IlBuilder::new(FileId(0));
    let params: Vec<NodeId> = (0..3)
        .map(|cid| b.add(NodeKind::Param, Payload::Cid(cid), sp, &[]))
        .collect();
    let vars: Vec<NodeId> = (0..3)
        .map(|cid| b.add(NodeKind::Var, Payload::Cid(cid), sp, &[]))
        .collect();
    let (inner, outer_other) = if left_associative {
        let inner = b.add(
            NodeKind::BinOp,
            Payload::Op(Op::Add),
            sp,
            &[vars[0], vars[1]],
        );
        (inner, vars[2])
    } else {
        let inner = b.add(
            NodeKind::BinOp,
            Payload::Op(Op::Add),
            sp,
            &[vars[1], vars[2]],
        );
        (inner, vars[0])
    };
    let outer_children = if left_associative {
        [inner, outer_other]
    } else {
        [outer_other, inner]
    };
    let outer = b.add(NodeKind::BinOp, Payload::Op(Op::Add), sp, &outer_children);
    let ret = b.add(NodeKind::Return, Payload::None, sp, &[outer]);
    let root = b.add(
        NodeKind::Func,
        Payload::None,
        sp,
        &[params[0], params[1], params[2], ret],
    );
    (finish(b, root, lang), interner, root)
}

fn mutation_at(index: i64) -> (Il, Interner, NodeId) {
    let interner = Interner::new();
    let sp = Span::synthetic(FileId(0));
    let mut b = IlBuilder::new(FileId(0));
    let collection = b.add(NodeKind::Param, Payload::Cid(0), sp, &[]);
    let value = b.add(NodeKind::Param, Payload::Cid(1), sp, &[]);
    let target_collection = b.add(NodeKind::Var, Payload::Cid(0), sp, &[]);
    let target_index = b.add(NodeKind::Lit, Payload::LitInt(index), sp, &[]);
    let target = b.add(
        NodeKind::Index,
        Payload::None,
        sp,
        &[target_collection, target_index],
    );
    let assigned_value = b.add(NodeKind::Var, Payload::Cid(1), sp, &[]);
    let assign = b.add(
        NodeKind::Assign,
        Payload::None,
        sp,
        &[target, assigned_value],
    );
    let result = b.add(NodeKind::Var, Payload::Cid(0), sp, &[]);
    let ret = b.add(NodeKind::Return, Payload::None, sp, &[result]);
    let block = b.add(NodeKind::Block, Payload::None, sp, &[assign, ret]);
    let root = b.add(
        NodeKind::Func,
        Payload::None,
        sp,
        &[collection, value, block],
    );
    (finish(b, root, Lang::Python), interner, root)
}

fn unary_float(negate: bool, lang: Lang) -> (Il, Interner, NodeId) {
    let interner = Interner::new();
    let sp = Span::synthetic(FileId(0));
    let mut b = IlBuilder::new(FileId(0));
    let param = b.add(NodeKind::Param, Payload::Cid(0), sp, &[]);
    let value = b.add(NodeKind::Var, Payload::Cid(0), sp, &[]);
    let value = if negate {
        b.add(NodeKind::UnOp, Payload::Op(Op::Neg), sp, &[value])
    } else {
        value
    };
    let ret = b.add(NodeKind::Return, Payload::None, sp, &[value]);
    let root = b.add(NodeKind::Func, Payload::None, sp, &[param, ret]);
    (finish(b, root, lang), interner, root)
}

fn calibration_artifact() -> serde_json::Value {
    serde_json::from_str(include_str!(
        "../../../../bench/soundness/0.20.0/source-runtime-calibration.v1.json"
    ))
    .expect("checked source-runtime calibration must be valid JSON")
}

#[test]
fn string_order_witness_is_seeded_shrunk_and_deterministic() {
    let (mut il_a, interner, root_a) = two_arg_binop(Op::Add, (0, 1), Lang::Python);
    let (mut il_b, _, root_b) = two_arg_binop(Op::Add, (1, 0), Lang::Python);
    set_param_domain(&mut il_a, root_a, DomainEvidence::String);
    set_param_domain(&mut il_b, root_b, DomainEvidence::String);
    let first = falsify_pair(
        &il_a,
        root_a,
        &il_b,
        root_b,
        &interner,
        &[],
        4096,
        DEFAULT_FALSIFY_SEED,
    )
    .expect("string order must have a concrete witness");
    let second = falsify_pair(
        &il_a,
        root_a,
        &il_b,
        root_b,
        &interner,
        &[],
        4096,
        DEFAULT_FALSIFY_SEED,
    )
    .expect("same seed must replay");

    assert_eq!(first, second);
    assert_eq!(first.seed, DEFAULT_FALSIFY_SEED);
    assert!(matches!((&first.shrunk_inputs[0], &first.shrunk_inputs[1]),
        (Value::Str(left), Value::Str(right)) if left != right));
    assert_eq!(
        format_inputs(&first.shrunk_inputs),
        format_inputs(&second.shrunk_inputs)
    );
}

#[test]
fn float_associativity_hard_negative_is_found_automatically() {
    for (lang, domain) in [
        (Lang::Python, DomainEvidence::Float),
        (Lang::TypeScript, DomainEvidence::Number),
    ] {
        let (mut il_a, interner, root_a) = three_arg_add(true, lang);
        let (mut il_b, _, root_b) = three_arg_add(false, lang);
        set_param_domain(&mut il_a, root_a, domain);
        set_param_domain(&mut il_b, root_b, domain);
        let witness = falsify_pair(
            &il_a,
            root_a,
            &il_b,
            root_b,
            &interner,
            &[],
            64,
            DEFAULT_FALSIFY_SEED,
        )
        .expect("IEEE-754 addition is not associative");

        assert!(witness
            .shrunk_inputs
            .iter()
            .any(|value| matches!(value, Value::Float(_))));
    }
}

#[test]
fn javascript_int32_width_difference_is_found_automatically() {
    let (mut js, interner, js_root) = two_arg_binop(Op::BitAnd, (0, 1), Lang::JavaScript);
    let (mut python, _, python_root) = two_arg_binop(Op::BitAnd, (0, 1), Lang::Python);
    set_param_domain(&mut js, js_root, DomainEvidence::Integer);
    set_param_domain(&mut python, python_root, DomainEvidence::Integer);
    let witness = falsify_pair(
        &js,
        js_root,
        &python,
        python_root,
        &interner,
        &[],
        64,
        DEFAULT_FALSIFY_SEED,
    )
    .expect("JS int32 and Python integer bitwise semantics differ");

    assert!(witness
        .shrunk_inputs
        .iter()
        .all(|value| matches!(value, Value::Int(_))));
}

#[test]
fn mutation_coordinates_are_falsified_with_collection_inputs() {
    let (first, interner, first_root) = mutation_at(0);
    let (second, _, second_root) = mutation_at(1);
    let replay = ReplayPair {
        left: ReplayUnit {
            interpreter: PreparedInterpreter::new(&first, &interner, true),
            root: first_root,
        },
        right: ReplayUnit {
            interpreter: PreparedInterpreter::new(&second, &interner, true),
            root: second_root,
        },
        observe_exit: false,
    };
    assert!(replay.concrete_disagreement(&[
        Value::List(vec![Value::Int(1), Value::Int(2)]),
        Value::Int(9),
    ]));
    let witness = falsify_pair(
        &first,
        first_root,
        &second,
        second_root,
        &interner,
        &[],
        64,
        DEFAULT_FALSIFY_SEED,
    )
    .expect("writes to different collection coordinates must be distinguished");

    assert_eq!(witness.seed, DEFAULT_FALSIFY_SEED);
}

#[test]
fn incompatible_declared_domains_never_produce_a_hard_witness() {
    let (mut integer, interner, integer_root) = two_arg_binop(Op::Add, (0, 1), Lang::Python);
    let (mut string, _, string_root) = two_arg_binop(Op::Add, (1, 0), Lang::Python);
    set_param_domain(&mut integer, integer_root, DomainEvidence::Integer);
    set_param_domain(&mut string, string_root, DomainEvidence::String);

    assert!(falsify_pair(
        &integer,
        integer_root,
        &string,
        string_root,
        &interner,
        &[],
        4096,
        DEFAULT_FALSIFY_SEED,
    )
    .is_none());
}

#[test]
fn signed_zero_outputs_are_observable_but_nan_payloads_are_canonical() {
    let plus_zero = Behavior {
        ret: Value::Float(F64(0.0)),
        effects: Vec::new(),
        fields: Vec::new(),
    };
    let minus_zero = Behavior {
        ret: Value::Float(F64(-0.0)),
        effects: Vec::new(),
        fields: Vec::new(),
    };
    let first_nan = Behavior {
        ret: Value::Float(F64(f64::from_bits(0x7ff8_0000_0000_0001))),
        effects: Vec::new(),
        fields: Vec::new(),
    };
    let second_nan = Behavior {
        ret: Value::Float(F64(f64::from_bits(0x7ff8_0000_0000_0002))),
        effects: Vec::new(),
        fields: Vec::new(),
    };

    assert!(behaviors_concretely_differ(&plus_zero, &minus_zero));
    assert!(!behaviors_concretely_differ(&first_nan, &second_nan));

    let (mut identity, interner, identity_root) = unary_float(false, Lang::Python);
    let (mut negate, _, negate_root) = unary_float(true, Lang::Python);
    set_param_domain(&mut identity, identity_root, DomainEvidence::Float);
    set_param_domain(&mut negate, negate_root, DomainEvidence::Float);
    let replay = ReplayPair {
        left: ReplayUnit {
            interpreter: PreparedInterpreter::new(&identity, &interner, true),
            root: identity_root,
        },
        right: ReplayUnit {
            interpreter: PreparedInterpreter::new(&negate, &interner, true),
            root: negate_root,
        },
        observe_exit: false,
    };
    assert!(replay.concrete_disagreement(&[Value::Float(F64(0.0))]));
    assert!(falsify_pair(
        &identity,
        identity_root,
        &negate,
        negate_root,
        &interner,
        &[],
        64,
        DEFAULT_FALSIFY_SEED,
    )
    .is_some());
}

#[test]
fn unhosted_domains_and_missing_static_evidence_never_produce_a_hard_witness() {
    let (mut map_a, interner, map_a_root) = two_arg_binop(Op::Add, (0, 1), Lang::Rust);
    let (mut map_b, _, map_b_root) = two_arg_binop(Op::Add, (1, 0), Lang::Rust);
    set_param_domain(&mut map_a, map_a_root, DomainEvidence::Map);
    set_param_domain(&mut map_b, map_b_root, DomainEvidence::Map);
    assert!(falsify_pair(
        &map_a,
        map_a_root,
        &map_b,
        map_b_root,
        &interner,
        &[],
        64,
        DEFAULT_FALSIFY_SEED,
    )
    .is_none());

    let (static_a, interner, static_a_root) = two_arg_binop(Op::Add, (0, 1), Lang::Rust);
    let (static_b, _, static_b_root) = two_arg_binop(Op::Add, (1, 0), Lang::Rust);
    assert!(falsify_pair(
        &static_a,
        static_a_root,
        &static_b,
        static_b_root,
        &interner,
        &[],
        64,
        DEFAULT_FALSIFY_SEED,
    )
    .is_none());

    let (dynamic_a, interner, dynamic_a_root) = two_arg_binop(Op::Add, (0, 1), Lang::Python);
    let (dynamic_b, _, dynamic_b_root) = two_arg_binop(Op::Add, (1, 0), Lang::Python);
    assert!(falsify_pair(
        &dynamic_a,
        dynamic_a_root,
        &dynamic_b,
        dynamic_b_root,
        &interner,
        &[],
        64,
        DEFAULT_FALSIFY_SEED,
    )
    .is_some());
}

#[test]
fn number_pool_and_interpreter_host_ieee754_values() {
    let (mut identity, interner, root) = unary_float(false, Lang::TypeScript);
    set_param_domain(&mut identity, root, DomainEvidence::Number);

    let pool = domain_pool(Some(DomainEvidence::Number), &[]);
    assert!(pool.iter().all(|value| matches!(value, Value::Float(_))));
    assert_eq!(
        run_unit(&identity, &interner, root, &[Value::Float(F64(1.25))])
            .expect("Number input must interpret")
            .ret,
        Value::Float(F64(1.25))
    );
    assert_eq!(
        run_unit(&identity, &interner, root, &[Value::Int(7)])
            .expect("Number integer must promote")
            .ret,
        Value::Float(F64(7.0))
    );
}

#[test]
fn source_runtime_calibration_names_every_required_oracle_distinction() {
    let artifact = calibration_artifact();
    assert_eq!(
        artifact["required_oracle_distinctions"],
        serde_json::json!([
            "derived_float_associativity",
            "float_associativity",
            "javascript_number_edges",
            "javascript_int32_width",
            "mutation_coordinate",
            "string_order"
        ])
    );
}

#[test]
fn interpreter_matches_checked_source_runtime_calibration_facts() {
    let artifact = calibration_artifact();

    let (float_left, interner, float_left_root) = three_arg_add(true, Lang::Python);
    let (float_right, _, float_right_root) = three_arg_add(false, Lang::Python);
    let float_row = [
        Value::Float(F64(1e16)),
        Value::Float(F64(-1e16)),
        Value::Float(F64(1.0)),
    ];
    let left = run_unit(&float_left, &interner, float_left_root, &float_row)
        .expect("left float association must interpret")
        .ret;
    let right = run_unit(&float_right, &interner, float_right_root, &float_row)
        .expect("right float association must interpret")
        .ret;
    let bits = |value: Value| match value {
        Value::Float(F64(value)) => format!("{:016x}", value.to_bits()),
        other => panic!("expected calibrated float, got {other:?}"),
    };
    assert_eq!(
        bits(left),
        artifact["observations"]["python"]["float_associativity"]["left_bits"]
    );
    assert_eq!(
        bits(right),
        artifact["observations"]["python"]["float_associativity"]["right_bits"]
    );

    let (js, interner, js_root) = two_arg_binop(Op::BitAnd, (0, 1), Lang::JavaScript);
    let (python, _, python_root) = two_arg_binop(Op::BitAnd, (0, 1), Lang::Python);
    let integer_row = [Value::Int(0xF_0000_0003), Value::Int(0xF_0000_0005)];
    let integer = |il: &Il, root: NodeId| match run_unit(il, &interner, root, &integer_row)
        .expect("integer-width calibration must interpret")
        .ret
    {
        Value::Int(value) => value.to_string(),
        other => panic!("expected calibrated integer, got {other:?}"),
    };
    assert_eq!(
        integer(&js, js_root),
        artifact["observations"]["node"]["integer_width"]["bitand"]
    );
    assert_eq!(
        integer(&python, python_root),
        artifact["observations"]["python"]["integer_width"]["bitand"]
    );

    let (mutation_0, interner, mutation_0_root) = mutation_at(0);
    let (mutation_1, _, mutation_1_root) = mutation_at(1);
    let mutation_row = [
        Value::List(vec![Value::Int(1), Value::Int(2)]),
        Value::Int(9),
    ];
    let mutation = |il: &Il, root: NodeId| {
        run_unit(il, &interner, root, &mutation_row)
            .expect("mutation calibration must interpret")
            .ret
    };
    assert_eq!(
        mutation(&mutation_0, mutation_0_root),
        Value::List(vec![Value::Int(9), Value::Int(2)])
    );
    assert_eq!(
        mutation(&mutation_1, mutation_1_root),
        Value::List(vec![Value::Int(1), Value::Int(9)])
    );
}

#[test]
fn identical_units_have_no_distinguisher() {
    let (il_a, interner, root_a) = two_arg_binop(Op::Add, (0, 1), Lang::Python);
    let (il_b, _, root_b) = two_arg_binop(Op::Add, (0, 1), Lang::Python);
    assert!(falsify_pair(
        &il_a,
        root_a,
        &il_b,
        root_b,
        &interner,
        &[],
        4096,
        DEFAULT_FALSIFY_SEED,
    )
    .is_none());
}
