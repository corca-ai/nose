use anyhow::{Context, Result};
use ignore::overrides::{Override, OverrideBuilder};
use std::collections::BTreeSet;
use std::path::{Path, PathBuf};

use crate::path_utils::absolute_lexical;

#[derive(Default)]
pub(crate) struct GeneratedPathAssertions {
    matcher: Option<Override>,
    roots: Vec<AssertionRoot>,
}

struct AssertionRoot {
    display: PathBuf,
    canonical: PathBuf,
    is_file: bool,
}

impl GeneratedPathAssertions {
    pub(crate) fn new(roots: &[PathBuf], patterns: Vec<String>) -> Result<Self> {
        let patterns = patterns.into_iter().collect::<BTreeSet<_>>();
        if patterns.is_empty() {
            return Ok(Self::default());
        }

        let mut builder = OverrideBuilder::new(".");
        for pattern in &patterns {
            validate_pattern(pattern)?;
            builder
                .add(&format!("/{pattern}"))
                .with_context(|| format!("invalid generated-path glob {pattern:?}"))?;
        }
        let matcher = builder
            .build()
            .context("building generated-path glob matcher")?;
        let roots = roots
            .iter()
            .filter_map(|root| {
                let display = absolute_lexical(root)?;
                let canonical = std::fs::canonicalize(root).ok()?;
                let is_file = std::fs::metadata(root).ok()?.is_file();
                Some(AssertionRoot {
                    display,
                    canonical,
                    is_file,
                })
            })
            .collect();
        Ok(Self {
            matcher: Some(matcher),
            roots,
        })
    }

    pub(crate) fn matches(&self, file: &str) -> bool {
        let Some(matcher) = &self.matcher else {
            return false;
        };
        let Ok(open) = std::fs::File::open(file) else {
            return false;
        };
        if !open.metadata().is_ok_and(|metadata| metadata.is_file()) {
            return false;
        }
        let Ok(canonical) = std::fs::canonicalize(file) else {
            return false;
        };
        let Some(display) = absolute_lexical(Path::new(file)) else {
            return false;
        };

        self.roots.iter().any(|root| {
            root.relative_path(&display, &canonical)
                .is_some_and(|relative| matcher.matched(relative, false).is_whitelist())
        })
    }
}

impl AssertionRoot {
    fn relative_path<'a>(&'a self, display: &'a Path, canonical: &'a Path) -> Option<&'a Path> {
        if self.is_file {
            if canonical != self.canonical {
                return None;
            }
            return self.display.file_name().map(Path::new);
        }
        let canonical_relative = canonical.strip_prefix(&self.canonical).ok()?;
        if canonical_relative.as_os_str().is_empty() {
            return None;
        }
        display
            .strip_prefix(&self.display)
            .ok()
            .filter(|relative| !relative.as_os_str().is_empty())
            .or(Some(canonical_relative))
    }
}

fn validate_pattern(pattern: &str) -> Result<()> {
    let invalid = if pattern.is_empty() {
        Some("it is empty")
    } else if pattern.starts_with('!') {
        Some("negation is not supported")
    } else if pattern.contains('\\') {
        Some("use `/` as the portable path separator")
    } else if Path::new(pattern).is_absolute() || pattern.starts_with('/') {
        Some("absolute patterns are not supported")
    } else if pattern
        .split('/')
        .any(|component| matches!(component, "." | ".."))
    {
        Some("`.` and `..` path components are not supported")
    } else {
        None
    };
    if let Some(reason) = invalid {
        anyhow::bail!("invalid generated-path glob {pattern:?}: {reason}");
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicU64, Ordering};

    static SEQUENCE: AtomicU64 = AtomicU64::new(0);

    fn temp_dir(tag: &str) -> PathBuf {
        let sequence = SEQUENCE.fetch_add(1, Ordering::Relaxed);
        let dir = std::env::temp_dir().join(format!(
            "nose_generated_paths_{tag}_{}_{}",
            std::process::id(),
            sequence
        ));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        dir
    }

    fn write(path: &Path) {
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        std::fs::write(path, "def example():\n    return 1\n").unwrap();
    }

    #[test]
    fn patterns_are_anchored_to_each_root_and_fail_open_outside() {
        let first = temp_dir("anchored_first");
        let second = temp_dir("anchored_second");
        let direct = first.join("generated/direct.py");
        let nested = first.join("pkg/generated/nested.py");
        let other_root = second.join("generated/other.py");
        let outside = temp_dir("anchored_outside").join("generated/outside.py");
        for path in [&direct, &nested, &other_root, &outside] {
            write(path);
        }

        let assertions = GeneratedPathAssertions::new(
            &[first.clone(), second.clone()],
            vec!["generated/**".to_string()],
        )
        .unwrap();
        assert!(assertions.matches(direct.to_str().unwrap()));
        assert!(assertions.matches(other_root.to_str().unwrap()));
        assert!(!assertions.matches(nested.to_str().unwrap()));
        assert!(!assertions.matches(outside.to_str().unwrap()));

        let nested_assertions = GeneratedPathAssertions::new(
            std::slice::from_ref(&first),
            vec!["**/generated/**".to_string()],
        )
        .unwrap();
        assert!(nested_assertions.matches(nested.to_str().unwrap()));

        let _ = std::fs::remove_dir_all(first);
        let _ = std::fs::remove_dir_all(second);
        let _ = std::fs::remove_dir_all(outside.parent().unwrap().parent().unwrap());
    }

    #[test]
    fn file_roots_and_missing_files_have_explicit_behavior() {
        let dir = temp_dir("file_root");
        let file = dir.join("snapshot.py");
        write(&file);
        let assertions = GeneratedPathAssertions::new(
            std::slice::from_ref(&file),
            vec!["snapshot.py".to_string()],
        )
        .unwrap();
        assert!(assertions.matches(file.to_str().unwrap()));
        std::fs::remove_file(&file).unwrap();
        assert!(!assertions.matches(file.to_str().unwrap()));
        let _ = std::fs::remove_dir_all(dir);
    }

    #[test]
    fn invalid_or_ambiguous_patterns_are_rejected() {
        let root = temp_dir("invalid");
        for pattern in [
            "",
            "!generated/**",
            "/generated/**",
            "../generated/**",
            ".\\x",
        ] {
            assert!(
                GeneratedPathAssertions::new(
                    std::slice::from_ref(&root),
                    vec![pattern.to_string()],
                )
                .is_err(),
                "{pattern:?}"
            );
        }
        let _ = std::fs::remove_dir_all(root);
    }

    #[cfg(unix)]
    #[test]
    fn explicit_symlink_roots_work_but_nested_escapes_fail_open() {
        use std::os::unix::fs::symlink;

        let parent = temp_dir("symlink");
        let target = parent.join("target");
        let outside = parent.join("outside");
        let inside_file = target.join("generated/inside.py");
        let outside_file = outside.join("generated/outside.py");
        write(&inside_file);
        write(&outside_file);

        let alias = parent.join("alias");
        symlink(&target, &alias).unwrap();
        let explicit = GeneratedPathAssertions::new(
            std::slice::from_ref(&alias),
            vec!["generated/**".to_string()],
        )
        .unwrap();
        assert!(explicit.matches(alias.join("generated/inside.py").to_str().unwrap()));

        let escape = target.join("escape");
        symlink(&outside, &escape).unwrap();
        let contained = GeneratedPathAssertions::new(
            std::slice::from_ref(&target),
            vec!["**/generated/**".to_string()],
        )
        .unwrap();
        assert!(!contained.matches(escape.join("generated/outside.py").to_str().unwrap()));
        let _ = std::fs::remove_dir_all(parent);
    }
}
