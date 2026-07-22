use super::git::{
    canonical, ensure_base_ref_available, git_changed_ranges_and_entries, git_repo_root,
    repo_relative_paths, reroot_paths, BaseWorktree, DiffEntry,
};
use super::targets::{direct_targets, same_loc};
use super::*;
use crate::cli_args::QueryArgs;
use crate::query_dataset::{build_divergence_families, prepare_divergence_query};
use crate::query_witness::enrich_graded_witnesses;
use crate::source_lines::{varying_spots_of, FileLineCache};
use crate::timing::time_stage;
use std::collections::{HashMap, HashSet};
use std::time::{Duration, Instant};

mod paths;
use paths::{repo_relative, repo_relative_loc};

const NEW_COPY_SOURCE_FILE_BUDGET: usize = 2;

/// The detection half for `nose query base=<ref>`. Returns the flagged divergences plus how
/// many files changed; `None` when there is nothing comparable (an adds-only / empty diff).
/// The temporary base worktree is created and torn down inside; returned `Divergence`s own
/// their data.
pub(crate) fn detect_divergences(
    args: &QueryArgs,
    base_ref: &str,
) -> Result<Option<(Vec<Divergence>, usize)>> {
    let root = git_repo_root().context(
        "nose needs a git repository to compare the working tree to a git ref (`base=`/`--base`)",
    )?;
    let cfg = crate::config::load_query(args.config.as_deref())?;
    let ignore_file = args.ignore_file.clone().or_else(|| cfg.ignore_file.clone());

    // Structured ignores suppress accepted divergences, so an intentional fork doesn't
    // re-fail every PR. Load them before diff short-circuiting so malformed ignore files
    // fail consistently even on empty diffs.
    let ignore_set = crate::ignores::load_for_query(ignore_file.as_deref())?;
    if let Some(set) = &ignore_set {
        set.warn_expired();
    }

    let divergence_paths = repo_relative_paths(&args.paths, &root);
    ensure_base_ref_available(&root, base_ref)?;
    let (changed, current_changed, diff_entries) = time_stage("base_diff", || {
        git_changed_ranges_and_entries(&root, base_ref, &divergence_paths)
    })?;
    let current_lane_requested = has_current_tree_new_copy_trigger(&diff_entries);
    if changed.is_empty() && !current_lane_requested {
        return Ok(None);
    }
    // Detect clone families at the base, where every copy is still intact. A temporary
    // worktree gives the base tree on disk without disturbing the user's working tree.
    let base_tree = BaseWorktree::create(&root, base_ref)?;
    let base_paths = reroot_paths(&divergence_paths, &base_tree.path);
    let base_refs = base_paths.iter().map(PathBuf::as_path).collect::<Vec<_>>();
    let plan = prepare_divergence_query(args)?;
    let current_projection_opts = *plan.options();
    // Current-tree witness projection is independent of base detection. Overlap it on one
    // dedicated thread, but keep the projection itself serial: letting both sides consume
    // the global Rayon pool inflated the measured base parse/normalize stages through CPU
    // contention. One bounded worker hides the independent I/O without perturbing the
    // detector's parallel work.
    let (detected, preprojected_current) = std::thread::scope(|scope| {
        let projection = scope.spawn(|| {
            time_stage("base_preproject", || {
                change_witness::preproject_current_files(
                    &root,
                    &current_changed,
                    &current_projection_opts,
                )
            })
        });
        let detected = time_stage("base_detect", || {
            build_divergence_families(args, &base_refs, plan)
        });
        let preprojected_current = projection
            .join()
            .expect("bounded current-tree projection worker panicked");
        (detected, preprojected_current)
    });
    let (families, enrich_opts, retained_base) = detected?;

    // Reuse the resolved options for per-flagged-family graded-witness enrichment,
    // so re-derived unit roots line up with the family locations' spans.
    let mut flagged = time_stage("base_flag", || {
        flag_divergences(
            &families,
            ignore_set.as_ref(),
            &changed,
            &base_tree.path,
            &enrich_opts,
        )
    });
    time_stage("base_witness", || {
        change_witness::enrich_semantic_change_witnesses(
            &mut flagged,
            change_witness::SemanticWitnessInputs {
                base_root: &base_tree.path,
                current_root: &root,
                base_changed: &changed,
                current_changed: &current_changed,
                diff_entries: &diff_entries,
                opts: &enrich_opts,
                retained_base,
                preprojected_current,
            },
        )
    });
    if current_lane_requested {
        let current_paths = reroot_paths(&divergence_paths, &root);
        let current_refs = current_paths
            .iter()
            .map(PathBuf::as_path)
            .collect::<Vec<_>>();
        let current_plan = prepare_divergence_query(args)?;
        let (mut current_families, _, _) =
            build_divergence_families(args, &current_refs, current_plan)?;
        let current_prefix = canonical(&root);
        for family in &mut current_families {
            for loc in &mut family.locations {
                repo_relative_loc(loc, &root, &current_prefix);
            }
        }
        flagged.extend(flag_new_copy_divergences(
            &current_families,
            &families,
            ignore_set.as_ref(),
            &current_changed,
            &diff_entries,
            &base_tree.path,
        ));
    }

    // base_tree is removed by Drop after we finish reading families.
    drop(base_tree);
    Ok(Some((flagged, changed_file_count(&diff_entries))))
}

/// Whether a flagged set fires the v2 strict CI gate.
pub(crate) fn divergences_fire(flagged: &[Divergence]) -> bool {
    flagged.iter().any(Divergence::gate_fail_default)
}

/// Flag families with *some but not all* members changed by the diff, most likely
/// un-propagated fix first. Member paths are normalized to repo-relative first, so the
/// family_id is stable across runs (the base worktree lives at a per-run temp path) and
/// matches what the ignore file uses.
fn flag_divergences(
    families: &[RefactorFamily],
    ignore_set: Option<&crate::ignores::IgnoreSet>,
    changed: &HashMap<String, Vec<(u32, u32)>>,
    base_root: &Path,
    enrich_opts: &nose_detect::DetectOptions,
) -> Vec<Divergence> {
    let prefix = canonical(base_root);
    let mut lines = FileLineCache::default();
    let mut flagged: Vec<Divergence> = Vec::new();
    let mut graded_inputs = Vec::new();
    let timed = std::env::var_os("NOSE_TIME").is_some();
    let mut relative_elapsed = Duration::ZERO;
    let mut target_elapsed = Duration::ZERO;
    let mut touch_elapsed = Duration::ZERO;
    for orig in families {
        let started = Instant::now();
        let fam = repo_relative(orig, base_root, &prefix);
        relative_elapsed += started.elapsed();
        if ignore_set.is_some_and(|set| set.match_family(&fam).is_some()) {
            continue;
        }
        let (changed_members, untouched): (Vec<&Loc>, Vec<&Loc>) = fam
            .locations
            .iter()
            .partition(|loc| site_touched_loc(loc, changed));
        if changed_members.is_empty() || untouched.is_empty() {
            continue;
        }
        // Keep the original ABSOLUTE base-worktree paths for graded enrichment. We enrich
        // all flagged families together below so a file shared by several families is
        // lowered once, rather than once per family.
        graded_inputs.push(orig.clone());
        let witness_kind = fam.witness.as_ref().map(|w| w.kind);
        // A family is a transitive component, not a propagation-target list. Build
        // only changed -> skipped pairs that the detector accepted directly, then
        // compute shared-line contact against that exact sibling.
        let started = Instant::now();
        let targets = direct_targets(&fam, base_root, &mut lines, changed);
        target_elapsed += started.elapsed();
        let started = Instant::now();
        let touches: Vec<Option<bool>> = changed_members
            .iter()
            .map(|member| {
                if let [sibling] = untouched.as_slice() {
                    if let Some(touch) = targets.iter().find(|target| {
                        same_loc(member, &target.changed)
                            && same_loc(sibling, &target.skipped)
                            && Some(target.direct_witness.kind) == witness_kind
                    }) {
                        return touch.changed.touches_shared;
                    }
                }
                touches_shared_lines(
                    member,
                    &untouched,
                    witness_kind,
                    base_root,
                    &mut lines,
                    changed,
                )
            })
            .collect();
        touch_elapsed += started.elapsed();
        // All-test families are divergence context, not gate material: §BG-audit
        // found test variants legitimately diverge, and on the §BR labels the
        // scope term doubled gate precision at zero true-positive cost.
        let fire_eligible = touches.contains(&Some(true)) && fam.scope != "test";
        flagged.push(Divergence {
            lane: DivergenceLane::BaseDivergence,
            family_id: crate::baseline::family_id(&fam),
            similarity: fam.mean_score,
            hazard: fam.hazard(),
            divergence_priority: divergence_priority(&fam, &changed_members, &untouched),
            // Heaviest changed member's value-graph size — a cheap complexity proxy. A
            // small edit inside a computation-rich clone is the Krinke "critical change"
            // profile (the most likely un-propagated fix); an edit in a trivial clone is
            // likely benign.
            complexity: changed_members.iter().map(|l| l.sem).max().unwrap_or(0),
            scope: fam.scope,
            witness_kind,
            fire_eligible,
            graded: None,
            changed: changed_members
                .iter()
                .zip(&touches)
                .map(|(l, t)| to_site_touch(l, *t))
                .collect(),
            not_updated: untouched.iter().map(|l| to_site(l)).collect(),
            targets,
        });
    }
    let graded_started = Instant::now();
    enrich_graded_witnesses(&mut graded_inputs, enrich_opts);
    let graded_elapsed = graded_started.elapsed();
    for (divergence, family) in flagged.iter_mut().zip(graded_inputs) {
        divergence.graded = family.witness.and_then(|witness| witness.graded);
    }
    log_base_stage_timings(
        timed,
        relative_elapsed,
        target_elapsed,
        touch_elapsed,
        graded_elapsed,
    );
    // Most likely un-propagated fix first.
    flagged.sort_by(|a, b| {
        b.divergence_priority
            .cmp(&a.divergence_priority)
            .then(b.hazard.total_cmp(&a.hazard))
            .then(b.complexity.cmp(&a.complexity))
            .then(b.similarity.total_cmp(&a.similarity))
    });
    flagged
}

fn log_base_stage_timings(
    timed: bool,
    relative: Duration,
    targets: Duration,
    touches: Duration,
    graded: Duration,
) {
    if !timed {
        return;
    }
    for (stage, elapsed) in [
        ("base-relative", relative),
        ("base-targets ", targets),
        ("base-touches ", touches),
        ("base-graded  ", graded),
    ] {
        eprintln!("  [time] {stage} {:>7.1}ms", elapsed.as_secs_f64() * 1e3);
    }
}

fn flag_new_copy_divergences(
    current_families: &[RefactorFamily],
    base_families: &[RefactorFamily],
    ignore_set: Option<&crate::ignores::IgnoreSet>,
    current_changed: &HashMap<String, Vec<(u32, u32)>>,
    diff_entries: &[DiffEntry],
    base_root: &Path,
) -> Vec<Divergence> {
    let current_to_base = current_to_base_paths(diff_entries);
    let created_current_paths = created_current_paths(diff_entries);
    let base_prefix = canonical(base_root);
    let base_relative: Vec<RefactorFamily> = base_families
        .iter()
        .map(|fam| repo_relative(fam, base_root, &base_prefix))
        .collect();
    let base_signatures = family_signatures(&base_relative, &HashMap::new(), true);
    let base_identity_signatures = family_signatures(&base_relative, &HashMap::new(), false);
    let mut flagged = Vec::new();
    for fam in current_families {
        if ignore_set.is_some_and(|set| set.match_family(fam).is_some()) {
            continue;
        }
        let (changed_members, untouched): (Vec<&Loc>, Vec<&Loc>) = fam
            .locations
            .iter()
            .partition(|loc| site_touched_loc(loc, current_changed));
        if changed_members.is_empty() || untouched.is_empty() {
            continue;
        }
        if !changed_members
            .iter()
            .any(|loc| created_current_paths.contains(&loc.file))
        {
            continue;
        }
        let mapped_signature = family_signature(fam, &current_to_base, true);
        let mapped_identity = family_signature(fam, &current_to_base, false);
        if base_signatures.contains(&mapped_signature)
            || base_identity_signatures.contains(&mapped_identity)
        {
            continue;
        }
        let witness_kind = fam.witness.as_ref().map(|w| w.kind);
        flagged.push(Divergence {
            lane: DivergenceLane::NewCopy,
            family_id: crate::baseline::family_id(fam),
            similarity: fam.mean_score,
            hazard: fam.hazard(),
            divergence_priority: divergence_priority(fam, &changed_members, &untouched),
            complexity: changed_members.iter().map(|l| l.sem).max().unwrap_or(0),
            scope: fam.scope,
            witness_kind,
            fire_eligible: false,
            graded: None,
            changed: changed_members.iter().map(|l| to_site(l)).collect(),
            not_updated: untouched.iter().map(|l| to_site(l)).collect(),
            targets: Vec::new(),
        });
    }
    flagged.sort_by(|a, b| {
        b.divergence_priority
            .cmp(&a.divergence_priority)
            .then(b.hazard.total_cmp(&a.hazard))
            .then(b.complexity.cmp(&a.complexity))
            .then(b.similarity.total_cmp(&a.similarity))
    });
    flagged
}

fn has_current_tree_new_copy_trigger(entries: &[DiffEntry]) -> bool {
    let source_entries = entries
        .iter()
        .filter(|entry| diff_entry_touches_source(entry));
    let mut count = 0;
    let mut has_created_source = false;
    for entry in source_entries {
        count += 1;
        has_created_source |= entry.status.creates_current_path()
            && entry.new_path.as_deref().is_some_and(source_like_path);
        if count > NEW_COPY_SOURCE_FILE_BUDGET {
            return false;
        }
    }
    has_created_source
}

fn diff_entry_touches_source(entry: &DiffEntry) -> bool {
    entry.new_path.as_deref().is_some_and(source_like_path)
        || entry.old_path.as_deref().is_some_and(source_like_path)
}

fn created_current_paths(entries: &[DiffEntry]) -> HashSet<String> {
    if !has_current_tree_new_copy_trigger(entries) {
        return HashSet::new();
    }
    entries
        .iter()
        .filter(|entry| {
            entry.status.creates_current_path()
                && entry.new_path.as_deref().is_some_and(source_like_path)
        })
        .filter_map(|entry| entry.new_path.clone())
        .collect()
}

fn changed_file_count(entries: &[DiffEntry]) -> usize {
    entries
        .iter()
        .filter_map(|entry| entry.new_path.as_ref().or(entry.old_path.as_ref()))
        .collect::<HashSet<_>>()
        .len()
}

fn current_to_base_paths(entries: &[DiffEntry]) -> HashMap<String, Option<String>> {
    entries
        .iter()
        .filter_map(|entry| {
            entry
                .new_path
                .as_ref()
                .map(|new_path| (new_path.clone(), entry.old_path.clone()))
        })
        .collect()
}

fn source_like_path(path: &str) -> bool {
    nose_il::Lang::from_path(path).is_some()
}

fn family_signatures(
    families: &[RefactorFamily],
    current_to_base: &HashMap<String, Option<String>>,
    include_span: bool,
) -> HashSet<Vec<String>> {
    families
        .iter()
        .map(|fam| family_signature(fam, current_to_base, include_span))
        .collect()
}

fn family_signature(
    fam: &RefactorFamily,
    current_to_base: &HashMap<String, Option<String>>,
    include_span: bool,
) -> Vec<String> {
    let mut members: Vec<String> = fam
        .locations
        .iter()
        .map(|loc| member_signature(loc, current_to_base, include_span))
        .collect();
    members.sort_unstable();
    members
}

fn member_signature(
    loc: &Loc,
    current_to_base: &HashMap<String, Option<String>>,
    include_span: bool,
) -> String {
    let mapped_file = match current_to_base.get(&loc.file) {
        Some(Some(old)) => old.as_str(),
        Some(None) => "<new-current-member>",
        None => loc.file.as_str(),
    };
    let span = if include_span {
        format!(":{}-{}", loc.start_line, loc.end_line)
    } else {
        String::new()
    };
    format!(
        "{}|{}{}|{:?}|{}|{}|{:?}|{}",
        mapped_file,
        loc.lang,
        span,
        loc.kind,
        loc.name.as_deref().unwrap_or_default(),
        loc.is_fragment,
        loc.fragment_kind,
        loc.reason_code.unwrap_or_default(),
    )
}

pub(super) fn to_site(loc: &Loc) -> Site {
    Site {
        file: loc.file.clone(),
        name: loc.name.clone(),
        start_line: loc.start_line,
        end_line: loc.end_line,
        lang: loc.lang.clone(),
        kind: loc.kind,
        span_lines: loc.span_lines,
        span_tokens: loc.span_tokens,
        is_fragment: loc.is_fragment,
        fragment_kind: loc.fragment_kind,
        reason_code: loc.reason_code,
        enclosing_unit: loc.enclosing_unit.clone(),
        touches_shared: None,
        semantic_change: None,
    }
}

pub(super) fn to_site_touch(loc: &Loc, touches_shared: Option<bool>) -> Site {
    Site {
        touches_shared,
        ..to_site(loc)
    }
}

/// Does the diff PROVABLY touch lines `member` shares with an un-updated sibling?
///
/// Two proof shapes, by the family's equivalence witness:
///
/// - `exact-value-graph`: the WHOLE span is shared logic by the channel's own
///   proof — equal value fingerprints retain literal VALUES, so the copies
///   compute identically down to constants, and the typical exact clone is a
///   *renamed* twin whose every line differs textually while all of the logic
///   is shared (a line diff would under-fire exactly on the strongest
///   families). Any in-span change qualifies.
/// - everything else (`copy-paste-run`, `structural-similarity`,
///   `shared-sub-dag`): shared lines = the member's span minus its side of the
///   varying spots vs the first sibling whose source diffs cleanly. The token
///   channel abstracts identifiers/literals, so a `copy-paste-run` member may
///   legitimately vary in exactly those spots — and the §BR 51% bucket (span
///   overlap without shared-logic contact) lives in the fuzzy families. `None`
///   (unknown) when no sibling pair diffs — unreadable source, or the spot list
///   hit its cap (a truncated list under-counts variance, which would
///   over-claim shared lines). The gate treats unknown as not-eligible: it
///   fires on proof, never on absence of one.
pub(super) fn touches_shared_lines(
    member: &Loc,
    siblings: &[&Loc],
    witness_kind: Option<&'static str>,
    base_root: &Path,
    lines: &mut FileLineCache,
    changed: &HashMap<String, Vec<(u32, u32)>>,
) -> Option<bool> {
    const SPOT_CAP: usize = 16; // mirrors varying_spots_of's cap
    let changed_ranges = changed.get(&member.file)?;
    if witness_kind == Some("exact-value-graph") {
        return Some(true);
    }
    let abs = |loc: &Loc| {
        let mut l = loc.clone();
        l.file = base_root.join(&loc.file).to_string_lossy().into_owned();
        l
    };
    let a = abs(member);
    let spots = siblings.iter().find_map(|s| {
        // Same-language siblings only: a cross-language "diff" is all-varying noise.
        (s.lang == member.lang).then(|| varying_spots_of(&a, &abs(s), lines))?
    })?;
    if spots.len() >= SPOT_CAP {
        return None;
    }
    let varying: Vec<(u32, u32)> = spots.iter().filter_map(|s| s.a_lines).collect();
    let shared_touched = changed_ranges.iter().any(|&(cs, ce)| {
        // Walk the member's span; a changed line inside the span that is not in
        // any varying range is a shared-line hit. (Pure insertions are encoded
        // as empty ranges between lines and count as touching the gap they sit in.)
        let lo = cs.max(member.start_line);
        let hi = ce.min(member.end_line);
        if lo > hi {
            // Empty/insertion range: touches shared logic when it falls inside
            // the span but not strictly inside a varying range.
            let inside = cs > member.start_line && ce < member.end_line;
            return inside && !varying.iter().any(|&(vs, ve)| ce >= vs && cs <= ve);
        }
        (lo..=hi).any(|line| !varying.iter().any(|&(vs, ve)| line >= vs && line <= ve))
    });
    Some(shared_touched)
}

pub(super) fn divergence_priority(
    fam: &RefactorFamily,
    changed: &[&Loc],
    untouched: &[&Loc],
) -> u8 {
    let any_fragment = changed.iter().chain(untouched).any(|loc| loc.is_fragment);
    if !any_fragment {
        return 0;
    }
    let any_enclosing = changed
        .iter()
        .chain(untouched)
        .any(|loc| loc.enclosing_unit.is_some());
    match fam.recommended_surface() {
        "divergence" => 3,
        "hidden" if any_enclosing => 2,
        "hidden" => 1,
        _ => 1,
    }
}

/// Does this member's (repo-relative) base span overlap a changed range of its file?
pub(super) fn site_touched_loc(loc: &Loc, changed: &HashMap<String, Vec<(u32, u32)>>) -> bool {
    changed
        .get(&loc.file)
        .is_some_and(|ranges| ranges_touch(ranges, loc.start_line, loc.end_line))
}

/// Does the inclusive span `[start, end]` overlap any changed range? A pure-insertion range
/// is encoded as `(a+1, a)` (an empty interval *between* base lines a and a+1), which by this
/// test only matches a span that strictly straddles the gap — not one that merely ends at a.
pub(super) fn ranges_touch(ranges: &[(u32, u32)], start: u32, end: u32) -> bool {
    ranges.iter().any(|&(s, e)| start <= e && s <= end)
}
