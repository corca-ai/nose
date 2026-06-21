use super::*;
use std::collections::{HashMap, HashSet};

#[derive(Clone, PartialEq, Eq, Debug)]
pub struct SemanticPackRequirementSummary {
    pub ref_id: String,
    pub subject: String,
    pub required: bool,
    pub same_anchor_as: Option<String>,
    pub within_scope: Option<String>,
    pub before: Option<String>,
    pub after: Option<String>,
}

impl SemanticPackRequirementSummary {
    pub(super) fn from_manifest(requirement: &ManifestRequirement) -> Self {
        Self {
            ref_id: requirement.ref_id.clone(),
            subject: requirement.subject.clone(),
            required: requirement.required,
            same_anchor_as: requirement.same_anchor_as.clone(),
            within_scope: requirement.within_scope.clone(),
            before: requirement.before.clone(),
            after: requirement.after.clone(),
        }
    }
}

#[derive(Clone, PartialEq, Eq, Debug)]
pub struct ExternalEvidenceProducerRow {
    pub pack_id: String,
    pub pack_hash: u64,
    pub manifest_path: PathBuf,
    pub producer_id: String,
    pub producer_hash: u64,
    pub kind: String,
    pub anchors: Vec<SemanticPackAnchor>,
    pub channel: SemanticPackChannel,
    pub emits: Vec<String>,
    pub requirements: Vec<SemanticPackRequirementSummary>,
    pub stable_hash_inputs: Vec<String>,
    pub conflict_policy: String,
    pub notes: Option<String>,
}

impl ExternalEvidenceProducerRow {
    pub(super) fn from_manifest(
        manifest_path: &std::path::Path,
        manifest: &SemanticPackManifest,
        producer: &ManifestEvidenceProducer,
    ) -> Self {
        Self {
            pack_id: manifest.pack.id.clone(),
            pack_hash: semantic_pack_hash(&manifest.pack.id),
            manifest_path: manifest_path.to_path_buf(),
            producer_id: producer.id.clone(),
            producer_hash: stable_symbol_hash(&producer.id),
            kind: producer.kind.clone(),
            anchors: producer.anchors.clone(),
            channel: producer.channel,
            emits: producer.emits.clone(),
            requirements: producer
                .requires
                .iter()
                .map(SemanticPackRequirementSummary::from_manifest)
                .collect(),
            stable_hash_inputs: producer.stable_hash_inputs.clone(),
            conflict_policy: producer.conflict_policy.clone(),
            notes: producer.notes.clone(),
        }
    }
}

#[derive(Clone, PartialEq, Eq, Debug)]
pub struct ExternalContractRow {
    pub pack_id: String,
    pub pack_hash: u64,
    pub manifest_path: PathBuf,
    pub contract_id: String,
    pub contract_hash: u64,
    pub surface: serde_json::Value,
    pub requirements: Vec<SemanticPackRequirementSummary>,
    pub semantics: serde_json::Value,
    pub channel: SemanticPackChannel,
    pub proof_status: SemanticPackProofStatus,
    pub conformance_refs: Vec<String>,
    pub known_unsupported: Vec<String>,
    pub notes: Option<String>,
}

impl ExternalContractRow {
    pub(super) fn from_manifest(
        manifest_path: &std::path::Path,
        manifest: &SemanticPackManifest,
        contract: &ManifestContract,
    ) -> Self {
        Self {
            pack_id: manifest.pack.id.clone(),
            pack_hash: semantic_pack_hash(&manifest.pack.id),
            manifest_path: manifest_path.to_path_buf(),
            contract_id: contract.id.clone(),
            contract_hash: stable_symbol_hash(&contract.id),
            surface: contract.surface.clone(),
            requirements: contract
                .requires
                .iter()
                .map(SemanticPackRequirementSummary::from_manifest)
                .collect(),
            semantics: contract.semantics.clone(),
            channel: contract.channel,
            proof_status: contract.proof_status,
            conformance_refs: contract.conformance_refs.clone(),
            known_unsupported: contract.known_unsupported.clone(),
            notes: contract.notes.clone(),
        }
    }
}

#[derive(Clone, PartialEq, Eq, Debug)]
pub struct ExternalValueLawRow {
    pub pack_id: String,
    pub pack_hash: u64,
    pub manifest_path: PathBuf,
    pub law_id: String,
    pub law_hash: u64,
    pub channel: SemanticPackChannel,
    pub proof_status: SemanticPackProofStatus,
    pub requirements: Vec<SemanticPackRequirementSummary>,
    pub conformance_refs: Vec<String>,
    pub semantics: serde_json::Value,
}

impl ExternalValueLawRow {
    pub(super) fn from_manifest(
        manifest_path: &std::path::Path,
        manifest: &SemanticPackManifest,
        law: &ManifestValueLaw,
    ) -> Self {
        Self {
            pack_id: manifest.pack.id.clone(),
            pack_hash: semantic_pack_hash(&manifest.pack.id),
            manifest_path: manifest_path.to_path_buf(),
            law_id: law.id.clone(),
            law_hash: stable_symbol_hash(&law.id),
            channel: law.channel,
            proof_status: law.proof_status,
            requirements: law
                .requires
                .iter()
                .map(SemanticPackRequirementSummary::from_manifest)
                .collect(),
            conformance_refs: law.conformance_refs.clone(),
            semantics: law.semantics.clone(),
        }
    }
}

#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub enum ExternalRowKind {
    EvidenceProducer,
    Contract,
    ValueLaw,
}

impl ExternalRowKind {
    pub const fn as_str(self) -> &'static str {
        match self {
            ExternalRowKind::EvidenceProducer => "evidence-producer",
            ExternalRowKind::Contract => "contract",
            ExternalRowKind::ValueLaw => "value-law",
        }
    }
}

#[derive(Clone, PartialEq, Eq, Debug)]
pub struct ExternalRowConflict {
    pub kind: ExternalRowKind,
    pub row_id: String,
    pub row_hash: u64,
    pub external_pack_id: String,
    pub external_pack_hash: u64,
    pub external_manifest_path: PathBuf,
    pub conflicting_pack_id: String,
    pub conflicting_pack_hash: u64,
    pub conflicting_source: SemanticPackSource,
    pub conflicting_manifest_path: Option<PathBuf>,
}

#[derive(Clone, PartialEq, Eq, Debug)]
pub struct ExternalRowConflictReport {
    pub conflicts: Vec<ExternalRowConflict>,
}

impl ExternalRowConflictReport {
    pub fn passed(&self) -> bool {
        self.conflicts.is_empty()
    }

    pub fn conflict_count(&self) -> usize {
        self.conflicts.len()
    }
}

#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub enum ExternalInfluenceBlocker {
    DataOnlyRegistration,
    DependencyBackedEvidenceUnavailable,
    ExplicitInfluenceTrustGateMissing,
    ExecutableConformanceUnavailable,
    RowConflict,
}

impl ExternalInfluenceBlocker {
    pub const fn as_str(self) -> &'static str {
        match self {
            ExternalInfluenceBlocker::DataOnlyRegistration => "data-only-registration",
            ExternalInfluenceBlocker::DependencyBackedEvidenceUnavailable => {
                "dependency-backed-evidence-unavailable"
            }
            ExternalInfluenceBlocker::ExplicitInfluenceTrustGateMissing => {
                "explicit-influence-trust-gate-missing"
            }
            ExternalInfluenceBlocker::ExecutableConformanceUnavailable => {
                "executable-conformance-unavailable"
            }
            ExternalInfluenceBlocker::RowConflict => "row-conflict",
        }
    }
}

#[derive(Clone, PartialEq, Eq, Debug)]
pub struct ExternalRowInfluencePreflight {
    pub kind: ExternalRowKind,
    pub row_id: String,
    pub row_hash: u64,
    pub pack_id: String,
    pub pack_hash: u64,
    pub manifest_path: PathBuf,
    pub channel: SemanticPackChannel,
    pub blockers: Vec<ExternalInfluenceBlocker>,
}

impl ExternalRowInfluencePreflight {
    pub fn passed(&self) -> bool {
        self.blockers.is_empty()
    }
}

#[derive(Clone, PartialEq, Eq, Debug)]
pub struct ExternalInfluencePreflightReport {
    pub rows: Vec<ExternalRowInfluencePreflight>,
}

impl ExternalInfluencePreflightReport {
    pub fn passed(&self) -> bool {
        self.rows.iter().all(ExternalRowInfluencePreflight::passed)
    }

    pub fn blocked_count(&self) -> usize {
        self.rows.iter().filter(|row| !row.passed()).count()
    }
}

#[derive(Clone)]
pub(super) struct ExternalRowCoordinate {
    pub(super) kind: ExternalRowKind,
    pub(super) row_id: String,
    pub(super) row_hash: u64,
    pub(super) pack_id: String,
    pub(super) pack_hash: u64,
    pub(super) manifest_path: PathBuf,
    pub(super) channel: SemanticPackChannel,
}

impl ExternalRowCoordinate {
    pub(super) fn from_producer(row: &ExternalEvidenceProducerRow) -> Self {
        Self {
            kind: ExternalRowKind::EvidenceProducer,
            row_id: row.producer_id.clone(),
            row_hash: row.producer_hash,
            pack_id: row.pack_id.clone(),
            pack_hash: row.pack_hash,
            manifest_path: row.manifest_path.clone(),
            channel: row.channel,
        }
    }

    pub(super) fn from_contract(row: &ExternalContractRow) -> Self {
        Self {
            kind: ExternalRowKind::Contract,
            row_id: row.contract_id.clone(),
            row_hash: row.contract_hash,
            pack_id: row.pack_id.clone(),
            pack_hash: row.pack_hash,
            manifest_path: row.manifest_path.clone(),
            channel: row.channel,
        }
    }

    pub(super) fn from_law(row: &ExternalValueLawRow) -> Self {
        Self {
            kind: ExternalRowKind::ValueLaw,
            row_id: row.law_id.clone(),
            row_hash: row.law_hash,
            pack_id: row.pack_id.clone(),
            pack_hash: row.pack_hash,
            manifest_path: row.manifest_path.clone(),
            channel: row.channel,
        }
    }
}

impl SemanticPackSet {
    pub fn external_row_conflicts(&self) -> ExternalRowConflictReport {
        let mut conflicts = Vec::new();
        let coordinates = self.external_row_coordinates();
        for coordinate in &coordinates {
            for builtin in builtin_pack_descriptors() {
                if builtin_descriptor_contains_row_id(*builtin, coordinate.kind, &coordinate.row_id)
                {
                    conflicts.push(ExternalRowConflict {
                        kind: coordinate.kind,
                        row_id: coordinate.row_id.clone(),
                        row_hash: coordinate.row_hash,
                        external_pack_id: coordinate.pack_id.clone(),
                        external_pack_hash: coordinate.pack_hash,
                        external_manifest_path: coordinate.manifest_path.clone(),
                        conflicting_pack_id: builtin.id.to_string(),
                        conflicting_pack_hash: semantic_pack_hash(builtin.id),
                        conflicting_source: SemanticPackSource::CompiledBuiltin,
                        conflicting_manifest_path: None,
                    });
                }
            }
        }

        let mut seen: HashMap<(ExternalRowKind, String), ExternalRowCoordinate> = HashMap::new();
        for coordinate in coordinates {
            let key = (coordinate.kind, coordinate.row_id.clone());
            if let Some(first) = seen.get(&key) {
                conflicts.push(ExternalRowConflict {
                    kind: coordinate.kind,
                    row_id: coordinate.row_id.clone(),
                    row_hash: coordinate.row_hash,
                    external_pack_id: coordinate.pack_id.clone(),
                    external_pack_hash: coordinate.pack_hash,
                    external_manifest_path: coordinate.manifest_path.clone(),
                    conflicting_pack_id: first.pack_id.clone(),
                    conflicting_pack_hash: first.pack_hash,
                    conflicting_source: SemanticPackSource::LocalManifest,
                    conflicting_manifest_path: Some(first.manifest_path.clone()),
                });
            } else {
                seen.insert(key, coordinate);
            }
        }
        ExternalRowConflictReport { conflicts }
    }

    pub fn external_influence_preflight(&self) -> ExternalInfluencePreflightReport {
        self.external_influence_preflight_with(|_| false)
    }

    pub fn external_influence_preflight_with_conformance(
        &self,
        conformance: &SemanticPackConformanceReport,
    ) -> ExternalInfluencePreflightReport {
        self.external_influence_preflight_with(|coordinate| {
            conformance.executable_conformance_passed_for(
                coordinate.kind,
                coordinate.row_hash,
                coordinate.pack_hash,
                &coordinate.manifest_path,
            )
        })
    }

    fn external_influence_preflight_with(
        &self,
        executable_conformance_passed: impl Fn(&ExternalRowCoordinate) -> bool,
    ) -> ExternalInfluencePreflightReport {
        let coordinates = self.external_row_coordinates();
        let mut conflicting_rows = HashSet::new();
        for conflict in self.external_row_conflicts().conflicts {
            conflicting_rows.insert((
                conflict.kind,
                conflict.row_hash,
                conflict.external_pack_hash,
            ));
            if conflict.conflicting_source == SemanticPackSource::LocalManifest {
                conflicting_rows.insert((
                    conflict.kind,
                    conflict.row_hash,
                    conflict.conflicting_pack_hash,
                ));
            }
        }
        let rows = coordinates
            .into_iter()
            .map(|coordinate| {
                let mut blockers = vec![
                    ExternalInfluenceBlocker::DataOnlyRegistration,
                    ExternalInfluenceBlocker::DependencyBackedEvidenceUnavailable,
                    ExternalInfluenceBlocker::ExplicitInfluenceTrustGateMissing,
                ];
                if coordinate.channel.exact_capable() && !executable_conformance_passed(&coordinate)
                {
                    blockers.push(ExternalInfluenceBlocker::ExecutableConformanceUnavailable);
                }
                if conflicting_rows.contains(&(
                    coordinate.kind,
                    coordinate.row_hash,
                    coordinate.pack_hash,
                )) {
                    blockers.push(ExternalInfluenceBlocker::RowConflict);
                }
                ExternalRowInfluencePreflight {
                    kind: coordinate.kind,
                    row_id: coordinate.row_id,
                    row_hash: coordinate.row_hash,
                    pack_id: coordinate.pack_id,
                    pack_hash: coordinate.pack_hash,
                    manifest_path: coordinate.manifest_path,
                    channel: coordinate.channel,
                    blockers,
                }
            })
            .collect();
        ExternalInfluencePreflightReport { rows }
    }

    fn external_row_coordinates(&self) -> Vec<ExternalRowCoordinate> {
        let mut rows = Vec::with_capacity(
            self.external_evidence_producer_rows.len()
                + self.external_contract_rows.len()
                + self.external_value_law_rows.len(),
        );
        rows.extend(
            self.external_evidence_producer_rows
                .iter()
                .map(ExternalRowCoordinate::from_producer),
        );
        rows.extend(
            self.external_contract_rows
                .iter()
                .map(ExternalRowCoordinate::from_contract),
        );
        rows.extend(
            self.external_value_law_rows
                .iter()
                .map(ExternalRowCoordinate::from_law),
        );
        rows
    }
}

fn builtin_descriptor_contains_row_id(
    descriptor: BuiltinPackDescriptor,
    kind: ExternalRowKind,
    row_id: &str,
) -> bool {
    match kind {
        ExternalRowKind::EvidenceProducer => descriptor
            .evidence_producer_ids
            .iter()
            .chain(descriptor.source_fact_producer_ids.iter())
            .any(|id| *id == row_id),
        ExternalRowKind::Contract => descriptor.contract_ids.contains(&row_id),
        ExternalRowKind::ValueLaw => descriptor.value_law_ids().contains(&row_id),
    }
}
