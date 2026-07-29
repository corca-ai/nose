use super::gates::large_test_file;
use super::roots::{collect_unit_roots, value_fingerprint_context_for_roots};
use crate::strict_exact::{strict_exact_safe_tree, StrictFacts};
use nose_il::{Il, Interner, NodeId};

/// Above this many normalized nodes a file is treated as pathological for witness
/// purposes (generated/minified): the graded witness is best-effort enrichment, so it
/// is skipped rather than paying an outsized cost on a file no one refactors by hand.
const WITNESS_MAX_FILE_NODES: usize = 60_000;

/// Export the value DAGs of the units at the given `(start_line, end_line)` spans, for
/// the graded witness. `il` is the file's raw IL; the result is aligned with `wanted`.
/// The per-file resolution context is built once and shared across requested roots.
pub fn unit_dags_at(
    il: &Il,
    interner: &Interner,
    opts: &crate::DetectOptions,
    wanted: &[(u32, u32)],
) -> Vec<Option<(nose_normalize::ValueDag, bool)>> {
    if large_test_file(il) {
        return vec![None; wanted.len()];
    }
    let norm_opts = nose_normalize::NormalizeOptions {
        cfg_norm: opts.cfg_norm,
        dce: opts.dce,
        ..Default::default()
    };
    let normalized = nose_normalize::normalize(il, interner, &norm_opts);
    if normalized.nodes.len() > WITNESS_MAX_FILE_NODES {
        return vec![None; wanted.len()];
    }
    let (roots, _) = collect_unit_roots(&normalized, interner, opts.block_units);
    let facts = StrictFacts::collect(&normalized, interner);
    let context = value_fingerprint_context_for_roots(&normalized, interner, roots.len());
    let referents = nose_normalize::FileReferents::new(&normalized, interner);

    let mut by_lines: rustc_hash::FxHashMap<(u32, u32), NodeId> = rustc_hash::FxHashMap::default();
    for unit_root in &roots {
        let span = normalized.node(unit_root.root).span;
        by_lines
            .entry((span.start_line, span.end_line))
            .or_insert(unit_root.root);
    }
    wanted
        .iter()
        .map(|&span| {
            let root = *by_lines.get(&span)?;
            let exact_safe = strict_exact_safe_tree(&normalized, interner, &facts, root);
            let dag = nose_normalize::value_dag(
                &normalized,
                root,
                interner,
                context.as_ref(),
                &referents,
            );
            Some((dag, exact_safe))
        })
        .collect()
}
