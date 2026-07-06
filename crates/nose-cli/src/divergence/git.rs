use super::*;
use std::collections::HashMap;

type LineRangesByFile = HashMap<String, Vec<(u32, u32)>>;
type ChangedRangesAndEntries = (LineRangesByFile, LineRangesByFile, Vec<DiffEntry>);

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

pub(super) fn git_repo_root() -> Result<PathBuf> {
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

pub(super) fn ensure_base_ref_available(root: &Path, base: &str) -> Result<()> {
    let commit_ref = format!("{base}^{{commit}}");
    let out = git(root, &["rev-parse", "--verify", "--quiet", &commit_ref])?;
    if !out.status.success() {
        anyhow::bail!("base ref `{base}` is not available locally; fetch it before running nose");
    }
    Ok(())
}

/// A throwaway worktree checked out at `base`, removed on drop.
pub(super) struct BaseWorktree {
    root: PathBuf,
    pub(super) path: PathBuf,
}

impl BaseWorktree {
    pub(super) fn create(root: &Path, base: &str) -> Result<Self> {
        // Unique per invocation (pid alone can be reused, racing parallel runs on the path).
        let nonce = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_nanos())
            .unwrap_or(0);
        let path =
            std::env::temp_dir().join(format!("nose-divergence-{}-{nonce}", std::process::id()));
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

/// Resolve user paths once, relative to the caller's cwd, into repo-relative
/// pathspecs. Git commands run with `-C <repo-root>`, so passing the raw user
/// path would reinterpret `src` from a nested cwd as `<repo>/src`.
pub(super) fn repo_relative_paths(paths: &[PathBuf], root: &Path) -> Vec<PathBuf> {
    paths
        .iter()
        .map(|p| match canonical(p).strip_prefix(root) {
            Ok(rel) if rel.as_os_str().is_empty() => PathBuf::from("."),
            Ok(rel) => rel.to_path_buf(),
            Err(_) => p.clone(),
        })
        .collect()
}

/// Re-root each repo-relative path under the base worktree so detection analyzes
/// the base copy of the same files the diff pathspec selected.
pub(super) fn reroot_paths(paths: &[PathBuf], base: &Path) -> Vec<PathBuf> {
    paths
        .iter()
        .map(|p| {
            if p.is_absolute() {
                p.clone()
            } else {
                base.join(p)
            }
        })
        .collect()
}

/// Changed line ranges on both sides plus per-file diff entries, all from one
/// `git diff --unified=0` invocation.
pub(super) fn git_changed_ranges_and_entries(
    root: &Path,
    base: &str,
    paths: &[PathBuf],
) -> Result<ChangedRangesAndEntries> {
    let out = git_diff(
        root,
        base,
        paths,
        &["--unified=0", "--no-color", "--find-renames=80%"],
    )?;
    let diff = String::from_utf8_lossy(&out.stdout);
    Ok((
        parse_old_side_ranges(&diff),
        parse_new_side_ranges(&diff),
        parse_patch_entries(&diff),
    ))
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(super) struct DiffEntry {
    pub(super) status: DiffStatus,
    pub(super) old_path: Option<String>,
    pub(super) new_path: Option<String>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) enum DiffStatus {
    Added,
    Copied,
    Deleted,
    Modified,
    Renamed,
    Other,
}

impl DiffStatus {
    pub(super) fn creates_current_path(self) -> bool {
        matches!(self, Self::Added | Self::Copied | Self::Renamed)
    }
}

fn git_diff(
    root: &Path,
    base: &str,
    paths: &[PathBuf],
    flags: &[&str],
) -> Result<std::process::Output> {
    let mut argv: Vec<String> = ["diff"].iter().map(|s| s.to_string()).collect();
    argv.extend(flags.iter().map(|s| (*s).to_string()));
    argv.push(base.to_string());
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
    Ok(out)
}

/// Parse `git diff --unified=0` text into base-side changed line ranges per repo-relative
/// path. Pure (no git) so it can be unit-tested against crafted diff output.
pub(super) fn parse_old_side_ranges(diff: &str) -> HashMap<String, Vec<(u32, u32)>> {
    parse_side_ranges(diff, DiffRangeSide::Old)
}

/// Parse `git diff --unified=0` text into current-side changed line ranges per
/// repo-relative path. Added files therefore carry ranges in the current tree,
/// while pure deletions do not.
pub(super) fn parse_new_side_ranges(diff: &str) -> HashMap<String, Vec<(u32, u32)>> {
    parse_side_ranges(diff, DiffRangeSide::New)
}

#[derive(Clone, Copy)]
enum DiffRangeSide {
    Old,
    New,
}

fn parse_side_ranges(diff: &str, side: DiffRangeSide) -> HashMap<String, Vec<(u32, u32)>> {
    let mut map: HashMap<String, Vec<(u32, u32)>> = HashMap::new();
    let mut old_file: Option<String> = None;
    let mut new_file: Option<String> = None;
    // `--- a/path` is the base-file header, but a *deleted* line whose content starts with
    // "-- " also renders as "--- …" in the hunk body. They're disambiguated structurally:
    // the file header sits in the per-file block before the first `@@`; once hunks begin a
    // "--- " line is body content. `diff --git` resets the block for the next file.
    let mut in_hunks = false;
    for line in diff.lines() {
        if line.starts_with("diff --git") {
            in_hunks = false;
            old_file = None;
            new_file = None;
        } else if !in_hunks && line.starts_with("--- ") {
            // "--- a/path" → base-side path; "--- /dev/null" (added file) → no base member
            old_file = parse_file_header_path(line, "--- ", "a/");
        } else if !in_hunks && line.starts_with("+++ ") {
            // "+++ b/path" → current-side path; "+++ /dev/null" (deleted file) → no current member
            new_file = parse_file_header_path(line, "+++ ", "b/");
        } else if line.starts_with("@@") {
            in_hunks = true;
            let (file, parsed) = match side {
                DiffRangeSide::Old => (old_file.as_ref(), parse_hunk_old(line)),
                DiffRangeSide::New => (new_file.as_ref(), parse_hunk_new(line)),
            };
            if let (Some(file), Some((start, count))) = (file, parsed) {
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

#[cfg(test)]
pub(super) fn parse_name_status(status: &str) -> Vec<DiffEntry> {
    status
        .lines()
        .filter_map(|line| {
            let parts = line.split('\t').collect::<Vec<_>>();
            let raw = parts.first().copied().unwrap_or_default();
            let code = raw.chars().next()?;
            match code {
                'A' => Some(DiffEntry {
                    status: DiffStatus::Added,
                    old_path: None,
                    new_path: parts.get(1).map(|p| (*p).to_string()),
                }),
                'C' => Some(DiffEntry {
                    status: DiffStatus::Copied,
                    old_path: parts.get(1).map(|p| (*p).to_string()),
                    new_path: parts.get(2).map(|p| (*p).to_string()),
                }),
                'D' => Some(DiffEntry {
                    status: DiffStatus::Deleted,
                    old_path: parts.get(1).map(|p| (*p).to_string()),
                    new_path: None,
                }),
                'M' => Some(DiffEntry {
                    status: DiffStatus::Modified,
                    old_path: parts.get(1).map(|p| (*p).to_string()),
                    new_path: parts.get(1).map(|p| (*p).to_string()),
                }),
                'R' => Some(DiffEntry {
                    status: DiffStatus::Renamed,
                    old_path: parts.get(1).map(|p| (*p).to_string()),
                    new_path: parts.get(2).map(|p| (*p).to_string()),
                }),
                _ => Some(DiffEntry {
                    status: DiffStatus::Other,
                    old_path: parts.get(1).map(|p| (*p).to_string()),
                    new_path: parts.get(1).map(|p| (*p).to_string()),
                }),
            }
        })
        .collect()
}

pub(super) fn parse_patch_entries(diff: &str) -> Vec<DiffEntry> {
    let mut entries = Vec::new();
    let mut current: Option<DiffEntry> = None;
    for line in diff.lines() {
        if let Some(rest) = line.strip_prefix("diff --git ") {
            finish_patch_entry(&mut entries, &mut current);
            let (old_path, new_path) = parse_diff_git_paths(rest);
            current = Some(DiffEntry {
                status: DiffStatus::Other,
                old_path,
                new_path,
            });
        } else if line.starts_with("new file mode") {
            if let Some(entry) = &mut current {
                entry.status = DiffStatus::Added;
                entry.old_path = None;
            }
        } else if line.starts_with("deleted file mode") {
            if let Some(entry) = &mut current {
                entry.status = DiffStatus::Deleted;
                entry.new_path = None;
            }
        } else if let Some(path) = line.strip_prefix("rename from ") {
            if let Some(entry) = &mut current {
                entry.status = DiffStatus::Renamed;
                entry.old_path = Some(path.to_string());
            }
        } else if let Some(path) = line.strip_prefix("rename to ") {
            if let Some(entry) = &mut current {
                entry.status = DiffStatus::Renamed;
                entry.new_path = Some(path.to_string());
            }
        } else if let Some(path) = line.strip_prefix("copy from ") {
            if let Some(entry) = &mut current {
                entry.status = DiffStatus::Copied;
                entry.old_path = Some(path.to_string());
            }
        } else if let Some(path) = line.strip_prefix("copy to ") {
            if let Some(entry) = &mut current {
                entry.status = DiffStatus::Copied;
                entry.new_path = Some(path.to_string());
            }
        } else if line.starts_with("--- ") {
            if let Some(entry) = &mut current {
                entry.old_path = parse_file_header_path(line, "--- ", "a/");
            }
        } else if line.starts_with("+++ ") {
            if let Some(entry) = &mut current {
                entry.new_path = parse_file_header_path(line, "+++ ", "b/");
            }
        }
    }
    finish_patch_entry(&mut entries, &mut current);
    entries
}

fn parse_file_header_path(line: &str, marker: &str, prefix: &str) -> Option<String> {
    let path = line.strip_prefix(marker)?;
    if path == "/dev/null" || path.starts_with("/dev/null\t") {
        return None;
    }
    path.strip_prefix(prefix)
        .map(trim_diff_path_metadata)
        .map(ToOwned::to_owned)
}

fn trim_diff_path_metadata(path: &str) -> &str {
    path.split_once('\t').map_or(path, |(path, _)| path)
}

fn parse_diff_git_paths(rest: &str) -> (Option<String>, Option<String>) {
    let Some(rest) = rest.strip_prefix("a/") else {
        return (None, None);
    };
    rest.split_once(" b/").map_or((None, None), |(old, new)| {
        (Some(old.to_string()), Some(new.to_string()))
    })
}

fn finish_patch_entry(entries: &mut Vec<DiffEntry>, current: &mut Option<DiffEntry>) {
    if let Some(mut entry) = current.take() {
        if entry.status == DiffStatus::Other {
            entry.status = match (&entry.old_path, &entry.new_path) {
                (None, Some(_)) => DiffStatus::Added,
                (Some(_), None) => DiffStatus::Deleted,
                (Some(old), Some(new)) if old != new => DiffStatus::Renamed,
                _ => DiffStatus::Modified,
            };
        }
        entries.push(entry);
    }
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

/// Parse the current-side range from a hunk header `@@ -a,b +c,d @@ ...` → `(c, d)`,
/// where a missing `,d` means a count of 1.
fn parse_hunk_new(line: &str) -> Option<(u32, u32)> {
    let after_plus = line.split('+').nth(1)?;
    let spec = after_plus.split(' ').next()?.trim();
    let mut parts = spec.split(',');
    let start: u32 = parts.next()?.parse().ok()?;
    let count: u32 = match parts.next() {
        Some(c) => c.parse().ok()?,
        None => 1,
    };
    Some((start, count))
}

/// Best-effort absolute, symlink-resolved path.
pub(super) fn canonical(p: &Path) -> PathBuf {
    std::fs::canonicalize(p).unwrap_or_else(|_| p.to_path_buf())
}
