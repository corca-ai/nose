#![allow(clippy::too_many_lines)]

use super::*;

fn import_fact_evidence(
    id: u32,
    lang: Lang,
    span: Span,
    kind: EvidenceKind,
    status: EvidenceStatus,
) -> EvidenceRecord {
    import_fact_evidence_with_provenance(
        id,
        span,
        kind,
        language_core_provenance(lang),
        status,
        Vec::new(),
    )
}

fn import_fact_evidence_with_provenance(
    id: u32,
    span: Span,
    kind: EvidenceKind,
    provenance: EvidenceProvenance,
    status: EvidenceStatus,
    dependencies: Vec<EvidenceId>,
) -> EvidenceRecord {
    EvidenceRecord {
        id: EvidenceId(id),
        anchor: EvidenceAnchor::sequence(span),
        kind,
        provenance,
        dependencies,
        status,
    }
}

fn language_core_provenance(lang: Lang) -> EvidenceProvenance {
    let (pack_id, producer_id) = language_core_evidence_provenance(lang);
    EvidenceProvenance {
        emitter: EvidenceEmitter::Builtin,
        pack_hash: Some(stable_symbol_hash(pack_id)),
        rule_hash: Some(stable_symbol_hash(producer_id)),
    }
}

fn binding_import_fact(module: &str, exported: &str) -> EvidenceKind {
    EvidenceKind::Import(ImportEvidenceKind::Binding {
        module_hash: stable_symbol_hash(module),
        exported_hash: stable_symbol_hash(exported),
    })
}

fn namespace_import_fact(module: &str) -> EvidenceKind {
    EvidenceKind::Import(ImportEvidenceKind::Namespace {
        module_hash: stable_symbol_hash(module),
    })
}

fn imported_literal_evidence_with_provenance(
    id: u32,
    span: Span,
    kind: EvidenceKind,
    provenance: EvidenceProvenance,
    status: EvidenceStatus,
    dependencies: Vec<EvidenceId>,
) -> EvidenceRecord {
    EvidenceRecord {
        id: EvidenceId(id),
        anchor: EvidenceAnchor::node(span, NodeKind::Seq),
        kind,
        provenance,
        dependencies,
        status,
    }
}

struct ImportFactProbe {
    il: Il,
    node: NodeId,
    span: Span,
}

impl ImportFactProbe {
    fn binding(line: u32, module: &str, exported: &str) -> Self {
        let span = sp(line);
        let mut builder = IlBuilder::new(FileId(0));
        let module = builder.add(
            NodeKind::Lit,
            Payload::LitStr(stable_symbol_hash(module)),
            span,
            &[],
        );
        let exported = builder.add(
            NodeKind::Lit,
            Payload::LitStr(stable_symbol_hash(exported)),
            span,
            &[],
        );
        let node = builder.add(NodeKind::Seq, Payload::None, span, &[module, exported]);
        let root = builder.add(NodeKind::Module, Payload::None, span, &[node]);
        Self {
            il: finish_il(builder, root, Lang::Python),
            node,
            span,
        }
    }

    fn push(
        &mut self,
        id: u32,
        kind: EvidenceKind,
        provenance: EvidenceProvenance,
        dependencies: Vec<EvidenceId>,
    ) {
        self.il.evidence.push(import_fact_evidence_with_provenance(
            id,
            self.span,
            kind,
            provenance,
            EvidenceStatus::Asserted,
            dependencies,
        ));
    }

    fn resolved(&self) -> Option<ImportFact> {
        import_fact_evidence_rhs(&self.il, self.node)
    }
}

mod core;
mod occurrences;
