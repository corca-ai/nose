use crate::path_utils::paths_as_refs;
use crate::query_options::{DetectionChannels, DetectionMode, DIVERGENCE_DEFAULT_MODES};
use anyhow::{Context, Result};
use std::path::PathBuf;

struct ChannelDetector {
    name: &'static str,
    detectors: Vec<Box<dyn nose_detect::Detector>>,
}

impl nose_detect::Detector for ChannelDetector {
    fn name(&self) -> &str {
        self.name
    }

    fn score(&self, a: &nose_detect::UnitFeat, b: &nose_detect::UnitFeat) -> f64 {
        self.detectors
            .iter()
            .map(|d| d.score(a, b))
            .fold(0.0, f64::max)
    }

    fn connected_witness(
        &self,
        a: &nose_detect::UnitFeat,
        b: &nose_detect::UnitFeat,
        a_seed: nose_detect::LineSpan,
        b_seed: nose_detect::LineSpan,
    ) -> Option<nose_detect::ConnectedWitness> {
        self.detectors
            .iter()
            .filter_map(|detector| detector.connected_witness(a, b, a_seed, b_seed))
            .max_by_key(|witness| (witness.mapped_nodes, std::cmp::Reverse(witness.holes)))
    }
}

/// Lower + detect + rank clone families for divergence's base tree. This keeps
/// divergence's conservative default channel policy (`syntax,semantic`) explicit;
/// query use their own dataset construction and default to `syntax,semantic,near`.
pub(crate) fn detect_divergence_base_families(
    paths: &[PathBuf],
    exclude: &[String],
    mode: Vec<DetectionMode>,
    cfg_mode: Vec<DetectionMode>,
    min_tokens: usize,
    min_lines: u32,
) -> Result<Vec<nose_detect::RefactorFamily>> {
    validate_exclude_globs(exclude)?;
    let refs = paths_as_refs(paths);
    let channels = DetectionChannels::resolve(mode, cfg_mode, DIVERGENCE_DEFAULT_MODES)?;
    let opts = detection_options(channels, min_tokens, min_lines);
    let detector = detection_engine(channels, &opts);
    let corpus = nose_frontend::lower_corpus_filtered(&refs, exclude);
    let report = nose_detect::detect(&corpus, &opts, detector.as_ref());
    let mut families = nose_detect::rank_families(&report);
    if channels.abstraction_only() {
        families.retain(|f| f.abstraction_witness.is_some());
    }
    // The graded witness is NOT attached here: `divergence` enriches only the *flagged*
    // families (a small subset of a diff) in `flag_divergences`, not every near family
    // in the repo — enriching all of them on every gate run would be wasted work.
    Ok(families)
}

/// Detection options for the resolved query channels — shared by `analysis` and `divergence`.
pub(crate) fn detection_options(
    channels: DetectionChannels,
    min_tokens: usize,
    min_lines: u32,
) -> nose_detect::DetectOptions {
    nose_detect::DetectOptions {
        threshold: channels.threshold(),
        min_lines,
        min_tokens,
        contiguous_min_tokens: min_tokens,
        contiguous_min_lines: min_lines,
        structural: channels.structural(),
        contiguous: channels.syntax,
        // Near also generates VALUE candidates so behaviorally-convergent but shape-divergent
        // pairs (async `.then` ≡ await, impure loop ≡ comprehension) reach the candidate scorer —
        // they share no shape band, so shape-LSH alone would never propose them.
        value_candidates: channels.semantic || channels.near || channels.abstraction,
        shape_candidates: channels.near || channels.abstraction,
        shape_features: channels.near || channels.abstraction,
        abstraction_witnesses: channels.abstraction,
        emit_pairs: false,
        ..Default::default()
    }
}

pub(crate) fn validate_exclude_globs(exclude: &[String]) -> Result<()> {
    if exclude.is_empty() {
        return Ok(());
    }
    let mut builder = ignore::overrides::OverrideBuilder::new(".");
    for glob in exclude {
        builder
            .add(&format!("!{glob}"))
            .with_context(|| format!("invalid exclude glob {glob:?}"))?;
    }
    builder.build().context("building exclude glob matcher")?;
    Ok(())
}

pub(crate) fn detection_engine(
    channels: DetectionChannels,
    opts: &nose_detect::DetectOptions,
) -> Box<dyn nose_detect::Detector> {
    let mut detectors: Vec<Box<dyn nose_detect::Detector>> = Vec::new();
    if channels.semantic {
        detectors.push(Box::new(nose_detect::ExactBehaviorDetector));
    }
    if channels.near || channels.abstraction {
        detectors.push(Box::new(
            nose_detect::StructuralDetector::candidates(opts.jaccard_weight)
                .without_exact_behavior()
                .with_threshold(opts.threshold),
        ));
    }

    match detectors.len() {
        0 => Box::new(nose_detect::CopyPasteDetector),
        1 => detectors.pop().expect("one detector"),
        _ => Box::new(ChannelDetector {
            name: if channels.abstraction && !channels.near {
                "semantic+abstraction"
            } else if channels.abstraction {
                "semantic+near+abstraction"
            } else {
                "semantic+near"
            },
            detectors,
        }),
    }
}
