//! Optional on-disk cache of per-file detection units, keyed by source-content
//! hash. Re-running nose on a project where most files are unchanged then skips
//! the dominant cost (parse + lower + normalize + extract) for those files and
//! deserializes their units instead.
//!
//! Soundness rests on one property: a [`UnitFeat`]'s features are all
//! content-derived hashes (interner-independent), so a file's units depend only on
//! its bytes, its language, and the unit-affecting options — never on the rest of
//! the corpus. Each file is therefore lowered with a throwaway interner and cached
//! independently. The cache key folds in a schema version and an options signature,
//! so a format or option change transparently misses (never returns stale units).

use nose_detect::{DetectOptions, Stream, UnitFeat};
use nose_il::{FileId, Interner, Lang};
use rayon::prelude::*;
use std::path::Path;

/// Bump when the cached payload's layout, extraction, or feature hashing changes — old
/// cache entries then live under a different directory and are ignored. (v5: exact
/// Type-4 features include the newly modeled record, membership, flag-loop, and ordered
/// string-builder idioms.)
const SCHEMA: u32 = 6;

pub(crate) struct CachedUnits {
    pub units: Vec<UnitFeat>,
    pub streams: Vec<Stream>,
    pub files: usize,
    pub langs: Vec<(&'static str, usize)>,
}

/// Build detection units **and contiguous-channel streams** for every source file under
/// `roots`, using the on-disk cache at `dir`.
/// Falls back to recomputation for any file that misses (or whose entry fails to
/// read/parse), writing it back. Both the units and the stream are content-derived
/// (interner-independent), so a file's entry depends only on its bytes/language/options.
pub(crate) fn build_units_cached(
    roots: &[&Path],
    exclude: &[String],
    opts: &DetectOptions,
    dir: &Path,
) -> CachedUnits {
    // One bucket per (schema, options signature): changing an option that affects
    // units lands in a fresh bucket, so stale entries are never read.
    let bucket = dir.join(format!("v{SCHEMA}-{:016x}", options_signature(opts)));
    let _ = std::fs::create_dir_all(&bucket);

    // Discover + sort (same deterministic order the non-cached path uses).
    let mut paths: Vec<(String, Lang)> = roots
        .iter()
        .flat_map(|r| nose_frontend::discover_paths(r, exclude))
        .collect();
    paths.sort_unstable_by(|a, b| a.0.cmp(&b.0));
    let mut counts: std::collections::HashMap<&'static str, usize> =
        std::collections::HashMap::new();
    for (_, lang) in &paths {
        *counts.entry(lang.name()).or_insert(0) += 1;
    }
    let mut langs: Vec<(&'static str, usize)> = counts.into_iter().collect();
    langs.sort_by(|a, b| b.1.cmp(&a.1).then(a.0.cmp(b.0)));

    let per_file: Vec<(Vec<UnitFeat>, Option<Stream>)> = paths
        .par_iter()
        .map(|(path, lang)| {
            let src = match std::fs::read(path) {
                Ok(s) => s,
                Err(_) => return (Vec::new(), None),
            };
            let entry = bucket.join(format!("{:016x}.json", content_hash(*lang, &src)));

            // Hit: load and retarget the path (identical content at another path
            // shares the entry; only `path` differs between them).
            if let Ok(bytes) = std::fs::read(&entry) {
                if let Ok((mut units, mut stream)) =
                    serde_json::from_slice::<(Vec<UnitFeat>, Stream)>(&bytes)
                {
                    for u in &mut units {
                        u.path = path.clone();
                    }
                    stream.set_path(path.clone());
                    return (units, Some(stream));
                }
            }

            // Miss: lower with a throwaway interner (features are portable), build the
            // units and the contiguous stream from that one IL, and write both back.
            let interner = Interner::new();
            let il = match nose_frontend::lower_source(FileId(0), path, &src, *lang, &interner) {
                Ok(il) => il,
                Err(_) => return (Vec::new(), None),
            };
            let units = nose_detect::units_of_file(&il, &interner, opts);
            let stream = nose_detect::file_stream(&il, &interner);
            if let Ok(bytes) = serde_json::to_vec(&(&units, &stream)) {
                let _ = std::fs::write(&entry, bytes);
            }
            (units, Some(stream))
        })
        .collect();

    // Count only files that actually read + lowered (a `Some` stream), so the reported
    // count matches the non-cached path, which `filter_map`s read/parse failures away.
    let files = per_file.iter().filter(|(_, s)| s.is_some()).count();
    let mut all_units = Vec::new();
    let mut all_streams = Vec::new();
    for (u, s) in per_file {
        all_units.extend(u);
        if let Some(s) = s {
            all_streams.push(s);
        }
    }
    CachedUnits {
        units: all_units,
        streams: all_streams,
        files,
        langs,
    }
}

/// 64-bit FNV-1a over the language tag and source bytes. Collisions are
/// astronomically unlikely at corpus scale; a clash would at worst reuse one file's
/// units for another (never a crash), so 64 bits is ample for a cache.
fn content_hash(lang: Lang, src: &[u8]) -> u64 {
    let mut h = crate::fnv::OFFSET_BASIS;
    h = crate::fnv::mix(h, lang as u8 as u64);
    for &b in src {
        h = crate::fnv::mix(h, b as u64);
    }
    h
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

    /// `files` must count only files that were actually read+lowered — matching the
    /// non-cached path (which `filter_map`s read/parse failures away). A discovered but
    /// unreadable file used to inflate the count.
    #[test]
    fn file_count_excludes_unreadable_files() {
        let dir = std::env::temp_dir().join(format!("nose_cache_count_{}", std::process::id()));
        let cache = std::env::temp_dir().join(format!("nose_cache_dir_{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::create_dir_all(&cache).unwrap();

        std::fs::write(dir.join("ok.py"), "def f():\n    return 1\n").unwrap();
        let bad = dir.join("bad.py");
        std::fs::write(&bad, "def g():\n    return 2\n").unwrap();
        // Make `bad.py` unreadable so `std::fs::read` fails (skips root, where this is moot).
        std::fs::set_permissions(&bad, std::fs::Permissions::from_mode(0o000)).unwrap();

        let readable = std::fs::read(&bad).is_ok();
        let out = build_units_cached(&[dir.as_path()], &[], &DetectOptions::default(), &cache);

        // Restore perms so cleanup can proceed.
        let _ = std::fs::set_permissions(&bad, std::fs::Permissions::from_mode(0o644));
        let _ = std::fs::remove_dir_all(&dir);
        let _ = std::fs::remove_dir_all(&cache);

        if readable {
            // Running as root (CI sometimes) — the unreadable file is still readable, so
            // the discrepancy can't be exercised; don't assert.
            return;
        }
        assert_eq!(out.files, 1, "only the readable file should be counted");
    }
}
