use crate::{
    contiguous::{self, Stream},
    minhash,
    options::DetectOptions,
    units::{self, UnitFeat},
};
use nose_il::{Corpus, Il, Interner};
use nose_normalize::NormalizeOptions;
use rayon::prelude::*;

/// Build one file's syntax-channel token stream from its (raw) IL. Exposed so the
/// CLI's `--cache-dir` can cache it per file and pass it to [`super::detect_from_units`] —
/// the counterpart to [`units_of_file`] for the syntax channel.
pub fn file_stream(il: &Il, interner: &Interner) -> Stream {
    contiguous::stream(il, interner)
}

/// Normalize one file and extract its detection units. The resulting [`UnitFeat`]s
/// are interner-independent (every feature is a content-derived hash), so a caller
/// may pass a throwaway per-file interner — which is exactly what makes caching a
/// file's units by its source-content hash sound.
pub fn units_of_file(il: &Il, interner: &Interner, opts: &DetectOptions) -> Vec<UnitFeat> {
    let norm_opts = NormalizeOptions {
        cfg_norm: opts.cfg_norm,
        dce: opts.dce,
        ..Default::default()
    };
    let seeds = minhash::seeds(opts.minhash_k);
    extract_units_of_file(il, interner, opts, &norm_opts, &seeds)
}

/// Corpus features ready for detection. The caller may attach query-local evidence to
/// `units` before handing the immutable feature set to [`super::detect_from_units`].
pub struct CorpusFeatures {
    pub units: Vec<UnitFeat>,
    pub streams: Vec<Stream>,
    pub files: usize,
}

type RetainedFileFeatures = (
    Vec<UnitFeat>,
    Option<Stream>,
    Option<Il>,
    Option<nose_normalize::ValueFingerprintContext>,
);

pub fn corpus_features(corpus: &Corpus, opts: &DetectOptions) -> CorpusFeatures {
    let norm_opts = NormalizeOptions {
        cfg_norm: opts.cfg_norm,
        dce: opts.dce,
        ..Default::default()
    };
    let seeds = minhash::seeds(opts.minhash_k);
    let per_file: Vec<(Vec<UnitFeat>, Option<Stream>)> = corpus
        .files
        .par_iter()
        .map(|il| {
            let units = if opts.structural {
                extract_units_of_file(il, &corpus.interner, opts, &norm_opts, &seeds)
            } else {
                Vec::new()
            };
            // Build contiguous copy-paste streams from raw IL. Alpha-renaming is
            // function-scoped, so normalized identifiers would vary by enclosing unit;
            // renamed Type-2/3/4 matches remain the structural channel's job.
            let stream = opts
                .contiguous
                .then(|| contiguous::stream(il, &corpus.interner));
            (units, stream)
        })
        .collect();
    let unit_count = per_file.iter().map(|(units, _)| units.len()).sum();
    let stream_count = per_file
        .iter()
        .filter(|(_, stream)| stream.is_some())
        .count();
    let mut units = Vec::with_capacity(unit_count);
    let mut streams = Vec::with_capacity(stream_count);
    for (file_units, stream) in per_file {
        units.extend(file_units);
        if let Some(stream) = stream {
            streams.push(stream);
        }
    }
    CorpusFeatures {
        units,
        streams,
        files: corpus.files.len(),
    }
}

/// Build detection features while retaining the normalized IL that produced
/// structural units. Divergent-edit queries use the retained corpus for their
/// downstream semantic witness instead of parsing and normalizing the base files
/// a second time. Ordinary detection keeps using [`corpus_features`] so its hot
/// path and peak lifetime stay unchanged.
pub fn corpus_features_with_normalized(
    corpus: &Corpus,
    opts: &DetectOptions,
) -> (
    CorpusFeatures,
    Corpus,
    Vec<(String, nose_normalize::ValueFingerprintContext)>,
) {
    let norm_opts = NormalizeOptions {
        cfg_norm: opts.cfg_norm,
        dce: opts.dce,
        ..Default::default()
    };
    let seeds = minhash::seeds(opts.minhash_k);
    let per_file: Vec<RetainedFileFeatures> = corpus
        .files
        .par_iter()
        .map(|il| {
            let (units, normalized, context) = if opts.structural {
                extract_units_of_file_with_normalized(
                    il,
                    &corpus.interner,
                    opts,
                    &norm_opts,
                    &seeds,
                )
            } else {
                (Vec::new(), None, None)
            };
            let stream = opts
                .contiguous
                .then(|| contiguous::stream(il, &corpus.interner));
            (units, stream, normalized, context)
        })
        .collect();
    let unit_count = per_file.iter().map(|(units, _, _, _)| units.len()).sum();
    let stream_count = per_file
        .iter()
        .filter(|(_, stream, _, _)| stream.is_some())
        .count();
    let normalized_count = per_file
        .iter()
        .filter(|(_, _, normalized, _)| normalized.is_some())
        .count();
    let mut units = Vec::with_capacity(unit_count);
    let mut streams = Vec::with_capacity(stream_count);
    let mut normalized = Vec::with_capacity(normalized_count);
    let mut contexts = Vec::new();
    for (file_units, stream, file_normalized, context) in per_file {
        units.extend(file_units);
        if let Some(stream) = stream {
            streams.push(stream);
        }
        if let (Some(file_normalized), context) = (file_normalized, context) {
            if let Some(context) = context {
                contexts.push((file_normalized.meta.path.clone(), context));
            }
            normalized.push(file_normalized);
        }
    }
    (
        CorpusFeatures {
            units,
            streams,
            files: corpus.files.len(),
        },
        Corpus::new(corpus.interner.clone(), normalized),
        contexts,
    )
}

/// Keep the normalization/extraction body out of the Rayon closure. This path
/// is large and hot; sharing one non-inlined implementation with the cached
/// per-file entry point avoids code-layout-sensitive copies while preserving
/// the fused normalize-then-extract lifetime.
#[inline(never)]
fn extract_units_of_file(
    il: &Il,
    interner: &Interner,
    opts: &DetectOptions,
    norm_opts: &NormalizeOptions,
    seeds: &[u64],
) -> Vec<UnitFeat> {
    if units::raw_il_is_empty_module(il) || units::large_test_file(il) {
        return Vec::new();
    }
    let n = nose_normalize::normalize(il, interner, norm_opts);
    let block_units = units::block_units_for_file(&n, opts);
    units::extract(
        &n,
        interner,
        seeds,
        opts.min_lines,
        opts.min_tokens,
        block_units,
        units::ExtractFeatures {
            shape_features: opts.shape_features,
            abstraction_witnesses: opts.abstraction_witnesses,
            connected_witnesses: opts.connected_witnesses,
        },
    )
}

#[inline(never)]
fn extract_units_of_file_with_normalized(
    il: &Il,
    interner: &Interner,
    opts: &DetectOptions,
    norm_opts: &NormalizeOptions,
    seeds: &[u64],
) -> (
    Vec<UnitFeat>,
    Option<Il>,
    Option<nose_normalize::ValueFingerprintContext>,
) {
    if units::raw_il_is_empty_module(il) || units::large_test_file(il) {
        return (Vec::new(), None, None);
    }
    let normalized = nose_normalize::normalize(il, interner, norm_opts);
    let block_units = units::block_units_for_file(&normalized, opts);
    let (extracted, context) = units::extract_with_context(
        &normalized,
        interner,
        seeds,
        opts.min_lines,
        opts.min_tokens,
        block_units,
        units::ExtractFeatures {
            shape_features: opts.shape_features,
            abstraction_witnesses: opts.abstraction_witnesses,
            connected_witnesses: opts.connected_witnesses,
        },
    );
    (extracted, Some(normalized), context)
}
