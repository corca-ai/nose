use crate::intern::Symbol;
use crate::node::NodeId;
use crate::unit_domains::{UnitDomain, UnitDomains};
use crate::unit_evidence::{UnitEvidenceFlag, UnitEvidenceFlags};
use crate::unit_facets::{
    RegionKind, SourceGranularity, UnitBodyKind, UnitContainerKind, UnitSubkind,
};
use serde::{Deserialize, Serialize};

/// A detection unit: a syntactic region (function/method/class/block) tagged by a
/// frontend. Its span comes from `root`'s node. Boundaries are real syntactic
/// boundaries, which gives the detector accurate report spans for free.
#[derive(Clone, Copy, Debug, Serialize, Deserialize)]
pub struct Unit {
    pub root: NodeId,
    pub kind: UnitKind,
    pub name: Option<Symbol>,
    #[serde(default, skip_serializing_if = "UnitOrigin::is_unknown")]
    pub origin: UnitOrigin,
}

#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug, Serialize, Deserialize)]
pub enum UnitKind {
    Function,
    Method,
    Class,
    Block,
}

#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug, Serialize, Deserialize)]
pub struct UnitOrigin {
    #[serde(default, skip_serializing_if = "UnitDomains::is_empty")]
    pub domains: UnitDomains,
    #[serde(default, skip_serializing_if = "UnitSubkind::is_unknown")]
    pub subkind: UnitSubkind,
    #[serde(default, skip_serializing_if = "UnitBodyKind::is_unknown")]
    pub body_kind: UnitBodyKind,
    #[serde(default, skip_serializing_if = "SourceGranularity::is_unknown")]
    pub source_granularity: SourceGranularity,
    #[serde(default, skip_serializing_if = "RegionKind::is_unknown")]
    pub region_kind: RegionKind,
    #[serde(default, skip_serializing_if = "UnitContainerKind::is_unknown")]
    pub container_kind: UnitContainerKind,
    #[serde(default, skip_serializing_if = "UnitEvidenceFlags::is_empty")]
    pub evidence_flags: UnitEvidenceFlags,
}

impl UnitOrigin {
    pub const fn unknown() -> Self {
        Self {
            domains: UnitDomains::empty(),
            subkind: UnitSubkind::Unknown,
            body_kind: UnitBodyKind::Unknown,
            source_granularity: SourceGranularity::Unknown,
            region_kind: RegionKind::Unknown,
            container_kind: UnitContainerKind::Unknown,
            evidence_flags: UnitEvidenceFlags::empty(),
        }
    }

    pub const fn new(
        domains: UnitDomains,
        subkind: UnitSubkind,
        body_kind: UnitBodyKind,
        source_granularity: SourceGranularity,
        region_kind: RegionKind,
    ) -> Self {
        Self {
            domains,
            subkind,
            body_kind,
            source_granularity,
            region_kind,
            container_kind: UnitContainerKind::Unknown,
            evidence_flags: UnitEvidenceFlags::empty(),
        }
    }

    pub const fn with_domain(self, domain: UnitDomain) -> Self {
        Self {
            domains: self.domains.with(domain),
            ..self
        }
    }

    pub const fn with_container(self, container_kind: UnitContainerKind) -> Self {
        Self {
            container_kind,
            ..self
        }
    }

    pub const fn with_evidence(self, flag: UnitEvidenceFlag) -> Self {
        Self {
            evidence_flags: self.evidence_flags.with(flag),
            ..self
        }
    }

    pub const fn has_domain(self, domain: UnitDomain) -> bool {
        self.domains.contains(domain)
    }

    pub const fn has_evidence(self, flag: UnitEvidenceFlag) -> bool {
        self.evidence_flags.contains(flag)
    }

    pub fn is_unknown(&self) -> bool {
        self.domains.is_empty()
            && self.subkind.is_unknown()
            && self.body_kind.is_unknown()
            && self.source_granularity.is_unknown()
            && self.region_kind.is_unknown()
            && self.container_kind.is_unknown()
            && self.evidence_flags.is_empty()
    }
}

impl Default for UnitOrigin {
    fn default() -> Self {
        Self::unknown()
    }
}
