use super::{ChangeKind, RegionRecord, RegionSnapshot};
use nose_il::ContentDigest;
use std::collections::{BTreeMap, BTreeSet};

type NameKey<'a> = (&'a str, &'a str, &'a str, &'a str);
type LocalKey<'a> = (ContentDigest, &'a str, Option<&'a str>, &'a str);
type Buckets<K, V> = BTreeMap<K, Vec<V>>;

pub(super) struct CandidateIndex<'a> {
    content: Buckets<ContentDigest, &'a RegionRecord>,
    local: Buckets<LocalKey<'a>, &'a RegionRecord>,
    names: Buckets<NameKey<'a>, &'a RegionRecord>,
    values: Buckets<ContentDigest, &'a RegionRecord>,
}

fn local_key(region: &RegionRecord) -> LocalKey<'_> {
    (
        region.content_key,
        &region.file,
        region.name.as_deref(),
        &region.kind,
    )
}

fn name_key(region: &RegionRecord) -> Option<NameKey<'_>> {
    Some((
        &region.file,
        &region.lang,
        &region.kind,
        region.name.as_deref()?,
    ))
}

impl<'a> CandidateIndex<'a> {
    pub(super) fn new(snapshot: &'a RegionSnapshot, reserved: &BTreeSet<ContentDigest>) -> Self {
        let mut index = Self {
            content: BTreeMap::new(),
            local: BTreeMap::new(),
            names: BTreeMap::new(),
            values: BTreeMap::new(),
        };
        for region in &snapshot.regions {
            if reserved.contains(&region.observation_id) {
                continue;
            }
            index
                .content
                .entry(region.content_key)
                .or_default()
                .push(region);
            index
                .local
                .entry(local_key(region))
                .or_default()
                .push(region);
            if let Some(key) = name_key(region) {
                index.names.entry(key).or_default().push(region);
            }
            if let Some(key) = region.value_key {
                index.values.entry(key).or_default().push(region);
            }
        }
        for bucket in index
            .content
            .values_mut()
            .chain(index.local.values_mut())
            .chain(index.names.values_mut())
            .chain(index.values.values_mut())
        {
            bucket.sort_by_key(|region| region.observation_id);
        }
        index
    }

    pub(super) fn candidates<'b>(
        &'b self,
        region: &'b RegionRecord,
    ) -> (&'b [&'a RegionRecord], ChangeKind) {
        if let Some(rows) = self
            .local
            .get(&local_key(region))
            .or_else(|| self.content.get(&region.content_key))
        {
            return (rows, ChangeKind::ContentMatch);
        }
        if let Some(rows) = name_key(region).and_then(|key| self.names.get(&key)) {
            return (rows, ChangeKind::ModifiedCandidate);
        }
        if let Some(rows) = region.value_key.and_then(|key| self.values.get(&key)) {
            return (rows, ChangeKind::ValueCandidate);
        }
        (&[], ChangeKind::Unresolved)
    }
}
