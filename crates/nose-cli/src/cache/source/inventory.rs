use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};
use std::process::Command;

struct LogicalRoot {
    lexical_base: PathBuf,
    canonical_base: PathBuf,
}

pub(super) struct LogicalRoots {
    roots: Vec<LogicalRoot>,
    cwd: PathBuf,
}

impl LogicalRoots {
    pub(super) fn new(roots: &[&Path]) -> Self {
        let cwd = std::env::current_dir().unwrap_or_default();
        Self {
            roots: roots
                .iter()
                .map(|root| {
                    let lexical = if root.is_absolute() {
                        root.to_path_buf()
                    } else {
                        cwd.join(root)
                    };
                    let canonical = std::fs::canonicalize(root).unwrap_or_else(|_| lexical.clone());
                    LogicalRoot {
                        lexical_base: root_base(lexical),
                        canonical_base: root_base(canonical),
                    }
                })
                .collect(),
            cwd,
        }
    }

    pub(super) fn path(&self, path: &Path) -> String {
        let lexical = if path.is_absolute() {
            path.to_path_buf()
        } else {
            self.cwd.join(path)
        };
        for (index, root) in self.roots.iter().enumerate() {
            if let Ok(relative) = lexical.strip_prefix(&root.lexical_base) {
                return format!("{index}:{}", relative.to_string_lossy());
            }
        }
        let canonical = std::fs::canonicalize(path).unwrap_or(lexical);
        for (index, root) in self.roots.iter().enumerate() {
            if let Ok(relative) = canonical.strip_prefix(&root.canonical_base) {
                return format!("{index}:{}", relative.to_string_lossy());
            }
        }
        canonical.to_string_lossy().to_string()
    }
}

fn root_base(root: PathBuf) -> PathBuf {
    if root.is_file() {
        root.parent().unwrap_or(&root).to_path_buf()
    } else {
        root
    }
}

struct GitInventory {
    root: PathBuf,
    tracked: BTreeMap<PathBuf, String>,
    dirty: BTreeSet<PathBuf>,
}

pub(super) struct GitCatalog {
    inventories: Vec<GitInventory>,
    cwd: PathBuf,
}

impl GitCatalog {
    pub(super) fn new(roots: &[&Path]) -> Self {
        let mut git_roots: BTreeMap<PathBuf, BTreeSet<PathBuf>> = BTreeMap::new();
        for root in roots {
            let base = if root.is_file() {
                root.parent().unwrap_or(root)
            } else {
                root
            };
            let Some(git_root) = find_git_root(base) else {
                continue;
            };
            let scope = std::fs::canonicalize(base)
                .ok()
                .and_then(|root| root.strip_prefix(&git_root).ok().map(Path::to_path_buf))
                .unwrap_or_default();
            git_roots.entry(git_root).or_default().insert(scope);
        }
        Self {
            inventories: git_roots
                .into_iter()
                .filter_map(|(root, scopes)| GitInventory::load(root, &scopes))
                .collect(),
            cwd: std::env::current_dir().unwrap_or_default(),
        }
    }

    pub(super) fn clean_blob(&self, path: &Path) -> Option<&str> {
        let lexical = if path.is_absolute() {
            path.to_path_buf()
        } else {
            self.cwd.join(path)
        };
        for git in &self.inventories {
            if let Ok(relative) = lexical.strip_prefix(&git.root) {
                return (!git.dirty.contains(relative))
                    .then(|| git.tracked.get(relative).map(String::as_str))
                    .flatten();
            }
        }
        let canonical = std::fs::canonicalize(path).ok()?;
        self.inventories.iter().find_map(|git| {
            let relative = canonical.strip_prefix(&git.root).ok()?;
            (!git.dirty.contains(relative))
                .then(|| git.tracked.get(relative).map(String::as_str))
                .flatten()
        })
    }
}

impl GitInventory {
    fn load(root: PathBuf, scopes: &BTreeSet<PathBuf>) -> Option<Self> {
        let root = std::fs::canonicalize(root).ok()?;
        let mut listed = Command::new("git");
        listed.args(["-C", &root.to_string_lossy(), "ls-files", "--stage", "-z"]);
        append_scopes(&mut listed, scopes);
        let listed = listed.output().ok()?;
        if !listed.status.success() {
            return None;
        }
        if listed.stdout.is_empty() {
            return Some(Self {
                root,
                tracked: BTreeMap::new(),
                dirty: BTreeSet::new(),
            });
        }
        let mut status = Command::new("git");
        status.args([
            "-C",
            &root.to_string_lossy(),
            "status",
            "--porcelain=v1",
            "-z",
            "--untracked-files=no",
        ]);
        append_scopes(&mut status, scopes);
        let status = status.output().ok()?;
        if !listed.status.success() || !status.status.success() {
            return None;
        }
        let mut tracked = BTreeMap::new();
        for record in listed
            .stdout
            .split(|byte| *byte == 0)
            .filter(|row| !row.is_empty())
        {
            let Some(tab) = record.iter().position(|byte| *byte == b'\t') else {
                continue;
            };
            let header = String::from_utf8_lossy(&record[..tab]);
            let mut fields = header.split_whitespace();
            let _mode = fields.next();
            let Some(oid) = fields.next() else { continue };
            if fields.next() != Some("0") {
                continue;
            }
            tracked.insert(
                PathBuf::from(String::from_utf8_lossy(&record[tab + 1..]).as_ref()),
                oid.to_owned(),
            );
        }
        let mut dirty = BTreeSet::new();
        let records = status
            .stdout
            .split(|byte| *byte == 0)
            .filter(|row| !row.is_empty())
            .collect::<Vec<_>>();
        let mut index = 0;
        while index < records.len() {
            let record = records[index];
            if record.len() >= 4 {
                dirty.insert(PathBuf::from(
                    String::from_utf8_lossy(&record[3..]).as_ref(),
                ));
                if matches!(record[0], b'R' | b'C') || matches!(record[1], b'R' | b'C') {
                    index += 1;
                    if let Some(old) = records.get(index) {
                        dirty.insert(PathBuf::from(String::from_utf8_lossy(old).as_ref()));
                    }
                }
            }
            index += 1;
        }
        Some(Self {
            root,
            tracked,
            dirty,
        })
    }
}

fn find_git_root(base: &Path) -> Option<PathBuf> {
    let canonical = std::fs::canonicalize(base).ok()?;
    let start = if canonical.is_file() {
        canonical.parent()?
    } else {
        canonical.as_path()
    };
    start
        .ancestors()
        .find(|ancestor| ancestor.join(".git").exists())
        .map(Path::to_path_buf)
}

fn append_scopes(command: &mut Command, scopes: &BTreeSet<PathBuf>) {
    if scopes.iter().any(|scope| scope.as_os_str().is_empty()) {
        return;
    }
    command.arg("--");
    command.args(scopes);
}
