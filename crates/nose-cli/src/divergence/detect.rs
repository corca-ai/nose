use super::git::{
    canonical, git_changed_ranges, git_repo_root, repo_relative_paths, reroot_paths, BaseWorktree,
};
use super::*;
use crate::detect_pipeline::detect_divergence_base_families;
use crate::query_witness::enrich_graded_witnesses;
use crate::source_lines::{varying_spots_of, FileLineCache};

/// The detection half for `nose query base=<ref>`. Returns the flagged divergences plus how
/// many files changed; `None` when there is nothing comparable (an adds-only / empty diff).
/// The temporary base worktree is created and torn down inside; returned `Divergence`s own
/// their data.
pub(crate) fn detect_divergences(
    args: &DivergenceArgs,
) -> Result<Option<(Vec<Divergence>, usize)>> {
    let root = git_repo_root().context(
        "nose needs a git repository to compare the working tree to a git ref (`base=`/`--base`)",
    )?;
    let divergence_paths = repo_relative_paths(&args.paths, &root);
    let changed = git_changed_ranges(&root, &args.base, &divergence_paths)?;
    if changed.is_empty() {
        return Ok(None);
    }
    // Detect clone families at the base, where every copy is still intact. A temporary
    // worktree gives the base tree on disk without disturbing the user's working tree.
    let base_tree = BaseWorktree::create(&root, &args.base)?;
    let cfg = crate::config::load_query(args.config.as_deref())?;
    let mut exclude = cfg.exclude.clone();
    exclude.extend(args.exclude.iter().cloned());
    let min_tokens = args.min_size.or(cfg.min_size).unwrap_or(24);
    let min_lines = args.min_lines.or(cfg.min_lines).unwrap_or(5);
    let base_paths = reroot_paths(&divergence_paths, &base_tree.path);
    let families = detect_divergence_base_families(
        &base_paths,
        &exclude,
        args.mode.clone(),
        cfg.mode,
        min_tokens,
        min_lines,
    )?;

    // Structured ignores suppress accepted divergences, so an intentional fork doesn't
    // re-fail every PR.
    let ignore_set = crate::ignores::load_for_query(args.ignore_file.as_deref())?;
    if let Some(set) = &ignore_set {
        set.warn_expired();
    }

    // Normalization knobs for the per-flagged-family graded-witness enrichment; must
    // match how `detect_divergence_base_families` lowered (cfg_norm/dce/block_units default), so the
    // re-derived unit roots line up with the family locations' spans.
    let enrich_opts = nose_detect::DetectOptions {
        min_lines,
        min_tokens,
        ..Default::default()
    };
    let flagged = flag_divergences(
        &families,
        ignore_set.as_ref(),
        &changed,
        &base_tree.path,
        &enrich_opts,
    );

    // base_tree is removed by Drop after we finish reading families.
    drop(base_tree);
    Ok(Some((flagged, changed.len())))
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
    for orig in families {
        let fam = repo_relative(orig, &prefix);
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
        // This family is flagged; only now compute the graded witness — on a clone with
        // the original ABSOLUTE base-worktree paths (enrichment re-reads source), so the
        // cost is paid per flagged family, not per family in the repo.
        let graded = {
            let mut abs = orig.clone();
            enrich_graded_witnesses(std::slice::from_mut(&mut abs), enrich_opts);
            abs.witness.and_then(|w| w.graded)
        };
        // The #245 fire policy input: does the diff touch lines this changed
        // member SHARES with an un-updated sibling (its span minus the
        // varying spots)? §BR measured 51% of divergence false-fires as
        // span-overlap-but-not-shared-logic; a gate fires only on proof.
        let witness_kind = fam.witness.as_ref().map(|w| w.kind);
        let touches: Vec<Option<bool>> = changed_members
            .iter()
            .map(|c| {
                touches_shared_lines(c, &untouched, witness_kind, base_root, &mut lines, changed)
            })
            .collect();
        // All-test families are divergence context, not gate material: §BG-audit
        // found test variants legitimately diverge, and on the §BR labels the
        // scope term doubled gate precision at zero true-positive cost.
        let fire_eligible = touches.contains(&Some(true)) && fam.scope != "test";
        flagged.push(Divergence {
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
            graded,
            changed: changed_members
                .iter()
                .zip(&touches)
                .map(|(l, t)| to_site_touch(l, *t))
                .collect(),
            not_updated: untouched.iter().map(|l| to_site(l)).collect(),
        });
    }
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

/// Clone the family with every member path made repo-relative (stripping the base-worktree
/// prefix), so the family_id is stable across runs and the paths read naturally in reports.
fn repo_relative(fam: &RefactorFamily, base_prefix: &Path) -> RefactorFamily {
    let mut fam = fam.clone();
    for loc in &mut fam.locations {
        repo_relative_loc(loc, base_prefix);
    }
    fam
}

fn repo_relative_loc(loc: &mut Loc, base_prefix: &Path) {
    loc.file = repo_relative_file(&loc.file, base_prefix);
    if let Some(parent) = &mut loc.enclosing_unit {
        parent.file = repo_relative_file(&parent.file, base_prefix);
        parent.refresh_unit_key();
    }
}

fn repo_relative_file(file: &str, base_prefix: &Path) -> String {
    canonical(Path::new(file))
        .strip_prefix(base_prefix)
        .map(|p| p.to_string_lossy().into_owned())
        .unwrap_or_else(|_| file.to_string())
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
    }
}

fn to_site_touch(loc: &Loc, touches_shared: Option<bool>) -> Site {
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
fn touches_shared_lines(
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
fn site_touched_loc(loc: &Loc, changed: &HashMap<String, Vec<(u32, u32)>>) -> bool {
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
