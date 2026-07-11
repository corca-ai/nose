use crate::{Group, Loc, Report};
use nose_semantics::value_law_provenance;
use rayon::prelude::*;

use super::{
    model::{AcceptedCoverage, RefactorFamily},
    paths::{
        is_generated_loc, is_test_loc, module_of, overlap_frac, refactor_discount, span_lines,
    },
    score::refactor_value,
};

fn time_rank_stage<T>(stage: &str, f: impl FnOnce() -> T) -> T {
    if std::env::var_os("NOSE_TIME").is_none() {
        return f();
    }
    let t0 = std::time::Instant::now();
    let out = f();
    eprintln!(
        "  [time] rank_{stage:<7} {:>7.1}ms",
        t0.elapsed().as_secs_f64() * 1e3
    );
    out
}

/// The distinct keys of `locs` under `key`, sorted. Collect-then-`sort_unstable`+`dedup` is
/// the family-stat idiom for counting distinct files / modules / languages.
fn distinct_by<'a>(locs: &'a [Loc], key: impl Fn(&'a Loc) -> &'a str) -> Vec<&'a str> {
    let mut v: Vec<&'a str> = locs.iter().map(key).collect();
    v.sort_unstable();
    v.dedup();
    v
}

#[cfg(test)]
pub(super) fn family_of(group: &Group) -> RefactorFamily {
    family_of_with_edges(group, &[])
}

fn family_of_with_edges(group: &Group, group_edges: &[(u32, u32)]) -> RefactorFamily {
    // Collapse co-located units to one refactoring site. Block extraction yields a
    // function unit *and* inner blocks that overlap it, and near-identical spans can
    // differ by a line; all of these are one place to refactor, not several. Keep the
    // largest enclosing span per file and drop anything that substantially overlaps it.
    let mut locs = group.members.clone();
    // Largest span first (within a file) so the enclosing unit wins.
    locs.sort_by(|a, b| {
        a.file
            .cmp(&b.file)
            .then_with(|| span_lines(b).cmp(&span_lines(a)))
            .then_with(|| a.start_line.cmp(&b.start_line))
    });
    let mut kept: Vec<Loc> = Vec::with_capacity(locs.len());
    let mut kept_by_file: rustc_hash::FxHashMap<String, Vec<usize>> =
        rustc_hash::FxHashMap::default();
    for l in locs {
        let subsumed = kept_by_file
            .get(l.file.as_str())
            .is_some_and(|idxs| idxs.iter().any(|&i| overlap_frac(&kept[i], &l) >= 0.5));
        if !subsumed {
            let file = l.file.clone();
            kept.push(l);
            kept_by_file.entry(file).or_default().push(kept.len() - 1);
        }
    }
    let mut locs = kept;
    locs.sort_by_key(|b| std::cmp::Reverse(span_lines(b)));
    let members = locs.len();
    let total_lines: u32 = locs.iter().map(span_lines).sum();
    let mean_lines = if members > 0 {
        total_lines / members as u32
    } else {
        0
    };
    let dup_lines = mean_lines * (members.saturating_sub(1) as u32);

    let files = distinct_by(&locs, |l| l.file.as_str());
    let modules = distinct_by(&locs, |l| module_of(&l.file));
    let langs = distinct_by(&locs, |l| l.lang.as_str());

    let mean_sem = if members > 0 {
        locs.iter().map(|l| l.sem as f64).sum::<f64>() / members as f64
    } else {
        0.0
    };
    let n_test = locs.iter().filter(|l| is_test_loc(l)).count();
    let scope = if n_test == 0 {
        "prod"
    } else if n_test == members {
        "test"
    } else {
        "mixed"
    };
    let all_class = locs.iter().all(|l| l.kind == nose_il::UnitKind::Class);
    let all_generated = locs.iter().all(is_generated_loc);

    let discount = refactor_discount(all_class, mean_sem, all_generated);
    let value = refactor_value(
        mean_lines,
        members,
        group.score,
        files.len(),
        modules.len(),
        langs.len(),
    ) * discount;
    let collapsed_edges = collapsed_accepted_edges(group, &locs, group_edges);
    let accepted_coverage = (!collapsed_edges.is_empty())
        .then(|| {
            vec![AcceptedCoverage {
                sites: locs.clone(),
                edges: collapsed_edges,
            }]
        })
        .unwrap_or_default();
    RefactorFamily {
        value,
        members,
        files: files.len(),
        modules: modules.len(),
        languages: langs.len(),
        mean_score: group.score,
        mean_lines,
        dup_lines,
        // Filled in at the presentation layer (needs source access).
        shared_lines: 0,
        params: 0,
        shared_weight: 0.0,
        locations: locs,
        accepted_coverage,
        display_params: None,
        mean_sem,
        scope,
        discount,
        abstraction_witness: group.abstraction_witness.clone(),
        witness: group.witness.clone(),
        varying_spots: Vec::new(),
        semantic_laws: group
            .semantic_laws
            .iter()
            .filter_map(|&law| value_law_provenance(law))
            .collect(),
    }
}

fn collapsed_accepted_edges(
    group: &Group,
    collapsed_sites: &[Loc],
    group_edges: &[(u32, u32)],
) -> Vec<(u32, u32)> {
    let site_of: Vec<Option<u32>> = group
        .members
        .iter()
        .map(|member| {
            collapsed_sites
                .iter()
                .enumerate()
                .filter(|(_, site)| site.file == member.file)
                .map(|(index, site)| (index as u32, overlap_frac(site, member)))
                .filter(|(_, overlap)| *overlap >= 0.5)
                .max_by(|a, b| a.1.total_cmp(&b.1))
                .map(|(index, _)| index)
        })
        .collect();
    let mut edges: Vec<(u32, u32)> = group_edges
        .iter()
        .filter_map(|&(left, right)| {
            let left = site_of.get(left as usize).copied().flatten()?;
            let right = site_of.get(right as usize).copied().flatten()?;
            (left != right).then_some((left.min(right), left.max(right)))
        })
        .collect();
    edges.sort_unstable();
    edges.dedup();
    edges
}

/// Rank a detection report's groups as refactoring opportunities, highest value
/// first. Trivial families (a single pair of tiny fragments) sink to the bottom.
pub fn rank_families(report: &Report) -> Vec<RefactorFamily> {
    let mut fams: Vec<RefactorFamily> = time_rank_stage("map", || {
        report
            .groups
            .par_iter()
            .enumerate()
            .map(|(index, group)| {
                family_of_with_edges(
                    group,
                    report
                        .accepted_group_edges
                        .get(index)
                        .map(Vec::as_slice)
                        .unwrap_or_default(),
                )
            })
            // Drop families living entirely in generated / vendored / ambient-declaration
            // files (`vendor/`, `.min.`, `*.d.ts`, `// Generated`-style paths): you don't
            // refactor code a tool regenerates. A *partly*-generated family is kept — that's
            // a real leak of hand-written logic into generated output.
            .filter(|f| !f.locations.iter().all(is_generated_loc))
            // A raw group can collapse to one reported site when all of its matches are
            // overlapping windows in the same file. That is not a clone family, and it must
            // not subsume a real multi-site family before the CLI's min-member gate drops it.
            .filter(|f| f.members >= 2)
            .collect()
    });
    // Dedup overlapping families by total span, LARGEST FIRST, so the most complete
    // family of a region is the one kept and the sub-blocks inside it are dropped. (A
    // value/extractability order would keep whichever scored highest — often a sub-block
    // — leaving its enclosing family *also* in the list: the same OAuth test-server or
    // design-verifier function reported as several overlapping entries. The caller
    // re-sorts the survivors by the chosen key, so this order only governs dedup.)
    // Sort LARGEST span first (then value), with a min-location final tie-break so this is a TOTAL
    // order: families that tie on span AND value still sort deterministically by source position,
    // not by upstream group-iteration order. Without it, two equal-span/value families could be
    // deduped in either direction depending on map-iteration order, making the kept set (and so the
    // dup-gate count) sensitive to incidental ordering. (The gate is otherwise fully deterministic:
    // FxHash and IEEE `+−×÷`/`sqrt` are platform-independent, so CI and local agree exactly.)
    time_rank_stage("sort", || {
        fams.sort_by(|a, b| {
            total_span(b)
                .cmp(&total_span(a))
                .then(b.value.total_cmp(&a.value))
                .then_with(|| family_min_loc(a).cmp(&family_min_loc(b)))
        })
    });
    // Keep a family unless an already-kept (larger) one subsumes it. `subsumes(k, f)`
    // requires `k` to cover *every* `f` site, so it must cover `f`'s first site. Index
    // kept location intervals by file and test only families whose interval actually
    // covers that first site, instead of every family that merely touches the same file.
    // Same result: every possible subsumer is still in the candidate set, but generated
    // docs with thousands of overlapping HTML slices no longer walk the whole file bucket.
    let mut kept: Vec<RefactorFamily> = Vec::with_capacity(fams.len());
    let mut by_file: rustc_hash::FxHashMap<String, Vec<(u32, u32, usize)>> =
        rustc_hash::FxHashMap::default();
    time_rank_stage("dedup", || {
        for mut f in fams {
            let subsumer = f.locations.first().and_then(|first| {
                by_file.get(first.file.as_str()).and_then(|spans| {
                    let mut seen = rustc_hash::FxHashSet::default();
                    spans.iter().find_map(|&(start, end, ki)| {
                        (seen.insert(ki)
                            && overlap_frac_span(start, end, first) >= SUBSUME_COVER
                            && subsumes(&kept[ki], &f))
                        .then_some(ki)
                    })
                })
            });
            if let Some(ki) = subsumer {
                merge_accepted_coverage(
                    &mut kept[ki].accepted_coverage,
                    std::mem::take(&mut f.accepted_coverage),
                );
            } else {
                let ki = kept.len();
                for l in &f.locations {
                    by_file
                        .entry(l.file.clone())
                        .or_default()
                        .push((l.start_line, l.end_line, ki));
                }
                kept.push(f);
            }
        }
    });
    kept
}

fn merge_accepted_coverage(existing: &mut Vec<AcceptedCoverage>, incoming: Vec<AcceptedCoverage>) {
    for obligation in incoming {
        let duplicate = existing.iter().any(|current| {
            current.edges == obligation.edges
                && current.sites.len() == obligation.sites.len()
                && current.sites.iter().zip(&obligation.sites).all(|(a, b)| {
                    a.file == b.file && a.start_line == b.start_line && a.end_line == b.end_line
                })
        });
        if !duplicate {
            existing.push(obligation);
        }
    }
}

/// Total source lines a family spans across all its sites — its "size" for dedup.
fn total_span(f: &RefactorFamily) -> u32 {
    f.locations.iter().map(span_lines).sum()
}

/// A family's lexicographically smallest site `(file, start_line, end_line)` — a stable identity
/// used only as the final dedup-sort tie-break, so equal-span/value families order deterministically
/// by source position rather than by incidental group-iteration order.
pub(super) fn family_min_loc(f: &RefactorFamily) -> Option<(&str, u32, u32)> {
    f.locations
        .iter()
        .map(|l| (l.file.as_str(), l.start_line, l.end_line))
        .min()
}

/// Does family `outer` subsume `inner` — i.e. every `inner` site lands on (mostly
/// inside) some `outer` site in the same file? Then `inner` reports the same regions
/// already covered. This catches two cases the field eval flagged as double-counting:
/// strict containment (a shared loop body reported alongside the enclosing functions),
/// and **window-shifted overlap** — the contiguous channel finding the same run at a
/// few different start lines, surfacing as several near-identical families. Requiring
/// the bulk (≥60%) of each inner site to fall in an outer site collapses both without
/// merging genuinely distinct code (which would need >60% line overlap to qualify).
pub(super) fn subsumes(outer: &RefactorFamily, inner: &RefactorFamily) -> bool {
    // No site-count guard: a single large outer site can cover several smaller inner
    // sites, so requiring `outer.len() >= inner.len()` wrongly kept those (double-counted)
    // inner families. Coverage alone — every inner site ≥60% inside some same-file outer
    // site — is the criterion. The caller only ever asks whether a larger-span (kept)
    // family subsumes a smaller one, so this can't collapse genuinely distinct code.
    inner.locations.iter().all(|i| {
        outer
            .locations
            .iter()
            .any(|o| o.file == i.file && overlap_frac(o, i) >= SUBSUME_COVER)
    })
}

const SUBSUME_COVER: f64 = 0.60;

fn overlap_frac_span(start_line: u32, end_line: u32, inner: &Loc) -> f64 {
    let start = start_line.max(inner.start_line);
    let end = end_line.min(inner.end_line);
    if end < start {
        return 0.0;
    }
    (end - start + 1) as f64 / span_lines(inner).max(1) as f64
}
