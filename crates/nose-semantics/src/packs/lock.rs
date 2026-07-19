use super::{SemanticPackSet, SemanticPackV1Channel};
use serde::{Deserialize, Serialize};
use std::fmt;
use std::path::{Path, PathBuf};

pub const SEMANTIC_PACK_LOCK_API_VERSION_V1: &str = "nose.semantic-pack-lock.v1";

#[derive(Clone, Debug)]
pub struct SemanticPackLockOptions {
    pub allowed_channels: Vec<SemanticPackV1Channel>,
    pub selected_rows: Vec<String>,
    pub dependency_paths: Vec<PathBuf>,
    pub exact_receipt: Option<PathBuf>,
}

#[derive(Clone, PartialEq, Eq, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct SemanticPackProjectLockV1 {
    #[serde(rename = "$schema", skip_serializing_if = "Option::is_none")]
    schema: Option<String>,
    api_version: String,
    dependencies: Vec<SemanticPackLockedFileV1>,
    packs: Vec<SemanticPackLockedEntryV1>,
}

#[derive(Clone, PartialEq, Eq, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct SemanticPackLockedEntryV1 {
    manifest: String,
    manifest_api_version: String,
    pack_id: String,
    pack_version: String,
    nose_compatibility: String,
    semantic_digest: String,
    allowed_channels: Vec<SemanticPackV1Channel>,
    selected_rows: Vec<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    exact_receipt: Option<SemanticPackLockedFileV1>,
}

#[derive(Clone, PartialEq, Eq, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct SemanticPackLockedFileV1 {
    path: String,
    content_digest: String,
}

#[derive(Clone, PartialEq, Eq, Debug)]
pub struct SemanticPackLockedFile {
    declared_path: String,
    resolved_path: PathBuf,
    content_digest: String,
}

impl SemanticPackLockedFile {
    pub fn declared_path(&self) -> &str {
        &self.declared_path
    }

    pub fn resolved_path(&self) -> &Path {
        &self.resolved_path
    }

    pub fn content_digest(&self) -> &str {
        &self.content_digest
    }
}

#[derive(Clone, PartialEq, Eq, Debug)]
pub struct SemanticPackV1Authorization {
    pack_id: String,
    allowed_channels: Vec<SemanticPackV1Channel>,
    selected_rows: Vec<String>,
    dependencies: Vec<SemanticPackLockedFile>,
    exact_receipt: Option<SemanticPackLockedFile>,
}

impl SemanticPackV1Authorization {
    pub fn pack_id(&self) -> &str {
        &self.pack_id
    }

    pub fn allowed_channels(&self) -> &[SemanticPackV1Channel] {
        &self.allowed_channels
    }

    pub fn selected_rows(&self) -> &[String] {
        &self.selected_rows
    }

    pub fn dependencies(&self) -> &[SemanticPackLockedFile] {
        &self.dependencies
    }

    pub fn exact_receipt(&self) -> Option<&SemanticPackLockedFile> {
        self.exact_receipt.as_ref()
    }

    pub fn allows(&self, row_id: &str, channel: SemanticPackV1Channel) -> bool {
        self.allowed_channels.binary_search(&channel).is_ok()
            && self
                .selected_rows
                .binary_search_by(|selected| selected.as_str().cmp(row_id))
                .is_ok()
    }
}

#[derive(Clone, PartialEq, Eq, Debug)]
pub struct SemanticPackProjectLockSummary {
    api_version: &'static str,
    lock_path: PathBuf,
    decision_digest: String,
}

impl SemanticPackProjectLockSummary {
    pub fn api_version(&self) -> &'static str {
        self.api_version
    }

    pub fn lock_path(&self) -> &Path {
        &self.lock_path
    }

    pub fn decision_digest(&self) -> &str {
        &self.decision_digest
    }
}

#[derive(Debug)]
pub struct ValidatedSemanticPackProjectLock {
    summary: SemanticPackProjectLockSummary,
    authorizations: Vec<SemanticPackV1Authorization>,
    semantic_packs: SemanticPackSet,
}

impl ValidatedSemanticPackProjectLock {
    pub fn summary(&self) -> &SemanticPackProjectLockSummary {
        &self.summary
    }

    pub fn authorizations(&self) -> &[SemanticPackV1Authorization] {
        &self.authorizations
    }

    pub fn semantic_packs(&self) -> &SemanticPackSet {
        &self.semantic_packs
    }

    pub fn into_semantic_packs(mut self) -> SemanticPackSet {
        for pack in &mut self.semantic_packs.packs {
            if let Some(authorization) = self
                .authorizations
                .iter()
                .find(|authorization| authorization.pack_id == pack.id)
            {
                pack.influence = if authorization
                    .allowed_channels
                    .binary_search(&SemanticPackV1Channel::ExternalExact)
                    .is_ok()
                {
                    super::SemanticPackInfluence::ExternalClaimExact
                } else if authorization
                    .allowed_channels
                    .binary_search(&SemanticPackV1Channel::Near)
                    .is_ok()
                {
                    super::SemanticPackInfluence::NearOnly
                } else {
                    super::SemanticPackInfluence::MetadataOnly
                };
            }
        }
        self.semantic_packs.external_v1_authorizations = self
            .authorizations
            .iter()
            .cloned()
            .map(|authorization| (authorization.pack_id.clone(), authorization))
            .collect();
        self.semantic_packs.project_lock = Some(self.summary);
        self.semantic_packs
    }
}

#[derive(Debug)]
pub enum SemanticPackLockError {
    Io {
        path: PathBuf,
        source: std::io::Error,
    },
    Json {
        path: PathBuf,
        source: serde_json::Error,
    },
    Invalid {
        path: PathBuf,
        message: String,
    },
    PackLoad(super::SemanticPackLoadError),
}

impl fmt::Display for SemanticPackLockError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Io { path, source } => {
                write!(f, "reading semantic-pack lock {}: {source}", path.display())
            }
            Self::Json { path, source } => {
                write!(f, "parsing semantic-pack lock {}: {source}", path.display())
            }
            Self::Invalid { path, message } => {
                write!(
                    f,
                    "invalid semantic-pack lock {}: {message}",
                    path.display()
                )
            }
            Self::PackLoad(source) => source.fmt(f),
        }
    }
}

impl std::error::Error for SemanticPackLockError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Io { source, .. } => Some(source),
            Self::Json { source, .. } => Some(source),
            Self::PackLoad(source) => Some(source),
            Self::Invalid { .. } => None,
        }
    }
}

impl From<super::SemanticPackLoadError> for SemanticPackLockError {
    fn from(value: super::SemanticPackLoadError) -> Self {
        Self::PackLoad(value)
    }
}

mod paths;
mod validation;
mod version_ranges;
pub use validation::{create_project_lock, validate_project_lock};

#[cfg(test)]
mod tests;
