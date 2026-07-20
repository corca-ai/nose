//! Process-boundary persistence for `nose-detect`'s opaque incremental state.

use super::digest::ContentDigest;
use super::CacheRun;
use nose_detect::{DetectOptions, IncrementalDetectionState};

const STATE_SCHEMA: u32 = 1;

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
        let environment = influential_environment();
        Self {
            query: ContentDigest::derive(
                b"nose.incremental-detection-query.v1",
                &[
                    &semantic_packs,
                    detector.name().as_bytes(),
                    &options,
                    &environment,
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
        opts.shape_candidates as u64,
        opts.shape_features as u64,
        opts.connected_witnesses as u64,
        opts.abstraction_witnesses as u64,
        opts.emit_pairs as u64,
    ];
    values.into_iter().flat_map(u64::to_be_bytes).collect()
}

fn influential_environment() -> Vec<u8> {
    let mut rows = std::env::vars_os()
        .filter_map(|(name, value)| {
            let name = name.to_string_lossy();
            name.starts_with("NOSE_").then(|| {
                let mut row = name.as_bytes().to_vec();
                row.push(0);
                row.extend_from_slice(value.to_string_lossy().as_bytes());
                row
            })
        })
        .collect::<Vec<_>>();
    rows.sort();
    rows.into_iter().flatten().collect()
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
}
