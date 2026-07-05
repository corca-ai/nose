pub(super) use super::super::*;
pub(super) use nose_il::{
    stable_symbol_hash, EvidenceProvenance, EvidenceRecord, FileId, FileMeta, IlBuilder, Lang,
    Span, Unit,
};
pub(super) use nose_semantics::{
    call_target_evidence_at_call, direct_function_call_target_at_call,
    imported_function_call_target_at_call, imported_member_call_target_at_call,
    language_core_evidence_provenance,
    test_support::compat_test_evidence_with_dependencies as evidence_with_dependencies,
};

pub(super) fn sp(n: u32) -> Span {
    Span::new(FileId(0), n, n + 1, n, n)
}

pub(super) fn wide_sp(start: u32, end: u32) -> Span {
    Span::new(FileId(0), start, end, start, end)
}

pub(super) fn language_core_provenance(lang: Lang) -> EvidenceProvenance {
    let (pack_id, producer_id) = language_core_evidence_provenance(lang);
    EvidenceProvenance::builtin(pack_id, producer_id)
}

pub(super) fn binding_symbol(
    id: u32,
    span: Span,
    local: &str,
    symbol: SymbolEvidenceKind,
    status: EvidenceStatus,
) -> EvidenceRecord {
    evidence_with_dependencies(
        id,
        EvidenceAnchor::binding(span, stable_symbol_hash(local)),
        EvidenceKind::Symbol(symbol),
        status,
        Vec::new(),
    )
}

pub(super) fn function_with_call(
    interner: &Interner,
    func_name: &str,
    callee_name: &str,
    duplicate_unit: bool,
) -> (Il, NodeId, NodeId) {
    let mut b = IlBuilder::new(FileId(0));
    let func_sym = interner.intern(func_name);
    let callee_sym = interner.intern(callee_name);
    let callee = b.add(NodeKind::Var, Payload::Name(callee_sym), sp(10), &[]);
    let call = b.add(NodeKind::Call, Payload::None, sp(11), &[callee]);
    let ret = b.add(NodeKind::Return, Payload::None, sp(12), &[call]);
    let body = b.add(NodeKind::Block, Payload::None, sp(13), &[ret]);
    let func = b.add(NodeKind::Func, Payload::None, sp(14), &[body]);
    let module = b.add(NodeKind::Module, Payload::None, sp(15), &[func]);
    let mut units = vec![Unit {
        root: func,
        kind: UnitKind::Function,
        name: Some(func_sym),
        origin: Default::default(),
    }];
    if duplicate_unit {
        units.push(Unit {
            root: func,
            kind: UnitKind::Function,
            name: Some(func_sym),
            origin: Default::default(),
        });
    }
    let il = b.finish(
        module,
        FileMeta {
            path: "t".into(),
            lang: Lang::Python,
        },
        units,
        Vec::new(),
    );
    (il, func, call)
}

pub(super) fn promise_call_expr(
    builder: &mut IlBuilder,
    interner: &Interner,
    base_line: u32,
) -> NodeId {
    let producer = interner.intern("producer");
    let callee = builder.add(NodeKind::Var, Payload::Name(producer), sp(base_line), &[]);
    builder.add(NodeKind::Call, Payload::None, sp(base_line + 1), &[callee])
}

pub(super) fn promise_like_domain_record(il: &Il, id: EvidenceId, node: NodeId) -> EvidenceRecord {
    EvidenceRecord {
        id,
        anchor: EvidenceAnchor::node(il.node(node).span, NodeKind::Call),
        kind: EvidenceKind::Domain(DomainEvidence::PromiseLike),
        provenance: language_core_provenance(Lang::TypeScript),
        dependencies: Vec::new(),
        status: EvidenceStatus::Asserted,
    }
}

pub(super) fn direct_method_target_record(
    il: &Il,
    interner: &Interner,
    id: EvidenceId,
    call: NodeId,
    target: NodeId,
    method: Symbol,
) -> EvidenceRecord {
    EvidenceRecord {
        id,
        anchor: EvidenceAnchor::node(il.node(call).span, NodeKind::Call),
        kind: EvidenceKind::CallTarget(CallTargetEvidenceKind::DirectMethod {
            target_span: il.node(target).span,
            receiver_type_hash: stable_symbol_hash("Worker"),
            method_hash: interner.symbol_hash(method),
        }),
        provenance: language_core_provenance(Lang::TypeScript),
        dependencies: Vec::new(),
        status: EvidenceStatus::Asserted,
    }
}

pub(super) fn promise_like_domain_at_call(il: &Il, call: NodeId) -> Option<&EvidenceRecord> {
    il.evidence.iter().find(|record| {
        record.anchor == EvidenceAnchor::node(il.node(call).span, NodeKind::Call)
            && record.kind == EvidenceKind::Domain(DomainEvidence::PromiseLike)
    })
}
