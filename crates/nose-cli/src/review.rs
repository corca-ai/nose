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
use nose_detect::{Loc, RefactorFamily};

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
}

/// A flagged family: a clone whose copies were edited apart in this change set. Locations
/// are repo-relative (the report navigates the real working tree).
struct Divergence {
    family_id: String,
    similarity: f64,
    complexity: usize,
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
}

pub(crate) fn cmd_review(args: ReviewArgs) -> Result<()> {
    let root = git_repo_root().context(
        "nose review needs a git repository — it compares the working tree to a git ref",
    )?;
    let changed = git_changed_ranges(&root, &args.base, &args.paths)?;
    if changed.is_empty() {
        println!("no changes vs `{}` — nothing to review.", args.base);
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

    // Flag families with *some but not all* members changed by the diff. Member paths are
    // normalized to repo-relative first, so the family_id is stable across runs (the base
    // worktree lives at a per-run temp path) and matches what `scan` and the ignore file use.
    let prefix = canonical(&base_tree.path);
    let mut flagged: Vec<Divergence> = Vec::new();
    for fam in &families {
        let fam = repo_relative(fam, &prefix);
        if ignore_set
            .as_ref()
            .is_some_and(|set| set.match_family(&fam).is_some())
        {
            continue;
        }
        let (changed_members, untouched): (Vec<&Loc>, Vec<&Loc>) = fam
            .locations
            .iter()
            .partition(|loc| site_touched_loc(loc, &changed));
        if !changed_members.is_empty() && !untouched.is_empty() {
            flagged.push(Divergence {
                family_id: crate::baseline::family_id(&fam),
                similarity: fam.mean_score,
                // Heaviest changed member's value-graph size — a cheap complexity proxy. A
                // small edit inside a computation-rich clone is the Krinke "critical change"
                // profile (the most likely un-propagated fix); an edit in a trivial clone is
                // likely benign.
                complexity: changed_members.iter().map(|l| l.sem).max().unwrap_or(0),
                changed: changed_members.iter().map(|l| to_site(l)).collect(),
                not_updated: untouched.iter().map(|l| to_site(l)).collect(),
            });
        }
    }
    // Most likely un-propagated fix first.
    flagged.sort_by(|a, b| {
        b.complexity
            .cmp(&a.complexity)
            .then(b.similarity.total_cmp(&a.similarity))
    });

    // base_tree is removed by Drop after we finish reading families.
    drop(base_tree);

    let changed_files = changed.len();
    match args.format {
        ReportFormat::Json => println!("{}", review_json(&flagged, &args.base, changed_files)?),
        ReportFormat::Sarif => println!("{}", review_sarif(&flagged)?),
        _ => print_review_human(&flagged, &args.base, changed_files, args.top.unwrap_or(30)),
    }

    if args.fail && !flagged.is_empty() {
        std::process::exit(1);
    }
    Ok(())
}

/// Clone the family with every member path made repo-relative (stripping the base-worktree
/// prefix), so the family_id is stable across runs and the paths read naturally in reports.
fn repo_relative(fam: &RefactorFamily, base_prefix: &Path) -> RefactorFamily {
    let mut fam = fam.clone();
    for loc in &mut fam.locations {
        loc.file = canonical(Path::new(&loc.file))
            .strip_prefix(base_prefix)
            .map(|p| p.to_string_lossy().into_owned())
            .unwrap_or_else(|_| loc.file.clone());
    }
    fam
}

fn to_site(loc: &Loc) -> Site {
    Site {
        file: loc.file.clone(),
        name: loc.name.clone(),
        start_line: loc.start_line,
        end_line: loc.end_line,
        lang: loc.lang.clone(),
    }
}

/// Does this member's (repo-relative) base span overlap a changed range of its file?
fn site_touched_loc(loc: &Loc, changed: &HashMap<String, Vec<(u32, u32)>>) -> bool {
    let Some(ranges) = changed.get(&loc.file) else {
        return false;
    };
    ranges
        .iter()
        .any(|&(s, e)| loc.start_line <= e && s <= loc.end_line)
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
    let text = String::from_utf8_lossy(&out.stdout);
    let mut map: HashMap<String, Vec<(u32, u32)>> = HashMap::new();
    let mut current: Option<String> = None;
    for line in text.lines() {
        if let Some(rest) = line.strip_prefix("--- ") {
            // "--- a/path" → base-side path; "--- /dev/null" (added file) → no base member
            current = rest.strip_prefix("a/").map(|p| p.to_string());
        } else if line.starts_with("@@") {
            if let (Some(file), Some((start, count))) = (&current, parse_hunk_old(line)) {
                let end = if count == 0 { start } else { start + count - 1 };
                map.entry(file.clone()).or_default().push((start, end));
            }
        }
    }
    Ok(map)
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
            "#{}  changed: {}  (sim {:.2})",
            i + 1,
            d.changed
                .iter()
                .map(site_label)
                .collect::<Vec<_>>()
                .join(", "),
            d.similarity
        );
        for s in &d.not_updated {
            println!("    not updated: {}", site_label(s));
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
        })
    };
    let items: Vec<_> = flagged
        .iter()
        .map(|d| {
            json!({
                "family_id": d.family_id,
                "similarity": d.similarity,
                "complexity": d.complexity,
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
        json!({
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
