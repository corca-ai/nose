use super::*;

fn first_conditional_contract(il: &Il, interner: &Interner) -> FragmentContract {
    crate::fragment::recognize::recognized_contracts(il, interner)
        .into_iter()
        .find(|contract| contract.kind == FragmentKind::ConditionalGuard)
        .expect("conditional fragment contract")
}

#[test]
fn cardinality_projection_observes_early_return_without_erasing_elements() {
    let interner = Interner::new();
    let il = norm(
        &interner,
        "fn check(kids: &[u8]) -> Option<u8> {\n    if kids.len() != 2 {\n        return None;\n    }\n    Some(kids[0])\n}\n",
        Lang::Rust,
    );
    let contract = first_conditional_contract(&il, &interner);
    assert_eq!(
        fragment_input_projections(&il, &contract),
        vec![OracleInputProjection::Cardinality]
    );

    let (wrapper, root) = synthesize_wrapper(&il, &interner, &contract).expect("wrapper");
    let short = nose_normalize::run_unit_observing_exit(
        &wrapper,
        &interner,
        root,
        &[Value::List(vec![Value::Int(1)])],
    )
    .expect("exact Rust None is interpretable");
    let exact = nose_normalize::run_unit_observing_exit(
        &wrapper,
        &interner,
        root,
        &[Value::List(vec![Value::Int(1), Value::Int(2)])],
    )
    .expect("cardinality guard is interpretable");
    assert_eq!(short.0.ret, Value::Null);
    assert_eq!(exact.0.ret, Value::Null);
    assert_eq!(short.1, nose_normalize::UnitExit::Return);
    assert_eq!(exact.1, nose_normalize::UnitExit::Fallthrough);
}

#[test]
fn cardinality_projection_rejects_any_element_observation() {
    let interner = Interner::new();
    let il = norm(
        &interner,
        "fn check(kids: &[u8]) -> Option<u8> {\n    if kids.len() != 2 || kids[0] == 0 {\n        return None;\n    }\n    Some(kids[0])\n}\n",
        Lang::Rust,
    );
    let contract = first_conditional_contract(&il, &interner);
    assert_eq!(
        fragment_input_projections(&il, &contract),
        vec![OracleInputProjection::Declared]
    );
}

#[test]
fn cardinality_projection_checks_every_parent_edge_in_a_shared_dag() {
    let file = FileId(0);
    let span = Span::synthetic(file);
    let mut builder = IlBuilder::new(file);
    let input = builder.add(NodeKind::Var, Payload::Cid(0), span, &[]);
    let len = builder.add(
        NodeKind::Call,
        Payload::Builtin(Builtin::Len),
        span,
        &[input],
    );
    let zero = builder.add(NodeKind::Lit, Payload::LitInt(0), span, &[]);
    let index = builder.add(NodeKind::Index, Payload::None, span, &[input, zero]);
    let sequence = builder.add(NodeKind::Seq, Payload::None, span, &[len, index]);
    let root = builder.add(NodeKind::Return, Payload::None, span, &[sequence]);
    let il = builder.finish(
        root,
        FileMeta {
            path: "shared-dag.rs".into(),
            lang: Lang::Rust,
        },
        Vec::new(),
        Vec::new(),
    );
    let contract =
        FragmentContract::value_sink(FragmentKind::DirectReturn, root, vec![0], Exit::Return);

    assert_eq!(
        fragment_input_projections(&il, &contract),
        vec![OracleInputProjection::Declared],
        "a shared Var observed through Index must close the cardinality projection"
    );
}

#[test]
fn shadowed_rust_none_stays_uninterpretable() {
    let interner = Interner::new();
    let il = norm(
        &interner,
        "const None: Option<u8> = Some(7);\nfn check(kids: &[u8]) -> Option<u8> {\n    if kids.len() != 2 {\n        return None;\n    }\n    Some(kids[0])\n}\n",
        Lang::Rust,
    );
    let contract = first_conditional_contract(&il, &interner);
    let (wrapper, root) = synthesize_wrapper(&il, &interner, &contract).expect("wrapper");
    assert!(nose_normalize::run_unit_observing_exit(
        &wrapper,
        &interner,
        root,
        &[Value::List(vec![Value::Int(1)])],
    )
    .is_none());
}
