//! Optional on-disk cache of per-file detection units, keyed by the **resolved
//! IL and reporting metadata** content hash. Re-running nose on a project where
//! most files are unchanged then skips the dominant cost (normalize + extract)
//! for those files and deserializes their units instead.
//!
//! The corpus is lowered AND cross-file-resolved every run
//! (`lower_corpus_filtered` — parse + lower + `resolve_imported_immutable_bindings`,
//! the smaller half of the work per experiments §BQ); only the dominant
//! normalize+extract step is cached. The key is a content hash of each file's
//! *post-resolve* IL, so a file whose imported-immutable-literal context changed
//! (its provider edited) gets a different key and recomputes — fixing #275, where
//! the old source-content key skipped resolution entirely and the cached analysis
//! under-merged cross-file imported-literal convergence. A [`UnitFeat`]'s features
//! are interner-independent content hashes, so a hit needs no interner; the key
//! folds in unit names, source spans, semantic evidence, a schema version, and an
//! options signature so a report-affecting metadata/format/option change transparently
//! misses. Paths stay outside the key and are retargeted on a hit, preserving cache
//! reuse across checkout roots without allowing structurally identical files with
//! different symbols or line locations to reuse stale reporting metadata.

use nose_detect::{DetectOptions, Stream, UnitFeat};
use nose_il::{Corpus, Interner};
use rayon::prelude::*;
use std::hash::{Hash, Hasher};
use std::path::Path;
use std::sync::atomic::{AtomicU64, AtomicUsize, Ordering};

/// Bump when the cached payload's layout, extraction, or feature hashing changes — old
/// cache entries then live under a different directory and are ignored. (v14: report
/// metadata moved from JSON serialization to allocation-free stable hashing.)
const SCHEMA: u32 = 14;

pub(crate) struct CachedUnits {
    pub units: Vec<UnitFeat>,
    pub streams: Vec<Stream>,
    pub files: usize,
    pub stats: CacheStats,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct CacheStats {
    pub files: usize,
    pub hits: usize,
    pub misses: usize,
    pub read_bytes: u64,
    pub written_bytes: u64,
}

/// Build detection units **and contiguous-channel streams** for every file in an
/// already lowered+resolved `corpus`, using the on-disk cache at `dir`. The
/// corpus is lowered and cross-file-resolved by the caller (`lower_corpus_filtered`),
/// so each file's IL already carries its imported-immutable-literal inlining; the
/// cache keys on that *post-resolve* IL (fixing #275) and only the dominant
/// normalize+extract step is cached. A cache hit needs no interner (features are
/// content-derived); a miss recomputes and writes back.
pub(crate) fn build_units_cached(corpus: &Corpus, opts: &DetectOptions, dir: &Path) -> CachedUnits {
    // One bucket per (schema, options signature): changing an option that affects
    // units lands in a fresh bucket, so stale entries are never read.
    let bucket = dir.join(format!("v{SCHEMA}-{:016x}", options_signature(opts)));
    let _ = std::fs::create_dir_all(&bucket);
    let hits = AtomicUsize::new(0);
    let misses = AtomicUsize::new(0);
    let read_bytes = AtomicU64::new(0);
    let written_bytes = AtomicU64::new(0);

    let per_file: Vec<(Vec<UnitFeat>, Stream)> = corpus
        .files
        .par_iter()
        .map(|il| {
            let path = il.meta.path.clone();
            // Key on the post-resolve IL plus every report-affecting metadata
            // surface. Paths are deliberately excluded and retargeted below.
            let key = resolved_il_hash(il, &corpus.interner);
            let entry = bucket.join(format!("{key:016x}.json"));

            if let Ok(bytes) = std::fs::read(&entry) {
                read_bytes.fetch_add(bytes.len() as u64, Ordering::Relaxed);
                if let Ok((mut units, mut stream)) =
                    serde_json::from_slice::<(Vec<UnitFeat>, Stream)>(&bytes)
                {
                    hits.fetch_add(1, Ordering::Relaxed);
                    for u in &mut units {
                        u.path = path.clone();
                    }
                    stream.set_path(path.clone());
                    return (units, stream);
                }
            }

            misses.fetch_add(1, Ordering::Relaxed);
            let units = nose_detect::units_of_file(il, &corpus.interner, opts);
            let stream = nose_detect::file_stream(il, &corpus.interner);
            if let Ok(bytes) = serde_json::to_vec(&(&units, &stream)) {
                let len = bytes.len() as u64;
                if std::fs::write(&entry, bytes).is_ok() {
                    written_bytes.fetch_add(len, Ordering::Relaxed);
                }
            }
            (units, stream)
        })
        .collect();

    let files = per_file.len();
    let mut all_units = Vec::new();
    let mut all_streams = Vec::new();
    for (u, s) in per_file {
        all_units.extend(u);
        all_streams.push(s);
    }
    CachedUnits {
        units: all_units,
        streams: all_streams,
        files,
        stats: CacheStats {
            files,
            hits: hits.load(Ordering::Relaxed),
            misses: misses.load(Ordering::Relaxed),
            read_bytes: read_bytes.load(Ordering::Relaxed),
            written_bytes: written_bytes.load(Ordering::Relaxed),
        },
    }
}

/// Content hash of a file's *post-resolve* IL and reporting metadata — the cache key. Uses
/// `valued_tree_hash`: an interner-INDEPENDENT fold that retains literal values.
/// Interner-independence is essential because the corpus shares one interner whose
/// symbol ids depend on parallel interning order — serializing the raw IL (with
/// those ids) gave a key that varied run-to-run and never warm-hit. Value-retention
/// is essential because the structural `subtree_hashes` erases literal values, so a
/// resolved `LOOKUP = {…: 1}` vs `{…: 9}` would collide — the very post-resolve
/// distinction #275 turns on. The normalized tree is intentionally alpha-invariant,
/// however, while cached units also contain original unit names and source locations.
/// Those fields, all original symbol names, suppression ranges, and the complete
/// semantic-evidence records therefore join the key. Otherwise two clone-shaped files
/// can collide and a warm query can inherit the first file's function name or spans.
fn resolved_il_hash(il: &nose_il::Il, interner: &Interner) -> u64 {
    let mut h = crate::fnv::OFFSET_BASIS;
    h = crate::fnv::mix(h, il.meta.lang as u8 as u64);
    h = crate::fnv::mix(h, il.root.0 as u64);
    h = crate::fnv::mix(h, nose_normalize::valued_tree_hash(il, interner));

    for node in &il.nodes {
        for coordinate in [
            node.span.start_byte,
            node.span.end_byte,
            node.span.start_line,
            node.span.end_line,
        ] {
            h = crate::fnv::mix(h, coordinate as u64);
        }
    }
    for unit in &il.units {
        h = crate::fnv::mix(h, unit.root.0 as u64);
        h = crate::fnv::mix(h, unit.kind as u8 as u64);
        h = crate::fnv::mix(
            h,
            unit.name
                .map(|name| interner.symbol_hash(name))
                .unwrap_or_default(),
        );
        h = mix_hashable(h, &unit.origin);
    }
    for &name in &il.cid_names {
        h = crate::fnv::mix(h, interner.symbol_hash(name));
    }
    h = mix_hashable(h, &il.suppressed);
    mix_hashable(h, &il.evidence)
}

struct StableFnv(u64);

impl Hasher for StableFnv {
    fn finish(&self) -> u64 {
        self.0
    }

    fn write(&mut self, bytes: &[u8]) {
        for &byte in bytes {
            self.0 = crate::fnv::mix(self.0, byte as u64);
        }
    }
}

fn mix_hashable(h: u64, value: &impl Hash) -> u64 {
    let mut hasher = StableFnv(h);
    value.hash(&mut hasher);
    hasher.finish()
}

/// Fold every unit-affecting option into one value; changing any of them changes
/// the cache bucket. (`threshold`/`bands` only affect scoring/candidate-gen, not the
/// units themselves, so they are deliberately excluded.)
fn options_signature(opts: &DetectOptions) -> u64 {
    let mut h = crate::fnv::OFFSET_BASIS;
    for v in [
        opts.min_lines as u64,
        opts.min_tokens as u64,
        opts.block_units as u64,
        opts.cfg_norm as u64,
        opts.dce as u64,
        opts.minhash_k as u64,
        opts.shape_features as u64,
        opts.connected_witnesses as u64,
        opts.abstraction_witnesses as u64,
    ] {
        h = crate::fnv::mix(h, v);
    }
    h
}

#[cfg(all(test, unix))]
mod tests {
    use super::*;
    use std::os::unix::fs::PermissionsExt;

    /// `files` counts the lowered corpus the caller hands in — which already
    /// excludes unreadable/parse-failed files (`lower_corpus_filtered` filter_maps
    /// them). So a corpus where one file failed to read yields `files == 1`.
    #[test]
    fn file_count_matches_lowered_corpus() {
        let dir = std::env::temp_dir().join(format!("nose_cache_count_{}", std::process::id()));
        let cache = std::env::temp_dir().join(format!("nose_cache_dir_{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::create_dir_all(&cache).unwrap();

        std::fs::write(dir.join("ok.py"), "def f():\n    return 1\n").unwrap();
        let bad = dir.join("bad.py");
        std::fs::write(&bad, "def g():\n    return 2\n").unwrap();
        std::fs::set_permissions(&bad, std::fs::Permissions::from_mode(0o000)).unwrap();

        let readable = std::fs::read(&bad).is_ok();
        let corpus = nose_frontend::lower_corpus_filtered(&[dir.as_path()], &[]);
        let out = build_units_cached(&corpus, &DetectOptions::default(), &cache);

        let _ = std::fs::set_permissions(&bad, std::fs::Permissions::from_mode(0o644));
        let _ = std::fs::remove_dir_all(&dir);
        let _ = std::fs::remove_dir_all(&cache);

        if readable {
            // Running as root (CI sometimes) — the unreadable file is still readable.
            return;
        }
        assert_eq!(out.files, 1, "only the readable file should be counted");
    }

    #[test]
    fn schema_14_ignores_json_hashed_reporting_identity_entries() {
        let root = std::env::temp_dir().join(format!("nose_cache_schema_{}", std::process::id()));
        let source = root.join("source");
        let cache = root.join("cache");
        let _ = std::fs::remove_dir_all(&root);
        std::fs::create_dir_all(&source).unwrap();
        std::fs::create_dir_all(&cache).unwrap();
        std::fs::write(
            source.join("mixed.ts"),
            "function f(x: boolean) { if (x) { return 1; } }\n",
        )
        .unwrap();

        let corpus = nose_frontend::lower_corpus_filtered(&[source.as_path()], &[]);
        let options = DetectOptions::default();
        let signature = options_signature(&options);
        let current = cache.join(format!("v14-{signature:016x}"));
        let stale = cache.join(format!("v13-{signature:016x}"));
        let first = build_units_cached(&corpus, &options, &cache);
        assert!(!first.units.is_empty());
        std::fs::rename(&current, &stale).unwrap();
        assert!(!current.exists());

        let second = build_units_cached(&corpus, &options, &cache);
        assert_eq!(second.units.len(), first.units.len());
        assert!(
            current.is_dir(),
            "v13 entries must be ignored and a fresh v14 bucket written"
        );
        assert!(stale.is_dir(), "the stale bucket should remain untouched");

        let _ = std::fs::remove_dir_all(&root);
    }
}
