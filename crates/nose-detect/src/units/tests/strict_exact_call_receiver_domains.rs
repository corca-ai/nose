use super::support::*;
use crate::strict_exact::{strict_exact_collection_contains_call_safe, StrictFacts};
use nose_il::{
    EvidenceAnchor, EvidenceId, EvidenceKind, FileId, FileMeta, IlBuilder, Interner, Lang,
    NodeKind, Payload,
};

#[test]
fn strict_exact_contains_consumes_call_receiver_domain_evidence() {
    let interner = Interner::new();
    let mut b = IlBuilder::new(FileId(0));
    let receiver_callee = b.add(
        NodeKind::Var,
        Payload::Name(interner.intern("source")),
        sp(70),
        &[],
    );
    let receiver = b.add(NodeKind::Call, Payload::None, sp(71), &[receiver_callee]);
    let callee = b.add(
        NodeKind::Field,
        Payload::Name(interner.intern("includes")),
        sp(72),
        &[receiver],
    );
    let item = b.add(NodeKind::Lit, Payload::LitInt(7), sp(73), &[]);
    let call = b.add(NodeKind::Call, Payload::None, sp(74), &[callee, item]);
    let root = b.add(NodeKind::Block, Payload::None, sp(69), &[call]);
    let mut il = b.finish(
        root,
        FileMeta {
            path: "call_receiver.ts".into(),
            lang: Lang::TypeScript,
        },
        Vec::new(),
        Vec::new(),
    );

    il.evidence.push(evidence(
        0,
        EvidenceAnchor::node(sp(71), NodeKind::Call),
        EvidenceKind::Domain(nose_semantics::DomainEvidence::Collection),
        Vec::new(),
    ));
    il.evidence.push(method_call_library_api_evidence(
        1,
        Lang::TypeScript,
        "includes",
        sp(74),
        1,
        vec![EvidenceId(0)],
    ));

    let facts = StrictFacts::collect(&il, &interner);
    assert!(strict_exact_collection_contains_call_safe(
        &il, &interner, &facts, call, callee, "includes"
    ));

    il.evidence.push(evidence(
        2,
        EvidenceAnchor::node(sp(71), NodeKind::Call),
        EvidenceKind::Domain(nose_semantics::DomainEvidence::Map),
        Vec::new(),
    ));
    let facts = StrictFacts::collect(&il, &interner);
    assert!(
        !strict_exact_collection_contains_call_safe(
            &il, &interner, &facts, call, callee, "includes"
        ),
        "conflicting call-node receiver-domain evidence must close strict exact proof"
    );
}
