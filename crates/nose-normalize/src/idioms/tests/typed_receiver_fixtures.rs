use super::support::*;

pub(super) fn typed_method_call_il(
    lang: Lang,
    method: &str,
    semantic: ParamSemantic,
    duplicate_param_name: bool,
) -> (Il, Interner, NodeId) {
    let interner = Interner::new();
    let mut b = IlBuilder::new(FileId(0));
    let mut functions = Vec::new();
    let param_span = sp();
    let xs = interner.intern("xs");
    let param = b.add(NodeKind::Param, Payload::Name(xs), param_span, &[]);
    let receiver = b.add(NodeKind::Var, Payload::Name(xs), param_span, &[]);
    let field = b.add(
        NodeKind::Field,
        Payload::Name(interner.intern(method)),
        sp(),
        &[receiver],
    );
    let func_body = b.add(NodeKind::Block, Payload::None, sp(), &[]);
    let func = b.add(NodeKind::Lambda, Payload::None, sp(), &[func_body]);
    let call = b.add(NodeKind::Call, Payload::None, sp(), &[field, func]);
    let body = b.add(NodeKind::Block, Payload::None, sp(), &[call]);
    let function = b.add(NodeKind::Func, Payload::None, sp(), &[param, body]);
    functions.push(function);
    let duplicate_span = Span::new(FileId(0), 10, 11, 2, 2);
    if duplicate_param_name {
        let other_param = b.add(NodeKind::Param, Payload::Name(xs), duplicate_span, &[]);
        let other_body = b.add(NodeKind::Block, Payload::None, duplicate_span, &[]);
        let other_function = b.add(
            NodeKind::Func,
            Payload::None,
            duplicate_span,
            &[other_param, other_body],
        );
        functions.push(other_function);
    }
    let root = b.add(NodeKind::Module, Payload::None, sp(), &functions);
    let mut il = b.finish(
        root,
        FileMeta {
            path: "t".to_string(),
            lang,
        },
        Vec::new(),
        Vec::new(),
    );
    il.evidence.push(evidence(
        0,
        EvidenceAnchor::param(param_span),
        EvidenceKind::Domain(DomainEvidence::from_param_semantic(semantic)),
        EvidenceStatus::Asserted,
    ));
    if duplicate_param_name {
        il.evidence.push(evidence(
            1,
            EvidenceAnchor::param(duplicate_span),
            EvidenceKind::Domain(DomainEvidence::from_param_semantic(semantic)),
            EvidenceStatus::Asserted,
        ));
    }
    let _ = push_receiver_method_library_api_evidence(&mut il, &interner, call);
    (il, interner, call)
}

pub(super) fn receiver_domain_method_call_il(
    domain: DomainEvidence,
) -> (Il, Interner, NodeId, Span) {
    let interner = Interner::new();
    let mut b = IlBuilder::new(FileId(0));
    let receiver_span = sp_at(20, 22, 2);
    let receiver = b.add(
        NodeKind::Var,
        Payload::Name(interner.intern("xs")),
        receiver_span,
        &[],
    );
    let field = b.add(
        NodeKind::Field,
        Payload::Name(interner.intern("some")),
        sp_at(23, 28, 2),
        &[receiver],
    );
    let func_body = b.add(NodeKind::Block, Payload::None, sp_at(29, 30, 2), &[]);
    let func = b.add(
        NodeKind::Lambda,
        Payload::None,
        sp_at(29, 30, 2),
        &[func_body],
    );
    let call = b.add(
        NodeKind::Call,
        Payload::None,
        sp_at(20, 31, 2),
        &[field, func],
    );
    let root = b.add(NodeKind::Module, Payload::None, sp_at(0, 40, 1), &[call]);
    let mut il = b.finish(
        root,
        FileMeta {
            path: "t.ts".to_string(),
            lang: Lang::TypeScript,
        },
        Vec::new(),
        Vec::new(),
    );
    il.evidence.push(evidence(
        0,
        EvidenceAnchor::node(receiver_span, NodeKind::Var),
        EvidenceKind::Domain(domain),
        EvidenceStatus::Asserted,
    ));
    let _ = push_receiver_method_library_api_evidence(&mut il, &interner, call);
    (il, interner, call, receiver_span)
}
