//! Query-local protocol evidence for the locked external near lane.

use super::*;
use nose_il::{EvidenceAnchor, EvidenceKind, LibraryApiEvidenceKind, NodeId, NodeKind};
use std::collections::{BTreeMap, BTreeSet};

const NEAR_CAVEATS: &[&str] = &[
    "near-only",
    "not-an-equivalence-proof",
    "provider-claim-user-authorized",
    "exact-output-unchanged",
];

#[derive(Clone, PartialEq, Eq, PartialOrd, Ord, Debug, serde::Serialize, serde::Deserialize)]
pub struct SemanticPackNearDependency {
    pub coordinate: String,
    pub declared_version: String,
    pub matched_version: String,
    pub sources: Vec<SemanticPackDependencySource>,
}

#[derive(Clone, PartialEq, Eq, PartialOrd, Ord, Debug, serde::Serialize, serde::Deserialize)]
pub struct SemanticPackNearProvenance {
    pub pack_id: String,
    pub row_id: String,
    pub semantic_digest: String,
    pub row_digest: String,
    pub lane: SemanticPackV1Channel,
    pub trust: String,
    pub operation: SemanticPackV1ProtocolOperation,
    pub dependency: SemanticPackNearDependency,
    pub occurrence_file: String,
    pub call_start_line: u32,
    pub call_end_line: u32,
    pub caveats: Vec<String>,
}

#[derive(Clone, PartialEq, Eq, PartialOrd, Ord, Debug, serde::Serialize, serde::Deserialize)]
pub struct SemanticPackNearProtocol {
    pub operation: SemanticPackV1ProtocolOperation,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub provenance: Option<SemanticPackNearProvenance>,
}

#[derive(Clone, Default, PartialEq, Eq, Debug)]
pub struct SemanticPackNearPackCounts {
    pub selected_rows: usize,
    pub admitted_rows: usize,
    pub rejected_rows: usize,
    pub admitted_occurrences: usize,
    pub influential_occurrences: usize,
}

#[derive(Clone, Default, PartialEq, Eq, Debug)]
pub struct SemanticPackNearReport {
    packs: BTreeMap<String, SemanticPackNearPackCounts>,
}

impl SemanticPackNearReport {
    pub fn pack(&self, pack_id: &str) -> Option<&SemanticPackNearPackCounts> {
        self.packs.get(pack_id)
    }
}

#[derive(Clone, PartialEq, Eq, PartialOrd, Ord, Debug)]
struct ProtocolOccurrence {
    start_line: u32,
    end_line: u32,
    protocol: SemanticPackNearProtocol,
}

#[derive(Clone, Default, Debug)]
pub struct SemanticPackNearRegistry {
    by_file: BTreeMap<String, Vec<ProtocolOccurrence>>,
    report: SemanticPackNearReport,
}

impl SemanticPackNearRegistry {
    pub fn build(
        packs: &SemanticPackSet,
        evidence: &SemanticPackEvidenceIndex,
        corpus: &nose_il::Corpus,
    ) -> Self {
        let mut registry = Self::default();
        registry.index_external(packs, evidence, corpus);
        if registry.has_external_occurrences() {
            registry.index_builtin(corpus);
        }
        for occurrences in registry.by_file.values_mut() {
            occurrences.sort();
            occurrences.dedup();
        }
        registry
    }

    pub fn is_active(&self) -> bool {
        self.has_external_occurrences()
    }

    pub fn protocols_for_unit(
        &self,
        path: &str,
        start_line: u32,
        end_line: u32,
    ) -> Vec<SemanticPackNearProtocol> {
        let Some(occurrences) = self.by_file.get(path) else {
            return Vec::new();
        };
        let mut protocols = occurrences
            .iter()
            .filter(|occurrence| {
                start_line <= occurrence.start_line && occurrence.end_line <= end_line
            })
            .map(|occurrence| occurrence.protocol.clone())
            .collect::<Vec<_>>();
        protocols.sort();
        protocols.dedup();
        protocols
    }

    pub fn report_with_influential<'a>(
        &self,
        influential: impl IntoIterator<Item = &'a SemanticPackNearProvenance>,
    ) -> SemanticPackNearReport {
        let mut report = self.report.clone();
        let unique = influential.into_iter().collect::<BTreeSet<_>>();
        for provenance in unique {
            if let Some(counts) = report.packs.get_mut(&provenance.pack_id) {
                counts.influential_occurrences += 1;
            }
        }
        report
    }

    fn has_external_occurrences(&self) -> bool {
        self.by_file
            .values()
            .flatten()
            .any(|occurrence| occurrence.protocol.provenance.is_some())
    }

    fn index_external(
        &mut self,
        packs: &SemanticPackSet,
        evidence: &SemanticPackEvidenceIndex,
        corpus: &nose_il::Corpus,
    ) {
        for row in evidence
            .rows()
            .iter()
            .filter(|row| row.channel == SemanticPackV1Channel::Near)
        {
            let counts = self.report.packs.entry(row.pack_id.clone()).or_default();
            counts.selected_rows += 1;
            if row.blocker.is_some() {
                counts.rejected_rows += 1;
                continue;
            }
            counts.admitted_rows += 1;
            let Some(pack) = packs
                .compiled_external_v1_packs()
                .iter()
                .find(|pack| pack.pack_id() == row.pack_id)
            else {
                continue;
            };
            let Some(contract) = pack.contracts_by_id().get(&row.row_id) else {
                continue;
            };
            for occurrence in evidence.occurrences_for_row(&row.pack_id, &row.row_id) {
                let Some(dependency) = evidence.dependency(occurrence.dependency) else {
                    continue;
                };
                let Some(file) = corpus.files.get(occurrence.call_span.file.0 as usize) else {
                    continue;
                };
                counts.admitted_occurrences += 1;
                self.by_file
                    .entry(file.meta.path.clone())
                    .or_default()
                    .push(ProtocolOccurrence {
                        start_line: occurrence.call_span.start_line,
                        end_line: occurrence.call_span.end_line,
                        protocol: SemanticPackNearProtocol {
                            operation: contract.operation,
                            provenance: Some(SemanticPackNearProvenance {
                                pack_id: row.pack_id.clone(),
                                row_id: row.row_id.clone(),
                                semantic_digest: row.semantic_digest.clone(),
                                row_digest: row.row_digest.clone(),
                                lane: SemanticPackV1Channel::Near,
                                trust: PackTrust::ExternalOptIn.as_manifest_str().to_string(),
                                operation: contract.operation,
                                dependency: SemanticPackNearDependency {
                                    coordinate: dependency.coordinate.clone(),
                                    declared_version: dependency.declared_version.clone(),
                                    matched_version: dependency.matched_version.clone(),
                                    sources: dependency.sources.clone(),
                                },
                                occurrence_file: file.meta.path.clone(),
                                call_start_line: occurrence.call_span.start_line,
                                call_end_line: occurrence.call_span.end_line,
                                caveats: NEAR_CAVEATS
                                    .iter()
                                    .map(|caveat| (*caveat).to_string())
                                    .collect(),
                            }),
                        },
                    });
            }
        }
    }

    fn index_builtin(&mut self, corpus: &nose_il::Corpus) {
        for il in &corpus.files {
            for index in 0..il.nodes.len() {
                let call = NodeId(index as u32);
                if il.kind(call) != NodeKind::Call {
                    continue;
                }
                let span = il.node(call).span;
                for record in il.evidence_anchored_at(span).filter(|record| {
                    record.anchor == EvidenceAnchor::node(span, NodeKind::Call)
                        && matches!(
                            record.kind,
                            EvidenceKind::LibraryApi(LibraryApiEvidenceKind::Contract { .. })
                        )
                }) {
                    let Some(operation) =
                        crate::library_api::admitted_library_api_near_operation_for_call_record(
                            il,
                            &corpus.interner,
                            call,
                            record,
                        )
                    else {
                        continue;
                    };
                    self.by_file.entry(il.meta.path.clone()).or_default().push(
                        ProtocolOccurrence {
                            start_line: span.start_line,
                            end_line: span.end_line,
                            protocol: SemanticPackNearProtocol {
                                operation,
                                provenance: None,
                            },
                        },
                    );
                }
            }
        }
    }
}
