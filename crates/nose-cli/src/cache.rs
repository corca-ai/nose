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
/// cache entries then live under a different directory and are ignored. (v2: each entry
/// now stores the file's contiguous-channel [`Stream`] alongside its units, so the cache
/// path runs the copy-paste channel too instead of silently dropping it.)
const SCHEMA: u32 = 2;

/// Build detection units **and contiguous-channel streams** for every source file under
/// `roots`, using the on-disk cache at `dir`. Returns `(units, streams, files_seen)`.
/// Falls back to recomputation for any file that misses (or whose entry fails to
/// read/parse), writing it back. Both the units and the stream are content-derived
/// (interner-independent), so a file's entry depends only on its bytes/language/options.
pub(crate) fn build_units_cached(
    roots: &[&Path],
    exclude: &[String],
    opts: &DetectOptions,
    dir: &Path,
) -> (Vec<UnitFeat>, Vec<Stream>, usize) {
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

    let files = per_file.len();
    let mut all_units = Vec::new();
    let mut all_streams = Vec::new();
    for (u, s) in per_file {
        all_units.extend(u);
        if let Some(s) = s {
            all_streams.push(s);
        }
    }
    (all_units, all_streams, files)
}

/// 64-bit FNV-1a over the language tag and source bytes. Collisions are
/// astronomically unlikely at corpus scale; a clash would at worst reuse one file's
/// units for another (never a crash), so 64 bits is ample for a cache.
fn content_hash(lang: Lang, src: &[u8]) -> u64 {
    let mut h: u64 = 0xcbf2_9ce4_8422_2325;
    let mut mix = |b: u8| {
        h ^= b as u64;
        h = h.wrapping_mul(0x0000_0100_0000_01b3);
    };
    mix(lang as u8);
    for &b in src {
        mix(b);
    }
    h
}

/// Fold every unit-affecting option into one value; changing any of them changes
/// the cache bucket. (`threshold`/`bands` only affect scoring/candidate-gen, not the
/// units themselves, so they are deliberately excluded.)
fn options_signature(opts: &DetectOptions) -> u64 {
    let mut h: u64 = 0xcbf2_9ce4_8422_2325;
    for v in [
        opts.min_lines as u64,
        opts.min_tokens as u64,
        opts.block_units as u64,
        opts.cfg_norm as u64,
        opts.dce as u64,
        opts.minhash_k as u64,
    ] {
        h = (h ^ v).wrapping_mul(0x0000_0100_0000_01b3);
    }
    h
}
