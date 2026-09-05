use crate::query_options::DetectionChannels;
use anyhow::{Context, Result};

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
}

/// Detection options for the resolved query channels — shared by `analysis` and `divergence`.
pub(crate) fn detection_options(
    channels: DetectionChannels,
    min_tokens: usize,
    min_lines: u32,
) -> Result<nose_detect::DetectOptions> {
    let opts = nose_detect::DetectOptions {
        scoring: nose_detect::ScoreConfig::from_environment()?,
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
        value_lsh_candidates: channels.near || channels.abstraction,
        shape_candidates: channels.near || channels.abstraction,
        shape_features: channels.near || channels.abstraction,
        connected_witnesses: channels.near || channels.abstraction,
        abstraction_witnesses: channels.abstraction,
        emit_pairs: false,
        ..Default::default()
    };
    opts.validate()?;
    Ok(opts)
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
                .with_scoring(opts.scoring)
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

pub(crate) fn ensure_candidate_budget(
    units: &[nose_detect::UnitFeat],
    opts: &nose_detect::DetectOptions,
    explicit_limit: Option<usize>,
) -> Result<()> {
    let limit = candidate_limit(explicit_limit)?;
    nose_detect::ensure_candidate_budget(units, opts, limit)?;
    Ok(())
}

pub(crate) fn candidate_limit(explicit_limit: Option<usize>) -> Result<usize> {
    if let Some(limit) = explicit_limit {
        anyhow::ensure!(
            limit > 0,
            "--max-candidate-pairs must be a positive integer"
        );
        return Ok(limit);
    }
    let limit = match std::env::var("NOSE_MAX_CANDIDATE_PAIRS") {
        Ok(value) => value
            .parse::<usize>()
            .ok()
            .filter(|&n| n > 0)
            .context("NOSE_MAX_CANDIDATE_PAIRS must be a positive integer")?,
        Err(std::env::VarError::NotPresent) => 16_000_000,
        Err(error) => return Err(error.into()),
    };
    Ok(limit)
}
