//! Research instrument for the oracle-completeness campaign: which IL constructs
//! keep units OUT of the interpreter oracle, and how much fingerprint-merge mass
//! is therefore unverified.
//!
//! `nose verify --exclusion-census <path>` records one [`CensusUnit`] per
//! counted function unit — its oracle outcome, its value fingerprint, and the
//! raw construct tags present in the subtree the oracle would interpret. The
//! report then derives, per construct tag, how many units carrying it were
//! excluded and how many fingerprint-equal pairs are unverified because at
//! least one side was excluded.
//!
//! The census deliberately records raw tags (node kinds, builtin names, literal
//! retention classes) for BOTH interpretable and excluded units rather than a
//! hard-coded "unsupported construct" list: the interpreter's handled set
//! drifts (experiments §BF), so the discriminating constructs are derived from
//! the two populations at analysis time instead of asserted here.

use anyhow::Result;
use nose_il::{Il, NodeId, NodeKind, Payload};
use std::collections::{BTreeMap, HashMap, HashSet};
use std::path::Path;

const CLAIMABLE_FAMILY_CAP: usize = 8;

/// One counted function unit's census record.
#[derive(serde::Serialize)]
pub(crate) struct CensusUnit {
    pub(crate) loc: String,
    /// Legacy human location used to join the ordinary `verify --json` rows.
    #[serde(skip)]
    pub(crate) verify_loc: String,
    pub(crate) language: &'static str,
    /// `"interpretable"`, or the exclusion reason: `"battery-bail"`,
    /// `"empty-fp"`, `"no-core-span"`.
    pub(crate) reason: &'static str,
    /// Value fingerprint (empty for `"empty-fp"` units).
    pub(crate) fp: Vec<u64>,
    /// Sorted construct tags present in the interpreted subtree.
    pub(crate) tags: Vec<String>,
    pub(crate) exact_safe: bool,
    pub(crate) claimable: bool,
    pub(crate) classification: &'static str,
    pub(crate) obligation_family: String,
    pub(crate) obligation_subreason: String,
    pub(crate) first_blocker: Option<nose_normalize::InterpreterBlocker>,
}

pub(crate) struct CensusOutcome {
    pub(crate) reason: &'static str,
    pub(crate) exact_safe: bool,
    pub(crate) claimable: bool,
    pub(crate) classification: &'static str,
    pub(crate) obligation_family: String,
    pub(crate) obligation_subreason: String,
    pub(crate) first_blocker: Option<nose_normalize::InterpreterBlocker>,
}

impl CensusUnit {
    fn excluded(&self) -> bool {
        self.reason != "interpretable"
    }
}

/// The construct tags present in `root`'s subtree, sorted and deduplicated.
///
/// Tag vocabulary: `kind:<NodeKind>` for every node kind, refined for the two
/// payload-sensitive kinds — calls become `builtin:<Builtin>` / `call:named` /
/// `call:cid` / `call:other`, and literals tag only their *unretained* classes
/// (`lit:unretained:<class>`), since retained literal values are interpretable.
pub(crate) fn census_tags(il: &Il, root: NodeId) -> Vec<String> {
    let mut tags: HashSet<String> = HashSet::new();
    let mut stack = vec![root];
    while let Some(x) = stack.pop() {
        let node = il.node(x);
        match node.kind {
            NodeKind::Call => {
                tags.insert(match node.payload {
                    Payload::Builtin(b) => format!("builtin:{b:?}"),
                    Payload::Name(_) => "call:named".to_string(),
                    Payload::Cid(_) => "call:cid".to_string(),
                    _ => "call:other".to_string(),
                });
            }
            NodeKind::Lit => match node.payload {
                Payload::LitInt(_) | Payload::LitBool(_) | Payload::LitStr(_) => {}
                Payload::Lit(c) => {
                    tags.insert(format!("lit:unretained:{c:?}"));
                }
                _ => {
                    tags.insert("lit:other".to_string());
                }
            },
            k => {
                tags.insert(format!("kind:{k:?}"));
            }
        }
        stack.extend(il.children(x).iter().copied());
    }
    let mut v: Vec<String> = tags.into_iter().collect();
    v.sort();
    v
}

#[derive(serde::Serialize)]
struct TagRow {
    tag: String,
    interpretable_units: usize,
    excluded_units: usize,
    /// Fingerprint-equal pairs with ≥1 excluded side, attributed to every tag
    /// present in an excluded member of the group (multi-attributed by design —
    /// a pair is unverified because of *all* the constructs that kept its
    /// members out, and the ranking question is "which construct, if covered,
    /// touches the most unverified mass").
    unverified_pairs: usize,
    example_excluded: Vec<String>,
}

#[derive(serde::Serialize)]
struct CensusReport {
    schema: &'static str,
    units_total: usize,
    interpretable_units: usize,
    excluded_by_reason: BTreeMap<String, usize>,
    /// Fingerprint-equal pair mass: `verified` pairs had both sides interpreted
    /// by the oracle; `unverified` pairs carry no behavioral check at all.
    merge_pairs: MergePairs,
    claimable_merge_pairs: MergePairs,
    generic_unattributed_exclusions: usize,
    claimable_family_cap: usize,
    priority_rows_multi_attributed: bool,
    priority: Vec<PriorityRow>,
    /// Per-construct rows, sorted by unverified mass (the campaign order).
    tags: Vec<TagRow>,
    units: Vec<CensusUnitRow>,
}

#[derive(serde::Serialize)]
struct MergePairs {
    total: usize,
    verified: usize,
    unverified: usize,
}

#[derive(Debug, Eq, PartialEq, serde::Serialize)]
struct PriorityRow {
    language: String,
    obligation_family: String,
    construct: String,
    blocker_category: &'static str,
    capability_id: &'static str,
    risk_tier: &'static str,
    risk_weight: usize,
    excluded_units: usize,
    claimable_pair_mass: usize,
    capped_claimable_pair_mass: usize,
    priority_score: usize,
    example_excluded: Vec<String>,
}

#[derive(serde::Serialize)]
struct CensusUnitRow {
    loc: String,
    language: &'static str,
    reason: &'static str,
    exact_safe: bool,
    claimable: bool,
    classification: &'static str,
    obligation_family: String,
    obligation_subreason: String,
    value_fingerprint: Vec<u64>,
    constructs: Vec<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    first_blocker: Option<nose_normalize::InterpreterBlocker>,
}

#[derive(Clone, Eq, Hash, PartialEq)]
struct PriorityKey {
    language: &'static str,
    obligation_family: String,
    construct: String,
    blocker_category: &'static str,
    capability_id: &'static str,
}

#[derive(Default)]
struct PriorityCount {
    excluded_units: usize,
    claimable_pair_mass: usize,
    capped_claimable_pair_mass: usize,
    examples: Vec<String>,
}

fn pairs(n: usize) -> usize {
    n * n.saturating_sub(1) / 2
}

fn priority_key(unit: &CensusUnit) -> Option<PriorityKey> {
    let blocker = unit.first_blocker.as_ref()?;
    Some(PriorityKey {
        language: unit.language,
        obligation_family: unit.obligation_family.clone(),
        construct: blocker
            .blocker_stack
            .first()
            .map_or_else(|| "kind:Func".to_string(), |frame| frame.construct.clone()),
        blocker_category: blocker.category,
        capability_id: blocker.capability_id,
    })
}

fn count_merge_pairs<'a>(units: impl Iterator<Item = &'a CensusUnit>) -> MergePairs {
    let mut by_fp: HashMap<&[u64], Vec<&CensusUnit>> = HashMap::new();
    for unit in units.filter(|unit| !unit.fp.is_empty()) {
        by_fp.entry(&unit.fp).or_default().push(unit);
    }
    let mut merge = MergePairs {
        total: 0,
        verified: 0,
        unverified: 0,
    };
    for group in by_fp.values().filter(|group| group.len() >= 2) {
        let interpreted = group.iter().filter(|unit| !unit.excluded()).count();
        let total = pairs(group.len());
        let verified = pairs(interpreted);
        merge.total += total;
        merge.verified += verified;
        merge.unverified += total - verified;
    }
    merge
}

fn build_priority(units: &[CensusUnit]) -> Vec<PriorityRow> {
    let mut counts: HashMap<PriorityKey, PriorityCount> = HashMap::new();
    for unit in units
        .iter()
        .filter(|unit| unit.excluded() && unit.claimable)
    {
        if let Some(key) = priority_key(unit) {
            let count = counts.entry(key).or_default();
            count.excluded_units += 1;
            count.examples.push(unit.loc.clone());
        }
    }

    let mut by_fp: HashMap<&[u64], Vec<&CensusUnit>> = HashMap::new();
    for unit in units
        .iter()
        .filter(|unit| unit.claimable && !unit.fp.is_empty())
    {
        by_fp.entry(&unit.fp).or_default().push(unit);
    }
    for group in by_fp.values().filter(|group| group.len() >= 2) {
        let total = pairs(group.len());
        let interpreted = group.iter().filter(|unit| !unit.excluded()).count();
        let unverified = total - pairs(interpreted);
        if unverified == 0 {
            continue;
        }
        let keys: HashSet<PriorityKey> = group
            .iter()
            .filter(|unit| unit.excluded())
            .filter_map(|unit| priority_key(unit))
            .collect();
        for key in keys {
            let count = counts.entry(key).or_default();
            count.claimable_pair_mass += unverified;
            count.capped_claimable_pair_mass += unverified.min(CLAIMABLE_FAMILY_CAP);
        }
    }

    let mut rows: Vec<_> = counts
        .into_iter()
        .map(|(key, mut count)| {
            count.examples.sort_unstable();
            count.examples.dedup();
            count.examples.truncate(3);
            let risk_weight = 3;
            PriorityRow {
                language: key.language.to_string(),
                obligation_family: key.obligation_family,
                construct: key.construct,
                blocker_category: key.blocker_category,
                capability_id: key.capability_id,
                risk_tier: "A",
                risk_weight,
                excluded_units: count.excluded_units,
                claimable_pair_mass: count.claimable_pair_mass,
                capped_claimable_pair_mass: count.capped_claimable_pair_mass,
                priority_score: risk_weight * count.capped_claimable_pair_mass,
                example_excluded: count.examples,
            }
        })
        .filter(|row| row.capped_claimable_pair_mass > 0)
        .collect();
    rows.sort_by(|left, right| {
        right
            .priority_score
            .cmp(&left.priority_score)
            .then(right.claimable_pair_mass.cmp(&left.claimable_pair_mass))
            .then(left.language.cmp(&right.language))
            .then(left.obligation_family.cmp(&right.obligation_family))
            .then(left.construct.cmp(&right.construct))
            .then(left.capability_id.cmp(right.capability_id))
    });
    rows
}

fn build_unit_rows(units: &[CensusUnit]) -> Vec<CensusUnitRow> {
    let mut rows: Vec<_> = units
        .iter()
        .map(|unit| CensusUnitRow {
            loc: unit.loc.clone(),
            language: unit.language,
            reason: unit.reason,
            exact_safe: unit.exact_safe,
            claimable: unit.claimable,
            classification: unit.classification,
            obligation_family: unit.obligation_family.clone(),
            obligation_subreason: unit.obligation_subreason.clone(),
            value_fingerprint: unit.fp.clone(),
            constructs: unit.tags.clone(),
            first_blocker: unit.first_blocker.clone(),
        })
        .collect();
    rows.sort_by(|left, right| {
        left.loc
            .cmp(&right.loc)
            .then(left.reason.cmp(right.reason))
            .then_with(|| {
                let capability = |row: &CensusUnitRow| {
                    row.first_blocker
                        .as_ref()
                        .map(|blocker| blocker.capability_id)
                };
                capability(left).cmp(&capability(right))
            })
    });
    rows
}

fn build_report(units: &[CensusUnit]) -> CensusReport {
    let mut excluded_by_reason: BTreeMap<String, usize> = BTreeMap::new();
    let mut tag_interp: HashMap<&str, usize> = HashMap::new();
    let mut tag_excl: HashMap<&str, usize> = HashMap::new();
    let mut tag_examples: HashMap<&str, Vec<&str>> = HashMap::new();
    for u in units {
        if u.excluded() {
            *excluded_by_reason.entry(u.reason.to_string()).or_default() += 1;
        }
        for t in &u.tags {
            if u.excluded() {
                *tag_excl.entry(t).or_default() += 1;
                tag_examples.entry(t).or_default().push(&u.loc);
            } else {
                *tag_interp.entry(t).or_default() += 1;
            }
        }
    }

    let mut by_fp: HashMap<&[u64], Vec<&CensusUnit>> = HashMap::new();
    for u in units {
        if !u.fp.is_empty() {
            by_fp.entry(&u.fp).or_default().push(u);
        }
    }
    let mut merge = MergePairs {
        total: 0,
        verified: 0,
        unverified: 0,
    };
    let mut tag_unverified: HashMap<&str, usize> = HashMap::new();
    for group in by_fp.values() {
        if group.len() < 2 {
            continue;
        }
        let interp = group.iter().filter(|u| !u.excluded()).count();
        let (total, verified) = (pairs(group.len()), pairs(interp));
        merge.total += total;
        merge.verified += verified;
        let unverified = total - verified;
        if unverified == 0 {
            continue;
        }
        merge.unverified += unverified;
        let mut group_tags: HashSet<&str> = HashSet::new();
        for u in group.iter().filter(|u| u.excluded()) {
            group_tags.extend(u.tags.iter().map(String::as_str));
        }
        for t in group_tags {
            *tag_unverified.entry(t).or_default() += unverified;
        }
    }

    let all_tags: HashSet<&str> = tag_interp.keys().chain(tag_excl.keys()).copied().collect();
    let mut rows: Vec<TagRow> = all_tags
        .into_iter()
        .map(|t| {
            let mut examples: Vec<&str> = tag_examples.get(t).cloned().unwrap_or_default();
            examples.sort_unstable();
            examples.truncate(3);
            TagRow {
                tag: t.to_string(),
                interpretable_units: tag_interp.get(t).copied().unwrap_or(0),
                excluded_units: tag_excl.get(t).copied().unwrap_or(0),
                unverified_pairs: tag_unverified.get(t).copied().unwrap_or(0),
                example_excluded: examples.into_iter().map(str::to_string).collect(),
            }
        })
        .collect();
    rows.sort_by(|a, b| {
        b.unverified_pairs
            .cmp(&a.unverified_pairs)
            .then(b.excluded_units.cmp(&a.excluded_units))
            .then(a.tag.cmp(&b.tag))
    });

    let claimable_merge_pairs = count_merge_pairs(units.iter().filter(|unit| unit.claimable));
    let generic_unattributed_exclusions = units
        .iter()
        .filter(|unit| {
            unit.excluded()
                && (unit.first_blocker.is_none()
                    || unit.obligation_family.is_empty()
                    || unit.obligation_subreason.is_empty())
        })
        .count();
    let priority = build_priority(units);
    CensusReport {
        schema: "nose-oracle-exclusion-census/v2",
        units_total: units.len(),
        interpretable_units: units.iter().filter(|u| !u.excluded()).count(),
        excluded_by_reason,
        merge_pairs: merge,
        claimable_merge_pairs,
        generic_unattributed_exclusions,
        claimable_family_cap: CLAIMABLE_FAMILY_CAP,
        priority_rows_multi_attributed: true,
        priority,
        tags: rows,
        units: build_unit_rows(units),
    }
}

/// Write the exclusion-census JSON report. Deterministic: every list is sorted
/// on stable keys, so the file is byte-identical across runs and thread counts.
pub(crate) fn write_report(path: &Path, units: &[CensusUnit]) -> Result<()> {
    let report = build_report(units);
    std::fs::write(path, serde_json::to_string_pretty(&report)?)?;
    Ok(())
}

#[cfg(test)]
mod tests;
