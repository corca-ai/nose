use super::{append_unit_keys, rebuild_leaf, update_snapshot, FastCachedUnits, RestoredRegion};
use crate::cache::{source, CacheRun, CachedLineContext, CachedSourceFile, InvalidationReport};
use nose_detect::{DetectOptions, Stream, UnitFeat};
use nose_il::Lang;
use std::ops::Range;
use std::path::Path;
use std::sync::Arc;

pub(crate) struct FastUnitSession {
    units: Vec<UnitFeat>,
    unit_keys: Vec<[u8; 32]>,
    streams: Vec<Stream>,
    region_unit_counts: Vec<usize>,
    files: usize,
    langs: Vec<Lang>,
    workspace_digest: [u8; 32],
    semantic_pack_digest: [u8; 32],
    source_files: Arc<Vec<CachedSourceFile>>,
    source_path_indexes: std::collections::HashMap<std::path::PathBuf, usize>,
    snapshot: super::super::CachedUnitSnapshot,
    run: CacheRun,
    initial_report: Option<InvalidationReport>,
}

pub(crate) struct FastUnitSessionRefresh {
    pub(crate) invalidation: InvalidationReport,
    pub(crate) source_set_digest: String,
}

impl FastUnitSession {
    pub(crate) fn from_fast(fast: FastCachedUnits) -> Self {
        let FastCachedUnits {
            cached,
            report,
            workspace_digest,
            semantic_pack_digest,
            source_files,
            run,
            langs,
            snapshot,
            region_unit_counts,
        } = fast;
        let source_path_indexes = source_files
            .iter()
            .enumerate()
            .map(|(index, source)| (path_key(Path::new(&source.path)), index))
            .collect();
        Self {
            units: cached.units,
            unit_keys: cached.unit_keys,
            streams: cached.streams,
            region_unit_counts,
            files: cached.files,
            langs,
            workspace_digest,
            semantic_pack_digest,
            source_files: Arc::new(source_files),
            source_path_indexes,
            snapshot,
            run,
            initial_report: Some(report),
        }
    }

    pub(crate) fn refresh_leaf(
        &mut self,
        path: &Path,
        opts: &DetectOptions,
    ) -> Option<FastUnitSessionRefresh> {
        let source_index = *self.source_path_indexes.get(&path_key(path))?;
        let path = self.source_files[source_index].path.clone();
        let current = refreshed_source(&self.source_files[source_index])?;
        if current.digest == self.source_files[source_index].digest
            && current.source_kind == self.source_files[source_index].source_kind
        {
            return None;
        }

        let cas = self.run.cas();
        let replacements = rebuild_leaf(&path, &self.snapshot, &cas, opts)?;
        Arc::make_mut(&mut self.source_files)[source_index] = current;
        let current_lines = source::global_line_statistics_digest(&self.source_files);
        let invalidation = super::super::resolved::fast_invalidation_report_for_leaf(
            &self.snapshot,
            &self.source_files,
            &path,
            current_lines,
        );
        self.apply_replacements(replacements)?;
        update_snapshot(
            &mut self.snapshot,
            &self.source_files,
            Some(&path),
            Some(current_lines),
        )?;
        Some(FastUnitSessionRefresh {
            invalidation,
            source_set_digest: current_lines.hex(),
        })
    }

    fn apply_replacements(
        &mut self,
        replacements: std::collections::BTreeMap<usize, RestoredRegion>,
    ) -> Option<()> {
        for (region, restored) in replacements {
            let range = self.region_range(region)?;
            let unit_count = restored.units.len();
            let mut keys = Vec::with_capacity(unit_count);
            append_unit_keys(
                &mut keys,
                restored.artifact,
                &self.snapshot.contexts.get(region)?.region_path,
                unit_count,
            );
            self.units.splice(range.clone(), restored.units);
            self.unit_keys.splice(range, keys);
            *self.streams.get_mut(region)? = restored.stream;
            *self.region_unit_counts.get_mut(region)? = unit_count;
            *self.snapshot.artifacts.get_mut(region)? = restored.artifact;
        }
        Some(())
    }

    fn region_range(&self, region: usize) -> Option<Range<usize>> {
        let start = self.region_unit_counts.get(..region)?.iter().sum::<usize>();
        let len = *self.region_unit_counts.get(region)?;
        Some(start..start + len)
    }

    pub(crate) fn units(&self) -> &[UnitFeat] {
        &self.units
    }

    pub(crate) fn unit_keys(&self) -> &[[u8; 32]] {
        &self.unit_keys
    }

    pub(crate) fn streams(&self) -> &[Stream] {
        &self.streams
    }

    pub(crate) fn files(&self) -> usize {
        self.files
    }

    pub(crate) fn langs(&self) -> &[Lang] {
        &self.langs
    }

    pub(crate) fn workspace_digest(&self) -> [u8; 32] {
        self.workspace_digest
    }

    pub(crate) fn semantic_pack_digest(&self) -> [u8; 32] {
        self.semantic_pack_digest
    }

    pub(crate) fn line_context(&self) -> CachedLineContext {
        CachedLineContext {
            source_files: Arc::clone(&self.source_files),
            run: self.run.clone(),
        }
    }

    pub(crate) fn run(&self) -> &CacheRun {
        &self.run
    }

    pub(crate) fn source_set_digest(&self) -> String {
        source::global_line_statistics_digest(&self.source_files).hex()
    }

    pub(crate) fn take_initial_report(&mut self) -> Option<InvalidationReport> {
        self.initial_report.take()
    }

    pub(crate) fn source_path_for_event(&self, path: &Path) -> Option<std::path::PathBuf> {
        let index = *self.source_path_indexes.get(&path_key(path))?;
        Some(self.source_files.get(index)?.path.as_str().into())
    }
}

fn refreshed_source(previous: &CachedSourceFile) -> Option<CachedSourceFile> {
    let bytes = std::fs::read(&previous.path).ok()?;
    let digest = super::super::portable_il::source_digest(previous.lang, &bytes);
    Some(CachedSourceFile {
        path: previous.path.clone(),
        logical_path: previous.logical_path.clone(),
        digest: *digest.as_bytes(),
        lang: previous.lang,
        source_kind: source::SourceIdentityKind::ContentSha256,
    })
}

fn path_key(path: &Path) -> std::path::PathBuf {
    std::fs::canonicalize(path).unwrap_or_else(|_| {
        if path.is_absolute() {
            path.to_path_buf()
        } else {
            std::env::current_dir()
                .map(|cwd| cwd.join(path))
                .unwrap_or_else(|_| path.to_path_buf())
        }
    })
}
