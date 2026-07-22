use super::*;

/// Clone the family with every member path made repo-relative (stripping the base-worktree
/// prefix), so the family_id is stable across runs and the paths read naturally in reports.
pub(super) fn repo_relative(
    fam: &RefactorFamily,
    lexical_prefix: &Path,
    canonical_prefix: &Path,
) -> RefactorFamily {
    let mut fam = fam.clone();
    for loc in &mut fam.locations {
        repo_relative_loc(loc, lexical_prefix, canonical_prefix);
    }
    for obligation in &mut fam.accepted_coverage {
        for loc in &mut obligation.sites {
            repo_relative_loc(loc, lexical_prefix, canonical_prefix);
        }
    }
    fam
}

pub(super) fn repo_relative_loc(loc: &mut Loc, lexical_prefix: &Path, canonical_prefix: &Path) {
    loc.file = repo_relative_file(&loc.file, lexical_prefix, canonical_prefix);
    if let Some(parent) = &mut loc.enclosing_unit {
        parent.file = repo_relative_file(&parent.file, lexical_prefix, canonical_prefix);
        parent.refresh_unit_key();
    }
}

fn repo_relative_file(file: &str, lexical_prefix: &Path, canonical_prefix: &Path) -> String {
    let path = Path::new(file);
    if let Ok(relative) = path.strip_prefix(lexical_prefix) {
        return relative.to_string_lossy().into_owned();
    }
    if let Ok(relative) = path.strip_prefix(canonical_prefix) {
        return relative.to_string_lossy().into_owned();
    }
    canonical(path)
        .strip_prefix(canonical_prefix)
        .map(|relative| relative.to_string_lossy().into_owned())
        .unwrap_or_else(|_| file.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn relative_paths_accept_the_lexical_worktree_spelling_before_canonicalizing() {
        assert_eq!(
            repo_relative_file(
                "/var/folders/worktree/src/main.rs",
                Path::new("/var/folders/worktree"),
                Path::new("/private/var/folders/worktree"),
            ),
            "src/main.rs"
        );
        assert_eq!(
            repo_relative_file(
                "/private/var/folders/worktree/src/main.rs",
                Path::new("/var/folders/worktree"),
                Path::new("/private/var/folders/worktree"),
            ),
            "src/main.rs"
        );
    }
}
