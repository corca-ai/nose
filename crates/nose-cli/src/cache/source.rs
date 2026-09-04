use super::digest::ContentDigest;
use super::portable_il;
use super::store::{ArtifactKey, ArtifactStage, LayeredCas};
use super::{CacheRun, CachedSourceFile};
use inventory::{GitCatalog, LogicalRoots};
use nose_il::{Corpus, FileId, Il, Interner, Lang};
use rayon::prelude::*;
use serde::{Deserialize, Serialize};
use std::path::Path;

mod inventory;

const SOURCE_SNAPSHOT_SCHEMA: u32 = 1;
const RAW_IL_SCHEMA: u32 = 4;

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "kebab-case")]
pub(crate) enum SourceIdentityKind {
    GitBlob,
    ContentSha256,
}

pub(super) struct RawRegion {
    pub(super) il: Il,
    pub(super) raw_digest: ContentDigest,
    pub(super) raw_hit: bool,
    pub(super) source_kind: SourceIdentityKind,
    pub(super) logical_path: String,
    pub(super) source_path: String,
    pub(super) source_digest: ContentDigest,
}

pub(super) struct RawCorpus {
    pub(super) corpus: Corpus,
    pub(super) regions: Vec<RawRegionMetadata>,
    pub(super) discovery_digest: ContentDigest,
    pub(super) global_line_statistics_digest: ContentDigest,
    pub(super) workspace_digest: ContentDigest,
    pub(super) source_hits: usize,
    pub(super) source_misses: usize,
    pub(super) source_files: Vec<CachedSourceFile>,
}

pub(super) struct RawRegionMetadata {
    pub(super) raw_digest: ContentDigest,
    pub(super) raw_hit: bool,
    pub(super) source_kind: SourceIdentityKind,
    pub(super) logical_path: String,
    pub(super) region_id: String,
    pub(super) source_path: String,
    pub(super) source_digest: ContentDigest,
}

#[derive(Serialize, Deserialize)]
struct PortableRawBundle {
    schema: u32,
    regions: Vec<Vec<u8>>,
}

struct SourceResult {
    regions: Vec<RawRegion>,
    source_digest: Option<ContentDigest>,
    source_kind: Option<SourceIdentityKind>,
    logical_path: String,
    lang: Lang,
    snapshot_hit: bool,
}

struct SourceLoad<'a> {
    index: usize,
    path: &'a str,
    lang: Lang,
    logical_path: String,
    git: &'a GitCatalog,
    cas: &'a LayeredCas,
    interner: &'a Interner,
}

struct SourceSnapshot {
    digest: ContentDigest,
    kind: SourceIdentityKind,
    bytes: Option<Vec<u8>>,
}

impl SourceResult {
    fn unreadable(request: &SourceLoad<'_>, snapshot_hit: bool) -> Self {
        Self {
            regions: Vec::new(),
            source_digest: None,
            source_kind: None,
            logical_path: request.logical_path.clone(),
            lang: request.lang,
            snapshot_hit,
        }
    }

    fn from_lowered(
        request: &SourceLoad<'_>,
        snapshot: &SourceSnapshot,
        lowered: Vec<Il>,
        raw_hit: bool,
        snapshot_hit: bool,
    ) -> Self {
        Self {
            regions: lowered
                .into_iter()
                .map(|il| RawRegion {
                    raw_digest: portable_il::semantic_digest(&il, request.interner),
                    il,
                    raw_hit,
                    source_kind: snapshot.kind,
                    logical_path: request.logical_path.clone(),
                    source_path: request.path.to_owned(),
                    source_digest: snapshot.digest,
                })
                .collect(),
            source_digest: Some(snapshot.digest),
            source_kind: Some(snapshot.kind),
            logical_path: request.logical_path.clone(),
            lang: request.lang,
            snapshot_hit,
        }
    }
}

pub(super) fn build_raw_corpus_cached(
    roots: &[&Path],
    exclude: &[String],
    run: &CacheRun,
) -> RawCorpus {
    let paths = crate::timing::time_stage("cache_discover", || {
        nose_frontend::discover_source_inventory(roots, exclude)
    });
    let mut source_errors = paths.errors;
    let paths = paths.paths;
    run.set_portable_il_enabled(paths.len() <= super::MAX_FOREGROUND_PORTABLE_IL_FILES);
    let git = crate::timing::time_stage("cache_git", || GitCatalog::new(roots));
    let logical_roots = LogicalRoots::new(roots);
    let cas = run.cas();
    let interner = Interner::new();
    let results = crate::timing::time_stage("cache_source", || {
        paths
            .par_iter()
            .enumerate()
            .map(|(index, (path, lang))| {
                load_source(SourceLoad {
                    index,
                    path,
                    lang: *lang,
                    logical_path: logical_roots.path(Path::new(path)),
                    git: &git,
                    cas: &cas,
                    interner: &interner,
                })
            })
            .collect::<Vec<_>>()
    });

    for ((path, _), result) in paths.iter().zip(&results) {
        if result.source_digest.is_none() {
            source_errors.push(format!("reading source {path}: source is unreadable"));
        }
    }
    let source_hits = results.iter().filter(|result| result.snapshot_hit).count();
    let source_misses = results.len() - source_hits;
    let source_files = paths
        .iter()
        .zip(&results)
        .filter_map(|((path, _), result)| {
            result.source_digest.map(|digest| CachedSourceFile {
                path: path.clone(),
                logical_path: result.logical_path.clone(),
                digest: *digest.as_bytes(),
                lang: result.lang,
                source_kind: result
                    .source_kind
                    .expect("readable sources have an identity kind"),
            })
        })
        .collect::<Vec<_>>();
    let mut files = Vec::new();
    let mut regions = Vec::new();
    for result in results {
        for region in result.regions {
            regions.push(RawRegionMetadata {
                raw_digest: region.raw_digest,
                raw_hit: region.raw_hit,
                source_kind: region.source_kind,
                logical_path: region.logical_path,
                region_id: portable_il::region_identity(&region.il).hex(),
                source_path: region.source_path,
                source_digest: region.source_digest,
            });
            files.push(region.il);
        }
    }
    let mut corpus = Corpus::new(interner, files);
    corpus.source_errors = source_errors;
    RawCorpus {
        corpus,
        regions,
        discovery_digest: discovery_digest(&source_files),
        global_line_statistics_digest: global_line_statistics_digest(&source_files),
        workspace_digest: workspace_digest(roots),
        source_hits,
        source_misses,
        source_files,
    }
}

pub(super) fn discovery_digest(sources: &[CachedSourceFile]) -> ContentDigest {
    let mut rows = sources
        .iter()
        .map(|source| {
            framed(&[
                source.logical_path.as_bytes(),
                source.lang.name().as_bytes(),
            ])
        })
        .collect::<Vec<_>>();
    rows.sort();
    let rows = rows.iter().map(Vec::as_slice).collect::<Vec<_>>();
    ContentDigest::derive(b"nose.discovery-membership.v1", &rows)
}

pub(super) fn global_line_statistics_digest(sources: &[CachedSourceFile]) -> ContentDigest {
    let mut rows = sources
        .iter()
        .map(|source| framed(&[source.logical_path.as_bytes(), &source.digest]))
        .collect::<Vec<_>>();
    rows.sort();
    let rows = rows.iter().map(Vec::as_slice).collect::<Vec<_>>();
    ContentDigest::derive(b"nose.corpus-global-line-statistics.v1", &rows)
}

/// Resolve exact source identities without parsing or restoring IL. This is the
/// admission check for the bounded warm-unit path; any unreadable source or
/// membership mismatch makes that path fall back to the full pipeline.
pub(super) fn discover_source_files(
    roots: &[&Path],
    exclude: &[String],
) -> Option<Vec<CachedSourceFile>> {
    let inventory = nose_frontend::discover_source_inventory(roots, exclude);
    if !inventory.errors.is_empty() {
        return None;
    }
    let paths = inventory.paths;
    let git = GitCatalog::new(roots);
    let logical_roots = LogicalRoots::new(roots);
    paths
        .into_par_iter()
        .map(|(path, lang)| {
            let clean_blob = git.clean_blob(Path::new(&path));
            let (digest, source_kind) = match clean_blob {
                Some(blob) if std::fs::File::open(&path).is_ok() => (
                    ContentDigest::derive(
                        b"nose.source-snapshot.git-blob.v1",
                        &[lang.name().as_bytes(), blob.as_bytes()],
                    ),
                    SourceIdentityKind::GitBlob,
                ),
                _ => (
                    portable_il::source_digest(lang, &std::fs::read(&path).ok()?),
                    SourceIdentityKind::ContentSha256,
                ),
            };
            Some(CachedSourceFile {
                logical_path: logical_roots.path(Path::new(&path)),
                path,
                digest: *digest.as_bytes(),
                lang,
                source_kind,
            })
        })
        .collect()
}

pub(super) fn workspace_digest(roots: &[&Path]) -> ContentDigest {
    let rows = roots
        .iter()
        .map(|root| {
            std::fs::canonicalize(root)
                .unwrap_or_else(|_| root.to_path_buf())
                .to_string_lossy()
                .into_owned()
        })
        .collect::<Vec<_>>();
    let rows = rows.iter().map(String::as_bytes).collect::<Vec<_>>();
    ContentDigest::derive(b"nose.workspace-state.v1", &rows)
}

fn load_source(request: SourceLoad<'_>) -> SourceResult {
    let Some(mut snapshot) = source_snapshot(&request) else {
        return SourceResult::unreadable(&request, false);
    };
    let snapshot_key = ArtifactKey::derive(
        ArtifactStage::SourceSnapshot,
        SOURCE_SNAPSHOT_SCHEMA,
        &[snapshot.digest.as_bytes()],
    );
    let snapshot_hit = request.cas.load(snapshot_key).is_some();
    let raw_key = ArtifactKey::derive(
        ArtifactStage::RawIl,
        RAW_IL_SCHEMA,
        &[snapshot.digest.as_bytes()],
    );
    if let Some(restored) = restore_raw_bundle(&request, raw_key) {
        return SourceResult::from_lowered(&request, &snapshot, restored, true, snapshot_hit);
    }

    let source = match snapshot.bytes.take() {
        Some(source) => source,
        None => match std::fs::read(request.path) {
            Ok(source) => source,
            Err(_) => return SourceResult::unreadable(&request, snapshot_hit),
        },
    };
    let lowered =
        if nose_frontend::source_is_analyzable(Path::new(request.path), request.lang, &source) {
            nose_frontend::lower_source_regions(
                FileId(request.index as u32),
                request.path,
                &source,
                request.lang,
                request.interner,
            )
        } else {
            Vec::new()
        };
    store_raw_bundle(&request, raw_key, snapshot_key, &lowered);
    SourceResult::from_lowered(&request, &snapshot, lowered, false, snapshot_hit)
}

fn source_snapshot(request: &SourceLoad<'_>) -> Option<SourceSnapshot> {
    match request.git.clean_blob(Path::new(request.path)) {
        Some(blob) if std::fs::File::open(request.path).is_ok() => Some(SourceSnapshot {
            digest: ContentDigest::derive(
                b"nose.source-snapshot.git-blob.v1",
                &[request.lang.name().as_bytes(), blob.as_bytes()],
            ),
            kind: SourceIdentityKind::GitBlob,
            bytes: None,
        }),
        _ => {
            let bytes = std::fs::read(request.path).ok()?;
            Some(SourceSnapshot {
                digest: portable_il::source_digest(request.lang, &bytes),
                kind: SourceIdentityKind::ContentSha256,
                bytes: Some(bytes),
            })
        }
    }
}

fn restore_raw_bundle(request: &SourceLoad<'_>, raw_key: ArtifactKey) -> Option<Vec<Il>> {
    let entry = request.cas.load(raw_key)?;
    let bundle = rmp_serde::from_slice::<PortableRawBundle>(&entry.payload).ok()?;
    (bundle.schema == RAW_IL_SCHEMA).then_some(())?;
    bundle
        .regions
        .iter()
        .map(|bytes| {
            portable_il::decode(
                bytes,
                request.interner,
                FileId(request.index as u32),
                request.path.to_owned(),
            )
        })
        .collect::<anyhow::Result<Vec<_>>>()
        .ok()
}

fn store_raw_bundle(
    request: &SourceLoad<'_>,
    raw_key: ArtifactKey,
    snapshot_key: ArtifactKey,
    lowered: &[Il],
) {
    if request.cas.writes_portable_il() {
        let bundle = PortableRawBundle {
            schema: RAW_IL_SCHEMA,
            regions: lowered
                .iter()
                .filter_map(|il| portable_il::encode(il, request.interner).ok())
                .collect(),
        };
        if bundle.regions.len() == lowered.len() {
            if let Ok(payload) = rmp_serde::to_vec(&bundle) {
                let _ = request.cas.store(raw_key, &payload);
                let _ = request.cas.store(snapshot_key, b"nose-source-snapshot-v1");
            }
        }
    }
}

fn framed(components: &[&[u8]]) -> Vec<u8> {
    let mut out = Vec::new();
    for component in components {
        out.extend_from_slice(&(component.len() as u64).to_be_bytes());
        out.extend_from_slice(component);
    }
    out
}

#[cfg(test)]
mod tests;
