//! Bounded warm path for exact no-op and one independent leaf edit.
//!
//! The manifest never substitutes for semantic analysis unless its admission
//! proof is still true. Source membership and content identities are checked
//! exactly. A changed file must have neither incoming nor outgoing resolution
//! dependencies, must preserve its export and resolution summaries, and must
//! not participate in Swift-global or external semantic-pack influence. Every
//! other case falls back to the ordinary raw/resolved pipeline.

use super::digest::ContentDigest;
use super::portable_il;
use super::source;
use super::store::{ArtifactKey, ArtifactStage, LayeredCas};
use super::{
    CacheRun, CacheStats, CachedSourceFile, CachedUnitSnapshot, CachedUnits, InvalidationReport,
    UNITS_SYNTAX_SCHEMA,
};
use nose_detect::{DetectOptions, Stream, UnitFeat};
use nose_il::{Corpus, FileId, Interner, Lang};
use rayon::prelude::*;
use serde::{Deserialize, Serialize};
use std::path::Path;

const SNAPSHOT_SCHEMA: u32 = 2;

mod session;
pub(crate) use session::FastUnitSession;

pub(crate) struct FastCachedUnits {
    pub(crate) cached: CachedUnits,
    pub(crate) report: InvalidationReport,
    pub(crate) workspace_digest: [u8; 32],
    pub(crate) semantic_pack_digest: [u8; 32],
    pub(crate) source_files: Vec<CachedSourceFile>,
    pub(crate) run: CacheRun,
    pub(crate) langs: Vec<Lang>,
    pub(super) snapshot: CachedUnitSnapshot,
    pub(super) region_unit_counts: Vec<usize>,
}

#[derive(Deserialize, Serialize)]
struct StoredSnapshot {
    schema: u32,
    snapshot: CachedUnitSnapshot,
}

#[derive(Serialize)]
struct StoredSnapshotRef<'a> {
    schema: u32,
    snapshot: &'a CachedUnitSnapshot,
}

struct RestoredRegion {
    units: Vec<UnitFeat>,
    stream: Stream,
    artifact: [u8; 32],
    hit: bool,
    read_bytes: u64,
    written_bytes: u64,
}

struct UnitPackEntry {
    artifact: [u8; 32],
    payload_checksum: u32,
    start: usize,
    end: usize,
}

struct LoadedUnitPack {
    payload: Vec<u8>,
    entries: Vec<UnitPackEntry>,
    stored_bytes: u64,
}

pub(super) fn store_snapshot(run: &CacheRun, opts: &DetectOptions, snapshot: &CachedUnitSnapshot) {
    if !snapshot.fast_safe || snapshot.contexts.len() != snapshot.artifacts.len() {
        return;
    }
    let stored = StoredSnapshotRef {
        schema: SNAPSHOT_SCHEMA,
        snapshot,
    };
    let Ok(payload) = rmp_serde::to_vec_named(&stored) else {
        return;
    };
    run.store(&snapshot_slot(opts), SNAPSHOT_SCHEMA, &payload);
}

pub(super) fn try_build(
    roots: &[&Path],
    exclude: &[String],
    dir: &Path,
    max_bytes: u64,
    opts: &DetectOptions,
    semantic_pack_digest: [u8; 32],
) -> Option<FastCachedUnits> {
    let run = CacheRun::with_limit(dir, max_bytes);
    let workspace_digest = *source::workspace_digest(roots).as_bytes();
    run.set_workspace(workspace_digest);
    let bytes = run.load(&snapshot_slot(opts), SNAPSHOT_SCHEMA)?;
    let stored = rmp_serde::from_slice::<StoredSnapshot>(&bytes).ok()?;
    if stored.schema != SNAPSHOT_SCHEMA
        || !stored.snapshot.fast_safe
        || stored.snapshot.semantic_pack_digest != semantic_pack_digest
        || stored.snapshot.source_files.is_empty()
        || stored.snapshot.contexts.is_empty()
        || stored.snapshot.contexts.len() != stored.snapshot.artifacts.len()
    {
        return None;
    }
    let mut snapshot = stored.snapshot;
    let current_sources = source::discover_source_files(roots, exclude)?;
    let changed_source = compare_sources(&snapshot.source_files, &current_sources)?;
    let report = super::resolved::fast_invalidation_report(
        &snapshot,
        &current_sources,
        changed_source.as_deref(),
    );

    let cas = run.cas();
    let (restored, pack_read_bytes) = restore_snapshot_regions(
        &snapshot,
        &current_sources,
        changed_source.as_deref(),
        &cas,
        opts,
    )?;
    let mut all_units = Vec::new();
    let mut unit_keys = Vec::new();
    let mut streams = Vec::with_capacity(snapshot.contexts.len());
    let mut hits = 0;
    let mut misses = 0;
    let mut read_bytes = pack_read_bytes;
    let mut written_bytes = 0;
    let mut artifacts = Vec::with_capacity(snapshot.contexts.len());
    let mut region_unit_counts = Vec::with_capacity(snapshot.contexts.len());

    for (context, restored) in snapshot.contexts.iter().zip(restored) {
        hits += usize::from(restored.hit);
        misses += usize::from(!restored.hit);
        read_bytes += restored.read_bytes;
        written_bytes += restored.written_bytes;
        append_unit_keys(
            &mut unit_keys,
            restored.artifact,
            &context.region_path,
            restored.units.len(),
        );
        region_unit_counts.push(restored.units.len());
        all_units.extend(restored.units);
        streams.push(restored.stream);
        artifacts.push(restored.artifact);
    }
    let langs = update_snapshot(
        &mut snapshot,
        &current_sources,
        changed_source.as_deref(),
        None,
    )?;
    snapshot.artifacts = artifacts;
    store_snapshot(&run, opts, &snapshot);

    let files = streams.len();
    if langs.len() != files {
        return None;
    }
    Some(FastCachedUnits {
        cached: CachedUnits {
            units: all_units,
            unit_keys,
            streams,
            files,
            stats: CacheStats {
                files,
                hits,
                misses,
                read_bytes,
                written_bytes,
            },
        },
        report,
        workspace_digest,
        semantic_pack_digest,
        source_files: current_sources,
        run,
        langs,
        snapshot,
        region_unit_counts,
    })
}

fn restore_snapshot_regions(
    snapshot: &CachedUnitSnapshot,
    current_sources: &[CachedSourceFile],
    changed_source: Option<&str>,
    cas: &LayeredCas,
    opts: &DetectOptions,
) -> Option<(Vec<RestoredRegion>, u64)> {
    let pack = snapshot
        .unit_pack
        .and_then(|digest| load_unit_pack(cas, digest, snapshot.contexts.len()));
    let pack_read_bytes = pack.as_ref().map_or(0, |pack| pack.stored_bytes);
    let mut replacements = match changed_source {
        Some(path) => {
            let current = current_sources.iter().find(|source| source.path == path)?;
            rebuild_leaf(path, snapshot, current, cas, opts)?
        }
        None => std::collections::BTreeMap::new(),
    };
    let jobs = snapshot
        .contexts
        .iter()
        .enumerate()
        .map(|(index, context)| (index, context, replacements.remove(&index)))
        .collect::<Vec<_>>();
    if !replacements.is_empty() {
        return None;
    }
    let restored = jobs
        .into_par_iter()
        .map(|(index, context, replacement)| {
            replacement
                .or_else(|| {
                    pack.as_ref().and_then(|pack| {
                        load_packed_region(
                            pack,
                            index,
                            snapshot.artifacts[index],
                            &context.region_path,
                        )
                    })
                })
                .or_else(|| load_region(cas, snapshot.artifacts[index], &context.region_path))
        })
        .collect::<Option<Vec<_>>>()?;
    Some((restored, pack_read_bytes))
}

fn load_unit_pack(
    cas: &LayeredCas,
    digest: [u8; 32],
    expected_regions: usize,
) -> Option<LoadedUnitPack> {
    let key = ArtifactKey {
        stage: ArtifactStage::UnitsSyntax,
        schema: super::UNITS_PACK_SCHEMA,
        digest: ContentDigest::from_bytes(digest),
    };
    let entry = cas.load_chunked(key)?;
    let payload = entry.payload;
    if payload.len() < super::UNITS_PACK_HEADER_LEN || &payload[..8] != super::UNITS_PACK_MAGIC {
        return None;
    }
    let count = u32::from_be_bytes(payload[8..12].try_into().ok()?) as usize;
    let stored_key: [u8; 32] = payload[12..44].try_into().ok()?;
    let table_digest: [u8; 32] = payload[44..76].try_into().ok()?;
    if count != expected_regions || stored_key != digest {
        return None;
    }
    let data_start = super::UNITS_PACK_HEADER_LEN
        .checked_add(super::UNITS_PACK_ENTRY_LEN.checked_mul(count)?)?;
    if data_start > payload.len() {
        return None;
    }
    if ContentDigest::sha256(&payload[super::UNITS_PACK_HEADER_LEN..data_start]).as_bytes()
        != &table_digest
    {
        return None;
    }
    let mut entries = Vec::with_capacity(count);
    let mut expected_offset = 0usize;
    for index in 0..count {
        let cursor = super::UNITS_PACK_HEADER_LEN + index * super::UNITS_PACK_ENTRY_LEN;
        let artifact = payload[cursor..cursor + 32].try_into().ok()?;
        let payload_checksum =
            u32::from_be_bytes(payload[cursor + 32..cursor + 36].try_into().ok()?);
        let offset = usize::try_from(u64::from_be_bytes(
            payload[cursor + 36..cursor + 44].try_into().ok()?,
        ))
        .ok()?;
        let len = usize::try_from(u64::from_be_bytes(
            payload[cursor + 44..cursor + 52].try_into().ok()?,
        ))
        .ok()?;
        if offset != expected_offset {
            return None;
        }
        let start = data_start.checked_add(offset)?;
        let end = start.checked_add(len)?;
        if end > payload.len() {
            return None;
        }
        expected_offset = offset.checked_add(len)?;
        entries.push(UnitPackEntry {
            artifact,
            payload_checksum,
            start,
            end,
        });
    }
    if data_start.checked_add(expected_offset)? != payload.len() {
        return None;
    }
    Some(LoadedUnitPack {
        payload,
        entries,
        stored_bytes: entry.stored_bytes,
    })
}

fn load_packed_region(
    pack: &LoadedUnitPack,
    index: usize,
    artifact: [u8; 32],
    path: &str,
) -> Option<RestoredRegion> {
    let entry = pack.entries.get(index)?;
    if entry.artifact != artifact {
        return None;
    }
    let payload = &pack.payload[entry.start..entry.end];
    if crc32fast::hash(payload) != entry.payload_checksum {
        return None;
    }
    let (mut units, mut stream) = rmp_serde::from_slice::<(Vec<UnitFeat>, Stream)>(payload).ok()?;
    super::retarget(&mut units, &mut stream, path);
    Some(RestoredRegion {
        units,
        stream,
        artifact,
        hit: true,
        read_bytes: 0,
        written_bytes: 0,
    })
}

fn update_snapshot(
    snapshot: &mut CachedUnitSnapshot,
    current_sources: &[CachedSourceFile],
    changed_source: Option<&str>,
    current_lines: Option<ContentDigest>,
) -> Option<Vec<Lang>> {
    if let Some(changed) = changed_source {
        let current = current_sources
            .iter()
            .find(|source| source.path == changed)?;
        let stored = snapshot
            .source_files
            .iter_mut()
            .find(|source| source.path == changed)?;
        stored.clone_from(current);
        for context in &mut snapshot.contexts {
            if context.source_path == changed {
                context.source_digest = current.digest;
                context.source_kind = current.source_kind;
            }
        }
    }
    let langs = snapshot
        .contexts
        .iter()
        .map(|context| context.lang)
        .collect();
    if changed_source.is_some() {
        snapshot.global_line_statistics_digest = *current_lines
            .unwrap_or_else(|| source::global_line_statistics_digest(current_sources))
            .as_bytes();
    }
    Some(langs)
}

fn compare_sources(
    previous: &[CachedSourceFile],
    current: &[CachedSourceFile],
) -> Option<Option<String>> {
    if previous.len() != current.len() {
        return None;
    }
    let mut changed = None;
    for (before, after) in previous.iter().zip(current) {
        if before.path != after.path
            || before.logical_path != after.logical_path
            || before.lang != after.lang
        {
            return None;
        }
        if before.digest != after.digest || before.source_kind != after.source_kind {
            if changed.is_some() {
                return None;
            }
            changed = Some(after.path.clone());
        }
    }
    Some(changed)
}

fn rebuild_leaf(
    path: &str,
    snapshot: &CachedUnitSnapshot,
    source_file: &CachedSourceFile,
    cas: &LayeredCas,
    opts: &DetectOptions,
) -> Option<std::collections::BTreeMap<usize, RestoredRegion>> {
    if snapshot.swift_global_active {
        return None;
    }
    let previous = snapshot
        .contexts
        .iter()
        .enumerate()
        .filter(|(_, context)| context.source_path == path)
        .collect::<Vec<_>>();
    if previous.is_empty()
        || previous.iter().any(|(_, context)| {
            context.requires_resolution || context.over_invalidated || context.depended_on
        })
    {
        return None;
    }
    let source = std::fs::read(path).ok()?;
    if source_file.path != path
        || source::analysis_digest(path, source_file.lang, &source).as_bytes()
            != &source_file.digest
    {
        return None;
    }
    if !nose_frontend::source_is_analyzable(Path::new(path), source_file.lang, &source) {
        return None;
    }
    let interner = Interner::new();
    let files = nose_frontend::try_lower_source_regions(
        FileId(0),
        path,
        &source,
        source_file.lang,
        &interner,
    )
    .ok()?;
    if files.len() != previous.len() {
        return None;
    }
    let corpus = Corpus::new(interner, files);
    let summary = nose_frontend::resolution_dependency_summary(&corpus.files, &corpus.interner);
    if summary.swift_global_active || summary.files.len() != previous.len() {
        return None;
    }
    let mut rebuilt = std::collections::BTreeMap::new();
    for (((index, old), il), new) in previous.into_iter().zip(&corpus.files).zip(&summary.files) {
        if old.lang != il.meta.lang
            || old.region_id != portable_il::region_identity(il).hex()
            || old.export_digest != new.export_digest
            || old.resolution_digest != new.resolution_digest
            || new.requires_resolution
            || new.over_invalidated
            || !new.dependencies.is_empty()
        {
            return None;
        }
        let mut current = old.clone();
        current.source_digest = source_file.digest;
        current.source_kind = source_file.source_kind;
        rebuilt.insert(
            index,
            restore_or_build_region(cas, il, &corpus.interner, opts, &current),
        );
    }
    Some(rebuilt)
}

fn load_region(cas: &LayeredCas, artifact: [u8; 32], path: &str) -> Option<RestoredRegion> {
    let key = ArtifactKey {
        stage: ArtifactStage::UnitsSyntax,
        schema: UNITS_SYNTAX_SCHEMA,
        digest: ContentDigest::from_bytes(artifact),
    };
    let entry = cas.load(key)?;
    let (mut units, mut stream) =
        rmp_serde::from_slice::<(Vec<UnitFeat>, Stream)>(&entry.payload).ok()?;
    super::retarget(&mut units, &mut stream, path);
    Some(RestoredRegion {
        units,
        stream,
        artifact,
        hit: true,
        read_bytes: entry.stored_bytes,
        written_bytes: 0,
    })
}

fn restore_or_build_region(
    cas: &LayeredCas,
    il: &nose_il::Il,
    interner: &Interner,
    opts: &DetectOptions,
    context: &super::CachedUnitContext,
) -> RestoredRegion {
    let options = super::options_digest(opts);
    let key = super::unit_artifact_key(context, options);
    if let Some(restored) = load_region(cas, *key.digest.as_bytes(), &il.meta.path) {
        return restored;
    }
    let mut units = nose_detect::units_of_file(il, interner, opts);
    let mut stream = nose_detect::file_stream(il, interner);
    super::retarget(&mut units, &mut stream, "");
    let payload = rmp_serde::to_vec(&(&units, &stream));
    super::retarget(&mut units, &mut stream, &il.meta.path);
    let written_bytes = payload
        .ok()
        .and_then(|payload| cas.store(key, &payload).ok())
        .unwrap_or(0);
    RestoredRegion {
        units,
        stream,
        artifact: *key.digest.as_bytes(),
        hit: false,
        read_bytes: 0,
        written_bytes,
    }
}

fn append_unit_keys(keys: &mut Vec<[u8; 32]>, artifact: [u8; 32], path: &str, units: usize) {
    for index in 0..units {
        let ordinal = (index as u64).to_be_bytes();
        keys.push(
            *ContentDigest::derive(
                b"nose.cached-unit-identity.v1",
                &[&artifact, path.as_bytes(), &ordinal],
            )
            .as_bytes(),
        );
    }
}

fn snapshot_slot(opts: &DetectOptions) -> String {
    format!("unit-snapshot/{}", super::options_digest(opts).hex())
}
