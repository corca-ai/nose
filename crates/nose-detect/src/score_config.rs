use serde::Serialize;

/// Validated scoring parameters, frozen for one analysis. Library callers use
/// deterministic defaults; CLI research overrides are parsed at the boundary.
#[derive(Clone, Copy, Debug, PartialEq, Serialize)]
pub struct ScoreConfig {
    pub(crate) strict_weights: [f64; 3],
    pub(crate) candidate_weights: [f64; 3],
    pub(crate) candidate_value_accept: f64,
    pub(crate) data_heavy_ratio: f64,
    pub(crate) data_heavy_count: usize,
    pub(crate) return_base: f64,
    anchor_floor: f64,
    anchor_cap: f64,
    anchor_half: f64,
    anchor_min_weight: u32,
}

impl Default for ScoreConfig {
    fn default() -> Self {
        Self {
            strict_weights: [0.5, 0.3, 0.2],
            candidate_weights: [0.3, 0.5, 0.2],
            candidate_value_accept: 0.90,
            data_heavy_ratio: 0.20,
            data_heavy_count: 25,
            return_base: 0.80,
            anchor_floor: 0.72,
            anchor_cap: 0.90,
            anchor_half: 60.0,
            anchor_min_weight: nose_normalize::ANCHOR_MIN_WEIGHT,
        }
    }
}

impl ScoreConfig {
    pub fn from_environment() -> Result<Self, crate::InvalidDetectOptions> {
        let mut config = Self::from_lookup(|key| match std::env::var(key) {
            Ok(value) => Some(value),
            Err(std::env::VarError::NotPresent) => None,
            Err(std::env::VarError::NotUnicode(_)) => Some(String::new()),
        })?;
        // Use the same effective collection floor as normalization.
        config.anchor_min_weight = nose_normalize::anchor_min_weight();
        Ok(config)
    }

    pub fn from_lookup(
        lookup: impl Fn(&str) -> Option<String>,
    ) -> Result<Self, crate::InvalidDetectOptions> {
        let mut config = Self::default();
        let [wv, ws, wr] = &mut config.strict_weights;
        let [cwv, cws, cwr] = &mut config.candidate_weights;
        for (key, target) in [
            ("NOSE_WV", wv),
            ("NOSE_WS", ws),
            ("NOSE_WR", wr),
            ("NOSE_CWV", cwv),
            ("NOSE_CWS", cws),
            ("NOSE_CWR", cwr),
            ("NOSE_CAND_VJ", &mut config.candidate_value_accept),
            ("NOSE_DH", &mut config.data_heavy_ratio),
            ("NOSE_RET", &mut config.return_base),
            ("NOSE_ANCHOR_SCORE", &mut config.anchor_floor),
            ("NOSE_ANCHOR_SCORE_CAP", &mut config.anchor_cap),
            ("NOSE_ANCHOR_SCORE_REF", &mut config.anchor_half),
        ] {
            if let Some(value) = lookup(key) {
                *target = value.parse().map_err(|_| invalid(key))?;
            }
            let valid = if key == "NOSE_ANCHOR_SCORE_REF" {
                target.is_finite() && *target > 0.0
            } else {
                target.is_finite() && (0.0..=1.0).contains(target)
            };
            if !valid {
                return Err(invalid(key));
            }
        }
        if let Some(value) = lookup("NOSE_DHN") {
            config.data_heavy_count = value.parse().map_err(|_| invalid("NOSE_DHN"))?;
        }
        for weights in [config.strict_weights, config.candidate_weights] {
            if (weights.iter().sum::<f64>() - 1.0).abs() > 1e-9 {
                return Err(crate::InvalidDetectOptions(
                    "scoring weights must sum to 1".into(),
                ));
            }
        }
        if config.anchor_floor > config.anchor_cap {
            return Err(crate::InvalidDetectOptions(
                "anchor score floor exceeds cap".into(),
            ));
        }
        Ok(config)
    }

    pub(crate) fn anchor_score(&self, weight: u32) -> f64 {
        let extra = f64::from(weight.saturating_sub(self.anchor_min_weight));
        self.anchor_floor
            + (self.anchor_cap - self.anchor_floor) * extra / (extra + self.anchor_half)
    }
}

fn invalid(key: &str) -> crate::InvalidDetectOptions {
    crate::InvalidDetectOptions(format!("invalid scoring parameter {key}"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn independent_runs_do_not_share_first_use_parameters() {
        let first = ScoreConfig::from_lookup(|_| None).unwrap();
        let second =
            ScoreConfig::from_lookup(|key| (key == "NOSE_CAND_VJ").then(|| "0.95".into())).unwrap();
        assert_eq!(first.candidate_value_accept, 0.90);
        assert_eq!(second.candidate_value_accept, 0.95);
        assert_ne!(first, second);
    }

    #[test]
    fn invalid_overrides_are_errors() {
        for value in ["NaN", "inf", "-0.1", "1.1", "bad"] {
            assert!(
                ScoreConfig::from_lookup(|key| (key == "NOSE_CAND_VJ").then(|| value.into()))
                    .is_err()
            );
        }
        assert!(ScoreConfig::from_lookup(|key| (key == "NOSE_WV").then(|| "0.9".into())).is_err());
    }
}
