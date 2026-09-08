//! Conservative suffix bounds avoid extending runs that cannot meet the source floors.
use super::{Stream, LINE_RANGE_BLOCK};

#[derive(Clone, Copy)]
struct Suffix {
    start: u32,
    end: u32,
    has_operation: bool,
}

pub(super) struct RunFloors {
    suffixes: Vec<Suffix>,
    tokens: usize,
}

impl RunFloors {
    pub(super) fn new(stream: &Stream) -> Self {
        let mut bound = Suffix {
            start: u32::MAX,
            end: 0,
            has_operation: false,
        };
        let mut suffixes = Vec::with_capacity(stream.tags.len().div_ceil(LINE_RANGE_BLOCK));
        for block in (0..stream.tags.len().div_ceil(LINE_RANGE_BLOCK)).rev() {
            let lo = block * LINE_RANGE_BLOCK;
            let hi = (lo + LINE_RANGE_BLOCK).min(stream.tags.len());
            bound.start = bound.start.min(*stream.start[lo..hi].iter().min().unwrap());
            bound.end = bound.end.max(*stream.end[lo..hi].iter().max().unwrap());
            bound.has_operation |= stream.op[lo..hi].iter().any(|&op| op);
            suffixes.push(bound);
        }
        suffixes.reverse();
        Self {
            suffixes,
            tokens: stream.tags.len(),
        }
    }

    pub(super) fn could_match(&self, position: usize, min_tokens: usize, min_lines: u32) -> bool {
        let Some(bound) = self.suffixes.get(position / LINE_RANGE_BLOCK) else {
            return false;
        };
        self.tokens.saturating_sub(position) >= min_tokens
            && bound.has_operation
            && bound.end.saturating_sub(bound.start).saturating_add(1) >= min_lines
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::contiguous::{detect, tests::mk};

    #[test]
    fn rejected_one_line_stream_still_seeds_a_later_multiline_copy() {
        let mut one_line = mk("minified.js", vec![7; 8_192]);
        one_line.start.fill(1);
        one_line.end.fill(1);
        let floors = RunFloors::new(&one_line);
        assert!((0..one_line.tags.len()).all(|i| !floors.could_match(i, 24, 5)));
        let multiline = mk("formatted.js", vec![7; 8_192]);
        let groups = detect(&[one_line, multiline], 24, 5, false).0;
        assert_eq!(groups.len(), 1);
        assert_eq!(groups[0].members.len(), 2);
        assert_eq!(groups[0].members[0].file, "minified.js");
        assert_eq!(groups[0].members[1].file, "formatted.js");
    }

    #[test]
    fn bounds_never_reject_a_qualifying_run_with_nonmonotonic_source_spans() {
        let mut stream = mk("nested.py", (0..137).collect());
        for i in 0..stream.tags.len() {
            stream.start[i] = (i * 7 % 17 + 1) as u32;
            stream.end[i] = stream.start[i] + (i % 3) as u32;
            stream.op[i] = i % 23 == 0;
        }
        let floors = RunFloors::new(&stream);
        for lo in 0..stream.tags.len() {
            for hi in lo + 1..=stream.tags.len() {
                let lines = stream.end[lo..hi].iter().max().unwrap()
                    - stream.start[lo..hi].iter().min().unwrap()
                    + 1;
                if hi - lo >= 10 && lines >= 5 && stream.op[lo..hi].iter().any(|&op| op) {
                    assert!(floors.could_match(lo, 10, 5));
                }
            }
        }
        stream.op.fill(false);
        assert!(!RunFloors::new(&stream).could_match(0, 10, 5));
        assert!(!RunFloors::new(&mk("empty.py", vec![])).could_match(0, 10, 5));
    }
}
