//! Exact whole-input reuse for complete saved analyses, including large corpora.
use super::{
    digest::ContentDigest, source, ArtifactKey, ArtifactStage, CacheRun, CachedSourceFile,
};
use crate::cli_args::QueryArgs;
use crate::query_dataset::QuerySettings;
use std::collections::BTreeMap;
use std::path::PathBuf;

pub(crate) struct AnalysisCaptureCache {
    dir: PathBuf,
    max_bytes: u64,
    roots: Vec<PathBuf>,
    exclude: Vec<String>,
    policy: nose_frontend::InlineSuppression,
    configuration: ContentDigest,
}

impl AnalysisCaptureCache {
    pub(crate) fn prepare(
        args: &QueryArgs,
        settings: &QuerySettings,
        packs: &nose_semantics::SemanticPackSet,
    ) -> Option<Self> {
        let dir = args.cache_dir.as_ref()?;
        // External evidence producers can read inputs beyond the source corpus.
        super::semantic_packs_allow_fast_units(packs).then_some(())?;
        let mut environment = BTreeMap::new();
        for (key, value) in std::env::vars_os() {
            let Some(key) = key.to_str().filter(|key| key.starts_with("NOSE_")) else {
                continue;
            };
            if !matches!(key, "NOSE_TIME" | "NOSE_CACHE_STATS") {
                environment.insert(key.to_owned(), value.to_str()?.to_owned());
            }
        }
        // Development binaries can have the same version but different semantics.
        let engine = ContentDigest::sha256(&std::fs::read(std::env::current_exe().ok()?).ok()?);
        let channels = settings.channels;
        let configuration = serde_json::to_vec(&(
            &args.paths,
            std::env::current_dir().ok()?,
            settings.min_members,
            settings.min_value,
            settings.min_lines,
            settings.min_tokens,
            settings.sort,
            [
                channels.syntax,
                channels.semantic,
                channels.near,
                channels.abstraction,
            ],
            channels.threshold().to_bits(),
            settings.inline_suppression.as_str(),
            &settings.exclude,
            super::semantic_pack_digest(packs),
            packs
                .project_lock()
                .map(|lock| lock.decision_digest().to_string()),
            crate::detect_pipeline::candidate_limit(args.max_candidate_pairs).ok()?,
            environment,
            engine,
        ))
        .ok()?;
        Some(Self {
            dir: dir.clone(),
            max_bytes: settings.cache_max_bytes,
            roots: args.paths.clone(),
            exclude: settings.exclude.clone(),
            policy: settings.inline_suppression,
            configuration: ContentDigest::sha256(&configuration),
        })
    }

    pub(crate) fn load(&self) -> Option<Vec<u8>> {
        // A first capture pays no redundant inventory scan. Its input identity
        // comes from the source bytes actually consumed by the ordinary engine.
        self.dir
            .join("cas-v2/analysis-capture")
            .is_dir()
            .then_some(())?;
        let sources = self.current_sources()?;
        let run = CacheRun::with_limit(&self.dir, self.max_bytes);
        run.cas().load(self.key(sources)).map(|entry| entry.payload)
    }

    pub(crate) fn store(&self, analyzed_sources: Option<ContentDigest>, bytes: &[u8]) {
        let Some(sources) = analyzed_sources else {
            return;
        };
        // Do not publish a result under an inventory read before analysis if
        // the ordinary engine consumed different bytes, or sources changed later.
        if self.current_sources() != Some(sources) {
            return;
        }
        let run = CacheRun::with_limit(&self.dir, self.max_bytes);
        let _ = run.cas().store(self.key(sources), bytes);
        super::enforce_run_budget(run);
    }

    fn current_sources(&self) -> Option<ContentDigest> {
        let roots = self.roots.iter().map(PathBuf::as_path).collect::<Vec<_>>();
        let sources = source::discover_source_files(&roots, &self.exclude, self.policy)?;
        Some(capture_source_digest(&sources))
    }

    fn key(&self, sources: ContentDigest) -> ArtifactKey {
        ArtifactKey::derive(
            ArtifactStage::AnalysisCapture,
            1,
            &[self.configuration.as_bytes(), sources.as_bytes()],
        )
    }
}

pub(crate) fn capture_source_digest(sources: &[CachedSourceFile]) -> ContentDigest {
    let mut rows = sources
        .iter()
        .map(|source| {
            ContentDigest::derive(
                b"nose.capture-source.v1",
                &[
                    source.path.as_bytes(),
                    source.lang.name().as_bytes(),
                    &source.digest,
                    &[u8::from(source.skip_reason.is_some())],
                    source.skip_reason.as_deref().unwrap_or("").as_bytes(),
                ],
            )
        })
        .collect::<Vec<_>>();
    rows.sort();
    let rows = rows
        .iter()
        .map(|row| row.as_bytes().as_slice())
        .collect::<Vec<_>>();
    ContentDigest::derive(b"nose.capture-corpus.v1", &rows)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicU64, Ordering};

    struct Project(PathBuf);
    impl Drop for Project {
        fn drop(&mut self) {
            let _ = std::fs::remove_dir_all(&self.0);
        }
    }

    fn cache() -> (Project, AnalysisCaptureCache) {
        static NEXT: AtomicU64 = AtomicU64::new(0);
        let root = std::env::temp_dir().join(format!(
            "nose_capture_cache_{}_{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        std::fs::create_dir_all(&root).unwrap();
        std::fs::write(root.join("source.py"), "def f(x):\n    return x + 1\n").unwrap();
        let cache = AnalysisCaptureCache {
            dir: root.join("cache"),
            max_bytes: 1024 * 1024,
            roots: vec![root.clone()],
            exclude: Vec::new(),
            policy: nose_frontend::InlineSuppression::Respect,
            configuration: ContentDigest::sha256(b"test configuration"),
        };
        (Project(root), cache)
    }

    #[test]
    fn changed_sources_cannot_publish_an_analysis_under_old_inputs() {
        let (project, cache) = cache();
        let old = cache.current_sources().unwrap();
        cache.store(Some(old), b"original");
        assert_eq!(cache.load().unwrap(), b"original");
        std::fs::write(project.0.join("source.py"), "def f(x):\n    return x + 2\n").unwrap();
        cache.store(Some(old), b"stale analysis");
        assert!(cache.load().is_none());
        cache.store(None, b"missing provenance");
        assert!(cache.load().is_none());
        let current = cache.current_sources().unwrap();
        cache.store(Some(current), b"current analysis");
        assert_eq!(cache.load().unwrap(), b"current analysis");
    }

    #[test]
    fn captures_follow_the_managed_storage_budget() {
        let (_project, mut cache) = cache();
        cache.max_bytes = 1;
        cache.store(cache.current_sources(), b"larger than the budget");
        assert!(cache.load().is_none());
        assert!(super::super::admin::status(&cache.dir, cache.max_bytes).bytes <= 1);
    }
}
