use super::*;

#[test]
fn value_domain_inference_treats_retained_float_literals_as_numeric() {
    assert_eq!(
        inferred_domains_for_added_literal(Payload::LitInt(3)),
        vec![ValueDomain::Number]
    );
    assert_eq!(
        inferred_domains_for_added_literal(Payload::LitFloat(0xBEEF)),
        vec![ValueDomain::Number]
    );
}

#[test]
fn value_domain_inference_does_not_treat_python_repetition_as_numeric_proof() {
    assert_eq!(
        inferred_domains_for_multiplied_literal(Lang::Python),
        vec![ValueDomain::Unknown],
        "Python `*` can be string/list repetition, so untyped operands must fail closed"
    );
    assert_eq!(
        inferred_domains_for_multiplied_literal(Lang::Ruby),
        vec![ValueDomain::Unknown],
        "Ruby `*` can be repetition, so untyped operands must fail closed"
    );
    assert_eq!(
        inferred_domains_for_multiplied_literal(Lang::Rust),
        vec![ValueDomain::Number],
        "Rust `*` remains a strict numeric operator for the first-party language profile"
    );
}

fn inferred_domains_for_multiplied_literal(lang: Lang) -> Vec<ValueDomain> {
    let sp = Span::synthetic(FileId(0));
    let mut b = IlBuilder::new(FileId(0));
    let param = b.add(NodeKind::Param, Payload::Cid(0), sp, &[]);
    let varx = b.add(NodeKind::Var, Payload::Cid(0), sp, &[]);
    let lit = b.add(NodeKind::Lit, Payload::LitInt(2), sp, &[]);
    let mul = b.add(NodeKind::BinOp, Payload::Op(Op::Mul), sp, &[varx, lit]);
    let ret = b.add(NodeKind::Return, Payload::None, sp, &[mul]);
    let func = b.add(NodeKind::Func, Payload::None, sp, &[param, ret]);
    let il = b.finish(
        func,
        FileMeta {
            path: "t".into(),
            lang,
        },
        Vec::new(),
        Vec::new(),
    );
    semantics(lang)
        .operators()
        .infer_param_value_domains(&il, func)
}

#[test]
fn first_party_profile_wraps_each_language() {
    for &lang in ALL_LANGS {
        let profile = semantics(lang);
        assert_eq!(profile.lang(), lang);
        assert_eq!(profile.pack_id(), FIRST_PARTY_PACK_ID);
        assert_eq!(profile.trust(), PackTrust::DefaultFirstParty);
    }
}

#[test]
fn domain_evidence_preserves_param_semantic_boundaries() {
    assert_eq!(
        domain_evidence_from_param_semantic(ParamSemantic::Array),
        DomainEvidence::Array
    );
    assert_eq!(
        domain_evidence_from_param_semantic(ParamSemantic::Boolean),
        DomainEvidence::Boolean
    );
    assert_eq!(
        domain_evidence_from_param_semantic(ParamSemantic::Float),
        DomainEvidence::Float
    );
    assert_eq!(
        domain_evidence_from_param_semantic(ParamSemantic::FutureLike),
        DomainEvidence::FutureLike
    );
    assert_eq!(
        domain_evidence_from_param_semantic(ParamSemantic::Iterable),
        DomainEvidence::Iterable
    );
    assert_eq!(
        domain_evidence_from_param_semantic(ParamSemantic::Iterator),
        DomainEvidence::Iterator
    );
    assert_eq!(
        domain_evidence_from_param_semantic(ParamSemantic::Record),
        DomainEvidence::Record
    );
    assert_eq!(
        domain_evidence_from_param_semantic(ParamSemantic::Result),
        DomainEvidence::Result
    );
    assert!(DomainEvidence::Array.is_array_collection_or_set());
    assert!(DomainEvidence::Boolean.is_boolean());
    assert!(DomainEvidence::Collection.is_array_or_collection());
    assert!(DomainEvidence::Set.is_collection_or_set());
    assert!(DomainEvidence::Float.is_float());
    assert!(DomainEvidence::FutureLike.is_future_like());
    assert!(DomainEvidence::PromiseLike.is_future_like());
    assert!(DomainEvidence::Iterable.is_iterable_or_iterator());
    assert!(DomainEvidence::Iterator.is_iterable_or_iterator());
    assert!(DomainEvidence::Map.is_map());
    assert!(DomainEvidence::Nominal {
        type_hash: stable_symbol_hash("pkg.Widget")
    }
    .is_nominal(stable_symbol_hash("pkg.Widget")));
    assert!(DomainEvidence::Option.is_option());
    assert!(DomainEvidence::PromiseLike.is_promise_like());
    assert!(DomainEvidence::Record.is_record());
    assert!(DomainEvidence::Result.is_result());
    assert!(DomainEvidence::String.is_string());
    assert!(DomainEvidence::ByteArray.is_byte_array());
    assert!(DomainEvidence::Integer.is_integer());
    assert!(DomainEvidence::Number.is_integer_or_number());
    assert!(DomainEvidence::Float.is_integer_or_number());
    assert!(DomainEvidence::Integer.is_integer_or_number());
    assert!(!DomainEvidence::Number.is_integer());
    assert!(!DomainEvidence::Array.is_collection_or_set());
    assert!(!DomainEvidence::Set.is_array_or_collection());
    assert!(DomainRequirement::Boolean.accepts(DomainEvidence::Boolean));
    assert!(DomainRequirement::CollectionOrMap.accepts(DomainEvidence::Array));
    assert!(DomainRequirement::CollectionOrMap.accepts(DomainEvidence::Map));
    assert!(DomainRequirement::Float.accepts(DomainEvidence::Float));
    assert!(DomainRequirement::FutureLike.accepts(DomainEvidence::FutureLike));
    assert!(DomainRequirement::FutureLike.accepts(DomainEvidence::PromiseLike));
    assert!(DomainRequirement::Iterable.accepts(DomainEvidence::Iterable));
    assert!(DomainRequirement::Iterator.accepts(DomainEvidence::Iterator));
    assert!(DomainRequirement::IterableOrIterator.accepts(DomainEvidence::Iterable));
    assert!(DomainRequirement::IterableOrIterator.accepts(DomainEvidence::Iterator));
    assert!(DomainRequirement::Nominal {
        type_hash: stable_symbol_hash("pkg.Widget")
    }
    .accepts(DomainEvidence::Nominal {
        type_hash: stable_symbol_hash("pkg.Widget")
    }));
    assert!(DomainRequirement::Number.accepts(DomainEvidence::Float));
    assert!(DomainRequirement::Record.accepts(DomainEvidence::Record));
    assert!(DomainRequirement::Result.accepts(DomainEvidence::Result));
    assert!(DomainRequirement::SetOrMap.accepts(DomainEvidence::Set));
    assert!(DomainRequirement::SetOrMap.accepts(DomainEvidence::Map));
    assert!(!DomainRequirement::SetOrMap.accepts(DomainEvidence::Collection));
    assert!(DomainRequirement::PromiseLike.accepts(DomainEvidence::PromiseLike));
    assert!(!DomainRequirement::PromiseLike.accepts(DomainEvidence::String));
    assert_eq!(
        ValueDomain::from_domain_evidence(DomainEvidence::Boolean),
        Some(ValueDomain::Boolean)
    );
    assert_eq!(
        ValueDomain::from_domain_evidence(DomainEvidence::Float),
        Some(ValueDomain::Number)
    );
    assert_eq!(
        ValueDomain::from_domain_evidence(DomainEvidence::PromiseLike),
        None
    );
}

#[test]
fn type_domain_contracts_are_language_scoped_and_exact_enough() {
    assert_eq!(
        type_domain_from_source_text(Lang::TypeScript, "xs: Array<string>"),
        Some(DomainEvidence::Array)
    );
    assert_eq!(
        type_domain_from_source_text(Lang::TypeScript, "xs: string[]"),
        Some(DomainEvidence::Array)
    );
    assert_eq!(
        type_domain_from_source_text(Lang::TypeScript, "xs: Iterable<string>"),
        Some(DomainEvidence::Iterable)
    );
    assert_eq!(
        type_domain_from_source_text(Lang::TypeScript, "xs: Iterator<string>"),
        Some(DomainEvidence::Iterator)
    );
    assert_eq!(
        type_domain_from_source_text(Lang::TypeScript, "xs: Promise<string>"),
        Some(DomainEvidence::PromiseLike)
    );
    assert_eq!(
        type_domain_from_source_text(Lang::TypeScript, "xs: Record<string, number>"),
        Some(DomainEvidence::Record)
    );
    assert_eq!(
        type_domain_from_source_text(Lang::TypeScript, "xs: Result<string, Error>"),
        Some(DomainEvidence::Result)
    );
    assert_eq!(
        type_domain_from_source_text(Lang::TypeScript, "xs: boolean"),
        Some(DomainEvidence::Boolean)
    );
    assert_eq!(
        type_domain_from_source_text(Lang::TypeScript, "xs: Bitmap<string, number>"),
        None
    );
    assert_eq!(
        type_domain_from_source_text(Lang::TypeScript, "xs: Blacklist<string>"),
        None
    );

    assert_eq!(
        type_domain_from_source_text(Lang::Java, "@Nonnull List<String> xs"),
        Some(DomainEvidence::Collection)
    );
    assert_eq!(
        type_domain_from_source_text(Lang::Java, "Iterator<String> xs"),
        Some(DomainEvidence::Iterator)
    );
    assert_eq!(
        type_domain_from_source_text(Lang::Java, "CompletableFuture<String> xs"),
        Some(DomainEvidence::FutureLike)
    );
    assert_eq!(
        type_domain_from_source_text(Lang::Java, "boolean value"),
        Some(DomainEvidence::Boolean)
    );
    assert_eq!(
        type_domain_from_source_text(Lang::Java, "@Ann(\"...\") String value"),
        Some(DomainEvidence::String)
    );

    assert_eq!(
        type_domain_from_source_text(Lang::Rust, "std::collections::HashMap<String, i32>"),
        Some(DomainEvidence::Map)
    );
    assert_eq!(
        type_domain_from_source_text(Lang::Rust, "HashSet<i32>"),
        Some(DomainEvidence::Set)
    );
    assert_eq!(
        type_domain_from_source_text(Lang::Rust, "Result<String, Error>"),
        Some(DomainEvidence::Result)
    );
    assert_eq!(
        type_domain_from_source_text(Lang::Rust, "impl Iterator<Item = i32>"),
        Some(DomainEvidence::Iterator)
    );
    assert_eq!(
        type_domain_from_source_text(Lang::Rust, "std::pin::Pin<Box<T>>"),
        None
    );
    assert_eq!(
        type_domain_from_source_text(Lang::Rust, "bool"),
        Some(DomainEvidence::Boolean)
    );

    assert_eq!(type_domain_from_source_text(Lang::C, "int *xs"), None);
    assert_eq!(
        type_domain_from_source_text(Lang::C, "int xs"),
        Some(DomainEvidence::Integer)
    );
    assert_eq!(
        type_domain_from_source_text(Lang::C, "_Bool ok"),
        Some(DomainEvidence::Boolean)
    );
    assert_eq!(
        type_domain_from_source_text(Lang::Go, "value bool"),
        Some(DomainEvidence::Boolean)
    );
    assert_eq!(
        type_domain_from_source_text(Lang::Go, "type User struct { id int }"),
        Some(DomainEvidence::Record)
    );

    let iterable = python_stdlib_type_domain_contract("typing", "Iterable")
        .expect("typing.Iterable should be a first-party pack row");
    assert_eq!(iterable.pack_id, PYTHON_STDLIB_TYPE_DOMAIN_PACK_ID);
    assert_eq!(iterable.producer_id, PYTHON_STDLIB_TYPE_DOMAIN_PRODUCER_ID);
    assert_eq!(iterable.domain, DomainEvidence::Iterable);
    assert_eq!(
        python_stdlib_type_domain("collections.abc", "Mapping"),
        Some(DomainEvidence::Map)
    );
    assert_eq!(
        python_stdlib_type_domain("collections.abc", "Awaitable"),
        Some(DomainEvidence::FutureLike)
    );
    assert_eq!(
        python_stdlib_type_domain("asyncio", "Future"),
        Some(DomainEvidence::FutureLike)
    );
    assert_eq!(python_stdlib_type_domain("typing", "Blacklist"), None);
    assert_eq!(python_stdlib_type_domain("typing", "Future"), None);
    assert_eq!(python_stdlib_type_domain("collections.abc", "Dict"), None);
    assert_eq!(
        python_stdlib_type_domain("collections.abc", "FrozenSet"),
        None
    );
    assert_eq!(python_stdlib_type_domain("collections.abc", "Deque"), None);
    assert_eq!(python_stdlib_type_domain("collections.abc", "List"), None);
    assert_eq!(python_stdlib_type_domain("collections.abc", "Tuple"), None);
    assert_eq!(python_stdlib_type_domain("asyncio", "Awaitable"), None);
    assert_eq!(python_stdlib_type_domain("asyncio", "Coroutine"), None);
    assert_eq!(python_stdlib_type_domain("custom.typing", "Iterable"), None);
}

#[test]
fn nominal_type_domain_evidence_is_dependency_backed_and_fail_closed() {
    let interner = Interner::new();
    let mut b = IlBuilder::new(FileId(0));
    let receiver = b.add(
        NodeKind::Var,
        Payload::Name(interner.intern("value")),
        sp(12),
        &[],
    );
    let root = b.add(NodeKind::Block, Payload::None, sp(11), &[receiver]);
    let mut il = finish_il(b, root, Lang::TypeScript);
    let widget = stable_symbol_hash("pkg.Widget");
    il.evidence.push(evidence(
        0,
        EvidenceAnchor::node(sp(12), NodeKind::Var),
        EvidenceKind::Type(TypeEvidenceKind::NominalDomain {
            type_hash: widget,
            domain: DomainEvidence::Record,
        }),
        EvidenceStatus::Asserted,
    ));

    assert_eq!(
        nominal_type_domain_at_node(&il, receiver, widget),
        Some(DomainEvidence::Record)
    );

    il.evidence.push(evidence(
        1,
        EvidenceAnchor::node(sp(12), NodeKind::Var),
        EvidenceKind::Domain(DomainEvidence::Record),
        EvidenceStatus::Ambiguous,
    ));
    let gadget = stable_symbol_hash("pkg.Gadget");
    il.evidence.push(evidence_with_dependencies(
        2,
        EvidenceAnchor::node(sp(12), NodeKind::Var),
        EvidenceKind::Type(TypeEvidenceKind::NominalDomain {
            type_hash: gadget,
            domain: DomainEvidence::Record,
        }),
        EvidenceStatus::Asserted,
        vec![EvidenceId(1)],
    ));
    assert_eq!(
        nominal_type_domain_at_node(&il, receiver, gadget),
        None,
        "dependency-broken nominal type-domain records must fail closed"
    );

    il.evidence.push(evidence(
        3,
        EvidenceAnchor::node(sp(12), NodeKind::Var),
        EvidenceKind::Type(TypeEvidenceKind::NominalDomain {
            type_hash: widget,
            domain: DomainEvidence::Map,
        }),
        EvidenceStatus::Asserted,
    ));
    assert_eq!(
        nominal_type_domain_at_node(&il, receiver, widget),
        None,
        "conflicting nominal type-domain records must fail closed"
    );
}

#[test]
fn method_receiver_contracts_expose_only_domain_backed_obligations() {
    assert_eq!(
        method_receiver_domain_requirement(MethodReceiverContract::ExactCollection),
        Some(DomainRequirement::ArrayCollectionOrSet)
    );
    assert_eq!(
        method_receiver_domain_requirement(MethodReceiverContract::ExactProtocol),
        Some(DomainRequirement::ArrayCollectionOrSet)
    );
    assert_eq!(
        method_receiver_domain_requirement(MethodReceiverContract::ExactCollectionOrMap),
        Some(DomainRequirement::CollectionOrMap)
    );
    assert_eq!(
        method_receiver_domain_requirement(MethodReceiverContract::ExactSetOrMap),
        Some(DomainRequirement::SetOrMap)
    );
    assert_eq!(
        method_receiver_domain_requirement(MethodReceiverContract::RustMapGetOrExactOption),
        Some(DomainRequirement::Option)
    );
    assert_eq!(
        method_receiver_domain_requirement(MethodReceiverContract::ExactMapLiteral),
        None
    );
    assert_eq!(
        method_receiver_domain_requirement(MethodReceiverContract::ImportedNamespace("math")),
        None
    );
}

#[test]
fn domain_evidence_records_drive_param_domain_proof() {
    let mut b = IlBuilder::new(FileId(0));
    let param = b.add(NodeKind::Param, Payload::None, sp(3), &[]);
    let root = b.add(NodeKind::Func, Payload::None, sp(3), &[param]);
    let mut il = finish_il(b, root, Lang::TypeScript);
    il.evidence.push(evidence(
        0,
        EvidenceAnchor::param(sp(3)),
        EvidenceKind::Domain(DomainEvidence::Map),
        EvidenceStatus::Asserted,
    ));

    assert_eq!(
        domain_evidence_for_param(&il, param),
        Some(DomainEvidence::Map)
    );
}

#[test]
fn ambiguous_domain_evidence_stays_closed() {
    let mut b = IlBuilder::new(FileId(0));
    let param = b.add(NodeKind::Param, Payload::None, sp(4), &[]);
    let root = b.add(NodeKind::Func, Payload::None, sp(4), &[param]);
    let mut il = finish_il(b, root, Lang::TypeScript);
    il.evidence.push(evidence(
        0,
        EvidenceAnchor::param(sp(4)),
        EvidenceKind::Domain(DomainEvidence::Set),
        EvidenceStatus::Asserted,
    ));
    il.evidence.push(evidence(
        1,
        EvidenceAnchor::param(sp(4)),
        EvidenceKind::Domain(DomainEvidence::Map),
        EvidenceStatus::Asserted,
    ));

    assert_eq!(domain_evidence_for_param(&il, param), None);
}

#[test]
fn receiver_domain_evidence_at_node_is_preferred_over_param_evidence() {
    let interner = Interner::new();
    let mut b = IlBuilder::new(FileId(0));
    let param = b.add(NodeKind::Param, Payload::Cid(0), span(10, 12, 1), &[]);
    let receiver = b.add(NodeKind::Var, Payload::Cid(0), span(20, 22, 2), &[]);
    let stmt = b.add(
        NodeKind::ExprStmt,
        Payload::None,
        span(20, 22, 2),
        &[receiver],
    );
    let body = b.add(NodeKind::Block, Payload::None, span(18, 24, 2), &[stmt]);
    let root = b.add(
        NodeKind::Func,
        Payload::None,
        span(0, 30, 1),
        &[param, body],
    );
    let mut il = finish_il(b, root, Lang::TypeScript);
    il.evidence.push(evidence(
        0,
        EvidenceAnchor::param(span(10, 12, 1)),
        EvidenceKind::Domain(DomainEvidence::Set),
        EvidenceStatus::Asserted,
    ));
    il.evidence.push(evidence(
        1,
        EvidenceAnchor::node(span(20, 22, 2), NodeKind::Var),
        EvidenceKind::Domain(DomainEvidence::Map),
        EvidenceStatus::Asserted,
    ));

    assert_eq!(
        domain_evidence_for_param(&il, param),
        Some(DomainEvidence::Set)
    );
    assert_eq!(
        domain_evidence_for_receiver(&il, &interner, receiver),
        Some(DomainEvidence::Map)
    );
    assert!(receiver_satisfies_domain(
        &il,
        &interner,
        receiver,
        DomainRequirement::Map
    ));
    assert!(!receiver_satisfies_domain(
        &il,
        &interner,
        receiver,
        DomainRequirement::Set
    ));
}

#[test]
fn ambiguous_receiver_domain_evidence_blocks_param_fallback() {
    let mut b = IlBuilder::new(FileId(0));
    let param = b.add(NodeKind::Param, Payload::Cid(0), span(10, 12, 1), &[]);
    let receiver = b.add(NodeKind::Var, Payload::Cid(0), span(20, 22, 2), &[]);
    let body = b.add(NodeKind::Block, Payload::None, span(18, 24, 2), &[receiver]);
    let root = b.add(
        NodeKind::Func,
        Payload::None,
        span(0, 30, 1),
        &[param, body],
    );
    let mut il = finish_il(b, root, Lang::TypeScript);
    il.evidence.push(evidence(
        0,
        EvidenceAnchor::param(span(10, 12, 1)),
        EvidenceKind::Domain(DomainEvidence::Map),
        EvidenceStatus::Asserted,
    ));
    il.evidence.push(evidence(
        1,
        EvidenceAnchor::node(span(20, 22, 2), NodeKind::Var),
        EvidenceKind::Domain(DomainEvidence::Set),
        EvidenceStatus::Asserted,
    ));
    il.evidence.push(evidence(
        2,
        EvidenceAnchor::node(span(20, 22, 2), NodeKind::Var),
        EvidenceKind::Domain(DomainEvidence::Map),
        EvidenceStatus::Asserted,
    ));

    let interner = Interner::new();
    assert_eq!(domain_evidence_for_receiver(&il, &interner, receiver), None);
}

fn binding_receiver_fixture(interner: &Interner, module_receiver: bool) -> (Il, NodeId, NodeId) {
    let xs = interner.intern("xs");
    let mut b = IlBuilder::new(FileId(0));
    let lhs = b.add(NodeKind::Var, Payload::Cid(0), span(10, 12, 1), &[]);
    let rhs = b.add(NodeKind::Seq, Payload::None, span(15, 17, 1), &[]);
    let assign = b.add(
        NodeKind::Assign,
        Payload::None,
        span(10, 17, 1),
        &[lhs, rhs],
    );
    let receiver_payload = if module_receiver {
        Payload::Name(xs)
    } else {
        Payload::Cid(0)
    };
    let receiver = b.add(NodeKind::Var, receiver_payload, span(40, 42, 3), &[]);
    let root = if module_receiver {
        let stmt = b.add(
            NodeKind::ExprStmt,
            Payload::None,
            span(40, 42, 3),
            &[receiver],
        );
        let body = b.add(NodeKind::Block, Payload::None, span(38, 45, 3), &[stmt]);
        let func = b.add(NodeKind::Func, Payload::None, span(30, 50, 2), &[body]);
        b.add(
            NodeKind::Module,
            Payload::None,
            span(0, 60, 1),
            &[assign, func],
        )
    } else {
        let body = b.add(
            NodeKind::Block,
            Payload::None,
            span(10, 44, 1),
            &[assign, receiver],
        );
        b.add(NodeKind::Func, Payload::None, span(0, 50, 1), &[body])
    };
    let mut il = finish_il(b, root, Lang::TypeScript);
    il.cid_names = vec![xs];
    (il, lhs, receiver)
}

#[test]
fn binding_domain_evidence_drives_receiver_domain_proof() {
    let interner = Interner::new();
    let (mut il, lhs, receiver) = binding_receiver_fixture(&interner, false);
    il.evidence.push(evidence(
        0,
        EvidenceAnchor::binding(span(10, 12, 1), stable_symbol_hash("xs")),
        EvidenceKind::Domain(DomainEvidence::Collection),
        EvidenceStatus::Asserted,
    ));

    assert_eq!(
        domain_evidence_for_binding_lhs(&il, &interner, lhs),
        Some(DomainEvidence::Collection)
    );
    assert_eq!(
        domain_evidence_for_receiver(&il, &interner, receiver),
        Some(DomainEvidence::Collection)
    );

    il.evidence.push(evidence(
        1,
        EvidenceAnchor::binding(span(10, 12, 1), stable_symbol_hash("xs")),
        EvidenceKind::Domain(DomainEvidence::Map),
        EvidenceStatus::Asserted,
    ));
    assert_eq!(
        domain_evidence_for_receiver(&il, &interner, receiver),
        None,
        "conflicting binding-domain evidence must close receiver proof"
    );
}

#[test]
fn binding_domain_evidence_validates_dependencies() {
    let interner = Interner::new();
    let (mut il, _, receiver) = binding_receiver_fixture(&interner, false);
    il.evidence.push(evidence(
        0,
        EvidenceAnchor::sequence(span(15, 17, 1)),
        EvidenceKind::SequenceSurface(SequenceSurfaceKind::Collection),
        EvidenceStatus::Ambiguous,
    ));
    il.evidence.push(evidence_with_dependencies(
        1,
        EvidenceAnchor::binding(span(10, 12, 1), stable_symbol_hash("xs")),
        EvidenceKind::Domain(DomainEvidence::Collection),
        EvidenceStatus::Asserted,
        vec![EvidenceId(0)],
    ));

    assert_eq!(
        domain_evidence_for_receiver(&il, &interner, receiver),
        None,
        "dependency-broken binding-domain evidence must fail closed"
    );
}

#[test]
fn module_binding_domain_evidence_reaches_free_name_receiver() {
    let interner = Interner::new();
    let (mut il, _, receiver) = binding_receiver_fixture(&interner, true);
    il.evidence.push(evidence(
        0,
        EvidenceAnchor::binding(span(10, 12, 1), stable_symbol_hash("xs")),
        EvidenceKind::Domain(DomainEvidence::Collection),
        EvidenceStatus::Asserted,
    ));

    assert_eq!(
        domain_evidence_for_receiver(&il, &interner, receiver),
        Some(DomainEvidence::Collection)
    );
}

#[test]
fn binding_domain_evidence_requires_matching_local_hash() {
    let interner = Interner::new();
    let xs = interner.intern("xs");
    let ys = interner.intern("ys");
    let mut b = IlBuilder::new(FileId(0));
    let xs_lhs = b.add(NodeKind::Var, Payload::Cid(0), span(10, 12, 1), &[]);
    let xs_rhs = b.add(NodeKind::Seq, Payload::None, span(14, 15, 1), &[]);
    let xs_assign = b.add(
        NodeKind::Assign,
        Payload::None,
        span(10, 15, 1),
        &[xs_lhs, xs_rhs],
    );
    let ys_lhs = b.add(NodeKind::Var, Payload::Cid(1), span(10, 12, 1), &[]);
    let ys_rhs = b.add(NodeKind::Seq, Payload::None, span(18, 19, 1), &[]);
    let ys_assign = b.add(
        NodeKind::Assign,
        Payload::None,
        span(16, 19, 1),
        &[ys_lhs, ys_rhs],
    );
    let ys_receiver = b.add(NodeKind::Var, Payload::Cid(1), span(30, 32, 2), &[]);
    let body = b.add(
        NodeKind::Block,
        Payload::None,
        span(8, 34, 1),
        &[xs_assign, ys_assign, ys_receiver],
    );
    let root = b.add(NodeKind::Func, Payload::None, span(0, 40, 1), &[body]);
    let mut il = finish_il(b, root, Lang::TypeScript);
    il.cid_names = vec![xs, ys];
    il.evidence.push(evidence(
        0,
        EvidenceAnchor::binding(span(10, 12, 1), stable_symbol_hash("xs")),
        EvidenceKind::Domain(DomainEvidence::Collection),
        EvidenceStatus::Asserted,
    ));

    assert_eq!(
        domain_evidence_for_binding_lhs(&il, &interner, xs_lhs),
        Some(DomainEvidence::Collection)
    );
    assert_eq!(
        domain_evidence_for_binding_lhs(&il, &interner, ys_lhs),
        None,
        "same-span binding evidence must not cross local_hash boundaries"
    );
    assert_eq!(
        domain_evidence_for_receiver(&il, &interner, ys_receiver),
        None
    );
}

#[test]
fn binding_domain_evidence_requires_assignment_before_receiver() {
    let interner = Interner::new();
    let xs = interner.intern("xs");
    let mut b = IlBuilder::new(FileId(0));
    let receiver = b.add(NodeKind::Var, Payload::Cid(0), span(10, 12, 1), &[]);
    let lhs = b.add(NodeKind::Var, Payload::Cid(0), span(20, 22, 2), &[]);
    let rhs = b.add(NodeKind::Seq, Payload::None, span(24, 25, 2), &[]);
    let assign = b.add(
        NodeKind::Assign,
        Payload::None,
        span(20, 25, 2),
        &[lhs, rhs],
    );
    let body = b.add(
        NodeKind::Block,
        Payload::None,
        span(8, 28, 1),
        &[receiver, assign],
    );
    let root = b.add(NodeKind::Func, Payload::None, span(0, 30, 1), &[body]);
    let mut il = finish_il(b, root, Lang::TypeScript);
    il.cid_names = vec![xs];
    il.evidence.push(evidence(
        0,
        EvidenceAnchor::binding(span(20, 22, 2), stable_symbol_hash("xs")),
        EvidenceKind::Domain(DomainEvidence::Collection),
        EvidenceStatus::Asserted,
    ));

    assert_eq!(
        domain_evidence_for_binding_lhs(&il, &interner, lhs),
        Some(DomainEvidence::Collection)
    );
    assert_eq!(
        domain_evidence_for_receiver(&il, &interner, receiver),
        None,
        "binding-domain evidence must not prove use-before-assignment receivers"
    );
}

#[test]
fn cid_receiver_domain_uses_nearest_function_scope() {
    let interner = Interner::new();
    let mut b = IlBuilder::new(FileId(0));
    let first_param = b.add(NodeKind::Param, Payload::Cid(0), span(10, 12, 1), &[]);
    let first_body = b.add(NodeKind::Block, Payload::None, span(14, 20, 1), &[]);
    let first_func = b.add(
        NodeKind::Func,
        Payload::None,
        span(0, 30, 1),
        &[first_param, first_body],
    );
    let second_param = b.add(NodeKind::Param, Payload::Cid(0), span(50, 52, 3), &[]);
    let receiver = b.add(NodeKind::Var, Payload::Cid(0), span(60, 62, 4), &[]);
    let stmt = b.add(
        NodeKind::ExprStmt,
        Payload::None,
        span(60, 62, 4),
        &[receiver],
    );
    let second_body = b.add(NodeKind::Block, Payload::None, span(58, 66, 4), &[stmt]);
    let second_func = b.add(
        NodeKind::Func,
        Payload::None,
        span(40, 80, 3),
        &[second_param, second_body],
    );
    let root = b.add(
        NodeKind::Module,
        Payload::None,
        span(0, 90, 1),
        &[first_func, second_func],
    );
    let mut il = finish_il(b, root, Lang::TypeScript);
    il.evidence.push(evidence(
        0,
        EvidenceAnchor::param(span(10, 12, 1)),
        EvidenceKind::Domain(DomainEvidence::Collection),
        EvidenceStatus::Asserted,
    ));
    il.evidence.push(evidence(
        1,
        EvidenceAnchor::param(span(50, 52, 3)),
        EvidenceKind::Domain(DomainEvidence::Map),
        EvidenceStatus::Asserted,
    ));

    assert_eq!(
        domain_evidence_for_receiver(&il, &interner, receiver),
        Some(DomainEvidence::Map)
    );
}

#[test]
fn dependency_broken_receiver_domain_evidence_blocks_param_fallback() {
    let interner = Interner::new();
    let mut b = IlBuilder::new(FileId(0));
    let param = b.add(NodeKind::Param, Payload::Cid(0), span(10, 12, 1), &[]);
    let receiver = b.add(NodeKind::Var, Payload::Cid(0), span(20, 22, 2), &[]);
    let body = b.add(NodeKind::Block, Payload::None, span(18, 24, 2), &[receiver]);
    let root = b.add(
        NodeKind::Func,
        Payload::None,
        span(0, 30, 1),
        &[param, body],
    );
    let mut il = finish_il(b, root, Lang::TypeScript);
    il.evidence.push(evidence(
        0,
        EvidenceAnchor::param(span(10, 12, 1)),
        EvidenceKind::Domain(DomainEvidence::Set),
        EvidenceStatus::Asserted,
    ));
    il.evidence.push(evidence_with_dependencies(
        1,
        EvidenceAnchor::node(span(20, 22, 2), NodeKind::Var),
        EvidenceKind::Domain(DomainEvidence::Map),
        EvidenceStatus::Asserted,
        vec![EvidenceId(99)],
    ));

    assert_eq!(domain_evidence_for_receiver(&il, &interner, receiver), None);
}

#[test]
fn receiver_domain_index_uses_kernel_fail_closed_policy() {
    let interner = Interner::new();
    let mut b = IlBuilder::new(FileId(0));
    let param = b.add(NodeKind::Param, Payload::Cid(0), span(10, 12, 1), &[]);
    let receiver = b.add(NodeKind::Var, Payload::Cid(0), span(20, 22, 2), &[]);
    let body = b.add(NodeKind::Block, Payload::None, span(18, 24, 2), &[receiver]);
    let root = b.add(
        NodeKind::Func,
        Payload::None,
        span(0, 30, 1),
        &[param, body],
    );
    let mut il = finish_il(b, root, Lang::TypeScript);
    il.evidence.push(evidence(
        0,
        EvidenceAnchor::param(span(10, 12, 1)),
        EvidenceKind::Domain(DomainEvidence::Collection),
        EvidenceStatus::Asserted,
    ));
    il.evidence.push(evidence(
        1,
        EvidenceAnchor::node(span(20, 22, 2), NodeKind::Var),
        EvidenceKind::Domain(DomainEvidence::Map),
        EvidenceStatus::Ambiguous,
    ));

    let domains = ReceiverDomainEvidenceIndex::new(&il, &interner);
    assert_eq!(domains.domain_evidence_for_receiver(receiver), None);
    assert!(!domains.receiver_satisfies_domain(receiver, DomainRequirement::Collection));
}

#[test]
fn named_receiver_domain_requires_unassigned_param_scope() {
    let interner = Interner::new();
    let xs = interner.intern("xs");
    let mut b = IlBuilder::new(FileId(0));
    let param = b.add(NodeKind::Param, Payload::Name(xs), span(10, 12, 1), &[]);
    let receiver = b.add(NodeKind::Var, Payload::Name(xs), span(40, 42, 3), &[]);
    let stmt = b.add(
        NodeKind::ExprStmt,
        Payload::None,
        span(40, 42, 3),
        &[receiver],
    );
    let body = b.add(NodeKind::Block, Payload::None, span(20, 50, 2), &[stmt]);
    let root = b.add(
        NodeKind::Func,
        Payload::None,
        span(0, 60, 1),
        &[param, body],
    );
    let mut il = finish_il(b, root, Lang::TypeScript);
    il.evidence.push(evidence(
        0,
        EvidenceAnchor::param(span(10, 12, 1)),
        EvidenceKind::Domain(DomainEvidence::Collection),
        EvidenceStatus::Asserted,
    ));

    assert_eq!(
        domain_evidence_for_receiver(&il, &interner, receiver),
        Some(DomainEvidence::Collection)
    );

    let mut b = IlBuilder::new(FileId(0));
    let param = b.add(NodeKind::Param, Payload::Name(xs), span(10, 12, 1), &[]);
    let lhs = b.add(NodeKind::Var, Payload::Name(xs), span(24, 26, 2), &[]);
    let rhs = b.add(NodeKind::Lit, Payload::LitInt(1), span(29, 30, 2), &[]);
    let assign = b.add(
        NodeKind::Assign,
        Payload::None,
        span(24, 30, 2),
        &[lhs, rhs],
    );
    let receiver = b.add(NodeKind::Var, Payload::Name(xs), span(40, 42, 3), &[]);
    let stmt = b.add(
        NodeKind::ExprStmt,
        Payload::None,
        span(40, 42, 3),
        &[receiver],
    );
    let body = b.add(
        NodeKind::Block,
        Payload::None,
        span(20, 50, 2),
        &[assign, stmt],
    );
    let root = b.add(
        NodeKind::Func,
        Payload::None,
        span(0, 60, 1),
        &[param, body],
    );
    let mut il = finish_il(b, root, Lang::TypeScript);
    il.evidence.push(evidence(
        0,
        EvidenceAnchor::param(span(10, 12, 1)),
        EvidenceKind::Domain(DomainEvidence::Collection),
        EvidenceStatus::Asserted,
    ));

    assert_eq!(domain_evidence_for_receiver(&il, &interner, receiver), None);
}

#[test]
fn sequence_surface_contracts_keep_value_and_exact_axes_separate() {
    let array = seq_surface_contract(Lang::JavaScript, Some("array")).unwrap();
    assert_eq!(array.value_tag, SEQ_VALUE_COLLECTION);
    assert!(array.exact_tree_safe);
    assert!(array.membership_collection);

    let untagged = seq_surface_contract(Lang::JavaScript, None).unwrap();
    assert_eq!(untagged.value_tag, SEQ_VALUE_UNTAGGED);
    assert!(!untagged.exact_tree_safe);
    assert!(!untagged.membership_collection);

    let object = seq_surface_contract(Lang::JavaScript, Some("object")).unwrap();
    assert_eq!(object.value_tag, SEQ_VALUE_MAP);
    assert!(object.exact_tree_safe);
    assert!(!object.membership_collection);
    assert!(object.imported_literal);

    let go_map = seq_surface_contract(Lang::Go, Some("composite_literal")).unwrap();
    assert_eq!(
        go_map.value_tag,
        stable_symbol_hash("go_composite_map_literal")
    );
    assert!(!go_map.exact_tree_safe);
    assert!(!go_map.membership_collection);
    assert!(!go_map.imported_literal);

    let go_entry = seq_surface_contract(Lang::Go, Some("keyed_element")).unwrap();
    assert_eq!(go_entry.value_tag, stable_symbol_hash("keyed_element"));
    assert!(!go_entry.exact_tree_safe);
    assert!(!go_entry.membership_collection);

    assert!(seq_surface_contract(Lang::Python, Some("composite_literal")).is_none());
    assert!(seq_surface_contract(Lang::Python, Some("keyed_element")).is_none());
    assert!(imported_literal_seq_tag_safe(Lang::Python, "dictionary"));
    assert!(!imported_literal_seq_tag_safe(Lang::Ruby, "hash"));
}

#[test]
fn sequence_surface_evidence_must_match_the_lowered_surface() {
    let interner = Interner::new();
    let mut b = IlBuilder::new(FileId(0));
    let array = interner.intern("array");
    let seq = b.add(NodeKind::Seq, Payload::Name(array), sp(5), &[]);
    let root = b.add(NodeKind::Block, Payload::None, sp(5), &[seq]);
    let mut il = finish_il(b, root, Lang::JavaScript);

    assert_eq!(
        seq_surface_contract_for_node(&il, &interner, seq),
        None,
        "raw sequence tags do not prove semantic surfaces without evidence"
    );

    il.evidence.push(evidence(
        0,
        EvidenceAnchor::sequence(sp(5)),
        EvidenceKind::SequenceSurface(SequenceSurfaceKind::Collection),
        EvidenceStatus::Asserted,
    ));
    assert!(seq_surface_contract_for_node(&il, &interner, seq)
        .is_some_and(|contract| contract.membership_collection));

    il.evidence.push(evidence(
        1,
        EvidenceAnchor::sequence(sp(5)),
        EvidenceKind::SequenceSurface(SequenceSurfaceKind::Map),
        EvidenceStatus::Asserted,
    ));
    assert_eq!(seq_surface_contract_for_node(&il, &interner, seq), None);
}

#[test]
fn imported_literal_export_safety_requires_sequence_evidence() {
    let interner = Interner::new();
    let mut b = IlBuilder::new(FileId(0));
    let object = interner.intern("object");
    let key = b.add(
        NodeKind::Lit,
        Payload::LitStr(stable_symbol_hash("ready")),
        sp(6),
        &[],
    );
    let value = b.add(NodeKind::Lit, Payload::LitInt(1), sp(6), &[]);
    let entry = b.add(NodeKind::Seq, Payload::Name(object), sp(6), &[key, value]);
    let root = b.add(NodeKind::Block, Payload::None, sp(6), &[entry]);
    let mut il = finish_il(b, root, Lang::JavaScript);

    assert!(!imported_literal_export_safe(&il, &interner, entry));

    il.evidence.push(evidence(
        0,
        EvidenceAnchor::sequence(sp(6)),
        EvidenceKind::SequenceSurface(SequenceSurfaceKind::Map),
        EvidenceStatus::Asserted,
    ));
    assert!(imported_literal_export_safe(&il, &interner, entry));
}

#[test]
fn imported_literal_export_safety_rejects_import_coordinate_children() {
    let interner = Interner::new();
    let mut b = IlBuilder::new(FileId(0));
    let object = interner.intern("object");
    let imported = b.add(NodeKind::Seq, Payload::None, sp(7), &[]);
    let root_value = b.add(NodeKind::Seq, Payload::Name(object), sp(8), &[imported]);
    let root = b.add(NodeKind::Block, Payload::None, sp(8), &[root_value]);
    let mut il = finish_il(b, root, Lang::JavaScript);
    il.evidence.push(evidence(
        0,
        EvidenceAnchor::sequence(sp(8)),
        EvidenceKind::SequenceSurface(SequenceSurfaceKind::Map),
        EvidenceStatus::Asserted,
    ));
    il.evidence.push(evidence(
        1,
        EvidenceAnchor::sequence(sp(7)),
        EvidenceKind::Import(ImportEvidenceKind::Binding {
            module_hash: stable_symbol_hash("provider"),
            exported_hash: stable_symbol_hash("VALUE"),
        }),
        EvidenceStatus::Asserted,
    ));

    assert!(!imported_literal_export_safe(&il, &interner, root_value));
}

#[test]
fn go_zero_map_surface_helpers_require_evidence() {
    let interner = Interner::new();
    let mut b = IlBuilder::new(FileId(0));
    let key = b.add(
        NodeKind::Lit,
        Payload::LitStr(stable_symbol_hash("ready")),
        sp(32),
        &[],
    );
    let value = b.add(NodeKind::Lit, Payload::LitInt(1), sp(32), &[]);
    let entry = b.add(
        NodeKind::Seq,
        Payload::Name(interner.intern("keyed_element")),
        sp(32),
        &[key, value],
    );
    let map = b.add(
        NodeKind::Seq,
        Payload::Name(interner.intern("composite_literal")),
        sp(31),
        &[entry],
    );
    let root = b.add(NodeKind::Block, Payload::None, sp(31), &[map]);
    let mut il = finish_il(b, root, Lang::Go);

    assert!(go_zero_map_literal_contract_for_node(&il, &interner, map).is_none());
    assert!(go_zero_map_entry_contract_for_node(&il, &interner, entry).is_none());

    il.evidence.push(evidence(
        0,
        EvidenceAnchor::sequence(sp(31)),
        EvidenceKind::SequenceSurface(SequenceSurfaceKind::GoCompositeMapLiteral),
        EvidenceStatus::Asserted,
    ));
    assert!(go_zero_map_literal_contract_for_node(&il, &interner, map).is_some());
    assert!(go_zero_map_entry_contract_for_node(&il, &interner, entry).is_none());

    il.evidence.push(evidence(
        1,
        EvidenceAnchor::sequence(sp(32)),
        EvidenceKind::SequenceSurface(SequenceSurfaceKind::GoMapEntry),
        EvidenceStatus::Asserted,
    ));
    assert!(go_zero_map_entry_contract_for_node(&il, &interner, entry).is_some());
}

#[test]
fn import_fact_contracts_resolve_evidence_only_binding_and_namespace_proofs() {
    let mut b = IlBuilder::new(FileId(0));
    let collections = b.add(
        NodeKind::Lit,
        Payload::LitStr(stable_symbol_hash("collections")),
        sp(1),
        &[],
    );
    let deque = b.add(
        NodeKind::Lit,
        Payload::LitStr(stable_symbol_hash("deque")),
        sp(1),
        &[],
    );
    let binding = b.add(NodeKind::Seq, Payload::None, sp(1), &[collections, deque]);
    let math = b.add(
        NodeKind::Lit,
        Payload::LitStr(stable_symbol_hash("math")),
        sp(2),
        &[],
    );
    let namespace = b.add(NodeKind::Seq, Payload::None, sp(2), &[math]);
    let raw_coordinates = b.add(NodeKind::Seq, Payload::None, sp(3), &[math]);
    let root = b.add(
        NodeKind::Module,
        Payload::None,
        sp(1),
        &[binding, namespace, raw_coordinates],
    );
    let mut il = finish_il(b, root, Lang::Python);

    assert_eq!(
        import_fact_contract(ImportFactKind::Binding).channel,
        ChannelEligibility::ExactProven
    );
    assert_eq!(import_fact_evidence_rhs(&il, binding), None);
    assert_eq!(import_fact_evidence_rhs(&il, namespace), None);

    il.evidence.push(evidence(
        0,
        EvidenceAnchor::sequence(sp(1)),
        EvidenceKind::Import(ImportEvidenceKind::Binding {
            module_hash: stable_symbol_hash("collections"),
            exported_hash: stable_symbol_hash("deque"),
        }),
        EvidenceStatus::Asserted,
    ));
    il.evidence.push(evidence(
        1,
        EvidenceAnchor::sequence(sp(2)),
        EvidenceKind::Import(ImportEvidenceKind::Namespace {
            module_hash: stable_symbol_hash("math"),
        }),
        EvidenceStatus::Asserted,
    ));

    assert_eq!(
        import_fact_evidence_rhs(&il, binding),
        Some(ImportFact {
            kind: ImportFactKind::Binding,
            module_hash: stable_symbol_hash("collections"),
            exported_hash: Some(stable_symbol_hash("deque")),
        })
    );
    assert_eq!(
        import_fact_evidence_rhs(&il, namespace),
        Some(ImportFact {
            kind: ImportFactKind::Namespace,
            module_hash: stable_symbol_hash("math"),
            exported_hash: None,
        })
    );
    assert_eq!(import_fact_evidence_rhs(&il, raw_coordinates), None);
}

#[test]
fn ambiguous_import_evidence_stays_closed_without_raw_seq_fallback() {
    let mut b = IlBuilder::new(FileId(0));
    let module = b.add(
        NodeKind::Lit,
        Payload::LitStr(stable_symbol_hash("collections")),
        sp(10),
        &[],
    );
    let exported = b.add(
        NodeKind::Lit,
        Payload::LitStr(stable_symbol_hash("deque")),
        sp(10),
        &[],
    );
    let binding = b.add(NodeKind::Seq, Payload::None, sp(10), &[module, exported]);
    let root = b.add(NodeKind::Module, Payload::None, sp(10), &[binding]);
    let mut il = finish_il(b, root, Lang::Python);
    il.evidence.push(evidence(
        0,
        EvidenceAnchor::sequence(sp(10)),
        EvidenceKind::Import(ImportEvidenceKind::Namespace {
            module_hash: stable_symbol_hash("math"),
        }),
        EvidenceStatus::Asserted,
    ));

    assert_eq!(
        import_fact_evidence_rhs(&il, binding),
        Some(ImportFact {
            kind: ImportFactKind::Namespace,
            module_hash: stable_symbol_hash("math"),
            exported_hash: None,
        })
    );

    il.evidence.push(evidence(
        1,
        EvidenceAnchor::sequence(sp(10)),
        EvidenceKind::Import(ImportEvidenceKind::Binding {
            module_hash: stable_symbol_hash("collections"),
            exported_hash: stable_symbol_hash("deque"),
        }),
        EvidenceStatus::Asserted,
    ));
    assert_eq!(import_fact_evidence_rhs(&il, binding), None);
}

#[test]
fn imported_symbol_identity_does_not_fall_back_to_raw_import_seq() {
    let interner = Interner::new();
    let mut b = IlBuilder::new(FileId(0));
    let local = interner.intern("deque");
    let module = b.add(
        NodeKind::Lit,
        Payload::LitStr(stable_symbol_hash("collections")),
        sp(30),
        &[],
    );
    let exported = b.add(
        NodeKind::Lit,
        Payload::LitStr(stable_symbol_hash("deque")),
        sp(30),
        &[],
    );
    let lhs = b.add(NodeKind::Var, Payload::Name(local), sp(30), &[]);
    let rhs = b.add(NodeKind::Seq, Payload::None, sp(30), &[module, exported]);
    let assignment = b.add(NodeKind::Assign, Payload::None, sp(30), &[lhs, rhs]);
    let use_site = b.add(NodeKind::Var, Payload::Name(local), sp(31), &[]);
    let root = b.add(
        NodeKind::Module,
        Payload::None,
        sp(30),
        &[assignment, use_site],
    );
    let mut il = finish_il(b, root, Lang::Python);

    assert_eq!(import_fact_evidence_rhs(&il, rhs), None);
    assert!(!imported_binding_symbol(
        &il,
        &interner,
        use_site,
        "collections",
        "deque"
    ));

    il.evidence.push(evidence(
        0,
        EvidenceAnchor::binding(sp(30), stable_symbol_hash("deque")),
        EvidenceKind::Symbol(SymbolEvidenceKind::ImportedBinding {
            module_hash: stable_symbol_hash("collections"),
            exported_hash: stable_symbol_hash("deque"),
        }),
        EvidenceStatus::Asserted,
    ));
    assert!(imported_binding_symbol(
        &il,
        &interner,
        use_site,
        "collections",
        "deque"
    ));
}

#[test]
fn imported_occurrence_symbol_evidence_requires_binding_dependency() {
    let interner = Interner::new();
    let mut b = IlBuilder::new(FileId(0));
    let local_hash = stable_symbol_hash("m");
    let receiver = b.add(
        NodeKind::Var,
        Payload::Name(interner.intern("m")),
        sp(20),
        &[],
    );
    let root = b.add(NodeKind::Module, Payload::None, sp(20), &[receiver]);
    let mut il = finish_il(b, root, Lang::Python);
    il.evidence.push(evidence(
        0,
        EvidenceAnchor::node(sp(20), NodeKind::Var),
        EvidenceKind::Symbol(SymbolEvidenceKind::ImportedNamespace {
            module_hash: stable_symbol_hash("math"),
        }),
        EvidenceStatus::Asserted,
    ));

    assert!(!imported_namespace_symbol(&il, &interner, receiver, "math"));

    il.evidence.clear();
    il.evidence.push(evidence(
        0,
        EvidenceAnchor::binding(sp(19), local_hash),
        EvidenceKind::Symbol(SymbolEvidenceKind::ImportedNamespace {
            module_hash: stable_symbol_hash("math"),
        }),
        EvidenceStatus::Asserted,
    ));
    il.evidence.push(evidence_with_dependencies(
        1,
        EvidenceAnchor::node(sp(20), NodeKind::Var),
        EvidenceKind::Symbol(SymbolEvidenceKind::ImportedNamespace {
            module_hash: stable_symbol_hash("math"),
        }),
        EvidenceStatus::Asserted,
        vec![EvidenceId(0)],
    ));

    assert!(imported_namespace_symbol(&il, &interner, receiver, "math"));
    assert!(!imported_namespace_symbol(
        &il,
        &interner,
        receiver,
        "collections"
    ));
}

#[test]
fn symbol_evidence_blocks_import_assignment_fallback() {
    let interner = Interner::new();
    let mut b = IlBuilder::new(FileId(0));
    let local = interner.intern("math");
    let lhs = b.add(NodeKind::Var, Payload::Name(local), sp(21), &[]);
    let module = b.add(
        NodeKind::Lit,
        Payload::LitStr(stable_symbol_hash("math")),
        sp(21),
        &[],
    );
    let rhs = b.add(NodeKind::Seq, Payload::None, sp(21), &[module]);
    let assign = b.add(NodeKind::Assign, Payload::None, sp(21), &[lhs, rhs]);
    let receiver = b.add(NodeKind::Var, Payload::Name(local), sp(22), &[]);
    let root = b.add(NodeKind::Module, Payload::None, sp(21), &[assign, receiver]);
    let mut il = finish_il(b, root, Lang::Python);
    il.evidence.push(evidence(
        0,
        EvidenceAnchor::binding(sp(21), stable_symbol_hash("math")),
        EvidenceKind::Symbol(SymbolEvidenceKind::ImportedNamespace {
            module_hash: stable_symbol_hash("other"),
        }),
        EvidenceStatus::Asserted,
    ));

    assert!(!imported_namespace_symbol(&il, &interner, receiver, "math"));
}

#[test]
fn binding_symbol_evidence_does_not_prove_rebound_alias_uses() {
    let interner = Interner::new();
    let mut b = IlBuilder::new(FileId(0));
    let local = interner.intern("math");
    let lhs = b.add(NodeKind::Var, Payload::Name(local), sp(24), &[]);
    let module = b.add(
        NodeKind::Lit,
        Payload::LitStr(stable_symbol_hash("math")),
        sp(24),
        &[],
    );
    let rhs = b.add(NodeKind::Seq, Payload::None, sp(24), &[module]);
    let import_assign = b.add(NodeKind::Assign, Payload::None, sp(24), &[lhs, rhs]);
    let rebound_lhs = b.add(NodeKind::Var, Payload::Name(local), sp(25), &[]);
    let rebound_rhs = b.add(NodeKind::Lit, Payload::LitInt(0), sp(25), &[]);
    let rebound = b.add(
        NodeKind::Assign,
        Payload::None,
        sp(25),
        &[rebound_lhs, rebound_rhs],
    );
    let receiver = b.add(NodeKind::Var, Payload::Name(local), sp(26), &[]);
    let root = b.add(
        NodeKind::Module,
        Payload::None,
        sp(24),
        &[import_assign, rebound, receiver],
    );
    let mut il = finish_il(b, root, Lang::Python);
    il.evidence.push(evidence(
        0,
        EvidenceAnchor::binding(sp(24), stable_symbol_hash("math")),
        EvidenceKind::Symbol(SymbolEvidenceKind::ImportedNamespace {
            module_hash: stable_symbol_hash("math"),
        }),
        EvidenceStatus::Asserted,
    ));

    assert!(!imported_namespace_symbol(&il, &interner, receiver, "math"));
}

#[test]
fn ambiguous_global_symbol_evidence_blocks_name_fallback() {
    let interner = Interner::new();
    let mut b = IlBuilder::new(FileId(0));
    let math = b.add(
        NodeKind::Var,
        Payload::Name(interner.intern("Math")),
        sp(23),
        &[],
    );
    let root = b.add(NodeKind::Module, Payload::None, sp(23), &[math]);
    let mut il = finish_il(b, root, Lang::JavaScript);

    assert!(unshadowed_global_symbol(&il, &interner, math, "Math"));

    il.evidence.push(evidence(
        0,
        EvidenceAnchor::node(sp(23), NodeKind::Var),
        EvidenceKind::Symbol(SymbolEvidenceKind::UnshadowedGlobal {
            name_hash: stable_symbol_hash("Math"),
        }),
        EvidenceStatus::Ambiguous,
    ));
    assert!(!unshadowed_global_symbol(&il, &interner, math, "Math"));
}
