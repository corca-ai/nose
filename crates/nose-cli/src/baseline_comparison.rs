use crate::baseline;
use anyhow::Result;
use std::collections::{HashMap, HashSet};

#[derive(serde::Serialize)]
pub(super) struct BaselineSummary {
    pub(super) path: String,
    pub(super) mode: &'static str,
    pub(super) baseline_families: usize,
    pub(super) new_families: usize,
    pub(super) changed_families: usize,
    pub(super) unchanged_families: usize,
    pub(super) resolved_families: usize,
}

impl BaselineSummary {
    pub(super) fn line(&self) -> String {
        format!(
            "baseline: {} new · {} changed · {} unchanged · {} resolved",
            self.new_families,
            self.changed_families,
            self.unchanged_families,
            self.resolved_families
        )
    }
}

#[derive(Clone, Copy)]
pub(super) enum BaselineStatus {
    New,
    Changed,
}

impl BaselineStatus {
    pub(super) fn as_str(&self) -> &'static str {
        match self {
            BaselineStatus::New => "new",
            BaselineStatus::Changed => "changed",
        }
    }
}

#[derive(Clone, Copy)]
pub(super) enum BaselineMatch {
    None,
    PartialMembers,
    MemberLocations,
}

impl BaselineMatch {
    pub(super) fn as_str(&self) -> &'static str {
        match self {
            BaselineMatch::None => "none",
            BaselineMatch::PartialMembers => "partial-members",
            BaselineMatch::MemberLocations => "member-locations",
        }
    }
}

#[derive(Clone)]
pub(super) struct BaselineFamilyStatus {
    pub(super) status: BaselineStatus,
    pub(super) baseline_match: BaselineMatch,
    pub(super) matched_baseline_ids: Vec<u64>,
    pub(super) accepted_member_count: usize,
    pub(super) new_member_count: usize,
}

pub(super) struct BaselineComparison {
    pub(super) summary: BaselineSummary,
    pub(super) statuses: HashMap<u64, BaselineFamilyStatus>,
    pub(super) suppressed_keys: HashSet<u64>,
}

struct BaselineIndexes {
    accepted_by_member: HashMap<baseline::AcceptedMember, HashSet<u64>>,
    accepted_by_location: HashMap<baseline::MemberKey, HashSet<u64>>,
}

impl BaselineIndexes {
    fn build(entries: &[baseline::BaselineEntry]) -> Self {
        let mut accepted_by_member = HashMap::new();
        let mut accepted_by_location = HashMap::new();
        for entry in entries {
            for member in &entry.members {
                accepted_by_member
                    .entry(member.clone())
                    .or_insert_with(HashSet::new)
                    .insert(entry.key);
                accepted_by_location
                    .entry(member.key.clone())
                    .or_insert_with(HashSet::new)
                    .insert(entry.key);
            }
        }
        Self {
            accepted_by_member,
            accepted_by_location,
        }
    }
}

impl BaselineComparison {
    pub(super) fn new(
        path: &std::path::Path,
        baseline: &baseline::Baseline,
        families: &[nose_detect::RefactorFamily],
    ) -> Result<Self> {
        let indexes = BaselineIndexes::build(&baseline.entries);
        let mut statuses = HashMap::new();
        let mut suppressed_keys = HashSet::new();
        let mut matched_baseline = HashSet::new();

        for family in families {
            let key = baseline::family_key(family);
            let members = baseline::accepted_members(family)?;
            let member_count = members.len();

            if baseline
                .entries_by_key
                .get(&key)
                .into_iter()
                .flatten()
                .any(|entry_index| same_accepted_members(&baseline.entries[*entry_index], &members))
            {
                suppressed_keys.insert(key);
                matched_baseline.insert(key);
                continue;
            }

            let (accepted_member_count, exact_member_matches) =
                accepted_member_matches(&indexes, &members);
            if accepted_member_count == member_count && member_count > 0 {
                suppressed_keys.insert(key);
                matched_baseline.extend(exact_member_matches);
                continue;
            }

            let location_matches = location_matches(&indexes, &members);
            let status = family_status(
                accepted_member_count,
                member_count,
                &exact_member_matches,
                &location_matches,
            );
            matched_baseline.extend(status.matched_baseline_ids.iter().copied());
            statuses.insert(key, status);
        }

        let resolved_families = baseline
            .keys
            .iter()
            .filter(|key| !matched_baseline.contains(key))
            .count();
        let changed_families = statuses
            .values()
            .filter(|status| matches!(status.status, BaselineStatus::Changed))
            .count();
        let new_families = statuses.len().saturating_sub(changed_families);
        let unchanged_families = suppressed_keys.len();
        Ok(BaselineComparison {
            summary: BaselineSummary {
                path: path.display().to_string(),
                mode: "new-only",
                baseline_families: baseline.entries.len(),
                new_families,
                changed_families,
                unchanged_families,
                resolved_families,
            },
            statuses,
            suppressed_keys,
        })
    }
}

fn accepted_member_matches(
    indexes: &BaselineIndexes,
    members: &[baseline::AcceptedMember],
) -> (usize, HashSet<u64>) {
    let mut matches = HashSet::new();
    let mut count = 0usize;
    for member in members {
        if let Some(ids) = indexes.accepted_by_member.get(member) {
            count += 1;
            matches.extend(ids.iter().copied());
        }
    }
    (count, matches)
}

fn location_matches(
    indexes: &BaselineIndexes,
    members: &[baseline::AcceptedMember],
) -> HashSet<u64> {
    let mut matches = HashSet::new();
    for member in members {
        if let Some(ids) = indexes.accepted_by_location.get(&member.key) {
            matches.extend(ids.iter().copied());
        }
    }
    matches
}

fn family_status(
    accepted_member_count: usize,
    member_count: usize,
    exact_member_matches: &HashSet<u64>,
    location_matches: &HashSet<u64>,
) -> BaselineFamilyStatus {
    if accepted_member_count > 0 || !location_matches.is_empty() {
        let mut matched_baseline_ids: Vec<u64> = exact_member_matches
            .union(location_matches)
            .copied()
            .collect();
        matched_baseline_ids.sort_unstable();
        return BaselineFamilyStatus {
            status: BaselineStatus::Changed,
            baseline_match: if accepted_member_count > 0 {
                BaselineMatch::PartialMembers
            } else {
                BaselineMatch::MemberLocations
            },
            matched_baseline_ids,
            accepted_member_count,
            new_member_count: member_count.saturating_sub(accepted_member_count),
        };
    }
    BaselineFamilyStatus {
        status: BaselineStatus::New,
        baseline_match: BaselineMatch::None,
        matched_baseline_ids: Vec::new(),
        accepted_member_count: 0,
        new_member_count: member_count,
    }
}

fn same_accepted_members(
    entry: &baseline::BaselineEntry,
    members: &[baseline::AcceptedMember],
) -> bool {
    if entry.members.len() != members.len() {
        return false;
    }
    let accepted: HashSet<&baseline::AcceptedMember> = entry.members.iter().collect();
    members.iter().all(|member| accepted.contains(member))
}
