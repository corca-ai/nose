use super::{AnalysisSnapshot, FamilyObservation};
use crate::regions::{digest, reconcile, ChangeKind};
use nose_il::ContentDigest;
use serde::Serialize;
use std::collections::{BTreeMap, BTreeSet};

#[derive(Debug, Serialize)]
pub struct Change {
    pub id: ContentDigest,
    pub before: Option<ContentDigest>,
    pub after: Vec<ContentDigest>,
    pub correspondence: String,
    pub reasons: Vec<String>,
    pub unchanged_evidence: bool,
}

#[derive(Debug, Serialize)]
pub struct Comparison {
    pub schema: &'static str,
    pub profile_matches: bool,
    pub complete: bool,
    pub candidates_examined: usize,
    pub changes: Vec<Change>,
}

struct Mapping {
    after: BTreeSet<ContentDigest>,
    exact: bool,
    capped: bool,
    ambiguous: bool,
}

/// Compare complete captured populations before applying any display selection.
/// Equal content proposes correspondence; it never proves ancestry or approval.
pub fn compare(
    before: &AnalysisSnapshot,
    after: &AnalysisSnapshot,
    budget: usize,
) -> Result<Comparison, String> {
    before.validate()?;
    after.validate()?;
    let regions = reconcile(&before.regions(), &after.regions(), budget)?;
    let mut result = Comparison {
        schema: "nose.analysis-changes/v1",
        profile_matches: before.profile == after.profile,
        complete: before.complete && after.complete && regions.complete,
        candidates_examined: regions.candidates_examined,
        changes: Vec::new(),
    };
    let mapping: BTreeMap<_, _> = regions
        .correspondences
        .iter()
        .filter_map(|r| {
            r.before.map(|id| {
                (
                    id,
                    Mapping {
                        after: r.after.iter().copied().collect(),
                        exact: matches!(r.kind, ChangeKind::Unchanged | ChangeKind::ContentMatch),
                        capped: r.kind == ChangeKind::BudgetExceeded,
                        ambiguous: r.kind == ChangeKind::Ambiguous,
                    },
                )
            })
        })
        .collect();
    let index = member_index(&after.families);
    let current: BTreeMap<_, _> = after.families.iter().map(|f| (f.id, f)).collect();
    let mut old: Vec<_> = before.families.iter().collect();
    old.sort_by_key(|f| f.id);
    for family in old {
        let (candidates, exact, capped, ambiguous) = candidates(
            family,
            &mapping,
            &index,
            &current,
            budget,
            &mut result.candidates_examined,
        );
        result.complete &= !capped;
        let kind = if capped {
            "budget-exceeded"
        } else {
            match candidates.len() {
                0 => "unresolved",
                1 if ambiguous => "ambiguous",
                1 if exact => "matched",
                1 => "candidate",
                _ => "ambiguous",
            }
        };
        result
            .changes
            .push(change(Some(family.id), candidates, kind));
    }
    reject_competitors(&mut result.changes);
    let referenced: BTreeSet<_> = result
        .changes
        .iter()
        .flat_map(|r| &r.after)
        .copied()
        .collect();
    for family in current.values() {
        if !referenced.contains(&family.id) {
            result
                .changes
                .push(change(None, vec![family.id], "unmatched-current"));
        }
    }
    let old: BTreeMap<_, _> = before.families.iter().map(|f| (f.id, f)).collect();
    for row in &mut result.changes {
        explain(row, &old, &current, result.profile_matches, result.complete);
    }
    result.changes.sort_by_key(|r| r.id);
    Ok(result)
}

fn member_ids(f: &FamilyObservation) -> Vec<ContentDigest> {
    let mut ids: Vec<_> = f
        .members
        .iter()
        .filter_map(|m| m.region().map(|r| r.observation_id))
        .collect();
    ids.sort();
    ids
}

fn member_index(
    families: &[FamilyObservation],
) -> BTreeMap<ContentDigest, BTreeSet<ContentDigest>> {
    let mut index: BTreeMap<_, BTreeSet<_>> = BTreeMap::new();
    for f in families {
        for id in member_ids(f) {
            index.entry(id).or_default().insert(f.id);
        }
    }
    index
}

fn candidates(
    f: &FamilyObservation,
    mapping: &BTreeMap<ContentDigest, Mapping>,
    index: &BTreeMap<ContentDigest, BTreeSet<ContentDigest>>,
    current: &BTreeMap<ContentDigest, &FamilyObservation>,
    budget: usize,
    examined: &mut usize,
) -> (Vec<ContentDigest>, bool, bool, bool) {
    let ids = member_ids(f);
    let mut mapped = Vec::new();
    let mut ambiguous = false;
    let mut exact = ids.len() == f.members.len();
    for id in ids {
        let Some(m) = mapping.get(&id) else {
            exact = false;
            continue;
        };
        if m.capped {
            return (Vec::new(), false, true, false);
        }
        ambiguous |= m.ambiguous;
        exact &= m.exact && m.after.len() == 1;
        mapped.extend(m.after.iter().copied());
    }
    mapped.sort();
    let mut candidates = BTreeSet::new();
    for id in &mapped {
        if let Some(bucket) = index.get(id) {
            if bucket.len() > budget.saturating_sub(*examined) {
                return (Vec::new(), false, true, false);
            }
            *examined += bucket.len();
            candidates.extend(bucket.iter().copied());
        }
    }
    let full: Vec<_> = candidates
        .iter()
        .copied()
        .filter(|id| member_ids(current[id]) == mapped)
        .collect();
    if !full.is_empty() {
        (full, exact, false, ambiguous)
    } else {
        (candidates.into_iter().collect(), false, false, ambiguous)
    }
}

fn change(before: Option<ContentDigest>, after: Vec<ContentDigest>, kind: &str) -> Change {
    Change {
        id: digest(b"nose.analysis-change/v1", &(before, &after)),
        before,
        after,
        correspondence: kind.into(),
        reasons: Vec::new(),
        unchanged_evidence: false,
    }
}

fn reject_competitors(rows: &mut [Change]) {
    let mut claims = BTreeMap::new();
    for row in rows.iter() {
        for id in &row.after {
            *claims.entry(*id).or_insert(0usize) += 1;
        }
    }
    for row in rows {
        if row.after.iter().any(|id| claims[id] > 1) {
            row.correspondence = "ambiguous".into();
        }
    }
}

fn explain(
    row: &mut Change,
    old: &BTreeMap<ContentDigest, &FamilyObservation>,
    current: &BTreeMap<ContentDigest, &FamilyObservation>,
    profiles: bool,
    complete: bool,
) {
    let mut reasons = BTreeSet::new();
    if !profiles {
        reasons.insert("profile-changed".to_string());
    }
    if !complete {
        reasons.insert("incomplete-coverage".to_string());
    }
    if let (Some(id), [next]) = (row.before, row.after.as_slice()) {
        let a = old[&id];
        let b = current[next];
        if a.members.len() != b.members.len() {
            reasons.insert("membership-changed".into());
        }
        let contents = |f: &FamilyObservation| {
            let mut v: Vec<_> = f.members.iter().map(|m| m.content_key).collect();
            v.sort();
            v
        };
        if contents(a) != contents(b) {
            reasons.insert("member-content-changed".into());
        }
        if member_ids(a) != member_ids(b) {
            reasons.insert("source-address-changed".into());
        }
        if a.scope != b.scope || member_scopes(a) != member_scopes(b) {
            reasons.insert("scope-changed".into());
        }
        if a.witness != b.witness || a.value_nodes != b.value_nodes {
            reasons.insert("witness-changed".into());
        }
        for (key, value) in &a.evidence {
            if b.evidence.get(key) != Some(value) {
                reasons.insert(format!("{key}-changed"));
            }
        }
        if a.review_key != b.review_key {
            reasons.insert("review-evidence-changed".into());
        }
        if a.review_key.is_none() || b.review_key.is_none() {
            reasons.insert("evidence-unavailable".into());
        }
        row.unchanged_evidence = row.correspondence == "matched"
            && complete
            && profiles
            && a.review_key.is_some()
            && a.review_key == b.review_key
            && a.scope == b.scope
            && member_scopes(a) == member_scopes(b);
        if row.unchanged_evidence {
            reasons.insert("review-evidence-retained".into());
        }
    }
    if row.correspondence != "matched" {
        reasons.insert(row.correspondence.clone());
    }
    row.reasons = reasons.into_iter().collect();
}

fn member_scopes(family: &FamilyObservation) -> Vec<(Option<ContentDigest>, bool)> {
    let mut scopes: Vec<_> = family
        .members
        .iter()
        .map(|m| (m.content_key, m.in_test))
        .collect();
    scopes.sort();
    scopes
}
