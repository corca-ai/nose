//! `nose review` — flag clone families edited inconsistently in a change set.
//!
//! Given a git ref (`--base`), `review` detects clone families **at that base** (where every
//! copy still matches), finds which lines the diff changed, and flags every family where
//! *some* copies were edited but *siblings were not* — a likely un-propagated edit ("you
//! changed X; its clone Y was not updated"). This is the divergent-edit (Kim *Inconsistent
//! Change*) predicate applied to one diff.
//!
//! Detection runs at the base, not the working tree, on purpose: an edit can push a copy out
//! of its clone family (a fix changes its shape), so it would be invisible in the current
//! tree. At the base the family is still intact, and the diff tells us which member moved.
//!
//! The structural signal is a candidate surfacer, not a proof: review the flagged siblings
//! yourself.

use anyhow::{Context, Result};
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::process::Command;

use crate::{ReportFormat, ScanMode};
use nose_detect::{EnclosingUnit, FragmentKind, Loc, RefactorFamily};

pub(crate) struct ReviewArgs {
    pub paths: Vec<PathBuf>,
    pub base: String,
    pub mode: Vec<ScanMode>,
    pub min_size: Option<usize>,
    pub min_lines: Option<u32>,
    pub exclude: Vec<String>,
    pub config: Option<PathBuf>,
    pub ignore_file: Option<PathBuf>,
    pub format: ReportFormat,
    pub top: Option<usize>,
    pub fail: bool,
    pub fail_on: ReviewFailOn,
}

/// What `--fail` fires on. The default is the #245 conservative tier: only
/// findings where the diff PROVABLY touches lines a changed member shares with
/// its un-updated sibling (§BR measured span-overlap firing at 33% of merged
/// PRs with ~4% top-1 precision — a gate that cries wolf gets disabled).
#[derive(Clone, Copy, PartialEq, Eq, Default, clap::ValueEnum)]
pub(crate) enum ReviewFailOn {
    /// Fire only on shared-logic-touching findings (the conservative gate).
    #[default]
    SharedLogic,
    /// Fire on any flagged finding (the pre-#245 span-overlap behavior).
    Any,
}

/// A flagged family: a clone whose copies were edited apart in this change set. Locations
/// are repo-relative (the report navigates the real working tree).
struct Divergence {
    family_id: String,
    similarity: f64,
    hazard: f64,
    review_priority: u8,
    complexity: usize,
    /// Family scope: `prod` / `test` / `mixed` (test scaffolding fires differently).
    scope: &'static str,
    /// The family's equivalence-witness kind (`exact-value-graph`,
    /// `copy-paste-run`, `shared-sub-dag`, `structural-similarity`).
    witness_kind: Option<&'static str>,
    /// The #245 conservative gate verdict: some changed member PROVABLY touches
    /// lines it shares with an un-updated sibling. `--fail` fires only on these;
    /// `--fail-on any` restores span-overlap firing.
    fire_eligible: bool,
    /// The near family's graded equivalence witness (#315), when present — evidence
    /// for the consumer to judge a fire: a clean `equal_modulo_holes` family is a
    /// strong missed-propagation candidate, while `referent_mismatches` /
    /// `decorator-differs` mark a family whose copies are not really the same logic
    /// (a likely false fire). It does NOT gate `fire_eligible` (that would risk
    /// dropping a genuine shared-body propagation).
    graded: Option<nose_detect::GradedWitness>,
    /// Members whose base span was changed by the diff (the edit landed here).
    changed: Vec<Site>,
    /// Sibling members the change did *not* touch (where it may be missing).
    not_updated: Vec<Site>,
}

#[derive(Clone)]
struct Site {
    file: String,
    name: Option<String>,
    start_line: u32,
    end_line: u32,
    lang: String,
    kind: nose_il::UnitKind,
    span_lines: u32,
    span_tokens: usize,
    is_fragment: bool,
    fragment_kind: Option<FragmentKind>,
    reason_code: Option<&'static str>,
    enclosing_unit: Option<EnclosingUnit>,
    /// For CHANGED sites: does the diff touch lines this member shares with an
    /// un-updated sibling? `Some(false)` = the edit stayed inside this member's
    /// varying spots; `None` = unprovable (unreadable source / capped diff) or a
    /// not-updated site.
    touches_shared: Option<bool>,
}

pub(crate) fn cmd_review(args: ReviewArgs) -> Result<()> {
    let root = git_repo_root().context(
        "nose review needs a git repository — it compares the working tree to a git ref",
    )?;
    let changed = git_changed_ranges(&root, &args.base, &args.paths)?;
    if changed.is_empty() {
        // Nothing reviewable (e.g. an adds-only diff), but the machine formats
        // must still emit their contract — a JSON consumer parses stdout.
        match args.format {
            ReportFormat::Json => println!("{}", review_json(&[], &args.base, 0)?),
            ReportFormat::Sarif => println!("{}", review_sarif(&[])?),
            _ => println!("no changes vs `{}` — nothing to review.", args.base),
        }
        return Ok(());
    }

    // Detect clone families at the base, where every copy is still intact. A temporary
    // worktree gives the base tree on disk without disturbing the user's working tree.
    let base_tree = BaseWorktree::create(&root, &args.base)?;
    let cfg = crate::config::load_scan(args.config.as_deref())?;
    let mut exclude = cfg.exclude.clone();
    exclude.extend(args.exclude.iter().cloned());
    let min_tokens = args.min_size.or(cfg.min_size).unwrap_or(24);
    let min_lines = args.min_lines.or(cfg.min_lines).unwrap_or(5);
    let base_paths = reroot_paths(&args.paths, &root, &base_tree.path);
    let families = crate::detect_families(
        &base_paths,
        &exclude,
        args.mode,
        cfg.mode,
        min_tokens,
        min_lines,
    )?;

    // Structured ignores suppress reviewed-and-accepted divergences (same nose.ignore.json
    // as `scan`), so an intentional fork doesn't re-fail every PR.
    let ignore_set = crate::ignores::load_for_scan(args.ignore_file.as_deref())?;
    if let Some(set) = &ignore_set {
        set.warn_expired();
    }

    // Normalization knobs for the per-flagged-family graded-witness enrichment; must
    // match how `detect_families` lowered (cfg_norm/dce/block_units default), so the
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

    let changed_files = changed.len();
    match args.format {
        ReportFormat::Json => println!("{}", review_json(&flagged, &args.base, changed_files)?),
        ReportFormat::Sarif => println!("{}", review_sarif(&flagged)?),
        _ => print_review_human(&flagged, &args.base, changed_files, args.top.unwrap_or(30)),
    }

    if args.fail {
        let fires = match args.fail_on {
            ReviewFailOn::SharedLogic => flagged.iter().any(|d| d.fire_eligible),
            ReviewFailOn::Any => !flagged.is_empty(),
        };
        if fires {
            std::process::exit(1);
        }
    }
    Ok(())
}

/// Flag families with *some but not all* members changed by the diff, most likely
/// un-propagated fix first. Member paths are normalized to repo-relative first, so the
/// family_id is stable across runs (the base worktree lives at a per-run temp path) and
/// matches what `scan` and the ignore file use.
fn flag_divergences(
    families: &[RefactorFamily],
    ignore_set: Option<&crate::ignores::IgnoreSet>,
    changed: &HashMap<String, Vec<(u32, u32)>>,
    base_root: &Path,
    enrich_opts: &nose_detect::DetectOptions,
) -> Vec<Divergence> {
    let prefix = canonical(base_root);
    let mut lines = crate::FileLineCache::default();
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
            crate::enrich_graded_witnesses(std::slice::from_mut(&mut abs), enrich_opts);
            abs.witness.and_then(|w| w.graded)
        };
        // The #245 fire policy input: does the diff touch lines this changed
        // member SHARES with an un-updated sibling (its span minus the
        // varying spots)? §BR measured 51% of review false-fires as
        // span-overlap-but-not-shared-logic; a gate fires only on proof.
        let witness_kind = fam.witness.as_ref().map(|w| w.kind);
        let touches: Vec<Option<bool>> = changed_members
            .iter()
            .map(|c| {
                touches_shared_lines(c, &untouched, witness_kind, base_root, &mut lines, changed)
            })
            .collect();
        // All-test families are review context, not gate material: §BG-audit
        // found test variants legitimately diverge, and on the §BR labels the
        // scope term doubled gate precision at zero true-positive cost.
        let fire_eligible = touches.contains(&Some(true)) && fam.scope != "test";
        flagged.push(Divergence {
            family_id: crate::baseline::family_id(&fam),
            similarity: fam.mean_score,
            hazard: fam.hazard(),
            review_priority: review_priority(&fam, &changed_members, &untouched),
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
        b.review_priority
            .cmp(&a.review_priority)
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

fn to_site(loc: &Loc) -> Site {
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
    lines: &mut crate::FileLineCache,
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
        (s.lang == member.lang).then(|| crate::varying_spots_of(&a, &abs(s), lines))?
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

fn review_priority(fam: &RefactorFamily, changed: &[&Loc], untouched: &[&Loc]) -> u8 {
    let any_fragment = changed.iter().chain(untouched).any(|loc| loc.is_fragment);
    if !any_fragment {
        return 0;
    }
    let any_enclosing = changed
        .iter()
        .chain(untouched)
        .any(|loc| loc.enclosing_unit.is_some());
    match fam.recommended_surface() {
        "review" => 3,
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
fn ranges_touch(ranges: &[(u32, u32)], start: u32, end: u32) -> bool {
    ranges.iter().any(|&(s, e)| start <= e && s <= end)
}

// ---- git plumbing ---------------------------------------------------------------

/// A git command rooted at `root`, with inherited git env vars cleared so it always
/// operates on `root`'s repo — not on a `GIT_DIR`/`GIT_WORK_TREE` set by an outer hook.
fn git(root: &Path, args: &[&str]) -> Result<std::process::Output> {
    git_cmd()
        .arg("-C")
        .arg(root)
        .args(args)
        .output()
        .context("failed to run git (is it installed and on PATH?)")
}

fn git_cmd() -> Command {
    let mut cmd = Command::new("git");
    cmd.env_remove("GIT_DIR")
        .env_remove("GIT_WORK_TREE")
        .env_remove("GIT_INDEX_FILE")
        .env_remove("GIT_OBJECT_DIRECTORY")
        .env_remove("GIT_COMMON_DIR");
    cmd
}

fn git_repo_root() -> Result<PathBuf> {
    let out = git_cmd()
        .args(["rev-parse", "--show-toplevel"])
        .output()
        .context("failed to run git (is it installed and on PATH?)")?;
    if !out.status.success() {
        anyhow::bail!("not inside a git repository");
    }
    Ok(PathBuf::from(
        String::from_utf8_lossy(&out.stdout).trim().to_string(),
    ))
}

/// A throwaway worktree checked out at `base`, removed on drop.
struct BaseWorktree {
    root: PathBuf,
    path: PathBuf,
}

impl BaseWorktree {
    fn create(root: &Path, base: &str) -> Result<Self> {
        // Unique per invocation (pid alone can be reused, racing parallel runs on the path).
        let nonce = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_nanos())
            .unwrap_or(0);
        let path = std::env::temp_dir().join(format!("nose-review-{}-{nonce}", std::process::id()));
        let _ = std::fs::remove_dir_all(&path);
        // Clear any stale registration from a previously-killed run at this path.
        let _ = git(root, &["worktree", "prune"]);
        let out = git(
            root,
            &[
                "worktree",
                "add",
                "--detach",
                "--quiet",
                &path.to_string_lossy(),
                base,
            ],
        )?;
        if !out.status.success() {
            anyhow::bail!(
                "could not check out base `{base}`: {}",
                String::from_utf8_lossy(&out.stderr).trim()
            );
        }
        Ok(Self {
            root: root.to_path_buf(),
            path,
        })
    }
}

impl Drop for BaseWorktree {
    fn drop(&mut self) {
        let _ = git(
            &self.root,
            &[
                "worktree",
                "remove",
                "--force",
                &self.path.to_string_lossy(),
            ],
        );
        let _ = std::fs::remove_dir_all(&self.path);
    }
}

/// Re-root each user path under the base worktree (so we scan the base copy of the same
/// files). A path outside the repo is passed through unchanged.
fn reroot_paths(paths: &[PathBuf], root: &Path, base: &Path) -> Vec<PathBuf> {
    paths
        .iter()
        .map(|p| match canonical(p).strip_prefix(root) {
            Ok(rel) => base.join(rel),
            Err(_) => p.clone(),
        })
        .collect()
}

/// Changed line ranges on the **base (old) side**, keyed by repo-relative path.
fn git_changed_ranges(
    root: &Path,
    base: &str,
    paths: &[PathBuf],
) -> Result<HashMap<String, Vec<(u32, u32)>>> {
    let mut argv: Vec<String> = ["diff", "--unified=0", "--no-color", base]
        .iter()
        .map(|s| s.to_string())
        .collect();
    if !paths.is_empty() {
        argv.push("--".into());
        for p in paths {
            argv.push(p.to_string_lossy().into_owned());
        }
    }
    let refs: Vec<&str> = argv.iter().map(|s| s.as_str()).collect();
    let out = git(root, &refs)?;
    if !out.status.success() {
        anyhow::bail!(
            "`git diff {base}` failed: {}",
            String::from_utf8_lossy(&out.stderr).trim()
        );
    }
    Ok(parse_old_side_ranges(&String::from_utf8_lossy(&out.stdout)))
}

/// Parse `git diff --unified=0` text into base-side changed line ranges per repo-relative
/// path. Pure (no git) so it can be unit-tested against crafted diff output.
fn parse_old_side_ranges(diff: &str) -> HashMap<String, Vec<(u32, u32)>> {
    let mut map: HashMap<String, Vec<(u32, u32)>> = HashMap::new();
    let mut current: Option<String> = None;
    // `--- a/path` is the base-file header, but a *deleted* line whose content starts with
    // "-- " also renders as "--- …" in the hunk body. They're disambiguated structurally:
    // the file header sits in the per-file block before the first `@@`; once hunks begin a
    // "--- " line is body content. `diff --git` resets the block for the next file.
    let mut in_hunks = false;
    for line in diff.lines() {
        if line.starts_with("diff --git") {
            in_hunks = false;
            current = None;
        } else if !in_hunks && line.starts_with("--- ") {
            // "--- a/path" → base-side path; "--- /dev/null" (added file) → no base member
            current = line
                .strip_prefix("--- ")
                .and_then(|r| r.strip_prefix("a/"))
                .map(|p| p.to_string());
        } else if line.starts_with("@@") {
            in_hunks = true;
            if let (Some(file), Some((start, count))) = (&current, parse_hunk_old(line)) {
                // count == 0 is a pure insertion *after* base line `start` (no base line
                // changed): encode the gap as `(start+1, start)` so it touches only members
                // that straddle it, not one that merely ends at `start`.
                let range = if count == 0 {
                    (start + 1, start)
                } else {
                    (start, start + count - 1)
                };
                map.entry(file.clone()).or_default().push(range);
            }
        }
    }
    map
}

/// Parse the old-side range from a hunk header `@@ -a,b +c,d @@ ...` → `(a, b)`, where a
/// missing `,b` means a count of 1.
fn parse_hunk_old(line: &str) -> Option<(u32, u32)> {
    let after_minus = line.split('-').nth(1)?;
    let spec = after_minus.split([' ', '+']).next()?.trim();
    let mut parts = spec.split(',');
    let start: u32 = parts.next()?.parse().ok()?;
    let count: u32 = match parts.next() {
        Some(c) => c.parse().ok()?,
        None => 1,
    };
    Some((start, count))
}

/// Best-effort absolute, symlink-resolved path.
fn canonical(p: &Path) -> PathBuf {
    std::fs::canonicalize(p).unwrap_or_else(|_| p.to_path_buf())
}

// ---- output ---------------------------------------------------------------------

fn site_label(s: &Site) -> String {
    match &s.name {
        Some(n) if !n.is_empty() => format!("{} ({}:{}-{})", n, s.file, s.start_line, s.end_line),
        _ => format!("{}:{}-{}", s.file, s.start_line, s.end_line),
    }
}

fn fragment_context(s: &Site) -> Option<String> {
    if !s.is_fragment {
        return None;
    }
    let kind = s
        .fragment_kind
        .map(|k| {
            k.reason_code()
                .strip_prefix("exact-")
                .unwrap_or(k.reason_code())
                .to_string()
        })
        .unwrap_or_else(|| "fragment".to_string());
    let reason = s.reason_code.unwrap_or("unknown");
    let parent = s.enclosing_unit.as_ref().map(|p| {
        let name = p
            .name
            .as_deref()
            .filter(|n| !n.is_empty())
            .map(|n| format!(" `{n}`"))
            .unwrap_or_default();
        format!(
            " in {:?}{name} {}:{}-{}",
            p.kind, p.file, p.start_line, p.end_line
        )
    });
    Some(format!(
        "{kind} fragment ({reason}){}",
        parent.unwrap_or_default()
    ))
}

fn print_review_human(flagged: &[Divergence], base: &str, changed_files: usize, top: usize) {
    let plural = |n: usize, s: &str| {
        if n == 1 {
            s.to_string()
        } else {
            format!("{s}s")
        }
    };
    println!(
        "reviewing changes vs `{base}` · {changed_files} {} changed",
        plural(changed_files, "file")
    );
    if flagged.is_empty() {
        println!("\n✓ no clone families were changed inconsistently.");
        return;
    }
    let families = if flagged.len() == 1 {
        "family"
    } else {
        "families"
    };
    println!(
        "\n⚠ {} clone {families} changed inconsistently — a copy was edited but its sibling(s) were not:\n",
        flagged.len(),
    );
    for (i, d) in flagged.iter().enumerate() {
        if top != 0 && i >= top {
            break;
        }
        println!(
            "#{}  changed: {}  (sim {:.2}){}",
            i + 1,
            d.changed
                .iter()
                .map(site_label)
                .collect::<Vec<_>>()
                .join(", "),
            d.similarity,
            if d.fire_eligible {
                "  [gate: touches shared lines]"
            } else {
                ""
            }
        );
        for s in &d.changed {
            if let Some(context) = fragment_context(s) {
                println!("      changed context: {context}");
            }
        }
        for s in &d.not_updated {
            println!("    not updated: {}", site_label(s));
            if let Some(context) = fragment_context(s) {
                println!("      context: {context}");
            }
        }
        println!("    → review whether the change should also apply to the sibling(s)\n");
    }
    if top != 0 && flagged.len() > top {
        println!("({} more — pass --top 0 to show all)", flagged.len() - top);
    }
}

fn review_json(flagged: &[Divergence], base: &str, changed_files: usize) -> Result<String> {
    use serde_json::json;
    let site = |s: &Site| {
        json!({
            "file": s.file, "name": s.name,
            "start_line": s.start_line, "end_line": s.end_line, "lang": s.lang,
            "kind": s.kind,
            "span_lines": s.span_lines,
            "span_tokens": s.span_tokens,
            "is_fragment": s.is_fragment,
            "fragment_kind": s.fragment_kind,
            "reason_code": s.reason_code,
            "enclosing_unit": s.enclosing_unit,
            "touches_shared": s.touches_shared,
        })
    };
    let items: Vec<_> = flagged
        .iter()
        .map(|d| {
            json!({
                "family_id": d.family_id,
                "similarity": d.similarity,
                "complexity": d.complexity,
                "scope": d.scope,
                "witness_kind": d.witness_kind,
                "fire_eligible": d.fire_eligible,
                "graded": d.graded,
                "changed": d.changed.iter().map(&site).collect::<Vec<_>>(),
                "not_updated": d.not_updated.iter().map(&site).collect::<Vec<_>>(),
            })
        })
        .collect();
    Ok(serde_json::to_string_pretty(&json!({
        "schema_version": 1,
        "tool_version": env!("CARGO_PKG_VERSION"),
        "base": base,
        "changed_files": changed_files,
        "inconsistent_families": flagged.len(),
        "findings": items,
    }))?)
}

fn review_sarif(flagged: &[Divergence]) -> Result<String> {
    use serde_json::json;
    let phys = |s: &Site| {
        let message = fragment_context(s).unwrap_or_else(|| site_label(s));
        json!({
            "message": { "text": message },
            "physicalLocation": {
                "artifactLocation": { "uri": s.file },
                "region": { "startLine": s.start_line, "endLine": s.end_line }
            }
        })
    };
    // The SARIF *location* is each un-updated sibling (where a fix may be missing), so a CI
    // annotation lands on the copy the change skipped; the changed copies are related.
    let results: Vec<_> = flagged
        .iter()
        .map(|d| {
            let changed = d
                .changed
                .iter()
                .map(site_label)
                .collect::<Vec<_>>()
                .join(", ");
            json!({
                "ruleId": "unpropagated-change",
                "level": "warning",
                "message": { "text": format!(
                    "A clone of this code was changed ({changed}) but this copy was not — \
                     review whether the change should propagate here."
                ) },
                "locations": d.not_updated.iter().map(&phys).collect::<Vec<_>>(),
                "relatedLocations": d.changed.iter().map(&phys).collect::<Vec<_>>(),
                "properties": { "family_id": d.family_id },
            })
        })
        .collect();
    let run = json!({
        "tool": { "driver": {
            "name": "nose",
            "informationUri": "https://github.com/corca-ai/nose",
            "version": env!("CARGO_PKG_VERSION"),
            "rules": [{
                "id": "unpropagated-change",
                "name": "UnpropagatedChange",
                "shortDescription": { "text": "A clone was changed but a sibling copy was not" }
            }]
        }},
        "results": results,
        "properties": { "inconsistent_families": flagged.len() },
    });
    Ok(serde_json::to_string_pretty(&json!({
        "version": "2.1.0",
        "$schema": "https://json.schemastore.org/sarif-2.1.0.json",
        "runs": [run],
    }))?)
}

#[cfg(test)]
mod tests {
    use super::*;
    use nose_detect::{LineSpan, LocInit};

    // `git diff --unified=0` for: base "keep1\n-- marker\nkeep2a\nkeep2b\nzzz\n"
    // → new "KEEP1\nkeep2a\nkeep2b\nZZZ\n". The deleted "-- marker" line shows in the body
    // as "--- marker", which must NOT be parsed as a "--- a/path" file header.
    const DIFF_WITH_DASHDASH_CONTENT: &str = "\
diff --git a/f.txt b/f.txt
index 1111111..2222222 100644
--- a/f.txt
+++ b/f.txt
@@ -1,2 +1 @@
-keep1
--- marker
+KEEP1
@@ -5 +4 @@
-zzz
+ZZZ
";

    #[test]
    fn parse_ignores_deleted_content_lines_that_look_like_headers() {
        let ranges = parse_old_side_ranges(DIFF_WITH_DASHDASH_CONTENT);
        let f = ranges.get("f.txt").expect("f.txt has changed ranges");
        assert!(f.contains(&(1, 2)), "first hunk: {f:?}");
        assert!(
            f.contains(&(5, 5)),
            "second hunk must survive the `--- marker` body line: {f:?}"
        );
        assert_eq!(
            ranges.len(),
            1,
            "no phantom file key from a content line: {ranges:?}"
        );
    }

    #[test]
    fn pure_insertion_does_not_touch_a_member_ending_at_the_insertion_point() {
        // Insert a line after base line 1: `@@ -1,0 +2 @@`. The insertion sits *between*
        // base lines 1 and 2, so a member occupying only line 1 was not edited.
        let diff =
            "diff --git a/g.txt b/g.txt\n--- a/g.txt\n+++ b/g.txt\n@@ -1,0 +2 @@\n+inserted\n";
        let r = parse_old_side_ranges(diff);
        let ranges = r.get("g.txt").expect("g.txt range");
        assert!(
            !ranges_touch(ranges, 1, 1),
            "a member ending at the insertion point is not touched: {ranges:?}"
        );
        assert!(
            ranges_touch(ranges, 1, 3),
            "a member straddling the insertion gap IS touched: {ranges:?}"
        );
    }

    fn fragment_loc(file: &str, start: u32, end: u32) -> Loc {
        let mut loc = Loc::new(LocInit {
            file: file.into(),
            source_span: LineSpan::new(start, end),
            lang: "rust".into(),
            kind: nose_il::UnitKind::Block,
            name: None,
            sem: 4,
            span_tokens: 8,
        });
        loc.is_fragment = true;
        loc.fragment_kind = Some(FragmentKind::ConditionalGuard);
        loc.reason_code = Some(FragmentKind::ConditionalGuard.reason_code());
        loc.enclosing_unit = Some(EnclosingUnit {
            file: file.into(),
            start_line: 1,
            end_line: 20,
            kind: nose_il::UnitKind::Function,
            name: Some("owner".into()),
            unit_key: String::new(),
        });
        loc.enclosing_unit.as_mut().unwrap().refresh_unit_key();
        loc
    }

    fn review_family(locs: Vec<Loc>) -> RefactorFamily {
        RefactorFamily {
            value: 1.0,
            members: locs.len(),
            files: locs.len(),
            modules: 1,
            languages: 1,
            mean_score: 1.0,
            mean_lines: 4,
            dup_lines: 4,
            shared_lines: 4,
            params: 0,
            shared_weight: 4.0,
            locations: locs,
            mean_sem: 4.0,
            scope: "prod",
            discount: 1.0,
            abstraction_witness: None,
            witness: None,
            varying_spots: Vec::new(),
            semantic_laws: Vec::new(),
        }
    }

    #[test]
    fn fragment_context_names_enclosing_unit() {
        let site = to_site(&fragment_loc("src/a.rs", 8, 9));
        let context = fragment_context(&site).expect("fragment context");
        assert!(context.contains("conditional-guard fragment"));
        assert!(context.contains("`owner`"));
        assert!(context.contains("src/a.rs:1-20"));
    }

    #[test]
    fn review_priority_promotes_fragment_surface() {
        let changed = fragment_loc("src/a.rs", 8, 11);
        let sibling = fragment_loc("src/b.rs", 8, 11);
        let family = review_family(vec![changed.clone(), sibling.clone()]);
        assert_eq!(family.recommended_surface(), "review");
        assert_eq!(
            review_priority(&family, &[&changed], &[&sibling]),
            3,
            "review-surface fragment hazards should rank before generic clone divergences"
        );
    }
}
