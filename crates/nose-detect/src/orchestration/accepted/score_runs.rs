//! Repeated scores retain the exact result of sequential IEEE-754 addition.
use std::ops::Range;

pub(super) struct ScoreRuns(Vec<(usize, f64)>);

impl ScoreRuns {
    pub(super) fn new(targets: &[(usize, f64)]) -> Option<Self> {
        if targets.len() < 32 {
            return None;
        }
        let mut runs = Vec::new();
        let mut end = 0;
        for run in targets.chunk_by(|a, b| a.1.to_bits() == b.1.to_bits()) {
            end += run.len();
            runs.push((end, run[0].1));
        }
        // Short runs are cheaper to fold and need no retained index.
        (runs.len() <= targets.len() / 8).then_some(Self(runs))
    }

    pub(super) fn sum(&self, range: Range<usize>, mut sum: f64) -> f64 {
        let first = self.0.partition_point(|&(end, _)| end <= range.start);
        let mut start = range.start;
        for &(end, value) in &self.0[first..] {
            let end = end.min(range.end);
            sum = repeated_add(sum, value, end - start);
            if end == range.end {
                break;
            }
            start = end;
        }
        sum
    }
}

fn repeated_add(mut sum: f64, value: f64, mut count: usize) -> f64 {
    if count < 8 || !sum.is_finite() || !value.is_finite() || sum < 0.0 || value < 0.0 {
        return (0..count).fold(sum, |sum, _| sum + value);
    }
    const FRACTION: u64 = (1 << 52) - 1;
    while count != 0 {
        let initial = sum.to_bits();
        sum += value;
        count -= 1;
        if count == 0 || !sum.is_finite() {
            return sum;
        }
        let before = sum.to_bits();
        sum += value;
        count -= 1;
        let after = sum.to_bits();
        if !sum.is_finite() {
            return sum;
        }
        if initial >> 52 != before >> 52 || before >> 52 != after >> 52 {
            continue;
        }
        // Within one exponent interval the spacing is constant. After two
        // additions a half-ULP tie has reached even parity; the bit increment
        // remains constant until the next exponent boundary. Subnormals share
        // one spacing too. Cross-boundary additions always run normally.
        let increment = after - before;
        if increment == 0 {
            return sum;
        }
        let room = (FRACTION - (after & FRACTION)) / increment;
        let jump = count.min(usize::try_from(room).unwrap_or(usize::MAX));
        sum = f64::from_bits(after + increment * jump as u64);
        count -= jump;
    }
    sum
}

#[cfg(test)]
mod tests {
    use super::*;

    fn assert_sum(sum: f64, value: f64, count: usize) {
        let expected = (0..count).fold(sum, |sum, _| sum + value);
        assert_eq!(
            repeated_add(sum, value, count).to_bits(),
            expected.to_bits(),
            "sum={sum:?} value={value:?} count={count}"
        );
    }

    #[test]
    fn repeated_add_matches_ordered_rounding_across_spacing_and_tie_boundaries() {
        let tiny = f64::from_bits(1);
        for sum in [
            0.0,
            -0.0,
            tiny,
            f64::MIN_POSITIVE - tiny,
            f64::MIN_POSITIVE,
            0.5,
            1.0,
            f64::from_bits(1.0f64.to_bits() - 1),
            9_007_199_254_740_990.0,
            9_007_199_254_740_992.0,
            f64::MAX,
        ] {
            for value in [
                0.0,
                -0.0,
                tiny,
                f64::MIN_POSITIVE,
                f64::EPSILON / 2.0,
                f64::EPSILON * 1.5,
                0.1,
                0.3,
                0.812345,
                1.0,
                f64::MAX,
            ] {
                for count in [0, 1, 2, 3, 7, 8, 31, 4097] {
                    assert_sum(sum, value, count);
                }
            }
        }
        for (sum, value) in [
            (-1.0, 0.1),
            (1.0, -0.1),
            (f64::INFINITY, 1.0),
            (1.0, f64::NAN),
            (f64::NAN, 1.0),
        ] {
            assert_sum(sum, value, 100);
        }
    }

    #[test]
    fn repeated_add_matches_seeded_full_exponent_samples() {
        let mut state = 0xbfe3_857a_492d_610c_u64;
        let mut next = || {
            state ^= state << 13;
            state ^= state >> 7;
            state ^= state << 17;
            state
        };
        for _ in 0..20_000 {
            let sum = f64::from_bits(next() & 0x7fef_ffff_ffff_ffff);
            let value = f64::from_bits(next() & 0x7fef_ffff_ffff_ffff);
            assert_sum(sum, value, (next() % 1024) as usize);
        }
    }

    #[test]
    fn half_spacing_steps_settle_after_entering_each_exponent_interval() {
        for exponent in (-1022..=1023).step_by(17) {
            let base = ((exponent + 1023) as u64) << 52;
            let spacing = f64::from_bits(base) * f64::EPSILON;
            for bits in [base - 2, base - 1, base, base + 1, base + 2] {
                for multiplier in [0.5, 1.5, 2.5, 7.5, 127.5] {
                    assert_sum(f64::from_bits(bits), spacing * multiplier, 1025);
                }
            }
        }
    }

    #[test]
    fn indexed_slices_keep_score_changes_and_exclusion_boundaries() {
        let values = [0.3, 1.0, 0.812345, 0.1];
        let targets = (0..256).map(|i| (i, values[i / 64])).collect::<Vec<_>>();
        let index = ScoreRuns::new(&targets).unwrap();
        for start in 0..256 {
            for end in [start, (start + 17).min(256), 256] {
                let expected = targets[start..end].iter().fold(0.7, |sum, &(_, x)| sum + x);
                assert_eq!(index.sum(start..end, 0.7).to_bits(), expected.to_bits());
            }
        }
        let alternating = (0..256).map(|i| (i, values[i % 4])).collect::<Vec<_>>();
        assert!(ScoreRuns::new(&alternating).is_none());
    }
}
