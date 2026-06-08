//! Frontends: parse source with tree-sitter and lower each file's CST into raw
//! IL. One [`Il`] per file; files are lowered in parallel and collected into a
//! [`Corpus`] sharing a single interner.

mod c;
mod coverage;
mod embedded;
mod go;
mod java;
mod js_ts;
mod lower;
mod module_imports;
mod python;
mod ruby;
mod rust;
mod type_domain_aliases;

pub use coverage::{coverage, CoverageReport};

use nose_il::{Corpus, FileId, Il, Interner, Lang};
use rayon::prelude::*;
use std::path::Path;

#[cfg(test)]
pub(crate) mod test_helpers {
    use nose_il::{Il, Interner, NodeId, NodeKind, Payload, SourceProtocolKind};
    use nose_semantics::source_protocol_at_node;

    pub(crate) fn expect_raw_protocol_boundary(
        il: &Il,
        interner: &Interner,
        tag: &str,
        protocol: SourceProtocolKind,
    ) -> NodeId {
        let nodes: Vec<NodeId> = il
            .nodes
            .iter()
            .enumerate()
            .filter_map(|(idx, node)| match node.payload {
                Payload::Name(sym)
                    if node.kind == NodeKind::Raw && interner.resolve(sym) == tag =>
                {
                    Some(NodeId(idx as u32))
                }
                _ => None,
            })
            .collect();
        assert_eq!(
            nodes.len(),
            1,
            "{tag} should stay as one raw protocol boundary: {nodes:?}"
        );
        assert_eq!(
            source_protocol_at_node(il, nodes[0]),
            Some(protocol),
            "{tag} boundary should carry source protocol evidence"
        );
        nodes[0]
    }
}

/// Lower a single in-memory source buffer.
pub fn lower_source(
    file: FileId,
    path: &str,
    src: &[u8],
    lang: Lang,
    interner: &Interner,
) -> anyhow::Result<Il> {
    match lang {
        Lang::Python => python::lower(file, path, src, interner),
        Lang::JavaScript | Lang::TypeScript => js_ts::lower(file, path, src, lang, interner),
        Lang::Go => go::lower(file, path, src, interner),
        Lang::Rust => rust::lower(file, path, src, interner),
        Lang::Java => java::lower(file, path, src, interner),
        Lang::C => c::lower(file, path, src, interner),
        Lang::Ruby => ruby::lower(file, path, src, interner),
        Lang::Vue | Lang::Svelte | Lang::Html => embedded::lower(file, path, src, lang, interner),
    }
}

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
    // a path that happens to sit under an ignored dir should still scan it.
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

/// Discover, read, and lower every supported file under `root`, in parallel.
/// Files that fail to read or parse are skipped. Each surviving [`Il`] carries a
/// unique [`FileId`] (its index in the discovered path list) and its own path in
/// `meta`, so reporting never needs a corpus-wide id table.
pub fn lower_corpus(root: &Path) -> Corpus {
    lower_corpus_many(std::slice::from_ref(&root))
}

/// Like [`lower_corpus`] but discovers across several roots into one corpus
/// (sharing a single interner — required for cross-root/cross-language matching).
pub fn lower_corpus_many(roots: &[&Path]) -> Corpus {
    lower_corpus_filtered(roots, &[])
}

/// Like [`lower_corpus_many`] but applies gitignore-syntax `exclude` globs during
/// discovery (e.g. `tests`, `vendor/**`, `**/*.generated.ts`).
pub fn lower_corpus_filtered(roots: &[&Path], exclude: &[String]) -> Corpus {
    let timing = std::env::var_os("NOSE_TIME").is_some();
    let t0 = std::time::Instant::now();

    let interner = Interner::new();
    let mut paths = Vec::new();
    for r in roots {
        paths.extend(discover_paths(r, exclude));
    }
    // The parallel walk yields paths in nondeterministic order; sort by path (unique)
    // so each file's `FileId` (its index here) is stable across runs and machines.
    paths.sort_unstable_by(|a, b| a.0.cmp(&b.0));
    if timing {
        eprintln!(
            "  [time] {:<12} {:>7.1}ms  ({} files)",
            "discover",
            t0.elapsed().as_secs_f64() * 1e3,
            paths.len()
        );
    }

    let t1 = std::time::Instant::now();
    let mut files: Vec<Il> = paths
        .par_iter()
        .enumerate()
        .filter_map(|(i, (path, lang))| {
            let src = std::fs::read(path).ok()?;
            lower_source(FileId(i as u32), path, &src, *lang, &interner).ok()
        })
        .collect();
    if timing {
        eprintln!(
            "  [time] {:<12} {:>7.1}ms  (read+parse+lower, parallel)",
            "parse+lower",
            t1.elapsed().as_secs_f64() * 1e3
        );
    }

    let t2 = std::time::Instant::now();
    module_imports::resolve_imported_immutable_bindings(&mut files, &interner);
    if timing {
        eprintln!(
            "  [time] {:<12} {:>7.1}ms  (corpus import facts)",
            "import-resolve",
            t2.elapsed().as_secs_f64() * 1e3
        );
    }
    Corpus::new(interner, files)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    fn temp_dir(tag: &str) -> std::path::PathBuf {
        let dir = std::env::temp_dir().join(format!("nose_frontend_{tag}_{}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).unwrap();
        dir
    }

    #[test]
    fn discover_paths_accepts_direct_supported_file() {
        let dir = temp_dir("direct_supported_file");
        let file = dir.join("sample.py");
        fs::write(&file, "def f():\n    return 1\n").unwrap();

        let paths = discover_paths(&file, &[]);

        assert_eq!(
            paths,
            vec![(file.to_string_lossy().to_string(), Lang::Python)]
        );
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn discover_paths_ignores_direct_unsupported_file() {
        let dir = temp_dir("direct_unsupported_file");
        let file = dir.join("README.txt");
        fs::write(&file, "not source\n").unwrap();

        assert!(discover_paths(&file, &[]).is_empty());
        let _ = fs::remove_dir_all(&dir);
    }
}
