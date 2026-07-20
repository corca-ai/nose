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
use serde::{Deserialize, Serialize};
use std::path::Path;

const SNAPSHOT_SCHEMA: u32 = 1;

pub(crate) struct FastCachedUnits {
    pub(crate) cached: CachedUnits,
    pub(crate) report: InvalidationReport,
    pub(crate) workspace_digest: [u8; 32],
    pub(crate) semantic_pack_digest: [u8; 32],
    pub(crate) source_files: Vec<CachedSourceFile>,
    pub(crate) run: CacheRun,
    pub(crate) langs: Vec<Lang>,
}

#[derive(Deserialize, Serialize)]
struct StoredSnapshot {
    schema: u32,
    snapshot: CachedUnitSnapshot,
}

struct RestoredRegion {
    units: Vec<UnitFeat>,
    stream: Stream,
    artifact: [u8; 32],
    hit: bool,
    read_bytes: u64,
    written_bytes: u64,
}

pub(super) fn store_snapshot(
    run: &CacheRun,
    opts: &DetectOptions,
    mut snapshot: CachedUnitSnapshot,
    artifacts: &[[u8; 32]],
) {
    if !snapshot.fast_safe || snapshot.contexts.len() != artifacts.len() {
        return;
    }
    snapshot.artifacts = artifacts.to_vec();
    let stored = StoredSnapshot {
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
    let current_sources = source::discover_source_files(roots, exclude);
    let changed_source = compare_sources(&snapshot.source_files, &current_sources)?;
    let report = super::resolved::fast_invalidation_report(
        &snapshot,
        &current_sources,
        changed_source.as_deref(),
    );

    let cas = run.cas();
    let mut replacements = match changed_source.as_deref() {
        Some(path) => Some(rebuild_leaf(path, &snapshot, &cas, opts)?),
        None => None,
    };
    let mut all_units = Vec::new();
    let mut unit_keys = Vec::new();
    let mut streams = Vec::with_capacity(snapshot.contexts.len());
    let mut hits = 0;
    let mut misses = 0;
    let mut read_bytes = 0;
    let mut written_bytes = 0;
    let mut artifacts = Vec::with_capacity(snapshot.contexts.len());

    for (index, context) in snapshot.contexts.iter().enumerate() {
        let restored = replacements
            .as_mut()
            .and_then(|replacements| replacements.remove(&index))
            .or_else(|| load_region(&cas, snapshot.artifacts[index], &context.region_path))?;
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
        all_units.extend(restored.units);
        streams.push(restored.stream);
        artifacts.push(restored.artifact);
    }
    if replacements.is_some_and(|replacements| !replacements.is_empty()) {
        return None;
    }

    let langs = update_snapshot(&mut snapshot, &current_sources, changed_source.as_deref())?;
    store_snapshot(&run, opts, snapshot, &artifacts);

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
    })
}

fn update_snapshot(
    snapshot: &mut CachedUnitSnapshot,
    current_sources: &[CachedSourceFile],
    changed_source: Option<&str>,
) -> Option<Vec<Lang>> {
    if let Some(changed) = changed_source {
        let current = current_sources
            .iter()
            .find(|source| source.path == changed)?;
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
    snapshot.source_files.clone_from_slice(current_sources);
    snapshot.discovery_digest = *source::discovery_digest(current_sources).as_bytes();
    snapshot.global_line_statistics_digest =
        *source::global_line_statistics_digest(current_sources).as_bytes();
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
    let source_file = snapshot
        .source_files
        .iter()
        .find(|source| source.path == path)?;
    if !nose_frontend::source_is_analyzable(Path::new(path), source_file.lang, &source) {
        return None;
    }
    let interner = Interner::new();
    let files =
        nose_frontend::lower_source_regions(FileId(0), path, &source, source_file.lang, &interner);
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
        rebuilt.insert(
            index,
            restore_or_build_region(cas, il, &corpus.interner, opts, old.resolution_digest),
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
    resolution_digest: [u8; 32],
) -> RestoredRegion {
    let resolved = portable_il::semantic_digest(il, interner);
    let options = super::options_digest(opts);
    let key = ArtifactKey::derive(
        ArtifactStage::UnitsSyntax,
        UNITS_SYNTAX_SCHEMA,
        &[resolved.as_bytes(), options.as_bytes(), &resolution_digest],
    );
    if let Some(restored) = load_region(cas, *key.digest.as_bytes(), &il.meta.path) {
        return restored;
    }
    let mut units = nose_detect::units_of_file(il, interner, opts);
    let mut stream = nose_detect::file_stream(il, interner);
    super::retarget(&mut units, &mut stream, "");
    let payload = rmp_serde::to_vec_named(&(&units, &stream));
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
