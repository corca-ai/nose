use crate::path_utils::absolute_lexical;
use anyhow::{Context, Result};
use ignore::gitignore::{Gitignore, GitignoreBuilder};
use std::path::{Path, PathBuf};

pub(super) struct PathMatcher {
    matcher: Gitignore,
    cwd: PathBuf,
    base: PathBuf,
    canonical_base: Option<PathBuf>,
}

impl PathMatcher {
    pub(super) fn new(index: usize, patterns: &[String], base: &Path) -> Result<Option<Self>> {
        if patterns.is_empty() {
            return Ok(None);
        }
        let mut builder = GitignoreBuilder::new(".");
        builder.allow_unclosed_class(false);
        for pattern in patterns {
            let pattern = pattern.trim();
            if pattern.is_empty() {
                anyhow::bail!("ignores[{index}].paths contains an empty pattern");
            }
            if pattern.starts_with('!') {
                anyhow::bail!(
                    "ignores[{index}].paths does not support negative pattern {pattern:?}"
                );
            }
            builder
                .add_line(None, pattern)
                .with_context(|| format!("ignores[{index}].paths has invalid glob {pattern:?}"))?;
        }
        let matcher = builder
            .build()
            .with_context(|| format!("building path matcher for ignores[{index}]"))?;
        Ok(Some(Self {
            matcher,
            cwd: absolute_lexical(Path::new(".")).context("resolving ignore working directory")?,
            base: absolute_lexical(base).context("resolving ignore file directory")?,
            canonical_base: base.canonicalize().ok(),
        }))
    }

    pub(super) fn matches(&self, file: &str) -> bool {
        let path = Path::new(file);
        let Some(absolute) = absolute_lexical(&self.cwd.join(path)) else {
            return false;
        };
        // Keep both supported relative bases, regardless of how a location was displayed.
        if [&self.cwd, &self.base].iter().any(|base| {
            absolute
                .strip_prefix(base)
                .is_ok_and(|relative| self.matches_path(relative))
        }) || self.matcher.matched(&absolute, false).is_ignore()
        {
            return true;
        }
        // Source discovery can canonicalize an explicitly supplied symlink root.
        self.canonical_base.as_ref().is_some_and(|base| {
            path.canonicalize().is_ok_and(|canonical| {
                canonical
                    .strip_prefix(base)
                    .is_ok_and(|relative| self.matches_path(relative))
            })
        })
    }

    fn matches_path(&self, path: &Path) -> bool {
        self.matcher
            .matched_path_or_any_parents(path, false)
            .is_ignore()
    }
}
