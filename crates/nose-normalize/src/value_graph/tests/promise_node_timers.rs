use super::support::*;

#[test]
fn node_timers_settimeout_without_options_recovers_fulfilled_payload_boundary() {
    let interner = Interner::new();
    let mut b = IlBuilder::new(FileId(0));
    let delay = b.add(
        NodeKind::Var,
        Payload::Name(interner.intern("delay")),
        sp(300),
        &[],
    );
    let timeout = b.add(NodeKind::Lit, Payload::LitInt(0), sp(301), &[]);
    let payload = b.add(NodeKind::Lit, Payload::LitInt(1), sp(302), &[]);
    let producer_call = b.add(
        NodeKind::Call,
        Payload::None,
        sp(303),
        &[delay, timeout, payload],
    );
    let then_callee = b.add(
        NodeKind::Field,
        Payload::Name(interner.intern("then")),
        sp(304),
        &[producer_call],
    );
    let callback = add_increment_lambda(&mut b, 305, 1);
    let continuation_call = b.add(
        NodeKind::Call,
        Payload::None,
        sp(310),
        &[then_callee, callback],
    );
    let sync_add = add_sync_add(&mut b, 311);
    let root = b.add(
        NodeKind::Block,
        Payload::None,
        sp(314),
        &[continuation_call, sync_add],
    );
    let mut il = finish_test_il(b, root, Lang::TypeScript);
    il.push_evidence(language_core_symbol_evidence(
        100,
        Lang::TypeScript,
        EvidenceAnchor::binding(sp(299), stable_symbol_hash("delay")),
        SymbolEvidenceKind::ImportedBinding {
            module_hash: stable_symbol_hash("node:timers/promises"),
            exported_hash: stable_symbol_hash("setTimeout"),
        },
    ));

    crate::library_api_evidence::run(&mut il, &interner);
    crate::call_target_evidence::run(&mut il, &interner);

    assert_eq!(
        nose_semantics::promise_settled_value_evidence_at_call(&il, &interner, producer_call)
            .map(|settled| settled.payload),
        Some(payload),
        "setTimeout(delay, value) should expose the fulfilled value payload"
    );
    let mut builder = Builder::new(&il, &interner);
    let promise_value = builder.eval(continuation_call, &FxHashMap::default());
    let recovered_payload = assert_resolved_promise_boundary(&builder, promise_value);
    let sync_value = builder.eval(sync_add, &FxHashMap::default());
    assert_eq!(recovered_payload, sync_value);
    assert_ne!(
        promise_value, sync_value,
        "node timers recovery must preserve the Promise boundary"
    );
}

#[test]
fn node_timers_setimmediate_without_options_recovers_fulfilled_payload_boundary() {
    let interner = Interner::new();
    let mut b = IlBuilder::new(FileId(0));
    let immediate = b.add(
        NodeKind::Var,
        Payload::Name(interner.intern("immediate")),
        sp(320),
        &[],
    );
    let payload = b.add(NodeKind::Lit, Payload::LitInt(1), sp(321), &[]);
    let producer_call = b.add(
        NodeKind::Call,
        Payload::None,
        sp(322),
        &[immediate, payload],
    );
    let then_callee = b.add(
        NodeKind::Field,
        Payload::Name(interner.intern("then")),
        sp(323),
        &[producer_call],
    );
    let callback = add_increment_lambda(&mut b, 324, 1);
    let continuation_call = b.add(
        NodeKind::Call,
        Payload::None,
        sp(329),
        &[then_callee, callback],
    );
    let sync_add = add_sync_add(&mut b, 330);
    let root = b.add(
        NodeKind::Block,
        Payload::None,
        sp(333),
        &[continuation_call, sync_add],
    );
    let mut il = finish_test_il(b, root, Lang::TypeScript);
    il.push_evidence(language_core_symbol_evidence(
        100,
        Lang::TypeScript,
        EvidenceAnchor::binding(sp(319), stable_symbol_hash("immediate")),
        SymbolEvidenceKind::ImportedBinding {
            module_hash: stable_symbol_hash("node:timers/promises"),
            exported_hash: stable_symbol_hash("setImmediate"),
        },
    ));

    crate::library_api_evidence::run(&mut il, &interner);
    crate::call_target_evidence::run(&mut il, &interner);

    assert_eq!(
        nose_semantics::promise_settled_value_evidence_at_call(&il, &interner, producer_call)
            .map(|settled| settled.payload),
        Some(payload),
        "setImmediate(value) should expose the fulfilled value payload"
    );
    let mut builder = Builder::new(&il, &interner);
    let promise_value = builder.eval(continuation_call, &FxHashMap::default());
    let recovered_payload = assert_resolved_promise_boundary(&builder, promise_value);
    let sync_value = builder.eval(sync_add, &FxHashMap::default());
    assert_eq!(recovered_payload, sync_value);
}

#[test]
fn node_timers_safe_payload_contract_respects_thenable_guard() {
    let interner = Interner::new();
    let mut b = IlBuilder::new(FileId(0));
    let delay = b.add(
        NodeKind::Var,
        Payload::Name(interner.intern("delay")),
        sp(340),
        &[],
    );
    let timeout = b.add(NodeKind::Lit, Payload::LitInt(0), sp(341), &[]);
    let payload = b.add(
        NodeKind::Var,
        Payload::Name(interner.intern("maybeThenable")),
        sp(342),
        &[],
    );
    let producer_call = b.add(
        NodeKind::Call,
        Payload::None,
        sp(343),
        &[delay, timeout, payload],
    );
    let then_callee = b.add(
        NodeKind::Field,
        Payload::Name(interner.intern("then")),
        sp(344),
        &[producer_call],
    );
    let callback = add_increment_lambda(&mut b, 345, 1);
    let continuation_call = b.add(
        NodeKind::Call,
        Payload::None,
        sp(350),
        &[then_callee, callback],
    );
    let mut il = finish_test_il(b, continuation_call, Lang::TypeScript);
    il.push_evidence(language_core_symbol_evidence(
        100,
        Lang::TypeScript,
        EvidenceAnchor::binding(sp(339), stable_symbol_hash("delay")),
        SymbolEvidenceKind::ImportedBinding {
            module_hash: stable_symbol_hash("node:timers/promises"),
            exported_hash: stable_symbol_hash("setTimeout"),
        },
    ));

    crate::library_api_evidence::run(&mut il, &interner);
    crate::call_target_evidence::run(&mut il, &interner);

    assert_eq!(
        nose_semantics::promise_settled_value_evidence_at_call(&il, &interner, producer_call)
            .map(|settled| settled.payload),
        Some(payload),
        "setTimeout(delay, value) may name its fulfilled payload"
    );
    assert!(
        !matches!(
            eval_op(&il, &interner, continuation_call),
            ValOp::Call(code) if code == PROMISE_RESOLVED_CODE || code == PROMISE_REJECTED_CODE
        ),
        "possible thenable payloads must not be merged through the continuation"
    );
}

#[test]
fn node_timers_with_options_stays_domain_only() {
    let interner = Interner::new();
    let mut b = IlBuilder::new(FileId(0));
    let delay = b.add(
        NodeKind::Var,
        Payload::Name(interner.intern("delay")),
        sp(300),
        &[],
    );
    let timeout = b.add(NodeKind::Lit, Payload::LitInt(0), sp(301), &[]);
    let payload = b.add(NodeKind::Lit, Payload::LitInt(1), sp(302), &[]);
    let options = b.add(
        NodeKind::Var,
        Payload::Name(interner.intern("options")),
        sp(303),
        &[],
    );
    let producer_call = b.add(
        NodeKind::Call,
        Payload::None,
        sp(304),
        &[delay, timeout, payload, options],
    );
    let then_callee = b.add(
        NodeKind::Field,
        Payload::Name(interner.intern("then")),
        sp(305),
        &[producer_call],
    );
    let callback = add_increment_lambda(&mut b, 306, 1);
    let continuation_call = b.add(
        NodeKind::Call,
        Payload::None,
        sp(311),
        &[then_callee, callback],
    );
    let mut il = finish_test_il(b, continuation_call, Lang::TypeScript);
    il.push_evidence(language_core_symbol_evidence(
        100,
        Lang::TypeScript,
        EvidenceAnchor::binding(sp(299), stable_symbol_hash("delay")),
        SymbolEvidenceKind::ImportedBinding {
            module_hash: stable_symbol_hash("node:timers/promises"),
            exported_hash: stable_symbol_hash("setTimeout"),
        },
    ));

    crate::library_api_evidence::run(&mut il, &interner);
    crate::call_target_evidence::run(&mut il, &interner);

    assert_eq!(
        nose_semantics::domain_evidence_for_receiver(&il, &interner, producer_call),
        Some(DomainEvidence::PromiseLike),
        "node:timers/promises setTimeout should prove only the PromiseLike receiver domain"
    );
    assert!(
        nose_semantics::admitted_promise_then_at_call(&il, &interner, continuation_call).is_some(),
        "Promise.then should admit after the imported producer domain is materialized"
    );
    assert_eq!(
        nose_semantics::promise_settled_value_evidence_at_call(&il, &interner, producer_call),
        None,
        "setTimeout(delay, value, options) can reject through options.signal and stays domain-only"
    );
    assert!(
        !matches!(
            eval_op(&il, &interner, continuation_call),
            ValOp::Call(code) if code == PROMISE_RESOLVED_CODE || code == PROMISE_REJECTED_CODE
        ),
        "timers/promises producers need scheduling/settlement proof before payload recovery"
    );
}

#[test]
fn node_timers_commonjs_imported_binding_dependency_opens_promise_like_domain() {
    let interner = Interner::new();
    let mut b = IlBuilder::new(FileId(0));
    let delay = b.add(
        NodeKind::Var,
        Payload::Name(interner.intern("delay")),
        sp(300),
        &[],
    );
    let timeout = b.add(NodeKind::Lit, Payload::LitInt(0), sp(301), &[]);
    let producer_call = b.add(NodeKind::Call, Payload::None, sp(303), &[delay, timeout]);
    let mut il = finish_test_il(b, producer_call, Lang::TypeScript);
    il.push_evidence(language_core_symbol_evidence(
        100,
        Lang::TypeScript,
        EvidenceAnchor::node(sp(298), NodeKind::Var),
        SymbolEvidenceKind::UnshadowedGlobal {
            name_hash: stable_symbol_hash("require"),
        },
    ));
    il.push_evidence(language_core_symbol_evidence_with_dependencies(
        101,
        Lang::TypeScript,
        EvidenceAnchor::binding(sp(299), stable_symbol_hash("delay")),
        SymbolEvidenceKind::ImportedBinding {
            module_hash: stable_symbol_hash("timers/promises"),
            exported_hash: stable_symbol_hash("setTimeout"),
        },
        vec![EvidenceId(100)],
    ));

    crate::library_api_evidence::run(&mut il, &interner);

    assert_eq!(
        nose_semantics::domain_evidence_for_receiver(&il, &interner, producer_call),
        Some(DomainEvidence::PromiseLike),
        "dependency-closed CJS timers/promises destructuring should prove the PromiseLike domain"
    );
}

#[test]
fn node_timers_commonjs_imported_binding_requires_asserted_dependency() {
    let interner = Interner::new();
    let mut b = IlBuilder::new(FileId(0));
    let delay = b.add(
        NodeKind::Var,
        Payload::Name(interner.intern("delay")),
        sp(300),
        &[],
    );
    let timeout = b.add(NodeKind::Lit, Payload::LitInt(0), sp(301), &[]);
    let producer_call = b.add(NodeKind::Call, Payload::None, sp(303), &[delay, timeout]);
    let mut il = finish_test_il(b, producer_call, Lang::TypeScript);
    let mut require = language_core_symbol_evidence(
        100,
        Lang::TypeScript,
        EvidenceAnchor::node(sp(298), NodeKind::Var),
        SymbolEvidenceKind::UnshadowedGlobal {
            name_hash: stable_symbol_hash("require"),
        },
    );
    require.status = EvidenceStatus::Ambiguous;
    il.push_evidence(require);
    il.push_evidence(language_core_symbol_evidence_with_dependencies(
        101,
        Lang::TypeScript,
        EvidenceAnchor::binding(sp(299), stable_symbol_hash("delay")),
        SymbolEvidenceKind::ImportedBinding {
            module_hash: stable_symbol_hash("timers/promises"),
            exported_hash: stable_symbol_hash("setTimeout"),
        },
        vec![EvidenceId(100)],
    ));

    crate::library_api_evidence::run(&mut il, &interner);

    assert_eq!(
        nose_semantics::domain_evidence_for_receiver(&il, &interner, producer_call),
        None,
        "CJS timers/promises import proof must stay closed when require proof is ambiguous"
    );
}
