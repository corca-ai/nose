//! Process-boundary persistence for `nose-detect`'s opaque incremental state.

use super::digest::ContentDigest;
use super::CacheRun;
use nose_detect::{DetectOptions, IncrementalDetectionState};

const STATE_SCHEMA: u32 = 2;

pub(crate) struct DetectionCacheIdentity {
    query: ContentDigest,
}

impl DetectionCacheIdentity {
    pub(crate) fn new(
        _workspace: [u8; 32],
        semantic_packs: [u8; 32],
        opts: &DetectOptions,
        detector: &dyn nose_detect::Detector,
    ) -> Self {
        let options = options_bytes(opts);
        let environment = nose_detect::candidate_config_identity();
        let scoring = serde_json::to_vec(&opts.scoring).expect("finite score config");
        Self {
            query: ContentDigest::derive(
                b"nose.incremental-detection-query.v2",
                &[
                    &semantic_packs,
                    detector.name().as_bytes(),
                    &options,
                    &environment,
                    &scoring,
                ],
            ),
        }
    }

    fn slot(&self) -> String {
        format!("detection/{}", self.query.hex())
    }
}

pub(crate) fn load_detection_state(
    run: &CacheRun,
    identity: &DetectionCacheIdentity,
) -> Option<IncrementalDetectionState> {
    let bytes = run.load(&identity.slot(), STATE_SCHEMA)?;
    rmp_serde::from_slice(&bytes).ok()
}

pub(crate) fn store_detection_state(
    run: &CacheRun,
    identity: &DetectionCacheIdentity,
    state: &IncrementalDetectionState,
) {
    let Ok(bytes) = rmp_serde::to_vec(state) else {
        return;
    };
    run.store(&identity.slot(), STATE_SCHEMA, &bytes);
}

fn options_bytes(opts: &DetectOptions) -> Vec<u8> {
    let values = [
        opts.min_lines as u64,
        opts.min_tokens as u64,
        opts.threshold.to_bits(),
        opts.minhash_k as u64,
        opts.bands as u64,
        opts.cfg_norm as u64,
        opts.dce as u64,
        opts.jaccard_weight.to_bits(),
        opts.block_units as u64,
        opts.contiguous_min_tokens as u64,
        opts.contiguous_min_lines as u64,
        opts.contiguous as u64,
        opts.structural as u64,
        opts.value_candidates as u64,
        opts.value_lsh_candidates as u64,
        opts.shape_candidates as u64,
        opts.shape_features as u64,
        opts.connected_witnesses as u64,
        opts.abstraction_witnesses as u64,
        opts.emit_pairs as u64,
    ];
    values.into_iter().flat_map(u64::to_be_bytes).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn thresholds_and_modes_separate_detection_state() {
        let mut first = DetectOptions::default();
        let detector = nose_detect::StructuralDetector::strict(first.jaccard_weight);
        let a = DetectionCacheIdentity::new([1; 32], [2; 32], &first, &detector);
        first.threshold = 0.7;
        let b = DetectionCacheIdentity::new([1; 32], [2; 32], &first, &detector);
        assert_ne!(a.query, b.query);
    }

    #[test]
    fn effective_scoring_parameters_separate_detection_state() {
        let mut opts = DetectOptions::default();
        let detector = nose_detect::StructuralDetector::strict(opts.jaccard_weight);
        let first = DetectionCacheIdentity::new([1; 32], [2; 32], &opts, &detector);
        opts.scoring = nose_detect::ScoreConfig::from_lookup(|key| {
            (key == "NOSE_CAND_VJ").then(|| "0.95".into())
        })
        .unwrap();
        let second = DetectionCacheIdentity::new([1; 32], [2; 32], &opts, &detector);
        assert_ne!(first.query, second.query);
    }
}
