use super::support::*;

pub(super) fn typed_method_shadowed_by_untyped_inner_param_il() -> (Il, Interner, NodeId) {
    let interner = Interner::new();
    let mut b = IlBuilder::new(FileId(0));
    let xs = interner.intern("xs");
    let outer_param_span = Span::new(FileId(0), 2, 4, 1, 1);
    let inner_param_span = Span::new(FileId(0), 22, 24, 2, 2);
    let receiver_span = Span::new(FileId(0), 30, 32, 3, 3);
    let outer_param = b.add(NodeKind::Param, Payload::Name(xs), outer_param_span, &[]);
    let inner_param = b.add(NodeKind::Param, Payload::Name(xs), inner_param_span, &[]);
    let receiver = b.add(NodeKind::Var, Payload::Name(xs), receiver_span, &[]);
    let field = b.add(
        NodeKind::Field,
        Payload::Name(interner.intern("some")),
        Span::new(FileId(0), 30, 37, 3, 3),
        &[receiver],
    );
    let func = b.add(
        NodeKind::Var,
        Payload::Name(interner.intern("f")),
        Span::new(FileId(0), 38, 39, 3, 3),
        &[],
    );
    let call = b.add(
        NodeKind::Call,
        Payload::None,
        Span::new(FileId(0), 30, 42, 3, 3),
        &[field, func],
    );
    let inner_body = b.add(
        NodeKind::Block,
        Payload::None,
        Span::new(FileId(0), 25, 70, 2, 4),
        &[call],
    );
    let lambda = b.add(
        NodeKind::Lambda,
        Payload::None,
        Span::new(FileId(0), 20, 80, 2, 5),
        &[inner_param, inner_body],
    );
    let outer_body = b.add(
        NodeKind::Block,
        Payload::None,
        Span::new(FileId(0), 10, 90, 1, 6),
        &[lambda],
    );
    let function = b.add(
        NodeKind::Func,
        Payload::None,
        Span::new(FileId(0), 1, 100, 1, 7),
        &[outer_param, outer_body],
    );
    let root = b.add(
        NodeKind::Module,
        Payload::None,
        Span::new(FileId(0), 0, 101, 1, 7),
        &[function],
    );
    let mut il = b.finish(
        root,
        FileMeta {
            path: "t".to_string(),
            lang: Lang::TypeScript,
        },
        Vec::new(),
        Vec::new(),
    );
    il.push_evidence(evidence(
        0,
        EvidenceAnchor::param(outer_param_span),
        EvidenceKind::Domain(DomainEvidence::from_param_semantic(
            ParamSemantic::Collection,
        )),
        EvidenceStatus::Asserted,
    ));
    (il, interner, call)
}

pub(super) fn console_log_il(shadow_console: bool) -> (Il, Interner, NodeId) {
    let interner = Interner::new();
    let mut b = IlBuilder::new(FileId(0));
    let mut module_kids = Vec::new();
    let mut cid_names = Vec::new();
    if shadow_console {
        cid_names.push(interner.intern("console"));
        module_kids.push(b.add(NodeKind::Param, Payload::Cid(0), sp(), &[]));
    }
    let receiver = b.add(
        NodeKind::Var,
        Payload::Name(interner.intern("console")),
        sp(),
        &[],
    );
    let field = b.add(
        NodeKind::Field,
        Payload::Name(interner.intern("log")),
        sp(),
        &[receiver],
    );
    let arg = b.add(
        NodeKind::Var,
        Payload::Name(interner.intern("x")),
        sp(),
        &[],
    );
    let call = b.add(NodeKind::Call, Payload::None, sp(), &[field, arg]);
    module_kids.push(call);
    let root = b.add(NodeKind::Module, Payload::None, sp(), &module_kids);
    let mut il = b.finish(
        root,
        FileMeta {
            path: "t".to_string(),
            lang: Lang::JavaScript,
        },
        Vec::new(),
        cid_names,
    );
    if !shadow_console {
        il.push_evidence(language_core_evidence(
            0,
            Lang::JavaScript,
            EvidenceAnchor::node(il.node(receiver).span, NodeKind::Var),
            EvidenceKind::Symbol(SymbolEvidenceKind::UnshadowedGlobal {
                name_hash: stable_symbol_hash("console"),
            }),
            EvidenceStatus::Asserted,
        ));
        let _ = push_receiver_method_library_api_evidence(&mut il, &interner, call);
    }
    (il, interner, call)
}

pub(super) fn go_math_abs_il(with_import: bool) -> (Il, Interner, NodeId) {
    let interner = Interner::new();
    let mut b = IlBuilder::new(FileId(0));
    let mut module_kids = Vec::new();
    if with_import {
        let lhs = b.add(
            NodeKind::Var,
            Payload::Name(interner.intern("math")),
            sp(),
            &[],
        );
        let module = b.add(
            NodeKind::Lit,
            Payload::LitStr(stable_symbol_hash("math")),
            sp(),
            &[],
        );
        let rhs = b.add(NodeKind::Seq, Payload::None, sp(), &[module]);
        module_kids.push(b.add(NodeKind::Assign, Payload::None, sp(), &[lhs, rhs]));
    }
    let receiver = b.add(
        NodeKind::Var,
        Payload::Name(interner.intern("math")),
        sp(),
        &[],
    );
    let field = b.add(
        NodeKind::Field,
        Payload::Name(interner.intern("Abs")),
        sp(),
        &[receiver],
    );
    let arg = b.add(
        NodeKind::Var,
        Payload::Name(interner.intern("x")),
        sp(),
        &[],
    );
    let call = b.add(NodeKind::Call, Payload::None, sp(), &[field, arg]);
    module_kids.push(call);
    let root = b.add(NodeKind::Module, Payload::None, sp(), &module_kids);
    let mut il = b.finish(
        root,
        FileMeta {
            path: "t".to_string(),
            lang: Lang::Go,
        },
        Vec::new(),
        Vec::new(),
    );
    if with_import {
        il.push_evidence(evidence(
            0,
            EvidenceAnchor::sequence(sp()),
            EvidenceKind::Import(ImportEvidenceKind::Namespace {
                module_hash: stable_symbol_hash("math"),
            }),
            EvidenceStatus::Asserted,
        ));
        il.push_evidence(evidence(
            1,
            EvidenceAnchor::binding(sp(), stable_symbol_hash("math")),
            EvidenceKind::Symbol(SymbolEvidenceKind::ImportedNamespace {
                module_hash: stable_symbol_hash("math"),
            }),
            EvidenceStatus::Asserted,
        ));
        il.push_evidence(evidence_with_dependencies(
            2,
            EvidenceAnchor::node(sp(), NodeKind::Var),
            EvidenceKind::Symbol(SymbolEvidenceKind::ImportedNamespace {
                module_hash: stable_symbol_hash("math"),
            }),
            EvidenceStatus::Asserted,
            vec![EvidenceId(1)],
        ));
        let _ = push_receiver_method_library_api_evidence(&mut il, &interner, call);
    }
    (il, interner, call)
}
