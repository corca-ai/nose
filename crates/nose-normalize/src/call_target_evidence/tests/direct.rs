use super::support::*;

#[test]
fn emits_direct_function_call_target_for_unique_unshadowed_function() {
    let interner = Interner::new();
    let (mut il, func, call) = function_with_call(&interner, "f", "f", false);
    run(&mut il, &interner);
    assert!(direct_function_call_target_at_call(
        &il, &interner, call, func
    ));
    let target = CallTargetEvidenceKind::DirectFunction {
        target_span: il.node(func).span,
        name_hash: stable_symbol_hash("f"),
    };
    let record = il
        .evidence
        .iter()
        .find(|record| record.kind == EvidenceKind::CallTarget(target))
        .expect("direct function call-target evidence");
    assert_eq!(record.provenance, language_core_provenance(Lang::Python));
}

#[test]
fn emits_promise_like_domain_for_direct_async_function_call_result() {
    let DirectReturnFixture {
        interner,
        mut il,
        target,
        call,
        async_boundary,
        ..
    } = direct_return_call_fixture(DirectReturnKind::AsyncLiteral);
    let protocol_id = EvidenceId(500);
    il.push_evidence(EvidenceRecord {
        id: protocol_id,
        anchor: EvidenceAnchor::source_span(il.node(async_boundary.unwrap()).span),
        kind: EvidenceKind::Source(SourceFactKind::Protocol(SourceProtocolKind::AsyncFunction)),
        provenance: language_core_provenance(Lang::TypeScript),
        dependencies: Vec::new(),
        status: EvidenceStatus::Asserted,
    });

    run(&mut il, &interner);

    assert!(direct_function_call_target_at_call(
        &il, &interner, call, target,
    ));
    let call_target = il
        .evidence
        .iter()
        .find(|record| {
            matches!(
                record.kind,
                EvidenceKind::CallTarget(CallTargetEvidenceKind::DirectFunction { .. })
            )
        })
        .expect("direct function call-target evidence");
    let domain = il
        .evidence
        .iter()
        .find(|record| {
            record.anchor == EvidenceAnchor::node(il.node(call).span, NodeKind::Call)
                && record.kind == EvidenceKind::Domain(DomainEvidence::PromiseLike)
        })
        .expect("PromiseLike domain evidence for async function call result");
    assert_eq!(domain.dependencies, vec![call_target.id, protocol_id]);
}

#[test]
fn direct_async_function_result_domain_is_js_ts_only() {
    for lang in [Lang::Python, Lang::Rust, Lang::Swift] {
        let DirectReturnFixture {
            interner,
            mut il,
            target,
            call,
            async_boundary,
            ..
        } = direct_return_call_fixture_for_lang(DirectReturnKind::AsyncLiteral, lang);
        il.push_evidence(EvidenceRecord {
            id: EvidenceId(500),
            anchor: EvidenceAnchor::source_span(il.node(async_boundary.unwrap()).span),
            kind: EvidenceKind::Source(SourceFactKind::Protocol(SourceProtocolKind::AsyncFunction)),
            provenance: language_core_provenance(lang),
            dependencies: Vec::new(),
            status: EvidenceStatus::Asserted,
        });

        run(&mut il, &interner);

        assert!(
            direct_function_call_target_at_call(&il, &interner, call, target),
            "{lang:?} should still get direct call-target evidence"
        );
        assert!(
            promise_like_domain_at_call(&il, call).is_none(),
            "{lang:?} async function calls must not be asserted as JS PromiseLike"
        );
    }
}

#[test]
fn emits_promise_like_domain_for_direct_function_returning_promise_like() {
    let DirectReturnFixture {
        interner,
        mut il,
        call,
        return_expr,
        ..
    } = direct_return_call_fixture(DirectReturnKind::PromiseCall);
    let return_domain_id = EvidenceId(500);
    let return_domain = promise_like_domain_record(&il, return_domain_id, return_expr);
    il.push_evidence(return_domain);

    run(&mut il, &interner);

    let call_target = il
        .evidence
        .iter()
        .find(|record| {
            matches!(
                record.kind,
                EvidenceKind::CallTarget(CallTargetEvidenceKind::DirectFunction { .. })
            )
        })
        .expect("direct function call-target evidence");
    let domain = il
        .evidence
        .iter()
        .find(|record| {
            record.anchor == EvidenceAnchor::node(il.node(call).span, NodeKind::Call)
                && record.kind == EvidenceKind::Domain(DomainEvidence::PromiseLike)
        })
        .expect("PromiseLike domain evidence for direct function call result");
    assert_eq!(domain.dependencies, vec![call_target.id, return_domain_id]);
}

#[test]
fn emits_promise_like_domain_for_branching_direct_function_returns() {
    let interner = Interner::new();
    let load = interner.intern("load");
    let use_value = interner.intern("useValue");
    let mut b = IlBuilder::new(FileId(0));
    let param = b.add(NodeKind::Param, Payload::Cid(0), sp(1), &[]);
    let cond = b.add(NodeKind::Var, Payload::Cid(0), sp(2), &[]);
    let then_expr = promise_call_expr(&mut b, &interner, 10);
    let then_ret = b.add(NodeKind::Return, Payload::None, sp(14), &[then_expr]);
    let then_block = b.add(NodeKind::Block, Payload::None, sp(15), &[then_ret]);
    let else_expr = promise_call_expr(&mut b, &interner, 20);
    let else_ret = b.add(NodeKind::Return, Payload::None, sp(24), &[else_expr]);
    let else_block = b.add(NodeKind::Block, Payload::None, sp(25), &[else_ret]);
    let branch = b.add(
        NodeKind::If,
        Payload::None,
        sp(26),
        &[cond, then_block, else_block],
    );
    let body = b.add(NodeKind::Block, Payload::None, sp(27), &[branch]);
    let target = b.add(NodeKind::Func, Payload::None, sp(28), &[param, body]);
    let callee = b.add(NodeKind::Var, Payload::Name(load), sp(30), &[]);
    let call_arg = b.add(NodeKind::Lit, Payload::LitBool(true), sp(31), &[]);
    let call = b.add(NodeKind::Call, Payload::None, sp(32), &[callee, call_arg]);
    let caller_ret = b.add(NodeKind::Return, Payload::None, sp(33), &[call]);
    let caller_body = b.add(NodeKind::Block, Payload::None, sp(34), &[caller_ret]);
    let caller = b.add(NodeKind::Func, Payload::None, sp(35), &[caller_body]);
    let module = b.add(NodeKind::Module, Payload::None, sp(36), &[target, caller]);
    let mut il = b.finish(
        module,
        FileMeta {
            path: "t".into(),
            lang: Lang::TypeScript,
        },
        vec![
            Unit {
                root: target,
                kind: UnitKind::Function,
                name: Some(load),
                origin: Default::default(),
            },
            Unit {
                root: caller,
                kind: UnitKind::Function,
                name: Some(use_value),
                origin: Default::default(),
            },
        ],
        Vec::new(),
    );
    let then_domain_id = EvidenceId(500);
    let else_domain_id = EvidenceId(501);
    for (id, expr) in [(then_domain_id, then_expr), (else_domain_id, else_expr)] {
        let record = promise_like_domain_record(&il, id, expr);
        il.push_evidence(record);
    }

    run(&mut il, &interner);

    let call_target = il
        .evidence
        .iter()
        .find(|record| {
            matches!(
                record.kind,
                EvidenceKind::CallTarget(CallTargetEvidenceKind::DirectFunction { .. })
            )
        })
        .expect("direct function call-target evidence");
    let domain = il
        .evidence
        .iter()
        .find(|record| {
            record.anchor == EvidenceAnchor::node(il.node(call).span, NodeKind::Call)
                && record.kind == EvidenceKind::Domain(DomainEvidence::PromiseLike)
        })
        .expect("PromiseLike domain evidence for branching direct function call result");
    assert_eq!(
        domain.dependencies,
        vec![call_target.id, then_domain_id, else_domain_id]
    );
}

#[test]
fn direct_function_return_domain_requires_return_expression_domain_proof() {
    let DirectReturnFixture {
        interner,
        mut il,
        target,
        call,
        ..
    } = direct_return_call_fixture(DirectReturnKind::Literal);

    run(&mut il, &interner);

    assert!(direct_function_call_target_at_call(
        &il, &interner, call, target
    ));
    assert!(
        il.evidence
            .iter()
            .find(|record| {
                record.anchor == EvidenceAnchor::node(il.node(call).span, NodeKind::Call)
                    && record.kind == EvidenceKind::Domain(DomainEvidence::PromiseLike)
            })
            .is_none(),
        "direct function result domain requires returned expression domain evidence"
    );
}

struct DirectReturnFixture {
    interner: Interner,
    il: Il,
    target: NodeId,
    call: NodeId,
    return_expr: NodeId,
    async_boundary: Option<NodeId>,
}

#[derive(Clone, Copy)]
enum DirectReturnKind {
    AsyncLiteral,
    PromiseCall,
    Literal,
}

fn direct_return_call_fixture(kind: DirectReturnKind) -> DirectReturnFixture {
    direct_return_call_fixture_for_lang(kind, Lang::TypeScript)
}

fn direct_return_call_fixture_for_lang(kind: DirectReturnKind, lang: Lang) -> DirectReturnFixture {
    let interner = Interner::new();
    let load = interner.intern("load");
    let use_value = interner.intern("useValue");
    let mut b = IlBuilder::new(FileId(0));
    let return_expr = match kind {
        DirectReturnKind::PromiseCall => {
            let producer = interner.intern("producer");
            let promise_callee = b.add(NodeKind::Var, Payload::Name(producer), sp(1), &[]);
            b.add(NodeKind::Call, Payload::None, sp(2), &[promise_callee])
        }
        DirectReturnKind::AsyncLiteral | DirectReturnKind::Literal => {
            b.add(NodeKind::Lit, Payload::LitInt(1), sp(1), &[])
        }
    };
    let ret = b.add(NodeKind::Return, Payload::None, sp(3), &[return_expr]);
    let body = b.add(NodeKind::Block, Payload::None, sp(4), &[ret]);
    let (target, async_boundary) = match kind {
        DirectReturnKind::AsyncLiteral => {
            let async_tag = interner.intern("async_function");
            let boundary = b.add(NodeKind::Raw, Payload::Name(async_tag), sp(5), &[body]);
            (
                b.add(NodeKind::Func, Payload::None, sp(6), &[boundary]),
                Some(boundary),
            )
        }
        DirectReturnKind::PromiseCall | DirectReturnKind::Literal => {
            (b.add(NodeKind::Func, Payload::None, sp(6), &[body]), None)
        }
    };
    let callee = b.add(NodeKind::Var, Payload::Name(load), sp(10), &[]);
    let call = b.add(NodeKind::Call, Payload::None, sp(11), &[callee]);
    let caller_ret = b.add(NodeKind::Return, Payload::None, sp(12), &[call]);
    let caller_body = b.add(NodeKind::Block, Payload::None, sp(13), &[caller_ret]);
    let caller = b.add(NodeKind::Func, Payload::None, sp(14), &[caller_body]);
    let module = b.add(NodeKind::Module, Payload::None, sp(15), &[target, caller]);
    let il = b.finish(
        module,
        FileMeta {
            path: "t".into(),
            lang,
        },
        vec![
            Unit {
                root: target,
                kind: UnitKind::Function,
                name: Some(load),
                origin: Default::default(),
            },
            Unit {
                root: caller,
                kind: UnitKind::Function,
                name: Some(use_value),
                origin: Default::default(),
            },
        ],
        Vec::new(),
    );
    DirectReturnFixture {
        interner,
        il,
        target,
        call,
        return_expr,
        async_boundary,
    }
}

#[test]
fn updates_legacy_first_party_direct_function_call_target() {
    let interner = Interner::new();
    let (mut il, func, call) = function_with_call(&interner, "f", "f", false);
    let target = EvidenceKind::CallTarget(CallTargetEvidenceKind::DirectFunction {
        target_span: il.node(func).span,
        name_hash: stable_symbol_hash("f"),
    });
    il.find_or_push_first_party_evidence(
        EvidenceAnchor::node(il.node(call).span, NodeKind::Call),
        target,
        BUILTIN_COMPAT_PACK_ID,
        "legacy_direct_function_call_target",
        Vec::new(),
    );

    run(&mut il, &interner);

    let records: Vec<_> = il
        .evidence
        .iter()
        .filter(|record| {
            record.anchor == EvidenceAnchor::node(il.node(call).span, NodeKind::Call)
                && record.kind == target
        })
        .collect();
    assert_eq!(records.len(), 1);
    assert_eq!(
        records[0].provenance,
        language_core_provenance(Lang::Python)
    );
}

#[test]
fn does_not_promote_current_and_legacy_direct_function_duplicates() {
    let interner = Interner::new();
    let (mut il, func, call) = function_with_call(&interner, "f", "f", false);
    let target = EvidenceKind::CallTarget(CallTargetEvidenceKind::DirectFunction {
        target_span: il.node(func).span,
        name_hash: stable_symbol_hash("f"),
    });
    il.find_or_push_first_party_evidence_with_provenance(
        EvidenceAnchor::node(il.node(call).span, NodeKind::Call),
        target,
        language_core_provenance(Lang::Python),
        Vec::new(),
    );
    il.find_or_push_first_party_evidence(
        EvidenceAnchor::node(il.node(call).span, NodeKind::Call),
        target,
        BUILTIN_COMPAT_PACK_ID,
        "legacy_direct_function_call_target",
        Vec::new(),
    );

    run(&mut il, &interner);

    let current_records = il
        .evidence
        .iter()
        .filter(|record| {
            record.kind == target && record.provenance == language_core_provenance(Lang::Python)
        })
        .count();
    let legacy_records = il
        .evidence
        .iter()
        .filter(|record| {
            record.kind == target
                && record.provenance.pack_hash == Some(stable_symbol_hash(BUILTIN_COMPAT_PACK_ID))
        })
        .count();
    assert_eq!(current_records, 1);
    assert_eq!(legacy_records, 1);
    assert!(direct_function_call_target_at_call(
        &il, &interner, call, func
    ));
}

#[test]
fn does_not_emit_when_local_binder_shadows_function_name() {
    let interner = Interner::new();
    let f = interner.intern("f");
    let mut b = IlBuilder::new(FileId(0));
    let param = b.add(NodeKind::Param, Payload::Name(f), sp(1), &[]);
    let callee = b.add(NodeKind::Var, Payload::Name(f), sp(2), &[]);
    let call = b.add(NodeKind::Call, Payload::None, sp(3), &[callee]);
    let ret = b.add(NodeKind::Return, Payload::None, sp(4), &[call]);
    let body = b.add(NodeKind::Block, Payload::None, sp(5), &[ret]);
    let func = b.add(NodeKind::Func, Payload::None, sp(6), &[param, body]);
    let module = b.add(NodeKind::Module, Payload::None, sp(7), &[func]);
    let mut il = b.finish(
        module,
        FileMeta {
            path: "t".into(),
            lang: Lang::Python,
        },
        vec![Unit {
            root: func,
            kind: UnitKind::Function,
            name: Some(f),
            origin: Default::default(),
        }],
        Vec::new(),
    );

    run(&mut il, &interner);
    assert!(!direct_function_call_target_at_call(
        &il, &interner, call, func
    ));
}

#[test]
fn does_not_emit_for_duplicate_function_names() {
    let interner = Interner::new();
    let (mut il, func, call) = function_with_call(&interner, "f", "f", true);
    run(&mut il, &interner);
    assert!(!direct_function_call_target_at_call(
        &il, &interner, call, func
    ));
}

#[test]
fn does_not_emit_for_method_bare_call() {
    let interner = Interner::new();
    let method_sym = interner.intern("fac");
    let mut b = IlBuilder::new(FileId(0));
    let callee = b.add(NodeKind::Var, Payload::Name(method_sym), sp(20), &[]);
    let call = b.add(NodeKind::Call, Payload::None, sp(21), &[callee]);
    let ret = b.add(NodeKind::Return, Payload::None, sp(22), &[call]);
    let body = b.add(NodeKind::Block, Payload::None, sp(23), &[ret]);
    let method = b.add(NodeKind::Func, Payload::None, sp(24), &[body]);
    let module = b.add(NodeKind::Module, Payload::None, sp(25), &[method]);
    let mut il = b.finish(
        module,
        FileMeta {
            path: "t".into(),
            lang: Lang::Java,
        },
        vec![Unit {
            root: method,
            kind: UnitKind::Method,
            name: Some(method_sym),
            origin: Default::default(),
        }],
        Vec::new(),
    );

    run(&mut il, &interner);
    assert!(!direct_function_call_target_at_call(
        &il, &interner, call, method
    ));
}

#[test]
fn does_not_emit_for_nested_function_not_visible_as_top_level() {
    let DirectFunctionCallTargetFixture {
        mut il,
        interner,
        target,
        call,
    } = direct_function_call_target_test_il(
        DirectFunctionFixtureScope::NestedTarget,
        DirectFunctionFixtureSelector::TargetName,
    );

    run(&mut il, &interner);
    assert!(!direct_function_call_target_at_call(
        &il, &interner, call, target
    ));
}

#[test]
fn does_not_emit_when_enclosing_scope_binds_function_name() {
    let DirectFunctionCallTargetFixture {
        mut il,
        interner,
        target,
        call,
    } = direct_function_call_target_test_il(
        DirectFunctionFixtureScope::EnclosingBinding,
        DirectFunctionFixtureSelector::TargetName,
    );

    run(&mut il, &interner);
    assert!(!direct_function_call_target_at_call(
        &il, &interner, call, target
    ));
}
