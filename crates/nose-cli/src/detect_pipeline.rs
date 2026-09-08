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

    fn prepare_scores<'a>(
        &'a self,
        units: &[&'a nose_detect::UnitFeat],
    ) -> Option<Box<dyn nose_detect::PreparedScores + 'a>> {
        Some(Box::new(ChannelScores {
            units: units.to_vec(),
            detectors: self
                .detectors
                .iter()
                .map(|detector| PreparedChannel {
                    detector: detector.as_ref(),
                    prepared: detector.prepare_scores(units),
                })
                .collect(),
        }))
    }

    fn score_classes(&self, units: &[nose_detect::UnitFeat]) -> Option<Vec<usize>> {
        let classes = self
            .detectors
            .iter()
            .map(|detector| detector.score_classes(units))
            .collect::<Option<Vec<_>>>()?;
        assert!(classes.iter().all(|ids| ids.len() == units.len()));
        let mut seen = std::collections::HashMap::new();
        Some(
            (0..units.len())
                .map(|i| {
                    let key = classes.iter().map(|ids| ids[i]).collect::<Vec<_>>();
                    *seen.entry(key).or_insert(i)
                })
                .collect(),
        )
    }
}

struct ChannelScores<'a> {
    units: Vec<&'a nose_detect::UnitFeat>,
    detectors: Vec<PreparedChannel<'a>>,
}

struct PreparedChannel<'a> {
    detector: &'a dyn nose_detect::Detector,
    prepared: Option<Box<dyn nose_detect::PreparedScores + 'a>>,
}

impl nose_detect::PreparedScores for ChannelScores<'_> {
    fn row(&self, left: usize, right: &[usize]) -> Vec<f64> {
        let mut scores = vec![0.0_f64; right.len()];
        for PreparedChannel { detector, prepared } in &self.detectors {
            if let Some(prepared) = prepared {
                for (score, next) in scores.iter_mut().zip(prepared.row(left, right)) {
                    *score = score.max(next);
                }
            } else {
                for (score, &right) in scores.iter_mut().zip(right) {
                    *score = score.max(detector.score(self.units[left], self.units[right]));
                }
            }
        }
        scores
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
    if let Some(limit) = candidate_limit(explicit_limit)? {
        nose_detect::ensure_candidate_budget(units, opts, limit)?;
    }
    Ok(())
}

pub(crate) fn candidate_limit(explicit_limit: Option<usize>) -> Result<Option<usize>> {
    if let Some(limit) = explicit_limit {
        anyhow::ensure!(
            limit > 0,
            "--max-candidate-pairs must be a positive integer"
        );
        return Ok(Some(limit));
    }
    let limit = match std::env::var("NOSE_MAX_CANDIDATE_PAIRS") {
        Ok(value) => value
            .parse::<usize>()
            .ok()
            .filter(|&n| n > 0)
            .context("NOSE_MAX_CANDIDATE_PAIRS must be a positive integer")?,
        Err(std::env::VarError::NotPresent) => return Ok(None),
        Err(error) => return Err(error.into()),
    };
    Ok(Some(limit))
}

#[cfg(test)]
mod tests {
    use super::*;
    use nose_detect::Detector as _;

    #[test]
    fn prepared_channels_preserve_the_maximum_of_each_original_scorer() {
        let interner = nose_il::Interner::new();
        let il = nose_frontend::lower_source(nose_il::FileId(0), "f.py",
            b"def square(x):\n    y = x * x\n    return y + 1\ndef cube(x):\n    y = x * x * x\n    return y + 2\n",
            nose_il::Lang::Python, &interner).unwrap();
        let opts = nose_detect::DetectOptions {
            min_tokens: 1,
            min_lines: 1,
            shape_features: true,
            ..Default::default()
        };
        let units = nose_detect::units_of_file(&il, &interner, &opts);
        assert!(units.len() >= 2);
        let detector = ChannelDetector {
            name: "test",
            detectors: vec![
                Box::new(nose_detect::ExactBehaviorDetector),
                Box::new(
                    nose_detect::StructuralDetector::candidates(0.75)
                        .without_exact_behavior()
                        .with_threshold(0.8),
                ),
            ],
        };
        let prepared = detector
            .prepare_scores(&units.iter().collect::<Vec<_>>())
            .unwrap();
        let right = (0..units.len()).collect::<Vec<_>>();
        for left in 0..units.len() {
            assert_eq!(
                prepared.row(left, &right),
                right
                    .iter()
                    .map(|&r| detector.score(&units[left], &units[r]))
                    .collect::<Vec<_>>()
            );
        }
    }
}
