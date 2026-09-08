use anyhow::Result;
use nose_il::Corpus;
use std::path::{Component, Path, PathBuf};

/// Borrow a slice of owned `PathBuf`s as `&Path` references — the form the detection entry
/// points take. Used by every analysis/refactor subcommand that holds its input paths as a
/// `Vec<PathBuf>`.
pub(crate) fn paths_as_refs(paths: &[PathBuf]) -> Vec<&Path> {
    paths.iter().map(|p| p.as_path()).collect()
}

/// Warn (to stderr) when discovery turned up nothing, so a mistyped path or an
/// unsupported tree doesn't masquerade as "no duplication found". Returns true if
/// the corpus is empty (caller may choose to stop early).
pub(crate) fn warn_if_empty(corpus: &Corpus, paths: &[PathBuf]) -> bool {
    if corpus.files.is_empty() {
        warn_no_files(paths);
        return true;
    }
    false
}

/// Render `file` relative to `cwd` when it sits underneath it; otherwise leave it
/// as-is (an absolute path outside cwd is more useful whole than mangled).
pub(crate) fn relativize(file: &str, cwd: &Path) -> String {
    Path::new(file)
        .strip_prefix(cwd)
        .ok()
        .and_then(|p| p.to_str())
        .map(str::to_string)
        .unwrap_or_else(|| file.to_string())
}

pub(crate) fn relativize_loc(loc: &mut nose_detect::Loc, cwd: &Path) {
    loc.file = relativize(&loc.file, cwd);
    if let Some(parent) = &mut loc.enclosing_unit {
        parent.file = relativize(&parent.file, cwd);
        parent.refresh_unit_key();
    }
}

/// Stderr notice that discovery found nothing — so a mistyped path or unsupported
/// tree doesn't masquerade as "no duplication found".
/// A named path that doesn't exist is a usage error, not an empty analysis: a typo'd
/// path in a CI gate must fail loudly instead of passing on a 0-file report.
/// "Exists but contains no supported files" stays a warning (`warn_no_files`).
pub(crate) fn require_paths_exist(paths: &[PathBuf]) -> Result<()> {
    let missing: Vec<String> = paths
        .iter()
        .filter(|p| !p.exists())
        .map(|p| p.display().to_string())
        .collect();
    if missing.is_empty() {
        return Ok(());
    }
    anyhow::bail!("path does not exist: {}", missing.join(", "))
}

pub(crate) fn warn_no_files(paths: &[PathBuf]) {
    let joined = paths
        .iter()
        .map(|p| p.display().to_string())
        .collect::<Vec<_>>()
        .join(", ");
    eprintln!(
        "warning: no supported source files found under: {joined}\n  \
         (supported extensions: py/pyi, js/jsx/mjs/cjs, ts/tsx/mts/cts, go, rs, java, c/h, rb, swift, css, vue/svelte/html/htm, md/markdown)"
    );
}

/// Make a path absolute and collapse dot components without requiring it to exist.
/// Callers that need physical containment must also resolve symlinks.
pub(crate) fn absolute_lexical(path: &Path) -> Option<PathBuf> {
    let absolute = if path.is_absolute() {
        path.to_path_buf()
    } else {
        std::env::current_dir().ok()?.join(path)
    };
    let mut cleaned = PathBuf::new();
    for component in absolute.components() {
        match component {
            Component::CurDir => {}
            Component::ParentDir => {
                cleaned.pop();
            }
            other => cleaned.push(other.as_os_str()),
        }
    }
    Some(cleaned)
}

/// Quote one shell word for runnable CLI navigation, including embedded apostrophes.
pub(crate) fn shell_quote(value: &str) -> String {
    format!("'{}'", value.replace('\'', "'\"'\"'"))
}
