//! Optional on-disk cache of per-file detection units, keyed by the **resolved
//! IL** content hash. Re-running nose on a project where most files are unchanged
//! then skips the dominant cost (normalize + extract) for those files and
//! deserializes their units instead.
//!
//! The corpus is lowered AND cross-file-resolved every run
//! (`lower_corpus_filtered` — parse + lower + `resolve_imported_immutable_bindings`,
//! the smaller half of the work per experiments §BQ); only the dominant
//! normalize+extract step is cached. The key is a content hash of each file's
//! *post-resolve* IL, so a file whose imported-immutable-literal context changed
//! (its provider edited) gets a different key and recomputes — fixing #275, where
//! the old source-content key skipped resolution entirely and the cached scan
//! under-merged cross-file imported-literal convergence. A [`UnitFeat`]'s features
//! are interner-independent content hashes, so a hit needs no interner; the key
//! folds in a schema version and an options signature so a format/option change
//! transparently misses.

use nose_detect::{DetectOptions, Stream, UnitFeat};
use nose_il::{Corpus, Interner};
use rayon::prelude::*;
use std::path::Path;

/// Bump when the cached payload's layout, extraction, or feature hashing changes — old
/// cache entries then live under a different directory and are ignored. (v8: keyed by
/// the post-resolve IL hash, not the source-content hash — fixes #275.)
const SCHEMA: u32 = 8;

pub(crate) struct CachedUnits {
    pub units: Vec<UnitFeat>,
    pub streams: Vec<Stream>,
    pub files: usize,
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

    let per_file: Vec<(Vec<UnitFeat>, Stream)> = corpus
        .files
        .par_iter()
        .map(|il| {
            let path = il.meta.path.clone();
            // Key on the post-resolve IL content hash — a stable, interner-
            // independent fold of every node's structural hash. Two files with
            // the same resolved IL (incl. identical imported-literal context)
            // share the entry; a provider edit changes a dependent's key.
            let key = resolved_il_hash(il, &corpus.interner);
            let entry = bucket.join(format!("{key:016x}.json"));

            if let Ok(bytes) = std::fs::read(&entry) {
                if let Ok((mut units, mut stream)) =
                    serde_json::from_slice::<(Vec<UnitFeat>, Stream)>(&bytes)
                {
                    for u in &mut units {
                        u.path = path.clone();
                    }
                    stream.set_path(path.clone());
                    return (units, stream);
                }
            }

            let units = nose_detect::units_of_file(il, &corpus.interner, opts);
            let stream = nose_detect::file_stream(il, &corpus.interner);
            if let Ok(bytes) = serde_json::to_vec(&(&units, &stream)) {
                let _ = std::fs::write(&entry, bytes);
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
    }
}

/// Content hash of a file's *post-resolve* IL — the cache key. Uses
/// `valued_tree_hash`: an interner-INDEPENDENT fold that retains literal values.
/// Interner-independence is essential because the corpus shares one interner whose
/// symbol ids depend on parallel interning order — serializing the raw IL (with
/// those ids) gave a key that varied run-to-run and never warm-hit. Value-retention
/// is essential because the structural `subtree_hashes` erases literal values, so a
/// resolved `LOOKUP = {…: 1}` vs `{…: 9}` would collide — the very post-resolve
/// distinction #275 turns on. The language and resolution evidence count
/// (resolution always rewrites the inlined literal into the nodes, which the valued
/// hash captures; the evidence count guards the rare evidence-only delta) complete
/// the key.
fn resolved_il_hash(il: &nose_il::Il, interner: &Interner) -> u64 {
    let mut h = crate::fnv::OFFSET_BASIS;
    h = crate::fnv::mix(h, il.meta.lang as u8 as u64);
    h = crate::fnv::mix(h, il.evidence.len() as u64);
    crate::fnv::mix(h, nose_normalize::valued_tree_hash(il, interner))
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
}
