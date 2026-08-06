//! Corpus discovery, lowering, and cross-file resolution.
//!
//! Single-buffer language dispatch stays at the crate facade. This module owns
//! the file-system and parallel-orchestration boundary that turns those lowered
//! buffers into a resolved corpus.

use crate::{
    discover_unique_paths, embedded, lower_source, module_imports, source_artifacts,
    swift_cross_file_shadows,
};
use nose_il::{Corpus, FileId, Il, Interner, Lang};
use rayon::prelude::*;
use std::path::Path;

/// Whether a discovered source buffer should enter the analysis corpus. Cache
/// loaders use the same generated/binary artifact gate as the uncached path so
/// an artifact never becomes analyzable merely because it was cached.
pub fn source_is_analyzable(path: &Path, lang: Lang, source: &[u8]) -> bool {
    source_artifacts::skip_reason(path, lang, source).is_none()
}

/// Lower every analyzable region of a file into separate [`Il`]s. For most languages
/// this is one `Il` (delegating to [`lower_source`]); for `<script>`/`<style>`-bearing
/// containers (Vue/Svelte/HTML) it is one per embedded region (JS/TS for `<script>`, CSS
/// for `<style>`), all sharing the container's [`FileId`] and path.
pub fn lower_source_regions(
    file: FileId,
    path: &str,
    src: &[u8],
    lang: Lang,
    interner: &Interner,
) -> Vec<Il> {
    match lang {
        Lang::Vue | Lang::Svelte | Lang::Html => {
            embedded::lower_regions(file, path, src, lang, interner)
        }
        _ => lower_source(file, path, src, lang, interner)
            .ok()
            .into_iter()
            .collect(),
    }
}

/// Discover, read, and lower every supported file under `root`, in parallel.
/// Files that fail to read or parse are skipped. Each surviving [`Il`] carries a
/// unique [`FileId`] and its own path in `meta`.
pub fn lower_corpus(root: &Path) -> Corpus {
    lower_corpus_many(std::slice::from_ref(&root))
}

/// Like [`lower_corpus`] but discovers across several roots into one corpus.
pub fn lower_corpus_many(roots: &[&Path]) -> Corpus {
    lower_corpus_filtered(roots, &[])
}

/// Like [`lower_corpus_many`] but applies gitignore-syntax `exclude` globs.
pub fn lower_corpus_filtered(roots: &[&Path], exclude: &[String]) -> Corpus {
    let mut corpus = lower_corpus_raw_filtered(roots, exclude);
    resolve_corpus(&mut corpus);
    corpus
}

/// Discover, read, parse, and lower a corpus without corpus-wide semantic
/// resolution. This is the portable raw-IL stage boundary used by the layered
/// cache; ordinary callers should prefer [`lower_corpus_filtered`].
pub fn lower_corpus_raw_filtered(roots: &[&Path], exclude: &[String]) -> Corpus {
    let timing = std::env::var_os("NOSE_TIME").is_some();
    let started = std::time::Instant::now();

    let interner = Interner::new();
    let paths = discover_unique_paths(roots, exclude);
    if timing {
        eprintln!(
            "  [time] {:<12} {:>7.1}ms  ({} files)",
            "discover",
            started.elapsed().as_secs_f64() * 1e3,
            paths.len()
        );
    }

    let started = std::time::Instant::now();
    // An embedded container lowers to several region ILs. Rayon's indexed
    // `flat_map` preserves path order, keeping FileIds deterministic.
    let files: Vec<Il> = paths
        .par_iter()
        .enumerate()
        .flat_map(|(index, (path, lang))| match std::fs::read(path) {
            Ok(source) if source_is_analyzable(Path::new(path), *lang, &source) => {
                lower_source_regions(FileId(index as u32), path, &source, *lang, &interner)
            }
            Ok(_) | Err(_) => Vec::new(),
        })
        .collect();
    if timing {
        eprintln!(
            "  [time] {:<12} {:>7.1}ms  (read+parse+lower, parallel)",
            "parse+lower",
            started.elapsed().as_secs_f64() * 1e3
        );
    }

    Corpus::new(interner, files)
}

/// Apply every corpus-wide frontend resolver to a raw corpus in place.
pub fn resolve_corpus(corpus: &mut Corpus) {
    let targets = vec![true; corpus.files.len()];
    resolve_corpus_affected(corpus, &targets);
}

/// Resolve only selected raw IL regions while reading dependency facts from the
/// complete raw corpus.
pub fn resolve_corpus_affected(corpus: &mut Corpus, targets: &[bool]) {
    assert_eq!(corpus.files.len(), targets.len());
    let started = std::time::Instant::now();
    module_imports::resolve_imported_immutable_bindings_affected(
        &mut corpus.files,
        &corpus.interner,
        targets,
    );
    swift_cross_file_shadows::close_shadowed_stdlib_apis_affected(
        &mut corpus.files,
        &corpus.interner,
        targets,
    );
    log_resolution_timing(started, "corpus import facts");
}

/// Raw-corpus dependency analysis retained for later selective resolution.
pub struct PreparedCorpusResolution {
    pub summary: module_imports::ResolutionDependencySummary,
    imports: module_imports::PreparedImportResolution,
}

pub fn prepare_corpus_resolution(corpus: &Corpus) -> PreparedCorpusResolution {
    let imports = module_imports::prepare_import_resolution(&corpus.files, &corpus.interner);
    let summary =
        module_imports::dependency_summary_prepared(&corpus.files, &corpus.interner, &imports);
    PreparedCorpusResolution { summary, imports }
}

pub fn resolve_corpus_prepared(
    corpus: &mut Corpus,
    targets: &[bool],
    prepared: PreparedCorpusResolution,
) -> module_imports::ResolutionDependencySummary {
    assert_eq!(corpus.files.len(), targets.len());
    let started = std::time::Instant::now();
    module_imports::apply_import_resolution(
        &mut corpus.files,
        &corpus.interner,
        targets,
        prepared.imports,
    );
    swift_cross_file_shadows::close_shadowed_stdlib_apis_affected(
        &mut corpus.files,
        &corpus.interner,
        targets,
    );
    log_resolution_timing(started, "prepared corpus import facts");
    prepared.summary
}

fn log_resolution_timing(started: std::time::Instant, detail: &str) {
    if std::env::var_os("NOSE_TIME").is_some() {
        eprintln!(
            "  [time] {:<12} {:>7.1}ms  ({detail})",
            "import-resolve",
            started.elapsed().as_secs_f64() * 1e3
        );
    }
}
