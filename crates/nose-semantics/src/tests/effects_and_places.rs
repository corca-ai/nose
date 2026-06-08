use super::*;

#[test]
fn exact_index_assignment_parts_require_effect_evidence() {
    let mut b = IlBuilder::new(FileId(0));
    let receiver = b.add(NodeKind::Var, Payload::Cid(1), sp(1), &[]);
    let key = b.add(NodeKind::Var, Payload::Cid(2), sp(1), &[]);
    let target = b.add(NodeKind::Index, Payload::None, sp(1), &[receiver, key]);
    let value = b.add(NodeKind::Var, Payload::Cid(3), sp(1), &[]);
    let assign = b.add(NodeKind::Assign, Payload::None, sp(1), &[target, value]);
    let mut il = finish_il(b, assign, Lang::Go);

    assert_eq!(
        exact_non_overloadable_index_assignment_parts(&il, assign),
        None
    );
    assert!(!exact_non_overloadable_index_assignment(&il, assign));

    push_node_effect(
        &mut il,
        0,
        assign,
        EffectEvidenceKind::NonOverloadableIndexWrite,
    );
    push_node_effect(&mut il, 2, assign, EffectEvidenceKind::BindingWrite);

    assert_eq!(
        exact_non_overloadable_index_assignment_parts(&il, assign),
        Some((receiver, Some(key), value))
    );
    assert!(exact_non_overloadable_index_assignment(&il, assign));

    push_node_effect(&mut il, 1, assign, EffectEvidenceKind::BuilderAppendCall);
    assert_eq!(
        exact_non_overloadable_index_assignment_parts(&il, assign),
        None
    );
    assert!(!exact_non_overloadable_index_assignment(&il, assign));
}

#[test]
fn builder_append_call_args_require_effect_evidence() {
    let interner = Interner::default();
    let append = interner.intern("append");
    let push = interner.intern("push");
    let mut b = IlBuilder::new(FileId(0));
    let receiver = b.add(NodeKind::Var, Payload::Cid(1), sp(1), &[]);
    let value = b.add(NodeKind::Var, Payload::Cid(2), sp(1), &[]);
    let builtin = b.add(
        NodeKind::Call,
        Payload::Builtin(Builtin::Append),
        sp(1),
        &[receiver, value],
    );
    let method = b.add(NodeKind::Field, Payload::Name(append), sp(2), &[receiver]);
    let call = b.add(NodeKind::Call, Payload::None, sp(2), &[method, value]);
    let push_method = b.add(NodeKind::Field, Payload::Name(push), sp(3), &[receiver]);
    let push_call = b.add(NodeKind::Call, Payload::None, sp(3), &[push_method, value]);
    let root = b.add(
        NodeKind::Block,
        Payload::None,
        sp(1),
        &[builtin, call, push_call],
    );
    let il = finish_il(b, root, Lang::Python);

    assert_eq!(builder_append_call_args(&il, &interner, builtin), None);
    let mut il = il;
    push_node_effect(&mut il, 0, builtin, EffectEvidenceKind::BuilderAppendCall);
    push_node_effect(&mut il, 3, builtin, EffectEvidenceKind::ReceiverMutation);
    assert_eq!(
        builder_append_call_args(&il, &interner, builtin),
        Some((receiver, value))
    );
    assert_eq!(builder_append_call_args(&il, &interner, call), None);
    assert_eq!(builder_append_call_args(&il, &interner, push_call), None);

    let mut rust_il = il.clone();
    rust_il.meta.lang = Lang::Rust;
    assert_eq!(
        builder_append_call_args(&rust_il, &interner, push_call),
        None
    );
}

#[test]
fn effect_evidence_can_prove_non_overloadable_index_write() {
    let mut b = IlBuilder::new(FileId(0));
    let receiver = b.add(NodeKind::Var, Payload::Cid(1), sp(1), &[]);
    let key = b.add(NodeKind::Var, Payload::Cid(2), sp(1), &[]);
    let target = b.add(NodeKind::Index, Payload::None, sp(1), &[receiver, key]);
    let value = b.add(NodeKind::Var, Payload::Cid(3), sp(1), &[]);
    let assign = b.add(NodeKind::Assign, Payload::None, sp(9), &[target, value]);
    let mut il = finish_il(b, assign, Lang::Ruby);

    assert_eq!(
        exact_non_overloadable_index_assignment_parts(&il, assign),
        None
    );

    push_node_effect(
        &mut il,
        0,
        assign,
        EffectEvidenceKind::NonOverloadableIndexWrite,
    );
    assert_eq!(
        exact_non_overloadable_index_assignment_parts(&il, assign),
        Some((receiver, Some(key), value))
    );

    push_node_effect(
        &mut il,
        1,
        assign,
        EffectEvidenceKind::SelfFieldWrite { field_hash: 1 },
    );
    assert_eq!(
        exact_non_overloadable_index_assignment_parts(&il, assign),
        None
    );

    let mut b = IlBuilder::new(FileId(0));
    let receiver = b.add(NodeKind::Var, Payload::Cid(1), sp(1), &[]);
    let key = b.add(NodeKind::Var, Payload::Cid(2), sp(1), &[]);
    let target = b.add(NodeKind::Index, Payload::None, sp(1), &[receiver, key]);
    let value = b.add(NodeKind::Var, Payload::Cid(3), sp(1), &[]);
    let call = b.add(NodeKind::Call, Payload::None, sp(10), &[target, value]);
    let mut non_assign = finish_il(b, call, Lang::Ruby);
    push_node_effect(
        &mut non_assign,
        0,
        call,
        EffectEvidenceKind::NonOverloadableIndexWrite,
    );
    assert_eq!(
        exact_non_overloadable_index_assignment_parts(&non_assign, call),
        None
    );
}

#[test]
fn append_effect_evidence_can_prove_raw_method_call() {
    let interner = Interner::default();
    let append = interner.intern("append");
    let mut b = IlBuilder::new(FileId(0));
    let receiver = b.add(NodeKind::Var, Payload::Cid(1), sp(1), &[]);
    let value = b.add(NodeKind::Var, Payload::Cid(2), sp(1), &[]);
    let method = b.add(NodeKind::Field, Payload::Name(append), sp(2), &[receiver]);
    let call = b.add(NodeKind::Call, Payload::None, sp(3), &[method, value]);
    let mut il = finish_il(b, call, Lang::Ruby);

    assert_eq!(builder_append_call_args(&il, &interner, call), None);

    push_node_effect(&mut il, 0, call, EffectEvidenceKind::BuilderAppendCall);
    assert_eq!(
        builder_append_call_args(&il, &interner, call),
        Some((receiver, value))
    );

    push_node_effect(
        &mut il,
        1,
        call,
        EffectEvidenceKind::NonOverloadableIndexWrite,
    );
    assert_eq!(builder_append_call_args(&il, &interner, call), None);
}

#[test]
fn place_evidence_is_authoritative_for_self_field_proof() {
    let interner = Interner::default();
    let this = interner.intern("this");
    let field_name = interner.intern("value");
    let field_hash = stable_symbol_hash("value");
    let mut b = IlBuilder::new(FileId(0));
    let receiver = b.add(NodeKind::Var, Payload::Name(this), sp(1), &[]);
    let field = b.add(
        NodeKind::Field,
        Payload::Name(field_name),
        sp(2),
        &[receiver],
    );
    let value = b.add(NodeKind::Var, Payload::Cid(1), sp(3), &[]);
    let assign = b.add(NodeKind::Assign, Payload::None, sp(4), &[field, value]);
    let ret = b.add(NodeKind::Return, Payload::None, sp(5), &[receiver]);
    let root = b.add(NodeKind::Block, Payload::None, sp(1), &[assign, ret]);
    let mut il = finish_il(b, root, Lang::Ruby);

    assert!(!exact_java_this_var(&il, &interner, receiver));
    assert!(!exact_java_this_field(&il, &interner, field));
    assert!(!exact_java_return_this(&il, &interner, ret));
    assert!(!exact_self_field_write_assignment(&il, &interner, assign));

    let receiver_evidence = push_node_place(&mut il, 0, receiver, PlaceEvidenceKind::SelfReceiver);
    let field_evidence = push_node_place_with_dependencies(
        &mut il,
        1,
        field,
        PlaceEvidenceKind::SelfField { field_hash },
        vec![receiver_evidence],
    );
    push_node_effect_with_dependencies(
        &mut il,
        2,
        assign,
        EffectEvidenceKind::SelfFieldWrite { field_hash },
        vec![field_evidence],
    );
    assert!(exact_java_this_var(&il, &interner, receiver));
    assert!(exact_java_this_field(&il, &interner, field));
    assert!(exact_java_return_this(&il, &interner, ret));
    assert!(exact_self_field_write_assignment(&il, &interner, assign));

    push_node_place(&mut il, 3, field, PlaceEvidenceKind::SelfReceiver);
    assert!(!exact_java_this_field(&il, &interner, field));
    assert!(!exact_self_field_write_assignment(&il, &interner, assign));

    push_node_place(
        &mut il,
        4,
        receiver,
        PlaceEvidenceKind::SelfField { field_hash },
    );
    assert!(!exact_java_this_var(&il, &interner, receiver));
    assert!(!exact_java_return_this(&il, &interner, ret));

    let other = interner.intern("other");
    let mut b = IlBuilder::new(FileId(0));
    let receiver = b.add(NodeKind::Var, Payload::Name(other), sp(5), &[]);
    let field = b.add(
        NodeKind::Field,
        Payload::Name(field_name),
        sp(6),
        &[receiver],
    );
    let value = b.add(NodeKind::Var, Payload::Cid(1), sp(7), &[]);
    let assign = b.add(NodeKind::Assign, Payload::None, sp(8), &[field, value]);
    let mut il = finish_il(b, assign, Lang::Ruby);
    push_node_place(
        &mut il,
        0,
        field,
        PlaceEvidenceKind::SelfField { field_hash },
    );
    push_node_effect(
        &mut il,
        1,
        assign,
        EffectEvidenceKind::SelfFieldWrite { field_hash },
    );
    assert!(!exact_java_this_field(&il, &interner, field));
    assert!(!exact_self_field_write_assignment(&il, &interner, assign));
}

#[test]
fn static_membership_predicate_operator_requires_js_strict_equality() {
    assert!(exact_static_membership_predicate_operator(
        Lang::JavaScript,
        Op::Eq,
        SourceOperatorKind::StrictEquality
    ));
    assert!(exact_static_membership_predicate_operator(
        Lang::TypeScript,
        Op::Ne,
        SourceOperatorKind::StrictInequality
    ));
    assert!(!exact_static_membership_predicate_operator(
        Lang::JavaScript,
        Op::Eq,
        SourceOperatorKind::LooseEquality
    ));
    assert!(!exact_static_membership_predicate_operator(
        Lang::Python,
        Op::Eq,
        SourceOperatorKind::ValueEquality
    ));
    assert!(!exact_static_membership_predicate_operator(
        Lang::JavaScript,
        Op::Eq,
        SourceOperatorKind::TypeMembership
    ));
}
