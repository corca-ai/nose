//! Content-addressed Markdown preprocessing and complete corpus results.
use super::digest::ContentDigest;
use super::{ArtifactKey, ArtifactStage, CacheRun};
use nose_markdown::detect::{detect_prepared, PreparedDocument};
use nose_markdown::{Family, Options};
use rayon::prelude::*;
use std::path::Path;

pub(crate) fn detect(
    docs: &[(String, String)],
    root: Option<&Path>,
    max_bytes: u64,
) -> Vec<Family> {
    let Some(root) = root else {
        return nose_markdown::detect(docs, &Options::default());
    };
    let run = CacheRun::with_limit(root, max_bytes);
    let cas = run.cas();
    let digests = docs
        .iter()
        .map(|(_, source)| {
            ContentDigest::derive(b"nose.markdown-document.v1", &[source.as_bytes()])
        })
        .collect::<Vec<_>>();
    let mut components = vec![b"nose.markdown-report.v1".as_slice()];
    for ((path, _), digest) in docs.iter().zip(&digests) {
        components.push(path.as_bytes());
        components.push(digest.as_bytes());
    }
    let result_key = ArtifactKey::derive(ArtifactStage::StateRecord, 1, &components);
    if let Some(families) = cas
        .load(result_key)
        .and_then(|r| rmp_serde::from_slice::<Vec<Family>>(&r.payload).ok())
    {
        if std::env::var_os("NOSE_CACHE_STATS").is_some() {
            eprintln!("  [markdown-cache] report_hit=true");
        }
        return families;
    }
    let hits = std::sync::atomic::AtomicUsize::new(0);
    let prepared = docs
        .par_iter()
        .zip(&digests)
        .map(|((path, source), digest)| {
            let key = ArtifactKey::derive(
                ArtifactStage::StateRecord,
                1,
                &[b"nose.markdown-document.v1", digest.as_bytes()],
            );
            let mut document = match cas
                .load(key)
                .and_then(|r| rmp_serde::from_slice::<PreparedDocument>(&r.payload).ok())
            {
                Some(document) => {
                    hits.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
                    document
                }
                None => {
                    let document = PreparedDocument::new("", source);
                    if let Ok(payload) = rmp_serde::to_vec(&document) {
                        let _ = cas.store(key, &payload);
                    }
                    document
                }
            };
            document.rebind(path);
            document
        })
        .collect::<Vec<_>>();
    let families = detect_prepared(&prepared, &Options::default());
    if let Ok(payload) = rmp_serde::to_vec(&families) {
        let _ = cas.store(result_key, &payload);
    }
    if std::env::var_os("NOSE_CACHE_STATS").is_some() {
        eprintln!(
            "  [markdown-cache] report_hit=false document_hits={} documents={}",
            hits.load(std::sync::atomic::Ordering::Relaxed),
            docs.len()
        );
    }
    drop(cas);
    super::enforce_run_budget(run);
    families
}
