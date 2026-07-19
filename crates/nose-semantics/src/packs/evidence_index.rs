//! Kernel-owned dependency and occurrence evidence for locked typed packs.
//!
//! This index is deliberately separate from IL evidence. It consumes content-pinned
//! project inputs and builtin frontend facts, but cannot mutate the corpus or make a
//! detector admit a result by itself.

use super::*;
use nose_il::{Corpus, EvidenceId, Span};
use rustc_hash::FxHashMap;
use std::collections::BTreeMap;

mod maven;
mod occurrences;

use maven::{MavenCatalog, MavenResolution};
use occurrences::occurrences_for_contract;

#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Debug)]
pub struct SemanticPackDependencyEvidenceId(pub u32);

#[derive(Clone, PartialEq, Eq, PartialOrd, Ord, Debug, serde::Serialize, serde::Deserialize)]
pub struct SemanticPackDependencySource {
    pub declared_path: String,
    pub content_digest: String,
}

#[derive(Clone, PartialEq, Eq, Debug)]
pub struct SemanticPackDependencyEvidence {
    pub id: SemanticPackDependencyEvidenceId,
    pub ecosystem: SemanticPackV1PackageEcosystem,
    pub coordinate: String,
    pub declared_version: String,
    pub matched_version: String,
    pub sources: Vec<SemanticPackDependencySource>,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum SemanticPackEvidenceBlocker {
    MissingDependency,
    InvalidDependencyVersion,
    AmbiguousDependencyVersion,
    OutOfRangeDependencyVersion,
}

impl SemanticPackEvidenceBlocker {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::MissingDependency => "missing-dependency",
            Self::InvalidDependencyVersion => "invalid-dependency-version",
            Self::AmbiguousDependencyVersion => "ambiguous-dependency-version",
            Self::OutOfRangeDependencyVersion => "out-of-range-dependency-version",
        }
    }
}

#[derive(Clone, PartialEq, Eq, Debug)]
pub struct SemanticPackEvidenceRow {
    pub pack_id: String,
    pub row_id: String,
    pub semantic_digest: String,
    pub row_digest: String,
    pub channel: SemanticPackV1Channel,
    pub dependency: Option<SemanticPackDependencyEvidenceId>,
    pub blocker: Option<SemanticPackEvidenceBlocker>,
    occurrence_start: usize,
    occurrence_end: usize,
}

impl SemanticPackEvidenceRow {
    pub fn occurrence_count(&self) -> usize {
        self.occurrence_end - self.occurrence_start
    }
}

#[derive(Clone, PartialEq, Eq, Debug)]
pub struct SemanticPackOccurrenceEvidence {
    pub pack_id: String,
    pub row_id: String,
    pub channel: SemanticPackV1Channel,
    pub call_span: Span,
    pub arity: u16,
    pub dependency: SemanticPackDependencyEvidenceId,
    pub import_evidence: EvidenceId,
    pub symbol_evidence: EvidenceId,
    pub receiver_evidence: Option<EvidenceId>,
    pub receiver_span: Option<Span>,
    pub effect_evidence: Vec<EvidenceId>,
    pub call_target_evidence: Vec<EvidenceId>,
    pub domain_evidence: Vec<EvidenceId>,
    pub place_evidence: Vec<EvidenceId>,
}

#[derive(Clone, Default, PartialEq, Eq, Debug)]
pub struct SemanticPackEvidenceIndex {
    dependencies: Vec<SemanticPackDependencyEvidence>,
    rows: Vec<SemanticPackEvidenceRow>,
    occurrences: Vec<SemanticPackOccurrenceEvidence>,
    row_by_id: BTreeMap<(String, String), usize>,
    occurrences_by_span: FxHashMap<Span, Vec<usize>>,
}

impl SemanticPackEvidenceIndex {
    pub fn build(packs: &SemanticPackSet, corpus: &Corpus) -> Self {
        let catalog = MavenCatalog::from_authorizations(packs.external_v1_authorizations.values());
        let mut builder = EvidenceIndexBuilder::new(catalog);
        let mut compiled = packs.compiled_external_v1_packs.iter().collect::<Vec<_>>();
        compiled.sort_by(|left, right| left.pack_id().cmp(right.pack_id()));
        for pack in compiled {
            let Some(authorization) = packs.external_v1_authorization(pack.pack_id()) else {
                continue;
            };
            for (row_id, contract) in pack.contracts_by_id() {
                if authorization.allows(row_id, contract.channel) {
                    builder.push_row(pack, contract, corpus);
                }
            }
        }
        builder.finish()
    }

    pub fn dependencies(&self) -> &[SemanticPackDependencyEvidence] {
        &self.dependencies
    }

    pub fn dependency(
        &self,
        id: SemanticPackDependencyEvidenceId,
    ) -> Option<&SemanticPackDependencyEvidence> {
        self.dependencies.get(id.0 as usize)
    }

    pub fn rows(&self) -> &[SemanticPackEvidenceRow] {
        &self.rows
    }

    pub fn occurrences(&self) -> &[SemanticPackOccurrenceEvidence] {
        &self.occurrences
    }

    pub fn row(&self, pack_id: &str, row_id: &str) -> Option<&SemanticPackEvidenceRow> {
        let index = self
            .row_by_id
            .get(&(pack_id.to_string(), row_id.to_string()))?;
        self.rows.get(*index)
    }

    pub fn occurrences_for_row(
        &self,
        pack_id: &str,
        row_id: &str,
    ) -> &[SemanticPackOccurrenceEvidence] {
        let Some(row) = self.row(pack_id, row_id) else {
            return &[];
        };
        &self.occurrences[row.occurrence_start..row.occurrence_end]
    }

    pub fn occurrences_at(
        &self,
        span: Span,
    ) -> impl Iterator<Item = &SemanticPackOccurrenceEvidence> {
        self.occurrences_by_span
            .get(&span)
            .into_iter()
            .flatten()
            .filter_map(|index| self.occurrences.get(*index))
    }
}

struct EvidenceIndexBuilder {
    catalog: MavenCatalog,
    dependencies: Vec<SemanticPackDependencyEvidence>,
    dependency_by_key: BTreeMap<maven::MavenEvidenceKey, SemanticPackDependencyEvidenceId>,
    rows: Vec<SemanticPackEvidenceRow>,
    occurrences: Vec<SemanticPackOccurrenceEvidence>,
}

impl EvidenceIndexBuilder {
    fn new(catalog: MavenCatalog) -> Self {
        Self {
            catalog,
            dependencies: Vec::new(),
            dependency_by_key: BTreeMap::new(),
            rows: Vec::new(),
            occurrences: Vec::new(),
        }
    }

    fn push_row(
        &mut self,
        pack: &CompiledSemanticPackV1,
        contract: &SemanticPackV1Contract,
        corpus: &Corpus,
    ) {
        let start = self.occurrences.len();
        let (dependency, blocker) = self.resolve_dependency(pack, contract);
        if let Some(dependency) = dependency {
            self.occurrences.extend(occurrences_for_contract(
                pack.pack_id(),
                contract,
                dependency,
                corpus,
            ));
        }
        self.rows.push(SemanticPackEvidenceRow {
            pack_id: pack.pack_id().to_string(),
            row_id: contract.id.clone(),
            semantic_digest: pack.semantic_digest().to_string(),
            row_digest: pack
                .row_digest(&contract.id)
                .expect("compiled v1 contract has a row digest")
                .to_string(),
            channel: contract.channel,
            dependency,
            blocker,
            occurrence_start: start,
            occurrence_end: self.occurrences.len(),
        });
    }

    fn resolve_dependency(
        &mut self,
        pack: &CompiledSemanticPackV1,
        contract: &SemanticPackV1Contract,
    ) -> (
        Option<SemanticPackDependencyEvidenceId>,
        Option<SemanticPackEvidenceBlocker>,
    ) {
        let Some(package) = pack.packages_by_coordinate().get(&contract.package) else {
            return (None, Some(SemanticPackEvidenceBlocker::MissingDependency));
        };
        match self.catalog.resolve(&package.name, &package.versions) {
            MavenResolution::Resolved(key) => {
                let id = if let Some(id) = self.dependency_by_key.get(&key) {
                    *id
                } else {
                    let id = SemanticPackDependencyEvidenceId(self.dependencies.len() as u32);
                    self.dependencies.push(key.to_evidence(id));
                    self.dependency_by_key.insert(key, id);
                    id
                };
                (Some(id), None)
            }
            MavenResolution::Missing => {
                (None, Some(SemanticPackEvidenceBlocker::MissingDependency))
            }
            MavenResolution::InvalidVersion => (
                None,
                Some(SemanticPackEvidenceBlocker::InvalidDependencyVersion),
            ),
            MavenResolution::AmbiguousVersion => (
                None,
                Some(SemanticPackEvidenceBlocker::AmbiguousDependencyVersion),
            ),
            MavenResolution::OutOfRange => (
                None,
                Some(SemanticPackEvidenceBlocker::OutOfRangeDependencyVersion),
            ),
        }
    }

    fn finish(self) -> SemanticPackEvidenceIndex {
        let row_by_id = self
            .rows
            .iter()
            .enumerate()
            .map(|(index, row)| ((row.pack_id.clone(), row.row_id.clone()), index))
            .collect();
        let mut occurrences_by_span = FxHashMap::<Span, Vec<usize>>::default();
        for (index, occurrence) in self.occurrences.iter().enumerate() {
            occurrences_by_span
                .entry(occurrence.call_span)
                .or_default()
                .push(index);
        }
        SemanticPackEvidenceIndex {
            dependencies: self.dependencies,
            rows: self.rows,
            occurrences: self.occurrences,
            row_by_id,
            occurrences_by_span,
        }
    }
}
