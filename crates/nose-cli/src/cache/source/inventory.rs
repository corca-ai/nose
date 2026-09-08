use std::path::{Path, PathBuf};

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
