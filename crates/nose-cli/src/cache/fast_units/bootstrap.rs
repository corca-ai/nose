//! Reuse the ordinary corpus/unit builders without requiring a persisted seed.
use super::*;

pub(crate) fn open_session(
    roots: &[&Path],
    exclude: &[String],
    dir: &Path,
    packs: &nose_semantics::SemanticPackSet,
    max_bytes: u64,
    opts: &DetectOptions,
) -> anyhow::Result<Option<FastUnitSession>> {
    if !super::super::semantic_packs_allow_fast_units(packs) {
        return Ok(None);
    }
    let fast = match super::super::try_build_units_fast(roots, exclude, dir, packs, max_bytes, opts)
    {
        Some(fast) => fast,
        None => {
            let mut cached =
                super::super::build_corpus_cached(roots, exclude, dir, packs, max_bytes);
            cached.corpus.ensure_complete()?;
            let mut snapshot = cached.unit_snapshot;
            let langs = snapshot
                .contexts
                .iter()
                .map(|context| context.lang)
                .collect();
            let built = super::super::build_units_cached_inner(
                &mut cached.corpus,
                opts,
                &cached.run,
                Some(&snapshot.contexts),
            );
            snapshot.artifacts = built.region_artifacts;
            snapshot.unit_pack = built.unit_pack;
            store_snapshot(&cached.run, opts, &snapshot);
            let mut counts = rustc_hash::FxHashMap::default();
            for unit in &built.public.units {
                *counts.entry(unit.path.as_str()).or_insert(0) += 1;
            }
            let region_unit_counts = snapshot
                .contexts
                .iter()
                .map(|context| {
                    counts
                        .get(context.region_path.as_str())
                        .copied()
                        .unwrap_or(0)
                })
                .collect();
            FastCachedUnits {
                cached: built.public,
                report: cached.report,
                workspace_digest: cached.workspace_digest,
                semantic_pack_digest: cached.semantic_pack_digest,
                source_files: cached.source_files,
                run: cached.run,
                langs,
                snapshot,
                region_unit_counts,
            }
        }
    };
    Ok(Some(FastUnitSession::from_fast(fast)))
}
