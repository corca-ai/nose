use crate::baseline;
use rustc_hash::{FxHashMap, FxHashSet};
use std::cmp::Ordering;

mod accepted_coverage;
mod origin_hints;

use accepted_coverage::{
    accepted_edges_covered_by_roots, accepted_obligations_covered, direct_suppression_forest,
    opportunity_root,
};
pub(crate) use origin_hints::hint_reasons;
use origin_hints::origin_extract_hint;

pub(crate) fn total_dup_lines_refs(fs: &[&nose_detect::RefactorFamily]) -> u32 {
    fs.iter().map(|f| f.dup_lines).sum()
}

/// Overlap grouping (issues #263/#264): families whose members are
/// overlapping slices of the same source regions are one refactoring
/// *opportunity*, not several. The primary (best-ranked) family keeps its
/// numbered entry; its slices fold into a one-line note under it and carry
/// suppression navigation in JSON. Grouping is presentation policy: a folded
/// family remains addressable with `id=`, while bulk lists and gates emit the
/// visible roots.
#[derive(Default)]
pub(crate) struct OpportunityGroups {
    /// Slice family id → its direct suppressor's family id. Syntax-only chains
    /// may point through another slice; accepted evidence points to a covering
    /// visible root.
    pub(crate) primary_of: FxHashMap<String, String>,
    /// Direct suppressor family id → slice family ids, in rank order.
    pub(crate) slices_of: FxHashMap<String, Vec<String>>,
    /// Slice ids folded in the default surface. The all-surface fold forest
    /// remains stable when provenance moves a primary to another surface, but
    /// a default slice must stay visible when its direct suppressor is no
    /// longer present in the default view.
    default_slices: FxHashSet<String>,
}

impl OpportunityGroups {
    /// Group `families`, which arrive in rank order. Two families overlap as slices when at least two
    /// distinct member pairs overlap by half of the shorter span. A fold is
    /// allowed only when an earlier, still-visible primary also covers every
    /// structural accepted-family obligation carried by the slice. Syntax-only
    /// windows have no such obligation and retain the established folding
    /// policy. Requiring direct obligation coverage prevents partial overlap or
    /// an A-B-C bridge from hiding accepted endpoints the primary does not
    /// represent.
    #[cfg(test)]
    pub(crate) fn from_ranked(families: &[&nose_detect::RefactorFamily]) -> Self {
        Self::from_ranked_with_default(families, |_| true)
    }

    /// Build the stable all-surface fold forest and, while the direct endpoints are already
    /// known, decide which edges may also fold on the default surface. Computing that decision
    /// here avoids a second full-family identity pass after the graph has been built (#892).
    pub(crate) fn from_ranked_with_default(
        families: &[&nose_detect::RefactorFamily],
        is_default: impl Fn(&nose_detect::RefactorFamily) -> bool,
    ) -> Self {
        // A file listing implausibly many families would make candidate
        // generation quadratic; skip it rather than risk query speed.
        const PER_FILE_CAP: usize = 200;
        let mut by_file: FxHashMap<&str, FileOpportunityBucket> = FxHashMap::default();
        let mut family_files: Vec<Vec<&str>> = Vec::with_capacity(families.len());
        for (i, f) in families.iter().enumerate() {
            let mut files: Vec<&str> = f.locations.iter().map(|l| l.file.as_str()).collect();
            files.sort_unstable();
            files.dedup();
            for &file in &files {
                by_file.entry(file).or_default().families.push(i);
            }
            for loc in &f.locations {
                by_file
                    .entry(loc.file.as_str())
                    .or_default()
                    .intervals
                    .push(MemberInterval {
                        family: i,
                        start: loc.start_line,
                        end: loc.end_line,
                    });
            }
            family_files.push(files);
        }
        let capped_files: FxHashSet<&str> = by_file
            .iter()
            .filter_map(|(&file, bucket)| (bucket.families.len() <= PER_FILE_CAP).then_some(file))
            .collect();
        // Keep the old cap semantics (a pair needs at least one capped shared file), but
        // only run the full greedy overlap check for pairs that have two possible member
        // overlaps anywhere. The greedy pass below remains the behavioral authority.
        let mut candidates: Vec<(usize, usize)> = overlapping_candidate_counts(&by_file)
            .into_iter()
            .filter_map(|(pair @ (i, j), count)| {
                (count >= 2 && share_capped_file(&family_files[i], &family_files[j], &capped_files))
                    .then_some(pair)
            })
            .collect();
        candidates.sort_unstable();
        let direct_pairs: Vec<(usize, usize)> = candidates
            .into_iter()
            .filter(|&(i, j)| overlapping_member_pairs(families[i], families[j]) >= 2)
            // A newly admitted pair-local witness may be ranked above a broad existing
            // family because its source bounds are tighter. It can be folded under that
            // existing family when covered, but it must never become the presentation
            // root that hides previously visible product output.
            .filter(|&(i, j)| {
                !is_same_unit(families[i])
                    && !is_same_unit(families[j])
                    && (!is_connected(families[i]) || is_connected(families[j]))
            })
            .collect();
        // Presentation overlap is a graph, not an equivalence relation. A direct
        // spanning forest keeps syntax-only suppression edges navigable without
        // manufacturing a transitive C → A edge. Accepted carriers may instead
        // point at the visible component root only when it covers every direct
        // accepted edge; otherwise they become visible.
        let (direct_parent, original_roots) =
            direct_suppression_forest(families.len(), &direct_pairs);
        let mut primary_index = direct_parent.clone();
        for (index, family) in families.iter().enumerate() {
            if family.accepted_coverage.is_empty() {
                continue;
            }
            let root = opportunity_root(&direct_parent, index);
            primary_index[index] = (root != index
                && accepted_obligations_covered(families[root], family))
            .then_some(root);
        }
        // A carrier rejected by its component root can still be redundant when
        // the old visible roots collectively cover each of its accepted edges.
        // Check exact edge pairs (never endpoint sites independently), then keep
        // only carriers that add at least one previously uncovered accepted edge.
        let mut coverage_roots = vec![false; families.len()];
        for root in original_roots {
            coverage_roots[root] = true;
        }
        for index in 0..families.len() {
            if primary_index[index].is_some()
                || direct_parent[index].is_none()
                || families[index].accepted_coverage.is_empty()
            {
                continue;
            }
            if accepted_edges_covered_by_roots(families[index], &coverage_roots, &by_file) {
                primary_index[index] = direct_parent[index];
            } else {
                coverage_roots[index] = true;
            }
        }
        let mut groups = Self::default();
        for (i, primary) in primary_index.into_iter().enumerate() {
            if let Some(primary_index) = primary {
                let primary = baseline::family_id(families[primary_index]);
                let slice = baseline::family_id(families[i]);
                if is_default(families[i]) && is_default(families[primary_index]) {
                    groups.default_slices.insert(slice.clone());
                }
                groups.primary_of.insert(slice.clone(), primary.clone());
                groups.slices_of.entry(primary).or_default().push(slice);
            }
        }
        groups
    }

    pub(crate) fn is_slice(&self, family: &nose_detect::RefactorFamily) -> bool {
        self.primary_of.contains_key(&baseline::family_id(family))
    }

    pub(crate) fn is_default_slice(&self, family: &nose_detect::RefactorFamily) -> bool {
        self.default_slices.contains(&baseline::family_id(family))
    }

    pub(crate) fn slices(&self, family: &nose_detect::RefactorFamily) -> Option<&[String]> {
        self.slices_of
            .get(&baseline::family_id(family))
            .map(Vec::as_slice)
    }
}

fn is_connected(family: &nose_detect::RefactorFamily) -> bool {
    matches!(
        family.witness.as_ref().map(|witness| witness.kind()),
        Some("connected-mapped-sub-dag" | "bounded-same-unit-window")
    )
}

fn is_same_unit(family: &nose_detect::RefactorFamily) -> bool {
    family.witness.as_ref().map(|witness| witness.kind()) == Some("bounded-same-unit-window")
}

#[derive(Default)]
struct FileOpportunityBucket {
    families: Vec<usize>,
    intervals: Vec<MemberInterval>,
}

#[derive(Clone, Copy)]
struct MemberInterval {
    family: usize,
    start: u32,
    end: u32,
}

fn overlapping_candidate_counts(
    by_file: &FxHashMap<&str, FileOpportunityBucket>,
) -> FxHashMap<(usize, usize), u8> {
    let mut counts: FxHashMap<(usize, usize), u8> = FxHashMap::default();
    for bucket in by_file.values() {
        let mut intervals = bucket.intervals.clone();
        intervals.sort_unstable_by_key(|iv| (iv.start, iv.end, iv.family));
        for left in 0..intervals.len() {
            let a = intervals[left];
            for &b in &intervals[left + 1..] {
                if b.start > a.end {
                    break;
                }
                if a.family == b.family || !intervals_half_overlap(a, b) {
                    continue;
                }
                let key = (a.family.min(b.family), a.family.max(b.family));
                let count = counts.entry(key).or_insert(0);
                if *count < 2 {
                    *count += 1;
                }
            }
        }
    }
    counts
}

fn intervals_half_overlap(a: MemberInterval, b: MemberInterval) -> bool {
    let lo = a.start.max(b.start);
    let hi = a.end.min(b.end);
    if lo > hi {
        return false;
    }
    let overlap = hi - lo + 1;
    let len_a = a.end - a.start + 1;
    let len_b = b.end - b.start + 1;
    overlap * 2 >= len_a.min(len_b)
}

fn share_capped_file(a: &[&str], b: &[&str], capped_files: &FxHashSet<&str>) -> bool {
    let (mut i, mut j) = (0, 0);
    while let (Some(&fa), Some(&fb)) = (a.get(i), b.get(j)) {
        match fa.cmp(fb) {
            Ordering::Less => i += 1,
            Ordering::Greater => j += 1,
            Ordering::Equal => {
                if capped_files.contains(fa) {
                    return true;
                }
                i += 1;
                j += 1;
            }
        }
    }
    false
}

/// Greedy one-to-one count of member pairs that overlap on the same file by
/// at least half of the shorter span.
fn overlapping_member_pairs(
    a: &nose_detect::RefactorFamily,
    b: &nose_detect::RefactorFamily,
) -> usize {
    let mut used = vec![false; b.locations.len()];
    let mut pairs = 0;
    for a_loc in &a.locations {
        for (j, b_loc) in b.locations.iter().enumerate() {
            if used[j] || a_loc.file != b_loc.file {
                continue;
            }
            let lo = a_loc.start_line.max(b_loc.start_line);
            let hi = a_loc.end_line.min(b_loc.end_line);
            if lo > hi {
                continue;
            }
            let overlap = hi - lo + 1;
            let a_len = a_loc.end_line - a_loc.start_line + 1;
            let b_len = b_loc.end_line - b_loc.start_line + 1;
            if overlap * 2 >= a_len.min(b_len) {
                used[j] = true;
                pairs += 1;
                break;
            }
        }
    }
    pairs
}

/// Distinct languages in a family, sorted — e.g. `"python, typescript"`. Empty
/// when the family is single-language (caller decides whether to show anything).
pub(crate) fn family_langs(f: &nose_detect::RefactorFamily) -> String {
    if f.languages <= 1 {
        return String::new();
    }
    let mut langs: Vec<&str> = f.locations.iter().map(|l| l.lang.as_str()).collect();
    langs.sort_unstable();
    langs.dedup();
    langs.join(", ")
}

/// Observation and an inspection direction; never an edit instruction.
pub(crate) fn family_hint(f: &nose_detect::RefactorFamily) -> String {
    let (shared, params) = (f.shared_lines, f.display_params.unwrap_or(f.params));
    let assessment = crate::query_assessment::assessment(f, shared, params);
    let explanation = assessment["explanation"].as_str().unwrap();
    if f.languages > 1 || (f.display_params.is_some() && (shared == 0 || f.shared_weight <= 0.0)) {
        return explanation.into();
    }
    if let Some(helper) = crate::query_model::family_existing_helper(f) {
        return format!("existing helper candidate `{}` ({}); compare its call contract and the inline regions — callability is not established", helper.name.as_deref().unwrap_or("unnamed"), helper.file);
    }
    let context = if crate::query_model::family_same_symbol(f) {
        format!(
            "same symbol `{}` at {} sites",
            f.locations[0].name.as_deref().unwrap_or("unnamed"),
            f.members
        )
    } else {
        format!(
            "{} related regions across {} director{}",
            f.members,
            f.modules,
            if f.modules == 1 { "y" } else { "ies" }
        )
    };
    let direction =
        origin_extract_hint(f).unwrap_or("compare the shared computation and differing regions");
    format!("{context} — {direction}")
}
