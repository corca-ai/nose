use crate::strict_exact::{strict_exact_collection_contains_call_safe, StrictFacts};
use nose_il::{
    stable_symbol_hash, DomainEvidence, EvidenceAnchor, EvidenceId, EvidenceKind, FileId, FileMeta,
    Il, IlBuilder, Interner, Lang, NodeId, NodeKind, Payload, Span,
};
use nose_semantics::test_support::{
    compat_test_asserted_evidence, method_call_library_api_test_evidence_with_dependencies,
};

pub(crate) struct BindingDomainContainsFixture {
    pub(crate) il: Il,
    pub(crate) interner: Interner,
    call: NodeId,
    callee: NodeId,
    binding_span: Span,
}

impl BindingDomainContainsFixture {
    pub(crate) fn before_receiver_use() -> Self {
        Self::new(true)
    }

    pub(crate) fn after_receiver_use() -> Self {
        Self::new(false)
    }

    fn new(binding_before_use: bool) -> Self {
        let interner = Interner::new();
        let xs = interner.intern("xs");
        let mut builder = IlBuilder::new(FileId(0));
        let binding_span = span(30);
        let lhs = builder.add(NodeKind::Var, Payload::Cid(0), binding_span, &[]);
        let sequence = builder.add(NodeKind::Seq, Payload::None, span(31), &[]);
        let assignment = builder.add(
            NodeKind::Assign,
            Payload::None,
            binding_span,
            &[lhs, sequence],
        );
        let receiver_line = if binding_before_use { 32 } else { 20 };
        let receiver = builder.add(NodeKind::Var, Payload::Cid(0), span(receiver_line), &[]);
        let callee = builder.add(
            NodeKind::Field,
            Payload::Name(interner.intern("includes")),
            span(receiver_line + 1),
            &[receiver],
        );
        let item = builder.add(
            NodeKind::Lit,
            Payload::LitInt(7),
            span(receiver_line + 2),
            &[],
        );
        let call = builder.add(
            NodeKind::Call,
            Payload::None,
            span(receiver_line + 3),
            &[callee, item],
        );
        let children = if binding_before_use {
            [assignment, call]
        } else {
            [call, assignment]
        };
        let root_line = if binding_before_use { 29 } else { 19 };
        let root = builder.add(NodeKind::Block, Payload::None, span(root_line), &children);
        let mut il = builder.finish(
            root,
            FileMeta {
                path: "t.ts".into(),
                lang: Lang::TypeScript,
            },
            Vec::new(),
            vec![xs],
        );
        il.push_evidence(compat_test_asserted_evidence(
            0,
            EvidenceAnchor::binding(binding_span, stable_symbol_hash("xs")),
            EvidenceKind::Domain(DomainEvidence::Collection),
            Vec::new(),
        ));
        il.push_evidence(method_call_library_api_test_evidence_with_dependencies(
            1,
            Lang::TypeScript,
            "includes",
            il.node(call).span,
            1,
            vec![EvidenceId(0)],
        ));
        Self {
            il,
            interner,
            call,
            callee,
            binding_span,
        }
    }

    pub(crate) fn is_safe(&self) -> bool {
        let facts = StrictFacts::collect(&self.il, &self.interner);
        strict_exact_collection_contains_call_safe(
            &self.il,
            &self.interner,
            &facts,
            self.call,
            self.callee,
            "includes",
        )
    }

    pub(crate) fn add_conflicting_map_domain(&mut self) {
        self.il.push_evidence(compat_test_asserted_evidence(
            2,
            EvidenceAnchor::binding(self.binding_span, stable_symbol_hash("xs")),
            EvidenceKind::Domain(DomainEvidence::Map),
            Vec::new(),
        ));
    }
}

fn span(line: u32) -> Span {
    Span::new(FileId(0), line, line, line, line)
}
