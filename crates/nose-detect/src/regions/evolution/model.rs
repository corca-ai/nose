use super::super::{digest, member_review_key, region_key, review_evidence, review_key};
use crate::{Loc, RefactorFamily};
use nose_il::{ContentDigest, SourceRegion};
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct MemberObservation {
    pub file: String,
    pub lang: String,
    pub kind: String,
    pub name: Option<String>,
    pub start_line: u32,
    pub end_line: u32,
    pub in_test: bool,
    pub source: Option<SourceRegion>,
    pub content_key: Option<ContentDigest>,
    pub analysis_key: Option<ContentDigest>,
    pub review_key: Option<ContentDigest>,
}

impl MemberObservation {
    fn capture(loc: &Loc) -> Self {
        Self {
            file: loc.file.clone(),
            lang: loc.lang.clone(),
            kind: format!("{:?}/{:?}/{:?}", loc.kind, loc.origin, loc.fragment_kind),
            name: loc.name.clone(),
            start_line: loc.start_line,
            end_line: loc.end_line,
            in_test: crate::is_test_loc(loc),
            source: loc.source_region.clone(),
            content_key: region_key(loc),
            analysis_key: loc.analysis_digest,
            review_key: member_review_key(loc),
        }
    }
    pub(super) fn region(&self) -> Option<super::super::RegionRecord> {
        let mut r = super::super::RegionRecord {
            observation_id: self.content_key?,
            file: self.file.clone(),
            lang: self.lang.clone(),
            kind: self.kind.clone(),
            name: self.name.clone(),
            in_test: self.in_test,
            source: self.source.clone()?,
            content_key: self.content_key?,
            analysis_key: self
                .analysis_key
                .unwrap_or_else(|| digest(b"nose.no-analysis/v1", &())),
            value_key: None,
        };
        r.observation_id = r.address();
        Some(r)
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct FamilyObservation {
    pub id: ContentDigest,
    pub review_key: Option<ContentDigest>,
    pub scope: String,
    pub witness: String,
    pub value_nodes: Option<usize>,
    pub members: Vec<MemberObservation>,
    /// Independent projections identify changed evidence categories. Opaque
    /// analysis internals are not reconstructed from their digests.
    pub evidence: BTreeMap<String, ContentDigest>,
    pub pack_rows: Vec<String>,
    pub laws: Vec<nose_semantics::ValueLawProvenance>,
    pub near_provenance: Vec<nose_semantics::SemanticPackNearProvenance>,
    pub exact_provenance: Vec<nose_semantics::SemanticPackExternalExactProvenance>,
    pub abstraction_template: Vec<String>,
}

impl FamilyObservation {
    pub fn capture(family: &RefactorFamily) -> Self {
        let mut members: Vec<_> = family
            .locations
            .iter()
            .map(MemberObservation::capture)
            .collect();
        members.sort_by_key(|m| digest(b"nose.family-member-observation/v1", m));
        let mut packs: Vec<_> = family
            .locations
            .iter()
            .map(review_evidence::pack_keys)
            .collect();
        packs.sort();
        let mut analyses: Vec<_> = members.iter().map(|m| m.analysis_key).collect();
        analyses.sort();
        let mut laws = family.semantic_laws.clone();
        laws.sort();
        let mut pack_rows: Vec<_> = family
            .semantic_pack_near
            .iter()
            .map(|p| format!("{}:{} (near)", p.pack_id, p.row_id))
            .chain(
                family
                    .semantic_pack_external_exact
                    .iter()
                    .map(|p| format!("{}:{} (external-claim-exact)", p.pack_id, p.row_id)),
            )
            .collect();
        pack_rows.sort();
        pack_rows.dedup();
        let mut near_provenance = family.semantic_pack_near.clone();
        near_provenance.sort();
        let mut exact_provenance = family.semantic_pack_external_exact.clone();
        exact_provenance.sort();
        let mut row = Self {
            id: digest(b"nose.empty/v1", &()),
            review_key: review_key(family),
            scope: family.scope.into(),
            witness: family
                .witness
                .as_ref()
                .map_or("unavailable", |w| w.kind())
                .into(),
            value_nodes: family.witness.as_ref().and_then(|w| w.value_nodes()),
            members,
            evidence: BTreeMap::from([
                (
                    "analysis".into(),
                    digest(b"nose.family-analysis/v1", &analyses),
                ),
                ("packs".into(), digest(b"nose.family-packs/v1", &packs)),
                ("laws".into(), digest(b"nose.family-laws/v1", &laws)),
                (
                    "abstraction".into(),
                    digest(
                        b"nose.family-abstraction/v1",
                        &family
                            .abstraction_witness
                            .as_ref()
                            .map(review_evidence::abstraction_key),
                    ),
                ),
            ]),
            pack_rows,
            laws,
            near_provenance,
            exact_provenance,
            abstraction_template: family
                .abstraction_witness
                .as_ref()
                .map(|w| w.template.clone())
                .unwrap_or_default(),
        };
        row.id = row.address();
        row
    }
    pub(super) fn address(&self) -> ContentDigest {
        digest(
            b"nose.family-observation/v1",
            &(
                self.review_key,
                &self.scope,
                &self.witness,
                self.value_nodes,
                &self.members,
                &self.evidence,
                &self.pack_rows,
                &self.laws,
                &self.near_provenance,
                &self.exact_provenance,
                &self.abstraction_template,
            ),
        )
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct AnalysisSnapshot {
    pub schema: String,
    pub profile: BTreeMap<String, String>,
    pub roots: Vec<String>,
    pub path_base: String,
    pub scanned_files: usize,
    pub skipped_sources: usize,
    pub population: String,
    pub complete: bool,
    pub families: Vec<FamilyObservation>,
}

impl AnalysisSnapshot {
    pub fn validate(&self) -> Result<(), String> {
        if self.schema != "nose.analysis/v1"
            || self.population != "admitted-query-families"
            || self.profile.is_empty()
            || self.roots.is_empty()
            || self.path_base.is_empty()
        {
            return Err("expected nose.analysis/v1 capture; query/baseline/region JSON is not a complete family analysis".into());
        }
        let mut ids = BTreeSet::new();
        for f in &self.families {
            if f.id != f.address()
                || !ids.insert(f.id)
                || f.members.is_empty()
                || f.members.iter().any(|m| {
                    m.file.is_empty()
                        || m.lang.is_empty()
                        || m.start_line == 0
                        || m.start_line > m.end_line
                        || m.source
                            .as_ref()
                            .is_some_and(|s| s.start_byte >= s.end_byte)
                })
            {
                return Err("invalid or duplicate family observation".into());
            }
        }
        Ok(())
    }
    pub(super) fn regions(&self) -> super::super::RegionSnapshot {
        let mut rows = BTreeMap::new();
        let mut unavailable = self.skipped_sources;
        for member in self.families.iter().flat_map(|f| &f.members) {
            if let Some(r) = member.region() {
                rows.insert(r.observation_id, r);
            } else {
                unavailable += 1;
            }
        }
        super::super::RegionSnapshot {
            schema: "nose.regions/v1".into(),
            profile: digest(b"nose.analysis-profile/v1", &self.profile).hex(),
            regions: rows.into_values().collect(),
            unavailable_regions: unavailable,
        }
    }
}
