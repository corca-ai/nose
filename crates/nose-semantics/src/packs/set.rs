use super::compiled::compiled_builtin_packs;
use super::external::{ExternalContractRow, ExternalEvidenceProducerRow, ExternalValueLawRow};
use super::loading::{self, discover_manifest_paths, SemanticPackLoadError};
use super::lock::{
    validate_project_lock, SemanticPackLockError, SemanticPackProjectLockSummary,
    SemanticPackV1Authorization,
};
use super::model::SemanticPackSummary;
use super::v1::CompiledSemanticPackV1;
use std::collections::BTreeMap;
use std::path::PathBuf;

#[derive(Clone, PartialEq, Eq, Debug)]
pub struct SemanticPackSet {
    pub(super) packs: Vec<SemanticPackSummary>,
    pub(super) external_evidence_producer_rows: Vec<ExternalEvidenceProducerRow>,
    pub(super) external_contract_rows: Vec<ExternalContractRow>,
    pub(super) external_value_law_rows: Vec<ExternalValueLawRow>,
    pub(super) compiled_external_v1_packs: Vec<CompiledSemanticPackV1>,
    pub(super) external_v1_authorizations: BTreeMap<String, SemanticPackV1Authorization>,
    pub(super) project_lock: Option<SemanticPackProjectLockSummary>,
}

impl SemanticPackSet {
    pub fn new_locked(lock_path: &std::path::Path) -> Result<Self, SemanticPackLockError> {
        Ok(validate_project_lock(lock_path)?.into_semantic_packs())
    }

    pub fn new_local(paths: &[PathBuf]) -> Result<Self, SemanticPackLoadError> {
        let manifest_paths = discover_manifest_paths(paths)?;
        let mut packs = compiled_builtin_packs();
        let mut external_evidence_producer_rows = Vec::new();
        let mut external_contract_rows = Vec::new();
        let mut external_value_law_rows = Vec::new();
        let mut compiled_external_v1_packs = Vec::new();
        for path in manifest_paths {
            let loaded = loading::load_local_manifest_with_rows(&path)?;
            if let Some(existing) = packs
                .iter()
                .find(|existing| existing.id == loaded.summary.id)
            {
                return Err(SemanticPackLoadError::DuplicatePackId {
                    id: loaded.summary.id,
                    first_path: existing.manifest_path.clone(),
                    second_path: Some(path),
                });
            }
            external_evidence_producer_rows.extend(loaded.external_evidence_producer_rows);
            external_contract_rows.extend(loaded.external_contract_rows);
            external_value_law_rows.extend(loaded.external_value_law_rows);
            if let Some(compiled) = loaded.compiled_v1 {
                compiled_external_v1_packs.push(compiled);
            }
            packs.push(loaded.summary);
        }
        Ok(Self {
            packs,
            external_evidence_producer_rows,
            external_contract_rows,
            external_value_law_rows,
            compiled_external_v1_packs,
            external_v1_authorizations: BTreeMap::new(),
            project_lock: None,
        })
    }

    pub fn builtin_only() -> Self {
        Self {
            packs: compiled_builtin_packs(),
            external_evidence_producer_rows: Vec::new(),
            external_contract_rows: Vec::new(),
            external_value_law_rows: Vec::new(),
            compiled_external_v1_packs: Vec::new(),
            external_v1_authorizations: BTreeMap::new(),
            project_lock: None,
        }
    }

    pub fn first_party_only() -> Self {
        Self::builtin_only()
    }

    pub fn packs(&self) -> &[SemanticPackSummary] {
        &self.packs
    }

    pub fn external_evidence_producer_rows(&self) -> &[ExternalEvidenceProducerRow] {
        &self.external_evidence_producer_rows
    }

    pub fn external_contract_rows(&self) -> &[ExternalContractRow] {
        &self.external_contract_rows
    }

    pub fn external_value_law_rows(&self) -> &[ExternalValueLawRow] {
        &self.external_value_law_rows
    }

    pub fn compiled_external_v1_packs(&self) -> &[CompiledSemanticPackV1] {
        &self.compiled_external_v1_packs
    }

    pub fn external_v1_authorization(&self, pack_id: &str) -> Option<&SemanticPackV1Authorization> {
        self.external_v1_authorizations.get(pack_id)
    }

    pub fn project_lock(&self) -> Option<&SemanticPackProjectLockSummary> {
        self.project_lock.as_ref()
    }
}
