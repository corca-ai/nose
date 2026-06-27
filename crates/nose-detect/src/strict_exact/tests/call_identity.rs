use super::super::*;
use super::support::*;

#[test]
fn same_spelled_function_call_requires_direct_call_target_evidence() {
    let interner = Interner::new();
    let helper = interner.intern("helper");
    let mut b = IlBuilder::new(FileId(0));
    let body = b.add(NodeKind::Lit, Payload::LitInt(1), sp(40), &[]);
    let function = b.add(NodeKind::Func, Payload::None, sp(40), &[body]);
    let callee = b.add(NodeKind::Var, Payload::Name(helper), sp(50), &[]);
    let arg = b.add(NodeKind::Lit, Payload::LitInt(2), sp(51), &[]);
    let call = b.add(NodeKind::Call, Payload::None, sp(52), &[callee, arg]);
    let root = b.add(NodeKind::Block, Payload::None, sp(39), &[function, call]);
    let mut il = b.finish(
        root,
        FileMeta {
            path: "t.ts".into(),
            lang: Lang::TypeScript,
        },
        vec![Unit {
            root: function,
            kind: UnitKind::Function,
            name: Some(helper),
            origin: Default::default(),
        }],
        Vec::new(),
    );

    let facts = StrictFacts::collect(&il, &interner);
    assert!(
        !strict_exact_safe_tree(&il, &interner, &facts, call),
        "same spelling alone must not prove a direct function callee"
    );

    il.evidence.push(call_target_evidence(
        0,
        Lang::TypeScript,
        sp(52),
        CallTargetEvidenceKind::DirectFunction {
            target_span: sp(40),
            name_hash: stable_symbol_hash("helper"),
        },
        Vec::new(),
    ));
    let facts = StrictFacts::collect(&il, &interner);
    assert!(strict_exact_safe_tree(&il, &interner, &facts, call));
}

#[test]
fn imported_function_call_target_opens_opaque_exact_identity() {
    let interner = Interner::new();
    let prod = interner.intern("prod");
    let mut b = IlBuilder::new(FileId(0));
    let callee = b.add(NodeKind::Var, Payload::Name(prod), sp(80), &[]);
    let arg = b.add(NodeKind::Lit, Payload::LitInt(2), sp(81), &[]);
    let call = b.add(NodeKind::Call, Payload::None, sp(82), &[callee, arg]);
    let root = b.add(NodeKind::Block, Payload::None, sp(79), &[call]);
    let mut il = b.finish(
        root,
        FileMeta {
            path: "t.py".into(),
            lang: Lang::Python,
        },
        Vec::new(),
        Vec::new(),
    );

    let facts = StrictFacts::collect(&il, &interner);
    assert!(
        !strict_exact_safe_tree(&il, &interner, &facts, call),
        "same local function spelling must not prove imported call identity"
    );

    il.evidence.push(call_target_evidence(
        0,
        Lang::Python,
        sp(82),
        CallTargetEvidenceKind::ImportedFunction {
            module_hash: stable_symbol_hash("math"),
            exported_hash: stable_symbol_hash("prod"),
            local_hash: interner.symbol_hash(prod),
        },
        Vec::new(),
    ));
    let facts = StrictFacts::collect(&il, &interner);
    assert!(strict_exact_safe_tree(&il, &interner, &facts, call));
}

#[test]
fn ambiguous_call_target_evidence_blocks_parameter_callee_fallback() {
    let interner = Interner::new();
    let mut b = IlBuilder::new(FileId(0));
    let callee_param = b.add(NodeKind::Param, Payload::Cid(0), sp(90), &[]);
    let value_param = b.add(NodeKind::Param, Payload::Cid(1), sp(91), &[]);
    let callee = b.add(NodeKind::Var, Payload::Cid(0), sp(92), &[]);
    let value = b.add(NodeKind::Var, Payload::Cid(1), sp(93), &[]);
    let call = b.add(NodeKind::Call, Payload::None, sp(94), &[callee, value]);
    let root = b.add(
        NodeKind::Func,
        Payload::None,
        sp(89),
        &[callee_param, value_param, call],
    );
    let mut il = b.finish(
        root,
        FileMeta {
            path: "t.py".into(),
            lang: Lang::Python,
        },
        Vec::new(),
        Vec::new(),
    );

    let facts = StrictFacts::collect(&il, &interner);
    assert!(strict_exact_safe_tree(&il, &interner, &facts, call));

    il.evidence.push(call_target_evidence(
        0,
        Lang::Python,
        sp(94),
        CallTargetEvidenceKind::ImportedFunction {
            module_hash: stable_symbol_hash("math"),
            exported_hash: stable_symbol_hash("prod"),
            local_hash: stable_symbol_hash("prod"),
        },
        Vec::new(),
    ));
    il.evidence.push(call_target_evidence(
        1,
        Lang::Python,
        sp(94),
        CallTargetEvidenceKind::ImportedFunction {
            module_hash: stable_symbol_hash("statistics"),
            exported_hash: stable_symbol_hash("prod"),
            local_hash: stable_symbol_hash("prod"),
        },
        Vec::new(),
    ));
    let facts = StrictFacts::collect(&il, &interner);
    assert!(
        !strict_exact_safe_tree(&il, &interner, &facts, call),
        "conflicting call-target evidence must not reopen opaque callee identity"
    );
}

#[test]
fn imported_member_call_target_opens_static_member_identity() {
    let interner = Interner::new();
    let mut b = IlBuilder::new(FileId(0));
    let receiver = b.add(
        NodeKind::Var,
        Payload::Name(interner.intern("math")),
        sp(100),
        &[],
    );
    let callee = b.add(
        NodeKind::Field,
        Payload::Name(interner.intern("prod")),
        sp(101),
        &[receiver],
    );
    let arg = b.add(NodeKind::Lit, Payload::LitInt(3), sp(102), &[]);
    let call = b.add(NodeKind::Call, Payload::None, sp(103), &[callee, arg]);
    let root = b.add(NodeKind::Block, Payload::None, sp(99), &[call]);
    let mut il = b.finish(
        root,
        FileMeta {
            path: "t.py".into(),
            lang: Lang::Python,
        },
        Vec::new(),
        Vec::new(),
    );

    let facts = StrictFacts::collect(&il, &interner);
    assert!(
        !strict_exact_safe_tree(&il, &interner, &facts, call),
        "namespace/member spelling without proof is not exact call identity"
    );

    il.evidence.push(call_target_evidence(
        0,
        Lang::Python,
        sp(103),
        CallTargetEvidenceKind::ImportedMember {
            module_hash: stable_symbol_hash("math"),
            exported_hash: stable_symbol_hash("math"),
            member_hash: interner.symbol_hash(interner.intern("prod")),
        },
        Vec::new(),
    ));
    let facts = StrictFacts::collect(&il, &interner);
    assert!(strict_exact_safe_tree(&il, &interner, &facts, call));
}

#[test]
fn imported_member_call_target_opens_scoped_var_member_identity() {
    let interner = Interner::new();
    let mut b = IlBuilder::new(FileId(0));
    let callee = b.add(
        NodeKind::Var,
        Payload::Name(interner.intern("Span::new")),
        sp(170),
        &[],
    );
    let call = b.add(NodeKind::Call, Payload::None, sp(171), &[callee]);
    let root = b.add(NodeKind::Block, Payload::None, sp(169), &[call]);
    let mut il = b.finish(
        root,
        FileMeta {
            path: "t.rs".into(),
            lang: Lang::Rust,
        },
        Vec::new(),
        Vec::new(),
    );

    let facts = StrictFacts::collect(&il, &interner);
    assert!(
        !strict_exact_safe_tree(&il, &interner, &facts, call),
        "scoped spelling alone must not prove a static member call target"
    );

    il.evidence.push(call_target_evidence(
        0,
        Lang::Rust,
        sp(171),
        CallTargetEvidenceKind::ImportedMember {
            module_hash: stable_symbol_hash("nose_il"),
            exported_hash: stable_symbol_hash("Span"),
            member_hash: stable_symbol_hash("new"),
        },
        Vec::new(),
    ));
    let facts = StrictFacts::collect(&il, &interner);
    assert!(strict_exact_safe_tree(&il, &interner, &facts, call));
}

#[test]
fn normalized_imported_function_call_target_opens_opaque_exact_identity() {
    let interner = Interner::new();
    let il = normalized_python(
        "from acme.ops import transform as tx\n\ndef f(x):\n    return tx(x)\n",
        &interner,
    );
    let call = first_call_with_target(&il, &interner, |target| {
        matches!(
            target,
            CallTargetEvidenceKind::ImportedFunction {
                module_hash,
                exported_hash,
                ..
            } if module_hash == stable_symbol_hash("acme.ops")
                && exported_hash == stable_symbol_hash("transform")
        )
    });

    let facts = StrictFacts::collect(&il, &interner);
    assert!(strict_exact_safe_tree(&il, &interner, &facts, call));
}

#[test]
fn normalized_imported_namespace_member_target_opens_static_member_identity() {
    let interner = Interner::new();
    let il = normalized_python(
        "import acme.ops as ops\n\ndef f(x):\n    return ops.transform(x)\n",
        &interner,
    );
    let call = first_call_with_target(&il, &interner, |target| {
        matches!(
            target,
            CallTargetEvidenceKind::ImportedMember {
                module_hash,
                exported_hash,
                member_hash,
            } if module_hash == stable_symbol_hash("acme.ops")
                && exported_hash == stable_symbol_hash("transform")
                && member_hash == stable_symbol_hash("transform")
        )
    });

    let facts = StrictFacts::collect(&il, &interner);
    assert!(strict_exact_safe_tree(&il, &interner, &facts, call));
}

#[test]
fn direct_method_call_target_does_not_skip_receiver_identity() {
    let interner = Interner::new();
    let mut b = IlBuilder::new(FileId(0));
    let method_body = b.add(NodeKind::Block, Payload::None, sp(110), &[]);
    let method = b.add(NodeKind::Func, Payload::None, sp(111), &[method_body]);
    let receiver = b.add(
        NodeKind::Var,
        Payload::Name(interner.intern("worker")),
        sp(112),
        &[],
    );
    let callee = b.add(
        NodeKind::Field,
        Payload::Name(interner.intern("run")),
        sp(113),
        &[receiver],
    );
    let call = b.add(NodeKind::Call, Payload::None, sp(114), &[callee]);
    let root = b.add(NodeKind::Module, Payload::None, sp(109), &[method, call]);
    let mut il = b.finish(
        root,
        FileMeta {
            path: "t.ts".into(),
            lang: Lang::TypeScript,
        },
        Vec::new(),
        Vec::new(),
    );
    il.evidence.push(call_target_evidence(
        0,
        Lang::TypeScript,
        sp(114),
        CallTargetEvidenceKind::DirectMethod {
            target_span: il.node(method).span,
            receiver_type_hash: stable_symbol_hash("Worker"),
            method_hash: interner.symbol_hash(interner.intern("run")),
        },
        Vec::new(),
    ));

    let facts = StrictFacts::collect(&il, &interner);
    assert!(
        !strict_exact_safe_tree(&il, &interner, &facts, call),
        "direct method target proof does not prove the receiver value identity"
    );
}

#[test]
fn parameter_callee_identity_is_exact_safe_without_library_semantics() {
    let interner = Interner::new();
    let mut b = IlBuilder::new(FileId(0));
    let callee_param = b.add(NodeKind::Param, Payload::Cid(0), sp(10), &[]);
    let value_param = b.add(NodeKind::Param, Payload::Cid(1), sp(11), &[]);
    let callee = b.add(NodeKind::Var, Payload::Cid(0), sp(12), &[]);
    let value = b.add(NodeKind::Var, Payload::Cid(1), sp(13), &[]);
    let call = b.add(NodeKind::Call, Payload::None, sp(14), &[callee, value]);
    let root = b.add(
        NodeKind::Func,
        Payload::None,
        sp(9),
        &[callee_param, value_param, call],
    );
    let il = b.finish(
        root,
        FileMeta {
            path: "t.py".into(),
            lang: Lang::Python,
        },
        vec![Unit {
            root,
            kind: UnitKind::Function,
            name: None,
            origin: Default::default(),
        }],
        Vec::new(),
    );

    let facts = StrictFacts::collect(&il, &interner);
    assert!(
        strict_exact_safe_tree(&il, &interner, &facts, call),
        "a parameter callee is opaque value identity, not library/API semantics"
    );
    assert!(strict_exact_safe_tree(&il, &interner, &facts, root));
}

#[test]
fn function_name_is_not_a_membership_collection_proof() {
    let interner = Interner::new();
    let helper = interner.intern("helper");
    let mut b = IlBuilder::new(FileId(0));
    let body = b.add(NodeKind::Lit, Payload::LitInt(1), sp(60), &[]);
    let function = b.add(NodeKind::Func, Payload::None, sp(60), &[body]);
    let element = b.add(NodeKind::Lit, Payload::LitInt(2), sp(70), &[]);
    let collection = b.add(NodeKind::Var, Payload::Name(helper), sp(71), &[]);
    let membership = b.add(
        NodeKind::BinOp,
        Payload::Op(Op::In),
        sp(72),
        &[element, collection],
    );
    let root = b.add(
        NodeKind::Block,
        Payload::None,
        sp(59),
        &[function, membership],
    );
    let il = b.finish(
        root,
        FileMeta {
            path: "t.ts".into(),
            lang: Lang::TypeScript,
        },
        vec![Unit {
            root: function,
            kind: UnitKind::Function,
            name: Some(helper),
            origin: Default::default(),
        }],
        Vec::new(),
    );

    let facts = StrictFacts::collect(&il, &interner);
    assert!(
        !strict_exact_safe_tree(&il, &interner, &facts, membership),
        "function identity must not be reused as collection receiver evidence"
    );
}
