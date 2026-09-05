//! Optional dependency-aware layered content-addressed cache. Working source bytes
//! and the frontend extension profile identify every source, including Git-tracked
//! files. Raw IL, consumer-visible export/dependency summaries,
//! affected resolved IL, and detection units are separate stages.
//!
//! A resolved key contains the raw IL identity plus only facts that can affect that
//! region: imported literal/namespace export surfaces, Rust/Java/Go module outcomes,
//! unresolved-dependency catalogs, and Swift corpus-global sentinels. Provider
//! implementation edits therefore leave importer keys stable when the export
//! surface is unchanged, while export edits still invalidate #275 consumers.
//! Paths and process-local ids stay outside portable keys and are rebound on a hit,
//! so checkout moves and sorted `FileId` shifts do not fan out invalidation.
//!
//! Every stage uses the checksummed CAS envelope. Corrupt, truncated, or misplaced
//! entries are misses. Immutable state records commit through one complete workspace
//! generation; exact identities keep them acceleration-only, never correctness inputs.
//! `NOSE_CACHE_STATS` also emits a machine-readable
//! `nose.invalidation/v1` closure with exact reasons and explicit over-invalidation.

mod admin;
#[cfg(all(test, unix))]
mod basic_tests;
mod detection;
mod digest;
mod fast_units;
mod lines;
mod markdown;
pub(crate) use markdown::detect as detect_markdown;
mod portable_il;
mod resolved;
mod source;
mod store;
mod transaction;

use nose_detect::{DetectOptions, Stream, UnitFeat};
use nose_il::{Corpus, Lang};
use rayon::prelude::*;
use std::path::Path;
use std::sync::atomic::{AtomicU64, AtomicUsize, Ordering};

pub(crate) use self::admin::{
    clear as clear_store, prune as prune_store, status as store_status, PruneReport,
    DEFAULT_MAX_BYTES,
};
pub(crate) use self::detection::{
    load_detection_state, store_detection_state, DetectionCacheIdentity,
};
use self::digest::ContentDigest;
pub(crate) use self::fast_units::FastCachedUnits;
pub(crate) use self::fast_units::FastUnitSession;
pub(crate) use self::lines::{build_line_index, LineIndexStats};
pub(crate) use self::resolved::{CachedCorpus, InvalidationReport};
use self::store::{ArtifactKey, ArtifactStage};
pub(crate) use self::transaction::CacheRun;

#[derive(Clone, serde::Deserialize, serde::Serialize)]
pub(crate) struct CachedSourceFile {
    pub(crate) path: String,
    pub(crate) logical_path: String,
    pub(crate) digest: [u8; 32],
    pub(crate) lang: Lang,
    pub(crate) source_kind: source::SourceIdentityKind,
    #[serde(default)]
    pub(crate) skip_reason: Option<String>,
}

#[derive(Clone, serde::Deserialize, serde::Serialize)]
pub(crate) struct CachedUnitContext {
    pub(crate) region_path: String,
    pub(crate) region_id: String,
    pub(crate) source_path: String,
    pub(crate) source_digest: [u8; 32],
    pub(crate) source_kind: source::SourceIdentityKind,
    pub(crate) lang: Lang,
    pub(crate) resolution_digest: [u8; 32],
    pub(crate) export_digest: [u8; 32],
    pub(crate) requires_resolution: bool,
    pub(crate) over_invalidated: bool,
    pub(crate) depended_on: bool,
}

#[derive(Clone, serde::Deserialize, serde::Serialize)]
pub(crate) struct CachedUnitSnapshot {
    pub(crate) contexts: Vec<CachedUnitContext>,
    pub(crate) source_files: Vec<CachedSourceFile>,
    pub(crate) semantic_pack_digest: [u8; 32],
    pub(crate) discovery_digest: [u8; 32],
    pub(crate) global_line_statistics_digest: [u8; 32],
    pub(crate) swift_global_digest: [u8; 32],
    pub(crate) swift_global_active: bool,
    pub(crate) fast_safe: bool,
    pub(crate) artifacts: Vec<[u8; 32]>,
    pub(crate) unit_pack: Option<[u8; 32]>,
}

pub(crate) struct CachedLineContext {
    pub(crate) source_files: std::sync::Arc<Vec<CachedSourceFile>>,
    pub(crate) run: CacheRun,
}

pub(crate) fn finish_query_run(context: Option<CachedLineContext>) {
    let Some(context) = context else { return };
    if let Err(error) = context.run.commit() {
        if std::env::var_os("NOSE_CACHE_STATS").is_some() {
            eprintln!("  [cache-generation] commit skipped: {error}");
        }
    } else if std::env::var_os("NOSE_CACHE_STATS").is_some() {
        eprintln!(
            "  [cache-generation] written_bytes={}",
            context.run.written_bytes()
        );
    }
    enforce_run_budget(context.run);
}

pub(crate) fn build_corpus_cached(
    roots: &[&Path],
    exclude: &[String],
    dir: &Path,
    semantic_packs: &nose_semantics::SemanticPackSet,
    max_bytes: u64,
) -> CachedCorpus {
    // Raw/resolved artifacts and the dependency snapshot are part of the
    // incremental-cache contract: provider changes, path additions, and watch
    // restarts all depend on that history to make precise invalidation choices.
    let run = CacheRun::with_limit(dir, max_bytes);
    let raw = source::build_raw_corpus_cached(roots, exclude, &run);
    let mut cached =
        resolved::build_resolved_corpus_cached(raw, &run, semantic_pack_digest(semantic_packs));
    cached.unit_snapshot.fast_safe = semantic_packs_allow_fast_units(semantic_packs);
    cached
}

pub(crate) fn try_build_units_fast(
    roots: &[&Path],
    exclude: &[String],
    dir: &Path,
    semantic_packs: &nose_semantics::SemanticPackSet,
    max_bytes: u64,
    opts: &DetectOptions,
) -> Option<FastCachedUnits> {
    semantic_packs_allow_fast_units(semantic_packs).then_some(())?;
    fast_units::try_build(
        roots,
        exclude,
        dir,
        max_bytes,
        opts,
        *semantic_pack_digest(semantic_packs).as_bytes(),
    )
}

fn semantic_packs_allow_fast_units(packs: &nose_semantics::SemanticPackSet) -> bool {
    packs.external_evidence_producer_rows().is_empty()
        && packs.external_contract_rows().is_empty()
        && packs.external_value_law_rows().is_empty()
        && packs.compiled_external_v1_packs().is_empty()
}

pub(crate) fn invalidation_report_json(report: &InvalidationReport) -> String {
    serde_json::to_string(report).expect("invalidation report is always JSON serializable")
}

pub(crate) fn incremental_detection_stats_json(
    stats: &nose_detect::IncrementalDetectionStats,
) -> String {
    serde_json::to_string(stats).expect("incremental detection stats are JSON serializable")
}

pub(crate) fn line_index_stats_json(stats: &LineIndexStats) -> String {
    serde_json::to_string(stats).expect("line index stats are JSON serializable")
}

pub(crate) fn enforce_run_budget(run: CacheRun) {
    admin::enforce_run_budget(run);
}

fn semantic_pack_digest(packs: &nose_semantics::SemanticPackSet) -> ContentDigest {
    let mut rows = packs
        .packs()
        .iter()
        .map(|pack| {
            format!(
                "{}\0{}\0{}\0{}",
                pack.id,
                pack.version,
                pack.hash_hex(),
                pack.semantic_digest.as_deref().unwrap_or("")
            )
        })
        .collect::<Vec<_>>();
    rows.sort();
    let rows = rows.iter().map(String::as_bytes).collect::<Vec<_>>();
    ContentDigest::derive(b"nose.semantic-pack-influence.v1", &rows)
}

/// Bump when unit/stream serialization, extraction, or feature hashing changes.
const UNITS_SYNTAX_SCHEMA: u32 = 5;
const UNITS_PACK_SCHEMA: u32 = 4;
const UNITS_PACK_MAGIC: &[u8; 8] = b"NOSEUPK2";
const UNITS_PACK_HEADER_LEN: usize = 8 + 4 + 32 + 32;
const UNITS_PACK_ENTRY_LEN: usize = 32 + 4 + 8 + 8;

/// Portable raw/resolved IL is a fallback cache below the bounded unit fast
/// path. On large one-shot scans, publishing one artifact per source costs more
/// than re-lowering on the uncommon fallback, so keep that layer bounded.
pub(super) const MAX_FOREGROUND_PORTABLE_IL_FILES: usize = 512;

pub(crate) struct CachedUnits {
    pub units: Vec<UnitFeat>,
    pub unit_keys: Vec<[u8; 32]>,
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
#[cfg(test)]
fn build_units_cached(mut corpus: Corpus, opts: &DetectOptions, dir: &Path) -> CachedUnits {
    build_units_cached_inner(&mut corpus, opts, &CacheRun::new(dir), None).into_public()
}

pub(crate) fn build_units_cached_with_context(
    corpus: &mut Corpus,
    opts: &DetectOptions,
    run: &CacheRun,
    mut snapshot: CachedUnitSnapshot,
) -> CachedUnits {
    assert_eq!(corpus.files.len(), snapshot.contexts.len());
    let cached = build_units_cached_inner(corpus, opts, run, Some(&snapshot.contexts));
    snapshot.artifacts.clone_from(&cached.region_artifacts);
    snapshot.unit_pack = cached.unit_pack;
    fast_units::store_snapshot(run, opts, &snapshot);
    cached.into_public()
}

struct CachedUnitsInner {
    public: CachedUnits,
    region_artifacts: Vec<[u8; 32]>,
    unit_pack: Option<[u8; 32]>,
}

struct RestoredUnitFile {
    units: Vec<UnitFeat>,
    stream: Stream,
    artifact: ContentDigest,
    path: String,
    packed_payload: Option<Vec<u8>>,
    payload_checksum: Option<u32>,
}

struct PreparedUnitPack {
    key: ArtifactKey,
    header: Vec<u8>,
    table: Vec<u8>,
    payloads: Vec<Vec<u8>>,
    stored_bytes: u64,
}

impl CachedUnitsInner {
    fn into_public(self) -> CachedUnits {
        self.public
    }
}

fn build_units_cached_inner(
    corpus: &mut Corpus,
    opts: &DetectOptions,
    run: &CacheRun,
    contexts: Option<&[CachedUnitContext]>,
) -> CachedUnitsInner {
    let cas = run.cas();
    let options = options_digest(opts);
    let hits = AtomicUsize::new(0);
    let misses = AtomicUsize::new(0);
    let read_bytes = AtomicU64::new(0);
    let written_bytes = AtomicU64::new(0);
    let pack_units = contexts.is_some() && !run.writes_portable_il();

    // Detection owns the resolved IL after semantic-pack registries and query scope
    // have been built. Drain it here so each file can be released as soon as its
    // cached features are restored; retaining the whole resolved corpus alongside
    // every UnitFeat makes warm cache hits peak near a clean scan's RSS.
    let files = std::mem::take(&mut corpus.files);
    let interner = &corpus.interner;
    let restore = |(index, il): (usize, nose_il::Il)| {
        let path = il.meta.path.clone();
        let context = contexts.map(|contexts| &contexts[index]);
        let key = match context {
            Some(context) => unit_artifact_key(context, options),
            None => {
                let resolved = portable_il::semantic_digest(&il, interner);
                ArtifactKey::derive(
                    ArtifactStage::UnitsSyntax,
                    UNITS_SYNTAX_SCHEMA,
                    &[resolved.as_bytes(), options.as_bytes()],
                )
            }
        };

        if let Some(entry) = cas.load(key) {
            read_bytes.fetch_add(entry.stored_bytes, Ordering::Relaxed);
            if let Ok((mut units, mut stream)) =
                rmp_serde::from_slice::<(Vec<UnitFeat>, Stream)>(&entry.payload)
            {
                hits.fetch_add(1, Ordering::Relaxed);
                retarget(&mut units, &mut stream, &path);
                let payload_checksum = pack_units.then(|| crc32fast::hash(&entry.payload));
                let payload = pack_units.then_some(entry.payload);
                return RestoredUnitFile {
                    units,
                    stream,
                    artifact: key.digest,
                    path,
                    packed_payload: payload,
                    payload_checksum,
                };
            }
        }

        misses.fetch_add(1, Ordering::Relaxed);
        let mut units = nose_detect::units_of_file(&il, interner, opts);
        let mut stream = nose_detect::file_stream(&il, interner);
        // Checkout paths are presentation state, not payload identity. Blank
        // them before serialization and restore them for this query.
        retarget(&mut units, &mut stream, "");
        // UnitFeat's cache representation is a fixed-width record owned by this
        // stage schema, so compact MessagePack avoids repeating field names.
        let payload = rmp_serde::to_vec(&(&units, &stream));
        retarget(&mut units, &mut stream, &path);
        let mut packed_payload = None;
        let mut payload_checksum = None;
        if let Ok(payload) = payload {
            if pack_units {
                payload_checksum = Some(crc32fast::hash(&payload));
                packed_payload = Some(payload);
            } else if let Ok(bytes) = cas.store(key, &payload) {
                written_bytes.fetch_add(bytes, Ordering::Relaxed);
            }
        }
        RestoredUnitFile {
            units,
            stream,
            artifact: key.digest,
            path,
            packed_payload,
            payload_checksum,
        }
    };
    // Match the clean scan's work-stealing granularity. Artificial batch
    // barriers leave cores idle behind a few expensive files and turn a large
    // checkout into dozens of mostly serial normalization waves. Moving the
    // ILs into the parallel iterator still lets each file be released as soon
    // as its cached features have been produced.
    let mut per_file = files
        .into_par_iter()
        .enumerate()
        .map(restore)
        .collect::<Vec<_>>();

    let region_artifacts = per_file
        .iter()
        .map(|region| *region.artifact.as_bytes())
        .collect::<Vec<_>>();
    let unit_pack = pack_units
        .then(|| prepare_unit_pack(&region_artifacts, &mut per_file))
        .flatten()
        .map(|pack| {
            let digest = *pack.key.digest.as_bytes();
            written_bytes.fetch_add(pack.stored_bytes, Ordering::Relaxed);
            let pack_cas = run.cas();
            run.spawn_write(move || publish_unit_pack(&pack_cas, pack));
            digest
        });
    let files = per_file.len();
    assemble_cached_units(
        per_file,
        region_artifacts,
        unit_pack,
        CacheStats {
            files,
            hits: hits.load(Ordering::Relaxed),
            misses: misses.load(Ordering::Relaxed),
            read_bytes: read_bytes.load(Ordering::Relaxed),
            written_bytes: written_bytes.load(Ordering::Relaxed),
        },
    )
}

fn assemble_cached_units(
    per_file: Vec<RestoredUnitFile>,
    region_artifacts: Vec<[u8; 32]>,
    unit_pack: Option<[u8; 32]>,
    stats: CacheStats,
) -> CachedUnitsInner {
    let mut all_units = Vec::new();
    let mut unit_keys = Vec::new();
    let mut all_streams = Vec::new();
    for region in per_file {
        for index in 0..region.units.len() {
            let ordinal = (index as u64).to_be_bytes();
            unit_keys.push(
                *ContentDigest::derive(
                    b"nose.cached-unit-identity.v1",
                    &[region.artifact.as_bytes(), region.path.as_bytes(), &ordinal],
                )
                .as_bytes(),
            );
        }
        all_units.extend(region.units);
        all_streams.push(region.stream);
    }
    CachedUnitsInner {
        public: CachedUnits {
            units: all_units,
            unit_keys,
            streams: all_streams,
            files: stats.files,
            stats,
        },
        region_artifacts,
        unit_pack,
    }
}

fn prepare_unit_pack(
    artifacts: &[[u8; 32]],
    regions: &mut [RestoredUnitFile],
) -> Option<PreparedUnitPack> {
    let count = u32::try_from(regions.len()).ok()?;
    let table_len =
        UNITS_PACK_HEADER_LEN.checked_add(UNITS_PACK_ENTRY_LEN.checked_mul(regions.len())?)?;
    let mut table = Vec::with_capacity(table_len);
    let mut offset = 0u64;
    for (artifact, region) in artifacts.iter().zip(regions.iter()) {
        let payload = region.packed_payload.as_deref()?;
        let payload_checksum = region.payload_checksum?;
        let len = u64::try_from(payload.len()).ok()?;
        table.extend_from_slice(artifact);
        table.extend_from_slice(&payload_checksum.to_be_bytes());
        table.extend_from_slice(&offset.to_be_bytes());
        table.extend_from_slice(&len.to_be_bytes());
        offset = offset.checked_add(len)?;
    }
    let key = ArtifactKey::derive(
        ArtifactStage::UnitsSyntax,
        UNITS_PACK_SCHEMA,
        &[b"nose.unit-pack.v2", &table],
    );
    let table_digest = ContentDigest::sha256(&table);
    let mut header = Vec::with_capacity(UNITS_PACK_HEADER_LEN);
    header.extend_from_slice(UNITS_PACK_MAGIC);
    header.extend_from_slice(&count.to_be_bytes());
    header.extend_from_slice(key.digest.as_bytes());
    header.extend_from_slice(table_digest.as_bytes());
    let payloads = regions
        .iter_mut()
        .map(|region| region.packed_payload.take())
        .collect::<Option<Vec<_>>>()?;
    let stored_bytes = header
        .len()
        .checked_add(table.len())?
        .checked_add(payloads.iter().map(Vec::len).sum::<usize>())?;
    Some(PreparedUnitPack {
        key,
        header,
        table,
        payloads,
        stored_bytes: u64::try_from(stored_bytes).ok()?,
    })
}

fn publish_unit_pack(cas: &store::LayeredCas, pack: PreparedUnitPack) -> std::io::Result<u64> {
    let mut parts = Vec::with_capacity(pack.payloads.len() + 2);
    parts.push(pack.header.as_slice());
    parts.push(pack.table.as_slice());
    for payload in &pack.payloads {
        parts.push(payload.as_slice());
    }
    cas.store_chunked(pack.key, &parts)
}

fn unit_artifact_key(context: &CachedUnitContext, options: ContentDigest) -> ArtifactKey {
    ArtifactKey::derive(
        ArtifactStage::UnitsSyntax,
        UNITS_SYNTAX_SCHEMA,
        &[
            &context.source_digest,
            context.region_id.as_bytes(),
            &context.resolution_digest,
            options.as_bytes(),
        ],
    )
}

fn retarget(units: &mut [UnitFeat], stream: &mut Stream, path: &str) {
    for unit in units {
        path.clone_into(&mut unit.path);
        unit.source_document = stream.source_document();
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
