//! Optional layered content-addressed cache. The currently active product layer
//! stores per-file detection units keyed by the **resolved IL and reporting
//! metadata** SHA-256 digest. Re-running nose on a project where
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
//! are interner-independent content hashes, so a hit needs no interner. The key
//! covers nodes, edges, spans, facets, symbol strings, suppression, and complete
//! semantic evidence plus stage/schema/options identity. Paths and process-local
//! ids stay outside the key and are rebound on a hit. A checksummed envelope makes
//! corrupt, truncated, or misplaced entries clean misses rather than silent reuse.

mod digest;
mod portable_il;
mod store;

use nose_detect::{DetectOptions, Stream, UnitFeat};
use nose_il::Corpus;
use rayon::prelude::*;
use std::path::Path;
use std::sync::atomic::{AtomicU64, AtomicUsize, Ordering};

use self::digest::ContentDigest;
use self::store::{ArtifactKey, ArtifactStage, LayeredCas};

/// Bump when unit/stream serialization, extraction, or feature hashing changes.
const UNITS_SYNTAX_SCHEMA: u32 = 2;

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
    let cas = LayeredCas::new(dir);
    let options = options_digest(opts);
    let hits = AtomicUsize::new(0);
    let misses = AtomicUsize::new(0);
    let read_bytes = AtomicU64::new(0);
    let written_bytes = AtomicU64::new(0);

    let per_file: Vec<(Vec<UnitFeat>, Stream)> = corpus
        .files
        .par_iter()
        .map(|il| {
            let path = il.meta.path.clone();
            let resolved = portable_il::semantic_digest(il, &corpus.interner);
            let key = ArtifactKey::derive(
                ArtifactStage::UnitsSyntax,
                UNITS_SYNTAX_SCHEMA,
                &[resolved.as_bytes(), options.as_bytes()],
            );

            if let Some(entry) = cas.load(key) {
                read_bytes.fetch_add(entry.stored_bytes, Ordering::Relaxed);
                if let Ok((mut units, mut stream)) =
                    rmp_serde::from_slice::<(Vec<UnitFeat>, Stream)>(&entry.payload)
                {
                    hits.fetch_add(1, Ordering::Relaxed);
                    retarget(&mut units, &mut stream, &path);
                    return (units, stream);
                }
            }

            misses.fetch_add(1, Ordering::Relaxed);
            let mut units = nose_detect::units_of_file(il, &corpus.interner, opts);
            let mut stream = nose_detect::file_stream(il, &corpus.interner);
            // Checkout paths are presentation state, not payload identity. Blank
            // them before serialization and restore them for this query.
            retarget(&mut units, &mut stream, "");
            // Named MessagePack preserves serde's default/skip compatibility
            // while avoiding JSON's decimal expansion of feature hashes.
            let payload = rmp_serde::to_vec_named(&(&units, &stream));
            retarget(&mut units, &mut stream, &path);
            if let Ok(payload) = payload {
                if let Ok(bytes) = cas.store(key, &payload) {
                    written_bytes.fetch_add(bytes, Ordering::Relaxed);
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

fn retarget(units: &mut [UnitFeat], stream: &mut Stream, path: &str) {
    for unit in units {
        path.clone_into(&mut unit.path);
    }
    stream.set_path(path.to_owned());
}

/// Fold every unit-affecting option into a collision-resistant, fixed-width
/// identity. Threshold/bands affect later stages and remain deliberately absent.
fn options_digest(opts: &DetectOptions) -> ContentDigest {
    let values = [
        opts.min_lines as u64,
        opts.min_tokens as u64,
        opts.block_units as u64,
        opts.cfg_norm as u64,
        opts.dce as u64,
        opts.minhash_k as u64,
        opts.shape_features as u64,
        opts.connected_witnesses as u64,
        opts.abstraction_witnesses as u64,
    ];
    let mut bytes = Vec::with_capacity(values.len() * 8);
    for value in values {
        bytes.extend_from_slice(&value.to_be_bytes());
    }
    ContentDigest::derive(b"nose.units-syntax-options.v1", &[&bytes])
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
    fn layered_cas_ignores_legacy_schema_14_entries() {
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
        let stale = cache.join("v14-deadbeef");
        std::fs::create_dir_all(&stale).unwrap();
        std::fs::write(stale.join("legacy.json"), b"[]").unwrap();
        let first = build_units_cached(&corpus, &options, &cache);
        assert!(!first.units.is_empty());
        let second = build_units_cached(&corpus, &options, &cache);
        assert_eq!(second.units.len(), first.units.len());
        assert_eq!(second.stats.hits, 1);
        assert!(cache.join("cas-v1/units-syntax").is_dir());
        assert!(stale.is_dir(), "the legacy bucket should remain untouched");

        let _ = std::fs::remove_dir_all(&root);
    }

    #[test]
    fn checkout_path_moves_reuse_payload_and_rebind_locations() {
        let root = std::env::temp_dir().join(format!("nose_cache_move_{}", std::process::id()));
        let source_a = root.join("checkout-a");
        let source_b = root.join("checkout-b");
        let cache = root.join("cache");
        let _ = std::fs::remove_dir_all(&root);
        std::fs::create_dir_all(&source_a).unwrap();
        std::fs::create_dir_all(&source_b).unwrap();
        let source = "def total(xs):\n    return sum(xs)\n";
        std::fs::write(source_a.join("same.py"), source).unwrap();
        std::fs::write(source_b.join("same.py"), source).unwrap();

        let options = DetectOptions::default();
        let first_corpus = nose_frontend::lower_corpus_filtered(&[source_a.as_path()], &[]);
        let first = build_units_cached(&first_corpus, &options, &cache);
        assert_eq!((first.stats.hits, first.stats.misses), (0, 1));
        let second_corpus = nose_frontend::lower_corpus_filtered(&[source_b.as_path()], &[]);
        let second = build_units_cached(&second_corpus, &options, &cache);
        assert_eq!((second.stats.hits, second.stats.misses), (1, 0));
        assert!(second
            .units
            .iter()
            .all(|unit| unit.path.starts_with(source_b.to_string_lossy().as_ref())));
        let _ = std::fs::remove_dir_all(&root);
    }

    #[test]
    fn evidence_only_change_misses_then_reuses_resolved_artifact() {
        use nose_il::{
            EvidenceAnchor, EvidenceId, EvidenceKind, EvidenceProvenance, EvidenceRecord,
            EvidenceStatus, ParameterShapeEvidenceKind,
        };

        let root = std::env::temp_dir().join(format!("nose_cache_evidence_{}", std::process::id()));
        let source = root.join("source");
        let cache = root.join("cache");
        let _ = std::fs::remove_dir_all(&root);
        std::fs::create_dir_all(&source).unwrap();
        std::fs::write(source.join("same.py"), "def f(x):\n    return x\n").unwrap();
        let options = DetectOptions::default();
        let corpus = nose_frontend::lower_corpus_filtered(&[source.as_path()], &[]);
        let first = build_units_cached(&corpus, &options, &cache);
        assert_eq!((first.stats.hits, first.stats.misses), (0, 1));

        let mut changed = corpus.clone();
        let il = &mut changed.files[0];
        let span = il.node(il.root).span;
        il.evidence.push(EvidenceRecord::new(
            EvidenceId(il.evidence.len() as u32),
            EvidenceAnchor::param(span),
            EvidenceKind::ParameterShape(ParameterShapeEvidenceKind::NonPlain),
            EvidenceProvenance::builtin("nose.test", "evidence-only"),
            Vec::new(),
            EvidenceStatus::Asserted,
        ));
        let second = build_units_cached(&changed, &options, &cache);
        assert_eq!((second.stats.hits, second.stats.misses), (0, 1));
        let third = build_units_cached(&changed, &options, &cache);
        assert_eq!((third.stats.hits, third.stats.misses), (1, 0));
        let _ = std::fs::remove_dir_all(&root);
    }
}
