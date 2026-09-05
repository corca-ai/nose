use super::{digest, region_key};
use crate::UnitFeat;
use nose_il::{ContentDigest, SourceRegion};
use serde::{Deserialize, Serialize};
use std::collections::BTreeSet;

pub(super) const SNAPSHOT_SCHEMA: &str = "nose.regions/v1";

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RegionRecord {
    pub observation_id: ContentDigest,
    pub file: String,
    pub lang: String,
    pub kind: String,
    pub name: Option<String>,
    pub in_test: bool,
    pub source: SourceRegion,
    pub content_key: ContentDigest,
    pub analysis_key: ContentDigest,
    pub value_key: Option<ContentDigest>,
}

impl RegionRecord {
    fn address(&self) -> ContentDigest {
        digest(
            b"nose.region-observation/v1",
            &(
                &self.file,
                &self.lang,
                &self.kind,
                &self.source,
                self.content_key,
            ),
        )
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RegionSnapshot {
    pub schema: String,
    /// Explicit extraction/analysis profile; different profiles can be compared
    /// for source evidence but cannot reuse analysis-dependent decisions.
    pub profile: String,
    pub regions: Vec<RegionRecord>,
    pub unavailable_regions: usize,
}

impl RegionSnapshot {
    /// Includes singleton units as well as clones. Callers supply logical paths
    /// before capture so a checkout directory is never an identity namespace.
    pub fn from_units(units: &[UnitFeat], profile: String) -> Self {
        let mut snapshot = Self {
            schema: SNAPSHOT_SCHEMA.into(),
            profile,
            regions: Vec::new(),
            unavailable_regions: 0,
        };
        for unit in units {
            let loc = crate::locations::loc_of(unit, None);
            let (Some(source), Some(content_key)) = (loc.source_region.clone(), region_key(&loc))
            else {
                snapshot.unavailable_regions += 1;
                continue;
            };
            let unit_kind = match unit.kind {
                nose_il::UnitKind::Function => "function",
                nose_il::UnitKind::Method => "method",
                nose_il::UnitKind::Class => "class",
                nose_il::UnitKind::Block => "block",
            };
            let kind = format!(
                "{unit_kind}/{}",
                unit.fragment_kind.map_or("unit", |k| k.reason_code())
            );
            let mut record = RegionRecord {
                observation_id: content_key,
                file: unit.path.clone(),
                lang: unit.lang.name().into(),
                kind,
                name: unit.name.clone(),
                in_test: crate::is_test_loc(&loc),
                source,
                content_key,
                analysis_key: super::unit_analysis_key(unit),
                value_key: (unit.exact_safe && !unit.value.is_empty())
                    .then(|| digest(b"nose.region-value/v1", &unit.value)),
            };
            record.observation_id = record.address();
            snapshot.regions.push(record);
        }
        snapshot.regions.sort_by_key(|record| record.observation_id);
        snapshot
            .regions
            .dedup_by_key(|record| record.observation_id);
        snapshot
    }

    pub fn validate(&self) -> Result<(), String> {
        if self.schema != SNAPSHOT_SCHEMA || self.profile.is_empty() {
            return Err("unsupported region snapshot schema or empty profile".into());
        }
        let mut ids = BTreeSet::new();
        for region in &self.regions {
            if region.file.is_empty()
                || region.lang.is_empty()
                || region.kind.is_empty()
                || region.source.start_byte >= region.source.end_byte
                || region.address() != region.observation_id
                || !ids.insert(region.observation_id)
            {
                return Err("invalid or duplicate region observation".into());
            }
        }
        Ok(())
    }
}
