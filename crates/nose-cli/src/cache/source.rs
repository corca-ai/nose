use super::digest::ContentDigest;
use super::portable_il;
use super::store::{ArtifactKey, ArtifactStage, LayeredCas};
use super::{CacheRun, CachedSourceFile};
use nose_il::{Corpus, FileId, Il, Interner, Lang};
use rayon::prelude::*;
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};
use std::process::Command;

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

pub(super) fn build_raw_corpus_cached(
    roots: &[&Path],
    exclude: &[String],
    run: &CacheRun,
) -> RawCorpus {
    let paths = crate::timing::time_stage("cache_discover", || {
        nose_frontend::discover_unique_paths(roots, exclude)
    });
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
                load_source(
                    index,
                    path,
                    *lang,
                    logical_roots.path(Path::new(path)),
                    &git,
                    &cas,
                    &interner,
                )
            })
            .collect::<Vec<_>>()
    });

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
    RawCorpus {
        corpus: Corpus::new(interner, files),
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
pub(super) fn discover_source_files(roots: &[&Path], exclude: &[String]) -> Vec<CachedSourceFile> {
    let paths = nose_frontend::discover_unique_paths(roots, exclude);
    let git = GitCatalog::new(roots);
    let logical_roots = LogicalRoots::new(roots);
    paths
        .into_par_iter()
        .filter_map(|(path, lang)| {
            let clean_blob = git.clean_blob(Path::new(&path));
            let (digest, source_kind) = match clean_blob {
                Some(blob) if std::fs::metadata(&path).is_ok() => (
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

// Keep the hit and miss paths together: both must construct exactly the same
// region metadata before the parallel results are flattened into one corpus.
#[allow(clippy::too_many_lines)]
fn load_source(
    index: usize,
    path: &str,
    lang: Lang,
    logical_path: String,
    git: &GitCatalog,
    cas: &LayeredCas,
    interner: &Interner,
) -> SourceResult {
    let clean_blob = git.clean_blob(Path::new(path));
    let (source_digest, source_kind, source) = match clean_blob {
        Some(blob) if std::fs::metadata(path).is_ok() => (
            ContentDigest::derive(
                b"nose.source-snapshot.git-blob.v1",
                &[lang.name().as_bytes(), blob.as_bytes()],
            ),
            SourceIdentityKind::GitBlob,
            None,
        ),
        _ => match std::fs::read(path) {
            Ok(source) => (
                portable_il::source_digest(lang, &source),
                SourceIdentityKind::ContentSha256,
                Some(source),
            ),
            Err(_) => {
                return SourceResult {
                    regions: Vec::new(),
                    source_digest: None,
                    source_kind: None,
                    logical_path,
                    lang,
                    snapshot_hit: false,
                };
            }
        },
    };
    let snapshot_key = ArtifactKey::derive(
        ArtifactStage::SourceSnapshot,
        SOURCE_SNAPSHOT_SCHEMA,
        &[source_digest.as_bytes()],
    );
    let snapshot_hit = cas.load(snapshot_key).is_some();
    let raw_key = ArtifactKey::derive(
        ArtifactStage::RawIl,
        RAW_IL_SCHEMA,
        &[source_digest.as_bytes()],
    );
    if let Some(entry) = cas.load(raw_key) {
        if let Ok(bundle) = rmp_serde::from_slice::<PortableRawBundle>(&entry.payload) {
            if bundle.schema == RAW_IL_SCHEMA {
                let decoded = bundle
                    .regions
                    .iter()
                    .map(|bytes| {
                        portable_il::decode(bytes, interner, FileId(index as u32), path.to_owned())
                    })
                    .collect::<anyhow::Result<Vec<_>>>();
                if let Ok(decoded) = decoded {
                    return SourceResult {
                        regions: decoded
                            .into_iter()
                            .map(|il| RawRegion {
                                raw_digest: portable_il::semantic_digest(&il, interner),
                                il,
                                raw_hit: true,
                                source_kind,
                                logical_path: logical_path.clone(),
                                source_path: path.to_owned(),
                                source_digest,
                            })
                            .collect(),
                        source_digest: Some(source_digest),
                        source_kind: Some(source_kind),
                        logical_path,
                        lang,
                        snapshot_hit,
                    };
                }
            }
        }
    }

    let source = match source {
        Some(source) => source,
        None => match std::fs::read(path) {
            Ok(source) => source,
            Err(_) => {
                return SourceResult {
                    regions: Vec::new(),
                    source_digest: None,
                    source_kind: None,
                    logical_path,
                    lang,
                    snapshot_hit,
                };
            }
        },
    };
    let lowered = if nose_frontend::source_is_analyzable(Path::new(path), lang, &source) {
        nose_frontend::lower_source_regions(FileId(index as u32), path, &source, lang, interner)
    } else {
        Vec::new()
    };
    if cas.writes_portable_il() {
        let bundle = PortableRawBundle {
            schema: RAW_IL_SCHEMA,
            regions: lowered
                .iter()
                .filter_map(|il| portable_il::encode(il, interner).ok())
                .collect(),
        };
        if bundle.regions.len() == lowered.len() {
            if let Ok(payload) = rmp_serde::to_vec(&bundle) {
                let _ = cas.store(raw_key, &payload);
                let _ = cas.store(snapshot_key, b"nose-source-snapshot-v1");
            }
        }
    }
    SourceResult {
        regions: lowered
            .into_iter()
            .map(|il| RawRegion {
                raw_digest: portable_il::semantic_digest(&il, interner),
                il,
                raw_hit: false,
                source_kind,
                logical_path: logical_path.clone(),
                source_path: path.to_owned(),
                source_digest,
            })
            .collect(),
        source_digest: Some(source_digest),
        source_kind: Some(source_kind),
        logical_path,
        lang,
        snapshot_hit,
    }
}

struct LogicalRoot {
    lexical_base: PathBuf,
    canonical_base: PathBuf,
}

struct LogicalRoots {
    roots: Vec<LogicalRoot>,
    cwd: PathBuf,
}

impl LogicalRoots {
    fn new(roots: &[&Path]) -> Self {
        let cwd = std::env::current_dir().unwrap_or_default();
        Self {
            roots: roots
                .iter()
                .map(|root| {
                    let lexical = if root.is_absolute() {
                        root.to_path_buf()
                    } else {
                        cwd.join(root)
                    };
                    let canonical = std::fs::canonicalize(root).unwrap_or_else(|_| lexical.clone());
                    LogicalRoot {
                        lexical_base: root_base(lexical),
                        canonical_base: root_base(canonical),
                    }
                })
                .collect(),
            cwd,
        }
    }

    fn path(&self, path: &Path) -> String {
        let lexical = if path.is_absolute() {
            path.to_path_buf()
        } else {
            self.cwd.join(path)
        };
        for (index, root) in self.roots.iter().enumerate() {
            if let Ok(relative) = lexical.strip_prefix(&root.lexical_base) {
                return format!("{index}:{}", relative.to_string_lossy());
            }
        }
        let canonical = std::fs::canonicalize(path).unwrap_or(lexical);
        for (index, root) in self.roots.iter().enumerate() {
            if let Ok(relative) = canonical.strip_prefix(&root.canonical_base) {
                return format!("{index}:{}", relative.to_string_lossy());
            }
        }
        canonical.to_string_lossy().to_string()
    }
}

fn root_base(root: PathBuf) -> PathBuf {
    if root.is_file() {
        root.parent().unwrap_or(&root).to_path_buf()
    } else {
        root
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

struct GitInventory {
    root: PathBuf,
    tracked: BTreeMap<PathBuf, String>,
    dirty: BTreeSet<PathBuf>,
}

struct GitCatalog {
    inventories: Vec<GitInventory>,
    cwd: PathBuf,
}

impl GitCatalog {
    fn new(roots: &[&Path]) -> Self {
        let mut git_roots: BTreeMap<PathBuf, BTreeSet<PathBuf>> = BTreeMap::new();
        for root in roots {
            let base = if root.is_file() {
                root.parent().unwrap_or(root)
            } else {
                root
            };
            let Some(git_root) = find_git_root(base) else {
                continue;
            };
            let scope = std::fs::canonicalize(base)
                .ok()
                .and_then(|root| root.strip_prefix(&git_root).ok().map(Path::to_path_buf))
                .unwrap_or_default();
            git_roots.entry(git_root).or_default().insert(scope);
        }
        Self {
            inventories: git_roots
                .into_iter()
                .filter_map(|(root, scopes)| GitInventory::load(root, &scopes))
                .collect(),
            cwd: std::env::current_dir().unwrap_or_default(),
        }
    }

    fn clean_blob(&self, path: &Path) -> Option<&str> {
        let lexical = if path.is_absolute() {
            path.to_path_buf()
        } else {
            self.cwd.join(path)
        };
        for git in &self.inventories {
            if let Ok(relative) = lexical.strip_prefix(&git.root) {
                return (!git.dirty.contains(relative))
                    .then(|| git.tracked.get(relative).map(String::as_str))
                    .flatten();
            }
        }
        let canonical = std::fs::canonicalize(path).ok()?;
        self.inventories.iter().find_map(|git| {
            let relative = canonical.strip_prefix(&git.root).ok()?;
            (!git.dirty.contains(relative))
                .then(|| git.tracked.get(relative).map(String::as_str))
                .flatten()
        })
    }
}

impl GitInventory {
    fn load(root: PathBuf, scopes: &BTreeSet<PathBuf>) -> Option<Self> {
        let root = std::fs::canonicalize(root).ok()?;
        let mut listed = Command::new("git");
        listed.args(["-C", &root.to_string_lossy(), "ls-files", "--stage", "-z"]);
        append_scopes(&mut listed, scopes);
        let listed = listed.output().ok()?;
        if !listed.status.success() {
            return None;
        }
        if listed.stdout.is_empty() {
            return Some(Self {
                root,
                tracked: BTreeMap::new(),
                dirty: BTreeSet::new(),
            });
        }
        let mut status = Command::new("git");
        status.args([
            "-C",
            &root.to_string_lossy(),
            "status",
            "--porcelain=v1",
            "-z",
            "--untracked-files=no",
        ]);
        append_scopes(&mut status, scopes);
        let status = status.output().ok()?;
        if !listed.status.success() || !status.status.success() {
            return None;
        }
        let mut tracked = BTreeMap::new();
        for record in listed
            .stdout
            .split(|byte| *byte == 0)
            .filter(|row| !row.is_empty())
        {
            let Some(tab) = record.iter().position(|byte| *byte == b'\t') else {
                continue;
            };
            let header = String::from_utf8_lossy(&record[..tab]);
            let mut fields = header.split_whitespace();
            let _mode = fields.next();
            let Some(oid) = fields.next() else { continue };
            if fields.next() != Some("0") {
                continue;
            }
            tracked.insert(
                PathBuf::from(String::from_utf8_lossy(&record[tab + 1..]).as_ref()),
                oid.to_owned(),
            );
        }
        let mut dirty = BTreeSet::new();
        let records = status
            .stdout
            .split(|byte| *byte == 0)
            .filter(|row| !row.is_empty())
            .collect::<Vec<_>>();
        let mut index = 0;
        while index < records.len() {
            let record = records[index];
            if record.len() >= 4 {
                dirty.insert(PathBuf::from(
                    String::from_utf8_lossy(&record[3..]).as_ref(),
                ));
                if matches!(record[0], b'R' | b'C') || matches!(record[1], b'R' | b'C') {
                    index += 1;
                    if let Some(old) = records.get(index) {
                        dirty.insert(PathBuf::from(String::from_utf8_lossy(old).as_ref()));
                    }
                }
            }
            index += 1;
        }
        Some(Self {
            root,
            tracked,
            dirty,
        })
    }
}

fn find_git_root(base: &Path) -> Option<PathBuf> {
    let canonical = std::fs::canonicalize(base).ok()?;
    let start = if canonical.is_file() {
        canonical.parent()?
    } else {
        canonical.as_path()
    };
    start
        .ancestors()
        .find(|ancestor| ancestor.join(".git").exists())
        .map(Path::to_path_buf)
}

fn append_scopes(command: &mut Command, scopes: &BTreeSet<PathBuf>) {
    if scopes.iter().any(|scope| scope.as_os_str().is_empty()) {
        return;
    }
    command.arg("--");
    command.args(scopes);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn same_size_and_mtime_never_override_content_identity() {
        let first = portable_il::source_digest(Lang::Python, b"return x + 1\n");
        let second = portable_il::source_digest(Lang::Python, b"return x - 1\n");
        assert_ne!(first, second);
    }
}
