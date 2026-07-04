use nose_il::Lang;
use std::collections::BTreeSet;
use std::path::{Component, Path, PathBuf as StdPathBuf};

/// Walk `root` (respecting .gitignore) and collect supported source files, skipping
/// any matching an `exclude` glob. The walk runs on multiple threads (`ignore`'s
/// parallel walker), so .gitignore parsing and traversal don't serialize before
/// lowering. Excludes are gitignore-syntax globs (`tests`, `**/*.test.ts`,
/// `vendor/**`) applied during the walk, so excluded directories are pruned, not
/// just filtered. Results come back in walk order (nondeterministic); the caller sorts.
pub fn discover_paths(root: &Path, exclude: &[String]) -> Vec<(String, Lang)> {
    use ignore::overrides::OverrideBuilder;
    use ignore::{WalkBuilder, WalkState};
    use std::sync::Mutex;

    // A file path on the command line does not need a directory walker. This keeps
    // explicit fixture/file scans cheap while leaving configured excludes on the
    // existing walker path, where their gitignore semantics are already defined.
    if exclude.is_empty() && root.is_file() {
        return Lang::from_file_path(root)
            .map(|lang| vec![(root.to_string_lossy().to_string(), lang)])
            .unwrap_or_default();
    }

    // Honor .gitignore *within* the target tree (skips node_modules, build dirs)
    // but not gitignores in parent directories outside it — pointing the tool at
    // a path that happens to sit under an ignored dir should still analyze it.
    // `require_git(false)` so a tree's .gitignore is respected even when it isn't a
    // git checkout (extracted tarball, sub-tree, vendored copy) — otherwise `ignore`
    // only activates gitignore rules under an actual `.git`, and generated/vendored
    // files leak into the report (a real surprise the field eval hit).
    let mut builder = WalkBuilder::new(root);
    builder.parents(false).require_git(false);
    if !exclude.is_empty() {
        // `!glob` in an override means "ignore matches"; with only ignore globs,
        // every non-matching file is still included.
        let mut ob = OverrideBuilder::new(root);
        for g in exclude {
            let _ = ob.add(&format!("!{g}"));
        }
        if let Ok(ov) = ob.build() {
            builder.overrides(ov);
        }
    }
    let out = Mutex::new(Vec::new());
    builder.build_parallel().run(|| {
        let out = &out;
        Box::new(move |result| {
            if let Ok(entry) = result {
                if entry.file_type().is_some_and(|t| t.is_file()) {
                    if let Some(lang) = Lang::from_file_path(entry.path()) {
                        let path = entry.path().to_string_lossy().to_string();
                        out.lock().unwrap().push((path, lang));
                    }
                }
            }
            WalkState::Continue
        })
    });
    out.into_inner().unwrap()
}

fn clean_discovered_path(path: &Path) -> String {
    let mut cleaned = StdPathBuf::new();
    for component in path.components() {
        if !matches!(component, Component::CurDir) {
            cleaned.push(component.as_os_str());
        }
    }
    if cleaned.as_os_str().is_empty() {
        path.to_string_lossy().to_string()
    } else {
        cleaned.to_string_lossy().to_string()
    }
}

#[derive(Debug, Eq, Ord, PartialEq, PartialOrd)]
struct DiscoveredPathIdentity(StdPathBuf);

fn discovered_path_identity(path: &str) -> DiscoveredPathIdentity {
    DiscoveredPathIdentity(std::fs::canonicalize(path).unwrap_or_else(|_| StdPathBuf::from(path)))
}

struct DiscoveredPath {
    root_index: usize,
    path: String,
    lang: Lang,
}

/// Discover supported source files under all `roots`, then sort and deduplicate
/// canonical path aliases. The returned path spelling is still the stable,
/// user-facing discovered path, so reports do not need a separate display map.
pub fn discover_unique_paths(roots: &[&Path], exclude: &[String]) -> Vec<(String, Lang)> {
    let mut paths = Vec::new();
    for (root_index, root) in roots.iter().enumerate() {
        paths.extend(
            discover_paths(root, exclude)
                .into_iter()
                .map(|(path, lang)| DiscoveredPath {
                    root_index,
                    path: clean_discovered_path(Path::new(&path)),
                    lang,
                }),
        );
    }
    // The parallel walk yields paths in nondeterministic order. First choose the
    // user-facing spelling from the earliest explicit root that discovered a
    // physical path; then sort those representatives by path so `FileId`s remain
    // stable across runs and machines.
    paths.sort_unstable_by(|a, b| {
        a.root_index
            .cmp(&b.root_index)
            .then_with(|| a.path.cmp(&b.path))
    });
    let mut seen = BTreeSet::new();
    paths.retain(|entry| seen.insert(discovered_path_identity(&entry.path)));
    let mut paths = paths
        .into_iter()
        .map(|entry| (entry.path, entry.lang))
        .collect::<Vec<_>>();
    paths.sort_unstable_by(|a, b| a.0.cmp(&b.0));
    paths
}
