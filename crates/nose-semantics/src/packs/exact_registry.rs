//! Query-local kernel evidence and provenance for external-claim exact rows.

use super::*;
use nose_il::{
    Corpus, DomainEvidence, EvidenceAnchor, EvidenceId, EvidenceKind, EvidenceProvenance,
    EvidenceRecord, EvidenceStatus, LibraryApiEvidenceKind, NodeId, NodeKind, Span,
};
use std::collections::{BTreeMap, BTreeSet};

const EXACT_CAVEATS: &[&str] = &[
    "external-claim-not-builtin-certification",
    "provider-claim-user-authorized",
    "kernel-conformance-receipt",
    "local-content-pinned",
];

#[derive(Clone, PartialEq, Eq, PartialOrd, Ord, Debug, serde::Serialize, serde::Deserialize)]
pub struct SemanticPackExternalExactProvenance {
    pub pack_id: String,
    pub row_id: String,
    pub semantic_digest: String,
    pub row_digest: String,
    pub lane: SemanticPackV1Channel,
    pub assurance: String,
    pub trust: String,
    pub dependency: SemanticPackNearDependency,
    pub receipt_digest: String,
    pub occurrence_file: String,
    pub call_start_line: u32,
    pub call_end_line: u32,
    pub caveats: Vec<String>,
}

#[derive(Clone, Default, PartialEq, Eq, Debug)]
pub struct SemanticPackExternalExactPackCounts {
    pub selected_rows: usize,
    pub admitted_rows: usize,
    pub rejected_rows: usize,
    pub admitted_occurrences: usize,
    pub influential_occurrences: usize,
}

#[derive(Clone, Default, PartialEq, Eq, Debug)]
pub struct SemanticPackExternalExactReport {
    packs: BTreeMap<String, SemanticPackExternalExactPackCounts>,
}

impl SemanticPackExternalExactReport {
    pub fn pack(&self, pack_id: &str) -> Option<&SemanticPackExternalExactPackCounts> {
        self.packs.get(pack_id)
    }
}

#[derive(Clone, PartialEq, Eq, Debug)]
struct ExactOccurrence {
    span: Span,
    arity: u16,
    provenance: SemanticPackExternalExactProvenance,
    dependencies: Vec<EvidenceId>,
}

#[derive(Clone, Default, Debug)]
pub struct SemanticPackExternalExactRegistry {
    by_file: BTreeMap<String, Vec<ExactOccurrence>>,
    report: SemanticPackExternalExactReport,
}

impl SemanticPackExternalExactRegistry {
    pub fn build(
        packs: &SemanticPackSet,
        evidence: &SemanticPackEvidenceIndex,
        corpus: &Corpus,
    ) -> Self {
        let mut registry = Self::default();
        for row in evidence
            .rows()
            .iter()
            .filter(|row| row.channel == SemanticPackV1Channel::ExternalExact)
        {
            let Some(authorization) = packs.external_v1_authorization(&row.pack_id) else {
                continue;
            };
            let Some(receipt) = authorization.exact_receipt() else {
                continue;
            };
            registry.index_row(
                packs,
                evidence,
                corpus,
                row,
                receipt.content_digest().to_string(),
            );
        }
        registry.finish()
    }

    pub fn build_for_conformance(
        pack: &CompiledSemanticPackV1,
        evidence: &SemanticPackEvidenceIndex,
        corpus: &Corpus,
    ) -> Self {
        let mut registry = Self::default();
        for row in evidence
            .rows()
            .iter()
            .filter(|row| row.channel == SemanticPackV1Channel::ExternalExact)
        {
            registry.index_compiled_row(
                pack,
                evidence,
                corpus,
                row,
                "kernel-conformance-run".to_string(),
            );
        }
        registry.finish()
    }

    pub fn is_active(&self) -> bool {
        !self.by_file.is_empty()
    }

    pub fn apply(&self, corpus: &mut Corpus) {
        for il in &mut corpus.files {
            let Some(occurrences) = self.by_file.get(&il.meta.path) else {
                continue;
            };
            let mut next_id = il
                .evidence
                .iter()
                .map(|record| record.id.0)
                .max()
                .unwrap_or(0)
                .saturating_add(1);
            for occurrence in occurrences {
                let Some(call) =
                    (0..il.nodes.len())
                        .map(|index| NodeId(index as u32))
                        .find(|&node| {
                            il.kind(node) == NodeKind::Call && il.node(node).span == occurrence.span
                        })
                else {
                    continue;
                };
                let api_id = EvidenceId(next_id);
                il.push_evidence(EvidenceRecord::new(
                    api_id,
                    EvidenceAnchor::node(il.node(call).span, NodeKind::Call),
                    EvidenceKind::LibraryApi(LibraryApiEvidenceKind::ExternalCollectionFactory {
                        arity: occurrence.arity,
                    }),
                    EvidenceProvenance::external(
                        &occurrence.provenance.pack_id,
                        &occurrence.provenance.row_id,
                    ),
                    occurrence.dependencies.clone(),
                    EvidenceStatus::Asserted,
                ));
                next_id = next_id.saturating_add(1);
                il.push_evidence(EvidenceRecord::new(
                    EvidenceId(next_id),
                    EvidenceAnchor::node(il.node(call).span, NodeKind::Call),
                    EvidenceKind::Domain(DomainEvidence::Collection),
                    EvidenceProvenance::external(
                        &occurrence.provenance.pack_id,
                        &occurrence.provenance.row_id,
                    ),
                    vec![api_id],
                    EvidenceStatus::Asserted,
                ));
                next_id = next_id.saturating_add(1);
            }
        }
    }

    pub fn claims_for_unit(
        &self,
        path: &str,
        start_line: u32,
        end_line: u32,
    ) -> Vec<SemanticPackExternalExactProvenance> {
        let Some(occurrences) = self.by_file.get(path) else {
            return Vec::new();
        };
        let mut claims = occurrences
            .iter()
            .filter(|occurrence| {
                start_line <= occurrence.span.start_line && occurrence.span.end_line <= end_line
            })
            .map(|occurrence| occurrence.provenance.clone())
            .collect::<Vec<_>>();
        claims.sort();
        claims.dedup();
        claims
    }

    pub fn report_with_influential<'a>(
        &self,
        influential: impl IntoIterator<Item = &'a SemanticPackExternalExactProvenance>,
    ) -> SemanticPackExternalExactReport {
        let mut report = self.report.clone();
        for provenance in influential.into_iter().collect::<BTreeSet<_>>() {
            if let Some(counts) = report.packs.get_mut(&provenance.pack_id) {
                counts.influential_occurrences += 1;
            }
        }
        report
    }

    fn index_row(
        &mut self,
        packs: &SemanticPackSet,
        evidence: &SemanticPackEvidenceIndex,
        corpus: &Corpus,
        row: &SemanticPackEvidenceRow,
        receipt_digest: String,
    ) {
        let Some(pack) = packs
            .compiled_external_v1_packs()
            .iter()
            .find(|pack| pack.pack_id() == row.pack_id)
        else {
            return;
        };
        self.index_compiled_row(pack, evidence, corpus, row, receipt_digest);
    }

    fn index_compiled_row(
        &mut self,
        pack: &CompiledSemanticPackV1,
        evidence: &SemanticPackEvidenceIndex,
        corpus: &Corpus,
        row: &SemanticPackEvidenceRow,
        receipt_digest: String,
    ) {
        let counts = self.report.packs.entry(row.pack_id.clone()).or_default();
        counts.selected_rows += 1;
        let Some(contract) = pack.contracts_by_id().get(&row.row_id) else {
            counts.rejected_rows += 1;
            return;
        };
        if row.blocker.is_some()
            || contract.operation != SemanticPackV1ProtocolOperation::CollectionFactory
            || contract.channel != SemanticPackV1Channel::ExternalExact
        {
            counts.rejected_rows += 1;
            return;
        }
        counts.admitted_rows += 1;
        for occurrence in evidence.occurrences_for_row(&row.pack_id, &row.row_id) {
            let Some(dependency) = evidence.dependency(occurrence.dependency) else {
                continue;
            };
            let Some(file) = corpus.files.get(occurrence.call_span.file.0 as usize) else {
                continue;
            };
            let mut dependencies = vec![occurrence.import_evidence, occurrence.symbol_evidence];
            dependencies.extend(occurrence.receiver_evidence);
            dependencies.extend(occurrence.effect_evidence.iter().copied());
            dependencies.extend(occurrence.call_target_evidence.iter().copied());
            dependencies.extend(occurrence.domain_evidence.iter().copied());
            dependencies.extend(occurrence.place_evidence.iter().copied());
            dependencies.sort_by_key(|id| id.0);
            dependencies.dedup();
            counts.admitted_occurrences += 1;
            self.by_file
                .entry(file.meta.path.clone())
                .or_default()
                .push(ExactOccurrence {
                    span: occurrence.call_span,
                    arity: occurrence.arity,
                    provenance: SemanticPackExternalExactProvenance {
                        pack_id: row.pack_id.clone(),
                        row_id: row.row_id.clone(),
                        semantic_digest: row.semantic_digest.clone(),
                        row_digest: row.row_digest.clone(),
                        lane: SemanticPackV1Channel::ExternalExact,
                        assurance: "external-claim-exact".to_string(),
                        trust: PackTrust::ExternalOptIn.as_manifest_str().to_string(),
                        dependency: SemanticPackNearDependency {
                            coordinate: dependency.coordinate.clone(),
                            declared_version: dependency.declared_version.clone(),
                            matched_version: dependency.matched_version.clone(),
                            sources: dependency.sources.clone(),
                        },
                        receipt_digest: receipt_digest.clone(),
                        occurrence_file: file.meta.path.clone(),
                        call_start_line: occurrence.call_span.start_line,
                        call_end_line: occurrence.call_span.end_line,
                        caveats: EXACT_CAVEATS
                            .iter()
                            .map(|caveat| (*caveat).to_string())
                            .collect(),
                    },
                    dependencies,
                });
        }
    }

    fn finish(mut self) -> Self {
        for occurrences in self.by_file.values_mut() {
            occurrences.sort_by(|left, right| {
                (left.span.start_byte, left.span.end_byte, &left.provenance).cmp(&(
                    right.span.start_byte,
                    right.span.end_byte,
                    &right.provenance,
                ))
            });
            occurrences.dedup();
        }
        self
    }
}
