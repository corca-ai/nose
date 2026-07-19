use super::*;
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

pub const SEMANTIC_PACK_API_VERSION_V1: &str = "nose.semantic-pack.v1";
const MAX_V1_ARITY: u8 = 32;

#[derive(Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct SemanticPackManifestV1 {
    #[serde(rename = "$schema")]
    pub(super) _schema: Option<String>,
    pub(super) api_version: String,
    pub(super) pack: ManifestV1Pack,
    pub(super) provenance: ManifestV1Provenance,
    pub(super) compatibility: ManifestV1Compatibility,
    pub(super) supported_languages: Vec<SemanticPackV1Language>,
    pub(super) packages: Vec<SemanticPackV1Package>,
    pub(super) declares: ManifestV1Declares,
}

#[derive(Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct ManifestV1Pack {
    pub(super) id: String,
    pub(super) kind: SemanticPackKind,
    pub(super) version: String,
    pub(super) display_name: String,
    #[serde(default)]
    pub(super) description: Option<String>,
    pub(super) trust: ManifestV1Trust,
    pub(super) enabled_by_default: bool,
}

#[derive(Clone, Copy, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub(super) enum ManifestV1Trust {
    ExternalOptIn,
}

#[derive(Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct ManifestV1Provenance {
    pub(super) provider: ManifestV1Provider,
    pub(super) license: String,
    pub(super) repository: String,
    #[serde(default)]
    pub(super) source_revision: Option<String>,
}

#[derive(Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct ManifestV1Provider {
    pub(super) name: String,
    #[serde(default)]
    pub(super) contact: Option<String>,
}

#[derive(Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct ManifestV1Compatibility {
    pub(super) nose: String,
}

#[derive(Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct ManifestV1Declares {
    pub(super) api_contracts: Vec<SemanticPackV1Contract>,
}

#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Debug, Deserialize, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum SemanticPackV1Language {
    Java,
}

#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Debug, Deserialize, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum SemanticPackV1PackageEcosystem {
    Maven,
}

#[derive(Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct SemanticPackV1Package {
    pub ecosystem: SemanticPackV1PackageEcosystem,
    pub name: String,
    pub versions: String,
}

#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Debug, Deserialize, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum SemanticPackV1Anchor {
    CallNode,
}

#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Debug, Deserialize, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum SemanticPackV1Matcher {
    ImportedApi,
}

#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Debug, Deserialize, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum SemanticPackV1ImportRole {
    Type,
    StaticMember,
}

#[derive(Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct SemanticPackV1Import {
    pub role: SemanticPackV1ImportRole,
    pub module: String,
    pub name: String,
}

#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Debug, Deserialize, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum SemanticPackV1CallShape {
    StaticMethod,
    FreeFunction,
}

#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Debug, Deserialize, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum SemanticPackV1ReceiverRole {
    ImportedType,
    None,
}

#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Debug, Deserialize, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum SemanticPackV1ArityKind {
    Range,
    Set,
}

#[derive(Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct SemanticPackV1Arity {
    pub kind: SemanticPackV1ArityKind,
    #[serde(default)]
    pub min: Option<u8>,
    #[serde(default)]
    pub max: Option<u8>,
    #[serde(default)]
    pub values: Vec<u8>,
}

#[derive(Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct SemanticPackV1Call {
    pub shape: SemanticPackV1CallShape,
    pub member: String,
    pub arity: SemanticPackV1Arity,
    pub receiver: SemanticPackV1ReceiverRole,
}

#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Debug, Deserialize, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum SemanticPackV1ProtocolOperation {
    CollectionFactory,
    MapFactory,
}

#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Debug, Deserialize, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum SemanticPackV1ResultDomain {
    Collection,
    Map,
}

#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Debug, Deserialize, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum SemanticPackV1DemandProfile {
    Eager,
}

#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Debug, Deserialize, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum SemanticPackV1EffectProfile {
    Pure,
}

#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Debug, Deserialize, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum SemanticPackV1ExceptionProfile {
    NoThrow,
    MayThrow,
}

#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Debug, Deserialize, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum SemanticPackV1MutationProfile {
    None,
}

#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Debug, Deserialize, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum SemanticPackV1IdentityProfile {
    Fresh,
}

#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct SemanticPackV1Profiles {
    pub demand: SemanticPackV1DemandProfile,
    pub effects: SemanticPackV1EffectProfile,
    pub exceptions: SemanticPackV1ExceptionProfile,
    pub mutation: SemanticPackV1MutationProfile,
    pub identity: SemanticPackV1IdentityProfile,
}

#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Debug, Deserialize, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum SemanticPackV1Channel {
    Near,
    ExternalExact,
}

impl SemanticPackV1Channel {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Near => "near",
            Self::ExternalExact => "external-exact",
        }
    }
}

#[derive(Clone, PartialEq, Eq, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct SemanticPackV1Contract {
    pub id: String,
    pub language: SemanticPackV1Language,
    pub package: SemanticPackV1PackageCoordinate,
    pub anchor: SemanticPackV1Anchor,
    pub matcher: SemanticPackV1Matcher,
    pub import: SemanticPackV1Import,
    pub call: SemanticPackV1Call,
    pub operation: SemanticPackV1ProtocolOperation,
    pub result_domain: SemanticPackV1ResultDomain,
    pub profiles: SemanticPackV1Profiles,
    pub channel: SemanticPackV1Channel,
}

#[derive(Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct SemanticPackV1PackageCoordinate {
    pub ecosystem: SemanticPackV1PackageEcosystem,
    pub name: String,
}

#[derive(Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Debug)]
pub struct SemanticPackV1Coordinate {
    pub language: SemanticPackV1Language,
    pub package: SemanticPackV1PackageCoordinate,
    pub import: SemanticPackV1Import,
    pub call_shape: SemanticPackV1CallShape,
    pub member: String,
    pub receiver: SemanticPackV1ReceiverRole,
}

#[derive(Clone, PartialEq, Eq, Debug)]
pub struct CompiledSemanticPackV1 {
    pack_id: String,
    pack_version: String,
    nose_compatibility: String,
    semantic_digest: String,
    row_digests_by_id: BTreeMap<String, String>,
    packages_by_coordinate: BTreeMap<SemanticPackV1PackageCoordinate, SemanticPackV1Package>,
    contracts_by_id: BTreeMap<String, SemanticPackV1Contract>,
    contract_ids_by_coordinate: BTreeMap<SemanticPackV1Coordinate, Vec<String>>,
    contract_ids_by_operation: BTreeMap<SemanticPackV1ProtocolOperation, Vec<String>>,
}

impl CompiledSemanticPackV1 {
    pub fn pack_id(&self) -> &str {
        &self.pack_id
    }

    pub fn pack_version(&self) -> &str {
        &self.pack_version
    }

    pub fn nose_compatibility(&self) -> &str {
        &self.nose_compatibility
    }

    pub fn semantic_digest(&self) -> &str {
        &self.semantic_digest
    }

    pub fn packages_by_coordinate(
        &self,
    ) -> &BTreeMap<SemanticPackV1PackageCoordinate, SemanticPackV1Package> {
        &self.packages_by_coordinate
    }

    pub fn contracts_by_id(&self) -> &BTreeMap<String, SemanticPackV1Contract> {
        &self.contracts_by_id
    }

    pub fn row_digest(&self, row_id: &str) -> Option<&str> {
        self.row_digests_by_id.get(row_id).map(String::as_str)
    }

    pub fn contract_ids_by_coordinate(&self) -> &BTreeMap<SemanticPackV1Coordinate, Vec<String>> {
        &self.contract_ids_by_coordinate
    }

    pub fn contract_ids_by_operation(
        &self,
    ) -> &BTreeMap<SemanticPackV1ProtocolOperation, Vec<String>> {
        &self.contract_ids_by_operation
    }
}

mod compiler;
pub(super) use compiler::compile_manifest_v1;

#[cfg(test)]
mod tests;
