use super::{RegionRecord, RegionSnapshot};
use nose_il::ContentDigest;
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum ChangeKind {
    Unchanged,
    ContentMatch,
    ModifiedCandidate,
    ValueCandidate,
    CopiedCandidate,
    Ambiguous,
    Unresolved,
    UnmatchedCurrent,
    BudgetExceeded,
}

#[derive(Clone, Debug, Serialize)]
pub struct Correspondence {
    pub before: Option<ContentDigest>,
    pub after: Vec<ContentDigest>,
    pub kind: ChangeKind,
    /// Source, admitted analysis and test scope are unchanged under the same
    /// profile. This is evidence for a caller's policy, never an approval.
    pub unchanged_evidence: bool,
}

#[derive(Debug, Serialize)]
pub struct Reconciliation {
    pub schema: &'static str,
    pub profile_matches: bool,
    pub complete: bool,
    pub candidates_examined: usize,
    pub correspondences: Vec<Correspondence>,
}

/// Deterministic, indexed correspondence under an explicit global candidate
/// budget. Multiple candidates and competing claims are never broken by order.
/// Missing findings are unresolved, since an extraction census is not history.
pub fn reconcile(
    before: &RegionSnapshot,
    after: &RegionSnapshot,
    max_candidates: usize,
) -> Result<Reconciliation, String> {
    before.validate()?;
    after.validate()?;

    let addresses: BTreeMap<_, _> = after
        .regions
        .iter()
        .map(|r| (r.observation_id, r))
        .collect();
    let reserved: BTreeSet<_> = before
        .regions
        .iter()
        .map(|r| r.observation_id)
        .filter(|id| addresses.contains_key(id))
        .collect();
    let index = super::candidate_index::CandidateIndex::new(after, &reserved);
    let profile_matches = before.profile == after.profile;
    let mut result = Reconciliation {
        schema: "nose.region-correspondence/v1",
        profile_matches,
        complete: before.unavailable_regions == 0 && after.unavailable_regions == 0,
        candidates_examined: 0,
        correspondences: Vec::new(),
    };
    let mut old: Vec<_> = before.regions.iter().collect();
    old.sort_by_key(|r| r.observation_id);
    for region in old {
        let row = if let Some(current) = addresses.get(&region.observation_id) {
            correspondence(region, &[current], ChangeKind::Unchanged, profile_matches)
        } else {
            let (candidates, kind) = index.candidates(region);
            if candidates.len() > max_candidates.saturating_sub(result.candidates_examined) {
                result.complete = false;
                correspondence(region, &[], ChangeKind::BudgetExceeded, false)
            } else {
                result.candidates_examined += candidates.len();
                match_candidates(region, candidates, kind, profile_matches)
            }
        };
        result.correspondences.push(row);
    }
    reject_competing_claims(&mut result.correspondences);
    append_unmatched(before, after, &mut result.correspondences);
    if !result.complete {
        for row in &mut result.correspondences {
            row.unchanged_evidence = false;
        }
    }
    Ok(result)
}

fn append_unmatched(
    before: &RegionSnapshot,
    after: &RegionSnapshot,
    rows: &mut Vec<Correspondence>,
) {
    let referenced: BTreeSet<_> = rows
        .iter()
        .flat_map(|row| row.after.iter().copied())
        .collect();
    let retained: BTreeSet<_> = rows
        .iter()
        .filter(|row| matches!(row.kind, ChangeKind::Unchanged | ChangeKind::ContentMatch))
        .filter_map(|row| row.before)
        .collect();
    let retained_content: BTreeSet<_> = before
        .regions
        .iter()
        .filter(|region| retained.contains(&region.observation_id))
        .map(|region| region.content_key)
        .collect();
    let mut current: Vec<_> = after.regions.iter().collect();
    current.sort_by_key(|r| r.observation_id);
    for region in current {
        if referenced.contains(&region.observation_id) {
            continue;
        }
        let kind = if retained_content.contains(&region.content_key) {
            ChangeKind::CopiedCandidate
        } else {
            ChangeKind::UnmatchedCurrent
        };
        rows.push(Correspondence {
            before: None,
            after: vec![region.observation_id],
            kind,
            unchanged_evidence: false,
        });
    }
}

fn match_candidates(
    region: &RegionRecord,
    candidates: &[&RegionRecord],
    kind: ChangeKind,
    profile: bool,
) -> Correspondence {
    if candidates.is_empty() {
        return correspondence(region, &[], ChangeKind::Unresolved, false);
    }
    let local: Vec<_> = candidates
        .iter()
        .copied()
        .filter(|r| r.file == region.file && r.name == region.name && r.kind == region.kind)
        .collect();
    let candidates = if local.is_empty() { candidates } else { &local };
    let kind = if candidates.len() == 1 {
        kind
    } else {
        ChangeKind::Ambiguous
    };
    correspondence(region, candidates, kind, profile)
}

fn correspondence(
    before: &RegionRecord,
    after: &[&RegionRecord],
    kind: ChangeKind,
    profile: bool,
) -> Correspondence {
    let unchanged_evidence = profile
        && matches!(kind, ChangeKind::Unchanged | ChangeKind::ContentMatch)
        && after.len() == 1
        && before.analysis_key == after[0].analysis_key
        && before.content_key == after[0].content_key
        && before.in_test == after[0].in_test;
    Correspondence {
        before: Some(before.observation_id),
        after: after.iter().map(|r| r.observation_id).collect(),
        kind,
        unchanged_evidence,
    }
}

fn reject_competing_claims(rows: &mut [Correspondence]) {
    let mut claims: BTreeMap<ContentDigest, usize> = BTreeMap::new();
    for row in rows.iter() {
        for id in &row.after {
            *claims.entry(*id).or_default() += 1;
        }
    }
    for row in rows {
        if row.after.iter().any(|id| claims[id] > 1) {
            row.kind = ChangeKind::Ambiguous;
            row.unchanged_evidence = false;
        }
    }
}
