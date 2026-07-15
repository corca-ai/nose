use super::*;
use nose_il::{
    EvidenceAnchor, EvidenceId, EvidenceKind, EvidenceProvenance, EvidenceRecord, EvidenceStatus,
    FileId, FileMeta, IlBuilder, Lang, Op, Span,
};

fn set_param_domain(il: &mut Il, root: NodeId, domain: DomainEvidence) {
    let param = il.children(root)[0];
    il.evidence.push(EvidenceRecord::new(
        EvidenceId(0),
        EvidenceAnchor::param(il.node(param).span),
        EvidenceKind::Domain(domain),
        EvidenceProvenance::builtin("nose.falsify.test", "declared-domain"),
        Vec::new(),
        EvidenceStatus::Asserted,
    ));
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

fn three_arg_add(left_associative: bool) -> (Il, Interner, NodeId) {
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
    (finish(b, root, Lang::Python), interner, root)
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
    let (mut il_a, interner, root_a) = three_arg_add(true);
    let (mut il_b, _, root_b) = three_arg_add(false);
    set_param_domain(&mut il_a, root_a, DomainEvidence::Float);
    set_param_domain(&mut il_b, root_b, DomainEvidence::Float);
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
        .inputs
        .iter()
        .all(|value| matches!(value, Value::Float(_))));
    assert!(witness
        .shrunk_inputs
        .iter()
        .all(|value| matches!(value, Value::Float(_))));
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
    let (mut first, interner, first_root) = mutation_at(0);
    let (mut second, _, second_root) = mutation_at(1);
    set_param_domain(&mut first, first_root, DomainEvidence::Collection);
    set_param_domain(&mut second, second_root, DomainEvidence::Collection);
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

    assert!(witness
        .inputs
        .iter()
        .all(|value| matches!(value, Value::List(_))));
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
fn source_runtime_calibration_names_every_required_oracle_distinction() {
    let artifact = calibration_artifact();
    assert_eq!(
        artifact["required_oracle_distinctions"],
        serde_json::json!([
            "float_associativity",
            "javascript_int32_width",
            "mutation_coordinate",
            "string_order"
        ])
    );
}

#[test]
fn interpreter_matches_checked_source_runtime_calibration_facts() {
    let artifact = calibration_artifact();

    let (float_left, interner, float_left_root) = three_arg_add(true);
    let (float_right, _, float_right_root) = three_arg_add(false);
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

#[test]
fn every_supported_domain_has_distinct_boundary_values() {
    use DomainEvidence as D;
    let domains = [
        D::Integer,
        D::Float,
        D::Number,
        D::Boolean,
        D::String,
        D::Array,
        D::ByteArray,
        D::Collection,
        D::Iterable,
        D::Iterator,
        D::Set,
        D::Option,
    ];
    for domain in domains {
        let pool = domain_pool(Some(domain), &[]);
        assert!(pool.len() >= 2, "{domain:?} domain is under-sampled");
        assert!(pool.iter().all(|value| value_conforms(value, Some(domain))));
    }
    let float_receipts: Vec<String> = float_values().iter().map(format_value).collect();
    assert!(float_receipts.contains(&"float:0e0".to_string()));
    assert!(float_receipts.contains(&"float:-0".to_string()));
    assert!(float_receipts.contains(&"float:nan".to_string()));
}
